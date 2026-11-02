use super::group::GroupService;
use crate::accounting::model::{CreateLedgerRequest, Ledger, UpdateLedgerRequest};
use crate::accounting::validator::validate_ledger_name;
use crate::errors::OpenTallyError;
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct LedgerService;

impl LedgerService {
    /// Lists all ledgers for a company, with optional filtering by group_id and/or search term.
    pub async fn list_ledgers(
        pool: &SqlitePool,
        company_id: &str,
        group_id: Option<&str>,
        search_query: Option<&str>,
    ) -> Result<Vec<Ledger>, OpenTallyError> {
        let mut sql = "SELECT id, company_id, group_id, name, ledger_classification, is_reserved, description, created_at, updated_at
                       FROM ledgers WHERE company_id = ?".to_string();

        if group_id.is_some() {
            sql.push_str(" AND group_id = ?");
        }

        if search_query.is_some() {
            sql.push_str(" AND name LIKE '%' || ? || '%' COLLATE NOCASE");
        }

        sql.push_str(" ORDER BY name ASC");

        let mut query = sqlx::query_as::<_, Ledger>(&sql).bind(company_id);

        if let Some(gid) = group_id {
            query = query.bind(gid);
        }

        if let Some(sq) = search_query {
            query = query.bind(sq);
        }

        let ledgers = query.fetch_all(pool).await?;
        Ok(ledgers)
    }

    /// Fetches a single ledger by ID.
    pub async fn get_ledger(
        pool: &SqlitePool,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<Ledger, OpenTallyError> {
        let ledger = sqlx::query_as::<_, Ledger>(
            "SELECT id, company_id, group_id, name, ledger_classification, is_reserved, description, created_at, updated_at
             FROM ledgers
             WHERE company_id = ? AND id = ?"
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| OpenTallyError::Validation(format!("Ledger '{}' not found", ledger_id)))?;

        Ok(ledger)
    }

    /// Creates a standard user-defined ledger.
    pub async fn create_ledger(
        pool: &SqlitePool,
        company_id: &str,
        req: CreateLedgerRequest,
    ) -> Result<Ledger, OpenTallyError> {
        let name = validate_ledger_name(&req.name)?;

        // Verify parent group exists in company and is not hidden
        let group = GroupService::get_group(pool, company_id, &req.group_id).await?;
        if group.is_hidden {
            return Err(OpenTallyError::Validation(
                "Standard ledgers cannot belong to hidden system groups".to_string(),
            ));
        }

        // Verify ledger name uniqueness in company
        let exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&name)
        .fetch_optional(pool)
        .await?;

        if exists.is_some() {
            return Err(OpenTallyError::Validation(format!(
                "A ledger named '{}' already exists in this company",
                name
            )));
        }

        let new_id = Uuid::new_v4().to_string();
        let desc = req.description.map(|s| s.trim().to_string());

        sqlx::query(
            "INSERT INTO ledgers (
                id, company_id, group_id, name, ledger_classification,
                is_reserved, description
             ) VALUES (?, ?, ?, ?, 'standard', 0, ?)",
        )
        .bind(&new_id)
        .bind(company_id)
        .bind(&group.id)
        .bind(&name)
        .bind(&desc)
        .execute(pool)
        .await?;

        Self::get_ledger(pool, company_id, &new_id).await
    }

    /// Updates permitted fields of a ledger.
    pub async fn update_ledger(
        pool: &SqlitePool,
        company_id: &str,
        ledger_id: &str,
        req: UpdateLedgerRequest,
    ) -> Result<Ledger, OpenTallyError> {
        let existing = Self::get_ledger(pool, company_id, ledger_id).await?;

        let name = validate_ledger_name(&req.name)?;

        // Reserved ledger protections
        if existing.is_reserved {
            if !name.eq_ignore_ascii_case(&existing.name) {
                return Err(OpenTallyError::Validation(format!(
                    "Reserved system ledger '{}' cannot be renamed",
                    existing.name
                )));
            }

            if let Some(ref gid) = req.group_id {
                if gid != &existing.group_id {
                    return Err(OpenTallyError::Validation(format!(
                        "Reserved system ledger '{}' cannot be reparented",
                        existing.name
                    )));
                }
            }
        }

        // Check name uniqueness if changed
        if !name.eq_ignore_ascii_case(&existing.name) {
            let conflict: Option<String> = sqlx::query_scalar(
                "SELECT id FROM ledgers WHERE company_id = ? AND name = ? COLLATE NOCASE AND id != ?",
            )
            .bind(company_id)
            .bind(&name)
            .bind(ledger_id)
            .fetch_optional(pool)
            .await?;

            if conflict.is_some() {
                return Err(OpenTallyError::Validation(format!(
                    "A ledger named '{}' already exists in this company",
                    name
                )));
            }
        }

        let target_group_id = if let Some(ref gid) = req.group_id {
            if gid != &existing.group_id {
                let target_group = GroupService::get_group(pool, company_id, gid).await?;
                if target_group.is_hidden {
                    return Err(OpenTallyError::Validation(
                        "Standard ledgers cannot belong to hidden system groups".to_string(),
                    ));
                }
                gid.clone()
            } else {
                existing.group_id.clone()
            }
        } else {
            existing.group_id.clone()
        };

        let desc = req.description.map(|s| s.trim().to_string());

        sqlx::query(
            "UPDATE ledgers
             SET name = ?, group_id = ?, description = ?, updated_at = (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             WHERE company_id = ? AND id = ?",
        )
        .bind(&name)
        .bind(&target_group_id)
        .bind(&desc)
        .bind(company_id)
        .bind(ledger_id)
        .execute(pool)
        .await?;

        Self::get_ledger(pool, company_id, ledger_id).await
    }

    /// Deletes a ledger if eligible (unreserved and has no opening balances or voucher references).
    pub async fn delete_ledger(
        pool: &SqlitePool,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<(), OpenTallyError> {
        let existing = Self::get_ledger(pool, company_id, ledger_id).await?;

        if existing.is_reserved {
            return Err(OpenTallyError::Validation(format!(
                "Reserved system ledger '{}' cannot be deleted",
                existing.name
            )));
        }

        // Check opening balance references
        let opb_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM opening_balances WHERE company_id = ? AND ledger_id = ?",
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_one(pool)
        .await?;

        if opb_count > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete ledger '{}': opening balance exists",
                existing.name
            )));
        }

        sqlx::query("DELETE FROM ledgers WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(ledger_id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
