use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Budget {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub from_date: String,
    pub to_date: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BudgetLine {
    pub id: String,
    pub company_id: String,
    pub budget_id: String,
    pub target_type: String, // 'Group', 'Ledger', 'CostCenter'
    pub target_id: String,
    pub budget_type: String, // 'OnNetTransactions', 'OnClosingBalance'
    pub amount: String,
    pub dr_cr: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetLineRequest {
    pub target_type: String,
    pub target_id: String,
    pub budget_type: String,
    pub amount: Decimal,
    pub dr_cr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRequest {
    pub name: String,
    pub from_date: String,
    pub to_date: String,
    pub lines: Vec<BudgetLineRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetDetails {
    pub budget: Budget,
    pub lines: Vec<BudgetLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetVarianceRow {
    pub line_id: String,
    pub target_type: String,
    pub target_id: String,
    pub target_name: String,
    pub budget_type: String,
    pub dr_cr: String,
    pub budget_amount: String,
    pub actual_amount: String,
    pub variance_amount: String,
    pub utilization_pct: String,
    pub is_over_budget: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetVarianceReport {
    pub budget_id: String,
    pub budget_name: String,
    pub company_id: String,
    pub as_of_date: String,
    pub from_date: String,
    pub to_date: String,
    pub rows: Vec<BudgetVarianceRow>,
    pub total_budget: String,
    pub total_actual: String,
    pub total_variance: String,
}

pub struct BudgetService;

impl BudgetService {
    pub async fn list_budgets(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<Budget>, OpenTallyError> {
        let cid = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let budgets = sqlx::query_as::<_, Budget>(
                    "SELECT id, company_id, name, from_date, to_date, created_at, updated_at
                     FROM budgets WHERE company_id = ? ORDER BY name ASC",
                )
                .bind(&cid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
                Ok(budgets)
            })
        })
        .await
    }

    pub async fn get_budget(
        pool: &Pool<Sqlite>,
        company_id: &str,
        budget_id: &str,
    ) -> Result<BudgetDetails, OpenTallyError> {
        let cid = company_id.to_string();
        let bid = budget_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let budget = sqlx::query_as::<_, Budget>(
                    "SELECT id, company_id, name, from_date, to_date, created_at, updated_at
                     FROM budgets WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&bid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| OpenTallyError::NotFound(format!("Budget '{}' not found", bid)))?;

                let lines = sqlx::query_as::<_, BudgetLine>(
                    "SELECT id, company_id, budget_id, target_type, target_id, budget_type, amount, dr_cr, created_at
                     FROM budget_lines WHERE company_id = ? AND budget_id = ? ORDER BY created_at ASC",
                )
                .bind(&cid)
                .bind(&bid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                Ok(BudgetDetails { budget, lines })
            })
        })
        .await
    }

    pub async fn create_budget(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: BudgetRequest,
    ) -> Result<BudgetDetails, OpenTallyError> {
        if req.name.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "Budget name cannot be empty".to_string(),
            ));
        }
        if req.to_date < req.from_date {
            return Err(OpenTallyError::Validation(
                "Budget to_date cannot be earlier than from_date".to_string(),
            ));
        }

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let budget_id = Uuid::new_v4().to_string();
        let name_trimmed = req.name.trim();

        // 1. Insert budget header
        sqlx::query(
            "INSERT INTO budgets (id, company_id, name, from_date, to_date)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&budget_id)
        .bind(company_id)
        .bind(name_trimmed)
        .bind(&req.from_date)
        .bind(&req.to_date)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // 2. Insert budget lines
        let mut created_lines = Vec::new();
        for l in req.lines {
            if l.amount < Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Budget amount must be non-negative".to_string(),
                ));
            }
            if l.dr_cr != "Dr" && l.dr_cr != "Cr" {
                return Err(OpenTallyError::Validation(
                    "dr_cr must be 'Dr' or 'Cr'".to_string(),
                ));
            }

            let line_id = Uuid::new_v4().to_string();
            let amount_str = l.amount.round_dp(2).normalize().to_string();

            sqlx::query(
                "INSERT INTO budget_lines (id, company_id, budget_id, target_type, target_id, budget_type, amount, dr_cr)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(company_id)
            .bind(&budget_id)
            .bind(&l.target_type)
            .bind(&l.target_id)
            .bind(&l.budget_type)
            .bind(&amount_str)
            .bind(&l.dr_cr)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            created_lines.push(BudgetLine {
                id: line_id,
                company_id: company_id.to_string(),
                budget_id: budget_id.clone(),
                target_type: l.target_type,
                target_id: l.target_id,
                budget_type: l.budget_type,
                amount: amount_str,
                dr_cr: l.dr_cr,
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        // 3. MCA Audit Log
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or_else(|_| "2026-04-01".to_string());

        let audit_payload = serde_json::json!({
            "action": "CREATE",
            "budget_id": budget_id,
            "budget_name": name_trimmed,
            "line_count": created_lines.len(),
        })
        .to_string();

        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev = prev_hash.unwrap_or_else(|| "GENESIS".to_string());
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let entry_hash = AuditCertificationService::compute_entry_hash(
            &prev,
            &audit_id,
            company_id,
            "budget",
            &budget_id,
            "CREATE",
            &timestamp,
            Some(&audit_payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, 'budget', ?, ?, 'CREATE', ?, 'system', ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(&budget_id)
        .bind(&fy_begins)
        .bind(&audit_payload)
        .bind(&timestamp)
        .bind(&prev)
        .bind(&entry_hash)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        let budget = Budget {
            id: budget_id,
            company_id: company_id.to_string(),
            name: name_trimmed.to_string(),
            from_date: req.from_date,
            to_date: req.to_date,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        Ok(BudgetDetails {
            budget,
            lines: created_lines,
        })
    }

    pub async fn delete_budget(
        pool: &Pool<Sqlite>,
        company_id: &str,
        budget_id: &str,
    ) -> Result<(), OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM budgets WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(budget_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

        if exists.is_none() {
            return Err(OpenTallyError::NotFound(format!(
                "Budget '{}' not found",
                budget_id
            )));
        }

        sqlx::query("DELETE FROM budget_lines WHERE company_id = ? AND budget_id = ?")
            .bind(company_id)
            .bind(budget_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::query("DELETE FROM budgets WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(budget_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or_else(|_| "2026-04-01".to_string());

        let audit_payload = serde_json::json!({
            "action": "DELETE",
            "budget_id": budget_id,
        })
        .to_string();

        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev = prev_hash.unwrap_or_else(|| "GENESIS".to_string());
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();
        let entry_hash = AuditCertificationService::compute_entry_hash(
            &prev,
            &audit_id,
            company_id,
            "budget",
            budget_id,
            "DELETE",
            &timestamp,
            Some(&audit_payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, 'budget', ?, ?, 'DELETE', ?, 'system', ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(budget_id)
        .bind(&fy_begins)
        .bind(&audit_payload)
        .bind(&timestamp)
        .bind(&prev)
        .bind(&entry_hash)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    pub async fn get_budget_variance_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        budget_id: &str,
        as_of_date: &str,
    ) -> Result<BudgetVarianceReport, OpenTallyError> {
        let cid = company_id.to_string();
        let bid = budget_id.to_string();
        let as_of = as_of_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let budget = sqlx::query_as::<_, Budget>(
                    "SELECT id, company_id, name, from_date, to_date, created_at, updated_at
                     FROM budgets WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&bid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| OpenTallyError::NotFound(format!("Budget '{}' not found", bid)))?;

                let lines = sqlx::query_as::<_, BudgetLine>(
                    "SELECT id, company_id, budget_id, target_type, target_id, budget_type, amount, dr_cr, created_at
                     FROM budget_lines WHERE company_id = ? AND budget_id = ?",
                )
                .bind(&cid)
                .bind(&bid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let effective_end_date = if as_of < budget.to_date {
                    &as_of
                } else {
                    &budget.to_date
                };

                let mut rows = Vec::new();
                let mut total_budget = Decimal::ZERO;
                let mut total_actual = Decimal::ZERO;

                for l in lines {
                    let b_amt = Decimal::from_str(&l.amount).unwrap_or(Decimal::ZERO);
                    let (target_name, actual_amt) = match l.target_type.as_str() {
                        "Group" => {
                            let name: String = sqlx::query_scalar(
                                "SELECT name FROM account_groups WHERE company_id = ? AND id = ?",
                            )
                            .bind(&cid)
                            .bind(&l.target_id)
                            .fetch_one(&mut *conn)
                            .await
                            .unwrap_or_else(|_| "Unknown Group".to_string());

                            let act = Self::calc_group_actual(
                                conn,
                                &cid,
                                &l.target_id,
                                &l.budget_type,
                                &l.dr_cr,
                                &budget.from_date,
                                effective_end_date,
                            )
                            .await?;
                            (name, act)
                        }
                        "Ledger" => {
                            let name: String = sqlx::query_scalar(
                                "SELECT name FROM ledgers WHERE company_id = ? AND id = ?",
                            )
                            .bind(&cid)
                            .bind(&l.target_id)
                            .fetch_one(&mut *conn)
                            .await
                            .unwrap_or_else(|_| "Unknown Ledger".to_string());

                            let act = Self::calc_ledger_actual(
                                conn,
                                &cid,
                                &l.target_id,
                                &l.budget_type,
                                &l.dr_cr,
                                &budget.from_date,
                                effective_end_date,
                            )
                            .await?;
                            (name, act)
                        }
                        "CostCenter" => {
                            let name: String = sqlx::query_scalar(
                                "SELECT name FROM cost_centres WHERE company_id = ? AND id = ?",
                            )
                            .bind(&cid)
                            .bind(&l.target_id)
                            .fetch_one(&mut *conn)
                            .await
                            .unwrap_or_else(|_| "Unknown Cost Centre".to_string());

                            let act = Self::calc_cost_centre_actual(
                                conn,
                                &cid,
                                &l.target_id,
                                &budget.from_date,
                                effective_end_date,
                            )
                            .await?;
                            (name, act)
                        }
                        _ => ("Unknown".to_string(), Decimal::ZERO),
                    };

                    let variance = b_amt - actual_amt;
                    let utilization_pct = if b_amt > Decimal::ZERO {
                        format!("{:.2}%", ((actual_amt / b_amt) * Decimal::new(100, 0)).round_dp(2))
                    } else {
                        "N/A".to_string()
                    };
                    let is_over_budget = actual_amt > b_amt;

                    total_budget += b_amt;
                    total_actual += actual_amt;

                    rows.push(BudgetVarianceRow {
                        line_id: l.id,
                        target_type: l.target_type,
                        target_id: l.target_id,
                        target_name,
                        budget_type: l.budget_type,
                        dr_cr: l.dr_cr,
                        budget_amount: b_amt.round_dp(2).to_string(),
                        actual_amount: actual_amt.round_dp(2).to_string(),
                        variance_amount: variance.round_dp(2).to_string(),
                        utilization_pct,
                        is_over_budget,
                    });
                }

                let total_variance = total_budget - total_actual;

                Ok(BudgetVarianceReport {
                    budget_id: bid,
                    budget_name: budget.name,
                    company_id: cid,
                    as_of_date: as_of,
                    from_date: budget.from_date,
                    to_date: budget.to_date,
                    rows,
                    total_budget: total_budget.round_dp(2).to_string(),
                    total_actual: total_actual.round_dp(2).to_string(),
                    total_variance: total_variance.round_dp(2).to_string(),
                })
            })
        })
        .await
    }

    async fn calc_group_actual(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        group_id: &str,
        budget_type: &str,
        target_dr_cr: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let ledger_ids: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE grp_tree AS (
                SELECT id FROM account_groups WHERE company_id = ? AND id = ?
                UNION ALL
                SELECT ag.id FROM account_groups ag
                JOIN grp_tree gt ON ag.parent_id = gt.id
                WHERE ag.company_id = ?
             )
             SELECT l.id FROM ledgers l
             JOIN grp_tree gt ON l.group_id = gt.id
             WHERE l.company_id = ?",
        )
        .bind(company_id)
        .bind(group_id)
        .bind(company_id)
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        let mut sum = Decimal::ZERO;
        for lid in &ledger_ids {
            let act = Self::calc_ledger_actual(
                conn,
                company_id,
                lid,
                budget_type,
                target_dr_cr,
                from_date,
                to_date,
            )
            .await?;
            sum += act;
        }

        Ok(sum)
    }

    async fn calc_ledger_actual(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        ledger_id: &str,
        budget_type: &str,
        target_dr_cr: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let mut balance = Decimal::ZERO;

        if budget_type == "OnClosingBalance" {
            // Opening balance
            let openings: Vec<(String, String)> = sqlx::query_as(
                "SELECT amount, balance_type FROM opening_balances WHERE company_id = ? AND ledger_id = ?",
            )
            .bind(company_id)
            .bind(ledger_id)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for (amt_str, balance_type) in openings {
                if let Ok(amt) = Decimal::from_str(&amt_str) {
                    if balance_type == "Dr" {
                        balance += amt;
                    } else {
                        balance -= amt;
                    }
                }
            }
        }

        let date_clause = if budget_type == "OnClosingBalance" {
            "v.date <= ?"
        } else {
            "v.date >= ? AND v.date <= ?"
        };

        let query_str = format!(
            "SELECT vl.amount, vl.entry_type FROM voucher_lines vl
             JOIN vouchers v ON v.id = vl.voucher_id AND v.company_id = vl.company_id
             WHERE vl.company_id = ? AND vl.ledger_id = ? AND v.status = 'posted' AND {}",
            date_clause
        );

        let movements: Vec<(String, String)> = if budget_type == "OnClosingBalance" {
            sqlx::query_as(&query_str)
                .bind(company_id)
                .bind(ledger_id)
                .bind(to_date)
                .fetch_all(&mut *conn)
                .await
                .unwrap_or_default()
        } else {
            sqlx::query_as(&query_str)
                .bind(company_id)
                .bind(ledger_id)
                .bind(from_date)
                .bind(to_date)
                .fetch_all(&mut *conn)
                .await
                .unwrap_or_default()
        };

        for (amt_str, entry_type) in movements {
            if let Ok(amt) = Decimal::from_str(&amt_str) {
                if entry_type == "Dr" {
                    balance += amt;
                } else {
                    balance -= amt;
                }
            }
        }

        // Return signed or positive amount matching target dr_cr
        let res = if target_dr_cr == "Dr" {
            balance
        } else {
            -balance
        };

        Ok(res.max(Decimal::ZERO))
    }

    async fn calc_cost_centre_actual(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        cost_centre_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let allocations: Vec<String> = sqlx::query_scalar(
            "SELECT ca.amount FROM voucher_cost_allocations ca
             JOIN vouchers v ON v.id = ca.voucher_id AND v.company_id = ca.company_id
             WHERE ca.company_id = ? AND ca.cost_centre_id = ?
             AND v.status = 'posted' AND v.date >= ? AND v.date <= ?",
        )
        .bind(company_id)
        .bind(cost_centre_id)
        .bind(from_date)
        .bind(to_date)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        let mut sum = Decimal::ZERO;
        for a_str in allocations {
            if let Ok(amt) = Decimal::from_str(&a_str) {
                sum += amt;
            }
        }

        Ok(sum)
    }
}
