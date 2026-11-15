use crate::errors::OpenTallyError;
use crate::reports::calculation::parse_report_amount;
use crate::reports::model::{
    CashFlowReport, CashFlowRequest, CashFlowRow, ReportDateRange, ReportOpeningBalance,
};
use crate::reports::query::ReportQueryService;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, sqlx::FromRow)]
struct GroupMeta {
    id: String,
    parent_id: Option<String>,
    name: String,
}
#[derive(Debug, Clone, sqlx::FromRow)]
struct LedgerMeta {
    ledger_id: String,
    group_id: String,
    ledger_classification: String,
}
#[derive(Debug, Clone, sqlx::FromRow)]
struct CashLine {
    voucher_id: String,
    voucher_number: String,
    voucher_date: String,
    narration: Option<String>,
    ledger_id: String,
    ledger_name: String,
    entry_type: String,
    amount: String,
    group_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activity {
    Operating,
    Investing,
    Financing,
}
impl Activity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Operating => "operating",
            Self::Investing => "investing",
            Self::Financing => "financing",
        }
    }
}

pub struct CashFlowService;

impl CashFlowService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: CashFlowRequest,
    ) -> Result<CashFlowReport, OpenTallyError> {
        let period = ReportDateRange::new(
            request.period.date_from.clone(),
            request.period.date_to.clone(),
        )
        .map_err(OpenTallyError::Validation)?;
        let fy: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;
        let fy_date =
            crate::reports::model::parse_report_date(&fy).map_err(OpenTallyError::Validation)?;
        let from_date = crate::reports::model::parse_report_date(&period.date_from)
            .map_err(OpenTallyError::Validation)?;
        if from_date < fy_date {
            return Err(OpenTallyError::Validation(
                "Cash flow period cannot precede the financial year start".into(),
            ));
        }
        let groups = list_groups(pool, company_id).await?;
        let ledgers = list_ledgers(pool, company_id).await?;
        let selected = ledgers
            .iter()
            .filter(|l| is_cash_or_bank(l, &groups))
            .filter(|l| {
                request
                    .cash_ledger_id
                    .as_deref()
                    .map(|id| id == l.ledger_id)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        if let Some(id) = request.cash_ledger_id.as_deref() {
            if !ledgers.iter().any(|l| l.ledger_id == id) {
                return Err(OpenTallyError::Validation(format!(
                    "Cash/bank ledger '{}' was not found for this company",
                    id
                )));
            }
            if selected.is_empty() {
                return Err(OpenTallyError::Validation(format!(
                    "Ledger '{}' is not classified as cash or bank",
                    id
                )));
            }
        }
        let cash_ids = selected
            .iter()
            .map(|l| l.ledger_id.clone())
            .collect::<HashSet<_>>();
        let openings =
            ReportQueryService::list_opening_balances(pool, company_id, &fy, None).await?;
        let lines = list_lines(pool, company_id, &fy, &period.date_to).await?;
        let mut opening = opening_amount(&openings, &cash_ids)?;
        let mut actual_closing = opening;
        let mut period_cash = Vec::new();
        for line in &lines {
            if !cash_ids.contains(&line.ledger_id) {
                continue;
            }
            actual_closing += signed_cash(line)?;
            if line.voucher_date < period.date_from {
                opening += signed_cash(line)?;
            } else {
                period_cash.push(line.clone());
            }
        }
        let context = lines_for_vouchers(&lines, &cash_ids);
        let mut by_voucher = HashMap::<String, Vec<CashLine>>::new();
        for line in context {
            by_voucher
                .entry(line.voucher_id.clone())
                .or_default()
                .push(line.clone());
        }
        let mut seen = HashSet::new();
        let mut warnings = Vec::new();
        let mut rows = Vec::new();
        for cash in period_cash {
            if !seen.insert((cash.voucher_id.clone(), cash.ledger_id.clone())) {
                continue;
            }
            let voucher_lines = by_voucher
                .get(&cash.voucher_id)
                .cloned()
                .unwrap_or_default();
            let counterparts = voucher_lines
                .iter()
                .filter(|l| !cash_ids.contains(&l.ledger_id))
                .collect::<Vec<_>>();
            if counterparts.is_empty() {
                continue;
            }
            if voucher_lines
                .iter()
                .filter(|l| cash_ids.contains(&l.ledger_id))
                .count()
                != 1
            {
                warnings.push(format!(
                    "Voucher '{}' has multiple cash/bank lines; activity omitted",
                    cash.voucher_number
                ));
                continue;
            }
            for counterpart in counterparts {
                let activity = classify_activity(&counterpart.group_id, &groups)?;
                if request
                    .activity_filter
                    .as_deref()
                    .map(|f| f != activity.as_str())
                    .unwrap_or(false)
                {
                    continue;
                }
                let amount = parse_report_amount(&counterpart.amount)?;
                let amount = if cash.entry_type == "Dr" {
                    amount
                } else {
                    -amount
                };
                rows.push(CashFlowRow {
                    activity: activity.as_str().into(),
                    ledger_id: cash.ledger_id.clone(),
                    ledger_name: cash.ledger_name.clone(),
                    voucher_id: cash.voucher_id.clone(),
                    voucher_number: cash.voucher_number.clone(),
                    date: cash.voucher_date.clone(),
                    description: cash.narration.clone(),
                    amount: amount.to_string(),
                });
            }
        }
        rows.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| a.voucher_number.cmp(&b.voucher_number))
                .then_with(|| a.ledger_name.cmp(&b.ledger_name))
                .then_with(|| a.voucher_id.cmp(&b.voucher_id))
        });
        let operating_rows = rows
            .iter()
            .filter(|r| r.activity == Activity::Operating.as_str())
            .cloned()
            .collect::<Vec<_>>();
        let investing_rows = rows
            .iter()
            .filter(|r| r.activity == Activity::Investing.as_str())
            .cloned()
            .collect::<Vec<_>>();
        let financing_rows = rows
            .iter()
            .filter(|r| r.activity == Activity::Financing.as_str())
            .cloned()
            .collect::<Vec<_>>();
        let operating = subtotal(&operating_rows)?;
        let investing = subtotal(&investing_rows)?;
        let financing = subtotal(&financing_rows)?;
        let net = operating + investing + financing;
        let closing = opening + net;
        let difference = closing - actual_closing;
        Ok(CashFlowReport {
            company_id: company_id.into(),
            date_from: period.date_from,
            date_to: period.date_to,
            opening_cash_balance: opening.to_string(),
            operating_rows,
            operating_subtotal: operating.to_string(),
            investing_rows,
            investing_subtotal: investing.to_string(),
            financing_rows,
            financing_subtotal: financing.to_string(),
            net_change_in_cash: net.to_string(),
            closing_cash_balance: closing.to_string(),
            actual_closing_cash_balance: actual_closing.to_string(),
            reconciliation_difference: difference.to_string(),
            is_reconciled: difference.is_zero(),
            ledger_count: selected.len(),
            warnings,
        })
    }
}

