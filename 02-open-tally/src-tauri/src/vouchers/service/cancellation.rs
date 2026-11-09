use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    CancelVoucherRequest, ValidationMode, Voucher, VoucherLine, VoucherWithLines,
};
use crate::vouchers::service::DoubleEntryValidator;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

/// Authoritative service handling atomic voucher cancellation under SQLite BEGIN IMMEDIATE lock.
pub struct VoucherCancellationService;

impl VoucherCancellationService {
    /// Atomically cancels an existing posted voucher.
    ///
    /// # Lifecycle Transitions:
    /// - Strictly permits `POSTED -> CANCELLED`.
    /// - Rejects draft vouchers (`Draft vouchers cannot be cancelled`).
    /// - Rejects already cancelled vouchers (`Voucher is already cancelled`).
    /// - Rejects reversed vouchers (`Cannot cancel a reversed voucher`).
    /// - Rejects reversing vouchers (`Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable`).
    /// - Rejects vouchers with an existing posted reversal (`Cannot cancel a voucher that has already been reversed`).
    ///
    /// # Immutability & Audit:
    /// - Preserves 100% of line items in `voucher_lines` unchanged.
    /// - Preserves voucher numbers and sequence numbers; sequence counter is never decremented.
    /// - Emits an immutable `audit_log` record with `action = 'CANCEL'`.
    pub async fn cancel_voucher(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CancelVoucherRequest,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        // Step 1: Validate reason string (trimmed length >= 3)
        let reason = req.reason.trim();
        if reason.len() < 3 {
            return Err(OpenTallyError::Validation(
                "Cancellation reason is mandatory (minimum 3 characters)".to_string(),
            ));
        }

        // Step 2: Dedicated connection acquisition and exclusive serialized write lock
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let result = Self::execute_cancellation_transaction(
            &mut conn,
            company_id,
            &req.voucher_id,
            reason,
            req.actor_id.as_deref().unwrap_or("system"),
        )
        .await;

        match result {
            Ok(cancelled_voucher) => match sqlx::query("COMMIT").execute(&mut *conn).await {
                Ok(_) => Ok(cancelled_voucher),
                Err(commit_err) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    let _ = conn.close().await;
                    Err(OpenTallyError::Database(commit_err))
                }
            },
            Err(e) => {
                let rollback_res = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                if rollback_res.is_err() {
                    let _ = conn.close().await;
                }
                Err(e)
            }
        }
    }

    async fn execute_cancellation_transaction(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        reason: &str,
        actor_id: &str,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        // Step 3: Load target voucher header within company tenant boundary
        let voucher = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Voucher '{}' not found in active company",
                voucher_id
            ))
        })?;

        // Step 4: Strict status eligibility validation
        if voucher.status == "draft" {
            return Err(OpenTallyError::Validation(
                "Draft vouchers cannot be cancelled; drafts must be edited or deleted".to_string(),
            ));
        }
        if voucher.status == "cancelled" {
            return Err(OpenTallyError::Validation(
                "Voucher is already cancelled".to_string(),
            ));
        }
        if voucher.status == "reversed" {
            return Err(OpenTallyError::Validation(
                "Cannot cancel a reversed voucher".to_string(),
            ));
        }
        if voucher.status != "posted" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot cancel voucher in status '{}'",
                voucher.status
            )));
        }

        // Step 5: Check reversal constraints
        if voucher.reversal_of_voucher_id.is_some() {
            return Err(OpenTallyError::Validation(
                "Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable"
                    .to_string(),
            ));
        }

        let has_posted_reversal: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM vouchers
             WHERE company_id = $1 AND reversal_of_voucher_id = $2 AND status = 'posted'",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if has_posted_reversal.is_some() {
            return Err(OpenTallyError::Validation(
                "Cannot cancel a voucher that has already been reversed".to_string(),
            ));
        }

        // Step 6: Execute cancellation update
        let rows_affected = sqlx::query(
            "UPDATE vouchers
             SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
             WHERE company_id = $1 AND id = $2 AND status = 'posted'",
        )
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(OpenTallyError::Validation(
                "Voucher is no longer in posted status or was concurrently modified".to_string(),
            ));
        }

        // Step 7: Load preserved voucher lines
        let lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 8: Fetch company settings for audit and precision
        let row_company = sqlx::query_as::<_, (String, i64)>(
            "SELECT financial_year_begins, currency_decimal_places FROM companies WHERE id = $1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Company '{}' not found", company_id)))?;

        let (fy_begins, precision) = row_company;

        // Step 9: Write immutable audit log record
        let audit_id = Uuid::new_v4().to_string();
        let old_values = serde_json::json!({
            "status": "posted",
            "voucher_number": voucher.voucher_number
        })
        .to_string();

        let new_values = serde_json::json!({
            "status": "cancelled",
            "voucher_number": voucher.voucher_number,
            "reason": reason
        })
        .to_string();

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
            ) VALUES ($1, $2, 'voucher', $3, NULL, $4, 'CANCEL', $5, $6, $7)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(&fy_begins)
        .bind(&old_values)
        .bind(&new_values)
        .bind(actor_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 10: Fetch finalized cancelled voucher header
        let cancelled_header = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 11: Assert foreign key integrity before commit
        let fk_violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if !fk_violations.is_empty() {
            return Err(OpenTallyError::Validation(format!(
                "Foreign key integrity violation during cancellation: {} violation(s) detected",
                fk_violations.len()
            )));
        }

        // Calculate double-entry totals for return representation
        let val_res =
            DoubleEntryValidator::validate(&lines, precision as u32, ValidationMode::Draft);

        Ok(VoucherWithLines {
            voucher: cancelled_header,
            lines,
            debit_total: val_res.debit_total.to_string(),
            credit_total: val_res.credit_total.to_string(),
            difference: val_res.difference.to_string(),
            is_balanced: val_res.is_balanced,
        })
    }
}
