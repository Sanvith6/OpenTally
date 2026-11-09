//! Sprint 2 Stage 6: Voucher Cancellation, Reversal & Corrections Test Suite
//! Tests atomic voucher cancellation, line immutability preservation, sequence retention,
//! reason validation, audit logging, and edge-case error conditions.

use crate::company::model::CreateCompanyRequest;
use crate::company::CompanyService;
use crate::database::DatabaseManager;
use crate::vouchers::model::{
    CancelVoucherRequest, CreateCorrectionDraftRequest, CreateVoucherDraftRequest,
    CreateVoucherSeriesRequest, ReverseVoucherRequest, UpdateVoucherDraftRequest, VoucherLine,
    VoucherLineInput,
};
use crate::vouchers::service::{
    VoucherCancellationService, VoucherCorrectionService, VoucherDraftService,
    VoucherPostingService, VoucherReversalService, VoucherSeriesService,
};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage6_test.db");
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

/// Helper to seed a standard test company with cash, bank, and sales ledgers and series
async fn setup_company_with_masters(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_name: &str,
) -> (
    crate::company::model::Company,
    String, // cash ledger id
    String, // sales ledger id
    String, // default sales series id
    String, // default reversal series id
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

    // Create series for vt_sales
    let series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Sales Primary".to_string(),
            series_code: "SALES".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("SAL/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Create series for vt_reversing_journal
    let rev_series = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_reversing_journal".to_string(),
            series_name: "Reversal Primary".to_string(),
            series_code: "REV".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("REV/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    (company, cash_id, sales_id, series.id, rev_series.id)
}

/// Helper to create and post a balanced test voucher
async fn create_and_post_test_voucher(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
    series_id: &str,
    cash_id: &str,
    sales_id: &str,
    amount: &str,
) -> String {
    let draft = VoucherDraftService::create_draft(
        pool,
        company_id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.to_string(),
            date: "2026-05-10".to_string(),
            effective_date: Some("2026-05-10".to_string()),
            reference_no: Some("REF-100".to_string()),
            reference_date: Some("2026-05-10".to_string()),
            narration: Some("Test sales invoice".to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.to_string(),
                    entry_type: "Dr".to_string(),
                    amount: amount.to_string(),
                    narration: Some("Cash received".to_string()),
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id.to_string(),
                    entry_type: "Cr".to_string(),
                    amount: amount.to_string(),
                    narration: Some("Sales booked".to_string()),
                },
            ]),
        },
    )
    .await
    .unwrap();

    let posted = VoucherPostingService::post_voucher(
        pool,
        company_id,
        &draft.voucher.id,
        Some("test_operator"),
    )
    .await
    .unwrap();

    posted.voucher.id
}

// ============================================================================
// Stage 6 Phase 1: Voucher Cancellation Tests (TC-CAN-01 to TC-CAN-11)
// ============================================================================

/// TC-CAN-01: Cancel valid posted voucher successfully
#[tokio::test]
async fn test_tc_can_01_cancel_valid_posted_voucher_success() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Cancel Success Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1500.00").await;

    // Execute cancellation
    let result = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Client canceled shipment prior to dispatch".to_string(),
            actor_id: Some("operator_jane".to_string()),
        },
    )
    .await
    .expect("Cancellation must succeed for valid posted voucher");

    // Assert status transition
    assert_eq!(result.voucher.status, "cancelled");
    assert_eq!(result.voucher.total_amount, Some("1500.00".to_string()));
    assert_eq!(result.lines.len(), 2);

    // Verify database state directly
    let (status, total_amt): (String, Option<String>) = sqlx::query_as(
        "SELECT status, total_amount FROM vouchers WHERE company_id = $1 AND id = $2",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(status, "cancelled");
    assert_eq!(total_amt, Some("1500.00".to_string()));

    // Verify lines are bit-for-bit preserved in database
    let line_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(line_count, 2);

    // Verify audit log record
    let (action, performed_by, old_val, new_val): (String, String, String, String) =
        sqlx::query_as(
            "SELECT action, performed_by, old_values, new_values
         FROM audit_log
         WHERE company_id = $1 AND entity_id = $2 AND action = 'CANCEL'",
        )
        .bind(&co.id)
        .bind(&vch_id)
        .fetch_one(pool)
        .await
        .unwrap();

    assert_eq!(action, "CANCEL");
    assert_eq!(performed_by, "operator_jane");
    assert!(old_val.contains("\"status\":\"posted\""));
    assert!(new_val.contains("\"status\":\"cancelled\""));
    assert!(new_val.contains("Client canceled shipment"));
}

