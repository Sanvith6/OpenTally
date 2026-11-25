use super::models::*;
use super::validators::{validate_pan, validate_tan};
use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

pub struct TdsService;

pub fn to_canonical_decimal(d: Decimal) -> String {
    let mut s = d.to_string();
    if !s.contains('.') {
        s.push_str(".00");
    } else if let Some(dot_pos) = s.find('.') {
        let decs = s.len() - dot_pos - 1;
        if decs < 2 {
            s.push('0');
        }
    }
    s
}

impl TdsService {
    /// Retrieve company TDS profile.
    pub async fn get_company_tds_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Option<CompanyTdsProfile>, OpenTallyError> {
        let res = sqlx::query_as::<_, CompanyTdsProfile>(
            "SELECT company_id, tan, deductor_type, tds_enabled, created_at, updated_at
             FROM company_tds_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Retrieve company TDS profile inside an active transaction.
    pub async fn get_company_tds_profile_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
    ) -> Result<Option<CompanyTdsProfile>, OpenTallyError> {
        let res = sqlx::query_as::<_, CompanyTdsProfile>(
            "SELECT company_id, tan, deductor_type, tds_enabled, created_at, updated_at
             FROM company_tds_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Save or update company TDS profile with TAN validation and MCA audit log.
    pub async fn save_company_tds_profile(
        pool: &Pool<Sqlite>,
        input: SaveCompanyTdsProfileInput,
        actor_id: Option<&str>,
    ) -> Result<CompanyTdsProfile, OpenTallyError> {
        if let Some(ref tan) = input.tan {
            let trimmed = tan.trim();
            if !trimmed.is_empty() {
                validate_tan(trimmed)?;
            }
        }

        let mut tx = pool.begin().await?;

        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => {
                    return Err(OpenTallyError::NotFound(format!(
                        "Company '{}' not found",
                        input.company_id
                    )))
                }
            };

        let clean_tan = input
            .tan
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_uppercase());

        let tds_enabled_int = if input.tds_enabled { 1i64 } else { 0i64 };

        let old_profile = Self::get_company_tds_profile_tx(&mut tx, &input.company_id).await?;

        sqlx::query(
            r#"
            INSERT INTO company_tds_profiles (company_id, tan, deductor_type, tds_enabled, updated_at)
            VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id) DO UPDATE SET
                tan = excluded.tan,
                deductor_type = excluded.deductor_type,
                tds_enabled = excluded.tds_enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&input.company_id)
        .bind(&clean_tan)
        .bind(&input.deductor_type)
        .bind(tds_enabled_int)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let audit_id = Uuid::new_v4().to_string();
        let old_val = serde_json::to_string(&old_profile).unwrap_or_default();
        let new_val = serde_json::json!({
            "tan": clean_tan,
            "deductor_type": input.deductor_type,
            "tds_enabled": input.tds_enabled,
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, old_values, new_values, performed_by
            ) VALUES (?, ?, 'company_tds_profile', ?, ?, 'UPDATE', ?, ?, ?)
            "#,
        )
        .bind(&audit_id)
        .bind(&input.company_id)
        .bind(&input.company_id)
        .bind(&fy_begins)
        .bind(&old_val)
        .bind(&new_val)
        .bind(actor_id.unwrap_or("system"))
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let saved = Self::get_company_tds_profile_tx(&mut tx, &input.company_id)
            .await?
            .ok_or_else(|| OpenTallyError::Database(sqlx::Error::RowNotFound))?;

        tx.commit().await?;
        Ok(saved)
    }

