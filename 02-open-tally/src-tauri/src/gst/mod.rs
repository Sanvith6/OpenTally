pub mod einvoice;
pub mod reconciliation;

pub use einvoice::{
    EinvoiceService, EinvoiceV11Payload, EwayBillJsonPayload, TransportDetailsInput,
    VoucherEinvoiceMetadataDto, VoucherEinvoiceMetadataInput,
};
pub use reconciliation::{
    Gstr2bReconciledInvoiceRow, Gstr2bReconciliationReport, Gstr2bReconciliationSummary,
    Gstr2bService,
};

use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, SqliteConnection};
use uuid::Uuid;

const GSTIN_ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Check if a two-digit code is a valid Indian GST state or territory code.
/// 01 to 38 represent standard states and union territories, 97 represents other territory.
pub fn is_valid_state_code(code: &str) -> bool {
    if code.len() != 2 || !code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match code.parse::<u32>() {
        Ok(n) => (1..=38).contains(&n) || n == 97,
        Err(_) => false,
    }
}

/// Check if a 15-character string matches the structural pattern of an Indian GSTIN:
/// - 2 digits (State Code)
/// - 5 uppercase letters (PAN alphabets)
/// - 4 digits (PAN numeric characters)
/// - 1 uppercase letter (PAN check character)
/// - 1 alphanumeric character (entity number of same PAN in the state)
/// - 1 'Z' character (default 14th character)
/// - 1 alphanumeric character (check digit)
pub fn matches_gstin_structure(s: &str) -> bool {
    if s.len() != 15 {
        return false;
    }
    let b = s.as_bytes();
    b[0..2].iter().all(|c| c.is_ascii_digit())
        && b[2..7].iter().all(|c| c.is_ascii_uppercase())
        && b[7..11].iter().all(|c| c.is_ascii_digit())
        && b[11].is_ascii_uppercase()
        && b[12].is_ascii_alphanumeric()
        && b[13] == b'Z'
        && b[14].is_ascii_alphanumeric()
}

/// Calculate the 15th check digit for a 14-character GSTIN prefix using the standard Luhn mod-36 algorithm.
pub fn calculate_gstin_check_digit(input_14: &str) -> Result<char, OpenTallyError> {
    if input_14.len() != 14 {
        return Err(OpenTallyError::Validation(format!(
            "GSTIN prefix must be exactly 14 characters, got {}",
            input_14.len()
        )));
    }

    let mut total = 0u64;
    for (idx, ch) in input_14.chars().enumerate() {
        let val = match ch {
            '0'..='9' => (ch as u8 - b'0') as u64,
            'A'..='Z' => (ch as u8 - b'A' + 10) as u64,
            _ => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid character '{}' in GSTIN",
                    ch
                )))
            }
        };

        let factor = if idx % 2 == 0 { 1 } else { 2 };
        let product = val * factor;
        let quotient = product / 36;
        let remainder = product % 36;
        total += quotient + remainder;
    }

    let rem = total % 36;
    let check_val = (36 - rem) % 36;
    Ok(GSTIN_ALPHABET[check_val as usize] as char)
}