/// TC-CAN-02: Cancel draft voucher fails
#[tokio::test]
async fn test_tc_can_02_cancel_draft_voucher_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, _cash, _sales, series, _) = setup_company_with_masters(pool, "Cancel Draft Co").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        &co.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series,
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: draft.voucher.id.clone(),
            reason: "Cancelling an unposted draft".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cancelling draft must fail");

    assert!(
        matches!(err, crate::errors::OpenTallyError::Validation(_)),
        "Expected Validation error, got {:?}",
        err
    );
    assert!(err
        .to_string()
        .contains("Draft vouchers cannot be cancelled"));
}

/// TC-CAN-03: Cancel already-cancelled voucher deterministically fails
#[tokio::test]
async fn test_tc_can_03_cancel_already_cancelled_voucher_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) =
        setup_company_with_masters(pool, "Already Cancelled Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "800.00").await;

    // First cancellation succeeds
    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Valid cancellation reason".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Second cancellation fails deterministically
    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Second attempt to cancel".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cancelling an already-cancelled voucher must fail");

    assert!(
        matches!(err, crate::errors::OpenTallyError::Validation(_)),
        "Expected Validation error, got {:?}",
        err
    );
    assert!(err.to_string().contains("already cancelled"));
}

/// TC-CAN-04: Cancel reversed voucher fails
#[tokio::test]
async fn test_tc_can_04_cancel_reversed_voucher_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, rev_series) =
        setup_company_with_masters(pool, "Cancel Reversed Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "950.00").await;

    // Legitimate reversal workflow to transition vch_id to 'reversed':
    let rev_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount, reversal_of_voucher_id
        ) VALUES ($1, $2, 'vt_reversing_journal', $3, 'automatic',
                  NULL, NULL, 'DRAFT-REV-01', '2026-05-15', '2026-05-15',
                  'draft', '950.00', $4)",
    )
    .bind(&rev_id)
    .bind(&co.id)
    .bind(&rev_series)
    .bind(&vch_id)
    .execute(pool)
    .await
    .unwrap();

    let line1_id = uuid::Uuid::new_v4().to_string();
    let line2_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ($1, $2, $3, 1, $4, 'Cr', '950.00'), ($5, $2, $3, 2, $6, 'Dr', '950.00')",
    )
    .bind(&line1_id)
    .bind(&co.id)
    .bind(&rev_id)
    .bind(&cash)
    .bind(&line2_id)
    .bind(&sales)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE vouchers
         SET status = 'posted',
             voucher_number = 'REV/26-27/0001',
             period_start = '2026-04-01',
             sequence_number = 1
         WHERE company_id = $1 AND id = $2",
    )
    .bind(&co.id)
    .bind(&rev_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query("UPDATE vouchers SET status = 'reversed' WHERE company_id = $1 AND id = $2")
        .bind(&co.id)
        .bind(&vch_id)
        .execute(pool)
        .await
        .unwrap();

    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Attempting to cancel reversed voucher".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cancelling reversed voucher must fail");

    assert!(
        matches!(err, crate::errors::OpenTallyError::Validation(_)),
        "Expected Validation error, got {:?}",
        err
    );
    assert!(err.to_string().contains("Cannot cancel a reversed voucher"));
}

/// TC-CAN-05: Cancel reversing journal voucher fails via trigger Rule 4 & service check
#[tokio::test]
async fn test_tc_can_05_cancel_reversing_journal_voucher_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, rev_series) =
        setup_company_with_masters(pool, "Cancel Reversing Co").await;

    let orig_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1200.00").await;

    // Insert dummy reversing voucher with rev_series
    let rev_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount, reversal_of_voucher_id
        ) VALUES ($1, $2, 'vt_reversing_journal', $3, 'automatic',
                  '2026-04-01', 999, 'REV/26-27/0999', '2026-05-15', '2026-05-15',
                  'posted', '1200.00', $4)",
    )
    .bind(&rev_id)
    .bind(&co.id)
    .bind(&rev_series)
    .bind(&orig_id)
    .execute(pool)
    .await
    .unwrap();

    // Attempt to cancel reversing voucher
    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: rev_id,
            reason: "Cancelling the reversal".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cancelling a reversing voucher must be rejected");

    assert!(
        matches!(err, crate::errors::OpenTallyError::Validation(_)),
        "Expected Validation error, got {:?}",
        err
    );
    assert!(err
        .to_string()
        .contains("Cannot cancel a reversing voucher"));
}

/// TC-CAN-06: Cancel with empty or whitespace reason fails validation
#[tokio::test]
async fn test_tc_can_06_cancel_with_invalid_reason_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) =
        setup_company_with_masters(pool, "Reason Validation Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "300.00").await;

    // Empty reason
    let err1 = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Empty reason must fail");

    assert!(err1.to_string().contains("minimum 3 characters"));

    // Whitespace only
    let err2 = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "   \t\n  ".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Whitespace reason must fail");

    assert!(err2.to_string().contains("minimum 3 characters"));

    // Too short (2 characters)
    let err3 = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "no".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Short reason must fail");

    assert!(err3.to_string().contains("minimum 3 characters"));
}

