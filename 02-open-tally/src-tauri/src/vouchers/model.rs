use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoucherTypeCategory {
    Accounting,
    Trading,
    Order,
    Inventory,
    Payroll,
    Scenario,
    NonFinancial,
}

impl VoucherTypeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accounting => "accounting",
            Self::Trading => "trading",
            Self::Order => "order",
            Self::Inventory => "inventory",
            Self::Payroll => "payroll",
            Self::Scenario => "scenario",
            Self::NonFinancial => "non_financial",
        }
    }
}

impl std::str::FromStr for VoucherTypeCategory {
    type Err = crate::errors::OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "accounting" => Ok(Self::Accounting),
            "trading" => Ok(Self::Trading),
            "order" => Ok(Self::Order),
            "inventory" => Ok(Self::Inventory),
            "payroll" => Ok(Self::Payroll),
            "scenario" => Ok(Self::Scenario),
            "non_financial" => Ok(Self::NonFinancial),
            _ => Err(crate::errors::OpenTallyError::Validation(format!(
                "Invalid voucher type category: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberingMethod {
    Automatic,
    Manual,
    MultiSeries,
}

impl NumberingMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
            Self::MultiSeries => "multi_series",
        }
    }
}

impl std::str::FromStr for NumberingMethod {
    type Err = crate::errors::OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "automatic" => Ok(Self::Automatic),
            "manual" => Ok(Self::Manual),
            "multi_series" => Ok(Self::MultiSeries),
            _ => Err(crate::errors::OpenTallyError::Validation(format!(
                "Invalid numbering method: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeriesRestartPeriod {
    Yearly,
    Monthly,
    Daily,
    Never,
}

impl SeriesRestartPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yearly => "yearly",
            Self::Monthly => "monthly",
            Self::Daily => "daily",
            Self::Never => "never",
        }
    }
}