/// Validate a complete 15-character Indian GSTIN:
/// 1. Length & alphanumeric regex structure.
/// 2. Valid 2-digit Indian state code prefix.
/// 3. Optional matching against expected state code.
/// 4. Mod-36 check-digit verification.
pub fn validate_gstin(
    gstin: &str,
    expected_state_code: Option<&str>,
) -> Result<(), OpenTallyError> {
    let clean = gstin.trim().to_uppercase();
    if clean.len() != 15 {
        return Err(OpenTallyError::Validation(format!(
            "GSTIN must be exactly 15 characters, got {}",
            clean.len()
        )));
    }

    if !matches_gstin_structure(&clean) {
        return Err(OpenTallyError::Validation(format!(
            "GSTIN '{}' does not match the standard 15-character Indian GSTIN format",
            clean
        )));
    }

    let state_code = &clean[..2];
    if !is_valid_state_code(state_code) {
        return Err(OpenTallyError::Validation(format!(
            "GSTIN '{}' contains invalid state code '{}'",
            clean, state_code
        )));
    }

    if let Some(expected) = expected_state_code {
        if state_code != expected {
            return Err(OpenTallyError::Validation(format!(
                "GSTIN state code '{}' does not match expected state code '{}'",
                state_code, expected
            )));
        }
    }

    let expected_check_char = calculate_gstin_check_digit(&clean[..14])?;
    let actual_check_char = clean.chars().nth(14).unwrap();
    if actual_check_char != expected_check_char {
        return Err(OpenTallyError::Validation(format!(
            "GSTIN '{}' has invalid check digit '{}'; expected '{}'",
            clean, actual_check_char, expected_check_char
        )));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SupplyType {
    IntraState,
    InterState,
}

impl SupplyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IntraState => "IntraState",
            Self::InterState => "InterState",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Taxability {
    Taxable,
    Exempt,
    NilRated,
    NonGST,
}

impl Taxability {
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "exempt" => Self::Exempt,
            "nilrated" | "nil_rated" | "nil rated" => Self::NilRated,
            "nongst" | "non_gst" | "non gst" => Self::NonGST,
            _ => Self::Taxable,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Taxable => "Taxable",
            Self::Exempt => "Exempt",
            Self::NilRated => "NilRated",
            Self::NonGST => "NonGST",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyGstProfile {
    pub company_id: String,
    pub gstin: Option<String>,
    pub state_code: String,
    pub registration_type: String,
    pub gst_enabled: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerGstProfile {
    pub id: String,
    pub company_id: String,
    pub ledger_id: String,
    pub gstin: Option<String>,
    pub state_code: String,
    pub registration_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HsnSacCode {
    pub id: String,
    pub company_id: String,
    pub code: String,
    pub description: Option<String>,
    pub code_type: String,
    pub default_gst_rate: String,
    pub cess_rate: String,
    pub effective_from: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ItemGstMapping {
    pub id: String,
    pub company_id: String,
    pub stock_item_id: String,
    pub hsn_sac_code_id: String,
    pub gst_rate: String,
    pub cess_rate: String,
    pub taxability: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GstLedgerMapping {
    pub company_id: String,
    pub output_cgst_ledger_id: String,
    pub output_sgst_ledger_id: String,
    pub output_igst_ledger_id: String,
    pub output_cess_ledger_id: Option<String>,
    pub input_cgst_ledger_id: String,
    pub input_sgst_ledger_id: String,
    pub input_igst_ledger_id: String,
    pub input_cess_ledger_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherGstDetail {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub party_ledger_id: String,
    pub party_gstin: Option<String>,
    pub hsn_sac_code: String,
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub taxability: String,
    pub taxable_value: String,
    pub gst_rate: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub is_reverse_charge: i64,
    pub is_cancelled: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstCalculationInput {
    pub company_state_code: String,
    pub place_of_supply_state_code: String,
    pub taxability: Taxability,
    pub taxable_value: Decimal,
    pub gst_rate: Decimal,
    pub cess_rate: Decimal,
    pub is_reverse_charge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GstCalculationResult {
    pub supply_type: SupplyType,
    pub taxable_value: Decimal,
    pub gst_rate: Decimal,
    pub cgst_rate: Decimal,
    pub cgst_amount: Decimal,
    pub sgst_rate: Decimal,
    pub sgst_amount: Decimal,
    pub igst_rate: Decimal,
    pub igst_amount: Decimal,
    pub cess_rate: Decimal,
    pub cess_amount: Decimal,
    pub total_tax: Decimal,
    pub grand_total: Decimal,
}

/// Computes Indian GST breakdown deterministically with pure rust_decimal (no floats).
pub fn calculate_gst(input: &GstCalculationInput) -> GstCalculationResult {
    let supply_type = if input.company_state_code == input.place_of_supply_state_code {
        SupplyType::IntraState
    } else {
        SupplyType::InterState
    };

    if input.taxability != Taxability::Taxable || input.taxable_value.is_zero() {
        return GstCalculationResult {
            supply_type,
            taxable_value: input.taxable_value,
            gst_rate: input.gst_rate,
            cgst_rate: Decimal::ZERO,
            cgst_amount: Decimal::ZERO,
            sgst_rate: Decimal::ZERO,
            sgst_amount: Decimal::ZERO,
            igst_rate: Decimal::ZERO,
            igst_amount: Decimal::ZERO,
            cess_rate: input.cess_rate,
            cess_amount: Decimal::ZERO,
            total_tax: Decimal::ZERO,
            grand_total: input.taxable_value,
        };
    }

    let (cgst_rate, cgst_amount, sgst_rate, sgst_amount, igst_rate, igst_amount) = match supply_type
    {
        SupplyType::IntraState => {
            let half_rate = (input.gst_rate / Decimal::TWO).round_dp(4);
            let cgst = (input.taxable_value * half_rate / Decimal::ONE_HUNDRED).round_dp(2);
            let sgst = (input.taxable_value * half_rate / Decimal::ONE_HUNDRED).round_dp(2);
            (
                half_rate,
                cgst,
                half_rate,
                sgst,
                Decimal::ZERO,
                Decimal::ZERO,
            )
        }
        SupplyType::InterState => {
            let igst = (input.taxable_value * input.gst_rate / Decimal::ONE_HUNDRED).round_dp(2);
            (
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                Decimal::ZERO,
                input.gst_rate,
                igst,
            )
        }
    };

    let cess_amount = if input.cess_rate > Decimal::ZERO {
        (input.taxable_value * input.cess_rate / Decimal::ONE_HUNDRED).round_dp(2)
    } else {
        Decimal::ZERO
    };

    let total_tax = cgst_amount + sgst_amount + igst_amount + cess_amount;
    let grand_total = input.taxable_value + total_tax;

    GstCalculationResult {
        supply_type,
        taxable_value: input.taxable_value,
        gst_rate: input.gst_rate,
        cgst_rate,
        cgst_amount,
        sgst_rate,
        sgst_amount,
        igst_rate,
        igst_amount,
        cess_rate: input.cess_rate,
        cess_amount,
        total_tax,
        grand_total,
    }
}

pub struct GstService;

impl GstService {
    // -------------------------------------------------------------------------
    // Company Profile
    // -------------------------------------------------------------------------
    pub async fn get_company_gst_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Option<CompanyGstProfile>, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        Self::get_company_gst_profile_tx(&mut conn, company_id).await
    }

    pub async fn get_company_gst_profile_tx(
        conn: &mut SqliteConnection,
        company_id: &str,
    ) -> Result<Option<CompanyGstProfile>, OpenTallyError> {
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'company_gst_profiles'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(None);
        }

        let profile = sqlx::query_as::<_, CompanyGstProfile>(
            "SELECT company_id, gstin, state_code, registration_type, gst_enabled, created_at, updated_at
             FROM company_gst_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(profile)
    }

    pub async fn save_company_gst_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
        gstin: Option<&str>,
        state_code: &str,
        registration_type: &str,
        gst_enabled: bool,
    ) -> Result<CompanyGstProfile, OpenTallyError> {
        if !is_valid_state_code(state_code) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid state code '{}'",
                state_code
            )));
        }

        let clean_gstin = match gstin {
            Some(g) if !g.trim().is_empty() => {
                let cleaned = g.trim().to_uppercase();
                validate_gstin(&cleaned, Some(state_code))?;
                Some(cleaned)
            }
            _ => None,
        };

        if !["Regular", "Composition", "Unregistered"].contains(&registration_type) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid registration type '{}'",
                registration_type
            )));
        }

        let gst_enabled_int = if gst_enabled { 1i64 } else { 0i64 };

        sqlx::query(
            "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled, updated_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET
                gstin = excluded.gstin,
                state_code = excluded.state_code,
                registration_type = excluded.registration_type,
                gst_enabled = excluded.gst_enabled,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(company_id)
        .bind(&clean_gstin)
        .bind(state_code)
        .bind(registration_type)
        .bind(gst_enabled_int)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::get_company_gst_profile(pool, company_id)
            .await?
            .ok_or_else(|| OpenTallyError::NotFound("Failed to load saved profile".to_string()))
    }

    // -------------------------------------------------------------------------
    // Ledger Profile
    // -------------------------------------------------------------------------
    pub async fn get_ledger_gst_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<Option<LedgerGstProfile>, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        Self::get_ledger_gst_profile_tx(&mut conn, company_id, ledger_id).await
    }

    pub async fn get_ledger_gst_profile_tx(
        conn: &mut SqliteConnection,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<Option<LedgerGstProfile>, OpenTallyError> {
        let profile = sqlx::query_as::<_, LedgerGstProfile>(
            "SELECT id, company_id, ledger_id, gstin, state_code, registration_type, created_at, updated_at
             FROM ledger_gst_profiles WHERE company_id = ? AND ledger_id = ?",
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(profile)
    }

    pub async fn save_ledger_gst_profile(
        pool: &Pool<Sqlite>,
        company_id: &str,
        ledger_id: &str,
        gstin: Option<&str>,
        state_code: &str,
        registration_type: &str,
    ) -> Result<LedgerGstProfile, OpenTallyError> {
        if !is_valid_state_code(state_code) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid state code '{}'",
                state_code
            )));
        }

        let clean_gstin = match gstin {
            Some(g) if !g.trim().is_empty() => {
                let cleaned = g.trim().to_uppercase();
                validate_gstin(&cleaned, Some(state_code))?;
                Some(cleaned)
            }
            _ => None,
        };

        if !["Regular", "Composition", "Unregistered", "Consumer"].contains(&registration_type) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid registration type '{}'",
                registration_type
            )));
        }

        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id, ledger_id) DO UPDATE SET
                gstin = excluded.gstin,
                state_code = excluded.state_code,
                registration_type = excluded.registration_type,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&id)
        .bind(company_id)
        .bind(ledger_id)
        .bind(&clean_gstin)
        .bind(state_code)
        .bind(registration_type)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::get_ledger_gst_profile(pool, company_id, ledger_id)
            .await?
            .ok_or_else(|| {
                OpenTallyError::NotFound("Failed to load saved ledger profile".to_string())
            })
    }

    // -------------------------------------------------------------------------
    // HSN/SAC Codes
    // -------------------------------------------------------------------------
    pub async fn list_hsn_sac_codes(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<HsnSacCode>, OpenTallyError> {
        let codes = sqlx::query_as::<_, HsnSacCode>(
            "SELECT id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from, created_at
             FROM hsn_sac_codes WHERE company_id = ? ORDER BY code ASC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(codes)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_hsn_sac_code(
        pool: &Pool<Sqlite>,
        company_id: &str,
        code: &str,
        description: Option<&str>,
        code_type: &str,
        default_gst_rate: &str,
        cess_rate: &str,
        effective_from: &str,
    ) -> Result<HsnSacCode, OpenTallyError> {
        let clean_code = code.trim().to_uppercase();
        if clean_code.is_empty() {
            return Err(OpenTallyError::Validation(
                "HSN/SAC code cannot be empty".to_string(),
            ));
        }

        if !["HSN", "SAC"].contains(&code_type) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid code_type '{}'; expected 'HSN' or 'SAC'",
                code_type
            )));
        }

        let parsed_gst = Decimal::from_str_exact(default_gst_rate).map_err(|_| {
            OpenTallyError::Validation(format!("Invalid GST rate '{}'", default_gst_rate))
        })?;
        let parsed_cess = Decimal::from_str_exact(cess_rate).map_err(|_| {
            OpenTallyError::Validation(format!("Invalid Cess rate '{}'", cess_rate))
        })?;

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO hsn_sac_codes (id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(&clean_code)
        .bind(description)
        .bind(code_type)
        .bind(parsed_gst.normalize().to_string())
        .bind(parsed_cess.normalize().to_string())
        .bind(effective_from)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let row = sqlx::query_as::<_, HsnSacCode>(
            "SELECT id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from, created_at
             FROM hsn_sac_codes WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(row)
    }

    // -------------------------------------------------------------------------
    // Item GST Mapping
    // -------------------------------------------------------------------------
    pub async fn get_item_gst_mapping(
        pool: &Pool<Sqlite>,
        company_id: &str,
        stock_item_id: &str,
    ) -> Result<Option<ItemGstMapping>, OpenTallyError> {
        let mapping = sqlx::query_as::<_, ItemGstMapping>(
            "SELECT id, company_id, stock_item_id, hsn_sac_code_id, gst_rate, cess_rate, taxability, created_at
             FROM item_gst_mappings WHERE company_id = ? AND stock_item_id = ?",
        )
        .bind(company_id)
        .bind(stock_item_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(mapping)
    }

    pub async fn save_item_gst_mapping(
        pool: &Pool<Sqlite>,
        company_id: &str,
        stock_item_id: &str,
        hsn_sac_code_id: &str,
        gst_rate: &str,
        cess_rate: &str,
        taxability: &str,
    ) -> Result<ItemGstMapping, OpenTallyError> {
        let parsed_gst = Decimal::from_str_exact(gst_rate)
            .map_err(|_| OpenTallyError::Validation(format!("Invalid GST rate '{}'", gst_rate)))?;
        let parsed_cess = Decimal::from_str_exact(cess_rate).map_err(|_| {
            OpenTallyError::Validation(format!("Invalid Cess rate '{}'", cess_rate))
        })?;

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO item_gst_mappings (id, company_id, stock_item_id, hsn_sac_code_id, gst_rate, cess_rate, taxability)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(company_id, stock_item_id) DO UPDATE SET
                hsn_sac_code_id = excluded.hsn_sac_code_id,
                gst_rate = excluded.gst_rate,
                cess_rate = excluded.cess_rate,
                taxability = excluded.taxability",
        )
        .bind(&id)
        .bind(company_id)
        .bind(stock_item_id)
        .bind(hsn_sac_code_id)
        .bind(parsed_gst.normalize().to_string())
        .bind(parsed_cess.normalize().to_string())
        .bind(taxability)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::get_item_gst_mapping(pool, company_id, stock_item_id)
            .await?
            .ok_or_else(|| {
                OpenTallyError::NotFound("Failed to load saved item mapping".to_string())
            })
    }

    // -------------------------------------------------------------------------
    // GST Ledger Mapping
    // -------------------------------------------------------------------------
    pub async fn get_gst_ledger_mapping(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Option<GstLedgerMapping>, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        Self::get_gst_ledger_mapping_tx(&mut conn, company_id).await
    }

    pub async fn get_gst_ledger_mapping_tx(
        conn: &mut SqliteConnection,
        company_id: &str,
    ) -> Result<Option<GstLedgerMapping>, OpenTallyError> {
        let mapping = sqlx::query_as::<_, GstLedgerMapping>(
            "SELECT company_id, output_cgst_ledger_id, output_sgst_ledger_id, output_igst_ledger_id, output_cess_ledger_id,
                    input_cgst_ledger_id, input_sgst_ledger_id, input_igst_ledger_id, input_cess_ledger_id,
                    created_at, updated_at
             FROM gst_ledger_mappings WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(mapping)
    }

    pub async fn save_gst_ledger_mapping(
        pool: &Pool<Sqlite>,
        mapping: GstLedgerMapping,
    ) -> Result<GstLedgerMapping, OpenTallyError> {
        sqlx::query(
            "INSERT INTO gst_ledger_mappings (
                company_id, output_cgst_ledger_id, output_sgst_ledger_id, output_igst_ledger_id, output_cess_ledger_id,
                input_cgst_ledger_id, input_sgst_ledger_id, input_igst_ledger_id, input_cess_ledger_id, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET
                output_cgst_ledger_id = excluded.output_cgst_ledger_id,
                output_sgst_ledger_id = excluded.output_sgst_ledger_id,
                output_igst_ledger_id = excluded.output_igst_ledger_id,
                output_cess_ledger_id = excluded.output_cess_ledger_id,
                input_cgst_ledger_id = excluded.input_cgst_ledger_id,
                input_sgst_ledger_id = excluded.input_sgst_ledger_id,
                input_igst_ledger_id = excluded.input_igst_ledger_id,
                input_cess_ledger_id = excluded.input_cess_ledger_id,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&mapping.company_id)
        .bind(&mapping.output_cgst_ledger_id)
        .bind(&mapping.output_sgst_ledger_id)
        .bind(&mapping.output_igst_ledger_id)
        .bind(&mapping.output_cess_ledger_id)
        .bind(&mapping.input_cgst_ledger_id)
        .bind(&mapping.input_sgst_ledger_id)
        .bind(&mapping.input_igst_ledger_id)
        .bind(&mapping.input_cess_ledger_id)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::get_gst_ledger_mapping(pool, &mapping.company_id)
            .await?
            .ok_or_else(|| {
                OpenTallyError::NotFound("Failed to load saved ledger mapping".to_string())
            })
    }

    // -------------------------------------------------------------------------
    // Cancellation & Reversal Guardrail
    // -------------------------------------------------------------------------
    pub async fn cancel_voucher_gst_details(
        conn: &mut SqliteConnection,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<(), OpenTallyError> {
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'voucher_gst_details'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        let rows = sqlx::query(
            "UPDATE voucher_gst_details
             SET is_cancelled = 1
             WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        if rows > 0 {
            let audit_id = Uuid::new_v4().to_string();
            let old_values = serde_json::json!({
                "voucher_id": voucher_id,
                "is_cancelled": 0
            })
            .to_string();
            let new_values = serde_json::json!({
                "voucher_id": voucher_id,
                "is_cancelled": 1,
                "affected_gst_lines": rows
            })
            .to_string();

            let _ = sqlx::query(
                "INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, ledger_id,
                    financial_year_begins, action, old_values, new_values, performed_by
                 ) VALUES (?, ?, 'voucher_gst_details', ?, NULL, '2026-04-01', 'CANCEL', ?, ?, 'system')",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(&old_values)
            .bind(&new_values)
            .execute(&mut *conn)
            .await;
        }

        Ok(())
    }
}