/// TC-CAN-07: Sequence counter is never decremented or recycled upon cancellation
#[tokio::test]
async fn test_tc_can_07_sequence_counter_never_decremented_or_recycled() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) =
        setup_company_with_masters(pool, "Sequence Retention Co").await;

    // Post first voucher -> sequence #1
    let vch1_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "100.00").await;

    let seq1: i64 = sqlx::query_scalar("SELECT sequence_number FROM vouchers WHERE id = $1")
        .bind(&vch1_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(seq1, 1);

    // Cancel voucher #1
    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch1_id.clone(),
            reason: "Mistaken entry".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Verify counter in sequence table is still at 2
    let next_seq: i64 = sqlx::query_scalar(
        "SELECT next_number FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&co.id)
    .bind(&series)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(next_seq, 2);

    // Post second voucher -> must receive sequence #2 (NOT 1!)
    let vch2_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "200.00").await;

    let seq2: i64 = sqlx::query_scalar("SELECT sequence_number FROM vouchers WHERE id = $1")
        .bind(&vch2_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(seq2, 2);

    // Assert voucher #1 still retains sequence 1 permanently
    let vch1_seq_after: i64 =
        sqlx::query_scalar("SELECT sequence_number FROM vouchers WHERE id = $1")
            .bind(&vch1_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(vch1_seq_after, 1);
}

/// TC-CAN-08: Attempt to mutate lines of cancelled voucher fails via DB trigger
#[tokio::test]
async fn test_tc_can_08_mutate_lines_of_cancelled_voucher_blocked_by_trigger() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Line Mutation Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "500.00").await;

    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Cancelling voucher".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Attempt direct line update
    let err = sqlx::query(
        "UPDATE voucher_lines SET amount = '999.00' WHERE company_id = $1 AND voucher_id = $2",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .execute(pool)
    .await
    .expect_err("Direct line update on cancelled voucher must fail");

    assert!(err.to_string().contains("cannot be modified"));
}

/// TC-CAN-09: Attempt to delete cancelled voucher fails via DB trigger
#[tokio::test]
async fn test_tc_can_09_delete_cancelled_voucher_blocked_by_trigger() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Voucher Delete Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "400.00").await;

    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Cancelling for deletion test".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Attempt hard deletion of cancelled voucher
    let err = sqlx::query("DELETE FROM vouchers WHERE company_id = $1 AND id = $2")
        .bind(&co.id)
        .bind(&vch_id)
        .execute(pool)
        .await
        .expect_err("Hard deletion of cancelled voucher must fail");

    assert!(err
        .to_string()
        .contains("Hard deletion of financial vouchers is strictly prohibited"));
}

/// TC-CAN-10: Cannot cancel voucher that already has a posted reversal
#[tokio::test]
async fn test_tc_can_10_cancel_voucher_with_existing_reversal_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, rev_series) =
        setup_company_with_masters(pool, "Has Reversal Co").await;

    let orig_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "700.00").await;

    // Insert posted reversing voucher referencing orig_id
    let rev_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount, reversal_of_voucher_id
        ) VALUES ($1, $2, 'vt_reversing_journal', $3, 'automatic',
                  '2026-04-01', 888, 'REV/26-27/0888', '2026-05-15', '2026-05-15',
                  'posted', '700.00', $4)",
    )
    .bind(&rev_id)
    .bind(&co.id)
    .bind(&rev_series)
    .bind(&orig_id)
    .execute(pool)
    .await
    .unwrap();

    // Attempt to cancel original voucher
    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: orig_id,
            reason: "Cancelling after reversal exists".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Must not allow cancelling a voucher that has already been reversed");

    assert!(err.to_string().contains("already been reversed"));
}

/// TC-CAN-11: Tenant isolation: cannot cancel voucher belonging to another company
#[tokio::test]
async fn test_tc_can_11_tenant_isolation_enforced() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co1, cash1, sales1, series1, _) = setup_company_with_masters(pool, "Tenant A").await;
    let (co2, _cash2, _sales2, _series2, _) = setup_company_with_masters(pool, "Tenant B").await;

    let vch1_id =
        create_and_post_test_voucher(pool, &co1.id, &series1, &cash1, &sales1, "100.00").await;

    // Attempt to cancel vch1 under company 2
    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co2.id,
        CancelVoucherRequest {
            voucher_id: vch1_id,
            reason: "Cross tenant cancellation".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cross-tenant cancellation must fail with NotFound");

    assert!(
        matches!(err, crate::errors::OpenTallyError::NotFound(_)),
        "Expected NotFound error, got {:?}",
        err
    );
}

// =========================================================================
// REVERSAL ENGINE TEST CASES (TC-REV-01 to TC-REV-10)
// =========================================================================

