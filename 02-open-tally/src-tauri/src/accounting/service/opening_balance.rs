use super::ledger::LedgerService;
use crate::accounting::model::{
    BalanceType, OpeningBalance, OpeningBalanceEquilibrium, SetOpeningBalanceRequest,
};
use crate::accounting::validator::validate_opening_balance_amount;
use crate::company::CompanyService;
use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::SqlitePool;
use std::str::FromStr;
use uuid::Uuid;

pub struct OpeningBalanceService;

impl OpeningBalanceService {
    /// Sets, updates, or clears an opening balance for a ledger within an atomic ACID transaction
    /// recording append-only audit trail changes.
    pub async fn set_opening_balance(
        pool: &SqlitePool,
        company_id: &str,
        req: SetOpeningBalanceRequest,
    ) -> Result<Option<OpeningBalance>, OpenTallyError> {
        let company = CompanyService::get_company(pool, company_id).await?;
        let _ledger = LedgerService::get_ledger(pool, company_id, &req.ledger_id).await?;

        let dec =
            validate_opening_balance_amount(&req.amount, company.currency_decimal_places as u8)?;
        let fy_begins = req
            .financial_year_begins
            .as_deref()
            .unwrap_or(&company.financial_year_begins)
            .to_string();

        let mut tx = pool.begin().await?;

        // Query existing opening balance
        let existing: Option<OpeningBalance> = sqlx::query_as::<_, OpeningBalance>(
            "SELECT id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
             FROM opening_balances
             WHERE company_id = ? AND ledger_id = ? AND financial_year_begins = ?"
        )
        .bind(company_id)
        .bind(&req.ledger_id)
        .bind(&fy_begins)
        .fetch_optional(&mut *tx)
        .await?;

        let result = if dec.is_zero() {
            // Explicit Clearing action
            if let Some(ref prev) = existing {
                sqlx::query(
                    "DELETE FROM opening_balances WHERE company_id = ? AND ledger_id = ? AND financial_year_begins = ?"
                )
                .bind(company_id)
                .bind(&req.ledger_id)
                .bind(&fy_begins)
                .execute(&mut *tx)
                .await?;

                let audit_id = Uuid::new_v4().to_string();
                let old_val = json!({
                    "balance_type": prev.balance_type,
                    "amount": prev.amount
                })
                .to_string();

                sqlx::query(
                    "INSERT INTO audit_log (
                        id, company_id, entity_type, entity_id, ledger_id,
                        financial_year_begins, action, old_values, new_values, performed_by
                     ) VALUES (?, ?, 'opening_balance', ?, ?, ?, 'CLEAR', ?, NULL, 'desktop_user')",
                )
                .bind(&audit_id)
                .bind(company_id)
                .bind(&prev.id)
                .bind(&req.ledger_id)
                .bind(&fy_begins)
                .bind(&old_val)
                .execute(&mut *tx)
                .await?;
            }
            None
        } else {
            // Upsert action
            let (record_id, action, old_val) = match existing {
                Some(ref prev) => (
                    prev.id.clone(),
                    "UPDATE",
                    Some(
                        json!({ "balance_type": prev.balance_type, "amount": prev.amount })
                            .to_string(),
                    ),
                ),
                None => (Uuid::new_v4().to_string(), "CREATE", None),
            };

            let formatted_amount = format!(
                "{:.precision$}",
                dec,
                precision = company.currency_decimal_places as usize
            );
            let new_val = json!({
                "balance_type": req.balance_type.as_str(),
                "amount": formatted_amount
            })
            .to_string();

            sqlx::query(
                "INSERT INTO opening_balances (
                    id, company_id, ledger_id, financial_year_begins, balance_type, amount
                 ) VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT (company_id, ledger_id, financial_year_begins) DO UPDATE SET
                    balance_type = excluded.balance_type,
                    amount = excluded.amount,
                    updated_at = CURRENT_TIMESTAMP",
            )
            .bind(&record_id)
            .bind(company_id)
            .bind(&req.ledger_id)
            .bind(&fy_begins)
            .bind(req.balance_type.as_str())
            .bind(&formatted_amount)
            .execute(&mut *tx)
            .await?;

            let audit_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, ledger_id,
                    financial_year_begins, action, old_values, new_values, performed_by
                 ) VALUES (?, ?, 'opening_balance', ?, ?, ?, ?, ?, ?, 'desktop_user')",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(&record_id)
            .bind(&req.ledger_id)
            .bind(&fy_begins)
            .bind(action)
            .bind(old_val)
            .bind(Some(new_val))
            .execute(&mut *tx)
            .await?;

            let saved: OpeningBalance = sqlx::query_as::<_, OpeningBalance>(
                "SELECT id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
                 FROM opening_balances
                 WHERE id = ?"
            )
            .bind(&record_id)
            .fetch_one(&mut *tx)
            .await?;

            Some(saved)
        };

        tx.commit().await?;
        Ok(result)
    }

    /// Fetches opening balance for a specific ledger.
    pub async fn get_opening_balance(
        pool: &SqlitePool,
        company_id: &str,
        ledger_id: &str,
        fy_begins: Option<&str>,
    ) -> Result<Option<OpeningBalance>, OpenTallyError> {
        let company = CompanyService::get_company(pool, company_id).await?;
        let fy = fy_begins.unwrap_or(&company.financial_year_begins);

        let opb = sqlx::query_as::<_, OpeningBalance>(
            "SELECT id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
             FROM opening_balances
             WHERE company_id = ? AND ledger_id = ? AND financial_year_begins = ?"
        )
        .bind(company_id)
        .bind(ledger_id)
        .bind(fy)
        .fetch_optional(pool)
        .await?;

        Ok(opb)
    }

    /// Calculates opening balance equilibrium across all ledgers for a company financial year.
    pub async fn get_equilibrium(
        pool: &SqlitePool,
        company_id: &str,
        fy_begins: Option<&str>,
    ) -> Result<OpeningBalanceEquilibrium, OpenTallyError> {
        let company = CompanyService::get_company(pool, company_id).await?;
        let fy = fy_begins.unwrap_or(&company.financial_year_begins);

        let balances: Vec<(String, String)> = sqlx::query_as(
            "SELECT balance_type, amount FROM opening_balances WHERE company_id = ? AND financial_year_begins = ?"
        )
        .bind(company_id)
        .bind(fy)
        .fetch_all(pool)
        .await?;

        let mut total_debit = Decimal::ZERO;
        let mut total_credit = Decimal::ZERO;

        for (btype, amt_str) in balances {
            let amt = Decimal::from_str(&amt_str).unwrap_or(Decimal::ZERO);
            match btype.as_str() {
                "Dr" => total_debit += amt,
                "Cr" => total_credit += amt,
                _ => {}
            }
        }

        let precision = company.currency_decimal_places as usize;
        let diff = (total_debit - total_credit).abs();
        let is_balanced = total_debit == total_credit;
        let diff_type = if total_debit > total_credit {
            Some(BalanceType::Dr)
        } else if total_credit > total_debit {
            Some(BalanceType::Cr)
        } else {
            None
        };

        Ok(OpeningBalanceEquilibrium {
            total_debit: format!("{:.precision$}", total_debit, precision = precision),
            total_credit: format!("{:.precision$}", total_credit, precision = precision),
            difference: format!("{:.precision$}", diff, precision = precision),
            difference_type: diff_type,
            is_balanced,
        })
    }
}
