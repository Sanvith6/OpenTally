use crate::billwise::{AgingBucketSummary, AgingReport, PartyAgingRow, PendingBill};
use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(Debug, sqlx::FromRow)]
struct RawBillRow {
    party_ledger_id: String,
    party_name: String,
    bill_name: String,
    bill_date: String,
    due_date: Option<String>,
    dr_cr: String,
    amount: String,
}

#[derive(Debug, sqlx::FromRow)]
struct RawSettlementRow {
    party_ledger_id: String,
    bill_name_lower: String,
    amount: String,
    dr_cr: String,
}

pub struct BillWiseReportService;

impl BillWiseReportService {
    /// Internal helper that loads active pending bills as of a given date,
    /// calculating exact settled amounts and remaining balances using rust_decimal.
    async fn fetch_pending_bills_tx(
        pool: &Pool<Sqlite>,
        company_id: &str,
        party_ledger_id_filter: Option<&str>,
        dr_cr_filter: Option<&str>,
        as_of_date: &str,
    ) -> Result<Vec<PendingBill>, OpenTallyError> {
        let _ = NaiveDate::parse_from_str(as_of_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid as_of_date '{}': must be in YYYY-MM-DD format",
                as_of_date
            ))
        })?;
        let as_of_date_str = as_of_date.to_string();
        let company_id_str = company_id.to_string();
        let party_filter_opt = party_ledger_id_filter.map(ToString::to_string);
        let dr_cr_filter_opt = dr_cr_filter.map(ToString::to_string);

        execute_read_only_query(pool, move |conn| {
            let as_of_date_str = as_of_date_str.clone();
            let company_id_str = company_id_str.clone();
            let party_filter_opt = party_filter_opt.clone();
            let dr_cr_filter_opt = dr_cr_filter_opt.clone();

            Box::pin(async move {
                // Defensive check: return empty if bill_allocations table does not exist
                let table_exists: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bill_allocations'",
                )
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if table_exists.is_none() {
                    return Ok(Vec::new());
                }

                // 1. Query all active NewRef and Advance bills as of date
                let mut sql_orig = String::from(
                    "SELECT ba.party_ledger_id, l.name as party_name, ba.bill_name,
                            ba.bill_date, ba.due_date, ba.dr_cr, ba.amount
                     FROM bill_allocations ba
                     JOIN ledgers l ON l.company_id = ba.company_id AND l.id = ba.party_ledger_id
                     WHERE ba.company_id = ?
                       AND ba.allocation_type IN ('NewRef', 'Advance')
                       AND ba.is_cancelled = 0
                       AND ba.bill_date <= ?",
                );

                if party_filter_opt.is_some() {
                    sql_orig.push_str(" AND ba.party_ledger_id = ?");
                }
                if dr_cr_filter_opt.is_some() {
                    sql_orig.push_str(" AND ba.dr_cr = ?");
                }
                sql_orig.push_str(" ORDER BY ba.bill_date ASC, ba.created_at ASC");

                let mut q_orig = sqlx::query_as::<_, RawBillRow>(&sql_orig)
                    .bind(&company_id_str)
                    .bind(&as_of_date_str);

                if let Some(ref pid) = party_filter_opt {
                    q_orig = q_orig.bind(pid);
                }
                if let Some(ref drcr) = dr_cr_filter_opt {
                    q_orig = q_orig.bind(drcr);
                }

                let orig_rows = q_orig
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                // 2. Query active AgstRef settlements up to as_of_date
                let mut sql_settle = String::from(
                    "SELECT party_ledger_id, lower(bill_name) as bill_name_lower, amount, dr_cr
                     FROM bill_allocations
                     WHERE company_id = ?
                       AND allocation_type = 'AgstRef'
                       AND is_cancelled = 0
                       AND bill_date <= ?",
                );
                if party_filter_opt.is_some() {
                    sql_settle.push_str(" AND party_ledger_id = ?");
                }

                let mut q_settle = sqlx::query_as::<_, RawSettlementRow>(&sql_settle)
                    .bind(&company_id_str)
                    .bind(&as_of_date_str);

                if let Some(ref pid) = party_filter_opt {
                    q_settle = q_settle.bind(pid);
                }

                let settle_rows = q_settle
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                // Group settlements by (party_ledger_id, lower(bill_name), dr_cr)
                let mut settled_map: HashMap<(String, String, String), Decimal> = HashMap::new();
                for s in settle_rows {
                    let amt = Decimal::from_str_exact(&s.amount).unwrap_or(Decimal::ZERO);
                    *settled_map
                        .entry((s.party_ledger_id, s.bill_name_lower, s.dr_cr))
                        .or_insert(Decimal::ZERO) += amt;
                }

                let parsed_as_of = NaiveDate::parse_from_str(&as_of_date_str, "%Y-%m-%d").ok();

                let mut pending_bills = Vec::new();
                for row in orig_rows {
                    let orig_amt = Decimal::from_str_exact(&row.amount).unwrap_or(Decimal::ZERO);
                    let opposing_dr_cr = if row.dr_cr == "Dr" { "Cr" } else { "Dr" };
                    let key = (
                        row.party_ledger_id.clone(),
                        row.bill_name.to_lowercase(),
                        opposing_dr_cr.to_string(),
                    );
                    let settled_amt = settled_map.get(&key).copied().unwrap_or(Decimal::ZERO);
                    let pending_amt = orig_amt - settled_amt;

                    if pending_amt > Decimal::ZERO {
                        let overdue_days = if let Some(as_of_d) = parsed_as_of {
                            let ref_date_str = row.due_date.as_deref().unwrap_or(&row.bill_date);
                            if let Ok(ref_date) = NaiveDate::parse_from_str(ref_date_str, "%Y-%m-%d") {
                                if as_of_d > ref_date {
                                    (as_of_d - ref_date).num_days()
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        } else {
                            0
                        };

                        pending_bills.push(PendingBill {
                            party_ledger_id: row.party_ledger_id,
                            party_name: row.party_name,
                            bill_name: row.bill_name,
                            bill_date: row.bill_date,
                            due_date: row.due_date,
                            dr_cr: row.dr_cr,
                            original_amount: format!("{:.2}", orig_amt),
                            settled_amount: format!("{:.2}", settled_amt),
                            pending_amount: format!("{:.2}", pending_amt),
                            overdue_days,
                        });
                    }
                }

                Ok(pending_bills)
            })
        })
        .await
    }

    /// Retrieves all outstanding bills receivable (Sundry Debtors Dr balances) as of date.
    pub async fn get_bills_receivable(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<Vec<PendingBill>, OpenTallyError> {
        Self::fetch_pending_bills_tx(pool, company_id, None, Some("Dr"), as_of_date).await
    }

    /// Retrieves all outstanding bills payable (Sundry Creditors Cr balances) as of date.
    pub async fn get_bills_payable(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<Vec<PendingBill>, OpenTallyError> {
        Self::fetch_pending_bills_tx(pool, company_id, None, Some("Cr"), as_of_date).await
    }

    /// Retrieves all pending bills for a specific party ledger as of date.
    pub async fn get_party_pending_bills(
        pool: &Pool<Sqlite>,
        company_id: &str,
        party_ledger_id: &str,
        as_of_date: &str,
    ) -> Result<Vec<PendingBill>, OpenTallyError> {
        Self::fetch_pending_bills_tx(pool, company_id, Some(party_ledger_id), None, as_of_date)
            .await
    }

    /// Generates an Aging Analysis report bucketed into:
    /// - Not Due
    /// - 0-30 Days
    /// - 31-60 Days
    /// - 61-90 Days
    /// - 90+ Days
    ///
    /// Subtotals by party with grand totals.
    pub async fn get_aging_analysis(
        pool: &Pool<Sqlite>,
        company_id: &str,
        report_type: &str,
        as_of_date: &str,
    ) -> Result<AgingReport, OpenTallyError> {
        let dr_cr_filter = match report_type.to_lowercase().as_str() {
            "receivable" | "receivables" | "debtor" | "debtors" => "Dr",
            "payable" | "payables" | "creditor" | "creditors" => "Cr",
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid aging report type '{}'; expected 'receivable' or 'payable'",
                    other
                )));
            }
        };

        let bills =
            Self::fetch_pending_bills_tx(pool, company_id, None, Some(dr_cr_filter), as_of_date)
                .await?;

        let parsed_as_of = NaiveDate::parse_from_str(as_of_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid as_of_date '{}': must be in YYYY-MM-DD format",
                as_of_date
            ))
        })?;

        // Group bills by party_ledger_id preserving first-seen party_name
        let mut party_order: Vec<(String, String)> = Vec::new();
        let mut party_bills_map: HashMap<String, Vec<PendingBill>> = HashMap::new();

        for b in bills {
            if !party_bills_map.contains_key(&b.party_ledger_id) {
                party_order.push((b.party_ledger_id.clone(), b.party_name.clone()));
            }
            party_bills_map
                .entry(b.party_ledger_id.clone())
                .or_default()
                .push(b);
        }

        let mut grand_not_due = Decimal::ZERO;
        let mut grand_0_30 = Decimal::ZERO;
        let mut grand_31_60 = Decimal::ZERO;
        let mut grand_61_90 = Decimal::ZERO;
        let mut grand_90_plus = Decimal::ZERO;
        let mut grand_total = Decimal::ZERO;

        let mut party_rows = Vec::new();

        for (pid, pname) in party_order {
            let p_bills = party_bills_map.remove(&pid).unwrap_or_default();

            let mut p_not_due = Decimal::ZERO;
            let mut p_0_30 = Decimal::ZERO;
            let mut p_31_60 = Decimal::ZERO;
            let mut p_61_90 = Decimal::ZERO;
            let mut p_90_plus = Decimal::ZERO;
            let mut p_total = Decimal::ZERO;

            for bill in &p_bills {
                let amt = Decimal::from_str_exact(&bill.pending_amount).unwrap_or(Decimal::ZERO);
                p_total += amt;

                let is_not_due = if let Some(ref due_str) = bill.due_date {
                    if let Ok(due_d) = NaiveDate::parse_from_str(due_str, "%Y-%m-%d") {
                        parsed_as_of <= due_d
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_not_due || bill.overdue_days == 0 {
                    p_not_due += amt;
                } else if bill.overdue_days <= 30 {
                    p_0_30 += amt;
                } else if bill.overdue_days <= 60 {
                    p_31_60 += amt;
                } else if bill.overdue_days <= 90 {
                    p_61_90 += amt;
                } else {
                    p_90_plus += amt;
                }
            }

            grand_not_due += p_not_due;
            grand_0_30 += p_0_30;
            grand_31_60 += p_31_60;
            grand_61_90 += p_61_90;
            grand_90_plus += p_90_plus;
            grand_total += p_total;

            party_rows.push(PartyAgingRow {
                party_ledger_id: pid,
                party_name: pname,
                buckets: AgingBucketSummary {
                    not_due: format!("{:.2}", p_not_due),
                    days_0_30: format!("{:.2}", p_0_30),
                    days_31_60: format!("{:.2}", p_31_60),
                    days_61_90: format!("{:.2}", p_61_90),
                    days_90_plus: format!("{:.2}", p_90_plus),
                    total: format!("{:.2}", p_total),
                },
                bills: p_bills,
            });
        }

        Ok(AgingReport {
            as_of_date: as_of_date.to_string(),
            report_type: report_type.to_string(),
            parties: party_rows,
            grand_total: AgingBucketSummary {
                not_due: format!("{:.2}", grand_not_due),
                days_0_30: format!("{:.2}", grand_0_30),
                days_31_60: format!("{:.2}", grand_31_60),
                days_61_90: format!("{:.2}", grand_61_90),
                days_90_plus: format!("{:.2}", grand_90_plus),
                total: format!("{:.2}", grand_total),
            },
        })
    }
}