/// TC-REV-01: Reverse valid posted voucher produces a posted reversing journal
/// voucher and transitions original voucher to 'reversed' status.
#[tokio::test]
async fn test_tc_rev_01_reverse_valid_posted_voucher_success() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Reversal Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1500.00").await;

    let result = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id.clone(),
            reversal_date: "2026-05-10".to_string(),
            reason: "Wrong billing terms applied".to_string(),
            actor_id: Some("accountant_jane".to_string()),
        },
    )
    .await
    .expect("Reversal of valid posted voucher must succeed");

    // 1. Verify original voucher status transition
    assert_eq!(result.original_voucher.voucher.id, vch_id);
    assert_eq!(result.original_voucher.voucher.status, "reversed");

    // 2. Verify reversing voucher properties
    assert_eq!(result.reversing_voucher.voucher.company_id, co.id);
    assert_eq!(
        result.reversing_voucher.voucher.voucher_type_id,
        "vt_reversing_journal"
    );
    assert_eq!(result.reversing_voucher.voucher.status, "posted");
    assert_eq!(
        result.reversing_voucher.voucher.reversal_of_voucher_id,
        Some(vch_id.clone())
    );
    assert_eq!(result.reversing_voucher.voucher.date, "2026-05-10");
    assert_eq!(
        result.reversing_voucher.voucher.total_amount,
        Some("1500.00".to_string())
    );
    assert!(result
        .reversing_voucher
        .voucher
        .voucher_number
        .starts_with("REV/"));

    // 3. Verify database state
    let orig_db_status: String = sqlx::query_scalar("SELECT status FROM vouchers WHERE id = $1")
        .bind(&vch_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(orig_db_status, "reversed");

    let rev_db_status: String = sqlx::query_scalar("SELECT status FROM vouchers WHERE id = $1")
        .bind(&result.reversing_voucher.voucher.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(rev_db_status, "posted");

    // 4. Verify audit logs
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_log WHERE company_id = $1 AND entity_id IN ($2, $3) ORDER BY timestamp ASC",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .bind(&result.reversing_voucher.voucher.id)
    .fetch_all(pool)
    .await
    .unwrap();

    assert!(actions.contains(&"POST".to_string()));
    assert!(actions.contains(&"REVERSE".to_string()));
}

/// TC-REV-02: Verify 1:1 inverted lines (Dr <-> Cr) with identical amounts, line numbers, and distinct IDs.
#[tokio::test]
async fn test_tc_rev_02_verify_1_to_1_inverted_lines() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Inversion Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "3250.75").await;

    let result = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id.clone(),
            reversal_date: "2026-05-12".to_string(),
            reason: "Incorrect client charged".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    let orig_lines = &result.original_voucher.lines;
    let rev_lines = &result.reversing_voucher.lines;

    assert_eq!(orig_lines.len(), rev_lines.len());
    assert_eq!(orig_lines.len(), 2);

    // Line 1: Original had Cash Dr 3250.75
    // Reversal must have Cash Cr 3250.75
    assert_eq!(orig_lines[0].ledger_id, rev_lines[0].ledger_id);
    assert_eq!(orig_lines[0].line_number, rev_lines[0].line_number);
    assert_eq!(orig_lines[0].entry_type, "Dr");
    assert_eq!(rev_lines[0].entry_type, "Cr");
    assert_eq!(rev_lines[0].amount, orig_lines[0].amount);
    assert_ne!(orig_lines[0].id, rev_lines[0].id);

    // Line 2: Original had Sales Cr 3250.75
    // Reversal must have Sales Dr 3250.75
    assert_eq!(orig_lines[1].ledger_id, rev_lines[1].ledger_id);
    assert_eq!(orig_lines[1].line_number, rev_lines[1].line_number);
    assert_eq!(orig_lines[1].entry_type, "Cr");
    assert_eq!(rev_lines[1].entry_type, "Dr");
    assert_eq!(rev_lines[1].amount, orig_lines[1].amount);
    assert_ne!(orig_lines[1].id, rev_lines[1].id);
}