fn lines_for_vouchers<'a>(lines: &'a [CashLine], cash_ids: &HashSet<String>) -> Vec<&'a CashLine> {
    let ids = lines
        .iter()
        .filter(|l| cash_ids.contains(&l.ledger_id))
        .map(|l| l.voucher_id.as_str())
        .collect::<HashSet<_>>();
    lines
        .iter()
        .filter(|l| ids.contains(l.voucher_id.as_str()))
        .collect()
}
fn signed_cash(line: &CashLine) -> Result<Decimal, OpenTallyError> {
    let amount = parse_report_amount(&line.amount)?;
    match line.entry_type.as_str() {
        "Dr" => Ok(amount),
        "Cr" => Ok(-amount),
        other => Err(OpenTallyError::Validation(format!(
            "Invalid cash flow entry type '{}'",
            other
        ))),
    }
}
fn opening_amount(
    openings: &[ReportOpeningBalance],
    ids: &HashSet<String>,
) -> Result<Decimal, OpenTallyError> {
    openings
        .iter()
        .filter(|o| ids.contains(&o.ledger_id))
        .try_fold(Decimal::ZERO, |total, o| {
            let amount = parse_report_amount(&o.amount)?;
            match o.balance_type.as_str() {
                "Dr" => Ok(total + amount),
                "Cr" => Ok(total - amount),
                other => Err(OpenTallyError::Validation(format!(
                    "Invalid opening balance type '{}'",
                    other
                ))),
            }
        })
}
fn subtotal(rows: &[CashFlowRow]) -> Result<Decimal, OpenTallyError> {
    rows.iter().try_fold(Decimal::ZERO, |total, row| {
        Ok(total + parse_report_amount(&row.amount)?)
    })
}
fn is_cash_or_bank(ledger: &LedgerMeta, groups: &HashMap<String, GroupMeta>) -> bool {
    ledger.ledger_classification == "cash"
        || has_ancestor(&ledger.group_id, groups, &["Cash-in-Hand", "Bank Accounts"])
}
fn classify_activity(
    group_id: &str,
    groups: &HashMap<String, GroupMeta>,
) -> Result<Activity, OpenTallyError> {
    let mut current = Some(group_id.to_string());
    let mut seen = HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            return Err(OpenTallyError::Validation(
                "Account group hierarchy contains a cycle".into(),
            ));
        }
        let group = groups
            .get(&id)
            .ok_or_else(|| OpenTallyError::Validation(format!("Missing account group '{}'", id)))?;
        if matches!(group.name.as_str(), "Fixed Assets" | "Investments") {
            return Ok(Activity::Investing);
        }
        if matches!(
            group.name.as_str(),
            "Capital Account" | "Loans (Liability)" | "Bank OD A/c"
        ) {
            return Ok(Activity::Financing);
        }
        current = group.parent_id.clone();
    }
    Ok(Activity::Operating)
}
fn has_ancestor(group_id: &str, groups: &HashMap<String, GroupMeta>, names: &[&str]) -> bool {
    let mut current = Some(group_id.to_string());
    let mut seen = HashSet::new();
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            return false;
        }
        let Some(group) = groups.get(&id) else {
            return false;
        };
        if names.iter().any(|name| *name == group.name) {
            return true;
        }
        current = group.parent_id.clone();
    }
    false
}

