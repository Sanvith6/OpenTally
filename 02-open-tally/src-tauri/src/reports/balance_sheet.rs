use crate::errors::OpenTallyError;
use crate::reports::calculation::parse_report_amount;
use crate::reports::model::{
    BalanceSheetLedgerRow, BalanceSheetReport, BalanceSheetRequest, ProfitAndLossRequest,
    ReportDateRange, ReportOpeningBalance,
};
use crate::reports::query::ReportQueryService;
use crate::reports::ProfitAndLossService;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(Debug, Clone, sqlx::FromRow)]
struct GroupMeta {
    id: String,
    parent_id: Option<String>,
    name: String,
    nature: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LedgerMeta {
    ledger_id: String,
    ledger_name: String,
    account_group_id: String,
    account_group_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct BalanceMovement {
    ledger_id: String,
    entry_type: String,
    amount: String,
}

#[derive(Debug, Default)]
struct Totals {
    debit: Decimal,
    credit: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Classification {
    FixedAssets,
    CurrentAssets,
    OtherAssets,
    LongTermLiabilities,
    CurrentLiabilities,
    OtherLiabilities,
    Equity,
    ProfitAndLoss,
}

impl Classification {
    fn as_str(self) -> &'static str {
        match self {
            Self::FixedAssets => "fixed_assets",
            Self::CurrentAssets => "current_assets",
            Self::OtherAssets => "other_assets",
            Self::LongTermLiabilities => "long_term_liabilities",
            Self::CurrentLiabilities => "current_liabilities",
            Self::OtherLiabilities => "other_liabilities",
            Self::Equity => "equity",
            Self::ProfitAndLoss => "profit_and_loss",
        }
    }
}

pub struct BalanceSheetService;

impl BalanceSheetService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: BalanceSheetRequest,
    ) -> Result<BalanceSheetReport, OpenTallyError> {
        let as_of = crate::reports::model::parse_report_date(&request.as_of_date)
            .map_err(OpenTallyError::Validation)?;
        let financial_year_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;
        let fy_start = crate::reports::model::parse_report_date(&financial_year_begins)
            .map_err(OpenTallyError::Validation)?;
        if as_of < fy_start {
            return Err(OpenTallyError::Validation(
                "Balance sheet date cannot precede the financial year start".to_string(),
            ));
        }

        let groups = list_groups(pool, company_id).await?;
        let ledgers = list_ledgers(pool, company_id, &request).await?;
        let movements = list_movements(
            pool,
            company_id,
            &financial_year_begins,
            &request.as_of_date,
            &request,
        )
        .await?;
        let openings = ReportQueryService::list_opening_balances(
            pool,
            company_id,
            &financial_year_begins,
            None,
        )
        .await?;
        let mut totals_by_ledger = HashMap::<String, Totals>::new();
        for opening in openings {
            apply_opening(&mut totals_by_ledger, &opening)?;
        }
        for movement in movements {
            let amount = parse_report_amount(&movement.amount)?;
            let totals = totals_by_ledger.entry(movement.ledger_id).or_default();
            match movement.entry_type.as_str() {
                "Dr" => totals.debit += amount,
                "Cr" => totals.credit += amount,
                other => {
                    return Err(OpenTallyError::Validation(format!(
                        "Invalid balance sheet entry type '{}'",
                        other
                    )))
                }
            }
        }

        let mut asset_rows = Vec::new();
        let mut liability_rows = Vec::new();
        let mut equity_rows = Vec::new();
        let mut total_assets = Decimal::ZERO;
        let mut total_liabilities = Decimal::ZERO;
        let mut total_equity = Decimal::ZERO;

        for ledger in ledgers {
            let classification = classify(&ledger.account_group_id, &groups)?;
            let totals = totals_by_ledger
                .remove(&ledger.ledger_id)
                .unwrap_or_default();
            if classification == Classification::ProfitAndLoss {
                continue;
            }
            let debit_normal = matches!(
                classification,
                Classification::FixedAssets
                    | Classification::CurrentAssets
                    | Classification::OtherAssets
            );
            let normal_balance = if debit_normal {
                totals.debit - totals.credit
            } else {
                totals.credit - totals.debit
            };
            if !request.include_zero_balances && normal_balance.is_zero() {
                continue;
            }
            let raw_balance = totals.debit - totals.credit;
            let (debit, credit) = if raw_balance >= Decimal::ZERO {
                (raw_balance, Decimal::ZERO)
            } else {
                (Decimal::ZERO, -raw_balance)
            };
            let balance_type = if normal_balance >= Decimal::ZERO {
                if debit_normal {
                    "Dr"
                } else {
                    "Cr"
                }
            } else if debit_normal {
                "Cr"
            } else {
                "Dr"
            };
            let row = BalanceSheetLedgerRow {
                ledger_id: ledger.ledger_id,
                ledger_name: ledger.ledger_name,
                account_group_id: ledger.account_group_id,
                account_group_name: ledger.account_group_name,
                classification: classification.as_str().to_string(),
                debit: debit.to_string(),
                credit: credit.to_string(),
                net_balance: normal_balance.to_string(),
                balance_type: balance_type.to_string(),
                presentation_amount: normal_balance.abs().to_string(),
            };
            match classification {
                Classification::FixedAssets
                | Classification::CurrentAssets
                | Classification::OtherAssets => {
                    total_assets += normal_balance;
                    asset_rows.push(row);
                }
                Classification::LongTermLiabilities
                | Classification::CurrentLiabilities
                | Classification::OtherLiabilities => {
                    total_liabilities += normal_balance;
                    liability_rows.push(row);
                }
                Classification::Equity => {
                    total_equity += normal_balance;
                    equity_rows.push(row);
                }
                Classification::ProfitAndLoss => unreachable!(),
            }
        }
        sort_rows(&mut asset_rows);
        sort_rows(&mut liability_rows);
        sort_rows(&mut equity_rows);
        let pnl = ProfitAndLossService::generate(
            pool,
            company_id,
            ProfitAndLossRequest {
                period: ReportDateRange::new(&financial_year_begins, &request.as_of_date)
                    .map_err(OpenTallyError::Validation)?,
                account_group_id: request.account_group_id.clone(),
                ledger_search: request.ledger_search.clone(),
                include_zero_balances: false,
            },
        )
        .await?;
        let pnl_amount = parse_report_amount(&pnl.net_profit_or_loss)?;
        let current_profit_or_loss = if pnl.net_result_type == "Profit" {
            pnl_amount
        } else {
            -pnl_amount
        };
        total_equity += current_profit_or_loss;
        let total_liabilities_and_equity = total_liabilities + total_equity;
        let balance_difference = total_assets - total_liabilities_and_equity;
        let ledger_count = asset_rows.len() + liability_rows.len() + equity_rows.len();
        Ok(BalanceSheetReport {
            company_id: company_id.to_string(),
            as_of_date: request.as_of_date,
            asset_rows,
            total_assets: total_assets.to_string(),
            liability_rows,
            total_liabilities: total_liabilities.to_string(),
            equity_rows,
            current_period_profit_or_loss: current_profit_or_loss.abs().to_string(),
            current_period_result_type: result_type(current_profit_or_loss),
            total_equity: total_equity.to_string(),
            total_liabilities_and_equity: total_liabilities_and_equity.to_string(),
            balance_difference: balance_difference.to_string(),
            is_balanced: balance_difference.is_zero(),
            ledger_count,
            warnings: vec![
                "Accumulated earnings are not a separate persisted account in schema 0.3.1; opening capital/equity balances are reported as stored.".to_string(),
                "Current-period profit or loss is calculated from financial-year start through the selected date and added once to equity.".to_string(),
            ],
        })
    }
}

