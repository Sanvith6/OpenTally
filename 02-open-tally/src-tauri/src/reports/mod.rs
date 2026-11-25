//! Stage 8A reporting foundation.
//!
//! This module owns shared report filters, read-only financial movement queries,
//! and exact-decimal balance accumulation. Individual reports are deliberately
//! deferred to later Stage 8 increments.

pub mod balance_sheet;
pub mod billwise;
pub mod calculation;
pub mod cash_flow;
pub mod day_book;
pub mod gst;
pub mod ledger_report;
pub mod model;
pub mod orders;
pub mod payroll;
pub mod profit_and_loss;
pub mod query;
pub mod ratios;
pub mod tds;
pub mod trial_balance;

pub use balance_sheet::BalanceSheetService;
pub use billwise::BillWiseReportService;
pub use calculation::{calculate_balance, parse_report_amount, BalanceAccumulator};
pub use cash_flow::CashFlowService;
pub use day_book::DayBookService;
pub use gst::GstReportService;
pub use ledger_report::LedgerReportService;
pub use model::{
    BalanceSheetLedgerRow, BalanceSheetReport, BalanceSheetRequest, CashFlowReport,
    CashFlowRequest, CashFlowRow, DayBookEntry, DayBookLine, DayBookRequest, LedgerReport,
    LedgerReportRequest, LedgerReportTransaction, ProfitAndLossLedgerRow, ProfitAndLossReport,
    ProfitAndLossRequest, ReportDateRange, ReportFilter, ReportMovement, ReportOpeningBalance,
    ReportPage, ReportPageRequest, TrialBalanceReport, TrialBalanceRequest, TrialBalanceRow,
};
pub use orders::OrderOutstandingReportService;
pub use payroll::PayrollReportService;
pub use profit_and_loss::ProfitAndLossService;
pub use query::ReportQueryService;
pub use ratios::{RatioAnalysisReport, RatioAnalysisService};
pub use tds::TdsReportService;
pub use trial_balance::TrialBalanceService;

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
