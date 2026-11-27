use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlGroup {
    pub name: String,
    pub parent: Option<String>,
    pub is_calculatable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlLedger {
    pub name: String,
    pub parent: String,
    pub opening_balance: Decimal,
    pub is_deemed_positive: bool, // true = Dr, false = Cr
    #[serde(default)]
    pub party_gstin: Option<String>,
    #[serde(default)]
    pub state_name: Option<String>,
    #[serde(default)]
    pub pan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlUnit {
    pub symbol: String,
    pub formal_name: String,
    pub decimal_places: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlGodown {
    pub name: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlStockGroup {
    pub name: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlStockItem {
    pub name: String,
    pub parent: Option<String>,
    pub base_units: String,
    pub opening_balance_qty: Decimal,
    pub opening_balance_rate: Decimal,
    pub opening_balance_val: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlBillAllocation {
    pub bill_name: String,
    pub bill_type: String, // "NewRef" | "AgstRef" | "Advance" | "OnAccount"
    pub credit_period_days: Option<i64>,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlBatchAllocation {
    pub godown_name: Option<String>,
    pub batch_name: Option<String>,
    pub mfg_date: Option<String>,
    pub expiry_period: Option<String>,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlInventoryEntry {
    pub stock_item_name: String,
    pub actual_qty: Decimal,
    pub billed_qty: Decimal,
    pub unit_symbol: Option<String>,
    pub rate: Decimal,
    pub discount: Decimal,
    pub amount: Decimal,
    #[serde(default)]
    pub batches: Vec<TallyXmlBatchAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlBankAllocation {
    pub transaction_type: String,
    pub instrument_number: Option<String>,
    pub instrument_date: Option<String>,
    pub bank_date: Option<String>,
    pub favouring_name: Option<String>,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlCostAllocation {
    pub category_name: String,
    pub cost_centre_name: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlVoucherLine {
    pub ledger_name: String,
    pub entry_type: String, // "Dr" or "Cr"
    pub amount: Decimal,    // Strictly positive
    pub narration: Option<String>,
    #[serde(default)]
    pub bill_allocations: Vec<TallyXmlBillAllocation>,
    #[serde(default)]
    pub bank_allocations: Vec<TallyXmlBankAllocation>,
    #[serde(default)]
    pub cost_allocations: Vec<TallyXmlCostAllocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlVoucher {
    pub voucher_type: String,
    pub voucher_number: String,
    pub date: String, // YYYY-MM-DD
    pub narration: Option<String>,
    #[serde(default)]
    pub party_gstin: Option<String>,
    #[serde(default)]
    pub place_of_supply: Option<String>,
    pub lines: Vec<TallyXmlVoucherLine>,
    #[serde(default)]
    pub inventory_entries: Vec<TallyXmlInventoryEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct TallyXmlDocument {
    pub groups: Vec<TallyXmlGroup>,
    pub ledgers: Vec<TallyXmlLedger>,
    pub units: Vec<TallyXmlUnit>,
    pub godowns: Vec<TallyXmlGodown>,
    pub stock_groups: Vec<TallyXmlStockGroup>,
    pub stock_items: Vec<TallyXmlStockItem>,
    pub vouchers: Vec<TallyXmlVoucher>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlValidationError {
    pub item_type: String, // "VOUCHER", "GROUP", "LEDGER", "STOCK_ITEM", "DATE"
    pub identifier: String,
    pub message: String,
    pub severity: String, // "FATAL" or "WARNING"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlImportPreview {
    pub groups_to_create: usize,
    pub groups_skipped: usize,
    pub ledgers_to_create: usize,
    pub ledgers_skipped: usize,
    pub stock_items_to_create: usize,
    pub stock_items_skipped: usize,
    pub vouchers_to_create: usize,
    pub vouchers_skipped: usize,
    pub can_import: bool,
    pub errors: Vec<TallyXmlValidationError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlMigrationReconciliationReport {
    pub source_voucher_count: usize,
    pub imported_voucher_count: usize,
    pub source_total_debit: String,
    pub imported_total_debit: String,
    pub source_total_credit: String,
    pub imported_total_credit: String,
    pub imported_trial_balance_difference: String,
    pub bill_allocations_imported: usize,
    pub inventory_lines_imported: usize,
    pub bank_allocations_imported: usize,
    pub cost_centre_allocations_imported: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlImportResult {
    pub success: bool,
    pub groups_created: usize,
    pub ledgers_created: usize,
    pub stock_items_created: usize,
    pub vouchers_created: usize,
    pub message: String,
    #[serde(default)]
    pub reconciliation: Option<XmlMigrationReconciliationReport>,
}