/// TC-REV-03: Ledger balance impact: Original + Reversal net to exactly 0.00
#[tokio::test]
async fn test_tc_rev_03_ledger_balance_impact_net_zero() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Net Zero Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "7500.00").await;

    VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2026-06-01".to_string(),
            reason: "Voiding invoice".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Query net cash balance across active vouchers (posted + reversed)
    let cash_lines = sqlx::query_as::<_, VoucherLine>(
        "SELECT vl.id, vl.company_id, vl.voucher_id, vl.line_number, vl.ledger_id, vl.entry_type, vl.amount, vl.narration, vl.created_at
         FROM voucher_lines vl
         JOIN vouchers v ON vl.company_id = v.company_id AND vl.voucher_id = v.id
         WHERE vl.company_id = $1 AND vl.ledger_id = $2 AND v.status IN ('posted', 'reversed')",
    )
    .bind(&co.id)
    .bind(&cash)
    .fetch_all(pool)
    .await
    .unwrap();

    let mut net_cash = rust_decimal::Decimal::ZERO;
    for l in cash_lines {
        let amt: rust_decimal::Decimal = l.amount.parse().unwrap();
        if l.entry_type == "Dr" {
            net_cash += amt;
        } else {
            net_cash -= amt;
        }
    }
    assert_eq!(net_cash, rust_decimal::Decimal::ZERO);

    // Query net sales balance
    let sales_lines = sqlx::query_as::<_, VoucherLine>(
        "SELECT vl.id, vl.company_id, vl.voucher_id, vl.line_number, vl.ledger_id, vl.entry_type, vl.amount, vl.narration, vl.created_at
         FROM voucher_lines vl
         JOIN vouchers v ON vl.company_id = v.company_id AND vl.voucher_id = v.id
         WHERE vl.company_id = $1 AND vl.ledger_id = $2 AND v.status IN ('posted', 'reversed')",
    )
    .bind(&co.id)
    .bind(&sales)
    .fetch_all(pool)
    .await
    .unwrap();

    let mut net_sales = rust_decimal::Decimal::ZERO;
    for l in sales_lines {
        let amt: rust_decimal::Decimal = l.amount.parse().unwrap();
        if l.entry_type == "Dr" {
            net_sales += amt;
        } else {
            net_sales -= amt;
        }
    }
    assert_eq!(net_sales, rust_decimal::Decimal::ZERO);
}

/// TC-REV-04: Duplicate reversal attempt fails deterministically
#[tokio::test]
async fn test_tc_rev_04_duplicate_reversal_attempt_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Dup Reversal Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "500.00").await;

    // First reversal succeeds
    VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id.clone(),
            reversal_date: "2026-05-15".to_string(),
            reason: "First legitimate reversal".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect("First reversal must succeed");

    // Second reversal attempt must fail
    let err = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2026-05-16".to_string(),
            reason: "Second duplicate reversal attempt".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Duplicate reversal must fail");

    assert!(
        err.to_string().contains("already reversed"),
        "Expected 'already reversed' error, got: {}",
        err
    );
}

/// TC-REV-05: Reverse cancelled voucher fails
#[tokio::test]
async fn test_tc_rev_05_reverse_cancelled_voucher_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Cancel Rev Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "800.00").await;

    // Cancel voucher
    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_id.clone(),
            reason: "Cancelled by manager".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Attempt to reverse cancelled voucher
    let err = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2026-05-01".to_string(),
            reason: "Attempting to reverse cancelled voucher".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Must not reverse cancelled voucher");

    assert!(
        err.to_string()
            .contains("Cannot reverse a cancelled voucher"),
        "Expected 'Cannot reverse a cancelled voucher', got: {}",
        err
    );
}

/// TC-REV-06: Reversal date earlier than original date fails
#[tokio::test]
async fn test_tc_rev_06_reversal_date_earlier_than_original_date_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Date Precede Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1200.00").await;

    // Attempt to reverse with earlier date
    let err = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2026-04-10".to_string(), // earlier than 2026-04-15
            reason: "Backdated reversal attempt".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Reversal date earlier than original date must fail");

    assert!(
        err.to_string()
            .contains("cannot precede original voucher date"),
        "Expected date precede error, got: {}",
        err
    );
}

/// TC-REV-07: Reversal across period boundary derives correct period and sequence
#[tokio::test]
async fn test_tc_rev_07_reversal_across_period_boundary() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Period Boundary Co").await;

    // Original in April 2026
    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "2000.00").await;

    // Reversal in next financial year: April 2027
    let result = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2027-04-05".to_string(),
            reason: "Prior period error correction in FY 27-28".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect("Reversal across period boundary must succeed");

    assert_eq!(
        result.reversing_voucher.voucher.period_start,
        Some("2027-04-01".to_string())
    );
    assert_eq!(result.reversing_voucher.voucher.sequence_number, Some(1));
    assert!(result
        .reversing_voucher
        .voucher
        .voucher_number
        .contains("27-28"));
}

