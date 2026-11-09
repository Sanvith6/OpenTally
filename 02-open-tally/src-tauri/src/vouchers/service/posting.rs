use crate::errors::OpenTallyError;
use crate::vouchers::formatter::CanonicalVoucherFormatter;
use crate::vouchers::model::{ValidationMode, Voucher, VoucherLine, VoucherWithLines};
use crate::vouchers::service::{DoubleEntryValidator, VoucherSequenceService};
use crate::vouchers::validator::{validate_voucher_date, validate_voucher_ledger_integrity};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

/// Authoritative service handling atomic voucher posting from draft to permanent status.
pub struct VoucherPostingService;

impl VoucherPostingService {
    /// Atomically promotes a voucher draft to POSTED status under BEGIN IMMEDIATE lock.
    pub async fn post_voucher(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        actor_id: Option<&str>,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        // Step 1: Dedicated connection acquisition & immediate write lock
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let result = Self::execute_posting_transaction(
            &mut conn,
            company_id,
            voucher_id,
            actor_id.unwrap_or("system"),
        )
        .await;

        match result {
            Ok(posted) => match sqlx::query("COMMIT").execute(&mut *conn).await {
                Ok(_) => Ok(posted),
                Err(commit_err) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    let _ = conn.close().await;
                    Err(OpenTallyError::Database(commit_err))
                }
            },
            Err(e) => {
                // Rollback on any failure
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    async fn execute_posting_transaction(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        actor_id: &str,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        // Step 2: Load and verify draft header
        let draft = sqlx::query_as::<_, Voucher>(
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
            OpenTallyError::NotFound(format!("Voucher draft '{}' not found", voucher_id))
        })?;

        // Step 3: Verify draft status
        if draft.status == "posted" {
            return Err(OpenTallyError::Validation(format!(
                "Voucher '{}' is already posted",
                draft.voucher_number
            )));
        }
        if draft.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot post voucher in terminal status '{}'",
                draft.status
            )));
        }

        // Step 4: Validate company fiscal boundaries and dates
        let row_company = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT financial_year_begins, books_beginning_from, currency_decimal_places
             FROM companies WHERE id = $1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Company '{}' not found", company_id)))?;

        let (fy_begins, books_beginning, precision) = row_company;
        validate_voucher_date(&draft.date, &books_beginning)?;
        validate_voucher_date(&draft.effective_date, &books_beginning)?;

        // Step 5: Validate voucher type and series
        let vt_active: Option<i64> = sqlx::query_scalar(
            "SELECT is_active FROM voucher_types WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&draft.voucher_type_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match vt_active {
            Some(1) => {}
            Some(_) => {
                return Err(OpenTallyError::Validation(format!(
                    "Voucher type '{}' is inactive",
                    draft.voucher_type_id
                )));
            }
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Voucher type '{}' not found",
                    draft.voucher_type_id
                )));
            }
        }

        let series_row = sqlx::query(
            "SELECT series_name, series_code, numbering_method, restart_period, is_active
             FROM voucher_number_series
             WHERE company_id = $1 AND voucher_type_id = $2 AND id = $3",
        )
        .bind(company_id)
        .bind(&draft.voucher_type_id)
        .bind(&draft.series_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Series '{}' not found for voucher type '{}'",
                draft.series_id, draft.voucher_type_id
            ))
        })?;

        let is_active: i64 = series_row.get("is_active");
        let series_name: String = series_row.get("series_name");
        let series_code: String = series_row.get("series_code");
        let numbering_method: String = series_row.get("numbering_method");
        let restart_period: String = series_row.get("restart_period");

        if is_active != 1 {
            return Err(OpenTallyError::Validation(format!(
                "Numbering series '{}' ({}) is inactive; posting blocked",
                series_name, series_code
            )));
        }

        // Verify effective config exists for this date
        let config = VoucherSequenceService::resolve_effective_config(
            conn,
            company_id,
            &draft.series_id,
            &draft.date,
        )
        .await?;

        // Step 6: Load lines and execute pure double-entry validation
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

        let val_res =
            DoubleEntryValidator::validate(&lines, precision as u32, ValidationMode::Posting);

        if !val_res.is_valid || !val_res.is_balanced {
            let error_details = val_res
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(OpenTallyError::Validation(format!(
                "Posting validation failed: {}",
                if error_details.is_empty() {
                    "Voucher out of balance".to_string()
                } else {
                    error_details
                }
            )));
        }

        let total_amount = val_res
            .single_side_total
            .ok_or_else(|| {
                OpenTallyError::Validation("Missing single-side transaction total".into())
            })?
            .to_string();

        // Step 7: Database contextual ledger integrity check
        validate_voucher_ledger_integrity(conn, company_id, &lines).await?;

        // Step 8: Permanent number allocation or validation
        let (permanent_number, period_start_val, sequence_num_val) = if numbering_method
            == "automatic"
        {
            let alloc = VoucherSequenceService::allocate_next_sequence(
                conn,
                company_id,
                &draft.voucher_type_id,
                &draft.series_id,
                &draft.date,
            )
            .await?;
            (
                alloc.voucher_number,
                Some(alloc.period_start),
                Some(alloc.sequence_number),
            )
        } else {
            // Manual numbering
            let manual_num = draft.voucher_number.trim();
            if manual_num.is_empty() || manual_num.starts_with("DRAFT-") {
                return Err(OpenTallyError::Validation(
                    "Manual voucher requires a permanent voucher number before posting".into(),
                ));
            }

            // Fetch all series codes in voucher type to assert zero usurpation
            let all_codes: Vec<String> = sqlx::query_scalar(
                    "SELECT series_code FROM voucher_number_series WHERE company_id = $1 AND voucher_type_id = $2",
                )
                .bind(company_id)
                .bind(&draft.voucher_type_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

            CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
                manual_num,
                &series_code,
                &config.prefix,
                &config.suffix,
                config.width as usize,
                &restart_period,
                &config.period_start,
                &all_codes,
            )?;

            (manual_num.to_string(), None::<String>, None::<i64>)
        };

        // Step 9 & 10: Transition voucher to POSTED
        let rows_affected = sqlx::query(
            "UPDATE vouchers
             SET series_id = $1,
                 numbering_method = $2,
                 voucher_number = $3,
                 period_start = $4,
                 sequence_number = $5,
                 status = 'posted',
                 total_amount = $6,
                 updated_at = CURRENT_TIMESTAMP
             WHERE company_id = $7 AND id = $8 AND status = 'draft'",
        )
        .bind(&draft.series_id)
        .bind(&numbering_method)
        .bind(&permanent_number)
        .bind(&period_start_val)
        .bind(sequence_num_val)
        .bind(&total_amount)
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows_affected == 0 {
            return Err(OpenTallyError::Validation(
                "Voucher is no longer in draft state or was concurrently modified".into(),
            ));
        }

        // Step 11: Write immutable audit log record
        let audit_id = Uuid::new_v4().to_string();
        let old_values = serde_json::json!({
            "status": "draft",
            "voucher_number": draft.voucher_number
        })
        .to_string();

        let new_values = serde_json::json!({
            "status": "posted",
            "voucher_number": permanent_number,
            "series_id": draft.series_id,
            "numbering_method": numbering_method,
            "period_start": period_start_val,
            "sequence_number": sequence_num_val,
            "total_amount": total_amount,
            "line_count": lines.len(),
            "date": draft.date
        })
        .to_string();

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
            ) VALUES ($1, $2, 'voucher', $3, NULL, $4, 'POST', $5, $6, $7)",
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

        // Fetch finalized posted header
        let posted_header = sqlx::query_as::<_, Voucher>(
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

        // Step 11b: Verify foreign key integrity prior to commit
        let fk_violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if !fk_violations.is_empty() {
            return Err(OpenTallyError::Validation(format!(
                "Foreign key integrity violation during posting: {} violation(s) detected",
                fk_violations.len()
            )));
        }

        Ok(VoucherWithLines {
            voucher: posted_header,
            lines,
            debit_total: val_res.debit_total.to_string(),
            credit_total: val_res.credit_total.to_string(),
            difference: val_res.difference.to_string(),
            is_balanced: val_res.is_balanced,
        })
    }
}
