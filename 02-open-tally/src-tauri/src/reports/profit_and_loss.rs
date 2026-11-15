use crate::errors::OpenTallyError;
use crate::reports::calculation::parse_report_amount;
use crate::reports::model::{
    ProfitAndLossLedgerRow, ProfitAndLossReport, ProfitAndLossRequest, ReportDateRange,
};
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(Debug, sqlx::FromRow)]
struct ProfitLedger {
    ledger_id: String,
    ledger_name: String,
    account_group_id: String,
    account_group_name: String,
    account_group_nature: String,
    affects_gross_profit: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct ProfitMovement {
    ledger_id: String,
    entry_type: String,
    amount: String,
}

#[derive(Debug, Default)]
struct MovementTotals {
    debit: Decimal,
    credit: Decimal,
}

pub struct ProfitAndLossService;

impl ProfitAndLossService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: ProfitAndLossRequest,
    ) -> Result<ProfitAndLossReport, OpenTallyError> {
        let period = ReportDateRange::new(
            request.period.date_from.clone(),
            request.period.date_to.clone(),
        )
        .map_err(OpenTallyError::Validation)?;
        let ledgers = list_ledgers(pool, company_id, &request).await?;
        let movements = list_movements(pool, company_id, &period, &request).await?;
        let mut totals = HashMap::<String, MovementTotals>::new();
        for movement in movements {
            let amount = parse_report_amount(&movement.amount)?;
            let entry = totals.entry(movement.ledger_id).or_default();
            match movement.entry_type.as_str() {
                "Dr" => entry.debit += amount,
                "Cr" => entry.credit += amount,
                other => {
                    return Err(OpenTallyError::Validation(format!(
                        "Invalid profit and loss entry type '{}'",
                        other
                    )))
                }
            }
        }

        let mut direct_income_rows = Vec::new();
        let mut direct_expense_rows = Vec::new();
        let mut indirect_income_rows = Vec::new();
        let mut indirect_expense_rows = Vec::new();
        let mut direct_income_subtotal = Decimal::ZERO;
        let mut direct_expense_subtotal = Decimal::ZERO;
        let mut indirect_income_subtotal = Decimal::ZERO;
        let mut indirect_expense_subtotal = Decimal::ZERO;

        for ledger in ledgers {
            let movement = totals.remove(&ledger.ledger_id).unwrap_or_default();
            let is_income = ledger.account_group_nature == "Income";
            let contribution = if is_income {
                movement.credit - movement.debit
            } else {
                movement.debit - movement.credit
            };
            if !request.include_zero_balances && contribution.is_zero() {
                continue;
            }
            let row = ProfitAndLossLedgerRow {
                ledger_id: ledger.ledger_id,
                ledger_name: ledger.ledger_name,
                account_group_id: ledger.account_group_id,
                account_group_name: ledger.account_group_name,
                account_group_nature: ledger.account_group_nature,
                is_direct: ledger.affects_gross_profit,
                debit: movement.debit.to_string(),
                credit: movement.credit.to_string(),
                contribution: contribution.to_string(),
            };
            match (is_income, ledger.affects_gross_profit) {
                (true, true) => {
                    direct_income_subtotal += contribution;
                    direct_income_rows.push(row);
                }
                (false, true) => {
                    direct_expense_subtotal += contribution;
                    direct_expense_rows.push(row);
                }
                (true, false) => {
                    indirect_income_subtotal += contribution;
                    indirect_income_rows.push(row);
                }
                (false, false) => {
                    indirect_expense_subtotal += contribution;
                    indirect_expense_rows.push(row);
                }
            }
        }

        sort_rows(&mut direct_income_rows);
        sort_rows(&mut direct_expense_rows);
        sort_rows(&mut indirect_income_rows);
        sort_rows(&mut indirect_expense_rows);
        let gross = direct_income_subtotal - direct_expense_subtotal;
        let net = gross + indirect_income_subtotal - indirect_expense_subtotal;
        let ledger_count = direct_income_rows.len()
            + direct_expense_rows.len()
            + indirect_income_rows.len()
            + indirect_expense_rows.len();
        Ok(ProfitAndLossReport {
            company_id: company_id.to_string(),
            date_from: period.date_from,
            date_to: period.date_to,
            direct_income_rows,
            direct_income_subtotal: direct_income_subtotal.to_string(),
            direct_expense_rows,
            direct_expense_subtotal: direct_expense_subtotal.to_string(),
            gross_profit_or_loss: gross.abs().to_string(),
            gross_result_type: result_type(gross, "Profit", "Loss"),
            indirect_income_rows,
            indirect_income_subtotal: indirect_income_subtotal.to_string(),
            indirect_expense_rows,
            indirect_expense_subtotal: indirect_expense_subtotal.to_string(),
            net_profit_or_loss: net.abs().to_string(),
            net_result_type: result_type(net, "Profit", "Loss"),
            ledger_count,
            warnings: vec![
                "Opening balances are excluded from period income and expense movements."
                    .to_string(),
            ],
        })
    }
}

