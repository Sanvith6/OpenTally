//! Stage 8A reporting foundation.
//!
//! This module owns shared report filters, read-only financial movement queries,
//! and exact-decimal balance accumulation. Individual reports are deliberately
//! deferred to later Stage 8 increments.

pub mod balance_sheet;
pub mod calculation;
pub mod cash_flow;
pub mod day_book;
pub mod ledger_report;
pub mod model;
pub mod profit_and_loss;
pub mod query;
pub mod trial_balance;

pub use balance_sheet::BalanceSheetService;
pub use calculation::{calculate_balance, parse_report_amount, BalanceAccumulator};
pub use cash_flow::CashFlowService;
pub use day_book::DayBookService;
pub use ledger_report::LedgerReportService;
pub use model::{
    BalanceSheetLedgerRow, BalanceSheetReport, BalanceSheetRequest, CashFlowReport,
    CashFlowRequest, CashFlowRow, DayBookEntry, DayBookLine, DayBookRequest, LedgerReport,
    LedgerReportRequest, LedgerReportTransaction, ProfitAndLossLedgerRow, ProfitAndLossReport,
    ProfitAndLossRequest, ReportDateRange, ReportFilter, ReportMovement, ReportOpeningBalance,
    ReportPage, ReportPageRequest, TrialBalanceReport, TrialBalanceRequest, TrialBalanceRow,
};
pub use profit_and_loss::ProfitAndLossService;
pub use query::ReportQueryService;
pub use trial_balance::TrialBalanceService;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