fn result_type(value: Decimal) -> String {
    if value >= Decimal::ZERO {
        "Profit"
    } else {
        "Loss"
    }
    .to_string()
}

fn apply_opening(
    totals_by_ledger: &mut HashMap<String, Totals>,
    opening: &ReportOpeningBalance,
) -> Result<(), OpenTallyError> {
    let amount = parse_report_amount(&opening.amount)?;
    let totals = totals_by_ledger
        .entry(opening.ledger_id.clone())
        .or_default();
    match opening.balance_type.as_str() {
        "Dr" => totals.debit += amount,
        "Cr" => totals.credit += amount,
        other => {
            return Err(OpenTallyError::Validation(format!(
                "Invalid opening balance type '{}'",
                other
            )))
        }
    }
    Ok(())
}

fn classify(
    group_id: &str,
    groups: &HashMap<String, GroupMeta>,
) -> Result<Classification, OpenTallyError> {
    let mut current = Some(group_id.to_string());
    let mut visited = HashMap::<String, bool>::new();
    while let Some(id) = current {
        if visited.insert(id.clone(), true).is_some() {
            return Err(OpenTallyError::Validation(
                "Account group hierarchy contains a cycle".to_string(),
            ));
        }
        let group = groups.get(&id).ok_or_else(|| {
            OpenTallyError::Validation(format!("Ledger references missing account group '{}'", id))
        })?;
        let classification = match group.name.as_str() {
            "Capital Account" => Some(Classification::Equity),
            "Fixed Assets" => Some(Classification::FixedAssets),
            "Current Assets" => Some(Classification::CurrentAssets),
            "Current Liabilities" => Some(Classification::CurrentLiabilities),
            "Loans (Liability)" => Some(Classification::LongTermLiabilities),
            _ if group.nature == "Income" || group.nature == "Expenses" => {
                Some(Classification::ProfitAndLoss)
            }
            _ => None,
        };
        if let Some(value) = classification {
            return Ok(value);
        }
        current = group.parent_id.clone();
    }
    let group = groups.get(group_id).ok_or_else(|| {
        OpenTallyError::Validation(format!(
            "Ledger references missing account group '{}'",
            group_id
        ))
    })?;
    match group.nature.as_str() {
        "Assets" => Ok(Classification::OtherAssets),
        "Liabilities" => Ok(Classification::OtherLiabilities),
        _ => Err(OpenTallyError::Validation(format!(
            "Unsupported balance sheet account classification for group '{}'",
            group.name
        ))),
    }
}

