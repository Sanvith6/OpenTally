use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use crate::security::models::{
    AuthSessionInfo, CompanySecurityConfig, CompanyUser, CreateUserInput, DeactivateUserInput,
    EnableSecurityInput, LoginInput, SecurityStatusDto, UpdateUserPasswordInput,
    UpdateUserRoleInput, UserRole,
};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

pub struct SecurityService;

impl SecurityService {
    /// Hashes a plaintext password using Argon2id with a cryptographically secure random salt.
    pub fn hash_password(password: &str) -> Result<String, OpenTallyError> {
        if password.len() < 4 {
            return Err(OpenTallyError::Validation(
                "Password must be at least 4 characters long".into(),
            ));
        }
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| OpenTallyError::Internal(format!("Password hashing failed: {}", e)))?
            .to_string();
        Ok(password_hash)
    }

    /// Verifies a password against an Argon2id hash in constant time.
    pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, OpenTallyError> {
        let parsed_hash = PasswordHash::new(password_hash)
            .map_err(|e| OpenTallyError::Internal(format!("Invalid password hash: {}", e)))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Fetches the security configuration and active user count for a company.
    pub async fn get_security_status(
        pool: &SqlitePool,
        company_id: &str,
        active_user: Option<AuthSessionInfo>,
    ) -> Result<SecurityStatusDto, OpenTallyError> {
        let cfg: Option<CompanySecurityConfig> = sqlx::query_as(
            "SELECT company_id, security_enabled, updated_at FROM company_security_config WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let security_enabled = cfg.map(|c| c.security_enabled == 1).unwrap_or(false);

        let user_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM company_users WHERE company_id = ? AND is_active = 1",
        )
        .bind(company_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(SecurityStatusDto {
            security_enabled,
            has_users: user_count > 0,
            active_user,
        })
    }

    /// Enables security for a company and creates the initial Admin / Owner account.
    pub async fn enable_company_security(
        pool: &SqlitePool,
        input: EnableSecurityInput,
        actor: &str,
    ) -> Result<AuthSessionInfo, OpenTallyError> {
        let admin_username = input.admin_username.trim();
        if admin_username.is_empty() || admin_username.len() > 50 {
            return Err(OpenTallyError::Validation(
                "Admin username must be between 1 and 50 characters".into(),
            ));
        }
        let password_hash = Self::hash_password(&input.admin_password)?;

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        // Ensure company exists
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        // Check if security is already enabled
        let cfg: Option<i64> = sqlx::query_scalar(
            "SELECT security_enabled FROM company_security_config WHERE company_id = ?",
        )
        .bind(&input.company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if cfg == Some(1) {
            return Err(OpenTallyError::Validation(
                "Security is already enabled for this company".into(),
            ));
        }

        // Upsert security config
        sqlx::query(
            "INSERT INTO company_security_config (company_id, security_enabled, updated_at)
             VALUES (?, 1, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET security_enabled = 1, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&input.company_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Check if admin user already exists (e.g. from prior activation)
        let existing_admin: Option<String> = sqlx::query_scalar(
            "SELECT id FROM company_users WHERE company_id = ? AND username = ? COLLATE NOCASE",
        )
        .bind(&input.company_id)
        .bind(admin_username)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let user_id = if let Some(id) = existing_admin {
            sqlx::query(
                "UPDATE company_users SET password_hash = ?, role = 'Admin', is_active = 1, last_login_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&password_hash)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
            id
        } else {
            let new_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO company_users (id, company_id, username, password_hash, role, is_active, created_at, last_login_at)
                 VALUES (?, ?, ?, ?, 'Admin', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            )
            .bind(&new_id)
            .bind(&input.company_id)
            .bind(admin_username)
            .bind(&password_hash)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
            new_id
        };

        // Write MCA audit trail with SHA-256 hash chaining
        let audit_payload = serde_json::json!({
            "action": "ENABLE_SECURITY",
            "admin_username": admin_username,
            "admin_user_id": &user_id,
        })
        .to_string();

        Self::record_security_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "company_security_config",
            &input.company_id,
            "UPDATE",
            actor,
            &audit_payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(AuthSessionInfo {
            user_id,
            username: admin_username.to_string(),
            role: UserRole::Admin,
            logged_in_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Authenticates a company user with password hash verification.
    pub async fn authenticate(
        pool: &SqlitePool,
        input: LoginInput,
    ) -> Result<AuthSessionInfo, OpenTallyError> {
        let username = input.username.trim();

        // 1. Check if security is enabled
        let cfg: Option<i64> = sqlx::query_scalar(
            "SELECT security_enabled FROM company_security_config WHERE company_id = ?",
        )
        .bind(&input.company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if cfg != Some(1) {
            return Err(OpenTallyError::Validation(
                "Security is not enabled for this company".into(),
            ));
        }

        // 2. Lookup user
        let user: Option<(String, String, String, String, i64)> = sqlx::query_as(
            "SELECT id, username, password_hash, role, is_active FROM company_users WHERE company_id = ? AND username = ? COLLATE NOCASE",
        )
        .bind(&input.company_id)
        .bind(username)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let (user_id, resolved_username, hash, role_str, is_active) = match user {
            Some(u) => u,
            None => {
                return Err(OpenTallyError::Unauthorized(
                    "Invalid username or password".into(),
                ));
            }
        };

        if is_active == 0 {
            return Err(OpenTallyError::Unauthorized(
                "User account has been deactivated".into(),
            ));
        }

        // 3. Constant-time password verification
        let is_valid = Self::verify_password(&input.password, &hash)?;
        if !is_valid {
            return Err(OpenTallyError::Unauthorized(
                "Invalid username or password".into(),
            ));
        }

        let role: UserRole = role_str.parse()?;
        let now_str = chrono::Utc::now().to_rfc3339();

        // 4. Update last_login_at
        sqlx::query("UPDATE company_users SET last_login_at = ? WHERE id = ?")
            .bind(&now_str)
            .bind(&user_id)
            .execute(pool)
            .await
            .map_err(OpenTallyError::Database)?;

        Ok(AuthSessionInfo {
            user_id,
            username: resolved_username,
            role,
            logged_in_at: now_str,
        })
    }

    /// Creates a new user in the company (requires Admin privileges).
    pub async fn create_user(
        pool: &SqlitePool,
        input: CreateUserInput,
        actor: &str,
    ) -> Result<CompanyUser, OpenTallyError> {
        let username = input.username.trim();
        if username.is_empty() || username.len() > 50 {
            return Err(OpenTallyError::Validation(
                "Username must be between 1 and 50 characters".into(),
            ));
        }

        let password_hash = Self::hash_password(&input.password)?;

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        // Check duplicate
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM company_users WHERE company_id = ? AND username = ? COLLATE NOCASE",
        )
        .bind(&input.company_id)
        .bind(username)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if existing.is_some() {
            return Err(OpenTallyError::Validation(format!(
                "A user with username '{}' already exists in this company",
                username
            )));
        }

        let user_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO company_users (id, company_id, username, password_hash, role, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP)",
        )
        .bind(&user_id)
        .bind(&input.company_id)
        .bind(username)
        .bind(&password_hash)
        .bind(input.role.as_str())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let payload = serde_json::json!({
            "action": "CREATE_USER",
            "user_id": &user_id,
            "username": username,
            "role": input.role.as_str(),
        })
        .to_string();

        Self::record_security_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "company_users",
            &user_id,
            "CREATE",
            actor,
            &payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        let created_user: CompanyUser = sqlx::query_as(
            "SELECT id, company_id, username, role, is_active, created_at, last_login_at FROM company_users WHERE id = ?",
        )
        .bind(&user_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(created_user)
    }

    /// Updates a user's role, ensuring that the last active Admin is never demoted.
    pub async fn update_user_role(
        pool: &SqlitePool,
        input: UpdateUserRoleInput,
        actor: &str,
    ) -> Result<CompanyUser, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        let user: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT username, role, is_active FROM company_users WHERE id = ? AND company_id = ?",
        )
        .bind(&input.user_id)
        .bind(&input.company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let (username, current_role, is_active) = match user {
            Some(u) => u,
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "User '{}' not found in company",
                    input.user_id
                )));
            }
        };

        // Guardrail: if demoting from Admin, ensure another active Admin exists
        if current_role == "Admin" && input.role != UserRole::Admin && is_active == 1 {
            let active_admins: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM company_users WHERE company_id = ? AND role = 'Admin' AND is_active = 1 AND id != ?",
            )
            .bind(&input.company_id)
            .bind(&input.user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if active_admins == 0 {
                return Err(OpenTallyError::Validation(
                    "Cannot demote the last active Administrator account".into(),
                ));
            }
        }

        sqlx::query("UPDATE company_users SET role = ? WHERE id = ?")
            .bind(input.role.as_str())
            .bind(&input.user_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        let payload = serde_json::json!({
            "action": "UPDATE_USER_ROLE",
            "user_id": &input.user_id,
            "username": &username,
            "old_role": &current_role,
            "new_role": input.role.as_str(),
        })
        .to_string();

        Self::record_security_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "company_users",
            &input.user_id,
            "UPDATE",
            actor,
            &payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        let updated_user: CompanyUser = sqlx::query_as(
            "SELECT id, company_id, username, role, is_active, created_at, last_login_at FROM company_users WHERE id = ?",
        )
        .bind(&input.user_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(updated_user)
    }

    /// Deactivates a user, ensuring that the last active Admin is never deactivated.
    pub async fn deactivate_user(
        pool: &SqlitePool,
        input: DeactivateUserInput,
        actor: &str,
    ) -> Result<(), OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        let user: Option<(String, String)> = sqlx::query_as(
            "SELECT username, role FROM company_users WHERE id = ? AND company_id = ? AND is_active = 1",
        )
        .bind(&input.user_id)
        .bind(&input.company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let (username, role) = match user {
            Some(u) => u,
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Active user '{}' not found in company",
                    input.user_id
                )));
            }
        };

        if role == "Admin" {
            let other_active_admins: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM company_users WHERE company_id = ? AND role = 'Admin' AND is_active = 1 AND id != ?",
            )
            .bind(&input.company_id)
            .bind(&input.user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if other_active_admins == 0 {
                return Err(OpenTallyError::Validation(
                    "Cannot deactivate the last active Administrator account".into(),
                ));
            }
        }

        sqlx::query("UPDATE company_users SET is_active = 0 WHERE id = ?")
            .bind(&input.user_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        let payload = serde_json::json!({
            "action": "DEACTIVATE_USER",
            "user_id": &input.user_id,
            "username": &username,
            "role": &role,
        })
        .to_string();

        Self::record_security_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "company_users",
            &input.user_id,
            "UPDATE",
            actor,
            &payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    /// Updates a user's password (by self or by Admin).
    pub async fn update_user_password(
        pool: &SqlitePool,
        input: UpdateUserPasswordInput,
        actor: &str,
    ) -> Result<(), OpenTallyError> {
        let password_hash = Self::hash_password(&input.new_password)?;

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        let user: Option<String> = sqlx::query_scalar(
            "SELECT username FROM company_users WHERE id = ? AND company_id = ? AND is_active = 1",
        )
        .bind(&input.user_id)
        .bind(&input.company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let username = match user {
            Some(u) => u,
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Active user '{}' not found in company",
                    input.user_id
                )));
            }
        };

        sqlx::query("UPDATE company_users SET password_hash = ? WHERE id = ?")
            .bind(&password_hash)
            .bind(&input.user_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        let payload = serde_json::json!({
            "action": "UPDATE_USER_PASSWORD",
            "user_id": &input.user_id,
            "username": &username,
        })
        .to_string();

        Self::record_security_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "company_users",
            &input.user_id,
            "UPDATE",
            actor,
            &payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    /// Lists all users for a company.
    pub async fn list_users(
        pool: &SqlitePool,
        company_id: &str,
    ) -> Result<Vec<CompanyUser>, OpenTallyError> {
        let users: Vec<CompanyUser> = sqlx::query_as(
            "SELECT id, company_id, username, role, is_active, created_at, last_login_at
             FROM company_users
             WHERE company_id = ?
             ORDER BY created_at ASC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(users)
    }

    /// Internal helper to record an immutable MCA audit_log row with SHA-256 hash chaining.
    #[allow(clippy::too_many_arguments)]
    async fn record_security_audit_log(
        conn: &mut SqliteConnection,
        company_id: &str,
        financial_year_begins: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        performed_by: &str,
        payload: &str,
    ) -> Result<(), OpenTallyError> {
        let prev_hash_opt: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev = prev_hash_opt.unwrap_or_else(|| "GENESIS".to_string());
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let entry_hash = AuditCertificationService::compute_entry_hash(
            &prev,
            &audit_id,
            company_id,
            entity_type,
            entity_id,
            action,
            &timestamp,
            Some(payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(financial_year_begins)
        .bind(action)
        .bind(payload)
        .bind(performed_by)
        .bind(&timestamp)
        .bind(&prev)
        .bind(&entry_hash)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }
}