async fn list_groups(
    pool: &Pool<Sqlite>,
    company_id: &str,
) -> Result<HashMap<String, GroupMeta>, OpenTallyError> {
    let id = company_id.to_string();
    let rows = execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            sqlx::query_as::<_, GroupMeta>(
                "SELECT id,parent_id,name FROM account_groups WHERE company_id = ? ORDER BY id",
            )
            .bind(id)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)
        })
    })
    .await?;
    Ok(rows.into_iter().map(|g| (g.id.clone(), g)).collect())
}
async fn list_ledgers(
    pool: &Pool<Sqlite>,
    company_id: &str,
) -> Result<Vec<LedgerMeta>, OpenTallyError> {
    let id = company_id.to_string();
    execute_read_only_query(pool,move|conn|Box::pin(async move{sqlx::query_as::<_,LedgerMeta>("SELECT l.id AS ledger_id,l.group_id,l.ledger_classification FROM ledgers l JOIN account_groups g ON g.company_id=l.company_id AND g.id=l.group_id WHERE l.company_id=? AND g.is_hidden=0 ORDER BY l.name,l.id").bind(id).fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)})).await
}
async fn list_lines(
    pool: &Pool<Sqlite>,
    company_id: &str,
    from: &str,
    to: &str,
) -> Result<Vec<CashLine>, OpenTallyError> {
    let id = company_id.to_string();
    let from = from.to_string();
    let to = to.to_string();
    execute_read_only_query(pool,move|conn|Box::pin(async move{sqlx::query_as::<_,CashLine>("SELECT v.id AS voucher_id,v.voucher_number,v.date AS voucher_date,v.narration,vl.ledger_id,l.name AS ledger_name,vl.entry_type,vl.amount,l.group_id FROM vouchers v JOIN voucher_lines vl ON vl.company_id=v.company_id AND vl.voucher_id=v.id JOIN ledgers l ON l.company_id=vl.company_id AND l.id=vl.ledger_id JOIN account_groups g ON g.company_id=l.company_id AND g.id=l.group_id WHERE v.company_id=? AND v.status IN ('posted','reversed') AND v.date>=? AND v.date<=? AND g.is_hidden=0 ORDER BY v.date,v.id,vl.line_number").bind(id).bind(from).bind(to).fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)})).await
}

#[cfg(test)]
mod tests {
    use super::{classify_activity, Activity, GroupMeta};
    use std::collections::HashMap;
    #[test]
    fn fixed_assets_are_investing() {
        let mut g = HashMap::new();
        g.insert(
            "f".into(),
            GroupMeta {
                id: "f".into(),
                parent_id: None,
                name: "Fixed Assets".into(),
            },
        );
        assert_eq!(classify_activity("f", &g).unwrap(), Activity::Investing);
    }
    #[test]
    fn capital_is_financing() {
        let mut g = HashMap::new();
        g.insert(
            "c".into(),
            GroupMeta {
                id: "c".into(),
                parent_id: None,
                name: "Capital Account".into(),
            },
        );
        assert_eq!(classify_activity("c", &g).unwrap(), Activity::Financing);
    }
}
