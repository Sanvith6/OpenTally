//! Sprint 2 Stage 5A: Double-Entry Validation Subsystem Test Suite
//! Tests pure double-entry arithmetic validation, invariants, and contextual ledger integrity.

use crate::database::DatabaseManager;
use crate::vouchers::model::{DoubleEntryLine, ValidationMode, ValidationSeverity};
use crate::vouchers::service::DoubleEntryValidator;
use crate::vouchers::validator::validate_voucher_ledger_integrity;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestLeg {
    pub line_number: Option<i64>,
    pub ledger_id: String,
    pub entry_type: String,
    pub amount: String,
}

impl DoubleEntryLine for TestLeg {
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

fn leg(line_no: i64, ledger: &str, entry: &str, amt: &str) -> TestLeg {
    TestLeg {
        line_number: Some(line_no),
        ledger_id: ledger.to_string(),
        entry_type: entry.to_string(),
        amount: amt.to_string(),
    }
}

/// TC-VAL-01: Balanced 2-leg entry (Dr 1000.00, Cr 1000.00)
#[test]
fn test_tc_val_01_balanced_two_leg_voucher() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1000.00"),
        leg(2, "led_sales", "Cr", "1000.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(res.is_valid);
    assert!(res.is_balanced);
    assert_eq!(res.line_count, 2);
    assert_eq!(res.debit_line_count, 1);
    assert_eq!(res.credit_line_count, 1);
    assert_eq!(res.debit_total, Decimal::from_str("1000.00").unwrap());
    assert_eq!(res.credit_total, Decimal::from_str("1000.00").unwrap());
    assert_eq!(res.difference, Decimal::ZERO);
    assert_eq!(
        res.single_side_total,
        Some(Decimal::from_str("1000.00").unwrap())
    );
    assert!(res.diagnostics.is_empty());
}

/// TC-VAL-02: Imbalance (Dr > Cr) in Mode::Draft permits draft saving with warnings
#[test]
fn test_tc_val_02_imbalance_dr_greater_in_draft_mode() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1500.00"),
        leg(2, "led_sales", "Cr", "1000.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Draft);

    assert!(
        res.is_valid,
        "Draft mode must remain valid even when unbalanced"
    );
    assert!(!res.is_balanced);
    assert_eq!(res.debit_total, Decimal::from_str("1500.00").unwrap());
    assert_eq!(res.credit_total, Decimal::from_str("1000.00").unwrap());
    assert_eq!(res.difference, Decimal::from_str("500.00").unwrap());
    assert_eq!(res.single_side_total, None);

    // Warning diagnostic expected
    let warning = res
        .diagnostics
        .iter()
        .find(|d| d.code == "UNBALANCED_TOTALS");
    assert!(warning.is_some());
    assert_eq!(warning.unwrap().severity, ValidationSeverity::Warning);
}

/// TC-VAL-03: Imbalance (Dr > Cr) in Mode::Posting is rejected with fatal error
#[test]
fn test_tc_val_03_imbalance_dr_greater_in_posting_mode() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1500.00"),
        leg(2, "led_sales", "Cr", "1000.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid, "Posting must fail closed on imbalance");
    assert!(!res.is_balanced);
    assert_eq!(res.difference, Decimal::from_str("500.00").unwrap());
    assert_eq!(res.single_side_total, None);

    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "UNBALANCED_TOTALS");
    assert!(err.is_some());
    assert_eq!(err.unwrap().severity, ValidationSeverity::Error);
}

/// TC-VAL-04: Imbalance (Cr > Dr) in Mode::Posting is rejected with fatal error
#[test]
fn test_tc_val_04_imbalance_cr_greater_in_posting_mode() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1000.00"),
        leg(2, "led_sales", "Cr", "1250.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    assert!(!res.is_balanced);
    assert_eq!(res.difference, Decimal::from_str("250.00").unwrap());
    assert_eq!(res.single_side_total, None);

    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "UNBALANCED_TOTALS");
    assert!(err.is_some());
    assert_eq!(err.unwrap().severity, ValidationSeverity::Error);
}

/// TC-VAL-05: Empty voucher in Mode::Draft
#[test]
fn test_tc_val_05_empty_voucher_in_draft_mode() {
    let lines: Vec<TestLeg> = vec![];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Draft);

    assert!(res.is_valid, "Empty draft is permitted during composition");
    assert!(!res.is_balanced);
    assert_eq!(res.line_count, 0);
    assert_eq!(res.debit_total, Decimal::ZERO);
    assert_eq!(res.credit_total, Decimal::ZERO);
    assert_eq!(res.difference, Decimal::ZERO);
    assert_eq!(res.single_side_total, None);

    let warning = res.diagnostics.iter().find(|d| d.code == "EMPTY_VOUCHER");
    assert!(warning.is_some());
    assert_eq!(warning.unwrap().severity, ValidationSeverity::Warning);
}