impl std::str::FromStr for SeriesRestartPeriod {
    type Err = crate::errors::OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "yearly" => Ok(Self::Yearly),
            "monthly" => Ok(Self::Monthly),
            "daily" => Ok(Self::Daily),
            "never" => Ok(Self::Never),
            _ => Err(crate::errors::OpenTallyError::Validation(format!(
                "Invalid restart period: {}",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherType {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub category: String,
    pub parent_type: Option<String>,
    pub affects_ledger_balance: bool,
    pub affects_inventory: bool,
    pub is_predefined: bool,
    pub is_active: bool,
    pub numbering_method: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherNumberSeries {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub series_name: String,
    pub series_code: String,
    pub numbering_method: String,
    pub restart_period: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherNumberConfig {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub series_id: String,
    pub applicable_from: String,
    pub prefix: String,
    pub suffix: String,
    pub start_number: i64,
    pub width: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVoucherSeriesRequest {
    pub voucher_type_id: String,
    pub series_name: String,
    pub series_code: String,
    pub numbering_method: String,
    pub restart_period: String,
    pub applicable_from: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub start_number: Option<i64>,
    pub width: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateVoucherSeriesRequest {
    pub series_name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVoucherNumberConfigRequest {
    pub applicable_from: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub start_number: Option<i64>,
    pub width: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedManualVoucherNumber {
    pub series_code: String,
    pub prefix: String,
    pub suffix: String,
    pub sequence_number: Option<i64>,
    pub is_numeric: bool,
    pub core: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesNamespaceInfo {
    pub series_name: String,
    pub series_code: String,
    pub numbering_method: String,
    pub prefix: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveNumberConfig {
    pub config_id: String,
    pub voucher_type_id: String,
    pub series_id: String,
    pub prefix: String,
    pub suffix: String,
    pub width: i64,
    pub start_number: i64,
    pub restart_period: String,
    pub applicable_from: String,
    pub period_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AllocatedVoucherNumber {
    pub voucher_number: String,
    pub period_start: String,
    pub sequence_number: i64,
    pub series_id: String,
    pub config_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SequenceRecoveryReport {
    pub company_id: String,
    pub voucher_type_id: String,
    pub series_id: String,
    pub period_start: String,
    pub start_number_baseline: i64,
    pub previous_next_number: i64,
    pub new_next_number: i64,
    pub max_allocated: i64,
    pub total_permanent_allocations: usize,
    pub historical_gaps: Vec<i64>,
    pub counter_tail_gaps: Vec<i64>,
    pub counter_updated: bool,
    pub operator_id: String,
    pub resolution_mode: String,
    pub resolution_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectSequenceRequest {
    pub series_id: String,
    pub period_start: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResyncSequenceRequest {
    pub series_id: String,
    pub period_start: String,
    pub resolution_mode: String,
    pub resolution_reason: Option<String>,
}

// ============================================================================
// Sprint 2 Stage 4: Voucher Header & Line Models and Draft DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct Voucher {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub series_id: String,
    pub numbering_method: String,
    pub period_start: Option<String>,
    pub sequence_number: Option<i64>,
    pub voucher_number: String,
    pub date: String,
    pub effective_date: String,
    pub status: String,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub total_amount: Option<String>,
    pub reversal_of_voucher_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct VoucherLine {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub line_number: i64,
    pub ledger_id: String,
    pub entry_type: String, // "Dr" or "Cr"
    pub amount: String,     // Canonical decimal string
    pub narration: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoucherWithLines {
    pub voucher: Voucher,
    pub lines: Vec<VoucherLine>,
    pub debit_total: String,
    pub credit_total: String,
    pub difference: String,
    pub is_balanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct VoucherSummary {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub voucher_type_name: String,
    pub series_id: String,
    pub series_name: String,
    pub voucher_number: String,
    pub date: String,
    pub status: String,
    pub total_amount: Option<String>,
    pub line_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VoucherLineInput {
    pub id: Option<String>,
    pub ledger_id: String,
    pub entry_type: String, // "Dr" or "Cr"
    pub amount: String,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVoucherDraftRequest {
    pub voucher_type_id: String,
    pub series_id: String,
    pub date: String,
    pub effective_date: Option<String>,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub lines: Option<Vec<VoucherLineInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateVoucherDraftRequest {
    pub voucher_type_id: Option<String>,
    pub series_id: Option<String>,
    pub date: Option<String>,
    pub effective_date: Option<String>,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddVoucherLineRequest {
    pub ledger_id: String,
    pub entry_type: String, // "Dr" or "Cr"
    pub amount: String,
    pub narration: Option<String>,
    pub line_number: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateVoucherLineRequest {
    pub ledger_id: Option<String>,
    pub entry_type: Option<String>,
    pub amount: Option<String>,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderVoucherLinesRequest {
    pub line_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveVoucherDraftRequest {
    pub voucher_id: Option<String>,
    pub voucher_type_id: String,
    pub series_id: String,
    pub date: String,
    pub effective_date: Option<String>,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub lines: Vec<VoucherLineInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostVoucherRequest {
    pub voucher_id: String,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelVoucherRequest {
    pub voucher_id: String,
    pub reason: String,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverseVoucherRequest {
    pub original_voucher_id: String,
    pub reversal_date: String,
    pub reason: String,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReversalResult {
    pub original_voucher: VoucherWithLines,
    pub reversing_voucher: VoucherWithLines,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCorrectionDraftRequest {
    pub original_voucher_id: String,
    pub reversal_date: Option<String>,
    pub reversal_reason: String,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionResult {
    pub original_voucher: VoucherWithLines,
    pub reversing_voucher: VoucherWithLines,
    pub corrected_draft: VoucherWithLines,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VoucherFilter {
    pub voucher_type_id: Option<String>,
    pub series_id: Option<String>,
    pub status: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

// ============================================================================
// Sprint 2 Stage 5A: Double-Entry Validation Models & Traits
// ============================================================================

pub trait DoubleEntryLine {
    fn line_number(&self) -> Option<i64>;
    fn ledger_id(&self) -> &str;
    fn entry_type(&self) -> &str; // "Dr" or "Cr"
    fn amount_str(&self) -> &str;
}

impl DoubleEntryLine for VoucherLine {
    fn line_number(&self) -> Option<i64> {
        Some(self.line_number)
    }
    fn ledger_id(&self) -> &str {
        &self.ledger_id
    }
    fn entry_type(&self) -> &str {
        &self.entry_type
    }
    fn amount_str(&self) -> &str {
        &self.amount
    }
}

impl DoubleEntryLine for VoucherLineInput {
    fn line_number(&self) -> Option<i64> {
        None
    }
    fn ledger_id(&self) -> &str {
        &self.ledger_id
    }
    fn entry_type(&self) -> &str {
        &self.entry_type
    }
    fn amount_str(&self) -> &str {
        &self.amount
    }
}

impl DoubleEntryLine for AddVoucherLineRequest {
    fn line_number(&self) -> Option<i64> {
        self.line_number
    }
    fn ledger_id(&self) -> &str {
        &self.ledger_id
    }
    fn entry_type(&self) -> &str {
        &self.entry_type
    }
    fn amount_str(&self) -> &str {
        &self.amount
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    Draft,
    Posting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationDiagnostic {
    pub code: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub line_number: Option<i64>,
    pub ledger_id: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoubleEntryValidationResult {
    pub is_valid: bool,
    pub is_balanced: bool,
    pub line_count: usize,
    pub debit_line_count: usize,
    pub credit_line_count: usize,
    pub debit_total: rust_decimal::Decimal,
    pub credit_total: rust_decimal::Decimal,
    pub difference: rust_decimal::Decimal,
    pub single_side_total: Option<rust_decimal::Decimal>,
    pub diagnostics: Vec<ValidationDiagnostic>,
}

// ============================================================================
// Sprint 2 Stage 7: Voucher Search & Register Foundation Models
// ============================================================================

/// Date basis for filtering vouchers in search and register queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateBasis {
    /// Filter against `vouchers.date` (transaction calendar date). Default.
    #[default]
    TransactionDate,
    /// Filter against `vouchers.effective_date` (posting/financial effective date).
    EffectiveDate,
}

/// Permitted sort fields for voucher register search queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoucherSortField {
    /// Sort by date (transaction or effective based on date_basis). Default.
    #[default]
    Date,
    /// Sort by alphanumeric voucher number.
    VoucherNumber,
    /// Sort by canonical total amount.
    TotalAmount,
    /// Sort by record creation timestamp.
    CreatedAt,
}

/// Sort direction for register search queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    /// Ascending order.
    Asc,
    /// Descending order. Default.
    #[default]
    Desc,
}

/// Parameter filter payload for searching and paginating vouchers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VoucherSearchFilter {
    /// Optional voucher type ID filter.
    pub voucher_type_id: Option<String>,
    /// Optional numbering series ID filter.
    pub series_id: Option<String>,
    /// Optional status filter ("posted" | "reversed" | "cancelled" | "draft" | "all").
    pub status: Option<String>,
    /// Optional ledger ID filter (vouchers touching this ledger on any line).
    pub ledger_id: Option<String>,
    /// Date basis selection (`TransactionDate` vs `EffectiveDate`).
    pub date_basis: Option<DateBasis>,
    /// Start date boundary in ISO 8601 YYYY-MM-DD.
    pub date_from: Option<String>,
    /// End date boundary in ISO 8601 YYYY-MM-DD.
    pub date_to: Option<String>,
    /// Minimum monetary amount string (canonical decimal).
    pub min_amount: Option<String>,
    /// Maximum monetary amount string (canonical decimal).
    pub max_amount: Option<String>,
    /// Text search query across voucher number, reference number, and narration (max 200 chars).
    pub search_query: Option<String>,
    /// Filter vouchers that are reversals of the specified voucher ID.
    pub reversal_of_voucher_id: Option<String>,
    /// 1-indexed page number (default 1).
    pub page: Option<u32>,
    /// Page size (default 20, max 100).
    pub page_size: Option<u32>,
    /// Field to sort by.
    pub sort_by: Option<VoucherSortField>,
    /// Sort direction (`Asc` or `Desc`).
    pub sort_direction: Option<SortDirection>,
}

/// Summary voucher item returned in paginated register listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherRegisterItem {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub voucher_type_name: String,
    pub series_id: String,
    pub series_name: String,
    pub voucher_number: String,
    pub date: String,
    pub effective_date: String,
    pub status: String,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub total_amount: Option<String>,
    pub line_count: i64,
    pub reversal_of_voucher_id: Option<String>,
    pub reversal_voucher_id: Option<String>,
    pub cancellation_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lineage graph response containing the chain of related vouchers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherLineageResponse {
    pub items: Vec<VoucherRegisterItem>,
    pub has_exceeded_depth_limit: bool,
    pub max_depth_reached: u32,
}
