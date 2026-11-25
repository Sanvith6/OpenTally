use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioAnalysisReport {
    pub company_id: String,
    pub from_date: String,
    pub to_date: String,
    // Left Pane: Principal Groups & Figures
    pub working_capital: String,
    pub cash_in_hand: String,
    pub bank_accounts: String,
    pub bank_od_ac: String,
    pub sundry_debtors: String,
    pub sundry_creditors: String,
    pub sales_accounts: String,
    pub purchase_accounts: String,
    pub closing_stock: String,
    pub net_profit: String,
    // Right Pane: Principal Ratios
    pub current_ratio: String,
    pub quick_ratio: String,
    pub debt_equity_ratio: String,
    pub gross_profit_pct: String,
    pub net_profit_pct: String,
    pub operating_cost_pct: String,
    pub receivables_turnover_days: String,
    pub inventory_turnover: String,
    pub return_on_working_capital_pct: String,
}

pub struct RatioAnalysisService;

impl RatioAnalysisService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<RatioAnalysisReport, OpenTallyError> {
        let cid = company_id.to_string();
        let fdate = from_date.to_string();
        let tdate = to_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(
                async move { Self::calculate_internal(&mut *conn, &cid, &fdate, &tdate).await },
            )
        })
        .await
    }

    async fn calculate_internal(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<RatioAnalysisReport, OpenTallyError> {
        // Using pool reference inside read-only connection
        // We calculate balances directly using pure SQL & rust_decimal

        // Group balances as of to_date
        let cash_in_hand =
            Self::sum_group_closing_balance(conn, company_id, "Cash-in-hand", to_date).await?;
        let bank_accounts =
            Self::sum_group_closing_balance(conn, company_id, "Bank Accounts", to_date).await?;
        let bank_od_ac =
            Self::sum_group_closing_balance(conn, company_id, "Bank OD A/c", to_date).await?;
        let sundry_debtors =
            Self::sum_group_closing_balance(conn, company_id, "Sundry Debtors", to_date).await?;
        let sundry_creditors =
            Self::sum_group_closing_balance(conn, company_id, "Sundry Creditors", to_date).await?;

        // Period movements
        let sales_accounts =
            Self::sum_group_period_movement(conn, company_id, "Sales Accounts", from_date, to_date)
                .await?;
        let purchase_accounts = Self::sum_group_period_movement(
            conn,
            company_id,
            "Purchase Accounts",
            from_date,
            to_date,
        )
        .await?;

        // Closing stock from cost layers
        let closing_stock_layers: Vec<(String, String)> = sqlx::query_as(
            "SELECT remaining_quantity, unit_cost FROM inventory_cost_layers
             WHERE company_id = ? AND receipt_date <= ?",
        )
        .bind(company_id)
        .bind(to_date)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        let mut closing_stock = Decimal::ZERO;
        for (q_str, c_str) in closing_stock_layers {
            let q = Decimal::from_str(&q_str).unwrap_or(Decimal::ZERO);
            let c = Decimal::from_str(&c_str).unwrap_or(Decimal::ZERO);
            closing_stock += q * c;
        }
        if closing_stock.is_zero() {
            closing_stock =
                Self::sum_group_closing_balance(conn, company_id, "Stock-in-hand", to_date).await?;
        }

        // Current Assets: Cash + Bank + Debtors + Closing Stock + any other Current Assets
        let other_ca =
            Self::sum_group_closing_balance(conn, company_id, "Current Assets", to_date).await?;
        let current_assets =
            (cash_in_hand + bank_accounts + sundry_debtors + closing_stock).max(other_ca);

        // Current Liabilities: Creditors + Bank OD + other Current Liabilities
        let other_cl =
            Self::sum_group_closing_balance(conn, company_id, "Current Liabilities", to_date)
                .await?;
        let current_liabilities = (sundry_creditors + bank_od_ac).max(other_cl);

        let working_capital = current_assets - current_liabilities;

        // Total Debt (Loans/Borrowings/Liabilities)
        let secured_loans =
            Self::sum_group_closing_balance(conn, company_id, "Secured Loans", to_date).await?;
        let unsecured_loans =
            Self::sum_group_closing_balance(conn, company_id, "Unsecured Loans", to_date).await?;
        let total_debt = secured_loans + unsecured_loans + bank_od_ac;

        // Equity (Capital Account + Reserves & Surplus)
        let capital_ac =
            Self::sum_group_closing_balance(conn, company_id, "Capital Account", to_date).await?;
        let reserves =
            Self::sum_group_closing_balance(conn, company_id, "Reserves & Surplus", to_date)
                .await?;
        let equity = capital_ac + reserves;

        // Gross Profit and Net Profit
        // Income ledgers net movements
        let total_income: Decimal =
            Self::sum_nature_period(conn, company_id, "income", from_date, to_date).await?;
        let total_expenses: Decimal =
            Self::sum_nature_period(conn, company_id, "expense", from_date, to_date).await?;
        let net_profit = total_income - total_expenses;
        let gross_profit = (sales_accounts - purchase_accounts).max(Decimal::ZERO);

        // COGS
        let cogs = (purchase_accounts).max(Decimal::ZERO);

        // Right pane calculations with exact Decimal
        let current_ratio = if current_liabilities > Decimal::ZERO {
            format!(
                "{:.2} : 1",
                (current_assets / current_liabilities).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        let quick_assets = current_assets - closing_stock;
        let quick_ratio = if current_liabilities > Decimal::ZERO {
            format!(
                "{:.2} : 1",
                (quick_assets / current_liabilities).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        let debt_equity_ratio = if equity > Decimal::ZERO {
            format!("{:.2} : 1", (total_debt / equity).round_dp(2))
        } else {
            "N/A".to_string()
        };

        let gross_profit_pct = if sales_accounts > Decimal::ZERO {
            format!(
                "{:.2}%",
                ((gross_profit / sales_accounts) * Decimal::new(100, 0)).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        let net_profit_pct = if sales_accounts > Decimal::ZERO {
            format!(
                "{:.2}%",
                ((net_profit / sales_accounts) * Decimal::new(100, 0)).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        let operating_expenses = total_expenses;
        let operating_cost_pct = if sales_accounts > Decimal::ZERO {
            format!(
                "{:.2}%",
                ((operating_expenses / sales_accounts) * Decimal::new(100, 0)).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        let receivables_turnover_days = if sales_accounts > Decimal::ZERO {
            let days = ((sundry_debtors / sales_accounts) * Decimal::new(365, 0)).round_dp(0);
            format!("{} days", days)
        } else {
            "N/A".to_string()
        };

        let inventory_turnover = if closing_stock > Decimal::ZERO {
            format!("{:.2}", (cogs / closing_stock).round_dp(2))
        } else {
            "N/A".to_string()
        };

        let return_on_working_capital_pct = if working_capital > Decimal::ZERO {
            format!(
                "{:.2}%",
                ((net_profit / working_capital) * Decimal::new(100, 0)).round_dp(2)
            )
        } else {
            "N/A".to_string()
        };

        Ok(RatioAnalysisReport {
            company_id: company_id.to_string(),
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            working_capital: working_capital.round_dp(2).to_string(),
            cash_in_hand: cash_in_hand.round_dp(2).to_string(),
            bank_accounts: bank_accounts.round_dp(2).to_string(),
            bank_od_ac: bank_od_ac.round_dp(2).to_string(),
            sundry_debtors: sundry_debtors.round_dp(2).to_string(),
            sundry_creditors: sundry_creditors.round_dp(2).to_string(),
            sales_accounts: sales_accounts.round_dp(2).to_string(),
            purchase_accounts: purchase_accounts.round_dp(2).to_string(),
            closing_stock: closing_stock.round_dp(2).to_string(),
            net_profit: net_profit.round_dp(2).to_string(),
            current_ratio,
            quick_ratio,
            debt_equity_ratio,
            gross_profit_pct,
            net_profit_pct,
            operating_cost_pct,
            receivables_turnover_days,
            inventory_turnover,
            return_on_working_capital_pct,
        })
    }

    async fn sum_group_closing_balance(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        group_name: &str,
        as_of_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        // Collect ledgers under this group and its recursive descendants
        let ledger_ids: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE grp_tree AS (
                SELECT id FROM account_groups WHERE company_id = ? AND name = ? COLLATE NOCASE
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
        .bind(group_name)
        .bind(company_id)
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        if ledger_ids.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let mut balance = Decimal::ZERO;

        // Opening balance
        for lid in &ledger_ids {
            let openings: Vec<(String, String)> = sqlx::query_as(
                "SELECT amount, balance_type FROM opening_balances WHERE company_id = ? AND ledger_id = ?",
            )
            .bind(company_id)
            .bind(lid)
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

            // Movements up to as_of_date
            let movements: Vec<(String, String)> = sqlx::query_as(
                "SELECT vl.amount, vl.entry_type FROM voucher_lines vl
                 JOIN vouchers v ON v.id = vl.voucher_id AND v.company_id = vl.company_id
                 WHERE vl.company_id = ? AND vl.ledger_id = ?
                 AND v.status = 'posted' AND v.date <= ?",
            )
            .bind(company_id)
            .bind(lid)
            .bind(as_of_date)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for (amt_str, entry_type) in movements {
                if let Ok(amt) = Decimal::from_str(&amt_str) {
                    if entry_type == "Dr" {
                        balance += amt;
                    } else {
                        balance -= amt;
                    }
                }
            }
        }

        Ok(balance.abs())
    }

    async fn sum_group_period_movement(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        group_name: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let ledger_ids: Vec<String> = sqlx::query_scalar(
            "WITH RECURSIVE grp_tree AS (
                SELECT id FROM account_groups WHERE company_id = ? AND name = ? COLLATE NOCASE
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
        .bind(group_name)
        .bind(company_id)
        .bind(company_id)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        if ledger_ids.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let mut period_total = Decimal::ZERO;
        for lid in &ledger_ids {
            let movements: Vec<(String, String)> = sqlx::query_as(
                "SELECT vl.amount, vl.entry_type FROM voucher_lines vl
                 JOIN vouchers v ON v.id = vl.voucher_id AND v.company_id = vl.company_id
                 WHERE vl.company_id = ? AND vl.ledger_id = ?
                 AND v.status = 'posted' AND v.date >= ? AND v.date <= ?",
            )
            .bind(company_id)
            .bind(lid)
            .bind(from_date)
            .bind(to_date)
            .fetch_all(&mut *conn)
            .await
            .unwrap_or_default();

            for (amt_str, _) in movements {
                if let Ok(amt) = Decimal::from_str(&amt_str) {
                    period_total += amt;
                }
            }
        }

        Ok(period_total)
    }

    async fn sum_nature_period(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        nature: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let nature_norm = if nature.eq_ignore_ascii_case("income") {
            "Income"
        } else if nature.eq_ignore_ascii_case("expense") || nature.eq_ignore_ascii_case("expenses")
        {
            "Expenses"
        } else {
            nature
        };

        let movements: Vec<(String, String)> = sqlx::query_as(
            "SELECT vl.amount, vl.entry_type FROM voucher_lines vl
             JOIN vouchers v ON v.id = vl.voucher_id AND v.company_id = vl.company_id
             JOIN ledgers l ON l.id = vl.ledger_id AND l.company_id = vl.company_id
             JOIN account_groups ag ON ag.id = l.group_id AND ag.company_id = l.company_id
             WHERE vl.company_id = ? AND ag.nature = ? COLLATE NOCASE
             AND v.status = 'posted' AND v.date >= ? AND v.date <= ?",
        )
        .bind(company_id)
        .bind(nature_norm)
        .bind(from_date)
        .bind(to_date)
        .fetch_all(&mut *conn)
        .await
        .unwrap_or_default();

        let mut total = Decimal::ZERO;
        for (amt_str, entry_type) in movements {
            if let Ok(amt) = Decimal::from_str(&amt_str) {
                if (nature_norm == "Income" && entry_type == "Cr")
                    || (nature_norm == "Expenses" && entry_type == "Dr")
                {
                    total += amt;
                } else {
                    total -= amt;
                }
            }
        }

        Ok(total.max(Decimal::ZERO))
    }
}
