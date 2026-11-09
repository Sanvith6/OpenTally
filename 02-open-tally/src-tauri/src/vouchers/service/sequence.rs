use crate::errors::OpenTallyError;
use crate::vouchers::formatter::CanonicalVoucherFormatter;
use crate::vouchers::model::{
    AllocatedVoucherNumber, EffectiveNumberConfig, SequenceRecoveryReport,
};
use crate::vouchers::service::period::derive_period_start;
use sqlx::{Row, SqliteConnection};
use uuid::Uuid;

/// Authoritative service handling effective configuration resolution,
/// atomic transaction-bound sequence allocation, and sequence recovery governance.
pub struct VoucherSequenceService;

impl VoucherSequenceService {
    /// Authoritative helper: Resolves the effective numbering configuration and period anchor
    /// for a series on a given voucher date in a single deterministic pass.
    pub async fn resolve_effective_config(
        conn: &mut SqliteConnection,
        company_id: &str,
        series_id: &str,
        voucher_date: &str,
    ) -> Result<EffectiveNumberConfig, OpenTallyError> {
        // 1. Retrieve company fiscal boundaries
        let (fy_begins, books_beginning_from): (String, String) = sqlx::query_as(
            "SELECT financial_year_begins, books_beginning_from FROM companies WHERE id = $1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!("Company with ID '{}' not found", company_id))
        })?;