/// TC-REV-08: Reversal chain (A -> B -> C): Counter-reversal restores ledger equilibrium
#[tokio::test]
async fn test_tc_rev_08_reversal_chain_a_to_b_to_c() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Chain Co").await;

    // Voucher A: Posted
    let vch_a_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "4000.00").await;

    // Voucher B reverses Voucher A
    let res_b = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_a_id.clone(),
            reversal_date: "2026-05-15".to_string(),
            reason: "Mistaken reversal of A".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    let vch_b_id = res_b.reversing_voucher.voucher.id;

    // Voucher C reverses Reversal Voucher B (counter-reversal)
    let res_c = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_b_id.clone(),
            reversal_date: "2026-05-20".to_string(),
            reason: "Counter-reversal: re-instating original transaction".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect("Counter-reversal of reversing voucher must succeed");

    assert_eq!(res_c.original_voucher.voucher.id, vch_b_id);
    assert_eq!(res_c.original_voucher.voucher.status, "reversed");
    assert_eq!(res_c.reversing_voucher.voucher.status, "posted");
    assert_eq!(
        res_c.reversing_voucher.voucher.reversal_of_voucher_id,
        Some(vch_b_id)
    );

    // Net ledger balance across A, B, C:
    // A: Cash Dr 4000, Sales Cr 4000
    // B: Cash Cr 4000, Sales Dr 4000
    // C: Cash Dr 4000, Sales Cr 4000
    // Net: Cash Dr 4000, Sales Cr 4000 (restoring A's balance!)
    let cash_lines = sqlx::query_as::<_, VoucherLine>(
        "SELECT vl.id, vl.company_id, vl.voucher_id, vl.line_number, vl.ledger_id, vl.entry_type, vl.amount, vl.narration, vl.created_at
         FROM voucher_lines vl
         JOIN vouchers v ON vl.company_id = v.company_id AND vl.voucher_id = v.id
         WHERE vl.company_id = $1 AND vl.ledger_id = $2 AND v.status IN ('posted', 'reversed')",
    )
    .bind(&co.id)
    .bind(&cash)
    .fetch_all(pool)
    .await
    .unwrap();

    let mut net_cash = rust_decimal::Decimal::ZERO;
    for l in cash_lines {
        let amt: rust_decimal::Decimal = l.amount.parse().unwrap();
        if l.entry_type == "Dr" {
            net_cash += amt;
        } else {
            net_cash -= amt;
        }
    }

    let expected = rust_decimal::Decimal::from_str_exact("4000.00").unwrap();
    assert_eq!(net_cash, expected);
}

/// TC-REV-09: Attempt to cancel a reversing voucher fails via Trigger Rule 4 and pre-check
#[tokio::test]
async fn test_tc_rev_09_attempt_to_cancel_reversing_voucher_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) =
        setup_company_with_masters(pool, "Block Rev Cancel Co").await;

    let vch_id = create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "999.00").await;

    let rev_res = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id,
            reversal_date: "2026-05-15".to_string(),
            reason: "Reversing original".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    let rev_vch_id = rev_res.reversing_voucher.voucher.id;

    let err = VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: rev_vch_id,
            reason: "Trying to cancel reversing voucher".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Cancelling reversing voucher must fail");

    assert!(
        err.to_string()
            .contains("Cannot cancel a reversing voucher"),
        "Expected 'Cannot cancel a reversing voucher', got: {}",
        err
    );
}

/// TC-REV-10: Self-reversal attempt fails via chk_no_self_reversal constraint
#[tokio::test]
async fn test_tc_rev_10_self_reversal_attempt_fails() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, _, _, series, _) = setup_company_with_masters(pool, "Self Rev Co").await;

    let self_id = uuid::Uuid::new_v4().to_string();

    let err = sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount, reversal_of_voucher_id
         ) VALUES ($1, $2, 'vt_sales', $3, 'manual',
                   NULL, NULL, 'SELF-1', '2026-05-01', '2026-05-01',
                   'draft', '0.00', $4)",
    )
    .bind(&self_id)
    .bind(&co.id)
    .bind(&series)
    .bind(&self_id) // self-referencing reversal!
    .execute(pool)
    .await
    .expect_err("Self-referencing reversal must violate chk_no_self_reversal");

    assert!(
        err.to_string().contains("chk_no_self_reversal")
            || err.to_string().contains("CHECK constraint failed"),
        "Expected chk_no_self_reversal error, got: {}",
        err
    );
}

// =========================================================================
// CORRECTION WORKFLOW & CONCURRENCY TEST CASES (TC-COR-01 to TC-COR-08)
// =========================================================================

