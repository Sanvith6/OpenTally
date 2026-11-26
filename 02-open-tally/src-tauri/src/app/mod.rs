use crate::config::AppConfig;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::security::models::{ActionPermission, AuthSessionInfo, UserRole};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

/// Global application state shared across Tauri IPC commands.
pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<DatabaseManager>,
    pub started_at: DateTime<Utc>,
    pub active_company_id: std::sync::RwLock<Option<String>>,
    pub active_user_sessions: std::sync::RwLock<HashMap<String, AuthSessionInfo>>,
    pub lan_server: std::sync::RwLock<Option<crate::lan::LanServerHandle>>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Arc<DatabaseManager>) -> Self {
        Self {
            config,
            db,
            started_at: Utc::now(),
            active_company_id: std::sync::RwLock::new(None),
            active_user_sessions: std::sync::RwLock::new(HashMap::new()),
            lan_server: std::sync::RwLock::new(None),
        }
    }

    /// Stores an active authenticated user session for a company.
    pub fn set_active_session(&self, company_id: String, session: AuthSessionInfo) {
        if let Ok(mut guard) = self.active_user_sessions.write() {
            guard.insert(company_id, session);
        }
    }

    /// Clears any active user session for a company upon logout.
    pub fn logout_session(&self, company_id: &str) {
        if let Ok(mut guard) = self.active_user_sessions.write() {
            guard.remove(company_id);
        }
    }

    /// Evicts an active user session if it matches the specified user_id.
    pub fn evict_user_session_by_id(&self, company_id: &str, user_id: &str) {
        if let Ok(mut guard) = self.active_user_sessions.write() {
            if let Some(session) = guard.get(company_id) {
                if session.user_id == user_id {
                    guard.remove(company_id);
                }
            }
        }
    }

    /// Resolves the authenticated actor for a given company and verifies role permissions.
    ///
    /// If security is disabled for the company, defaults to "system" with unrestricted access.
    /// If security is enabled, validates active session presence and role capability.
    pub async fn resolve_actor(
        &self,
        company_id: &str,
        required_action: ActionPermission,
    ) -> Result<String, OpenTallyError> {
        let pool = self.db.pool();

        let table_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'company_security_config'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if !table_exists {
            return Ok("system".to_string());
        }

        let security_enabled: Option<i64> = sqlx::query_scalar(
            "SELECT security_enabled FROM company_security_config WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if security_enabled != Some(1) {
            return Ok("system".to_string());
        }

        let session = {
            let guard = self
                .active_user_sessions
                .read()
                .map_err(|e| OpenTallyError::Internal(format!("Session lock poisoned: {}", e)))?;
            guard.get(company_id).cloned()
        };

        let session = match session {
            Some(s) => s,
            None => {
                return Err(OpenTallyError::Unauthorized(
                    "Authentication required: Company security is enabled. Please log in.".into(),
                ));
            }
        };

        // Query database to ensure user is still active and retrieve live role
        let db_user: Option<(String, i64)> = sqlx::query_as(
            "SELECT role, is_active FROM company_users WHERE id = ? AND company_id = ?",
        )
        .bind(&session.user_id)
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let (role_str, is_active) = match db_user {
            Some(u) => u,
            None => {
                self.logout_session(company_id);
                return Err(OpenTallyError::Unauthorized(
                    "User account no longer exists. Please log in again.".into(),
                ));
            }
        };

        if is_active != 1 {
            self.logout_session(company_id);
            return Err(OpenTallyError::Unauthorized(
                "User account has been deactivated. Please contact your company administrator."
                    .into(),
            ));
        }

        let current_role: UserRole = role_str.parse()?;
        if !current_role.can_perform(required_action) {
            return Err(OpenTallyError::Forbidden(format!(
                "Access Denied: Role '{:?}' does not have permission to perform '{:?}'",
                current_role, required_action
            )));
        }

        Ok(session.username)
    }
}
