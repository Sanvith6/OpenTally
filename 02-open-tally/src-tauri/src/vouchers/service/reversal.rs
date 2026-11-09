use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    ReversalResult, ReverseVoucherRequest, ValidationMode, Voucher, VoucherLine, VoucherWithLines,
};
use crate::vouchers::service::{DoubleEntryValidator, VoucherSequenceService};
use crate::vouchers::validator::validate_voucher_date;
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;

/// Authoritative service handling atomic voucher reversal under SQLite BEGIN IMMEDIATE lock.
pub struct VoucherReversalService;

impl VoucherReversalService {
    /// Atomically reverses an existing posted voucher, generating a 1:1 inverted
    /// reversing journal voucher in the dedicated REV series.
    ///
    /// # Lifecycle Transitions:
    /// - Original voucher: `POSTED -> REVERSED`
    /// - Reversing voucher: `DRAFT -> POSTED` under `vt_reversing_journal`
    ///
    /// # Immutability & Audit:
    /// - Inverts each debit leg to credit and each credit leg to debit with identical amounts.
    /// - Preserves line numbers and ledger associations.
    /// - Emits dual `audit_log` records:
    ///   * `action = 'POST'` for reversing voucher
    ///   * `action = 'REVERSE'` for original voucher
    /// - Preserves connection safety and cleans up on failure via `OBS-5B-01`.
    pub async fn reverse_voucher(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: ReverseVoucherRequest,
    ) -> Result<ReversalResult, OpenTallyError> {
        // Step 1: Validate reason string (trimmed length >= 3)
        let reason = req.reason.trim();
        if reason.len() < 3 {
            return Err(OpenTallyError::Validation(
                "Reversal reason is mandatory (minimum 3 characters)".to_string(),
            ));
        }

        // Step 2: Dedicated connection acquisition and exclusive serialized write lock
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let result = Self::execute_reversal_transaction(
            &mut conn,
            company_id,
            &req,
            reason,
            req.actor_id.as_deref().unwrap_or("system"),
        )
        .await;

        match result {
            Ok(reversal_result) => match sqlx::query("COMMIT").execute(&mut *conn).await {
                Ok(_) => Ok(reversal_result),
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

    async fn execute_reversal_transaction(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        req: &ReverseVoucherRequest,
        reason: &str,
        actor_id: &str,
    ) -> Result<ReversalResult, OpenTallyError> {
        // Step 3: Load target voucher header within company tenant boundary
        let orig_voucher = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&req.original_voucher_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Voucher '{}' not found in active company",
                req.original_voucher_id
            ))
        })?;