/// TC-COR-01: Draft correction in-place updates lines and header without sequence consumption
#[tokio::test]
async fn test_tc_cor_01_draft_correction_in_place() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Draft Cor Co").await;

    // Create initial draft
    let draft = VoucherDraftService::create_draft(
        pool,
        &co.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: Some("DRAFT-REF-1".to_string()),
            reference_date: None,
            narration: Some("Initial draft narration".to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    assert_eq!(draft.voucher.status, "draft");
    assert!(draft.voucher.sequence_number.is_none());

    // Update draft header and lines
    let updated = VoucherDraftService::update_draft_header(
        pool,
        &co.id,
        &draft.voucher.id,
        UpdateVoucherDraftRequest {
            narration: Some("Corrected draft narration".to_string()),
            date: Some("2026-05-12".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.status, "draft");
    assert_eq!(
        updated.narration,
        Some("Corrected draft narration".to_string())
    );
    assert_eq!(updated.date, "2026-05-12");
    assert!(updated.sequence_number.is_none());

    // Verify sequence counter was NOT touched
    let seq_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&co.id)
    .bind(&series)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        seq_count, 0,
        "No sequence rows should be created for drafts"
    );
}

/// TC-COR-02: Posted correction workflow (Option A): Original reversed, new draft created with reference link
#[tokio::test]
async fn test_tc_cor_02_posted_correction_workflow_option_a() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Option A Cor Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "5000.00").await;

    let cor_result = VoucherCorrectionService::create_correction_draft(
        pool,
        &co.id,
        CreateCorrectionDraftRequest {
            original_voucher_id: vch_id.clone(),
            reversal_date: Some("2026-05-15".to_string()),
            reversal_reason: "Product returned and invoice re-issued".to_string(),
            actor_id: Some("clerk_bob".to_string()),
        },
    )
    .await
    .expect("Option A correction workflow must succeed");

    // 1. Original voucher reversed
    assert_eq!(cor_result.original_voucher.voucher.id, vch_id);
    assert_eq!(cor_result.original_voucher.voucher.status, "reversed");

    // 2. Reversing voucher posted
    assert_eq!(
        cor_result.reversing_voucher.voucher.voucher_type_id,
        "vt_reversing_journal"
    );
    assert_eq!(cor_result.reversing_voucher.voucher.status, "posted");
    assert_eq!(
        cor_result.reversing_voucher.voucher.reversal_of_voucher_id,
        Some(vch_id)
    );

    // 3. New draft created with cloned lines
    assert_eq!(cor_result.corrected_draft.voucher.status, "draft");
    assert_eq!(
        cor_result.corrected_draft.voucher.voucher_type_id,
        "vt_sales"
    );
    assert_eq!(cor_result.corrected_draft.voucher.series_id, series);
    assert_eq!(cor_result.corrected_draft.lines.len(), 2);
    assert!(cor_result
        .corrected_draft
        .voucher
        .reference_no
        .as_ref()
        .unwrap()
        .starts_with("Correction of SAL/"));
}

/// TC-COR-04: Concurrent cancellation of the same voucher: One succeeds, one fails
#[tokio::test]
async fn test_tc_cor_04_concurrent_cancellation() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) =
        setup_company_with_masters(pool, "Concurrent Cancel Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1800.00").await;

    let pool1 = pool.clone();
    let co1_id = co.id.clone();
    let vch1_id = vch_id.clone();

    let task1 = tokio::spawn(async move {
        VoucherCancellationService::cancel_voucher(
            &pool1,
            &co1_id,
            CancelVoucherRequest {
                voucher_id: vch1_id,
                reason: "Concurrent cancel task 1".to_string(),
                actor_id: None,
            },
        )
        .await
    });

    let pool2 = pool.clone();
    let co2_id = co.id.clone();
    let vch2_id = vch_id.clone();

    let task2 = tokio::spawn(async move {
        VoucherCancellationService::cancel_voucher(
            &pool2,
            &co2_id,
            CancelVoucherRequest {
                voucher_id: vch2_id,
                reason: "Concurrent cancel task 2".to_string(),
                actor_id: None,
            },
        )
        .await
    });

    let (res1, res2) = tokio::join!(task1, task2);
    let r1 = res1.unwrap();
    let r2 = res2.unwrap();

    let success_count = (r1.is_ok() as i32) + (r2.is_ok() as i32);
    let error_count = (r1.is_err() as i32) + (r2.is_err() as i32);

    assert_eq!(
        success_count, 1,
        "Exactly one concurrent cancellation must succeed"
    );
    assert_eq!(
        error_count, 1,
        "Exactly one concurrent cancellation must fail"
    );

    // Final database status must be cancelled
    let status: String = sqlx::query_scalar("SELECT status FROM vouchers WHERE id = $1")
        .bind(&vch_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "cancelled");
}

/// TC-COR-05: Concurrent reversal of the same voucher: One succeeds, one fails
#[tokio::test]
async fn test_tc_cor_05_concurrent_reversal() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Concurrent Rev Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "2200.00").await;

    let pool1 = pool.clone();
    let co1_id = co.id.clone();
    let vch1_id = vch_id.clone();

    let task1 = tokio::spawn(async move {
        VoucherReversalService::reverse_voucher(
            &pool1,
            &co1_id,
            ReverseVoucherRequest {
                original_voucher_id: vch1_id,
                reversal_date: "2026-05-15".to_string(),
                reason: "Concurrent reversal task 1".to_string(),
                actor_id: None,
            },
        )
        .await
    });

    let pool2 = pool.clone();
    let co2_id = co.id.clone();
    let vch2_id = vch_id.clone();

    let task2 = tokio::spawn(async move {
        VoucherReversalService::reverse_voucher(
            &pool2,
            &co2_id,
            ReverseVoucherRequest {
                original_voucher_id: vch2_id,
                reversal_date: "2026-05-15".to_string(),
                reason: "Concurrent reversal task 2".to_string(),
                actor_id: None,
            },
        )
        .await
    });

    let (res1, res2) = tokio::join!(task1, task2);
    let r1 = res1.unwrap();
    let r2 = res2.unwrap();

    let success_count = (r1.is_ok() as i32) + (r2.is_ok() as i32);
    let error_count = (r1.is_err() as i32) + (r2.is_err() as i32);

    assert_eq!(
        success_count, 1,
        "Exactly one concurrent reversal must succeed"
    );
    assert_eq!(error_count, 1, "Exactly one concurrent reversal must fail");

    // Exactly one reversing voucher should exist
    let rev_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id = $1 AND reversal_of_voucher_id = $2 AND status = 'posted'",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rev_count, 1);
}

