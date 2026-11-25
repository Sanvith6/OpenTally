use super::models::*;
use crate::errors::OpenTallyError;
use crate::vouchers::model::VoucherLine;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

type RawReconciliationTuple = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
);

type RawStatementMatchTuple = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
);

pub struct BankingService;

impl BankingService {
    /// Validates whether a given ledger belongs to the 'Bank Accounts' or 'Bank OD A/c' group hierarchy.
    pub async fn is_bank_ledger(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<bool, OpenTallyError> {
        let is_bank: Option<i64> = sqlx::query_scalar(
            r#"
            WITH RECURSIVE group_hierarchy(id, parent_id, name) AS (
                SELECT g.id, g.parent_id, g.name
                FROM account_groups g
                WHERE g.company_id = ? AND g.id = (
                    SELECT l.group_id FROM ledgers l WHERE l.company_id = ? AND l.id = ?
                )
                UNION ALL
                SELECT p.id, p.parent_id, p.name
                FROM account_groups p
                JOIN group_hierarchy gh ON p.id = gh.parent_id
                WHERE p.company_id = ?
            )
            SELECT 1 FROM group_hierarchy WHERE name IN ('Bank Accounts', 'Bank OD A/c') COLLATE NOCASE LIMIT 1
            "#,
        )
        .bind(company_id)
        .bind(company_id)
        .bind(ledger_id)
        .bind(company_id)
        .fetch_optional(conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(is_bank.is_some())
    }

    /// Lists all bank ledgers for the active company with their current balances.
    pub async fn list_bank_ledgers(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<BankLedgerItem>, OpenTallyError> {
        let company_id_owned = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let rows: Vec<(String, String, String, String)> = sqlx::query_as(
                    r#"
                    WITH RECURSIVE bank_groups(id) AS (
                        SELECT id FROM account_groups
                        WHERE company_id = ? AND name IN ('Bank Accounts', 'Bank OD A/c') COLLATE NOCASE
                        UNION ALL
                        SELECT g.id FROM account_groups g
                        JOIN bank_groups bg ON g.parent_id = bg.id
                        WHERE g.company_id = ?
                    )
                    SELECT l.id, l.name, l.group_id, g.name as group_name
                    FROM ledgers l
                    JOIN account_groups g ON l.company_id = g.company_id AND l.group_id = g.id
                    WHERE l.company_id = ? AND l.group_id IN (SELECT id FROM bank_groups)
                    ORDER BY l.name ASC
                    "#,
                )
                .bind(&company_id_owned)
                .bind(&company_id_owned)
                .bind(&company_id_owned)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut items = Vec::with_capacity(rows.len());
                for (id, name, group_id, group_name) in rows {
                    let op_balance: Option<(String, String)> = sqlx::query_as(
                        "SELECT amount, balance_type FROM opening_balances WHERE company_id = ? AND ledger_id = ?",
                    )
                    .bind(&company_id_owned)
                    .bind(&id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    let mut net = Decimal::ZERO;
                    if let Some((amt_str, dr_cr)) = op_balance {
                        if let Ok(amt) = Decimal::from_str(&amt_str) {
                            if dr_cr == "Dr" {
                                net += amt;
                            } else {
                                net -= amt;
                            }
                        }
                    }

                    let movements: Vec<(String, String)> = sqlx::query_as(
                        r#"
                        SELECT vl.amount, vl.entry_type
                        FROM voucher_lines vl
                        JOIN vouchers v ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                        WHERE vl.company_id = ? AND vl.ledger_id = ? AND v.status = 'posted'
                        "#,
                    )
                    .bind(&company_id_owned)
                    .bind(&id)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    for (amt_str, dr_cr) in movements {
                        if let Ok(amt) = Decimal::from_str(&amt_str) {
                            if dr_cr == "Dr" {
                                net += amt;
                            } else {
                                net -= amt;
                            }
                        }
                    }

                    let (bal_str, bal_drcr) = if net >= Decimal::ZERO {
                        (net.to_string(), "Dr".to_string())
                    } else {
                        ((-net).to_string(), "Cr".to_string())
                    };

                    items.push(BankLedgerItem {
                        id,
                        name,
                        group_id,
                        group_name,
                        current_balance: bal_str,
                        current_balance_dr_cr: bal_drcr,
                    });
                }

                Ok(items)
            })
        })
        .await
    }

