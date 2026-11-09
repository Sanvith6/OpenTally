use crate::errors::OpenTallyError;
use crate::vouchers::model::VoucherType;
use sqlx::{Pool, Sqlite};

/// Service handling voucher types queries and metadata.
pub struct VoucherTypeService;

impl VoucherTypeService {
    /// List all predefined and user-defined voucher types for a given company.
    pub async fn list_voucher_types(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<VoucherType>, OpenTallyError> {
        let rows = sqlx::query_as::<_, VoucherType>(
            "SELECT id, company_id, name, category, parent_type, affects_ledger_balance, affects_inventory, is_predefined, is_active, numbering_method, created_at, updated_at
             FROM voucher_types
             WHERE company_id = $1
             ORDER BY is_predefined DESC, name ASC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }

    /// Retrieve a single voucher type by company_id and type_id.
    pub async fn get_voucher_type(
        pool: &Pool<Sqlite>,
        company_id: &str,
        type_id: &str,
    ) -> Result<VoucherType, OpenTallyError> {
        let row = sqlx::query_as::<_, VoucherType>(
            "SELECT id, company_id, name, category, parent_type, affects_ledger_balance, affects_inventory, is_predefined, is_active, numbering_method, created_at, updated_at
             FROM voucher_types
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(type_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        row.ok_or_else(|| {
            OpenTallyError::NotFound(format!("Voucher type with ID '{}' not found", type_id))
        })
    }
}