fn sort_rows(rows: &mut [BalanceSheetLedgerRow]) {
    rows.sort_by(|left, right| {
        left.classification
            .cmp(&right.classification)
            .then_with(|| left.account_group_name.cmp(&right.account_group_name))
            .then_with(|| left.ledger_name.cmp(&right.ledger_name))
            .then_with(|| left.ledger_id.cmp(&right.ledger_id))
    });
}

async fn list_groups(
    pool: &Pool<Sqlite>,
    company_id: &str,
) -> Result<HashMap<String, GroupMeta>, OpenTallyError> {
    let rows: Vec<GroupMeta> = execute_read_only_query(pool, move |conn| {
        let company_id = company_id.to_string();
        Box::pin(async move {
            sqlx::query_as::<_, GroupMeta>(
                "SELECT id, parent_id, name, nature
                 FROM account_groups WHERE company_id = ? ORDER BY id ASC",
            )
            .bind(company_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)
        })
    })
    .await?;
    Ok(rows
        .into_iter()
        .map(|group| (group.id.clone(), group))
        .collect())
}

async fn list_ledgers(
    pool: &Pool<Sqlite>,
    company_id: &str,
    request: &BalanceSheetRequest,
) -> Result<Vec<LedgerMeta>, OpenTallyError> {
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
                        l.group_id AS account_group_id, g.name AS account_group_name
                 FROM ledgers l JOIN account_groups g
                   ON g.company_id = l.company_id AND g.id = l.group_id
                 WHERE l.company_id = ? AND g.is_hidden = 0",
            );
            if group_id.is_some() {
                sql.push_str(" AND l.group_id = ?");
            }
            if search.is_some() {
                sql.push_str(" AND lower(l.name) LIKE ?");
            }
            sql.push_str(" ORDER BY g.name ASC, l.name ASC, l.id ASC");
            let mut query = sqlx::query_as::<_, LedgerMeta>(&sql).bind(company_id);
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
    from: &str,
    to: &str,
    request: &BalanceSheetRequest,
) -> Result<Vec<BalanceMovement>, OpenTallyError> {
    let company_id = company_id.to_string();
    let group_id = request.account_group_id.clone();
    let search = request
        .ledger_search
        .clone()
        .map(|value| format!("%{}%", value.to_lowercase()));
    let from = from.to_string();
    let to = to.to_string();
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let mut sql = String::from(
                "SELECT vl.ledger_id, vl.entry_type, vl.amount
                 FROM vouchers v JOIN voucher_lines vl
                   ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                 JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                 JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
                 WHERE v.company_id = ? AND v.status IN ('posted', 'reversed')
                   AND v.date >= ? AND v.date <= ? AND g.is_hidden = 0",
            );
            if group_id.is_some() {
                sql.push_str(" AND l.group_id = ?");
            }
            if search.is_some() {
                sql.push_str(" AND lower(l.name) LIKE ?");
            }
            let mut query = sqlx::query_as::<_, BalanceMovement>(&sql)
                .bind(company_id)
                .bind(from)
                .bind(to);
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
    use super::{classify, result_type, Classification, GroupMeta};
    use rust_decimal::Decimal;
    use std::collections::HashMap;

    #[test]
    fn canonical_group_ancestry_classifies_capital_as_equity() {
        let mut groups = HashMap::new();
        groups.insert(
            "capital".into(),
            GroupMeta {
                id: "capital".into(),
                parent_id: None,
                name: "Capital Account".into(),
                nature: "Liabilities".into(),
            },
        );
        groups.insert(
            "child".into(),
            GroupMeta {
                id: "child".into(),
                parent_id: Some("capital".into()),
                name: "Owner Funds".into(),
                nature: "Liabilities".into(),
            },
        );
        assert_eq!(classify("child", &groups).unwrap(), Classification::Equity);
    }

    #[test]
    fn balance_difference_is_not_forced_to_zero() {
        let difference = Decimal::new(125, 2);
        assert!(!difference.is_zero());
        assert_eq!(result_type(Decimal::new(-1, 2)), "Loss");
    }
}