    /// Automatically generates or attaches bank_allocations rows for any bank ledger line in a posted voucher.
    pub async fn auto_create_or_attach_bank_allocations(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_date: &str,
        lines: &[VoucherLine],
        custom_allocations: Option<&[BankAllocationInput]>,
    ) -> Result<(), OpenTallyError> {
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bank_allocations'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        let allowed_types: HashSet<&'static str> = [
            "Cheque", "NEFT", "RTGS", "IMPS", "UPI", "DD", "Card", "ECS", "Other",
        ]
        .into_iter()
        .collect();

        for line in lines {
            if Self::is_bank_ledger(conn, company_id, &line.ledger_id).await? {
                // Check if already exists for this line
                let exists: Option<i64> = sqlx::query_scalar(
                    "SELECT 1 FROM bank_allocations WHERE company_id = ? AND voucher_id = ? AND voucher_line_id = ?",
                )
                .bind(company_id)
                .bind(voucher_id)
                .bind(&line.id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if exists.is_some() {
                    continue;
                }

                // Match with custom allocation if provided
                let custom = custom_allocations.and_then(|allocs| {
                    allocs.iter().find(|a| {
                        a.voucher_line_id.as_deref() == Some(&line.id)
                            || a.bank_ledger_id.as_deref() == Some(&line.ledger_id)
                    })
                });

                let tx_type = custom
                    .and_then(|c| c.transaction_type.as_deref())
                    .unwrap_or("Other");
                let validated_tx_type = if allowed_types.contains(tx_type) {
                    tx_type
                } else {
                    "Other"
                };

                let inst_number = custom
                    .and_then(|c| c.instrument_number.as_deref().map(str::trim))
                    .filter(|s| !s.is_empty());
                let inst_date = custom
                    .and_then(|c| c.instrument_date.as_deref().map(str::trim))
                    .filter(|s| !s.is_empty());
                let fav_name = custom
                    .and_then(|c| c.favouring_name.as_deref().map(str::trim))
                    .filter(|s| !s.is_empty());
                let bank_date = custom
                    .and_then(|c| c.bank_date.as_deref().map(str::trim))
                    .filter(|s| !s.is_empty());

                if let Some(idate) = inst_date {
                    NaiveDate::parse_from_str(idate, "%Y-%m-%d").map_err(|_| {
                        OpenTallyError::Validation(format!(
                            "Invalid instrument date '{}'; expected YYYY-MM-DD",
                            idate
                        ))
                    })?;
                }

                if let Some(bdate) = bank_date {
                    let b = NaiveDate::parse_from_str(bdate, "%Y-%m-%d").map_err(|_| {
                        OpenTallyError::Validation(format!(
                            "Invalid bank clearance date '{}'; expected YYYY-MM-DD",
                            bdate
                        ))
                    })?;
                    let v = NaiveDate::parse_from_str(voucher_date, "%Y-%m-%d").map_err(|_| {
                        OpenTallyError::Validation(format!(
                            "Invalid voucher date '{}'; expected YYYY-MM-DD",
                            voucher_date
                        ))
                    })?;
                    if b < v {
                        return Err(OpenTallyError::Validation(format!(
                            "Bank clearance date '{}' cannot precede voucher date '{}'",
                            bdate, voucher_date
                        )));
                    }
                }

                let alloc_id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO bank_allocations (
                        id, company_id, voucher_id, voucher_line_id, bank_ledger_id,
                        transaction_type, instrument_number, instrument_date,
                        favouring_name, bank_date, amount, dr_cr, is_cancelled
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
                    "#,
                )
                .bind(&alloc_id)
                .bind(company_id)
                .bind(voucher_id)
                .bind(&line.id)
                .bind(&line.ledger_id)
                .bind(validated_tx_type)
                .bind(inst_number)
                .bind(inst_date)
                .bind(fav_name)
                .bind(bank_date)
                .bind(&line.amount)
                .bind(&line.entry_type)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }
        }

        Ok(())
    }

    /// Atomically marks all bank_allocations for a cancelled or reversed voucher as cancelled.
    pub async fn cancel_voucher_bank_allocations(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_number: &str,
        actor_id: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bank_allocations'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        let rows_affected = sqlx::query(
            "UPDATE bank_allocations SET is_cancelled = 1, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows_affected > 0 {
            let fy_begins: String =
                sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                    .bind(company_id)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

            let audit_id = Uuid::new_v4().to_string();
            let old_values = serde_json::json!({
                "voucher_number": voucher_number,
                "is_cancelled": 0
            })
            .to_string();
            let new_values = serde_json::json!({
                "voucher_number": voucher_number,
                "is_cancelled": 1,
                "cancelled_allocations": rows_affected
            })
            .to_string();

            sqlx::query(
                r#"
                INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, financial_year_begins,
                    action, old_values, new_values, performed_by
                ) VALUES (?, ?, 'bank_allocation', ?, ?, 'CANCEL', ?, ?, ?)
                "#,
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(&fy_begins)
            .bind(&old_values)
            .bind(&new_values)
            .bind(actor_id.unwrap_or("system"))
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    /// Reconciles one or more bank allocation entries by setting or clearing their bank clearance dates.
    pub async fn reconcile_bank_entries(
        pool: &Pool<Sqlite>,
        company_id: &str,
        updates: Vec<ReconcileEntryInput>,
        actor_id: Option<&str>,
    ) -> Result<ReconcileResult, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(OpenTallyError::NotFound(format!(
                        "Company '{}' not found",
                        company_id
                    )));
                }
            };

