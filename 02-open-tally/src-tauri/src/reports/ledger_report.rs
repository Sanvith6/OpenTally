use crate::accounting::LedgerService;
use crate::errors::OpenTallyError;
use crate::reports::calculation::parse_report_amount;
use crate::reports::model::{
    LedgerReport, LedgerReportRequest, LedgerReportTransaction, ReportDateRange, ReportPage,
};
use crate::reports::query::ReportQueryService;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};

pub struct LedgerReportService;

impl LedgerReportService {
    pub async fn generate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: LedgerReportRequest,
    ) -> Result<LedgerReport, OpenTallyError> {
        request
            .page
            .validate()
            .map_err(OpenTallyError::Validation)?;
        let ledger_id = request
            .filter
            .ledger_id
            .clone()
            .ok_or_else(|| OpenTallyError::Validation("A ledger is required".to_string()))?;
        let ledger = LedgerService::get_ledger(pool, company_id, &ledger_id).await?;
        let nature: String =
            sqlx::query_scalar("SELECT nature FROM account_groups WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(&ledger.group_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;
        let financial_year_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        let opening = ReportQueryService::list_opening_balances(
            pool,
            company_id,
            &financial_year_begins,
            Some(&ledger_id),
        )
        .await?;
        let period_movements =
            ReportQueryService::list_movements(pool, company_id, &request.filter).await?;
        let prior_movements = if request.filter.period.date_from > financial_year_begins {
            let previous_day =
                chrono::NaiveDate::parse_from_str(&request.filter.period.date_from, "%Y-%m-%d")
                    .map_err(|_| OpenTallyError::Validation("Invalid report start date".into()))?
                    .pred_opt()
                    .ok_or_else(|| {
                        OpenTallyError::Validation("Report start date underflow".into())
                    })?
                    .format("%Y-%m-%d")
                    .to_string();
            let prior_filter = crate::reports::ReportFilter {
                period: ReportDateRange::new(&financial_year_begins, previous_day)
                    .map_err(OpenTallyError::Validation)?,
                ledger_id: Some(ledger_id.clone()),
            };
            ReportQueryService::list_movements(pool, company_id, &prior_filter).await?
        } else {
            Vec::new()
        };

        let debit_normal = matches!(nature.as_str(), "Assets" | "Expenses");
        let mut opening_signed = Decimal::ZERO;
        for item in &opening {
            let amount = parse_report_amount(&item.amount)?;
            opening_signed += if (item.balance_type == "Dr") == debit_normal {
                amount
            } else {
                -amount
            };
        }
        for movement in &prior_movements {
            opening_signed += signed_movement(movement, debit_normal)?;
        }

        let offset = (request.page.page - 1)
            .checked_mul(request.page.page_size)
            .ok_or_else(|| {
                OpenTallyError::Validation("Report pagination offset overflow".into())
            })?;
        let mut running = opening_signed;
        let mut transactions = Vec::with_capacity(period_movements.len());
        for movement in &period_movements {
            let signed = signed_movement(movement, debit_normal)?;
            running += signed;
            transactions.push(LedgerReportTransaction {
                voucher_id: movement.voucher_id.clone(),
                date: movement.voucher_date.clone(),
                voucher_number: movement.voucher_number.clone(),
                voucher_type: movement.voucher_type_name.clone(),
                particulars: movement.narration.clone(),
                debit: if movement.entry_type == "Dr" {
                    movement.amount.clone()
                } else {
                    "0".into()
                },
                credit: if movement.entry_type == "Cr" {
                    movement.amount.clone()
                } else {
                    "0".into()
                },
                running_balance: running.abs().to_string(),
                running_balance_type: display_type(running, debit_normal),
            });
        }
        let closing = running;
        let total = transactions.len();
        let page_items = transactions
            .into_iter()
            .skip(offset)
            .take(request.page.page_size)
            .collect::<Vec<_>>();

        Ok(LedgerReport {
            ledger_id,
            ledger_name: ledger.name,
            ledger_nature: nature,
            opening_balance: opening_signed.abs().to_string(),
            opening_balance_type: display_type(opening_signed, debit_normal),
            transactions: ReportPage {
                items: page_items,
                total,
                page: request.page.page,
                page_size: request.page.page_size,
                total_pages: total.div_ceil(request.page.page_size),
            },
            closing_balance: closing.abs().to_string(),
            closing_balance_type: display_type(closing, debit_normal),
        })
    }
}

fn signed_movement(
    movement: &crate::reports::ReportMovement,
    debit_normal: bool,
) -> Result<Decimal, OpenTallyError> {
    let amount = parse_report_amount(&movement.amount)?;
    match movement.entry_type.as_str() {
        "Dr" => Ok(if debit_normal { amount } else { -amount }),
        "Cr" => Ok(if debit_normal { -amount } else { amount }),
        other => Err(OpenTallyError::Validation(format!(
            "Invalid ledger entry type '{}'",
            other
        ))),
    }
}

fn display_type(value: Decimal, debit_normal: bool) -> String {
    if value >= Decimal::ZERO {
        if debit_normal {
            "Dr"
        } else {
            "Cr"
        }
    } else if debit_normal {
        "Cr"
    } else {
        "Dr"
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{display_type, signed_movement};
    use crate::reports::ReportMovement;
    use rust_decimal::Decimal;

    fn movement(entry_type: &str) -> ReportMovement {
        ReportMovement {
            voucher_id: "v".into(),
            voucher_number: "1".into(),
            voucher_date: "2026-04-01".into(),
            voucher_type_name: "Journal".into(),
            status: "posted".into(),
            ledger_id: "l".into(),
            ledger_name: "Ledger".into(),
            line_number: 1,
            entry_type: entry_type.into(),
            amount: "10.25".into(),
            narration: None,
        }
    }

    #[test]
    fn debit_normal_ledger_signs_debit_positive() {
        assert_eq!(
            signed_movement(&movement("Dr"), true).unwrap(),
            Decimal::new(1025, 2)
        );
        assert_eq!(display_type(Decimal::new(-1, 2), true), "Cr");
    }

    #[test]
    fn credit_normal_ledger_signs_credit_positive() {
        assert_eq!(
            signed_movement(&movement("Cr"), false).unwrap(),
            Decimal::new(1025, 2)
        );
        assert_eq!(display_type(Decimal::new(-1, 2), false), "Dr");
    }
}
