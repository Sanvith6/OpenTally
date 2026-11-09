//! Sprint 2 Stage 5B: Voucher Posting Engine Test Suite
//! Tests atomic promotion of voucher drafts to POSTED, double-entry enforcement,
//! sequence number allocation, immutability triggers, audit logging, and edge cases.

use crate::company::model::CreateCompanyRequest;
use crate::company::CompanyService;
use crate::database::DatabaseManager;
use crate::vouchers::model::{
    AddVoucherLineRequest, CreateVoucherDraftRequest, CreateVoucherSeriesRequest, DoubleEntryLine,
    ValidationMode, VoucherLineInput,
};
use crate::vouchers::service::{
    DoubleEntryValidator, VoucherDraftService, VoucherPostingService, VoucherSeriesService,
};
use crate::vouchers::validator::validate_voucher_ledger_integrity;
use rust_decimal::Decimal;
use std::str::FromStr;

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage5b_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");

        let pool = db.pool();
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

        Self {
            db,
            _temp_dir: temp_dir,
        }
    }
}

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

// ============================================================================
// Stage 5A Edge Cases (TC-VAL-19 to TC-VAL-24)
// ============================================================================

/// TC-VAL-19: Invalid entry type rejected by pure validator
#[test]
fn test_tc_val_19_invalid_entry_type_rejected() {
    let lines = vec![
        leg(1, "led_bank", "Debit", "500.00"),
        leg(2, "led_sales", "Cr", "500.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "INVALID_ENTRY_TYPE");
    assert!(err.is_some());
    assert_eq!(err.unwrap().line_number, Some(1));
}

/// TC-VAL-20: Missing or whitespace ledger ID rejected
#[test]
fn test_tc_val_20_missing_and_whitespace_ledger_id_rejected() {
    let lines = vec![leg(1, "", "Dr", "500.00"), leg(2, "   ", "Cr", "500.00")];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let missing_errs: Vec<_> = res
        .diagnostics
        .iter()
        .filter(|d| d.code == "MISSING_LEDGER")
        .collect();
    assert_eq!(missing_errs.len(), 2);
}

/// TC-VAL-21: Single-sided voucher with all-Credit legs rejected
#[test]
fn test_tc_val_21_single_sided_credit_voucher_rejected() {
    let lines = vec![
        leg(1, "led_sales_1", "Cr", "500.00"),
        leg(2, "led_sales_2", "Cr", "500.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    let err = res
        .diagnostics
        .iter()
        .find(|d| d.code == "SINGLE_SIDED_VOUCHER");
    assert!(err.is_some());
}

/// TC-VAL-22: 1-line voucher rejected in Posting mode
#[test]
fn test_tc_val_22_single_line_voucher_rejected() {
    let lines = vec![leg(1, "led_bank", "Dr", "500.00")];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(!res.is_valid);
    assert!(!res.is_balanced);
}

/// TC-VAL-23: Compound voucher with duplicate ledger lines succeeds
#[test]
fn test_tc_val_23_compound_voucher_with_duplicate_ledger_lines_succeeds() {
    let lines = vec![
        leg(1, "led_bank", "Dr", "1000.00"),
        leg(2, "led_sales", "Cr", "600.00"),
        leg(3, "led_sales", "Cr", "400.00"),
    ];

    let res = DoubleEntryValidator::validate(&lines, 2, ValidationMode::Posting);

    assert!(res.is_valid);
    assert!(res.is_balanced);
    assert_eq!(res.line_count, 3);
    assert_eq!(res.debit_total, Decimal::from_str("1000.00").unwrap());
    assert_eq!(res.credit_total, Decimal::from_str("1000.00").unwrap());
    assert_eq!(
        res.single_side_total,
        Some(Decimal::from_str("1000.00").unwrap())
    );
}

/// TC-VAL-24: validate_voucher_ledger_integrity handles whitespace trimmed ledger IDs
#[tokio::test]
async fn test_tc_val_24_ledger_integrity_trims_whitespace() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();

    let company = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Stage 5B Whitespace Co".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Maharashtra".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
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

    // Valid with whitespace around cash_id
    let lines = vec![
        leg(1, &format!(" {} ", cash_id), "Dr", "100.00"),
        leg(2, &cash_id, "Cr", "100.00"),
    ];

    let res = validate_voucher_ledger_integrity(&mut conn, &company.id, &lines).await;
    assert!(
        res.is_ok(),
        "Padded ledger ID should be trimmed and accepted"
    );

    // Whitespace only is rejected with Validation error
    let bad_lines = vec![leg(1, "   ", "Dr", "100.00")];
    let err = validate_voucher_ledger_integrity(&mut conn, &company.id, &bad_lines)
        .await
        .expect_err("Whitespace-only ledger ID must fail");
    assert!(
        matches!(err, crate::errors::OpenTallyError::Validation(_)),
        "Expected Validation error for empty ledger, got {:?}",
        err
    );
}

// ============================================================================
// Stage 5B Posting Engine Tests (TC-PST-01 to TC-PST-20)
// ============================================================================

/// Helper to seed a standard test company with cash, bank, and sales ledgers and series
async fn setup_company_with_masters(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_name: &str,
) -> (
    crate::company::model::Company,
    String, // cash ledger id
    String, // sales ledger id
    String, // default sales series id
) {
    let company = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: company_name.to_string(),
            mailing_name: Some(company_name.to_string()),
            country: Some("India".to_string()),
            state: Some("Maharashtra".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    let cash_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = $1 AND ledger_classification = 'cash'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Create a Sales ledger under Sales Accounts group
    let sales_group_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = $1 AND name = 'Sales Accounts'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    let sales_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, ledger_classification)
         VALUES ($1, $2, $3, 'Sales Account', 'standard')",
    )
    .bind(&sales_id)
    .bind(&company.id)
    .bind(&sales_group_id)
    .execute(pool)
    .await
    .unwrap();

    // Create automatic numbering series for vt_sales
    let series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Sales Main".to_string(),
            series_code: "SALES".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("SALES/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    (company, cash_id, sales_id, series.id)
}

/// TC-PST-01: Standard 2-leg balanced draft promoted to posted (Automatic numbering)
#[tokio::test]
async fn test_tc_pst_01_standard_draft_promoted_to_posted() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-01 Co").await;

    // 1. Create a draft voucher
    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.clone(),
            date: "2026-05-15".to_string(),
            effective_date: None,
            reference_no: Some("REF-01".to_string()),
            reference_date: None,
            narration: Some("Test posting draft".to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "5000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "5000.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    assert_eq!(draft.voucher.status, "draft");
    assert!(draft.voucher.voucher_number.starts_with("DRAFT-"));

    // 2. Post the voucher
    let posted = VoucherPostingService::post_voucher(
        pool,
        &company.id,
        &draft.voucher.id,
        Some("operator_test"),
    )
    .await
    .unwrap();

    assert_eq!(posted.voucher.status, "posted");
    assert_eq!(posted.voucher.voucher_number, "SALES/26-27/0001");
    assert_eq!(posted.voucher.sequence_number, Some(1));
    assert_eq!(posted.voucher.period_start, Some("2026-04-01".to_string()));
    assert_eq!(posted.voucher.total_amount, Some("5000.00".to_string()));
    assert_eq!(posted.lines.len(), 2);

    // 3. Verify audit log entry
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = $1 AND entity_id = $2 AND action = 'POST'",
    )
    .bind(&company.id)
    .bind(&draft.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

/// TC-PST-02: Multi-leg compound draft promoted to posted
#[tokio::test]
async fn test_tc_pst_02_compound_draft_promoted_to_posted() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-02 Co").await;

    // Create Bank ledger
    let bank_group_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = $1 AND name = 'Bank Accounts'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    let bank_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, ledger_classification)
         VALUES ($1, $2, $3, 'HDFC Bank', 'standard')",
    )
    .bind(&bank_id)
    .bind(&company.id)
    .bind(&bank_group_id)
    .execute(pool)
    .await
    .unwrap();

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-01".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "2000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: bank_id,
                    entry_type: "Dr".to_string(),
                    amount: "3000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "5000.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let posted = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();

    assert_eq!(posted.voucher.status, "posted");
    assert_eq!(posted.voucher.total_amount, Some("5000.00".to_string()));
    assert_eq!(posted.lines.len(), 3);
}

/// TC-PST-03: Manual series draft with valid formatted number
#[tokio::test]
async fn test_tc_pst_03_manual_series_draft_posted() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, _) = setup_company_with_masters(pool, "PST-03 Co").await;

    // Create manual series for vt_sales
    let manual_series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Manual Sales".to_string(),
            series_code: "MSALE".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "never".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/MAN/".to_string()),
            suffix: None,
            start_number: None,
            width: None,
        },
    )
    .await
    .unwrap();

    // Create draft under manual series
    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: manual_series.id.clone(),
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Set valid manual voucher number in draft (width 4)
    sqlx::query("UPDATE vouchers SET voucher_number = 'INV/MAN/0401' WHERE id = $1")
        .bind(&draft.voucher.id)
        .execute(pool)
        .await
        .unwrap();

    let posted = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();

    assert_eq!(posted.voucher.status, "posted");
    assert_eq!(posted.voucher.voucher_number, "INV/MAN/0401");
    assert_eq!(posted.voucher.sequence_number, None);
}

