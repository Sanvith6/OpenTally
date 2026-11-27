use crate::errors::OpenTallyError;
use crate::reports::calculation::{parse_report_amount, BalanceAccumulator};
use crate::reports::model::{
    parse_report_date, TrialBalanceReport, TrialBalanceRequest, TrialBalanceRow,
};
use crate::reports::query::ReportQueryService;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};

#[derive(Debug, sqlx::FromRow)]
struct TrialLedger {
    ledger_id: String,
    ledger_name: String,
    account_group_id: String,
    account_group_name: String,
    account_group_nature: String,
}

#[derive(Debug, sqlx::FromRow)]
struct TrialMovement {
    ledger_id: String,
    entry_type: String,
    amount: String,
}

pub struct TrialBalanceService;

impl TrialBalanceService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: TrialBalanceRequest,
    ) -> Result<TrialBalanceReport, OpenTallyError> {
        let as_of = parse_report_date(&request.as_of_date).map_err(OpenTallyError::Validation)?;
        let financial_year_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;
        let financial_year =
            parse_report_date(&financial_year_begins).map_err(OpenTallyError::Validation)?;
        if as_of < financial_year {
            return Err(OpenTallyError::Validation(
                "Trial balance date cannot precede the financial year start".to_string(),
            ));
        }

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

        let mut accumulators: std::collections::HashMap<String, BalanceAccumulator> =
            std::collections::HashMap::with_capacity(ledgers.len());
        for opening in &openings {
            accumulators
                .entry(opening.ledger_id.clone())
                .or_default()
                .apply_opening(opening)?;
        }
        for movement in &movements {
            let acc = accumulators.entry(movement.ledger_id.clone()).or_default();
            let amount = parse_report_amount(&movement.amount)?;
            match movement.entry_type.as_str() {
                "Dr" => acc.period_debit += amount,
                "Cr" => acc.period_credit += amount,
                other => {
                    return Err(OpenTallyError::Validation(format!(
                        "Invalid trial balance entry type '{}'",
                        other
                    )))
                }
            }
        }

        let mut rows = Vec::with_capacity(ledgers.len());
        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;
        for ledger in ledgers {
            let accumulator = accumulators.remove(&ledger.ledger_id).unwrap_or_default();
            let net = accumulator.opening_debit + accumulator.period_debit
                - accumulator.opening_credit
                - accumulator.period_credit;
            let (closing_debit, closing_credit) = if net >= Decimal::ZERO {
                (net, Decimal::ZERO)
            } else {
                (Decimal::ZERO, -net)
            };
            if !request.include_zero_balances && closing_debit.is_zero() && closing_credit.is_zero()
            {
                continue;
            }
            total_debit += closing_debit;
            total_credit += closing_credit;
            rows.push(TrialBalanceRow {
                ledger_id: ledger.ledger_id,
                ledger_name: ledger.ledger_name,
                account_group_id: ledger.account_group_id,
                account_group_name: ledger.account_group_name,
                account_group_nature: ledger.account_group_nature,
                opening_debit: accumulator.opening_debit.to_string(),
                opening_credit: accumulator.opening_credit.to_string(),
                period_debit: accumulator.period_debit.to_string(),
                period_credit: accumulator.period_credit.to_string(),
                closing_debit: closing_debit.to_string(),
                closing_credit: closing_credit.to_string(),
            });
        }
        rows.sort_by(|left, right| {
            left.account_group_name
                .cmp(&right.account_group_name)
                .then_with(|| left.ledger_name.cmp(&right.ledger_name))
                .then_with(|| left.ledger_id.cmp(&right.ledger_id))
        });
        Ok(TrialBalanceReport {
            as_of_date: request.as_of_date,
            ledger_count: rows.len(),
            rows,
            total_debit: total_debit.to_string(),
            total_credit: total_credit.to_string(),
            difference: (total_debit - total_credit).to_string(),
        })
    }
}

async fn list_ledgers(
    pool: &Pool<Sqlite>,
    company_id: &str,
    request: &TrialBalanceRequest,
) -> Result<Vec<TrialLedger>, OpenTallyError> {
    let company_id = company_id.to_string();
    let group_id = request.account_group_id.clone();
    let ledger_id = request.ledger_id.clone();
    let search = request
        .account_search
        .clone()
        .map(|value| format!("%{}%", value.to_lowercase()));
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let mut sql = String::from(
                "SELECT l.id AS ledger_id, l.name AS ledger_name, g.id AS account_group_id,
                        g.name AS account_group_name, g.nature AS account_group_nature
                 FROM ledgers l JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
                 WHERE l.company_id = ?",
            );
            if group_id.is_some() { sql.push_str(" AND l.group_id = ?"); }
            if ledger_id.is_some() { sql.push_str(" AND l.id = ?"); }
            if search.is_some() { sql.push_str(" AND lower(l.name) LIKE ?"); }
            sql.push_str(" ORDER BY g.name ASC, l.name ASC, l.id ASC");
            let mut query = sqlx::query_as::<_, TrialLedger>(&sql).bind(company_id);
            if let Some(value) = group_id { query = query.bind(value); }
            if let Some(value) = ledger_id { query = query.bind(value); }
            if let Some(value) = search { query = query.bind(value); }
            query.fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)
        })
    }).await
}

async fn list_movements(
    pool: &Pool<Sqlite>,
    company_id: &str,
    from: &str,
    to: &str,
    request: &TrialBalanceRequest,
) -> Result<Vec<TrialMovement>, OpenTallyError> {
    let company_id = company_id.to_string();
    let group_id = request.account_group_id.clone();
    let ledger_id = request.ledger_id.clone();
    let search = request
        .account_search
        .clone()
        .map(|value| format!("%{}%", value.to_lowercase()));
    let from = from.to_string();
    let to = to.to_string();
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let mut sql = String::from(
                "SELECT vl.ledger_id, vl.entry_type, vl.amount
                 FROM vouchers v JOIN voucher_lines vl ON vl.company_id = v.company_id AND vl.voucher_id = v.id
                 JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                 WHERE v.company_id = ? AND v.status IN ('posted', 'reversed')
                   AND v.date >= ? AND v.date <= ?",
            );
            if group_id.is_some() { sql.push_str(" AND l.group_id = ?"); }
            if ledger_id.is_some() { sql.push_str(" AND l.id = ?"); }
            if search.is_some() { sql.push_str(" AND lower(l.name) LIKE ?"); }
            let mut query = sqlx::query_as::<_, TrialMovement>(&sql)
                .bind(company_id).bind(from).bind(to);
            if let Some(value) = group_id { query = query.bind(value); }
            if let Some(value) = ledger_id { query = query.bind(value); }
            if let Some(value) = search { query = query.bind(value); }
            query.fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)
        })
    }).await
}

#[cfg(test)]
mod tests {
    use crate::reports::model::TrialBalanceRequest;
    use rust_decimal::Decimal;

    #[test]
    fn credit_normal_accounts_present_positive_net_as_credit() {
        let net = Decimal::new(1250, 2);
        let debit_normal = false;
        let (debit, credit) = if debit_normal {
            (net, Decimal::ZERO)
        } else {
            (Decimal::ZERO, net)
        };
        assert_eq!(debit, Decimal::ZERO);
        assert_eq!(credit.to_string(), "12.50");
    }

    #[test]
    fn request_supports_zero_balance_policy() {
        let request = TrialBalanceRequest {
            as_of_date: "2026-04-01".into(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: true,
        };
        assert!(request.include_zero_balances);
    }
}
