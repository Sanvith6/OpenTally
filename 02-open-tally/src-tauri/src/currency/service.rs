use crate::audit::AuditCertificationService;
use crate::billwise::{AllocationType, BillAllocationInput, BillWiseService};
use crate::company::closing::FinancialYearClosingService;
use crate::currency::models::*;
use crate::errors::OpenTallyError;
use crate::pdf::cheque::ChequeTemplateDto;
use crate::vouchers::model::{SaveVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::search::execute_read_only_query;
use crate::vouchers::service::{VoucherDraftService, VoucherPostingService};
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::str::FromStr;
use uuid::Uuid;

pub struct CurrencyService;

impl CurrencyService {
    /// Helper to record a cryptographic SHA-256 hash-chained entry in `audit_log`.
    pub async fn record_audit_entry_tx(
        tx: &mut SqliteConnection,
        company_id: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        new_values: Option<&str>,
        performed_by: &str,
    ) -> Result<String, OpenTallyError> {
        let id = format!("aud_{}", Uuid::new_v4().simple());
        let now = chrono::Utc::now().to_rfc3339();

        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let actual_prev = prev_hash.unwrap_or_else(|| "GENESIS".to_string());
        let entry_hash = AuditCertificationService::compute_entry_hash(
            &actual_prev,
            &id,
            company_id,
            entity_type,
            entity_id,
            action,
            &now,
            new_values,
        );

        sqlx::query(
            r#"
            INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, timestamp, performed_by, new_values, prev_hash, entry_hash)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(action)
        .bind(&now)
        .bind(performed_by)
        .bind(new_values)
        .bind(&actual_prev)
        .bind(&entry_hash)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(id)
    }

    /// Idempotently seed base currency (`INR`) + standard foreign currencies (`USD`, `EUR`, `GBP`, `AED`, `SGD`),
    /// the `"Forex Gain / Loss"` ledger under `Indirect Incomes`, and 4 CTS-2010 cheque templates.
    pub async fn seed_company_currency_defaults_tx(
        tx: &mut SqliteConnection,
        company_id: &str,
        actor: &str,
    ) -> Result<CurrencySeedSummary, OpenTallyError> {
        // 1. Verify Company exists
        let comp_row: Option<(String, String)> = sqlx::query_as(
            "SELECT COALESCE(currency_symbol, '₹'), COALESCE(currency_formal_name, 'Indian Rupee') FROM companies WHERE id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let (base_sym, base_formal) = comp_row.ok_or_else(|| {
            OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
        })?;

        // 2. Check if Base Currency is seeded
        let has_base: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM currencies WHERE company_id = ? AND is_base = 1",
        )
        .bind(company_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut base_seeded = false;
        if has_base == 0 {
            let base_id = format!("cur_{}_INR", company_id);
            sqlx::query(
                r#"
                INSERT INTO currencies (id, company_id, code, symbol, formal_name, decimal_places, is_base)
                VALUES (?, ?, 'INR', ?, ?, 2, 1)
                "#,
            )
            .bind(&base_id)
            .bind(company_id)
            .bind(&base_sym)
            .bind(&base_formal)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            Self::record_audit_entry_tx(
                tx,
                company_id,
                "currency",
                &base_id,
                "CREATE",
                Some("{\"code\":\"INR\",\"is_base\":true}"),
                actor,
            )
            .await?;
            base_seeded = true;
        }

        // 3. Seed common foreign currencies
        let common = [
            ("USD", "$", "US Dollar", 2),
            ("EUR", "€", "Euro", 2),
            ("GBP", "£", "British Pound", 2),
            ("AED", "AED", "UAE Dirham", 2),
            ("SGD", "S$", "Singapore Dollar", 2),
        ];

        let mut seeded_count = 0;
        for (code, sym, name, dec) in common {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM currencies WHERE company_id = ? AND code = ?",
            )
            .bind(company_id)
            .bind(code)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if exists == 0 {
                let cur_id = format!("cur_{}_{}", company_id, code);
                sqlx::query(
                    r#"
                    INSERT INTO currencies (id, company_id, code, symbol, formal_name, decimal_places, is_base)
                    VALUES (?, ?, ?, ?, ?, ?, 0)
                    "#,
                )
                .bind(&cur_id)
                .bind(company_id)
                .bind(code)
                .bind(sym)
                .bind(name)
                .bind(dec)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                Self::record_audit_entry_tx(
                    tx,
                    company_id,
                    "currency",
                    &cur_id,
                    "CREATE",
                    Some(&format!("{{\"code\":\"{}\",\"is_base\":false}}", code)),
                    actor,
                )
                .await?;
                seeded_count += 1;
            }
        }

        // 4. Seed "Forex Gain / Loss" ledger
        let existing_fx_ledger: Option<String> = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Forex Gain / Loss'",
        )
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let forex_ledger_id = match existing_fx_ledger {
            Some(id) => id,
            None => {
                // Find Indirect Incomes or Indirect Expenses
                let group_id: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT id FROM account_groups
                    WHERE company_id = ? AND name IN ('Indirect Incomes', 'Indirect Expenses')
                    ORDER BY CASE WHEN name = 'Indirect Incomes' THEN 1 ELSE 2 END
                    LIMIT 1
                    "#,
                )
                .bind(company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                let target_group = group_id.ok_or_else(|| {
                    OpenTallyError::Validation(
                        "Cannot seed Forex Gain / Loss ledger: neither 'Indirect Incomes' nor 'Indirect Expenses' group found".to_string(),
                    )
                })?;

                let fx_id = format!("led_fx_{}", company_id);
                sqlx::query(
                    r#"
                    INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
                    VALUES (?, ?, ?, 'Forex Gain / Loss', 0, 'standard')
                    "#,
                )
                .bind(&fx_id)
                .bind(company_id)
                .bind(&target_group)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                Self::record_audit_entry_tx(
                    tx,
                    company_id,
                    "ledger",
                    &fx_id,
                    "CREATE",
                    Some("{\"name\":\"Forex Gain / Loss\",\"group\":\"Indirect Incomes\"}"),
                    actor,
                )
                .await?;

                fx_id
            }
        };

        // 5. Seed 4 CTS-2010 cheque templates
        let templates = [
            (
                "HDFC Bank",
                202.0,
                92.0,
                148.0,
                9.0,
                22.0,
                25.0,
                26.0,
                36.0,
                152.0,
                42.0,
            ),
            (
                "State Bank of India",
                203.0,
                92.0,
                150.0,
                10.0,
                20.0,
                26.0,
                28.0,
                38.0,
                154.0,
                43.0,
            ),
            (
                "ICICI Bank",
                202.0,
                92.0,
                149.0,
                9.5,
                21.0,
                25.5,
                27.0,
                37.0,
                153.0,
                42.5,
            ),
            (
                "Axis Bank",
                202.0,
                92.0,
                147.0,
                9.5,
                23.0,
                25.0,
                27.0,
                36.5,
                151.0,
                42.0,
            ),
        ];

        let mut templates_seeded = 0;
        for (b_name, w, h, dx, dy, px, py, wx, wy, ax, ay) in templates {
            let t_exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM cheque_templates WHERE company_id = ? AND bank_name = ?",
            )
            .bind(company_id)
            .bind(b_name)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if t_exists == 0 {
                let t_id = format!("tmpl_{}_{}", company_id, b_name.replace(' ', "_"));
                sqlx::query(
                    r#"
                    INSERT INTO cheque_templates (id, company_id, bank_name, width_mm, height_mm, date_x_mm, date_y_mm, payee_x_mm, payee_y_mm, words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
                    "#,
                )
                .bind(&t_id)
                .bind(company_id)
                .bind(b_name)
                .bind(w)
                .bind(h)
                .bind(dx)
                .bind(dy)
                .bind(px)
                .bind(py)
                .bind(wx)
                .bind(wy)
                .bind(ax)
                .bind(ay)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                templates_seeded += 1;
            }
        }

        Ok(CurrencySeedSummary {
            base_currency_seeded: base_seeded,
            currencies_seeded_count: seeded_count,
            forex_ledger_id,
            cheque_templates_seeded_count: templates_seeded,
        })
    }

    /// List all currencies configured for a company.
    pub async fn list_currencies(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<Currency>, OpenTallyError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            r#"
            SELECT id, company_id, code, symbol, formal_name, decimal_places, is_base, created_at
            FROM currencies
            WHERE company_id = ?
            ORDER BY is_base DESC, code ASC
            "#,
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows
            .into_iter()
            .map(|(id, cid, code, sym, name, dec, base, cat)| Currency {
                id,
                company_id: cid,
                code,
                symbol: sym,
                formal_name: name,
                decimal_places: dec,
                is_base: base != 0,
                created_at: cat,
            })
            .collect())
    }

    /// Create a new currency.
    pub async fn create_currency(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreateCurrencyInput,
        actor: &str,
    ) -> Result<Currency, OpenTallyError> {
        let code = input.code.trim().to_uppercase();
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(OpenTallyError::Validation(
                "Currency code must be a 3-letter uppercase ISO 4217 code (e.g. USD, EUR)"
                    .to_string(),
            ));
        }

        let symbol = input.symbol.trim();
        if symbol.is_empty() || symbol.len() > 12 {
            return Err(OpenTallyError::Validation(
                "Currency symbol must be 1 to 12 characters".to_string(),
            ));
        }

        let formal = input.formal_name.trim();
        if formal.is_empty() || formal.len() > 100 {
            return Err(OpenTallyError::Validation(
                "Currency formal name must be 1 to 100 characters".to_string(),
            ));
        }

        let dec = input.decimal_places.unwrap_or(2);
        if !(0..=4).contains(&dec) {
            return Err(OpenTallyError::Validation(
                "Currency decimal places must be between 0 and 4".to_string(),
            ));
        }

        let is_base = input.is_base.unwrap_or(false);

        let mut tx = pool.begin().await?;

        if is_base {
            let existing_base: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM currencies WHERE company_id = ? AND is_base = 1",
            )
            .bind(company_id)
            .fetch_one(&mut *tx)
            .await?;

            if existing_base > 0 {
                return Err(OpenTallyError::Validation(
                    "Company already has a base currency configured. Only one base currency is permitted.".to_string(),
                ));
            }
        }

        let cur_id = format!("cur_{}_{}", company_id, code);
        sqlx::query(
            r#"
            INSERT INTO currencies (id, company_id, code, symbol, formal_name, decimal_places, is_base)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&cur_id)
        .bind(company_id)
        .bind(&code)
        .bind(symbol)
        .bind(formal)
        .bind(dec)
        .bind(if is_base { 1 } else { 0 })
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::record_audit_entry_tx(
            &mut tx,
            company_id,
            "currency",
            &cur_id,
            "CREATE",
            Some(&format!(
                "{{\"code\":\"{}\",\"symbol\":\"{}\",\"is_base\":{}}}",
                code, symbol, is_base
            )),
            actor,
        )
        .await?;

        tx.commit().await?;

        Ok(Currency {
            id: cur_id,
            company_id: company_id.to_string(),
            code,
            symbol: symbol.to_string(),
            formal_name: formal.to_string(),
            decimal_places: dec,
            is_base,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Sets or updates a daily exchange rate for a currency.
    pub async fn set_exchange_rate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: SetExchangeRateInput,
        actor: &str,
    ) -> Result<ExchangeRate, OpenTallyError> {
        let code = input.currency_code.trim().to_uppercase();
        let eff_date = input.effective_date.trim();

        if eff_date.len() != 10
            || !eff_date.chars().enumerate().all(|(i, c)| match i {
                4 | 7 => c == '-',
                _ => c.is_ascii_digit(),
            })
        {
            return Err(OpenTallyError::Validation(
                "Effective date must be in YYYY-MM-DD format".to_string(),
            ));
        }

        let std_rate_dec = Decimal::from_str_exact(input.standard_rate.trim()).map_err(|_| {
            OpenTallyError::Validation(
                "Standard exchange rate must be a valid positive number".to_string(),
            )
        })?;
        if std_rate_dec <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Standard exchange rate must be strictly greater than zero".to_string(),
            ));
        }

        let std_rate_str = std_rate_dec.to_string();
        let sell_rate_str = input
            .selling_rate
            .map(|r| r.trim().to_string())
            .unwrap_or_else(|| std_rate_str.clone());
        let buy_rate_str = input
            .buying_rate
            .map(|r| r.trim().to_string())
            .unwrap_or_else(|| std_rate_str.clone());

        let mut tx = pool.begin().await?;

        // Verify currency exists and is NOT the base currency
        let cur_row: Option<(i64,)> =
            sqlx::query_as("SELECT is_base FROM currencies WHERE company_id = ? AND code = ?")
                .bind(company_id)
                .bind(&code)
                .fetch_optional(&mut *tx)
                .await?;

        let (is_base,) = cur_row.ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Currency '{}' not found for company '{}'",
                code, company_id
            ))
        })?;

        if is_base == 1 {
            return Err(OpenTallyError::Validation(
                "Cannot set exchange rate for the base currency (rate is always 1)".to_string(),
            ));
        }

        let rate_id = format!("rate_{}_{}_{}", company_id, code, eff_date);

        sqlx::query(
            r#"
            INSERT INTO exchange_rates (id, company_id, currency_code, effective_date, standard_rate, selling_rate, buying_rate)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (company_id, currency_code, effective_date) DO UPDATE SET
                standard_rate = excluded.standard_rate,
                selling_rate = excluded.selling_rate,
                buying_rate = excluded.buying_rate
            "#,
        )
        .bind(&rate_id)
        .bind(company_id)
        .bind(&code)
        .bind(eff_date)
        .bind(&std_rate_str)
        .bind(&sell_rate_str)
        .bind(&buy_rate_str)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::record_audit_entry_tx(
            &mut tx,
            company_id,
            "exchange_rate",
            &rate_id,
            "UPDATE",
            Some(&format!(
                "{{\"currency\":\"{}\",\"date\":\"{}\",\"rate\":\"{}\"}}",
                code, eff_date, std_rate_str
            )),
            actor,
        )
        .await?;

        tx.commit().await?;

        Ok(ExchangeRate {
            id: rate_id,
            company_id: company_id.to_string(),
            currency_code: code,
            effective_date: eff_date.to_string(),
            standard_rate: std_rate_str,
            selling_rate: sell_rate_str,
            buying_rate: buy_rate_str,
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// List exchange rates for a company, optionally filtered by currency code.
    pub async fn list_exchange_rates(
        pool: &Pool<Sqlite>,
        company_id: &str,
        currency_code: Option<&str>,
    ) -> Result<Vec<ExchangeRate>, OpenTallyError> {
        let rows = if let Some(code) = currency_code {
            sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
                r#"
                SELECT id, company_id, currency_code, effective_date, standard_rate, selling_rate, buying_rate, created_at
                FROM exchange_rates
                WHERE company_id = ? AND currency_code = ?
                ORDER BY effective_date DESC
                "#,
            )
            .bind(company_id)
            .bind(code)
            .fetch_all(pool)
            .await
        } else {
            sqlx::query_as::<_, (String, String, String, String, String, String, String, String)>(
                r#"
                SELECT id, company_id, currency_code, effective_date, standard_rate, selling_rate, buying_rate, created_at
                FROM exchange_rates
                WHERE company_id = ?
                ORDER BY currency_code ASC, effective_date DESC
                "#,
            )
            .bind(company_id)
            .fetch_all(pool)
            .await
        }
        .map_err(OpenTallyError::Database)?;

        Ok(rows
            .into_iter()
            .map(|(id, cid, code, ed, std, sell, buy, cat)| ExchangeRate {
                id,
                company_id: cid,
                currency_code: code,
                effective_date: ed,
                standard_rate: std,
                selling_rate: sell,
                buying_rate: buy,
                created_at: cat,
            })
            .collect())
    }

    /// Deterministic exchange rate lookup: returns the most recent exchange rate on or before `as_of_date`.
    pub async fn get_effective_rate_tx(
        conn: &mut SqliteConnection,
        company_id: &str,
        currency_code: &str,
        as_of_date: &str,
    ) -> Result<EffectiveRateDto, OpenTallyError> {
        let code = currency_code.trim().to_uppercase();

        // 1. Check if currency is base
        let is_base: Option<i64> =
            sqlx::query_scalar("SELECT is_base FROM currencies WHERE company_id = ? AND code = ?")
                .bind(company_id)
                .bind(&code)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

        let is_base_val = is_base.ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Currency '{}' not found for company '{}'",
                code, company_id
            ))
        })?;

        if is_base_val == 1 {
            return Ok(EffectiveRateDto {
                currency_code: code,
                effective_date: as_of_date.to_string(),
                standard_rate: Decimal::ONE,
                selling_rate: Decimal::ONE,
                buying_rate: Decimal::ONE,
                is_base: true,
            });
        }

        // 2. Query most recent rate on or before as_of_date
        let row: Option<(String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT effective_date, standard_rate, selling_rate, buying_rate
            FROM exchange_rates
            WHERE company_id = ? AND currency_code = ? AND effective_date <= ?
            ORDER BY effective_date DESC
            LIMIT 1
            "#,
        )
        .bind(company_id)
        .bind(&code)
        .bind(as_of_date)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let (ed, std_str, sell_str, buy_str) = row.ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "No exchange rate configured for currency '{}' on or before '{}'",
                code, as_of_date
            ))
        })?;

        let std_rate = Decimal::from_str_exact(&std_str).unwrap_or(Decimal::ONE);
        let sell_rate = Decimal::from_str_exact(&sell_str).unwrap_or(std_rate);
        let buy_rate = Decimal::from_str_exact(&buy_str).unwrap_or(std_rate);

        Ok(EffectiveRateDto {
            currency_code: code,
            effective_date: ed,
            standard_rate: std_rate,
            selling_rate: sell_rate,
            buying_rate: buy_rate,
            is_base: false,
        })
    }

    /// Lookup exchange rate via pool.
    pub async fn lookup_exchange_rate(
        pool: &Pool<Sqlite>,
        company_id: &str,
        currency_code: &str,
        as_of_date: &str,
    ) -> Result<EffectiveRateDto, OpenTallyError> {
        let mut conn = pool.acquire().await?;
        Self::get_effective_rate_tx(&mut conn, company_id, currency_code, as_of_date).await
    }

    /// Converts a foreign amount to base currency using deterministic exchange rate lookup.
    /// Uses exact `round_dp(2)` with zero floating-point arithmetic.
    pub async fn convert_currency_amount(
        pool: &Pool<Sqlite>,
        company_id: &str,
        currency_code: &str,
        foreign_amount: Decimal,
        as_of_date: &str,
    ) -> Result<Decimal, OpenTallyError> {
        let rate = Self::lookup_exchange_rate(pool, company_id, currency_code, as_of_date).await?;
        let base_amount = (foreign_amount * rate.standard_rate).round_dp(2);
        Ok(base_amount)
    }

    /// Lists open foreign currency bills (unsettled or partially settled `NewRef` bills).
    pub async fn list_open_foreign_bills(
        pool: &Pool<Sqlite>,
        company_id: &str,
        party_ledger_id: Option<&str>,
    ) -> Result<Vec<OpenForeignBill>, OpenTallyError> {
        let mut conn = pool.acquire().await?;

        // 1. Fetch all NewRef bills that have voucher_currency_details
        let mut sql = r#"
            SELECT ba.bill_name, ba.party_ledger_id, l.name as party_name,
                   ba.voucher_id, v.voucher_number, ba.bill_date, ba.due_date,
                   vcd.currency_code, vcd.exchange_rate, vcd.foreign_amount, ba.amount as original_inr, ba.dr_cr
            FROM bill_allocations ba
            JOIN vouchers v ON v.company_id = ba.company_id AND v.id = ba.voucher_id
            JOIN ledgers l ON l.company_id = ba.company_id AND l.id = ba.party_ledger_id
            JOIN voucher_currency_details vcd ON vcd.company_id = ba.company_id AND vcd.voucher_id = ba.voucher_id
            WHERE ba.company_id = ?
              AND ba.allocation_type = 'NewRef'
              AND ba.is_cancelled = 0
              AND v.status != 'cancelled'
        "#.to_string();

        if party_ledger_id.is_some() {
            sql.push_str(" AND ba.party_ledger_id = ?");
        }

        sql.push_str(" ORDER BY ba.bill_date ASC, ba.bill_name ASC");

        let mut q = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                String,
                String,
                String,
                String,
                String,
            ),
        >(&sql)
        .bind(company_id);

        if let Some(pid) = party_ledger_id {
            q = q.bind(pid);
        }

        let new_ref_rows = q
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let mut open_bills = Vec::new();

        for (
            b_name,
            p_id,
            p_name,
            v_id,
            v_num,
            b_date,
            due_d,
            curr,
            rate_str,
            orig_foreign_str,
            orig_inr_str,
            dr_cr,
        ) in new_ref_rows
        {
            let orig_foreign = Decimal::from_str_exact(&orig_foreign_str).unwrap_or(Decimal::ZERO);
            let orig_inr = Decimal::from_str_exact(&orig_inr_str).unwrap_or(Decimal::ZERO);
            let inv_rate = Decimal::from_str_exact(&rate_str).unwrap_or(Decimal::ONE);

            // Fetch settlements on this bill (both foreign from vcd and INR from bill_allocations)
            let settled_inr_str: Option<String> = sqlx::query_scalar(
                r#"
                SELECT COALESCE(CAST(SUM(CAST(ba.amount AS NUMERIC)) AS TEXT), '0.00')
                FROM bill_allocations ba
                JOIN vouchers v ON v.company_id = ba.company_id AND v.id = ba.voucher_id
                WHERE ba.company_id = ?
                  AND ba.party_ledger_id = ?
                  AND ba.bill_name = ?
                  AND ba.allocation_type = 'AgstRef'
                  AND ba.is_cancelled = 0
                  AND v.status != 'cancelled'
                "#,
            )
            .bind(company_id)
            .bind(&p_id)
            .bind(&b_name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let settled_inr = settled_inr_str
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(Decimal::ZERO);

            // Fetch settled foreign amount from voucher_currency_details on settlements
            let settled_foreign_str: Option<String> = sqlx::query_scalar(
                r#"
                SELECT COALESCE(CAST(SUM(CAST(vcd.foreign_amount AS NUMERIC)) AS TEXT), '0.00')
                FROM voucher_currency_details vcd
                JOIN vouchers v ON v.company_id = vcd.company_id AND v.id = vcd.voucher_id
                JOIN bill_allocations ba ON ba.company_id = vcd.company_id AND ba.voucher_id = vcd.voucher_id
                WHERE vcd.company_id = ?
                  AND ba.party_ledger_id = ?
                  AND ba.bill_name = ?
                  AND ba.allocation_type = 'AgstRef'
                  AND ba.is_cancelled = 0
                  AND v.status != 'cancelled'
                "#,
            )
            .bind(company_id)
            .bind(&p_id)
            .bind(&b_name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let settled_foreign = settled_foreign_str
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(Decimal::ZERO);

            let outstanding_foreign = (orig_foreign - settled_foreign).max(Decimal::ZERO);
            let outstanding_inr = (orig_inr - settled_inr).max(Decimal::ZERO);

            if outstanding_foreign > Decimal::ZERO {
                open_bills.push(OpenForeignBill {
                    bill_name: b_name,
                    party_ledger_id: p_id,
                    party_name: p_name,
                    voucher_id: v_id,
                    voucher_number: v_num,
                    bill_date: b_date,
                    due_date: due_d,
                    currency_code: curr,
                    invoice_rate: inv_rate,
                    original_foreign: orig_foreign,
                    settled_foreign,
                    outstanding_foreign,
                    original_inr: orig_inr,
                    settled_inr,
                    outstanding_inr,
                    dr_cr,
                });
            }
        }

        Ok(open_bills)
    }

    /// Settles an open foreign currency bill with automatic Realized Forex Gain / Loss calculation & posting.
    ///
    /// Double-Entry Invariants:
    /// - 1. Bank/Cash payment/receipt records exact settlement base amount (`foreign_settled * settlement_rate`).
    /// - 2. Party ledger balance is settled at book value (`foreign_settled * invoice_rate`).
    /// - 3. If `settlement_rate != invoice_rate`, balanced Forex Adjustment Journal (`vt_jnl`) is posted
    ///      in the same atomic transaction with MCA SHA-256 hash-chained audit log.
    /// - 4. Trial Balance stays 100% in equilibrium (`difference == 0`).
    pub async fn settle_foreign_bill_with_forex(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: SettleForeignBillInput,
        actor: &str,
    ) -> Result<RealizedForexResult, OpenTallyError> {
        let foreign_settled = Decimal::from_str_exact(input.foreign_settled_amount.trim())
            .map_err(|_| {
                OpenTallyError::Validation(
                    "Foreign settled amount must be a valid positive number".to_string(),
                )
            })?;
        if foreign_settled <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Foreign settled amount must be strictly greater than zero".to_string(),
            ));
        }

        // 1. Period Lock Guard
        FinancialYearClosingService::validate_period_not_locked(
            pool,
            company_id,
            &input.settlement_date,
        )
        .await?;

        // 2. Find target open bill
        let open_bills =
            Self::list_open_foreign_bills(pool, company_id, Some(&input.party_ledger_id)).await?;
        let bill = open_bills
            .into_iter()
            .find(|b| b.bill_name == input.bill_name)
            .ok_or_else(|| {
                OpenTallyError::NotFound(format!(
                    "Open foreign bill '{}' not found for party '{}'",
                    input.bill_name, input.party_ledger_id
                ))
            })?;

        if foreign_settled > bill.outstanding_foreign {
            return Err(OpenTallyError::Validation(format!(
                "Foreign settlement amount ({}) exceeds outstanding bill amount ({})",
                foreign_settled, bill.outstanding_foreign
            )));
        }

        // 3. Resolve settlement rate
        let settlement_rate = if let Some(r_str) = input.settlement_exchange_rate {
            Decimal::from_str_exact(r_str.trim()).map_err(|_| {
                OpenTallyError::Validation("Invalid settlement exchange rate".to_string())
            })?
        } else {
            let eff = Self::lookup_exchange_rate(
                pool,
                company_id,
                &bill.currency_code,
                &input.settlement_date,
            )
            .await?;
            eff.standard_rate
        };

        if settlement_rate <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Settlement exchange rate must be strictly greater than zero".to_string(),
            ));
        }

        let invoice_rate = bill.invoice_rate;
        let is_full_settlement = foreign_settled == bill.outstanding_foreign;

        // Book INR settled: on final tranche use exact remaining INR to prevent 1-paisa rounding residual
        let book_inr = if is_full_settlement {
            bill.outstanding_inr
        } else {
            (foreign_settled * invoice_rate).round_dp(2)
        };

        let settled_inr = (foreign_settled * settlement_rate).round_dp(2);
        let forex_diff = (settled_inr - book_inr).round_dp(2);

        // Resolve "Forex Gain / Loss" ledger
        let fx_ledger_id: String = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Forex Gain / Loss'",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::Validation(
                "Forex Gain / Loss ledger is not configured. Please enable multi-currency in F11."
                    .to_string(),
            )
        })?;

        // 4. Create Settlement Voucher:
        // If bill was Dr (Receivable / Sales), settlement is F6 Receipt (Dr Bank, Cr Party)
        // If bill was Cr (Payable / Purchase), settlement is F5 Payment (Dr Party, Cr Bank)
        let is_receivable = bill.dr_cr == "Dr";
        let vch_type_id = if is_receivable {
            "vt_receipt"
        } else {
            "vt_payment"
        };

        let series_row: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = ? AND is_active = 1 ORDER BY created_at ASC LIMIT 1",
        )
        .bind(company_id)
        .bind(vch_type_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let series_id = series_row
            .map(|(s,)| s)
            .unwrap_or_else(|| format!("{}_std", vch_type_id));

        let party_line_entry = if is_receivable { "Cr" } else { "Dr" };
        let bank_line_entry = if is_receivable { "Dr" } else { "Cr" };

        // Post settlement voucher via draft + posting
        let settlement_draft = VoucherDraftService::save_draft(
            pool,
            company_id,
            SaveVoucherDraftRequest {
                voucher_id: None,
                voucher_type_id: vch_type_id.to_string(),
                series_id: series_id.clone(),
                date: input.settlement_date.clone(),
                effective_date: None,
                reference_no: Some(format!("FX-STMT:{}", bill.bill_name)),
                reference_date: None,
                narration: input.narration.clone().or_else(|| {
                    Some(format!(
                        "Foreign currency settlement for bill {} ({} {} @ {})",
                        bill.bill_name, bill.currency_code, foreign_settled, settlement_rate
                    ))
                }),
                lines: vec![
                    VoucherLineInput {
                        id: None,
                        ledger_id: input.bank_or_cash_ledger_id.clone(),
                        entry_type: bank_line_entry.to_string(),
                        amount: settled_inr.to_string(),
                        narration: None,
                    },
                    VoucherLineInput {
                        id: None,
                        ledger_id: bill.party_ledger_id.clone(),
                        entry_type: party_line_entry.to_string(),
                        amount: settled_inr.to_string(),
                        narration: Some(format!("AgstRef {}", bill.bill_name)),
                    },
                ],
            },
        )
        .await?;

        let posted_settlement = VoucherPostingService::post_voucher(
            pool,
            company_id,
            &settlement_draft.voucher.id,
            Some(actor),
        )
        .await?;

        // Locate Party line on the posted voucher
        let party_line = posted_settlement
            .lines
            .iter()
            .find(|l| l.ledger_id == bill.party_ledger_id)
            .ok_or_else(|| {
                OpenTallyError::Internal("Party line missing on settlement voucher".to_string())
            })?;

        // Atomic transaction for currency detail and bill allocations
        let mut tx = pool.begin().await?;

        // Record voucher_currency_details on settlement
        let vcd_id = format!("vcd_{}", Uuid::new_v4().simple());
        sqlx::query(
            r#"
            INSERT INTO voucher_currency_details (id, company_id, voucher_id, voucher_line_id, currency_code, foreign_amount, exchange_rate, base_amount)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&vcd_id)
        .bind(company_id)
        .bind(&posted_settlement.voucher.id)
        .bind(&party_line.id)
        .bind(&bill.currency_code)
        .bind(foreign_settled.to_string())
        .bind(settlement_rate.to_string())
        .bind(settled_inr.to_string())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Attach Bill Allocation AgstRef (cap at book_inr so remaining bill INR doesn't over-settle)
        let agst_alloc_amount = if is_receivable {
            book_inr.min(settled_inr)
        } else {
            settled_inr.min(book_inr)
        };

        let mut allocs = vec![BillAllocationInput {
            bill_name: bill.bill_name.clone(),
            allocation_type: AllocationType::AgstRef,
            amount: agst_alloc_amount.to_string(),
            dr_cr: Some(party_line_entry.to_string()),
            due_date: None,
        }];

        if settled_inr > agst_alloc_amount {
            let diff = (settled_inr - agst_alloc_amount).round_dp(2);
            allocs.push(BillAllocationInput {
                bill_name: format!("{}-FX", bill.bill_name),
                allocation_type: AllocationType::OnAccount,
                amount: diff.to_string(),
                dr_cr: Some(party_line_entry.to_string()),
                due_date: None,
            });
        }

        BillWiseService::validate_and_post_allocations(
            &mut tx,
            company_id,
            &posted_settlement.voucher.id,
            &party_line.id,
            &bill.party_ledger_id,
            party_line_entry,
            settled_inr,
            &input.settlement_date,
            None,
            &allocs,
        )
        .await?;

        // 5. If Bank Cheque details provided, insert bank_allocation
        if let Some(inst_no) = input.cheque_instrument_number {
            let bank_line = posted_settlement
                .lines
                .iter()
                .find(|l| l.ledger_id == input.bank_or_cash_ledger_id)
                .ok_or_else(|| {
                    OpenTallyError::Internal("Bank line missing on settlement voucher".to_string())
                })?;

            let b_alloc_id = format!("ba_{}", Uuid::new_v4().simple());
            sqlx::query(
                r#"
                INSERT INTO bank_allocations (id, company_id, voucher_id, voucher_line_id, bank_ledger_id, transaction_type, instrument_number, instrument_date, favouring_name, amount)
                VALUES (?, ?, ?, ?, ?, 'Cheque', ?, ?, ?, ?)
                "#,
            )
            .bind(&b_alloc_id)
            .bind(company_id)
            .bind(&posted_settlement.voucher.id)
            .bind(&bank_line.id)
            .bind(&input.bank_or_cash_ledger_id)
            .bind(inst_no)
            .bind(&input.settlement_date)
            .bind(&bill.party_name)
            .bind(settled_inr.to_string())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        tx.commit().await?;

        // 6. Generate Balanced Realized Forex Adjustment Journal if forex_diff != 0
        let mut jnl_voucher_id = None;
        let abs_diff = forex_diff.abs();

        if abs_diff > Decimal::ZERO {
            let jnl_series_row: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = 'vt_jnl' AND is_active = 1 ORDER BY created_at ASC LIMIT 1",
            )
            .bind(company_id)
            .fetch_optional(pool)
            .await
            .map_err(OpenTallyError::Database)?;

            let jnl_series_id = jnl_series_row
                .map(|(s,)| s)
                .unwrap_or_else(|| "vt_jnl_std".to_string());

            let (party_entry, fx_entry) = if is_receivable {
                if forex_diff > Decimal::ZERO {
                    ("Dr", "Cr")
                } else {
                    ("Cr", "Dr")
                }
            } else if forex_diff < Decimal::ZERO {
                ("Dr", "Cr")
            } else {
                ("Cr", "Dr")
            };

            let jnl_draft = VoucherDraftService::save_draft(
                pool,
                company_id,
                SaveVoucherDraftRequest {
                    voucher_id: None,
                    voucher_type_id: "vt_jnl".to_string(),
                    series_id: jnl_series_id,
                    date: input.settlement_date.clone(),
                    effective_date: None,
                    reference_no: Some(format!("FX-SETTLE:{}", posted_settlement.voucher.id)),
                    reference_date: None,
                    narration: Some(format!(
                        "Realized Forex {} on settlement of bill {} ({} {} @ {} vs invoice @ {})",
                        if (is_receivable && forex_diff > Decimal::ZERO)
                            || (!is_receivable && forex_diff < Decimal::ZERO)
                        {
                            "Gain"
                        } else {
                            "Loss"
                        },
                        bill.bill_name,
                        bill.currency_code,
                        foreign_settled,
                        settlement_rate,
                        invoice_rate
                    )),
                    lines: vec![
                        VoucherLineInput {
                            id: None,
                            ledger_id: bill.party_ledger_id.clone(),
                            entry_type: party_entry.to_string(),
                            amount: abs_diff.to_string(),
                            narration: Some(format!(
                                "Forex adjustment for bill {}",
                                bill.bill_name
                            )),
                        },
                        VoucherLineInput {
                            id: None,
                            ledger_id: fx_ledger_id.clone(),
                            entry_type: fx_entry.to_string(),
                            amount: abs_diff.to_string(),
                            narration: Some(format!("Forex gain/loss on bill {}", bill.bill_name)),
                        },
                    ],
                },
            )
            .await?;

            let posted_jnl = VoucherPostingService::post_voucher(
                pool,
                company_id,
                &jnl_draft.voucher.id,
                Some(actor),
            )
            .await?;

            // If the forex adjustment needs to close out the remaining AgstRef bill balance:
            // For Receivable Loss or Payable Gain, the party line reduces the remaining bill!
            if (is_receivable && forex_diff < Decimal::ZERO)
                || (!is_receivable && forex_diff < Decimal::ZERO)
            {
                let jnl_party_line = posted_jnl
                    .lines
                    .iter()
                    .find(|l| l.ledger_id == bill.party_ledger_id)
                    .ok_or_else(|| {
                        OpenTallyError::Internal("Party line missing on forex journal".to_string())
                    })?;

                let mut tx2 = pool.begin().await?;
                BillWiseService::validate_and_post_allocations(
                    &mut tx2,
                    company_id,
                    &posted_jnl.voucher.id,
                    &jnl_party_line.id,
                    &bill.party_ledger_id,
                    party_entry,
                    abs_diff,
                    &input.settlement_date,
                    None,
                    &[BillAllocationInput {
                        bill_name: bill.bill_name.clone(),
                        allocation_type: AllocationType::AgstRef,
                        amount: abs_diff.to_string(),
                        dr_cr: Some(party_entry.to_string()),
                        due_date: None,
                    }],
                )
                .await?;
                tx2.commit().await?;
            }

            jnl_voucher_id = Some(posted_jnl.voucher.id);
        }

        let is_gain = if is_receivable {
            forex_diff > Decimal::ZERO
        } else {
            forex_diff < Decimal::ZERO
        };

        let mut tx_audit = pool.begin().await?;
        Self::record_audit_entry_tx(
            &mut tx_audit,
            company_id,
            "forex_settlement",
            &posted_settlement.voucher.id,
            "POST",
            Some(&format!(
                "{{\"bill\":\"{}\",\"currency\":\"{}\",\"foreign\":\"{}\",\"rate\":\"{}\",\"diff\":\"{}\",\"is_gain\":{}}}",
                bill.bill_name, bill.currency_code, foreign_settled, settlement_rate, forex_diff, is_gain
            )),
            actor,
        )
        .await?;
        tx_audit.commit().await?;

        Ok(RealizedForexResult {
            settlement_voucher_id: posted_settlement.voucher.id,
            forex_journal_voucher_id: jnl_voucher_id,
            foreign_settled,
            settlement_rate,
            invoice_rate,
            forex_difference: forex_diff,
            is_gain,
            settled_inr,
            book_inr,
        })
    }

    /// Evaluates Unrealized Forex Revaluation for open foreign currency bills as of `as_of_date`.
    ///
    /// Executed strictly inside `ReadOnlyConnectionGuard` to guarantee zero state mutation.
    pub async fn evaluate_unrealized_forex_revaluation(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<UnrealizedForexReport, OpenTallyError> {
        let cid = company_id.to_string();
        let aod = as_of_date.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let aod = aod.clone();
            Box::pin(async move {
                // 1. Fetch all active NewRef bills on or before as_of_date with voucher_currency_details
                let new_ref_rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
                    r#"
                    SELECT ba.bill_name, ba.party_ledger_id, l.name as party_name,
                           ba.dr_cr, vcd.currency_code, vcd.exchange_rate, vcd.foreign_amount
                    FROM bill_allocations ba
                    JOIN vouchers v ON v.company_id = ba.company_id AND v.id = ba.voucher_id
                    JOIN ledgers l ON l.company_id = ba.company_id AND l.id = ba.party_ledger_id
                    JOIN voucher_currency_details vcd ON vcd.company_id = ba.company_id AND vcd.voucher_id = ba.voucher_id
                    WHERE ba.company_id = ?
                      AND ba.allocation_type = 'NewRef'
                      AND ba.is_cancelled = 0
                      AND v.status != 'cancelled'
                      AND v.date <= ?
                    ORDER BY ba.bill_name ASC
                    "#,
                )
                .bind(&cid)
                .bind(&aod)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut bill_rows = Vec::new();
                let mut total_book = Decimal::ZERO;
                let mut total_revalued = Decimal::ZERO;
                let mut total_gain = Decimal::ZERO;
                let mut total_loss = Decimal::ZERO;

                for (b_name, p_id, p_name, dr_cr, curr, inv_rate_str, orig_foreign_str) in new_ref_rows {
                    let orig_foreign = Decimal::from_str_exact(&orig_foreign_str).unwrap_or(Decimal::ZERO);
                    let inv_rate = Decimal::from_str_exact(&inv_rate_str).unwrap_or(Decimal::ONE);

                    // Fetch settled foreign amount on or before as_of_date
                    let settled_foreign_str: Option<String> = sqlx::query_scalar(
                        r#"
                        SELECT COALESCE(CAST(SUM(CAST(vcd.foreign_amount AS NUMERIC)) AS TEXT), '0.00')
                        FROM voucher_currency_details vcd
                        JOIN vouchers v ON v.company_id = vcd.company_id AND v.id = vcd.voucher_id
                        JOIN bill_allocations ba ON ba.company_id = vcd.company_id AND ba.voucher_id = vcd.voucher_id
                        WHERE vcd.company_id = ?
                          AND ba.party_ledger_id = ?
                          AND ba.bill_name = ?
                          AND ba.allocation_type = 'AgstRef'
                          AND ba.is_cancelled = 0
                          AND v.status != 'cancelled'
                          AND v.date <= ?
                        "#,
                    )
                    .bind(&cid)
                    .bind(&p_id)
                    .bind(&b_name)
                    .bind(&aod)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    let settled_foreign = settled_foreign_str
                        .and_then(|s| Decimal::from_str(&s).ok())
                        .unwrap_or(Decimal::ZERO);

                    let open_foreign = (orig_foreign - settled_foreign).max(Decimal::ZERO);
                    if open_foreign <= Decimal::ZERO {
                        continue;
                    }

                    // Lookup effective exchange rate on as_of_date
                    let reval_rate = match Self::get_effective_rate_tx(&mut *conn, &cid, &curr, &aod).await {
                        Ok(eff) => eff.standard_rate,
                        Err(_) => inv_rate, // fallback to invoice rate if no later rate found
                    };

                    let book_val = (open_foreign * inv_rate).round_dp(2);
                    let reval_val = (open_foreign * reval_rate).round_dp(2);

                    let is_receivable = dr_cr == "Dr";
                    let diff = if is_receivable {
                        reval_val - book_val
                    } else {
                        book_val - reval_val
                    };

                    let is_gain = diff >= Decimal::ZERO;
                    if is_gain {
                        total_gain += diff;
                    } else {
                        total_loss += diff.abs();
                    }

                    total_book += book_val;
                    total_revalued += reval_val;

                    bill_rows.push(UnrealizedForexBillRow {
                        bill_name: b_name,
                        party_ledger_id: p_id,
                        party_name: p_name,
                        currency_code: curr,
                        dr_cr,
                        open_foreign_amount: open_foreign,
                        invoice_rate: inv_rate,
                        book_value_inr: book_val,
                        revaluation_rate: reval_rate,
                        revalued_value_inr: reval_val,
                        unrealized_gain_or_loss: diff,
                        is_gain,
                    });
                }

                let net_gain_loss = total_gain - total_loss;

                Ok(UnrealizedForexReport {
                    company_id: cid,
                    as_of_date: aod,
                    bills: bill_rows,
                    total_book_value_inr: total_book,
                    total_revalued_value_inr: total_revalued,
                    total_unrealized_gain: total_gain,
                    total_unrealized_loss: total_loss,
                    net_unrealized_gain_loss: net_gain_loss,
                })
            })
        })
        .await
    }

    /// List cheque templates for a company.
    pub async fn list_cheque_templates(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<ChequeTemplateDto>, OpenTallyError> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                f64,
                f64,
                f64,
                f64,
                f64,
                f64,
                f64,
                f64,
                f64,
                f64,
                i64,
            ),
        >(
            r#"
            SELECT id, company_id, bank_name, width_mm, height_mm,
                   date_x_mm, date_y_mm, payee_x_mm, payee_y_mm,
                   words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default
            FROM cheque_templates
            WHERE company_id = ?
            ORDER BY bank_name ASC
            "#,
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows
            .into_iter()
            .map(
                |(id, cid, b_name, w, h, dx, dy, px, py, wx, wy, ax, ay, ac)| ChequeTemplateDto {
                    id,
                    company_id: cid,
                    bank_name: b_name,
                    width_mm: w,
                    height_mm: h,
                    date_x_mm: dx,
                    date_y_mm: dy,
                    payee_x_mm: px,
                    payee_y_mm: py,
                    words_x_mm: wx,
                    words_y_mm: wy,
                    amount_x_mm: ax,
                    amount_y_mm: ay,
                    is_account_payee_default: ac != 0,
                },
            )
            .collect())
    }

    /// Upsert a cheque template.
    pub async fn upsert_cheque_template(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: UpsertChequeTemplateInput,
        actor: &str,
    ) -> Result<ChequeTemplateDto, OpenTallyError> {
        let b_name = input.bank_name.trim();
        if b_name.is_empty() || b_name.len() > 120 {
            return Err(OpenTallyError::Validation(
                "Bank name must be 1 to 120 characters".to_string(),
            ));
        }

        if input.width_mm <= 50.0 || input.width_mm > 400.0 {
            return Err(OpenTallyError::Validation(
                "Cheque width must be between 50mm and 400mm".to_string(),
            ));
        }
        if input.height_mm <= 30.0 || input.height_mm > 200.0 {
            return Err(OpenTallyError::Validation(
                "Cheque height must be between 30mm and 200mm".to_string(),
            ));
        }

        let ac_payee = input.is_account_payee_default.unwrap_or(true);
        let id = input
            .template_id
            .unwrap_or_else(|| format!("tmpl_{}_{}", company_id, Uuid::new_v4().simple()));

        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO cheque_templates (id, company_id, bank_name, width_mm, height_mm, date_x_mm, date_y_mm, payee_x_mm, payee_y_mm, words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT (company_id, bank_name) DO UPDATE SET
                width_mm = excluded.width_mm,
                height_mm = excluded.height_mm,
                date_x_mm = excluded.date_x_mm,
                date_y_mm = excluded.date_y_mm,
                payee_x_mm = excluded.payee_x_mm,
                payee_y_mm = excluded.payee_y_mm,
                words_x_mm = excluded.words_x_mm,
                words_y_mm = excluded.words_y_mm,
                amount_x_mm = excluded.amount_x_mm,
                amount_y_mm = excluded.amount_y_mm,
                is_account_payee_default = excluded.is_account_payee_default,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(b_name)
        .bind(input.width_mm)
        .bind(input.height_mm)
        .bind(input.date_x_mm)
        .bind(input.date_y_mm)
        .bind(input.payee_x_mm)
        .bind(input.payee_y_mm)
        .bind(input.words_x_mm)
        .bind(input.words_y_mm)
        .bind(input.amount_x_mm)
        .bind(input.amount_y_mm)
        .bind(if ac_payee { 1 } else { 0 })
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::record_audit_entry_tx(
            &mut tx,
            company_id,
            "cheque_template",
            &id,
            "UPDATE",
            Some(&format!("{{\"bank\":\"{}\"}}", b_name)),
            actor,
        )
        .await?;

        tx.commit().await?;

        Ok(ChequeTemplateDto {
            id,
            company_id: company_id.to_string(),
            bank_name: b_name.to_string(),
            width_mm: input.width_mm,
            height_mm: input.height_mm,
            date_x_mm: input.date_x_mm,
            date_y_mm: input.date_y_mm,
            payee_x_mm: input.payee_x_mm,
            payee_y_mm: input.payee_y_mm,
            words_x_mm: input.words_x_mm,
            words_y_mm: input.words_y_mm,
            amount_x_mm: input.amount_x_mm,
            amount_y_mm: input.amount_y_mm,
            is_account_payee_default: ac_payee,
        })
    }
}
