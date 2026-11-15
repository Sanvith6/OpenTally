use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Inclusive calendar period used by all Stage 8 reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportDateRange {
    pub date_from: String,
    pub date_to: String,
}

impl ReportDateRange {
    pub fn new(date_from: impl Into<String>, date_to: impl Into<String>) -> Result<Self, String> {
        let date_from = date_from.into();
        let date_to = date_to.into();
        let from = parse_report_date(&date_from)?;
        let to = parse_report_date(&date_to)?;
        if from > to {
            return Err("Report start date cannot be after the end date".to_string());
        }
        Ok(Self { date_from, date_to })
    }
}

pub(crate) fn parse_report_date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("Invalid report date '{}'; expected YYYY-MM-DD", value))
}

/// Shared filter contract. `ledger_id = None` means the complete company.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportFilter {
    pub period: ReportDateRange,
    pub ledger_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPageRequest {
    pub page: usize,
    pub page_size: usize,
}

impl ReportPageRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0 {
            return Err("Report page must be at least 1".to_string());
        }
        if !(1..=100).contains(&self.page_size) {
            return Err("Report page size must be between 1 and 100".to_string());
        }
        self.page
            .checked_sub(1)
            .and_then(|page| page.checked_mul(self.page_size))
            .ok_or_else(|| "Report pagination offset overflow".to_string())?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportPage<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBookRequest {
    pub filter: ReportFilter,
    pub voucher_type_id: Option<String>,
    pub voucher_number_query: Option<String>,
    pub page: ReportPageRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBookLine {
    pub ledger_id: String,
    pub ledger_name: String,
    pub debit: String,
    pub credit: String,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayBookEntry {
    pub voucher_id: String,
    pub date: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub narration: Option<String>,
    pub debit_total: String,
    pub credit_total: String,
    pub voucher_total: String,
    pub lines: Vec<DayBookLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerReportRequest {
    pub filter: ReportFilter,
    pub page: ReportPageRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerReportTransaction {
    pub voucher_id: String,
    pub date: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub particulars: Option<String>,
    pub debit: String,
    pub credit: String,
    pub running_balance: String,
    pub running_balance_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerReport {
    pub ledger_id: String,
    pub ledger_name: String,
    pub ledger_nature: String,
    pub opening_balance: String,
    pub opening_balance_type: String,
    pub transactions: ReportPage<LedgerReportTransaction>,
    pub closing_balance: String,
    pub closing_balance_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialBalanceRequest {
    pub as_of_date: String,
    pub account_group_id: Option<String>,
    pub ledger_id: Option<String>,
    pub account_search: Option<String>,
    pub include_zero_balances: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialBalanceRow {
    pub ledger_id: String,
    pub ledger_name: String,
    pub account_group_id: String,
    pub account_group_name: String,
    pub account_group_nature: String,
    pub opening_debit: String,
    pub opening_credit: String,
    pub period_debit: String,
    pub period_credit: String,
    pub closing_debit: String,
    pub closing_credit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialBalanceReport {
    pub as_of_date: String,
    pub rows: Vec<TrialBalanceRow>,
    pub total_debit: String,
    pub total_credit: String,
    pub difference: String,
    pub ledger_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfitAndLossRequest {
    pub period: ReportDateRange,
    pub account_group_id: Option<String>,
    pub ledger_search: Option<String>,
    pub include_zero_balances: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfitAndLossLedgerRow {
    pub ledger_id: String,
    pub ledger_name: String,
    pub account_group_id: String,
    pub account_group_name: String,
    pub account_group_nature: String,
    pub is_direct: bool,
    pub debit: String,
    pub credit: String,
    pub contribution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfitAndLossReport {
    pub company_id: String,
    pub date_from: String,
    pub date_to: String,
    pub direct_income_rows: Vec<ProfitAndLossLedgerRow>,
    pub direct_income_subtotal: String,
    pub direct_expense_rows: Vec<ProfitAndLossLedgerRow>,
    pub direct_expense_subtotal: String,
    pub gross_profit_or_loss: String,
    pub gross_result_type: String,
    pub indirect_income_rows: Vec<ProfitAndLossLedgerRow>,
    pub indirect_income_subtotal: String,
    pub indirect_expense_rows: Vec<ProfitAndLossLedgerRow>,
    pub indirect_expense_subtotal: String,
    pub net_profit_or_loss: String,
    pub net_result_type: String,
    pub ledger_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceSheetRequest {
    pub as_of_date: String,
    pub account_group_id: Option<String>,
    pub ledger_search: Option<String>,
    pub include_zero_balances: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceSheetLedgerRow {
    pub ledger_id: String,
    pub ledger_name: String,
    pub account_group_id: String,
    pub account_group_name: String,
    pub classification: String,
    pub debit: String,
    pub credit: String,
    pub net_balance: String,
    pub balance_type: String,
    pub presentation_amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceSheetReport {
    pub company_id: String,
    pub as_of_date: String,
    pub asset_rows: Vec<BalanceSheetLedgerRow>,
    pub total_assets: String,
    pub liability_rows: Vec<BalanceSheetLedgerRow>,
    pub total_liabilities: String,
    pub equity_rows: Vec<BalanceSheetLedgerRow>,
    pub current_period_profit_or_loss: String,
    pub current_period_result_type: String,
    pub total_equity: String,
    pub total_liabilities_and_equity: String,
    pub balance_difference: String,
    pub is_balanced: bool,
    pub ledger_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashFlowRequest {
    pub period: ReportDateRange,
    pub cash_ledger_id: Option<String>,
    pub activity_filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashFlowRow {
    pub activity: String,
    pub ledger_id: String,
    pub ledger_name: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub date: String,
    pub description: Option<String>,
    pub amount: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashFlowReport {
    pub company_id: String,
    pub date_from: String,
    pub date_to: String,
    pub opening_cash_balance: String,
    pub operating_rows: Vec<CashFlowRow>,
    pub operating_subtotal: String,
    pub investing_rows: Vec<CashFlowRow>,
    pub investing_subtotal: String,
    pub financing_rows: Vec<CashFlowRow>,
    pub financing_subtotal: String,
    pub net_change_in_cash: String,
    pub closing_cash_balance: String,
    pub actual_closing_cash_balance: String,
    pub reconciliation_difference: String,
    pub is_reconciled: bool,
    pub ledger_count: usize,
    pub warnings: Vec<String>,
}

/// A posted financial line in deterministic voucher/line order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReportMovement {
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub voucher_type_name: String,
    pub status: String,
    pub ledger_id: String,
    pub ledger_name: String,
    pub line_number: i64,
    pub entry_type: String,
    pub amount: String,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReportOpeningBalance {
    pub ledger_id: String,
    pub balance_type: String,
    pub amount: String,
}