/// TC-VAL-06: Empty voucher in Mode::Posting is strictly rejected
#[test]
fn test_tc_val_06_empty_voucher_in_posting_mode() {
    let lines: Vec<TestLeg> = vec![];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    assert!(!res.is_balanced);
    let err = res.diagnostics.iter().find(|d| d.code == "EMPTY_VOUCHER");
    assert!(err.is_some());
    assert_eq!(err.unwrap().severity, ValidationSeverity::Error);
}

/// TC-VAL-07: Single-sided voucher (all Dr legs)
#[test]
fn test_tc_val_07_single_sided_voucher_rejected() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "500.00"),
        leg(2, "led_cash", "Dr", "500.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    assert!(!res.is_balanced);
    assert_eq!(res.debit_line_count, 2);
    assert_eq!(res.credit_line_count, 0);

    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "SINGLE_SIDED_VOUCHER");
    assert!(err.is_some());
    assert_eq!(err.unwrap().severity, ValidationSeverity::Error);
}

/// TC-VAL-08: Multi-leg compound entry (3 Dr legs = 2 Cr legs)
#[test]
fn test_tc_val_08_multi_leg_compound_entry() {
    let lines = vec![
        leg(1, "led_cash", "Dr", "100.00"),
        leg(2, "led_bank", "Dr", "200.00"),
        leg(3, "led_discount", "Dr", "300.00"),
        leg(4, "led_debtor_a", "Cr", "400.00"),
        leg(5, "led_debtor_b", "Cr", "200.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(res.is_valid);
    assert!(res.is_balanced);
    assert_eq!(res.line_count, 5);
    assert_eq!(res.debit_line_count, 3);
    assert_eq!(res.credit_line_count, 2);
    assert_eq!(res.debit_total, Decimal::from_str("600.00").unwrap());
    assert_eq!(res.credit_total, Decimal::from_str("600.00").unwrap());
    assert_eq!(res.difference, Decimal::ZERO);
    assert_eq!(
        res.single_side_total,
        Some(Decimal::from_str("600.00").unwrap())
    );
    assert!(res.diagnostics.is_empty());
}

/// TC-VAL-09: Zero amount line rejected
#[test]
fn test_tc_val_09_zero_amount_line_rejected() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "0.00"),
        leg(2, "led_sales", "Cr", "0.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let zero_errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.code == "ZERO_AMOUNT")
        .collect();
    assert_eq!(zero_errs.len(), 2);
}

/// TC-VAL-10: Negative amount line rejected
#[test]
fn test_tc_val_10_negative_amount_line_rejected() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "-100.00"),
        leg(2, "led_sales", "Cr", "100.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let neg_err = res.diagnostics.iter().find(|d| d.code == "NEGATIVE_AMOUNT");
    assert!(neg_err.is_some());
    assert_eq!(neg_err.unwrap().line_number, Some(1));
}

/// TC-VAL-11: Scale violation (3 decimals when precision is 2)
#[test]
fn test_tc_val_11_scale_exceeding_company_precision_rejected() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "100.555"),
        leg(2, "led_sales", "Cr", "100.555"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let scale_errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.code == "SCALE_EXCEEDS_PRECISION")
        .collect();
    assert_eq!(scale_errs.len(), 2);
}

/// TC-VAL-12: Max database scale (4 decimals when precision is 4)
#[test]
fn test_tc_val_12_max_database_scale_accepted_at_precision_4() {
    let lines = vec![
        leg(1, "led_forex", "Dr", "100.1234"),
        leg(2, "led_sales", "Cr", "100.1234"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 4, ValidationMode::Posting);

    assert!(res.is_valid);
    assert!(res.is_balanced);
    assert_eq!(res.debit_total, Decimal::from_str("100.1234").unwrap());
    assert_eq!(res.credit_total, Decimal::from_str("100.1234").unwrap());
}

/// TC-VAL-13: Sub-unit scale overflow (5 decimal places exceeds DB max 4)
#[test]
fn test_tc_val_13_scale_overflow_exceeds_database_max_of_4() {
    let lines = vec![
        leg(1, "led_forex", "Dr", "100.12345"),
        leg(2, "led_sales", "Cr", "100.12345"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 4, ValidationMode::Posting);

    assert!(!res.is_valid);
    assert!(!res.is_balanced);
    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "INVALID_MONETARY_STRING");
    assert!(err.is_some());
}

/// TC-VAL-14: Large numbers boundary (10,000 lines of large amounts)
#[test]
fn test_tc_val_14_large_numbers_boundary_no_precision_loss() {
    let mut lines = Vec::with_capacity(10_000);
    let amount_str = "9999999999999999.99"; // 16 digits + 2 dec = 18 digits (within 25 char limit)

    // 5000 Debit legs
    for i in 1..=5000 {
        lines.push(leg(i, "led_asset", "Dr", amount_str));
    }
    // 5000 Credit legs
    for i in 5001..=10000 {
        lines.push(leg(i, "led_equity", "Cr", amount_str));
    }

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(res.is_valid);
    assert!(res.is_balanced);
    assert_eq!(res.line_count, 10_000);
    assert_eq!(res.debit_line_count, 5000);
    assert_eq!(res.credit_line_count, 5000);

    let expected_single =
        Decimal::from_str(amount_str).unwrap() * Decimal::from_str("5000").unwrap();
    assert_eq!(res.debit_total, expected_single);
    assert_eq!(res.credit_total, expected_single);
    assert_eq!(res.difference, Decimal::ZERO);
}

/// TC-VAL-15: Arithmetic overflow handling past Decimal::MAX
#[test]
fn test_tc_val_15_arithmetic_overflow_detected_cleanly() {
    let mut lines = Vec::with_capacity(7924);
    let amount_str = "9999999999999999999999999"; // Exactly 25 digits (within 25 char limit)

    // Decimal::MAX is ~7.9228 x 10^28.
    // 7922 * 10^25 < Decimal::MAX; 7923 * 10^25 overflows Decimal::MAX.
    for i in 1..=7923 {
        lines.push(leg(i, "led_big", "Dr", amount_str));
    }
    lines.push(leg(7924, "led_offset", "Cr", "100.00"));

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let of_err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "ARITHMETIC_OVERFLOW");
    assert!(of_err.is_some(), "Expected ARITHMETIC_OVERFLOW diagnostic");
    assert_eq!(of_err.unwrap().line_number, Some(7923));
}

