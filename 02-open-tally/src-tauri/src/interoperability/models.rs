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
pub struct TallyXmlVoucherLine {
    pub ledger_name: String,
    pub entry_type: String, // "Dr" or "Cr"
    pub amount: Decimal,    // Strictly positive
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TallyXmlVoucher {
    pub voucher_type: String,
    pub voucher_number: String,
    pub date: String, // YYYY-MM-DD
    pub narration: Option<String>,
    pub lines: Vec<TallyXmlVoucherLine>,
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
pub struct TallyXmlImportResult {
    pub success: bool,
    pub groups_created: usize,
    pub ledgers_created: usize,
    pub stock_items_created: usize,
    pub vouchers_created: usize,
    pub message: String,
}
