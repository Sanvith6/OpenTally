use super::calculation::{calculate_balance, parse_report_amount};
use super::model::{ReportDateRange, ReportMovement, ReportOpeningBalance};
use rust_decimal::Decimal;
use std::str::FromStr;

fn movement(entry_type: &str, amount: &str) -> ReportMovement {
    ReportMovement {
        voucher_id: "v1".to_string(),
        voucher_number: "1".to_string(),
        voucher_date: "2026-04-01".to_string(),
        voucher_type_name: "Journal".to_string(),
        status: "posted".to_string(),
        ledger_id: "ledger-1".to_string(),
        ledger_name: "Test Ledger".to_string(),
        line_number: 1,
        entry_type: entry_type.to_string(),
        amount: amount.to_string(),
        narration: None,
    }
}

#[test]
fn date_range_is_inclusive_and_validated() {
    assert!(ReportDateRange::new("2026-04-01", "2026-04-30").is_ok());
    assert!(ReportDateRange::new("2026-04-30", "2026-04-01").is_err());
    assert!(ReportDateRange::new("2026-02-30", "2026-03-01").is_err());
}

#[test]
fn balance_calculation_preserves_exact_decimal_arithmetic() {
    let result = calculate_balance(
        &[ReportOpeningBalance {
            ledger_id: "ledger-1".to_string(),
            balance_type: "Dr".to_string(),
            amount: "10000000000000000000.9999".to_string(),
        }],
        &[movement("Dr", "0.0001"), movement("Cr", "20.25")],
    )
    .expect("valid balance");

    assert_eq!(
        result.opening_debit,
        Decimal::from_str("10000000000000000000.9999").unwrap()
    );
    assert_eq!(result.period_debit, Decimal::new(1, 4));
    assert_eq!(result.period_credit, Decimal::new(2025, 2));
    assert_eq!(result.closing_credit, Decimal::ZERO);
    assert_eq!(
        result.closing_debit,
        Decimal::from_str("9999999999999999980.7500").unwrap()
    );
}

#[test]
fn credit_excess_produces_credit_closing_balance() {
    let result = calculate_balance(&[], &[movement("Cr", "75.50")]).expect("valid balance");
    assert_eq!(result.closing_debit, Decimal::ZERO);
    assert_eq!(result.closing_credit, Decimal::new(7550, 2));
}

#[test]
fn invalid_entry_and_amounts_fail_closed() {
    assert!(calculate_balance(&[], &[movement("Debit", "1.00")]).is_err());
    assert!(parse_report_amount("not-a-decimal").is_err());
}