/// TC-VAL-16: Input immutability assertion
#[test]
fn test_tc_val_16_input_lines_remain_immutable() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1000.00"),
        leg(2, "led_sales", "Cr", "1000.00"),
    ];
    let original = lines.clone();

    let _ = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert_eq!(lines, original, "Input lines must not be mutated");
}

// ============================================================================
// Contextual Database Tests (TC-VAL-17 & TC-VAL-18)
// ============================================================================

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage5a_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");
        Self {
            db,
            _temp_dir: temp_dir,
        }
    }
}

/// TC-VAL-17: Contextual check rejects reserved Profit & Loss ledger
#[tokio::test]
async fn test_tc_val_17_reserved_system_pnl_ledger_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();

    // Run migrations to 0.3.1
    crate::database::migrations::run_sprint1_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_stage3_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_stage4_hardening_migration(pool)
        .await
        .unwrap();

    let req = crate::company::model::CreateCompanyRequest {
        name: "Stage 5A PnL Co".to_string(),
        mailing_name: Some("Stage 5A PnL Co".to_string()),
        country: Some("India".to_string()),
        state: Some("Maharashtra".to_string()),
        financial_year_begins: "2026-04-01".to_string(),
        books_beginning_from: Some("2026-04-01".to_string()),
        currency_symbol: Some("₹".to_string()),
        currency_formal_name: Some("INR".to_string()),
        currency_decimal_places: Some(2),
    };
    let company = crate::company::CompanyService::create_company(pool, req)
        .await
        .unwrap();

    let pnl_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = $1 AND ledger_classification = 'profit_and_loss'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    let cash_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = $1 AND ledger_classification = 'cash'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();

    let bad_lines = vec![
        leg(1, &cash_id, "Dr", "100.00"),
        leg(2, &pnl_id, "Cr", "100.00"),
    ];

    let err = validate_voucher_ledger_integrity(&mut conn, &company.id, &bad_lines)
        .await
        .expect_err("Must reject posting to reserved profit_and_loss ledger");

    assert!(
        err.to_string()
            .contains("reserved Profit & Loss system ledger"),
        "Unexpected error: {:?}",
        err
    );
}

/// TC-VAL-18: Contextual check rejects nonexistent ledger
#[tokio::test]
async fn test_tc_val_18_nonexistent_ledger_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();

    crate::database::migrations::run_sprint1_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_stage3_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_migration(pool)
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_stage4_hardening_migration(pool)
        .await
        .unwrap();

    let req = crate::company::model::CreateCompanyRequest {
        name: "Stage 5A Nonexistent Co".to_string(),
        mailing_name: Some("Stage 5A Nonexistent Co".to_string()),
        country: Some("India".to_string()),
        state: Some("Maharashtra".to_string()),
        financial_year_begins: "2026-04-01".to_string(),
        books_beginning_from: Some("2026-04-01".to_string()),
        currency_symbol: Some("₹".to_string()),
        currency_formal_name: Some("INR".to_string()),
        currency_decimal_places: Some(2),
    };
    let company = crate::company::CompanyService::create_company(pool, req)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();

    let bad_lines = vec![
        leg(1, "led_ghost_1", "Dr", "100.00"),
        leg(2, "led_ghost_2", "Cr", "100.00"),
    ];

    let err = validate_voucher_ledger_integrity(&mut conn, &company.id, &bad_lines)
        .await
        .expect_err("Must reject nonexistent ledger");

    assert!(
        matches!(err, crate::errors::OpenTallyError::NotFound(_)),
        "Expected NotFound error, got {:?}",
        err
    );
}