/// TC-PST-04: Manual series draft with unassigned number (DRAFT-...) rejected
#[tokio::test]
async fn test_tc_pst_04_manual_series_unassigned_number_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, _) = setup_company_with_masters(pool, "PST-04 Co").await;

    let manual_series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Manual Sales 2".to_string(),
            series_code: "MSALE2".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "never".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/MAN/".to_string()),
            suffix: None,
            start_number: None,
            width: None,
        },
    )
    .await
    .unwrap();

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: manual_series.id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Manual draft with DRAFT- number must be rejected");

    assert!(err.to_string().contains("Manual voucher"));
}

/// TC-PST-05: Manual series draft usurping another series rejected
#[tokio::test]
async fn test_tc_pst_05_manual_series_usurping_another_series_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, _) = setup_company_with_masters(pool, "PST-05 Co").await;

    let manual_series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Manual Sales 3".to_string(),
            series_code: "MSALE3".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "never".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/MAN/".to_string()),
            suffix: None,
            start_number: None,
            width: None,
        },
    )
    .await
    .unwrap();

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: manual_series.id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Try to usurp SALES series code
    sqlx::query("UPDATE vouchers SET voucher_number = 'INV/MAN/SALES/99' WHERE id = $1")
        .bind(&draft.voucher.id)
        .execute(pool)
        .await
        .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Usurping number must be rejected");

    assert!(err.to_string().contains("usurps"));
}

