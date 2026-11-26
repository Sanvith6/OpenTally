use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currency {
    pub id: String,
    pub company_id: String,
    pub code: String,
    pub symbol: String,
    pub formal_name: String,
    pub decimal_places: i64,
    pub is_base: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCurrencyInput {
    pub code: String,
    pub symbol: String,
    pub formal_name: String,
    pub decimal_places: Option<i64>,
    pub is_base: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub id: String,
    pub company_id: String,
    pub currency_code: String,
    pub effective_date: String,
    pub standard_rate: String,
    pub selling_rate: String,
    pub buying_rate: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetExchangeRateInput {
    pub currency_code: String,
    pub effective_date: String,
    pub standard_rate: String,
    pub selling_rate: Option<String>,
    pub buying_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveRateDto {
    pub currency_code: String,
    pub effective_date: String,
    pub standard_rate: Decimal,
    pub selling_rate: Decimal,
    pub buying_rate: Decimal,
    pub is_base: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherCurrencyDetail {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub voucher_line_id: Option<String>,
    pub currency_code: String,
    pub foreign_amount: String,
    pub exchange_rate: String,
    pub base_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenForeignBill {
    pub bill_name: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub bill_date: String,
    pub due_date: Option<String>,
    pub currency_code: String,
    pub invoice_rate: Decimal,
    pub original_foreign: Decimal,
    pub settled_foreign: Decimal,
    pub outstanding_foreign: Decimal,
    pub original_inr: Decimal,
    pub settled_inr: Decimal,
    pub outstanding_inr: Decimal,
    pub dr_cr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettleForeignBillInput {
    pub bill_name: String,
    pub party_ledger_id: String,
    pub foreign_settled_amount: String,
    pub settlement_date: String,
    pub settlement_exchange_rate: Option<String>,
    pub payment_mode: String,
    pub bank_or_cash_ledger_id: String,
    pub narration: Option<String>,
    pub cheque_instrument_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealizedForexResult {
    pub settlement_voucher_id: String,
    pub forex_journal_voucher_id: Option<String>,
    pub foreign_settled: Decimal,
    pub settlement_rate: Decimal,
    pub invoice_rate: Decimal,
    pub forex_difference: Decimal,
    pub is_gain: bool,
    pub settled_inr: Decimal,
    pub book_inr: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnrealizedForexBillRow {
    pub bill_name: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub currency_code: String,
    pub dr_cr: String,
    pub open_foreign_amount: Decimal,
    pub invoice_rate: Decimal,
    pub book_value_inr: Decimal,
    pub revaluation_rate: Decimal,
    pub revalued_value_inr: Decimal,
    pub unrealized_gain_or_loss: Decimal,
    pub is_gain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnrealizedForexReport {
    pub company_id: String,
    pub as_of_date: String,
    pub bills: Vec<UnrealizedForexBillRow>,
    pub total_book_value_inr: Decimal,
    pub total_revalued_value_inr: Decimal,
    pub total_unrealized_gain: Decimal,
    pub total_unrealized_loss: Decimal,
    pub net_unrealized_gain_loss: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertChequeTemplateInput {
    pub template_id: Option<String>,
    pub bank_name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub date_x_mm: f64,
    pub date_y_mm: f64,
    pub payee_x_mm: f64,
    pub payee_y_mm: f64,
    pub words_x_mm: f64,
    pub words_y_mm: f64,
    pub amount_x_mm: f64,
    pub amount_y_mm: f64,
    pub is_account_payee_default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencySeedSummary {
    pub base_currency_seeded: bool,
    pub currencies_seeded_count: usize,
    pub forex_ledger_id: String,
    pub cheque_templates_seeded_count: usize,
}
