use crate::accounting::model::{
    AccountGroup, CreateAccountGroupRequest, UpdateAccountGroupRequest,
};
use crate::accounting::validator::{
    detect_group_cycle, validate_group_name, validate_nature_match,
};
use crate::errors::OpenTallyError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct GroupService;

impl GroupService {
    /// Lists all account groups for the company.
    /// By default (`include_hidden = false`), excludes system-hidden groups (e.g., Profit & Loss A/c System group).
    pub async fn list_groups(
        pool: &SqlitePool,
        company_id: &str,
        include_hidden: bool,
    ) -> Result<Vec<AccountGroup>, OpenTallyError> {
        let groups = if include_hidden {
            sqlx::query_as::<_, AccountGroup>(
                "SELECT id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at
                 FROM account_groups
                 WHERE company_id = ?
                 ORDER BY is_primary DESC, name ASC"
            )
            .bind(company_id)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, AccountGroup>(
                "SELECT id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at
                 FROM account_groups
                 WHERE company_id = ? AND is_hidden = 0
                 ORDER BY is_primary DESC, name ASC"
            )
            .bind(company_id)
            .fetch_all(pool)
            .await?
        };

        Ok(groups)
    }

    /// Retrieves an account group by ID for the company.
    pub async fn get_group(
        pool: &SqlitePool,
        company_id: &str,
        group_id: &str,
    ) -> Result<AccountGroup, OpenTallyError> {
        let group = sqlx::query_as::<_, AccountGroup>(
            "SELECT id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at
             FROM account_groups
             WHERE company_id = ? AND id = ?"
        )
        .bind(company_id)
        .bind(group_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| OpenTallyError::Validation(format!("Account group '{}' not found", group_id)))?;

        Ok(group)
    }

    /// Creates a user-defined account group under a valid parent group.
    pub async fn create_group(
        pool: &SqlitePool,
        company_id: &str,
        req: CreateAccountGroupRequest,
    ) -> Result<AccountGroup, OpenTallyError> {
        let name = validate_group_name(&req.name)?;

        // Verify parent group exists in active company
        let parent = Self::get_group(pool, company_id, &req.parent_id).await?;
        if parent.is_hidden {
            return Err(OpenTallyError::Validation(
                "Cannot create account group under a hidden system group".to_string(),
            ));
        }

        // Verify group name uniqueness for company (case-insensitive)
        let name_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&name)
        .fetch_optional(pool)
        .await?;

        if name_exists.is_some() {
            return Err(OpenTallyError::Validation(format!(
                "An account group named '{}' already exists in this company",
                name
            )));
        }

        let new_id = Uuid::new_v4().to_string();
        let nature = parent.nature;
        let affects_gross_profit = parent.affects_gross_profit;

        sqlx::query(
            "INSERT INTO account_groups (
                id, company_id, name, parent_id, nature,
                is_primary, is_predefined, is_hidden, affects_gross_profit
             ) VALUES (?, ?, ?, ?, ?, 0, 0, 0, ?)",
        )
        .bind(&new_id)
        .bind(company_id)
        .bind(&name)
        .bind(&parent.id)
        .bind(&nature)
        .bind(affects_gross_profit)
        .execute(pool)
        .await?;

        Self::get_group(pool, company_id, &new_id).await
    }

    /// Updates a user-defined account group.
    pub async fn update_group(
        pool: &SqlitePool,
        company_id: &str,
        group_id: &str,
        req: UpdateAccountGroupRequest,
    ) -> Result<AccountGroup, OpenTallyError> {
        let existing = Self::get_group(pool, company_id, group_id).await?;

        // Predefined groups cannot be modified or reparented
        if existing.is_predefined {
            return Err(OpenTallyError::Validation(format!(
                "Predefined group '{}' is protected and cannot be altered or reparented",
                existing.name
            )));
        }

        let name = validate_group_name(&req.name)?;

        // Verify name uniqueness if name is changed
        if !name.eq_ignore_ascii_case(&existing.name) {
            let name_conflict: Option<String> = sqlx::query_scalar(
                "SELECT id FROM account_groups WHERE company_id = ? AND name = ? COLLATE NOCASE AND id != ?",
            )
            .bind(company_id)
            .bind(&name)
            .bind(group_id)
            .fetch_optional(pool)
            .await?;

            if name_conflict.is_some() {
                return Err(OpenTallyError::Validation(format!(
                    "An account group named '{}' already exists in this company",
                    name
                )));
            }
        }

        // Reparenting validation
        let new_parent_id = match req.parent_id {
            Some(ref pid) => {
                if existing.parent_id.as_deref() != Some(pid.as_str()) {
                    let target_parent = Self::get_group(pool, company_id, pid).await?;
                    if target_parent.is_hidden {
                        return Err(OpenTallyError::Validation(
                            "Cannot reparent group under a hidden system group".to_string(),
                        ));
                    }

                    // Nature matching check
                    validate_nature_match(&existing.nature, &target_parent.nature)?;

                    // Cycle detection
                    let all_groups = Self::list_groups(pool, company_id, true).await?;
                    detect_group_cycle(&all_groups, group_id, pid)?;

                    Some(pid.clone())
                } else {
                    existing.parent_id.clone()
                }
            }
            None => {
                // User groups cannot be turned into primary groups without a parent
                return Err(OpenTallyError::Validation(
                    "User-defined account groups must have a valid parent group".to_string(),
                ));
            }
        };

        sqlx::query(
            "UPDATE account_groups
             SET name = ?, parent_id = ?, updated_at = (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             WHERE company_id = ? AND id = ?",
        )
        .bind(&name)
        .bind(&new_parent_id)
        .bind(company_id)
        .bind(group_id)
        .execute(pool)
        .await?;

        Self::get_group(pool, company_id, group_id).await
    }

    /// Deletes an account group if it is user-defined and has no children or ledgers.
    pub async fn delete_group(
        pool: &SqlitePool,
        company_id: &str,
        group_id: &str,
    ) -> Result<(), OpenTallyError> {
        let existing = Self::get_group(pool, company_id, group_id).await?;

        if existing.is_predefined {
            return Err(OpenTallyError::Validation(format!(
                "Predefined system account group '{}' cannot be deleted",
                existing.name
            )));
        }

        // Check for child groups
        let child_groups_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM account_groups WHERE company_id = ? AND parent_id = ?",
        )
        .bind(company_id)
        .bind(group_id)
        .fetch_one(pool)
        .await?;

        if child_groups_count > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete account group '{}': it contains {} child group(s)",
                existing.name, child_groups_count
            )));
        }

        // Check for child ledgers
        let child_ledgers_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ledgers WHERE company_id = ? AND group_id = ?",
        )
        .bind(company_id)
        .bind(group_id)
        .fetch_one(pool)
        .await?;

        if child_ledgers_count > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete account group '{}': it contains {} ledger(s)",
                existing.name, child_ledgers_count
            )));
        }

        sqlx::query("DELETE FROM account_groups WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(group_id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