        // 2. Fetch series master to verify existence, active status, and read immutable restart_period
        let series_row = sqlx::query(
            "SELECT id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active
             FROM voucher_number_series
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!("Numbering series '{}' not found", series_id))
        })?;

        let is_active: i64 = series_row.get("is_active");
        let series_name: String = series_row.get("series_name");
        let series_code: String = series_row.get("series_code");
        let voucher_type_id: String = series_row.get("voucher_type_id");
        let restart_period: String = series_row.get("restart_period");

        if is_active != 1 {
            return Err(OpenTallyError::Validation(format!(
                "Numbering series '{}' ({}) is inactive; posting blocked",
                series_name, series_code
            )));
        }

        // 3. Derive period_start in a single pass directly from immutable series.restart_period
        let period_start = derive_period_start(
            voucher_date,
            &restart_period,
            &fy_begins,
            &books_beginning_from,
        )?;

        // 4. Resolve configuration version applicable as of the effective period anchor
        // effective_anchor = max(period_start, books_beginning_from)
        let effective_anchor = std::cmp::max(period_start.as_str(), books_beginning_from.as_str());

        let config_row = sqlx::query(
            "SELECT id, prefix, suffix, width, start_number, applicable_from
             FROM voucher_number_configs
             WHERE company_id = $1 AND series_id = $2 AND applicable_from <= $3
             ORDER BY applicable_from DESC, created_at DESC
             LIMIT 1",
        )
        .bind(company_id)
        .bind(series_id)
        .bind(effective_anchor)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::Validation(format!(
                "Missing numbering configuration for series '{}' ({}) for period starting {} (effective anchor {}). An initial numbering rule must be configured with applicable_from on or before {}. Implicit fallback to default configuration (start_number = 1) is strictly prohibited.",
                series_name, series_code, period_start, effective_anchor, effective_anchor
            ))
        })?;

        let config_id: String = config_row.get("id");
        let prefix: String = config_row.get("prefix");
        let suffix: String = config_row.get("suffix");
        let width: i64 = config_row.get("width");
        let start_number: i64 = config_row.get("start_number");
        let applicable_from: String = config_row.get("applicable_from");

        Ok(EffectiveNumberConfig {
            config_id,
            voucher_type_id,
            series_id: series_id.to_string(),
            prefix,
            suffix,
            width,
            start_number,
            restart_period,
            applicable_from,
            period_start,
        })
    }

    /// Atomically allocates the next sequential voucher number for a period within the active transaction.
    /// Uses SQLite UPSERT with RETURNING to guarantee gapless monotonic allocation.
    /// CRITICAL TRANSACTION RULE: Does not commit; participates in the caller's transaction.
    pub async fn allocate_next_sequence(
        conn: &mut SqliteConnection,
        company_id: &str,
        voucher_type_id: &str,
        series_id: &str,
        voucher_date: &str,
    ) -> Result<AllocatedVoucherNumber, OpenTallyError> {
        // 1. Resolve effective configuration and period anchor
        let config =
            Self::resolve_effective_config(conn, company_id, series_id, voucher_date).await?;

        // 2. Verify series numbering method is automatic
        let series_row = sqlx::query(
            "SELECT series_code, numbering_method FROM voucher_number_series WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let series_code: String = series_row.get("series_code");
        let numbering_method: String = series_row.get("numbering_method");

        if numbering_method != "automatic" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot allocate sequence for non-automatic series '{}' (method: {})",
                series_id, numbering_method
            )));
        }

        // Prevent sequence overflow
        if config.start_number > i64::MAX - 2 {
            return Err(OpenTallyError::Validation(
                "Sequence number overflow error".into(),
            ));
        }

        // 3. Atomic SQLite UPSERT on sequence counter (company_id, series_id, period_start)
        // Counter Semantics: voucher_number_sequences.next_number stores next_number_to_allocate.
        // - New period bucket: Inserts next_number = config.start_number + 1, RETURNING (next_number - 1) = config.start_number.
        // - Existing period bucket: Increments next_number = next_number + 1, RETURNING (new next_number - 1) = old next_number.
        let seq_id = Uuid::new_v4().to_string();
        let initial_next = config.start_number + 1;

        let row = sqlx::query(
            "INSERT INTO voucher_number_sequences 
             (id, company_id, voucher_type_id, series_id, period_start, next_number)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (company_id, series_id, period_start)
             DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
             RETURNING (voucher_number_sequences.next_number - 1) AS allocated_number",
        )
        .bind(&seq_id)
        .bind(company_id)
        .bind(voucher_type_id)
        .bind(series_id)
        .bind(&config.period_start)
        .bind(initial_next)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let allocated_number: i64 = row.get("allocated_number");

        // 4. Format permanent voucher number string using CanonicalVoucherFormatter
        let formatted_str = CanonicalVoucherFormatter::format_automatic(
            &series_code,
            &config.prefix,
            &config.suffix,
            allocated_number,
            config.width as usize,
            &config.period_start,
        );

        Ok(AllocatedVoucherNumber {
            voucher_number: formatted_str,
            period_start: config.period_start,
            sequence_number: allocated_number,
            series_id: series_id.to_string(),
            config_id: config.config_id,
        })
    }

    /// Read-only inspection of sequence counters, permanent allocations, and gaps.
    pub async fn inspect_sequence(
        conn: &mut SqliteConnection,
        company_id: &str,
        series_id: &str,
        period_start: &str,
    ) -> Result<SequenceRecoveryReport, OpenTallyError> {
        Self::execute_sequence_diagnostics(
            conn,
            company_id,
            series_id,
            period_start,
            "system_inspector",
            "inspect_only",
            None,
            false,
        )
        .await
    }

    /// 10-step safe sequence recovery algorithm executing under caller's transaction lock.
    pub async fn resync_voucher_sequence(
        conn: &mut SqliteConnection,
        company_id: &str,
        series_id: &str,
        period_start: &str,
        operator_id: &str,
        resolution_mode: &str,
        resolution_reason: Option<&str>,
    ) -> Result<SequenceRecoveryReport, OpenTallyError> {
        Self::execute_sequence_diagnostics(
            conn,
            company_id,
            series_id,
            period_start,
            operator_id,
            resolution_mode,
            resolution_reason,
            true, // apply_repairs = true
        )
        .await
    }

    /// Internal core procedure implementing sequence inspection and repair logic.
    #[allow(clippy::too_many_arguments)]
    async fn execute_sequence_diagnostics(
        conn: &mut SqliteConnection,
        company_id: &str,
        series_id: &str,
        period_start: &str,
        operator_id: &str,
        resolution_mode: &str,
        resolution_reason: Option<&str>,
        apply_repairs: bool,
    ) -> Result<SequenceRecoveryReport, OpenTallyError> {
        // Step 1: Verify series master
        let series_row = sqlx::query(
            "SELECT voucher_type_id, series_name, series_code, numbering_method FROM voucher_number_series WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!("Numbering series '{}' not found", series_id))
        })?;

        let voucher_type_id: String = series_row.get("voucher_type_id");
        let series_name: String = series_row.get("series_name");
        let series_code: String = series_row.get("series_code");
        let numbering_method: String = series_row.get("numbering_method");

        if numbering_method != "automatic" {
            return Err(OpenTallyError::Validation(format!(
                "Sequence recovery only applies to automatic series; series '{}' is {}",
                series_code, numbering_method
            )));
        }

        // Step 2: Resolve configured starting baseline S_0 (NO DEFAULT FALLBACK)
        let config_row = sqlx::query(
            "SELECT start_number 
             FROM voucher_number_configs 
             WHERE company_id = $1 AND series_id = $2 AND applicable_from <= $3 
             ORDER BY applicable_from DESC, created_at DESC 
             LIMIT 1",
        )
        .bind(company_id)
        .bind(series_id)
        .bind(period_start)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::IntegrityViolation(format!(
                "Missing numbering configuration for series '{}' ({}) applicable as of period start {}. A numbering rule must be explicitly configured in voucher_number_configs before sequence recovery can run. Implicit fallback to start_number = 1 is strictly prohibited.",
                series_name, series_code, period_start
            ))
        })?;

        let start_number_baseline: i64 = config_row.get("start_number");

        // Step 3: Read current next_number (N) from sequence table
        let seq_row = sqlx::query(
            "SELECT id, next_number 
             FROM voucher_number_sequences 
             WHERE company_id = $1 AND series_id = $2 AND period_start = $3",
        )
        .bind(company_id)
        .bind(series_id)
        .bind(period_start)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let (seq_id, previous_next_number) = match seq_row {
            Some(row) => (row.get::<String, _>("id"), row.get::<i64, _>("next_number")),
            None => {
                // If sequence row does not exist yet, initialize it
                let new_seq_id = Uuid::new_v4().to_string();
                if apply_repairs {
                    sqlx::query(
                        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
                         VALUES ($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(&new_seq_id)
                    .bind(company_id)
                    .bind(&voucher_type_id)
                    .bind(series_id)
                    .bind(period_start)
                    .bind(start_number_baseline)
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                }
                (new_seq_id, start_number_baseline)
            }
        };

        // Step 4: Inspect permanent automatic voucher allocations
        // Manual vouchers excluded; cancelled and reversed vouchers included
        let voucher_rows = sqlx::query(
            "SELECT sequence_number, period_start, status
             FROM vouchers
             WHERE company_id = $1 AND series_id = $2 AND (period_start = $3 OR period_start IS NULL)
               AND status IN ('posted', 'reversed', 'cancelled') AND numbering_method = 'automatic'
             ORDER BY sequence_number ASC",
        )
        .bind(company_id)
        .bind(series_id)
        .bind(period_start)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let total_permanent_allocations = voucher_rows.len();

        // Step 5: Detect anomalies (missing metadata, duplicates, below-baseline allocations)
        let mut allocated_numbers = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for v in &voucher_rows {
            let seq_num_opt: Option<i64> = v.get("sequence_number");
            let p_start_opt: Option<String> = v.get("period_start");

            if seq_num_opt.is_none() || p_start_opt.is_none() {
                return Err(OpenTallyError::IntegrityViolation(format!(
                    "Permanent automatic voucher in series '{}' is missing required sequence metadata (period_start: {:?}, sequence_number: {:?})",
                    series_code, p_start_opt, seq_num_opt
                )));
            }

            let num = seq_num_opt.unwrap();
            if num < start_number_baseline {
                return Err(OpenTallyError::IntegrityViolation(format!(
                    "Allocations detected below configured start_number ({}): {:?}. Manual database administrative review required.",
                    start_number_baseline, num
                )));
            }

            if !seen.insert(num) {
                return Err(OpenTallyError::IntegrityViolation(format!(
                    "Duplicate sequence allocation detected in series '{}' for number {}",
                    series_code, num
                )));
            }

            allocated_numbers.push(num);
        }

        // Step 6: Compute maximum allocated sequence number M
        let max_allocated = allocated_numbers.last().copied().unwrap_or(0);

        // Step 7: Detect historical sequence gaps in baseline universe [S_0, M]
        // Safety bound: prevent unbounded memory allocation if counter/data is wildly corrupted
        if (max_allocated.saturating_sub(start_number_baseline)) > 100_000 {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Sequence span between baseline ({}) and max allocated ({}) exceeds 100,000. Automatic memory-bound recovery aborted; manual database audit required.",
                start_number_baseline, max_allocated
            )));
        }

        let mut historical_gaps = Vec::new();
        if max_allocated >= start_number_baseline {
            let allocated_set: std::collections::HashSet<i64> =
                allocated_numbers.iter().copied().collect();
            for expected in start_number_baseline..max_allocated {
                if !allocated_set.contains(&expected) {
                    historical_gaps.push(expected);
                }
            }
        }

        // Step 8: Detect counter-tail gaps in [expected_next, N - 1] when N > expected_next
        let expected_next = if max_allocated >= start_number_baseline {
            max_allocated + 1
        } else {
            start_number_baseline
        };

        if (previous_next_number.saturating_sub(expected_next)) > 100_000 {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Counter tail gap span between expected next ({}) and recorded next ({}) exceeds 100,000. Automatic recovery aborted; manual database audit required.",
                expected_next, previous_next_number
            )));
        }

        let mut counter_tail_gaps = Vec::new();
        if previous_next_number > expected_next {
            for gap in expected_next..previous_next_number {
                counter_tail_gaps.push(gap);
            }
        }

        // Step 9: Enforce gap policy across historical and tail gaps
        let all_gaps_exist = !historical_gaps.is_empty() || !counter_tail_gaps.is_empty();

        if all_gaps_exist && resolution_mode != "allow_historical_gaps" && apply_repairs {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Sequence gaps detected: historical_gaps={:?}, counter_tail_gaps={:?}. Automatic recovery strictly blocked; manual review or explicit 'allow_historical_gaps' policy required.",
                historical_gaps, counter_tail_gaps
            )));
        }

        if resolution_mode == "allow_historical_gaps" && apply_repairs {
            match resolution_reason {
                Some(r) if !r.trim().is_empty() => {}
                _ => {
                    return Err(OpenTallyError::Validation(
                        "resolution_reason is mandatory when resolution_mode is 'allow_historical_gaps'"
                            .into(),
                    ));
                }
            }
        }

        // Step 10: Sequence counter update policy
        // Lagged counter: Advance next_number to expected_next
        // Ahead counter: Counter is NEVER lowered; preserved at previous_next_number
        let mut counter_updated = false;
        let new_next_number = if previous_next_number < expected_next {
            if apply_repairs {
                sqlx::query(
                    "UPDATE voucher_number_sequences 
                     SET next_number = $1, updated_at = CURRENT_TIMESTAMP 
                     WHERE company_id = $2 AND id = $3",
                )
                .bind(expected_next)
                .bind(company_id)
                .bind(&seq_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
                counter_updated = true;
            }
            expected_next
        } else {
            previous_next_number
        };

        // Write immutable audit trail if applying repairs
        if apply_repairs {
            let audit_id = Uuid::new_v4().to_string();
            let details = serde_json::json!({
                "voucher_type_id": voucher_type_id,
                "series_id": series_id,
                "period_start": period_start,
                "start_number_baseline": start_number_baseline,
                "previous_next_number": previous_next_number,
                "new_next_number": new_next_number,
                "max_allocated": max_allocated,
                "total_permanent_allocations": total_permanent_allocations,
                "historical_gaps": historical_gaps,
                "counter_tail_gaps": counter_tail_gaps,
                "counter_updated": counter_updated,
                "operator_id": operator_id,
                "resolution_mode": resolution_mode,
                "resolution_reason": resolution_reason,
            });

            sqlx::query(
                "INSERT INTO audit_log 
                 (id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by)
                 VALUES ($1, $2, 'voucher_number_sequences', $3, NULL, NULL, 'UPDATE', NULL, $4, $5)",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(&seq_id)
            .bind(details.to_string())
            .bind(operator_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(SequenceRecoveryReport {
            company_id: company_id.to_string(),
            voucher_type_id,
            series_id: series_id.to_string(),
            period_start: period_start.to_string(),
            start_number_baseline,
            previous_next_number,
            new_next_number,
            max_allocated,
            total_permanent_allocations,
            historical_gaps,
            counter_tail_gaps,
            counter_updated,
            operator_id: operator_id.to_string(),
            resolution_mode: resolution_mode.to_string(),
            resolution_reason: resolution_reason.map(|s| s.to_string()),
        })
    }
}