fn result_type(value: Decimal, positive: &str, negative: &str) -> String {
    if value >= Decimal::ZERO {
        positive.to_string()
    } else {
        negative.to_string()
    }
}

fn sort_rows(rows: &mut [ProfitAndLossLedgerRow]) {
    rows.sort_by(|left, right| {
        left.account_group_name
            .cmp(&right.account_group_name)
            .then_with(|| left.ledger_name.cmp(&right.ledger_name))
            .then_with(|| left.ledger_id.cmp(&right.ledger_id))
    });
}

async fn list_ledgers(
    pool: &Pool<Sqlite>,
    company_id: &str,
    request: &ProfitAndLossRequest,
) -> Result<Vec<ProfitLedger>, OpenTallyError> {
    let company_id = company_id.to_string();
    let group_id = request.account_group_id.clone();
    let search = request
        .ledger_search
        .clone()
        .map(|value| format!("%{}%", value.to_lowercase()));
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let mut sql = String::from(
                "SELECT l.id AS ledger_id, l.name AS ledger_name,
                        g.id AS account_group_id, g.name AS account_group_name,
                        g.nature AS account_group_nature, g.affects_gross_profit
                 FROM ledgers l
                 JOIN account_groups g
                   ON g.company_id = l.company_id AND g.id = l.group_id
                 WHERE l.company_id = ? AND g.nature IN ('Income', 'Expenses')
                   AND g.is_hidden = 0",
            );
            if group_id.is_some() {
                sql.push_str(" AND l.group_id = ?");
            }
            if search.is_some() {
                sql.push_str(" AND lower(l.name) LIKE ?");
            }
            sql.push_str(" ORDER BY g.name ASC, l.name ASC, l.id ASC");
            let mut query = sqlx::query_as::<_, ProfitLedger>(&sql).bind(company_id);
            if let Some(value) = group_id {
                query = query.bind(value);
            }
            if let Some(value) = search {
                query = query.bind(value);
            }
            query
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
        })
    })
    .await
}

async fn list_movements(
    pool: &Pool<Sqlite>,
    company_id: &str,
    period: &ReportDateRange,
    request: &ProfitAndLossRequest,
) -> Result<Vec<ProfitMovement>, OpenTallyError> {
    let company_id = company_id.to_string();
    let group_id = request.account_group_id.clone();
    let search = request
        .ledger_search
        .clone()
        .map(|value| format!("%{}%", value.to_lowercase()));
    let date_from = period.date_from.clone();
    let date_to = period.date_to.clone();
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let mut sql = String::from(
                "SELECT vl.ledger_id, vl.entry_type, vl.amount
                 FROM vouchers v
                 JOIN voucher_lines vl
                   ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                 JOIN ledgers l
                   ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                 JOIN account_groups g
                   ON g.company_id = l.company_id AND g.id = l.group_id
                 WHERE v.company_id = ? AND v.status IN ('posted', 'reversed')
                   AND v.date >= ? AND v.date <= ?
                   AND g.nature IN ('Income', 'Expenses') AND g.is_hidden = 0",
            );
            if group_id.is_some() {
                sql.push_str(" AND l.group_id = ?");
            }
            if search.is_some() {
                sql.push_str(" AND lower(l.name) LIKE ?");
            }
            let mut query = sqlx::query_as::<_, ProfitMovement>(&sql)
                .bind(company_id)
                .bind(date_from)
                .bind(date_to);
            if let Some(value) = group_id {
                query = query.bind(value);
            }
            if let Some(value) = search {
                query = query.bind(value);
            }
            query
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::result_type;
    use rust_decimal::Decimal;

    #[test]
    fn income_and_expense_formulas_produce_gross_profit() {
        let income = Decimal::new(10000, 2);
        let expense = Decimal::new(7250, 2);
        assert_eq!((income - expense).to_string(), "27.50");
        assert_eq!(result_type(income - expense, "Profit", "Loss"), "Profit");
    }

    #[test]
    fn negative_result_is_reported_as_loss_without_sign_loss_in_calculation() {
        let result = Decimal::new(-1250, 2);
        assert_eq!(result.abs().to_string(), "12.50");
        assert_eq!(result_type(result, "Profit", "Loss"), "Loss");
    }
}