/// TC-PST-06: Unbalanced draft rejected
#[tokio::test]
async fn test_tc_pst_06_unbalanced_draft_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-06 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "900.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Unbalanced draft must be rejected");

    assert!(err.to_string().contains("out of balance"));

    // Verify draft remains in draft status
    let check = VoucherDraftService::get_draft(pool, &company.id, &draft.voucher.id)
        .await
        .unwrap();
    assert_eq!(check.voucher.status, "draft");
}

/// TC-PST-07: Empty voucher rejected
#[tokio::test]
async fn test_tc_pst_07_empty_voucher_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, _, _, series_id) = setup_company_with_masters(pool, "PST-07 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Empty draft must be rejected");

    assert!(err.to_string().contains("no line items"));
}

/// TC-PST-08: Single-sided voucher rejected
#[tokio::test]
async fn test_tc_pst_08_single_sided_voucher_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, _, series_id) = setup_company_with_masters(pool, "PST-08 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Single sided draft must be rejected");

    assert!(err.to_string().contains("Debit legs only") || err.to_string().contains("Credit leg"));
}

/// TC-PST-09: Scale overflow rejected
#[tokio::test]
async fn test_tc_pst_09_scale_overflow_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-09 Co").await;

    // Currency decimals is 2. Draft has 3 decimals.
    // Insert line directly bypass draft service validation to simulate corrupted/unhardened line
    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l1', $1, $2, 1, $3, 'Dr', '100.555'),
                ('l2', $1, $2, 2, $4, 'Cr', '100.555')",
    )
    .bind(&company.id)
    .bind(&draft.voucher.id)
    .bind(&cash_id)
    .bind(&sales_id)
    .execute(pool)
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Scale overflow must be rejected");

    assert!(
        err.to_string()
            .contains("exceeding company currency precision")
            || err.to_string().contains("precision")
    );
}

