use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BankAllocation {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub voucher_line_id: String,
    pub bank_ledger_id: String,
    pub transaction_type: String,
    pub instrument_number: Option<String>,
    pub instrument_date: Option<String>,
    pub favouring_name: Option<String>,
    pub bank_date: Option<String>,
    pub amount: String,
    pub dr_cr: String,
    pub is_cancelled: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAllocationInput {
    pub voucher_line_id: Option<String>,
    pub bank_ledger_id: Option<String>,
    pub transaction_type: Option<String>,
    pub instrument_number: Option<String>,
    pub instrument_date: Option<String>,
    pub favouring_name: Option<String>,
    pub bank_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileEntryInput {
    pub bank_allocation_id: String,
    pub bank_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileResult {
    pub success: bool,
    pub updated_count: usize,
    pub updated_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankReconciliationRow {
    pub bank_allocation_id: String,
    pub voucher_id: String,
    pub voucher_line_id: String,
    pub voucher_date: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub particulars: String,
    pub transaction_type: String,
    pub instrument_number: Option<String>,
    pub instrument_date: Option<String>,
    pub favouring_name: Option<String>,
    pub bank_date: Option<String>,
    pub debit_amount: String,
    pub credit_amount: String,
    pub is_reconciled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankReconciliationSummary {
    pub book_balance: String,
    pub book_balance_dr_cr: String,
    pub unreconciled_debit: String,
    pub unreconciled_credit: String,
    pub bank_balance: String,
    pub bank_balance_dr_cr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankReconciliationStatement {
    pub bank_ledger_id: String,
    pub bank_ledger_name: String,
    pub from_date: String,
    pub to_date: String,
    pub summary: BankReconciliationSummary,
    pub rows: Vec<BankReconciliationRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankLedgerItem {
    pub id: String,
    pub name: String,
    pub group_id: String,
    pub group_name: String,
    pub current_balance: String,
    pub current_balance_dr_cr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankStatementMatchRow {
    pub csv_row_index: usize,
    pub statement_date: String,
    pub statement_ref: String,
    pub statement_description: String,
    pub statement_amount: String,
    pub statement_dr_cr: String,
    pub matched_bank_allocation_id: Option<String>,
    pub matched_voucher_number: Option<String>,
    pub matched_particulars: Option<String>,
    pub suggested_bank_date: Option<String>,
    pub match_type: String, // 'exact', 'suggested', 'unmatched'
    pub confidence: String, // 'HIGH', 'MEDIUM', 'NONE'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankStatementMatchPreview {
    pub total_statement_rows: usize,
    pub exact_matches: usize,
    pub suggested_matches: usize,
    pub unmatched: usize,
    pub rows: Vec<BankStatementMatchRow>,
}
