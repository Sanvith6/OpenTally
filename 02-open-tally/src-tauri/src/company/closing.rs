use crate::errors::OpenTallyError;
use crate::reports::model::TrialBalanceRequest;
use crate::reports::trial_balance::TrialBalanceService;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinancialYearClosingRecord {
    pub id: String,
    pub company_id: String,
    pub financial_year_label: String,
    pub period_start: String,
    pub period_end: String,
    pub next_fy_start: String,
    pub closing_voucher_id: String,
    pub net_profit_or_loss: String,
    pub is_locked: bool,
    pub closed_at: String,
    pub closed_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NominalTransferPreview {
    pub ledger_id: String,
    pub ledger_name: String,
    pub account_group_nature: String,
    pub closing_debit: String,
    pub closing_credit: String,
    pub transfer_entry_type: String, // 'Dr' or 'Cr' needed to zero it out
    pub transfer_amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceSheetCarryForwardPreview {
    pub ledger_id: String,
    pub ledger_name: String,
    pub account_group_nature: String,
    pub balance_type: String, // 'Dr' or 'Cr'
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct YearEndClosingPreview {
    pub company_id: String,
    pub financial_year_label: String,
    pub period_start: String,
    pub period_end: String,
    pub next_fy_start: String,
    pub trial_balance_difference: String,
    pub is_equilibrium: bool,
    pub net_profit_or_loss: String,
    pub is_profit: bool,
    pub nominal_transfers: Vec<NominalTransferPreview>,
    pub balance_sheet_balances: Vec<BalanceSheetCarryForwardPreview>,
}

pub struct FinancialYearClosingService;

impl FinancialYearClosingService {
    /// Validates whether a given transaction date falls inside a locked financial year closing.
    pub async fn validate_period_not_locked<'e, E>(
        executor: E,
        company_id: &str,
        date: &str,
    ) -> Result<(), OpenTallyError>
    where
        E: sqlx::Executor<'e, Database = Sqlite>,
    {
        let locked_period_res = sqlx::query_scalar::<_, String>(
            "SELECT period_end FROM financial_year_closings
             WHERE company_id = $1 AND is_locked = 1 AND period_end >= $2
             ORDER BY period_end DESC LIMIT 1",
        )
        .bind(company_id)
        .bind(date)
        .fetch_optional(executor)
        .await;

        let locked_period: Option<String> = match locked_period_res {
            Ok(opt) => opt,
            Err(sqlx::Error::Database(ref db_err))
                if db_err
                    .message()
                    .contains("no such table: financial_year_closings") =>
            {
                None
            }
            Err(e) => return Err(OpenTallyError::Database(e)),
        };

        if let Some(period_end) = locked_period {
            return Err(OpenTallyError::PeriodLocked(format!(
                "Transaction date '{}' falls within locked financial year period ending '{}'",
                date, period_end
            )));
        }

        Ok(())
    }

    /// Returns the maximum locked period_end date for a company, if any.
    pub async fn get_max_locked_period(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Option<String>, OpenTallyError> {
        let max_locked: Option<String> = sqlx::query_scalar(
            "SELECT MAX(period_end) FROM financial_year_closings WHERE company_id = ? AND is_locked = 1",
        )
        .bind(company_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(max_locked)
    }

    /// Lists all closed financial years for a company.
    pub async fn list_closed_years(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<FinancialYearClosingRecord>, OpenTallyError> {
        let rows = sqlx::query(
            "SELECT id, company_id, financial_year_label, period_start, period_end, next_fy_start,
                    closing_voucher_id, net_profit_or_loss, is_locked, closed_at, closed_by
             FROM financial_year_closings
             WHERE company_id = ?
             ORDER BY period_end DESC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut list = Vec::with_capacity(rows.len());
        for r in rows {
            let is_locked_int: i64 = r.try_get("is_locked").map_err(OpenTallyError::Database)?;
            list.push(FinancialYearClosingRecord {
                id: r.try_get("id").map_err(OpenTallyError::Database)?,
                company_id: r.try_get("company_id").map_err(OpenTallyError::Database)?,
                financial_year_label: r
                    .try_get("financial_year_label")
                    .map_err(OpenTallyError::Database)?,
                period_start: r
                    .try_get("period_start")
                    .map_err(OpenTallyError::Database)?,
                period_end: r.try_get("period_end").map_err(OpenTallyError::Database)?,
                next_fy_start: r
                    .try_get("next_fy_start")
                    .map_err(OpenTallyError::Database)?,
                closing_voucher_id: r
                    .try_get("closing_voucher_id")
                    .map_err(OpenTallyError::Database)?,
                net_profit_or_loss: r
                    .try_get("net_profit_or_loss")
                    .map_err(OpenTallyError::Database)?,
                is_locked: is_locked_int == 1,
                closed_at: r.try_get("closed_at").map_err(OpenTallyError::Database)?,
                closed_by: r.try_get("closed_by").map_err(OpenTallyError::Database)?,
            });
        }

        Ok(list)
    }

    /// Previews year-end closing without modifying the database.
    pub async fn preview_year_end_closing(
        pool: &Pool<Sqlite>,
        company_id: &str,
        period_end: &str,
    ) -> Result<YearEndClosingPreview, OpenTallyError> {
        let end_date = NaiveDate::from_str(period_end).map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid period_end date '{}', expected YYYY-MM-DD",
                period_end
            ))
        })?;

        let next_fy_start = end_date
            .succ_opt()
            .ok_or_else(|| OpenTallyError::Validation("Next date overflow".to_string()))?
            .format("%Y-%m-%d")
            .to_string();

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
                })?;

        // Determine period_start: prior closing's next_fy_start or company financial_year_begins
        let prev_closing_next: Option<String> = sqlx::query_scalar(
            "SELECT next_fy_start FROM financial_year_closings
             WHERE company_id = ? AND period_end < ?
             ORDER BY period_end DESC LIMIT 1",
        )
        .bind(company_id)
        .bind(period_end)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let period_start = prev_closing_next.unwrap_or(fy_begins);

        // Compute Financial Year Label (e.g. FY 2024-25)
        let label = format!(
            "FY {}-{}",
            &period_start[..4.min(period_start.len())],
            &period_end[2..4.min(period_end.len())]
        );

        // Pre-flight check: Trial Balance
        let tb = TrialBalanceService::generate(
            pool,
            company_id,
            TrialBalanceRequest {
                as_of_date: period_end.to_string(),
                account_group_id: None,
                ledger_id: None,
                account_search: None,
                include_zero_balances: false,
            },
        )
        .await?;

        let tb_diff = Decimal::from_str(&tb.difference).unwrap_or(Decimal::ZERO);
        let is_equilibrium = tb_diff == Decimal::ZERO;

        let mut nominal_transfers = Vec::new();
        let mut balance_sheet_balances = Vec::new();
        let mut total_income = Decimal::ZERO;
        let mut total_expense = Decimal::ZERO;

        for row in tb.rows {
            let dr = Decimal::from_str(&row.closing_debit).unwrap_or(Decimal::ZERO);
            let cr = Decimal::from_str(&row.closing_credit).unwrap_or(Decimal::ZERO);

            if row.account_group_nature == "Income" {
                let net_cr = cr - dr;
                total_income += net_cr;
                if net_cr > Decimal::ZERO {
                    // Credit balance -> Zero out by Debiting Income, Crediting P&L
                    nominal_transfers.push(NominalTransferPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        closing_debit: row.closing_debit,
                        closing_credit: row.closing_credit,
                        transfer_entry_type: "Dr".to_string(),
                        transfer_amount: net_cr.to_string(),
                    });
                } else if net_cr < Decimal::ZERO {
                    // Debit balance in Income -> Zero out by Crediting Income, Debiting P&L
                    let net_dr = dr - cr;
                    nominal_transfers.push(NominalTransferPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        closing_debit: row.closing_debit,
                        closing_credit: row.closing_credit,
                        transfer_entry_type: "Cr".to_string(),
                        transfer_amount: net_dr.to_string(),
                    });
                }
            } else if row.account_group_nature == "Expenses" {
                let net_dr = dr - cr;
                total_expense += net_dr;
                if net_dr > Decimal::ZERO {
                    // Debit balance -> Zero out by Crediting Expense, Debiting P&L
                    nominal_transfers.push(NominalTransferPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        closing_debit: row.closing_debit,
                        closing_credit: row.closing_credit,
                        transfer_entry_type: "Cr".to_string(),
                        transfer_amount: net_dr.to_string(),
                    });
                } else if net_dr < Decimal::ZERO {
                    let net_cr = cr - dr;
                    nominal_transfers.push(NominalTransferPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        closing_debit: row.closing_debit,
                        closing_credit: row.closing_credit,
                        transfer_entry_type: "Dr".to_string(),
                        transfer_amount: net_cr.to_string(),
                    });
                }
            } else {
                // Balance sheet account (Assets, Liabilities)
                if dr > cr {
                    balance_sheet_balances.push(BalanceSheetCarryForwardPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        balance_type: "Dr".to_string(),
                        amount: (dr - cr).to_string(),
                    });
                } else if cr > dr {
                    balance_sheet_balances.push(BalanceSheetCarryForwardPreview {
                        ledger_id: row.ledger_id,
                        ledger_name: row.ledger_name,
                        account_group_nature: row.account_group_nature,
                        balance_type: "Cr".to_string(),
                        amount: (cr - dr).to_string(),
                    });
                }
            }
        }

        let net_profit_or_loss = total_income - total_expense;
        let is_profit = net_profit_or_loss >= Decimal::ZERO;

        Ok(YearEndClosingPreview {
            company_id: company_id.to_string(),
            financial_year_label: label,
            period_start,
            period_end: period_end.to_string(),
            next_fy_start,
            trial_balance_difference: tb.difference,
            is_equilibrium,
            net_profit_or_loss: net_profit_or_loss.to_string(),
            is_profit,
            nominal_transfers,
            balance_sheet_balances,
        })
    }

    /// Executes year-end closing under atomic BEGIN IMMEDIATE write lock.
    pub async fn execute_financial_year_closing(
        pool: &Pool<Sqlite>,
        company_id: &str,
        period_end: &str,
        closed_by: Option<&str>,
    ) -> Result<FinancialYearClosingRecord, OpenTallyError> {
        let preview = Self::preview_year_end_closing(pool, company_id, period_end).await?;

        if !preview.is_equilibrium {
            return Err(OpenTallyError::Validation(format!(
                "Trial balance difference is non-zero ({}); accounts must be in equilibrium before closing.",
                preview.trial_balance_difference
            )));
        }

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let result = Self::execute_closing_tx(
            &mut conn,
            company_id,
            preview,
            closed_by.unwrap_or("system"),
        )
        .await;

        match result {
            Ok(closing_record) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(closing_record)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    async fn execute_closing_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        preview: YearEndClosingPreview,
        actor: &str,
    ) -> Result<FinancialYearClosingRecord, OpenTallyError> {
        // 1. Verify period is not already closed
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM financial_year_closings WHERE company_id = ? AND period_end = ?",
        )
        .bind(company_id)
        .bind(&preview.period_end)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if existing.is_some() {
            return Err(OpenTallyError::Validation(format!(
                "Financial year ending '{}' has already been closed.",
                preview.period_end
            )));
        }

        // 2. Identify reserved Profit & Loss ledger
        let pnl_ledger_id: String = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND ledger_classification = 'profit_and_loss'",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound("Reserved Profit & Loss ledger not found".to_string()))?;

        // 3. Identify Journal voucher type & default series
        let vt_row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM voucher_types WHERE company_id = ? AND is_active = 1 AND (id = 'vt_jnl' OR name = 'Journal') LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let voucher_type_id = match vt_row {
            Some((id,)) => id,
            None => {
                return Err(OpenTallyError::NotFound(
                    "Active Journal voucher type not found for closing entry".to_string(),
                ));
            }
        };

        let series_row: Option<(String, String)> = sqlx::query_as(
            "SELECT id, numbering_method FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = ? AND is_active = 1 LIMIT 1",
        )
        .bind(company_id)
        .bind(&voucher_type_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let (series_id, numbering_method) = match series_row {
            Some(row) => (Some(row.0), row.1),
            None => (None, "manual".to_string()),
        };

        // 4. Generate Year-End Closing Journal Voucher
        let closing_vch_id = Uuid::new_v4().to_string();
        let short_uuid = &Uuid::new_v4().simple().to_string()[..4];
        let vch_number = format!(
            "YEC-{}-{}",
            preview.period_end.replace('-', ""),
            short_uuid.to_uppercase()
        );

        let net_pnl = Decimal::from_str(&preview.net_profit_or_loss).unwrap_or(Decimal::ZERO);

        // Build voucher lines
        // For each nominal transfer, we create a line to zero it out,
        // and one or more offsetting lines to Profit & Loss
        let mut lines = Vec::new();
        let mut line_num = 1;
        let mut total_dr = Decimal::ZERO;

        for transfer in &preview.nominal_transfers {
            let amt = Decimal::from_str(&transfer.transfer_amount).unwrap_or(Decimal::ZERO);
            if amt <= Decimal::ZERO {
                continue;
            }
            if transfer.transfer_entry_type == "Dr" {
                total_dr += amt;
            }
            lines.push((
                Uuid::new_v4().to_string(),
                line_num,
                transfer.ledger_id.clone(),
                transfer.transfer_entry_type.clone(),
                transfer.transfer_amount.clone(),
                format!("Year-End Transfer: {}", transfer.ledger_name),
            ));
            line_num += 1;
        }

        // Add P&L offsetting balance line(s)
        if net_pnl > Decimal::ZERO {
            // Net profit: Credit Profit & Loss
            lines.push((
                Uuid::new_v4().to_string(),
                line_num,
                pnl_ledger_id.clone(),
                "Cr".to_string(),
                net_pnl.to_string(),
                "Year-End Net Profit transferred to Retained Earnings".to_string(),
            ));
        } else if net_pnl < Decimal::ZERO {
            // Net loss: Debit Profit & Loss
            let abs_loss = net_pnl.abs();
            total_dr += abs_loss;
            lines.push((
                Uuid::new_v4().to_string(),
                line_num,
                pnl_ledger_id.clone(),
                "Dr".to_string(),
                abs_loss.to_string(),
                "Year-End Net Loss transferred to Retained Earnings".to_string(),
            ));
        }

        // Insert Voucher in 'draft' status first (to comply with SQLite triggers)
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, narration, total_amount, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, NULL, NULL, 'DRAFT', ?, ?, 'draft', ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&closing_vch_id)
        .bind(company_id)
        .bind(&voucher_type_id)
        .bind(&series_id)
        .bind(&numbering_method)
        .bind(&preview.period_end)
        .bind(&preview.period_end)
        .bind(format!("Automated Year-End Closing Journal for {}", preview.financial_year_label))
        .bind(total_dr.to_string())
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Insert voucher lines
        for (line_id, l_num, ledger_id, entry_type, amt, narration) in lines {
            sqlx::query(
                "INSERT INTO voucher_lines (
                    id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(line_id)
            .bind(company_id)
            .bind(&closing_vch_id)
            .bind(l_num)
            .bind(ledger_id)
            .bind(entry_type)
            .bind(amt)
            .bind(narration)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Transition voucher from 'draft' to 'posted'
        sqlx::query(
            "UPDATE vouchers SET status = 'posted', period_start = ?, sequence_number = 1, voucher_number = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
        )
        .bind(&preview.period_start)
        .bind(&vch_number)
        .bind(company_id)
        .bind(&closing_vch_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // 5. Roll forward Balance Sheet opening balances into next financial year
        // Next FY start date is preview.next_fy_start
        for bs in &preview.balance_sheet_balances {
            let op_id = Uuid::new_v4().to_string();
            // If the account is the P&L account, add net profit or loss
            let (final_type, final_amt) = if bs.ledger_id == pnl_ledger_id {
                let current_amt = Decimal::from_str(&bs.amount).unwrap_or(Decimal::ZERO);
                // Cr is positive retained earnings, Dr is negative
                let signed_current = if bs.balance_type == "Cr" {
                    current_amt
                } else {
                    -current_amt
                };
                let combined = signed_current + net_pnl;
                if combined >= Decimal::ZERO {
                    ("Cr".to_string(), combined.to_string())
                } else {
                    ("Dr".to_string(), combined.abs().to_string())
                }
            } else {
                (bs.balance_type.clone(), bs.amount.clone())
            };

            sqlx::query(
                "INSERT INTO opening_balances (
                    id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(company_id, ledger_id, financial_year_begins) DO UPDATE SET
                    balance_type = excluded.balance_type,
                    amount = excluded.amount,
                    updated_at = CURRENT_TIMESTAMP",
            )
            .bind(op_id)
            .bind(company_id)
            .bind(&bs.ledger_id)
            .bind(&preview.next_fy_start)
            .bind(final_type)
            .bind(final_amt)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // If P&L ledger was not in balance_sheet_balances (e.g. started at 0) but net_pnl != 0,
        // create its opening balance
        let pnl_in_bs = preview
            .balance_sheet_balances
            .iter()
            .any(|b| b.ledger_id == pnl_ledger_id);
        if !pnl_in_bs && net_pnl != Decimal::ZERO {
            let (bal_type, bal_amt) = if net_pnl > Decimal::ZERO {
                ("Cr".to_string(), net_pnl.to_string())
            } else {
                ("Dr".to_string(), net_pnl.abs().to_string())
            };
            let op_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO opening_balances (
                    id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                ON CONFLICT(company_id, ledger_id, financial_year_begins) DO UPDATE SET
                    balance_type = excluded.balance_type,
                    amount = excluded.amount,
                    updated_at = CURRENT_TIMESTAMP",
            )
            .bind(op_id)
            .bind(company_id)
            .bind(&pnl_ledger_id)
            .bind(&preview.next_fy_start)
            .bind(bal_type)
            .bind(bal_amt)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // 6. Record financial_year_closings entry
        let closing_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO financial_year_closings (
                id, company_id, financial_year_label, period_start, period_end, next_fy_start,
                closing_voucher_id, net_profit_or_loss, is_locked, closed_at, closed_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP, ?)",
        )
        .bind(&closing_id)
        .bind(company_id)
        .bind(&preview.financial_year_label)
        .bind(&preview.period_start)
        .bind(&preview.period_end)
        .bind(&preview.next_fy_start)
        .bind(&closing_vch_id)
        .bind(&preview.net_profit_or_loss)
        .bind(actor)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // 7. Insert MCA Audit Log record with SHA-256 hash chaining
        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev = prev_hash.unwrap_or_else(|| "GENESIS".to_string());
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let payload = serde_json::json!({
            "financial_year_label": preview.financial_year_label,
            "period_start": preview.period_start,
            "period_end": preview.period_end,
            "next_fy_start": preview.next_fy_start,
            "closing_voucher_id": closing_vch_id,
            "net_profit_or_loss": preview.net_profit_or_loss,
            "is_locked": 1,
            "closed_by": actor,
        })
        .to_string();

        let entry_hash = crate::audit::certification::AuditCertificationService::compute_entry_hash(
            &prev,
            &audit_id,
            company_id,
            "financial_year_closings",
            &closing_id,
            "CREATE",
            &timestamp,
            Some(&payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, 'financial_year_closings', ?, ?, 'CREATE', ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(&closing_id)
        .bind(&preview.period_start)
        .bind(&payload)
        .bind(actor)
        .bind(&timestamp)
        .bind(&prev)
        .bind(&entry_hash)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(FinancialYearClosingRecord {
            id: closing_id,
            company_id: company_id.to_string(),
            financial_year_label: preview.financial_year_label,
            period_start: preview.period_start,
            period_end: preview.period_end,
            next_fy_start: preview.next_fy_start,
            closing_voucher_id: closing_vch_id,
            net_profit_or_loss: preview.net_profit_or_loss,
            is_locked: true,
            closed_at: timestamp,
            closed_by: actor.to_string(),
        })
    }
}