        // Step 4: Status validation
        if orig_voucher.status == "draft" {
            return Err(OpenTallyError::Validation(
                "Draft vouchers cannot be reversed; use delete or edit".to_string(),
            ));
        }
        if orig_voucher.status == "cancelled" {
            return Err(OpenTallyError::Validation(
                "Cannot reverse a cancelled voucher".to_string(),
            ));
        }
        if orig_voucher.status == "reversed" {
            return Err(OpenTallyError::Validation(
                "Voucher is already reversed".to_string(),
            ));
        }
        if orig_voucher.status != "posted" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot reverse voucher in status '{}'",
                orig_voucher.status
            )));
        }

        // Step 5: Duplicate reversal pre-check
        let existing_reversal: Option<(String, String)> = sqlx::query_as(
            "SELECT id, voucher_number FROM vouchers WHERE company_id = $1 AND reversal_of_voucher_id = $2 AND status = 'posted'",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((rev_id, rev_num)) = existing_reversal {
            return Err(OpenTallyError::Validation(format!(
                "Cannot reverse voucher: already reversed by voucher '{}' ({})",
                rev_num, rev_id
            )));
        }

        // Step 6: Validate fiscal boundaries and reversal date
        let (_fy_begins, books_beginning, precision): (String, String, i64) = sqlx::query_as(
            "SELECT financial_year_begins, books_beginning_from, currency_decimal_places FROM companies WHERE id = $1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Company '{}' not found", company_id)))?;

        validate_voucher_date(&req.reversal_date, &books_beginning)?;

        if req.reversal_date < orig_voucher.date {
            return Err(OpenTallyError::Validation(format!(
                "Reversal date ({}) cannot precede original voucher date ({})",
                req.reversal_date, orig_voucher.date
            )));
        }

        // Step 7: Resolve dedicated series for vt_reversing_journal
        let rev_series_row = sqlx::query(
            "SELECT id, series_name, series_code, numbering_method, restart_period, is_active
             FROM voucher_number_series
             WHERE company_id = $1 AND voucher_type_id = 'vt_reversing_journal' AND is_active = 1
             ORDER BY (series_code = 'REV') DESC, created_at ASC
             LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::Validation(
                "No active numbering series found for Reversing Journal (vt_reversing_journal)"
                    .to_string(),
            )
        })?;

        let rev_series_id: String = rev_series_row.get("id");

        // Step 8: Allocate sequence number in caller's transaction
        let allocated = VoucherSequenceService::allocate_next_sequence(
            conn,
            company_id,
            "vt_reversing_journal",
            &rev_series_id,
            &req.reversal_date,
        )
        .await?;

        // Step 9: Load original lines and generate 1:1 inverted lines (Dr <-> Cr)
        let orig_lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if orig_lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "Cannot reverse voucher with zero lines".to_string(),
            ));
        }

        let rev_voucher_id = Uuid::new_v4().to_string();
        let mut rev_lines: Vec<VoucherLine> = Vec::with_capacity(orig_lines.len());

        for line in &orig_lines {
            let inverted_entry_type = match line.entry_type.as_str() {
                "Dr" => "Cr".to_string(),
                "Cr" => "Dr".to_string(),
                other => {
                    return Err(OpenTallyError::Validation(format!(
                        "Invalid entry_type '{}' in voucher line {}",
                        other, line.line_number
                    )))
                }
            };

            rev_lines.push(VoucherLine {
                id: Uuid::new_v4().to_string(),
                company_id: company_id.to_string(),
                voucher_id: rev_voucher_id.clone(),
                line_number: line.line_number,
                ledger_id: line.ledger_id.clone(),
                entry_type: inverted_entry_type,
                amount: line.amount.clone(),
                narration: Some(format!("Reversal of line {}: {}", line.line_number, reason)),
                created_at: String::new(),
            });
        }

        // Step 10: Validate double-entry balance on inverted lines
        let val_res =
            DoubleEntryValidator::validate(&rev_lines, precision as u32, ValidationMode::Posting);
        if !val_res.is_valid || !val_res.is_balanced {
            let error_details = val_res
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(OpenTallyError::Validation(format!(
                "Double-entry validation failed for reversal voucher: {}",
                error_details
            )));
        }

        // Step 11: Insert reversing voucher header as draft
        let rev_narration = Some(format!(
            "Reversal of voucher {}: {}",
            orig_voucher.voucher_number, reason
        ));
        let ref_no = Some(orig_voucher.voucher_number.clone());
        let ref_date = Some(orig_voucher.date.clone());

        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, reference_no, reference_date, narration, total_amount,
                reversal_of_voucher_id, created_at, updated_at
             ) VALUES ($1, $2, 'vt_reversing_journal', $3, 'automatic',
                       NULL, NULL, 'DRAFT', $4, $4,
                       'draft', $5, $6, $7, $8,
                       $9, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&rev_voucher_id)
        .bind(company_id)
        .bind(&rev_series_id)
        .bind(&req.reversal_date)
        .bind(&ref_no)
        .bind(&ref_date)
        .bind(&rev_narration)
        .bind(&orig_voucher.total_amount)
        .bind(&orig_voucher.id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 12: Insert inverted lines into voucher_lines
        for line in &rev_lines {
            sqlx::query(
                "INSERT INTO voucher_lines (
                    id, company_id, voucher_id, line_number, ledger_id,
                    entry_type, amount, narration, created_at
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)",
            )
            .bind(&line.id)
            .bind(company_id)
            .bind(&line.voucher_id)
            .bind(line.line_number)
            .bind(&line.ledger_id)
            .bind(&line.entry_type)
            .bind(&line.amount)
            .bind(&line.narration)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Step 13: Promote reversing voucher from draft to posted
        // Note: trg_validate_reversal_target_on_post fires and verifies orig_voucher is 'posted'
        let rows_rev = sqlx::query(
            "UPDATE vouchers
             SET status = 'posted',
                 period_start = $3,
                 sequence_number = $4,
                 voucher_number = $5,
                 updated_at = CURRENT_TIMESTAMP
             WHERE company_id = $1 AND id = $2 AND status = 'draft'",
        )
        .bind(company_id)
        .bind(&rev_voucher_id)
        .bind(&allocated.period_start)
        .bind(allocated.sequence_number)
        .bind(&allocated.voucher_number)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows_rev != 1 {
            return Err(OpenTallyError::Database(sqlx::Error::RowNotFound));
        }

        // Step 14: Transition original voucher from posted to reversed
        // Note: trg_prevent_posted_voucher_mutation Rule 5 fires and verifies posted reversal exists
        let rows_orig = sqlx::query(
            "UPDATE vouchers
             SET status = 'reversed',
                 updated_at = CURRENT_TIMESTAMP
             WHERE company_id = $1 AND id = $2 AND status = 'posted'",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows_orig != 1 {
            return Err(OpenTallyError::Database(sqlx::Error::RowNotFound));
        }

        // Step 15: Dual audit logging
        // A. POST action for reversing voucher
        let rev_audit_id = Uuid::new_v4().to_string();
        let rev_new_values = serde_json::json!({
            "status": "posted",
            "voucher_number": allocated.voucher_number,
            "voucher_type_id": "vt_reversing_journal",
            "reversal_of_voucher_id": orig_voucher.id,
            "total_amount": orig_voucher.total_amount,
            "date": req.reversal_date,
            "reason": reason,
        });

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
             ) VALUES ($1, $2, 'voucher', $3, NULL, $4, 'POST', NULL, $5, $6)",
        )
        .bind(&rev_audit_id)
        .bind(company_id)
        .bind(&rev_voucher_id)
        .bind(&allocated.period_start)
        .bind(rev_new_values.to_string())
        .bind(actor_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // B. REVERSE action for original voucher
        let orig_audit_id = Uuid::new_v4().to_string();
        let orig_old_values = serde_json::json!({
            "status": "posted"
        });
        let orig_new_values = serde_json::json!({
            "status": "reversed",
            "reversal_voucher_id": rev_voucher_id,
            "reversal_voucher_number": allocated.voucher_number,
            "reason": reason,
        });

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
             ) VALUES ($1, $2, 'voucher', $3, NULL, $4, 'REVERSE', $5, $6, $7)",
        )
        .bind(&orig_audit_id)
        .bind(company_id)
        .bind(&orig_voucher.id)
        .bind(&orig_voucher.period_start)
        .bind(orig_old_values.to_string())
        .bind(orig_new_values.to_string())
        .bind(actor_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 16: Foreign key check
        let fk_violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if !fk_violations.is_empty() {
            return Err(OpenTallyError::Validation(format!(
                "Foreign key integrity violation during reversal: {} violation(s)",
                fk_violations.len()
            )));
        }

        // Step 17: Fetch fully hydrated records for result
        let updated_orig_voucher = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let updated_orig_lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let rev_voucher = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&rev_voucher_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let fetched_rev_lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(&rev_voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let orig_val = DoubleEntryValidator::validate(
            &updated_orig_lines,
            precision as u32,
            ValidationMode::Draft,
        );
        let rev_val = DoubleEntryValidator::validate(
            &fetched_rev_lines,
            precision as u32,
            ValidationMode::Draft,
        );

        Ok(ReversalResult {
            original_voucher: VoucherWithLines {
                voucher: updated_orig_voucher,
                lines: updated_orig_lines,
                debit_total: orig_val.debit_total.to_string(),
                credit_total: orig_val.credit_total.to_string(),
                difference: orig_val.difference.to_string(),
                is_balanced: orig_val.is_balanced,
            },
            reversing_voucher: VoucherWithLines {
                voucher: rev_voucher,
                lines: fetched_rev_lines,
                debit_total: rev_val.debit_total.to_string(),
                credit_total: rev_val.credit_total.to_string(),
                difference: rev_val.difference.to_string(),
                is_balanced: rev_val.is_balanced,
            },
        })
    }
}