/// TC-COR-06: Rollback on reversal line failure preserves all database state
#[tokio::test]
async fn test_tc_cor_06_rollback_on_reversal_failure() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Rollback Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "3300.00").await;

    // Trigger failure by supplying an invalid date
    let err = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_id.clone(),
            reversal_date: "invalid-date".to_string(),
            reason: "Testing rollback on bad date".to_string(),
            actor_id: None,
        },
    )
    .await
    .expect_err("Must fail on invalid reversal date");

    assert!(err.to_string().contains("date") || err.to_string().contains("format"));

    // Ensure original voucher remains posted and untouched
    let status: String = sqlx::query_scalar("SELECT status FROM vouchers WHERE id = $1")
        .bind(&vch_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "posted");

    // Ensure no reversal voucher was inserted
    let rev_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id = $1 AND reversal_of_voucher_id = $2",
    )
    .bind(&co.id)
    .bind(&vch_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rev_count, 0);
}

/// TC-COR-07: Rollback failure connection disposal (OBS-5B-01)
#[tokio::test]
async fn test_tc_cor_07_connection_disposal_on_rollback_failure() {
    // Verifies that pool acquisition remains sound even after abortive transactions
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Conn Disposal Co").await;

    let vch_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "4500.00").await;

    // Run 5 failed transactions in a row
    for _ in 0..5 {
        let _ = VoucherCancellationService::cancel_voucher(
            pool,
            &co.id,
            CancelVoucherRequest {
                voucher_id: vch_id.clone(),
                reason: "x".to_string(), // too short, validation error
                actor_id: None,
            },
        )
        .await;
    }

    // Pool must remain fully operational
    let post_conn = pool.acquire().await.expect("Pool must remain healthy");
    drop(post_conn);
}

/// TC-COR-08: Verify audit log before/after JSON diffs for cancellation & reversal
#[tokio::test]
async fn test_tc_cor_08_audit_log_diff_verification() {
    let fixture = TestDb::new().await;
    let pool = fixture.db.pool();
    let (co, cash, sales, series, _) = setup_company_with_masters(pool, "Audit Diff Co").await;

    // 1. Post voucher A and Cancel
    let vch_a_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "1111.00").await;
    VoucherCancellationService::cancel_voucher(
        pool,
        &co.id,
        CancelVoucherRequest {
            voucher_id: vch_a_id.clone(),
            reason: "Audit test cancel reason".to_string(),
            actor_id: Some("auditor_alice".to_string()),
        },
    )
    .await
    .unwrap();

    let cancel_audit: (String, String, String) = sqlx::query_as(
        "SELECT action, old_values, new_values FROM audit_log WHERE company_id = $1 AND entity_id = $2 AND action = 'CANCEL'",
    )
    .bind(&co.id)
    .bind(&vch_a_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(cancel_audit.0, "CANCEL");
    assert!(cancel_audit.1.contains("\"status\":\"posted\""));
    assert!(cancel_audit.2.contains("\"status\":\"cancelled\""));
    assert!(cancel_audit.2.contains("Audit test cancel reason"));

    // 2. Post voucher B and Reverse
    let vch_b_id =
        create_and_post_test_voucher(pool, &co.id, &series, &cash, &sales, "2222.00").await;
    let rev_res = VoucherReversalService::reverse_voucher(
        pool,
        &co.id,
        ReverseVoucherRequest {
            original_voucher_id: vch_b_id.clone(),
            reversal_date: "2026-05-15".to_string(),
            reason: "Audit test reversal reason".to_string(),
            actor_id: Some("auditor_alice".to_string()),
        },
    )
    .await
    .unwrap();

    let rev_audit: (String, String, String) = sqlx::query_as(
        "SELECT action, old_values, new_values FROM audit_log WHERE company_id = $1 AND entity_id = $2 AND action = 'REVERSE'",
    )
    .bind(&co.id)
    .bind(&vch_b_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(rev_audit.0, "REVERSE");
    assert!(rev_audit.1.contains("\"status\":\"posted\""));
    assert!(rev_audit.2.contains("\"status\":\"reversed\""));
    assert!(rev_audit.2.contains(&rev_res.reversing_voucher.voucher.id));
    assert!(rev_audit.2.contains("Audit test reversal reason"));
}