    /// Retrieve TDS profile for a specific ledger.
    pub async fn get_ledger_tds_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<Option<LedgerTdsProfile>, OpenTallyError> {
        let res = sqlx::query_as::<_, LedgerTdsProfile>(
            "SELECT id, company_id, ledger_id, pan, deductee_type, lower_deduction_cert_no,
                    lower_deduction_rate, created_at, updated_at
             FROM ledger_tds_profiles WHERE company_id = ? AND ledger_id = ?",
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Retrieve TDS profile for a specific ledger inside an active transaction.
    pub async fn get_ledger_tds_profile_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<Option<LedgerTdsProfile>, OpenTallyError> {
        let res = sqlx::query_as::<_, LedgerTdsProfile>(
            "SELECT id, company_id, ledger_id, pan, deductee_type, lower_deduction_cert_no,
                    lower_deduction_rate, created_at, updated_at
             FROM ledger_tds_profiles WHERE company_id = ? AND ledger_id = ?",
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_optional(conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Save or update TDS profile for a ledger with PAN validation.
    pub async fn save_ledger_tds_profile(
        pool: &Pool<Sqlite>,
        input: SaveLedgerTdsProfileInput,
        actor_id: Option<&str>,
    ) -> Result<LedgerTdsProfile, OpenTallyError> {
        let clean_pan = if let Some(ref pan) = input.pan {
            let trimmed = pan.trim().to_uppercase();
            if !trimmed.is_empty() {
                validate_pan(&trimmed)?;
                Some(trimmed)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(ref ldr) = input.lower_deduction_rate {
            let rate = Decimal::from_str(ldr).map_err(|_| {
                OpenTallyError::Validation(format!("Invalid lower deduction rate '{}'", ldr))
            })?;
            if rate < Decimal::ZERO || rate > Decimal::from(100) {
                return Err(OpenTallyError::Validation(format!(
                    "Lower deduction rate '{}' must be between 0 and 100",
                    ldr
                )));
            }
        }

        let mut tx = pool.begin().await?;

        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => {
                    return Err(OpenTallyError::NotFound(format!(
                        "Company '{}' not found",
                        input.company_id
                    )))
                }
            };

        let old_profile =
            Self::get_ledger_tds_profile_tx(&mut tx, &input.company_id, &input.ledger_id).await?;
        let profile_id = old_profile
            .as_ref()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let clean_cert = input
            .lower_deduction_cert_no
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let clean_rate = input
            .lower_deduction_rate
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        sqlx::query(
            r#"
            INSERT INTO ledger_tds_profiles (
                id, company_id, ledger_id, pan, deductee_type,
                lower_deduction_cert_no, lower_deduction_rate, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id, ledger_id) DO UPDATE SET
                pan = excluded.pan,
                deductee_type = excluded.deductee_type,
                lower_deduction_cert_no = excluded.lower_deduction_cert_no,
                lower_deduction_rate = excluded.lower_deduction_rate,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&profile_id)
        .bind(&input.company_id)
        .bind(&input.ledger_id)
        .bind(&clean_pan)
        .bind(&input.deductee_type)
        .bind(clean_cert)
        .bind(clean_rate)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let audit_id = Uuid::new_v4().to_string();
        let old_val = serde_json::to_string(&old_profile).unwrap_or_default();
        let new_val = serde_json::json!({
            "pan": clean_pan,
            "deductee_type": input.deductee_type,
            "lower_deduction_cert_no": clean_cert,
            "lower_deduction_rate": clean_rate,
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, old_values, new_values, performed_by
            ) VALUES (?, ?, 'ledger_tds_profile', ?, ?, 'UPDATE', ?, ?, ?)
            "#,
        )
        .bind(&audit_id)
        .bind(&input.company_id)
        .bind(&input.ledger_id)
        .bind(&fy_begins)
        .bind(&old_val)
        .bind(&new_val)
        .bind(actor_id.unwrap_or("system"))
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let saved = Self::get_ledger_tds_profile_tx(&mut tx, &input.company_id, &input.ledger_id)
            .await?
            .ok_or_else(|| OpenTallyError::Database(sqlx::Error::RowNotFound))?;

        tx.commit().await?;
        Ok(saved)
    }

    /// List all Nature of Payment sections configured for a company.
    pub async fn list_tds_sections(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<TdsNatureOfPayment>, OpenTallyError> {
        let res = sqlx::query_as::<_, TdsNatureOfPayment>(
            "SELECT id, company_id, section_code, name, rate_individual_huf, rate_others,
                    rate_without_pan, single_threshold_limit, aggregate_threshold_limit,
                    tds_payable_ledger_id, is_tcs, created_at, updated_at
             FROM tds_nature_of_payments
             WHERE company_id = ?
             ORDER BY section_code ASC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// List all Nature of Payment sections inside a transaction.
    pub async fn list_tds_sections_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
    ) -> Result<Vec<TdsNatureOfPayment>, OpenTallyError> {
        let res = sqlx::query_as::<_, TdsNatureOfPayment>(
            "SELECT id, company_id, section_code, name, rate_individual_huf, rate_others,
                    rate_without_pan, single_threshold_limit, aggregate_threshold_limit,
                    tds_payable_ledger_id, is_tcs, created_at, updated_at
             FROM tds_nature_of_payments
             WHERE company_id = ?
             ORDER BY section_code ASC",
        )
        .bind(company_id)
        .fetch_all(conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Retrieve a specific section by code.
    pub async fn get_tds_section_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        section_code: &str,
    ) -> Result<Option<TdsNatureOfPayment>, OpenTallyError> {
        let res = sqlx::query_as::<_, TdsNatureOfPayment>(
            "SELECT id, company_id, section_code, name, rate_individual_huf, rate_others,
                    rate_without_pan, single_threshold_limit, aggregate_threshold_limit,
                    tds_payable_ledger_id, is_tcs, created_at, updated_at
             FROM tds_nature_of_payments
             WHERE company_id = ? AND section_code = ?",
        )
        .bind(company_id)
        .bind(section_code)
        .fetch_optional(conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(res)
    }

    /// Create or update a TDS nature of payment section.
    pub async fn create_tds_section(
        pool: &Pool<Sqlite>,
        input: CreateTdsNatureOfPaymentInput,
        actor_id: Option<&str>,
    ) -> Result<TdsNatureOfPayment, OpenTallyError> {
        let mut tx = pool.begin().await?;

        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => {
                    return Err(OpenTallyError::NotFound(format!(
                        "Company '{}' not found",
                        input.company_id
                    )))
                }
            };

        let id = Uuid::new_v4().to_string();
        let rate_wo_pan = input
            .rate_without_pan
            .unwrap_or_else(|| "20.00".to_string());
        let single_limit = input
            .single_threshold_limit
            .unwrap_or_else(|| "0.00".to_string());
        let agg_limit = input
            .aggregate_threshold_limit
            .unwrap_or_else(|| "0.00".to_string());
        let is_tcs_int = if input.is_tcs.unwrap_or(false) {
            1i64
        } else {
            0i64
        };

        sqlx::query(
            r#"
            INSERT INTO tds_nature_of_payments (
                id, company_id, section_code, name, rate_individual_huf, rate_others,
                rate_without_pan, single_threshold_limit, aggregate_threshold_limit,
                tds_payable_ledger_id, is_tcs, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id, section_code) DO UPDATE SET
                name = excluded.name,
                rate_individual_huf = excluded.rate_individual_huf,
                rate_others = excluded.rate_others,
                rate_without_pan = excluded.rate_without_pan,
                single_threshold_limit = excluded.single_threshold_limit,
                aggregate_threshold_limit = excluded.aggregate_threshold_limit,
                tds_payable_ledger_id = excluded.tds_payable_ledger_id,
                is_tcs = excluded.is_tcs,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(&input.company_id)
        .bind(&input.section_code)
        .bind(&input.name)
        .bind(&input.rate_individual_huf)
        .bind(&input.rate_others)
        .bind(&rate_wo_pan)
        .bind(&single_limit)
        .bind(&agg_limit)
        .bind(&input.tds_payable_ledger_id)
        .bind(is_tcs_int)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let audit_id = Uuid::new_v4().to_string();
        let new_val = serde_json::json!({
            "section_code": input.section_code,
            "name": input.name,
            "rate_individual_huf": input.rate_individual_huf,
            "rate_others": input.rate_others,
        })
        .to_string();

        sqlx::query(
            r#"
            INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, old_values, new_values, performed_by
            ) VALUES (?, ?, 'tds_nature_of_payment', ?, ?, 'CREATE', NULL, ?, ?)
            "#,
        )
        .bind(&audit_id)
        .bind(&input.company_id)
        .bind(&input.section_code)
        .bind(&fy_begins)
        .bind(&new_val)
        .bind(actor_id.unwrap_or("system"))
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let saved = Self::get_tds_section_tx(&mut tx, &input.company_id, &input.section_code)
            .await?
            .ok_or_else(|| OpenTallyError::Database(sqlx::Error::RowNotFound))?;

        tx.commit().await?;
        Ok(saved)
    }

    /// Auto-seeds standard Indian TDS sections and ensures 'TDS Payable' ledger exists under 'Duties & Taxes'.
    pub async fn seed_default_tds_sections_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
    ) -> Result<String, OpenTallyError> {
        // 1. Resolve or create 'TDS Payable' ledger under 'Duties & Taxes'
        let duties_group_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Duties & Taxes' COLLATE NOCASE LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let duties_group = duties_group_id.ok_or_else(|| {
            OpenTallyError::Validation(
                "Predefined group 'Duties & Taxes' missing in company".to_string(),
            )
        })?;

        let existing_tds_ledger: Option<String> = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = 'TDS Payable' COLLATE NOCASE LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let tds_payable_ledger_id = match existing_tds_ledger {
            Some(id) => id,
            None => {
                let new_id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO ledgers (
                        id, company_id, group_id, name, is_reserved, ledger_classification, created_at, updated_at
                    ) VALUES (?, ?, ?, 'TDS Payable', 0, 'standard', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    "#,
                )
                .bind(&new_id)
                .bind(company_id)
                .bind(&duties_group)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                new_id
            }
        };

        // 2. Standard Indian Sections
        let defaults = [
            (
                "194C",
                "Payments to Contractors",
                "1.00",
                "2.00",
                "20.00",
                "30000.00",
                "100000.00",
                0,
            ),
            (
                "194J",
                "Fees for Professional or Technical Services",
                "10.00",
                "10.00",
                "20.00",
                "30000.00",
                "30000.00",
                0,
            ),
            (
                "194I",
                "Rent of Land, Building or Furniture",
                "10.00",
                "10.00",
                "20.00",
                "240000.00",
                "240000.00",
                0,
            ),
            (
                "194H",
                "Commission or Brokerage",
                "5.00",
                "5.00",
                "20.00",
                "15000.00",
                "15000.00",
                0,
            ),
            (
                "194A",
                "Interest Other than Securities",
                "10.00",
                "10.00",
                "20.00",
                "40000.00",
                "40000.00",
                0,
            ),
            (
                "194Q",
                "Purchase of Goods",
                "0.10",
                "0.10",
                "5.00",
                "0.00",
                "5000000.00",
                0,
            ),
            (
                "206C(1H)",
                "TCS on Sale of Goods",
                "0.10",
                "0.10",
                "1.00",
                "0.00",
                "5000000.00",
                1,
            ),
        ];

        for (code, name, rate_ind, rate_oth, rate_wo_pan, s_thresh, agg_thresh, is_tcs) in defaults
        {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO tds_nature_of_payments (
                    id, company_id, section_code, name, rate_individual_huf, rate_others,
                    rate_without_pan, single_threshold_limit, aggregate_threshold_limit,
                    tds_payable_ledger_id, is_tcs, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(company_id, section_code) DO NOTHING
                "#,
            )
            .bind(&id)
            .bind(company_id)
            .bind(code)
            .bind(name)
            .bind(rate_ind)
            .bind(rate_oth)
            .bind(rate_wo_pan)
            .bind(s_thresh)
            .bind(agg_thresh)
            .bind(&tds_payable_ledger_id)
            .bind(is_tcs)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(tds_payable_ledger_id)
    }

    /// Evaluates Indian Financial Year bounds for a given voucher date (YYYY-MM-DD).
    /// Indian FY runs Apr 1 to Mar 31.
    pub fn get_financial_year_bounds(
        voucher_date: &str,
    ) -> Result<(String, String), OpenTallyError> {
        let parsed = NaiveDate::parse_from_str(voucher_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid date '{}'; expected YYYY-MM-DD",
                voucher_date
            ))
        })?;

        use chrono::Datelike;
        let year = parsed.year();
        let month = parsed.month();

        let (fy_start_year, fy_end_year) = if month >= 4 {
            (year, year + 1)
        } else {
            (year - 1, year)
        };

        Ok((
            format!("{:04}-04-01", fy_start_year),
            format!("{:04}-03-31", fy_end_year),
        ))
    }

    /// Deterministically calculates TDS on a taxable base according to statutory CBDT rules,
    /// evaluating single/aggregate thresholds and Sec 206AA missing-PAN 20% higher rate.
    pub async fn calculate_tds_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        party_ledger_id: &str,
        section_code: &str,
        taxable_amount: Decimal,
        voucher_date: &str,
    ) -> Result<TdsCalculationPreviewResult, OpenTallyError> {
        let section = Self::get_tds_section_tx(conn, company_id, section_code)
            .await?
            .ok_or_else(|| {
                OpenTallyError::Validation(format!(
                    "TDS Section '{}' is not configured in this company",
                    section_code
                ))
            })?;

        let party_profile =
            Self::get_ledger_tds_profile_tx(conn, company_id, party_ledger_id).await?;

        let deductee_type = party_profile
            .as_ref()
            .map(|p| p.deductee_type.clone())
            .unwrap_or_else(|| "Company_Domestic".to_string());

        let party_pan = party_profile.as_ref().and_then(|p| p.pan.clone());
        let is_pan_missing = party_pan.as_deref().map(str::trim).unwrap_or("").is_empty();

        let (applicable_rate, is_pan_missing_higher_rate) = if is_pan_missing {
            let statutory_rate = if deductee_type == "Individual_HUF" {
                Decimal::from_str(&section.rate_individual_huf).unwrap_or(Decimal::ZERO)
            } else {
                Decimal::from_str(&section.rate_others).unwrap_or(Decimal::ZERO)
            };
            let rate_wo_pan =
                Decimal::from_str(&section.rate_without_pan).unwrap_or(Decimal::from(20));
            (statutory_rate.max(rate_wo_pan), true)
        } else if let Some(ref cert_rate_str) = party_profile
            .as_ref()
            .and_then(|p| p.lower_deduction_rate.clone())
        {
            (
                Decimal::from_str(cert_rate_str).unwrap_or(Decimal::ZERO),
                false,
            )
        } else if deductee_type == "Individual_HUF" {
            (
                Decimal::from_str(&section.rate_individual_huf).unwrap_or(Decimal::ZERO),
                false,
            )
        } else {
            (
                Decimal::from_str(&section.rate_others).unwrap_or(Decimal::ZERO),
                false,
            )
        };

        // Threshold evaluation across active FY vouchers
        let (fy_start, fy_end) = Self::get_financial_year_bounds(voucher_date)?;

        let prior_vouchers_gross: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT vtd.gross_amount
            FROM voucher_tds_details vtd
            JOIN vouchers v ON vtd.company_id = v.company_id AND vtd.voucher_id = v.id
            WHERE vtd.company_id = ?
              AND vtd.party_ledger_id = ?
              AND vtd.section_code = ?
              AND vtd.is_cancelled = 0
              AND v.status = 'posted'
              AND v.date >= ? AND v.date <= ?
            "#,
        )
        .bind(company_id)
        .bind(party_ledger_id)
        .bind(section_code)
        .bind(&fy_start)
        .bind(&fy_end)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut prior_aggregate = Decimal::ZERO;
        for g_str in prior_vouchers_gross {
            if let Ok(d) = Decimal::from_str(&g_str) {
                prior_aggregate += d;
            }
        }

        let current_aggregate = prior_aggregate + taxable_amount;

        let single_limit =
            Decimal::from_str(&section.single_threshold_limit).unwrap_or(Decimal::ZERO);
        let agg_limit =
            Decimal::from_str(&section.aggregate_threshold_limit).unwrap_or(Decimal::ZERO);

        let threshold_exceeded = if single_limit.is_zero() && agg_limit.is_zero() {
            true
        } else {
            let single_exceeded = !single_limit.is_zero() && taxable_amount >= single_limit;
            let agg_exceeded = !agg_limit.is_zero() && current_aggregate >= agg_limit;
            single_exceeded || agg_exceeded
        };

        let tds_amount = if threshold_exceeded {
            let raw_tds = (taxable_amount * applicable_rate) / Decimal::from(100);
            raw_tds.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
        } else {
            Decimal::ZERO
        };

        let net_payable = taxable_amount - tds_amount;

        Ok(TdsCalculationPreviewResult {
            section_code: section.section_code,
            section_name: section.name,
            deductee_type,
            party_pan,
            is_pan_missing,
            is_pan_missing_higher_rate,
            taxable_base: to_canonical_decimal(taxable_amount),
            applicable_rate: to_canonical_decimal(applicable_rate),
            threshold_exceeded,
            single_threshold: to_canonical_decimal(single_limit),
            aggregate_threshold: to_canonical_decimal(agg_limit),
            prior_aggregate_amount: to_canonical_decimal(prior_aggregate),
            current_aggregate_amount: to_canonical_decimal(current_aggregate),
            tds_amount: to_canonical_decimal(tds_amount),
            net_payable_amount: to_canonical_decimal(net_payable),
            tds_payable_ledger_id: section.tds_payable_ledger_id,
        })
    }

    /// Read-only preview of TDS calculation.
    pub async fn preview_tds_calculation(
        pool: &Pool<Sqlite>,
        input: TdsCalculationPreviewInput,
    ) -> Result<TdsCalculationPreviewResult, OpenTallyError> {
        let taxable_amt = Decimal::from_str(&input.taxable_amount).map_err(|_| {
            OpenTallyError::Validation(format!("Invalid taxable amount '{}'", input.taxable_amount))
        })?;

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                Self::calculate_tds_tx(
                    conn,
                    &input.company_id,
                    &input.party_ledger_id,
                    &input.section_code,
                    taxable_amt,
                    &input.voucher_date,
                )
                .await
            })
        })
        .await
    }

    /// Atomically records TDS details for a posted commercial voucher.
    pub async fn record_voucher_tds_details_tx(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        party_ledger_id: &str,
        calc: &TdsCalculationPreviewResult,
    ) -> Result<String, OpenTallyError> {
        let id = Uuid::new_v4().to_string();
        let is_higher_int = if calc.is_pan_missing_higher_rate {
            1i64
        } else {
            0i64
        };

        sqlx::query(
            r#"
            INSERT INTO voucher_tds_details (
                id, company_id, voucher_id, party_ledger_id, party_pan,
                section_code, deductee_type, gross_amount, tds_rate,
                tds_amount, net_payable_amount, is_pan_missing_higher_rate,
                is_paid, is_cancelled, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(party_ledger_id)
        .bind(&calc.party_pan)
        .bind(&calc.section_code)
        .bind(&calc.deductee_type)
        .bind(&calc.taxable_base)
        .bind(&calc.applicable_rate)
        .bind(&calc.tds_amount)
        .bind(&calc.net_payable_amount)
        .bind(is_higher_int)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(id)
    }

    /// Enforces TDS cancellation guardrail: rejects if TDS has already been paid via Challan,
    /// otherwise marks TDS details as cancelled and logs to MCA audit log.
    pub async fn verify_and_cancel_voucher_tds(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_number: &str,
        actor_id: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        // Defensive check: does voucher_tds_details exist in sqlite_master?
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'voucher_tds_details'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        // 1. Guardrail check: reject if already paid via Challan
        let paid_row: Option<(String, String)> = sqlx::query_as(
            "SELECT section_code, tds_amount FROM voucher_tds_details
             WHERE company_id = ? AND voucher_id = ? AND is_paid = 1 AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((sec, amt)) = paid_row {
            return Err(OpenTallyError::Validation(format!(
                "Cannot cancel or reverse voucher '{}': TDS of ₹{} under Section {} has already been remitted to government via Challan",
                voucher_number, amt, sec
            )));
        }

        // 2. Mark active TDS details as cancelled
        let rows_affected = sqlx::query(
            "UPDATE voucher_tds_details SET is_cancelled = 1 WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0",
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
            let new_val = serde_json::json!({
                "voucher_number": voucher_number,
                "is_cancelled": 1,
            })
            .to_string();

            sqlx::query(
                r#"
                INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, financial_year_begins,
                    action, old_values, new_values, performed_by
                ) VALUES (?, ?, 'voucher_tds', ?, ?, 'CANCEL', NULL, ?, ?)
                "#,
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(&fy_begins)
            .bind(&new_val)
            .bind(actor_id.unwrap_or("system"))
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    /// Links one or more unpaid TDS records to a remitted Challan payment voucher inside BEGIN IMMEDIATE.
    pub async fn record_tds_challan_payment(
        pool: &Pool<Sqlite>,
        input: RecordTdsChallanPaymentInput,
    ) -> Result<usize, OpenTallyError> {
        let bsr_clean = input.challan_bsr_code.trim();
        if bsr_clean.len() != 7 || !bsr_clean.chars().all(|c| c.is_ascii_digit()) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid Challan BSR Code '{}': BSR code must be exactly 7 numeric digits",
                input.challan_bsr_code
            )));
        }

        let serial_clean = input.challan_serial_no.trim();
        if serial_clean.is_empty() {
            return Err(OpenTallyError::Validation(
                "Challan Serial Number cannot be empty".to_string(),
            ));
        }

        NaiveDate::parse_from_str(input.challan_date.trim(), "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid Challan Date '{}'; expected YYYY-MM-DD",
                input.challan_date
            ))
        })?;

        if input.voucher_tds_detail_ids.is_empty() {
            return Err(OpenTallyError::Validation(
                "No TDS deductions selected for Challan tagging".to_string(),
            ));
        }

        let mut tx = pool.begin().await?;

        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => {
                    return Err(OpenTallyError::NotFound(format!(
                        "Company '{}' not found",
                        input.company_id
                    )))
                }
            };

        if let Some(ref pv_id) = input.payment_voucher_id {
            let pv_exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM vouchers WHERE company_id = ? AND id = ? AND status = 'posted'",
            )
            .bind(&input.company_id)
            .bind(pv_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if pv_exists.is_none() {
                return Err(OpenTallyError::Validation(format!(
                    "Payment voucher '{}' not found or is not posted in this company",
                    pv_id
                )));
            }
        }

        let mut updated_count = 0;
        for detail_id in &input.voucher_tds_detail_ids {
            let row: Option<(i64, i64, String, String)> = sqlx::query_as(
                "SELECT is_paid, is_cancelled, section_code, tds_amount
                 FROM voucher_tds_details WHERE company_id = ? AND id = ?",
            )
            .bind(&input.company_id)
            .bind(detail_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let (is_paid, is_cancelled, sec, amt) = match row {
                Some(r) => r,
                None => {
                    return Err(OpenTallyError::NotFound(format!(
                        "TDS deduction detail '{}' not found",
                        detail_id
                    )))
                }
            };

            if is_cancelled == 1 {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot link Challan to cancelled TDS deduction '{}'",
                    detail_id
                )));
            }
            if is_paid == 1 {
                return Err(OpenTallyError::Validation(format!(
                    "TDS deduction '{}' (Sec {}, ₹{}) is already tagged to a remitted Challan",
                    detail_id, sec, amt
                )));
            }

            sqlx::query(
                r#"
                UPDATE voucher_tds_details
                SET challan_bsr_code = ?,
                    challan_date = ?,
                    challan_serial_no = ?,
                    payment_voucher_id = ?,
                    is_paid = 1
                WHERE company_id = ? AND id = ?
                "#,
            )
            .bind(bsr_clean)
            .bind(input.challan_date.trim())
            .bind(serial_clean)
            .bind(&input.payment_voucher_id)
            .bind(&input.company_id)
            .bind(detail_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let audit_id = Uuid::new_v4().to_string();
            let new_val = serde_json::json!({
                "challan_bsr_code": bsr_clean,
                "challan_date": input.challan_date.trim(),
                "challan_serial_no": serial_clean,
                "payment_voucher_id": input.payment_voucher_id,
                "is_paid": 1,
            })
            .to_string();

            sqlx::query(
                r#"
                INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, financial_year_begins,
                    action, old_values, new_values, performed_by
                ) VALUES (?, ?, 'tds_challan', ?, ?, 'UPDATE', NULL, ?, ?)
                "#,
            )
            .bind(&audit_id)
            .bind(&input.company_id)
            .bind(detail_id)
            .bind(&fy_begins)
            .bind(&new_val)
            .bind(input.actor_id.as_deref().unwrap_or("system"))
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            updated_count += 1;
        }

        tx.commit().await?;
        Ok(updated_count)
    }
}