        let mut updated_ids = Vec::with_capacity(updates.len());

        for upd in updates {
            let row: Option<(Option<String>, String)> = sqlx::query_as(
                r#"
                SELECT b.bank_date, v.date as voucher_date
                FROM bank_allocations b
                JOIN vouchers v ON b.company_id = v.company_id AND b.voucher_id = v.id
                WHERE b.company_id = ? AND b.id = ? AND b.is_cancelled = 0
                "#,
            )
            .bind(company_id)
            .bind(&upd.bank_allocation_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (old_bank_date, voucher_date) = match row {
                Some(r) => r,
                None => continue,
            };

            let clean_bdate = upd
                .bank_date
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());

            if let Some(bdate) = clean_bdate {
                let parsed_bdate = NaiveDate::parse_from_str(bdate, "%Y-%m-%d").map_err(|_| {
                    OpenTallyError::Validation(format!(
                        "Invalid bank clearance date '{}'; expected YYYY-MM-DD",
                        bdate
                    ))
                })?;
                let parsed_vdate =
                    NaiveDate::parse_from_str(&voucher_date, "%Y-%m-%d").map_err(|_| {
                        OpenTallyError::Validation(format!(
                            "Invalid voucher date '{}'; expected YYYY-MM-DD",
                            voucher_date
                        ))
                    })?;

                if parsed_bdate < parsed_vdate {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err(OpenTallyError::Validation(format!(
                        "Bank clearance date '{}' cannot precede voucher date '{}'",
                        bdate, voucher_date
                    )));
                }

                sqlx::query(
                    "UPDATE bank_allocations SET bank_date = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
                )
                .bind(bdate)
                .bind(company_id)
                .bind(&upd.bank_allocation_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let audit_id = Uuid::new_v4().to_string();
                let old_val = serde_json::json!({ "bank_date": old_bank_date }).to_string();
                let new_val = serde_json::json!({ "bank_date": bdate }).to_string();

                sqlx::query(
                    r#"
                    INSERT INTO audit_log (
                        id, company_id, entity_type, entity_id, financial_year_begins,
                        action, old_values, new_values, performed_by
                    ) VALUES (?, ?, 'bank_reconciliation', ?, ?, 'UPDATE', ?, ?, ?)
                    "#,
                )
                .bind(&audit_id)
                .bind(company_id)
                .bind(&upd.bank_allocation_id)
                .bind(&fy_begins)
                .bind(&old_val)
                .bind(&new_val)
                .bind(actor_id.unwrap_or("system"))
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            } else {
                // Clear bank date
                sqlx::query(
                    "UPDATE bank_allocations SET bank_date = NULL, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
                )
                .bind(company_id)
                .bind(&upd.bank_allocation_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let audit_id = Uuid::new_v4().to_string();
                let old_val = serde_json::json!({ "bank_date": old_bank_date }).to_string();
                let new_val = serde_json::json!({ "bank_date": null }).to_string();

                sqlx::query(
                    r#"
                    INSERT INTO audit_log (
                        id, company_id, entity_type, entity_id, financial_year_begins,
                        action, old_values, new_values, performed_by
                    ) VALUES (?, ?, 'bank_reconciliation', ?, ?, 'UPDATE', ?, ?, ?)
                    "#,
                )
                .bind(&audit_id)
                .bind(company_id)
                .bind(&upd.bank_allocation_id)
                .bind(&fy_begins)
                .bind(&old_val)
                .bind(&new_val)
                .bind(actor_id.unwrap_or("system"))
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }

            updated_ids.push(upd.bank_allocation_id);
        }

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        Ok(ReconcileResult {
            success: true,
            updated_count: updated_ids.len(),
            updated_ids,
        })
    }

    /// Generates full Bank Reconciliation Statement (BRS) with exact TallyPrime summary footer.
    pub async fn get_bank_reconciliation(
        pool: &Pool<Sqlite>,
        company_id: &str,
        bank_ledger_id: &str,
        from_date: &str,
        to_date: &str,
        include_reconciled: bool,
    ) -> Result<BankReconciliationStatement, OpenTallyError> {
        let p_from = NaiveDate::parse_from_str(from_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid from_date '{}'; expected YYYY-MM-DD",
                from_date
            ))
        })?;
        let p_to = NaiveDate::parse_from_str(to_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid to_date '{}'; expected YYYY-MM-DD",
                to_date
            ))
        })?;
        if p_to < p_from {
            return Err(OpenTallyError::Validation(format!(
                "to_date '{}' cannot precede from_date '{}'",
                to_date, from_date
            )));
        }

        let company_id_owned = company_id.to_string();
        let bank_ledger_id_owned = bank_ledger_id.to_string();
        let from_date_owned = from_date.to_string();
        let to_date_owned = to_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let ledger_name: String = sqlx::query_scalar(
                    "SELECT name FROM ledgers WHERE company_id = ? AND id = ?",
                )
                .bind(&company_id_owned)
                .bind(&bank_ledger_id_owned)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| OpenTallyError::NotFound(format!("Ledger '{}' not found", bank_ledger_id_owned)))?;

                // 1. Calculate Balance as per Company Books as of to_date
                let op_balance: Option<(String, String)> = sqlx::query_as(
                    "SELECT amount, balance_type FROM opening_balances WHERE company_id = ? AND ledger_id = ?",
                )
                .bind(&company_id_owned)
                .bind(&bank_ledger_id_owned)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut book_net = Decimal::ZERO;
                if let Some((amt_str, dr_cr)) = op_balance {
                    if let Ok(amt) = Decimal::from_str(&amt_str) {
                        if dr_cr == "Dr" {
                            book_net += amt;
                        } else {
                            book_net -= amt;
                        }
                    }
                }

                let movements: Vec<(String, String)> = sqlx::query_as(
                    r#"
                    SELECT vl.amount, vl.entry_type
                    FROM voucher_lines vl
                    JOIN vouchers v ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                    WHERE vl.company_id = ? AND vl.ledger_id = ? AND v.status = 'posted' AND v.date <= ?
                    "#,
                )
                .bind(&company_id_owned)
                .bind(&bank_ledger_id_owned)
                .bind(&to_date_owned)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                for (amt_str, dr_cr) in movements {
                    if let Ok(amt) = Decimal::from_str(&amt_str) {
                        if dr_cr == "Dr" {
                            book_net += amt;
                        } else {
                            book_net -= amt;
                        }
                    }
                }

                let (book_balance_str, book_balance_drcr) = if book_net >= Decimal::ZERO {
                    (book_net.to_string(), "Dr".to_string())
                } else {
                    ((-book_net).to_string(), "Cr".to_string())
                };

                // 2. Fetch all bank allocations for this bank ledger
                let raw_allocs: Vec<RawReconciliationTuple> = sqlx::query_as(
                    r#"
                    SELECT
                        b.id, b.voucher_id, b.voucher_line_id, v.date as voucher_date,
                        v.voucher_number, vt.name as voucher_type,
                        b.transaction_type, b.instrument_number, b.instrument_date,
                        b.favouring_name, b.bank_date, b.amount, b.dr_cr,
                        COALESCE(
                            (SELECT l2.name FROM voucher_lines vl2
                             JOIN ledgers l2 ON vl2.company_id = l2.company_id AND vl2.ledger_id = l2.id
                             WHERE vl2.company_id = b.company_id AND vl2.voucher_id = b.voucher_id AND vl2.id != b.voucher_line_id
                             LIMIT 1),
                            v.narration,
                            'Bank Transaction'
                        ) as particulars
                    FROM bank_allocations b
                    JOIN vouchers v ON b.company_id = v.company_id AND b.voucher_id = v.id
                    JOIN voucher_types vt ON v.company_id = vt.company_id AND v.voucher_type_id = vt.id
                    WHERE b.company_id = ? AND b.bank_ledger_id = ? AND b.is_cancelled = 0 AND v.status = 'posted'
                    ORDER BY v.date ASC, v.voucher_number ASC
                    "#,
                )
                .bind(&company_id_owned)
                .bind(&bank_ledger_id_owned)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut unreconciled_debit = Decimal::ZERO;
                let mut unreconciled_credit = Decimal::ZERO;
                let mut statement_rows = Vec::new();

                for (
                    id, v_id, vl_id, v_date, v_num, v_type, tx_type,
                    inst_num, inst_date, fav_name, b_date, amt_str, dr_cr, particulars
                ) in raw_allocs {
                    let amt = Decimal::from_str(&amt_str).unwrap_or(Decimal::ZERO);
                    // Unreconciled as of to_date if bank_date is null OR bank_date > to_date
                    let is_unreconciled = match &b_date {
                        Some(bd) => bd > &to_date_owned,
                        None => true,
                    };

                    if is_unreconciled {
                        if dr_cr == "Dr" {
                            unreconciled_debit += amt;
                        } else {
                            unreconciled_credit += amt;
                        }
                    }

                    let is_reconciled = !is_unreconciled;

                    // Filtering for display:
                    let should_include = if include_reconciled {
                        is_unreconciled
                            || (v_date >= from_date_owned && v_date <= to_date_owned)
                            || (b_date.as_deref().unwrap_or("") >= from_date_owned.as_str() && b_date.as_deref().unwrap_or("") <= to_date_owned.as_str())
                    } else {
                        is_unreconciled
                    };

                    if should_include {
                        let debit_amt = if dr_cr == "Dr" { amt_str.clone() } else { "0.00".to_string() };
                        let credit_amt = if dr_cr == "Cr" { amt_str.clone() } else { "0.00".to_string() };

                        statement_rows.push(BankReconciliationRow {
                            bank_allocation_id: id,
                            voucher_id: v_id,
                            voucher_line_id: vl_id,
                            voucher_date: v_date,
                            voucher_number: v_num,
                            voucher_type: v_type,
                            particulars,
                            transaction_type: tx_type,
                            instrument_number: inst_num,
                            instrument_date: inst_date,
                            favouring_name: fav_name,
                            bank_date: b_date,
                            debit_amount: debit_amt,
                            credit_amount: credit_amt,
                            is_reconciled,
                        });
                    }
                }

                // Balance as per Bank: Book_Balance - Unreconciled_Dr + Unreconciled_Cr
                let bank_net = book_net - unreconciled_debit + unreconciled_credit;
                let (bank_balance_str, bank_balance_drcr) = if bank_net >= Decimal::ZERO {
                    (bank_net.to_string(), "Dr".to_string())
                } else {
                    ((-bank_net).to_string(), "Cr".to_string())
                };

                let summary = BankReconciliationSummary {
                    book_balance: book_balance_str,
                    book_balance_dr_cr: book_balance_drcr,
                    unreconciled_debit: unreconciled_debit.to_string(),
                    unreconciled_credit: unreconciled_credit.to_string(),
                    bank_balance: bank_balance_str,
                    bank_balance_dr_cr: bank_balance_drcr,
                };

                Ok(BankReconciliationStatement {
                    bank_ledger_id: bank_ledger_id_owned,
                    bank_ledger_name: ledger_name,
                    from_date: from_date_owned,
                    to_date: to_date_owned,
                    summary,
                    rows: statement_rows,
                })
            })
        })
        .await
    }

    /// Deterministically matches imported CSV bank statement rows against unreconciled bank allocations.
    pub async fn preview_bank_statement_auto_match(
        pool: &Pool<Sqlite>,
        company_id: &str,
        bank_ledger_id: &str,
        csv_content: &str,
    ) -> Result<BankStatementMatchPreview, OpenTallyError> {
        let company_id_owned = company_id.to_string();
        let bank_ledger_id_owned = bank_ledger_id.to_string();

        let raw_allocs: Vec<RawStatementMatchTuple> =
            execute_read_only_query(pool, move |conn| {
                Box::pin(async move {
                    sqlx::query_as(
                        r#"
                        SELECT
                            b.id, v.date as voucher_date, v.voucher_number,
                            b.amount, b.dr_cr, b.instrument_number,
                            COALESCE(
                                (SELECT l2.name FROM voucher_lines vl2
                                 JOIN ledgers l2 ON vl2.company_id = l2.company_id AND vl2.ledger_id = l2.id
                                 WHERE vl2.company_id = b.company_id AND vl2.voucher_id = b.voucher_id AND vl2.id != b.voucher_line_id
                                 LIMIT 1),
                                v.narration,
                                'Bank Transaction'
                            ) as particulars
                        FROM bank_allocations b
                        JOIN vouchers v ON b.company_id = v.company_id AND b.voucher_id = v.id
                        WHERE b.company_id = ? AND b.bank_ledger_id = ? AND b.is_cancelled = 0
                          AND b.bank_date IS NULL AND v.status = 'posted'
                        ORDER BY v.date ASC, v.voucher_number ASC
                        "#,
                    )
                    .bind(&company_id_owned)
                    .bind(&bank_ledger_id_owned)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)
                })
            })
            .await?;

        // Parse CSV lines safely
        let mut match_rows = Vec::new();
        let mut matched_allocation_ids = HashSet::new();

        let lines: Vec<&str> = csv_content.lines().collect();
        let mut csv_index = 0;

        for raw_line in lines {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }

            // CWE-1236 safe tokenization
            let parts: Vec<String> = line
                .split(',')
                .map(|p| {
                    let s = p.trim().trim_matches('"');
                    // Strip formula injection prefixes
                    s.trim_start_matches(|c| ['=', '+', '-', '@'].contains(&c))
                        .trim()
                        .to_string()
                })
                .collect();

            if parts.len() < 3 {
                continue;
            }

            // Skip header if line contains 'date' or 'amount'
            let lower_first = parts[0].to_lowercase();
            if lower_first.contains("date") || lower_first.contains("particular") {
                continue;
            }

            csv_index += 1;

            let st_date = parts[0].clone();
            let st_desc = if parts.len() > 1 {
                parts[1].clone()
            } else {
                String::new()
            };
            let st_ref = if parts.len() > 2 {
                parts[2].clone()
            } else {
                String::new()
            };

            // Determine debit and credit from CSV
            let mut st_debit = Decimal::ZERO;
            let mut st_credit = Decimal::ZERO;

            if parts.len() >= 5 {
                st_debit = Decimal::from_str(&parts[3].replace(',', "")).unwrap_or(Decimal::ZERO);
                st_credit = Decimal::from_str(&parts[4].replace(',', "")).unwrap_or(Decimal::ZERO);
            } else if parts.len() == 4 {
                let amt = Decimal::from_str(&parts[3].replace(',', "")).unwrap_or(Decimal::ZERO);
                if amt >= Decimal::ZERO {
                    st_credit = amt; // Deposit
                } else {
                    st_debit = -amt; // Withdrawal
                }
            }

            let (st_amt, expected_book_drcr) = if !st_credit.is_zero() {
                // Bank Credit (Deposit) corresponds to Book Debit
                (st_credit, "Dr")
            } else if !st_debit.is_zero() {
                // Bank Debit (Withdrawal) corresponds to Book Credit
                (st_debit, "Cr")
            } else {
                continue;
            };

            let clean_ref = st_ref.to_lowercase().replace(' ', "");

            // 1. Check Exact Match
            let mut matched_alloc = None;
            for (alloc_id, _v_date, v_num, amt_str, dr_cr, inst_num, parts_str) in &raw_allocs {
                if matched_allocation_ids.contains(alloc_id) {
                    continue;
                }
                if dr_cr == expected_book_drcr {
                    if let Ok(amt) = Decimal::from_str(amt_str) {
                        if amt == st_amt {
                            let inst_clean = inst_num
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .replace(' ', "");
                            let ref_match = (!clean_ref.is_empty()
                                && !inst_clean.is_empty()
                                && (clean_ref == inst_clean
                                    || clean_ref.contains(&inst_clean)
                                    || inst_clean.contains(&clean_ref)))
                                || (!st_desc.is_empty()
                                    && !inst_clean.is_empty()
                                    && st_desc.to_lowercase().contains(&inst_clean));

                            if ref_match {
                                matched_alloc = Some((
                                    alloc_id.clone(),
                                    v_num.clone(),
                                    parts_str.clone(),
                                    "exact",
                                    "HIGH",
                                ));
                                break;
                            }
                        }
                    }
                }
            }

            // 2. Check Suggested Match (within ±3 days)
            if matched_alloc.is_none() {
                if let Ok(parsed_st_date) = NaiveDate::parse_from_str(&st_date, "%Y-%m-%d") {
                    for (alloc_id, v_date, v_num, amt_str, dr_cr, _inst_num, parts_str) in
                        &raw_allocs
                    {
                        if matched_allocation_ids.contains(alloc_id) {
                            continue;
                        }
                        if dr_cr == expected_book_drcr {
                            if let Ok(amt) = Decimal::from_str(amt_str) {
                                if amt == st_amt {
                                    if let Ok(parsed_v_date) =
                                        NaiveDate::parse_from_str(v_date, "%Y-%m-%d")
                                    {
                                        let diff_days = (parsed_st_date - parsed_v_date).num_days();
                                        if (0..=3).contains(&diff_days) {
                                            matched_alloc = Some((
                                                alloc_id.clone(),
                                                v_num.clone(),
                                                parts_str.clone(),
                                                "suggested",
                                                "MEDIUM",
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some((alloc_id, v_num, parts_str, m_type, conf)) = matched_alloc {
                matched_allocation_ids.insert(alloc_id.clone());
                match_rows.push(BankStatementMatchRow {
                    csv_row_index: csv_index,
                    statement_date: st_date.clone(),
                    statement_ref: st_ref,
                    statement_description: st_desc,
                    statement_amount: st_amt.to_string(),
                    statement_dr_cr: expected_book_drcr.to_string(),
                    matched_bank_allocation_id: Some(alloc_id),
                    matched_voucher_number: Some(v_num),
                    matched_particulars: Some(parts_str),
                    suggested_bank_date: Some(st_date),
                    match_type: m_type.to_string(),
                    confidence: conf.to_string(),
                });
            } else {
                match_rows.push(BankStatementMatchRow {
                    csv_row_index: csv_index,
                    statement_date: st_date,
                    statement_ref: st_ref,
                    statement_description: st_desc,
                    statement_amount: st_amt.to_string(),
                    statement_dr_cr: expected_book_drcr.to_string(),
                    matched_bank_allocation_id: None,
                    matched_voucher_number: None,
                    matched_particulars: None,
                    suggested_bank_date: None,
                    match_type: "unmatched".to_string(),
                    confidence: "NONE".to_string(),
                });
            }
        }

        let exact_matches = match_rows
            .iter()
            .filter(|r| r.match_type == "exact")
            .count();
        let suggested_matches = match_rows
            .iter()
            .filter(|r| r.match_type == "suggested")
            .count();
        let unmatched = match_rows
            .iter()
            .filter(|r| r.match_type == "unmatched")
            .count();

        Ok(BankStatementMatchPreview {
            total_statement_rows: match_rows.len(),
            exact_matches,
            suggested_matches,
            unmatched,
            rows: match_rows,
        })
    }
}
