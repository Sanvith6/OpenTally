use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use crate::lan::models::{
    LanLoginRequest, LanLoginResponse, LanLogoutResponse, LanSession, LanSessionDto,
};
use crate::security::models::{ActionPermission, UserRole};
use crate::security::service::SecurityService;
use chrono::{Duration, Utc};
use rand_core::{OsRng, RngCore};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Generates a cryptographically secure 256-bit (32-byte) hex-encoded Bearer token.
pub fn generate_bearer_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let mut s = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// In-memory session store for connected LAN clients.
#[derive(Clone, Default)]
pub struct LanSessionStore {
    sessions: Arc<RwLock<HashMap<String, LanSession>>>,
}

impl LanSessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Stores a new active session.
    pub fn insert(&self, session: LanSession) {
        if let Ok(mut lock) = self.sessions.write() {
            lock.insert(session.token.clone(), session);
        }
    }

    /// Retrieves an active session by Bearer token, checking expiration.
    pub fn get_by_token(&self, token: &str) -> Option<LanSession> {
        let lock = self.sessions.read().ok()?;
        let session = lock.get(token)?;
        if session.expires_at < Utc::now() {
            None
        } else {
            Some(session.clone())
        }
    }

    /// Updates last active timestamp.
    pub fn touch_session(&self, token: &str) {
        if let Ok(mut lock) = self.sessions.write() {
            if let Some(session) = lock.get_mut(token) {
                session.last_active_at = Utc::now();
            }
        }
    }

    /// Removes a session by Bearer token.
    pub fn remove_by_token(&self, token: &str) -> Option<LanSession> {
        if let Ok(mut lock) = self.sessions.write() {
            lock.remove(token)
        } else {
            None
        }
    }

    /// Revokes a session by session ID.
    pub fn revoke_by_id(&self, session_id: &str) -> Option<LanSession> {
        if let Ok(mut lock) = self.sessions.write() {
            let token_to_remove = lock
                .iter()
                .find(|(_, s)| s.session_id == session_id)
                .map(|(t, _)| t.clone());
            if let Some(token) = token_to_remove {
                return lock.remove(&token);
            }
        }
        None
    }

    /// Evicts all sessions for a deactivated user in a company.
    pub fn evict_user(&self, company_id: &str, user_id: &str) {
        if let Ok(mut lock) = self.sessions.write() {
            let tokens_to_remove: Vec<String> = lock
                .iter()
                .filter(|(_, s)| s.company_id == company_id && s.user_id == user_id)
                .map(|(t, _)| t.clone())
                .collect();
            for token in tokens_to_remove {
                lock.remove(&token);
            }
        }
    }

    /// Lists all active (non-expired) sessions for Host UI monitoring.
    pub fn list_active(&self) -> Vec<LanSessionDto> {
        let now = Utc::now();
        if let Ok(lock) = self.sessions.read() {
            lock.values()
                .filter(|s| s.expires_at >= now)
                .map(|s| LanSessionDto {
                    session_id: s.session_id.clone(),
                    company_id: s.company_id.clone(),
                    user_id: s.user_id.clone(),
                    username: s.username.clone(),
                    role: s.role.to_string(),
                    client_ip: s.client_ip.clone(),
                    created_at: s.created_at.to_rfc3339(),
                    expires_at: s.expires_at.to_rfc3339(),
                    last_active_at: s.last_active_at.to_rfc3339(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

/// Handles LAN client login request with Argon2id RBAC or open company default.
pub async fn handle_lan_login(
    pool: &Pool<Sqlite>,
    session_store: &LanSessionStore,
    req: LanLoginRequest,
    client_ip: Option<String>,
) -> Result<LanLoginResponse, OpenTallyError> {
    // 1. Verify company exists and is not archived
    let comp_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM companies WHERE id = ? AND status != 'archived'")
            .bind(&req.company_id)
            .fetch_optional(pool)
            .await
            .map_err(OpenTallyError::Database)?;

    if comp_name.is_none() {
        return Err(OpenTallyError::NotFound(format!(
            "Company '{}' does not exist or has been deleted",
            req.company_id
        )));
    }

    // 2. Check if company has security enabled
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'company_security_config'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    let security_enabled = if table_exists {
        let flag: Option<i64> = sqlx::query_scalar(
            "SELECT security_enabled FROM company_security_config WHERE company_id = ?",
        )
        .bind(&req.company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;
        flag == Some(1)
    } else {
        false
    };

    let (user_id, username, role) = if security_enabled {
        let password = req.password.as_deref().unwrap_or("");
        let auth_session = SecurityService::authenticate(
            pool,
            crate::security::models::LoginInput {
                company_id: req.company_id.clone(),
                username: req.username.clone(),
                password: password.to_string(),
            },
        )
        .await?;

        (
            auth_session.user_id,
            auth_session.username,
            auth_session.role,
        )
    } else {
        // Open company mode (security disabled): default to Admin role for the operator
        let uname = if req.username.trim().is_empty() {
            "lan_user".to_string()
        } else {
            req.username.trim().to_string()
        };
        ("system".to_string(), uname, UserRole::Admin)
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let token = generate_bearer_token();
    let now = Utc::now();
    let expires_at = now + Duration::hours(8);

    let session = LanSession {
        session_id: session_id.clone(),
        token: token.clone(),
        company_id: req.company_id.clone(),
        user_id: user_id.clone(),
        username: username.clone(),
        role,
        client_ip: client_ip.clone(),
        created_at: now,
        expires_at,
        last_active_at: now,
    };

    session_store.insert(session);

    // Record MCA audit log event with SHA-256 hash chaining
    let audit_details = format!(
        "LAN user '{}' (role: {:?}) connected from {:?}",
        username, role, client_ip
    );
    let _ = AuditCertificationService::record_chained_event(
        pool,
        &req.company_id,
        "LAN_SESSION",
        &session_id,
        "LAN_LOGIN",
        &username,
        Some(&audit_details),
    )
    .await;

    Ok(LanLoginResponse {
        session_id,
        token,
        company_id: req.company_id,
        user_id,
        username,
        role,
        expires_at,
    })
}

/// Handles LAN client logout request, revoking the token and emitting MCA audit log.
pub async fn handle_lan_logout(
    pool: &Pool<Sqlite>,
    session_store: &LanSessionStore,
    token: &str,
) -> Result<LanLogoutResponse, OpenTallyError> {
    if let Some(session) = session_store.remove_by_token(token) {
        let details = format!("LAN user '{}' logged out", session.username);
        let _ = AuditCertificationService::record_chained_event(
            pool,
            &session.company_id,
            "LAN_SESSION",
            &session.session_id,
            "LAN_LOGOUT",
            &session.username,
            Some(&details),
        )
        .await;
    }

    Ok(LanLogoutResponse {
        status: "logged_out".to_string(),
    })
}

/// Validates an incoming LAN Bearer token live against the database and RBAC permissions.
pub async fn validate_lan_session_live(
    pool: &Pool<Sqlite>,
    session_store: &LanSessionStore,
    token: &str,
    required_action: ActionPermission,
) -> Result<LanSession, OpenTallyError> {
    let session = session_store.get_by_token(token).ok_or_else(|| {
        OpenTallyError::Unauthorized(
            "Unauthorized: Invalid or expired LAN session token. Please re-authenticate.".into(),
        )
    })?;

    // Check if security is enabled for company
    let table_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'company_security_config'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if table_exists {
        let security_enabled: Option<i64> = sqlx::query_scalar(
            "SELECT security_enabled FROM company_security_config WHERE company_id = ?",
        )
        .bind(&session.company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if security_enabled == Some(1) {
            // Live query company_users to verify user account exists and is not deactivated
            let db_user: Option<(String, i64)> = sqlx::query_as(
                "SELECT role, is_active FROM company_users WHERE id = ? AND company_id = ?",
            )
            .bind(&session.user_id)
            .bind(&session.company_id)
            .fetch_optional(pool)
            .await
            .map_err(OpenTallyError::Database)?;

            let (role_str, is_active) = match db_user {
                Some(u) => u,
                None => {
                    session_store.remove_by_token(token);
                    return Err(OpenTallyError::Unauthorized(
                        "User account no longer exists in this company.".into(),
                    ));
                }
            };

            if is_active != 1 {
                session_store.remove_by_token(token);
                return Err(OpenTallyError::Unauthorized(
                    "User account has been deactivated by company administrator.".into(),
                ));
            }

            let current_role: UserRole = role_str.parse()?;
            if !current_role.can_perform(required_action) {
                return Err(OpenTallyError::Forbidden(format!(
                    "Access Denied: Role '{:?}' does not have permission to perform '{:?}' over LAN",
                    current_role, required_action
                )));
            }
        }
    }

    session_store.touch_session(token);
    Ok(session)
}