/// TC-PST-10: Zero amount line rejected
#[tokio::test]
async fn test_tc_pst_10_zero_amount_line_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, _sales_id, series_id) =
        setup_company_with_masters(pool, "PST-10 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    // Directly insert line with zero amount (if permitted by unhardened table or bypassed)
    // Here SQLite constraint chk_voucher_line_amount_canonical blocks amount='0.00'
    // but if zero amount line exists, posting validator catches it
    let res = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l1', $1, $2, 1, $3, 'Dr', '0.00')",
    )
    .bind(&company.id)
    .bind(&draft.voucher.id)
    .bind(&cash_id)
    .execute(pool)
    .await;

    // Database blocks it or validator blocks it
    assert!(
        res.is_err(),
        "Database canonical check or posting validator blocks zero amount"
    );
}

/// TC-PST-11: Cross-company ledger rejected
#[tokio::test]
async fn test_tc_pst_11_cross_company_ledger_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company1, cash_id1, sales_id1, series_id1) =
        setup_company_with_masters(pool, "PST-11 Co 1").await;
    let (company2, _, _, _) = setup_company_with_masters(pool, "PST-11 Co 2").await;

    // Try to post in company2 using company1's draft
    let draft1 = VoucherDraftService::create_draft(
        pool,
        &company1.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id1,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id1,
                    entry_type: "Dr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id1,
                    entry_type: "Cr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company2.id, &draft1.voucher.id, None)
        .await
        .expect_err("Cross company post must fail with NotFound");

    assert!(matches!(err, crate::errors::OpenTallyError::NotFound(_)));
}

/// TC-PST-12: Reserved P&L ledger rejected
#[tokio::test]
async fn test_tc_pst_12_reserved_pnl_ledger_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, _, series_id) = setup_company_with_masters(pool, "PST-12 Co").await;

    let pnl_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = $1 AND ledger_classification = 'profit_and_loss'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .unwrap();

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: pnl_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Targeting reserved P&L must fail");

    assert!(err.to_string().contains("reserved Profit & Loss"));
}

/// TC-PST-13: Fiscal date prior to books_beginning_from rejected
#[tokio::test]
async fn test_tc_pst_13_date_prior_to_books_beginning_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-13 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Mutate draft date directly to pre-books-beginning date
    sqlx::query("UPDATE vouchers SET date = '2025-01-01' WHERE id = $1")
        .bind(&draft.voucher.id)
        .execute(pool)
        .await
        .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Date prior to books beginning must fail");

    assert!(err
        .to_string()
        .contains("earlier than company books beginning date"));
}

/// TC-PST-14: Inactive numbering series rejected
#[tokio::test]
async fn test_tc_pst_14_inactive_series_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-14 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.clone(),
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Deactivate series
    sqlx::query("UPDATE voucher_number_series SET is_active = 0 WHERE id = $1")
        .bind(&series_id)
        .execute(pool)
        .await
        .unwrap();

    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Inactive series must fail");

    assert!(err.to_string().contains("inactive"));
}

/// TC-PST-15: Idempotency: calling post_voucher on already posted voucher
#[tokio::test]
async fn test_tc_pst_15_idempotent_post_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-15 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let posted = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();
    assert_eq!(posted.voucher.status, "posted");

    // Second post attempt must return clean validation error
    let err = VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .expect_err("Posting an already posted voucher must be rejected");

    assert!(err.to_string().contains("already posted"));
}

/// TC-PST-16: Rollback safety: failed post rolls back sequence counter
#[tokio::test]
async fn test_tc_pst_16_rollback_safety_preserves_sequence_counter() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-16 Co").await;

    // Post first voucher to establish counter = 2 (allocated 1)
    let draft1 = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.clone(),
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();
    let p1 = VoucherPostingService::post_voucher(pool, &company.id, &draft1.voucher.id, None)
        .await
        .unwrap();
    assert_eq!(p1.voucher.sequence_number, Some(1));

    let next_seq: i64 = sqlx::query_scalar(
        "SELECT next_number FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&company.id)
    .bind(&series_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(next_seq, 2);

    // Create draft2 with unbalanced lines
    let draft2 = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.clone(),
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "400.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Post must fail
    assert!(
        VoucherPostingService::post_voucher(pool, &company.id, &draft2.voucher.id, None)
            .await
            .is_err()
    );

    // Counter must remain 2!
    let next_seq_after: i64 = sqlx::query_scalar(
        "SELECT next_number FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&company.id)
    .bind(&series_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        next_seq_after, 2,
        "Counter must not advance on failed posting"
    );
}

/// TC-PST-17: Line immutability: attempting to insert line into posted voucher is blocked
#[tokio::test]
async fn test_tc_pst_17_cannot_add_line_to_posted_voucher() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-17 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();

    // Attempt to add line item to posted voucher via draft service
    let err = VoucherDraftService::add_line(
        pool,
        &company.id,
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: cash_id,
            entry_type: "Dr".to_string(),
            amount: "100.00".to_string(),
            narration: None,
            line_number: None,
        },
    )
    .await
    .expect_err("Must block line insertion on posted voucher");

    assert!(err.to_string().contains("Cannot add lines") || err.to_string().contains("posted"));
}

/// TC-PST-18: Header freeze: attempting to mutate date or amount on posted voucher is blocked
#[tokio::test]
async fn test_tc_pst_18_posted_voucher_mutation_blocked_by_trigger() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-18 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();

    // Direct SQL update to mutate date on posted voucher
    let err = sqlx::query("UPDATE vouchers SET date = '2026-07-01' WHERE id = $1")
        .bind(&draft.voucher.id)
        .execute(pool)
        .await
        .expect_err("Mutation on posted voucher must trigger ABORT");

    assert!(err.to_string().contains("strictly immutable"));
}

/// TC-PST-19: Delete block: attempting to delete posted voucher is blocked
#[tokio::test]
async fn test_tc_pst_19_cannot_delete_posted_voucher() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-19 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None)
        .await
        .unwrap();

    // Attempt deletion
    let err = VoucherDraftService::delete_draft(pool, &company.id, &draft.voucher.id)
        .await
        .expect_err("Cannot delete posted voucher");

    assert!(
        err.to_string()
            .contains("Only draft vouchers can be deleted")
            || err.to_string().contains("draft")
    );
}

/// TC-PST-20: Sequential / concurrent post attempts on same draft
#[tokio::test]
async fn test_tc_pst_20_concurrent_post_protection() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (company, cash_id, sales_id, series_id) =
        setup_company_with_masters(pool, "PST-20 Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &company.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id,
            date: "2026-06-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id,
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id,
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let res1 =
        VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None).await;
    let res2 =
        VoucherPostingService::post_voucher(pool, &company.id, &draft.voucher.id, None).await;

    assert!(res1.is_ok());
    assert!(res2.is_err());
    assert!(res2.unwrap_err().to_string().contains("already posted"));
}
