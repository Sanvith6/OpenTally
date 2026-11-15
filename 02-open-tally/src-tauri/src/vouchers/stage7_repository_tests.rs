//! Sprint 2 Stage 7A Phase 2: Read-Only Search Repository Integration Tests
//! Covers all 28 required test scenarios from the Stage 7 v2.2.2 specification.

use crate::company::model::CreateCompanyRequest;
use crate::company::CompanyService;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    CancelVoucherRequest, CreateVoucherDraftRequest, CreateVoucherSeriesRequest, DateBasis,
    ReverseVoucherRequest, SortDirection, VoucherLineInput, VoucherSearchFilter, VoucherSortField,
};
use crate::vouchers::service::{
    execute_read_only_query, VoucherCancellationService, VoucherDraftService,
    VoucherPostingService, VoucherReversalService, VoucherSearchService, VoucherSeriesService,
};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage7_repo_test.db");
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

/// Helper to seed a standard test company with cash and sales ledgers and series
async fn setup_test_company(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_name: &str,
) -> (
    crate::company::model::Company,
    String, // cash ledger id
    String, // sales ledger id
    String, // sales series id
    String, // reversal series id
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
#[allow(clippy::too_many_arguments)]
async fn create_and_post_voucher(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
    series_id: &str,
    cash_id: &str,
    sales_id: &str,
    date: &str,
    amount: &str,
    narration: Option<&str>,
    reference_no: Option<&str>,
) -> String {
    let draft = VoucherDraftService::create_draft(
        pool,
        company_id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_id.to_string(),
            date: date.to_string(),
            effective_date: Some(date.to_string()),
            reference_no: reference_no.map(|s| s.to_string()),
            reference_date: Some(date.to_string()),
            narration: narration.map(|s| s.to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.to_string(),
                    entry_type: "Dr".to_string(),
                    amount: amount.to_string(),
                    narration: Some("Debit line".to_string()),
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: sales_id.to_string(),
                    entry_type: "Cr".to_string(),
                    amount: amount.to_string(),
                    narration: Some("Credit line".to_string()),
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
// 1. Read-Only Protection Tests (Test 01 to 07)
// ============================================================================

#[tokio::test]
async fn test_01_successful_query_only_activation() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    let query_only_val: i64 = execute_read_only_query(pool, |conn| {
        Box::pin(async move {
            let val: i64 = sqlx::query_scalar("PRAGMA query_only;")
                .fetch_one(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(val)
        })
    })
    .await
    .expect("Read-only query should execute successfully");

    assert_eq!(
        query_only_val, 1,
        "PRAGMA query_only must be active (1) during read-only query execution"
    );
}

#[tokio::test]
async fn test_02_search_executes_successfully_in_read_only_mode() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (company, cash_id, sales_id, series_id, _) =
        setup_test_company(pool, "Read Only Search Co").await;

    create_and_post_voucher(
        pool,
        &company.id,
        &series_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "1500.00",
        Some("Read only verification"),
        Some("RO-01"),
    )
    .await;

    let res = VoucherSearchService::search_vouchers(
        pool,
        &company.id,
        VoucherSearchFilter {
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("Search should succeed in read-only mode");

    assert_eq!(res.total, 1);
    assert_eq!(res.items[0].reference_no.as_deref(), Some("RO-01"));
}

#[tokio::test]
async fn test_03_attempted_write_fails_with_sqlite_readonly() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    let result = execute_read_only_query(pool, |conn| {
        Box::pin(async move {
            sqlx::query(
                "INSERT INTO companies (id, name, financial_year_begins) VALUES ('c_fail', 'Fail Co', '2026-04-01');",
            )
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
            Ok(())
        })
    })
    .await;

    assert!(
        result.is_err(),
        "Attempted write through read-only connection must fail"
    );
    let err_str = result.unwrap_err().to_string().to_lowercase();
    assert!(
        err_str.contains("readonly") || err_str.contains("read-only"),
        "Error message should mention readonly: {}",
        err_str
    );
}

#[tokio::test]
async fn test_04_query_failure_triggers_cleanup() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    let result: Result<(), OpenTallyError> = execute_read_only_query(pool, |_conn| {
        Box::pin(async move {
            Err(OpenTallyError::Validation(
                "Simulated query closure failure".to_string(),
            ))
        })
    })
    .await;

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Validation error: Simulated query closure failure"
    );

    // Verify subsequent pool acquire works cleanly
    let pragma_val: i64 = sqlx::query_scalar("PRAGMA query_only;")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        pragma_val, 0,
        "Pool connection must not have query_only active after aborted read-only execution"
    );
}

#[tokio::test]
async fn test_05_and_06_and_07_no_contaminated_connection_returns_to_pool() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    // 1. Run a successful read-only query
    let _ = execute_read_only_query(pool, |conn| {
        Box::pin(async move {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companies;")
                .fetch_one(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(count)
        })
    })
    .await
    .unwrap();

    // 2. Run a failed write attempt (triggering rollback + close)
    let _ = execute_read_only_query(pool, |conn| {
        Box::pin(async move {
            sqlx::query("DELETE FROM companies WHERE id = 'nonexistent';")
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(())
        })
    })
    .await;

    // 3. Acquire and execute write operations to guarantee no connection in the pool is poisoned
    for i in 0..5 {
        let mut conn = pool.acquire().await.unwrap();
        let query_only: i64 = sqlx::query_scalar("PRAGMA query_only;")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(
            query_only, 0,
            "Contaminated connection with query_only = ON must never return to the pool"
        );
        drop(conn);

        // Verify write operations succeed through the pool
        CompanyService::create_company(
            pool,
            CreateCompanyRequest {
                name: format!("Pool Test Co {}", i),
                mailing_name: Some("Pool Test Co".to_string()),
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
        .expect("Write operation must succeed on pool connection");
    }
}

// ============================================================================
// 2. Company Isolation Tests (Test 08 to 11)
// ============================================================================

#[tokio::test]
async fn test_08_search_returns_only_requested_company_vouchers() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp_a, cash_a, sales_a, ser_a, _) = setup_test_company(pool, "Company Alpha").await;
    let (comp_b, cash_b, sales_b, ser_b, _) = setup_test_company(pool, "Company Beta").await;

    create_and_post_voucher(
        pool,
        &comp_a.id,
        &ser_a,
        &cash_a,
        &sales_a,
        "2026-05-10",
        "100.00",
        Some("Alpha voucher"),
        Some("A-01"),
    )
    .await;

    create_and_post_voucher(
        pool,
        &comp_b.id,
        &ser_b,
        &cash_b,
        &sales_b,
        "2026-05-10",
        "200.00",
        Some("Beta voucher"),
        Some("B-01"),
    )
    .await;

    // Search Company A
    let res_a = VoucherSearchService::search_vouchers(
        pool,
        &comp_a.id,
        VoucherSearchFilter {
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res_a.total, 1);
    assert_eq!(res_a.items[0].company_id, comp_a.id);
    assert_eq!(res_a.items[0].reference_no.as_deref(), Some("A-01"));

    // Search Company B
    let res_b = VoucherSearchService::search_vouchers(
        pool,
        &comp_b.id,
        VoucherSearchFilter {
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res_b.total, 1);
    assert_eq!(res_b.items[0].company_id, comp_b.id);
    assert_eq!(res_b.items[0].reference_no.as_deref(), Some("B-01"));
}

#[tokio::test]
async fn test_09_ledger_filter_cannot_expose_other_company_vouchers() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp_a, cash_a, sales_a, ser_a, _) = setup_test_company(pool, "Company A").await;
    let (comp_b, cash_b, sales_b, ser_b, _) = setup_test_company(pool, "Company B").await;

    create_and_post_voucher(
        pool,
        &comp_a.id,
        &ser_a,
        &cash_a,
        &sales_a,
        "2026-05-10",
        "500.00",
        Some("Alpha"),
        None,
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp_b.id,
        &ser_b,
        &cash_b,
        &sales_b,
        "2026-05-10",
        "500.00",
        Some("Beta"),
        None,
    )
    .await;

    // Filter Company A with Company B's cash ledger ID
    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp_a.id,
        VoucherSearchFilter {
            ledger_id: Some(cash_b),
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        res.total, 0,
        "Ledger filter with foreign company ledger must yield zero records"
    );
}

#[tokio::test]
async fn test_10_reversal_lineage_cannot_cross_company_boundaries() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp_a, _, _, _, _) = setup_test_company(pool, "Company A").await;
    let (comp_b, cash_b, sales_b, ser_b, _) = setup_test_company(pool, "Company B").await;

    let v_b = create_and_post_voucher(
        pool,
        &comp_b.id,
        &ser_b,
        &cash_b,
        &sales_b,
        "2026-05-10",
        "100.00",
        None,
        None,
    )
    .await;

    // Attempt to fetch lineage for Company B's voucher through Company A
    let res = VoucherSearchService::get_voucher_lineage(pool, &comp_a.id, &v_b).await;
    assert!(res.is_err(), "Lineage query across company must fail");
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("not found"),
        "Error must indicate not found: {}",
        err_str
    );
}

#[tokio::test]
async fn test_11_empty_optional_filters_preserves_company_isolation() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp_a, cash_a, sales_a, ser_a, _) = setup_test_company(pool, "Company Alpha").await;
    let (comp_b, cash_b, sales_b, ser_b, _) = setup_test_company(pool, "Company Beta").await;

    create_and_post_voucher(
        pool,
        &comp_a.id,
        &ser_a,
        &cash_a,
        &sales_a,
        "2026-05-10",
        "100.00",
        None,
        None,
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp_b.id,
        &ser_b,
        &cash_b,
        &sales_b,
        "2026-05-10",
        "200.00",
        None,
        None,
    )
    .await;

    // Search with completely empty filter (all None)
    let res_a =
        VoucherSearchService::search_vouchers(pool, &comp_a.id, VoucherSearchFilter::default())
            .await
            .unwrap();

    assert_eq!(res_a.total, 1);
    assert_eq!(res_a.items[0].company_id, comp_a.id);
}

// ============================================================================
// 3. Search Correctness Tests (Test 12 to 25)
// ============================================================================

#[tokio::test]
async fn test_12_voucher_type_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_sales, _) = setup_test_company(pool, "Voucher Type Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_sales,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "500.00",
        None,
        None,
    )
    .await;

    let res_sales = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            voucher_type_id: Some("vt_sales".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_sales.total, 1);

    let res_receipt = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            voucher_type_id: Some("vt_receipt".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_receipt.total, 0);
}

#[tokio::test]
async fn test_13_series_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_1, _) = setup_test_company(pool, "Series Co").await;

    let ser_2 = VoucherSeriesService::create_series(
        pool,
        &comp.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Sales Secondary".to_string(),
            series_code: "SALES2".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("SAL2/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_1,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        None,
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_2.id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "200.00",
        None,
        None,
    )
    .await;

    let res_ser1 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            series_id: Some(ser_1.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_ser1.total, 1);
    assert_eq!(res_ser1.items[0].series_id, ser_1);

    let res_ser2 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            series_id: Some(ser_2.id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_ser2.total, 1);
    assert_eq!(res_ser2.items[0].series_id, ser_2.id);
}

#[tokio::test]
async fn test_14_status_filtering_including_default_and_all() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _rev_ser_id) =
        setup_test_company(pool, "Status Filter Co").await;

    // 1. Posted voucher
    let _v_posted_id = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        None,
    )
    .await;

    // 2. Draft voucher
    let v_draft = VoucherDraftService::create_draft(
        pool,
        &comp.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: ser_id.clone(),
            date: "2026-05-11".to_string(),
            effective_date: Some("2026-05-11".to_string()),
            reference_no: None,
            reference_date: None,
            narration: Some("Draft voucher".to_string()),
            lines: None,
        },
    )
    .await
    .unwrap();

    // 3. Cancelled voucher
    let v_to_cancel = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-12",
        "200.00",
        None,
        None,
    )
    .await;
    VoucherCancellationService::cancel_voucher(
        pool,
        &comp.id,
        CancelVoucherRequest {
            voucher_id: v_to_cancel.clone(),
            reason: "Cancelled order".to_string(),
            actor_id: Some("operator".to_string()),
        },
    )
    .await
    .unwrap();

    // 4. Reversed voucher
    let v_to_reverse = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-13",
        "300.00",
        None,
        None,
    )
    .await;
    VoucherReversalService::reverse_voucher(
        pool,
        &comp.id,
        ReverseVoucherRequest {
            original_voucher_id: v_to_reverse,
            reversal_date: "2026-05-14".to_string(),
            reason: "Reversing test".to_string(),
            actor_id: Some("operator".to_string()),
        },
    )
    .await
    .unwrap();

    // Verification 1: Default status (None) excludes drafts and returns posted, cancelled, reversed (plus the reversal journal)
    let res_default =
        VoucherSearchService::search_vouchers(pool, &comp.id, VoucherSearchFilter::default())
            .await
            .unwrap();
    // posted (v_posted_id) + cancelled (v_to_cancel) + reversed (v_to_reverse) + reversing journal (posted) = 4 records
    assert_eq!(res_default.total, 4);
    assert!(
        res_default.items.iter().all(|i| i.status != "draft"),
        "Default status filter must exclude drafts"
    );

    // Verification 2: Explicit "draft" returns only draft
    let res_draft = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("draft".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_draft.total, 1);
    assert_eq!(res_draft.items[0].id, v_draft.voucher.id);

    // Verification 3: Explicit "all" returns all 5 records (including draft)
    let res_all = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_all.total, 5);

    // Verification 4: Explicit "posted" returns only posted
    let res_posted = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("posted".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    // v_posted_id + reversing journal = 2 posted records
    assert_eq!(res_posted.total, 2);
    assert!(res_posted.items.iter().all(|i| i.status == "posted"));

    // Verification 5: Explicit "cancelled" returns only cancelled
    let res_cancelled = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("cancelled".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_cancelled.total, 1);
    assert_eq!(res_cancelled.items[0].id, v_to_cancel);
}

#[tokio::test]
async fn test_15_transaction_date_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Date Range Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-01",
        "100.00",
        None,
        Some("D1"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "200.00",
        None,
        Some("D2"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-20",
        "300.00",
        None,
        Some("D3"),
    )
    .await;

    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            date_from: Some("2026-05-05".to_string()),
            date_to: Some("2026-05-15".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(res.items[0].reference_no.as_deref(), Some("D2"));
}

#[tokio::test]
async fn test_16_effective_date_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Effective Date Co").await;

    // Voucher with transaction date 2026-05-01, effective date 2026-05-25
    let draft = VoucherDraftService::create_draft(
        pool,
        &comp.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: ser_id,
            date: "2026-05-01".to_string(),
            effective_date: Some("2026-05-25".to_string()),
            reference_no: Some("EFF-01".to_string()),
            reference_date: Some("2026-05-01".to_string()),
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

    VoucherPostingService::post_voucher(pool, &comp.id, &draft.voucher.id, Some("operator"))
        .await
        .unwrap();

    // Query on TransactionDate basis between 2026-05-20 and 2026-05-30 -> should return 0
    let res_tx = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            date_basis: Some(DateBasis::TransactionDate),
            date_from: Some("2026-05-20".to_string()),
            date_to: Some("2026-05-30".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_tx.total, 0);

    // Query on EffectiveDate basis between 2026-05-20 and 2026-05-30 -> should return 1
    let res_eff = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            date_basis: Some(DateBasis::EffectiveDate),
            date_from: Some("2026-05-20".to_string()),
            date_to: Some("2026-05-30".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_eff.total, 1);
    assert_eq!(res_eff.items[0].reference_no.as_deref(), Some("EFF-01"));
}

#[tokio::test]
async fn test_17_ledger_touching_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Ledger Touching Co").await;

    // Create a 3rd ledger (Bank Account)
    let bank_id = uuid::Uuid::new_v4().to_string();
    let bank_group_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = $1 AND name = 'Bank Accounts'",
    )
    .bind(&comp.id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, ledger_classification)
         VALUES ($1, $2, $3, 'HDFC Bank', 'standard')",
    )
    .bind(&bank_id)
    .bind(&comp.id)
    .bind(&bank_group_id)
    .execute(pool)
    .await
    .unwrap();

    // Voucher 1: Cash -> Sales
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        Some("V1"),
    )
    .await;

    // Voucher 2: Bank -> Sales
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &bank_id,
        &sales_id,
        "2026-05-10",
        "200.00",
        None,
        Some("V2"),
    )
    .await;

    // Filter by Cash ledger
    let res_cash = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            ledger_id: Some(cash_id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_cash.total, 1);
    assert_eq!(res_cash.items[0].reference_no.as_deref(), Some("V1"));

    // Filter by Bank ledger
    let res_bank = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            ledger_id: Some(bank_id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_bank.total, 1);
    assert_eq!(res_bank.items[0].reference_no.as_deref(), Some("V2"));

    // Filter by Sales ledger (touches both)
    let res_sales = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            ledger_id: Some(sales_id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_sales.total, 2);
}

#[tokio::test]
async fn test_18_text_filtering_and_wildcard_escaping() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Wildcard Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        Some("Special 10% discount applied"),
        Some("REF-PCT"),
    )
    .await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "200.00",
        Some("Item_code_123 with underscore"),
        Some("REF_UNDER"),
    )
    .await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "300.00",
        Some(r"Path C:\projects\ledger with backslash"),
        Some(r"REF\SLASH"),
    )
    .await;

    // 1. Search literal '%'
    let res_pct = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            search_query: Some("10%".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_pct.total, 1);
    assert_eq!(res_pct.items[0].reference_no.as_deref(), Some("REF-PCT"));

    // 2. Search literal '_'
    let res_under = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            search_query: Some("code_123".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_under.total, 1);
    assert_eq!(
        res_under.items[0].reference_no.as_deref(),
        Some("REF_UNDER")
    );

    // 3. Search literal '\'
    let res_slash = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            search_query: Some(r"\projects\".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_slash.total, 1);
    assert_eq!(
        res_slash.items[0].reference_no.as_deref(),
        Some(r"REF\SLASH")
    );
}

#[tokio::test]
async fn test_19_reversal_ancestry_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _rev_ser_id) =
        setup_test_company(pool, "Reversal Ancestry Co").await;

    let v_orig = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        Some("ORIG-01"),
    )
    .await;

    let v_rev = VoucherReversalService::reverse_voucher(
        pool,
        &comp.id,
        ReverseVoucherRequest {
            original_voucher_id: v_orig.clone(),
            reversal_date: "2026-05-11".to_string(),
            reason: "Customer cancellation".to_string(),
            actor_id: Some("operator".to_string()),
        },
    )
    .await
    .unwrap();

    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            reversal_of_voucher_id: Some(v_orig.clone()),
            status: Some("all".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(res.items[0].id, v_rev.reversing_voucher.voucher.id);
    assert_eq!(
        res.items[0].reversal_of_voucher_id.as_deref(),
        Some(v_orig.as_str())
    );
}

#[tokio::test]
async fn test_20_and_21_exact_amount_boundary_filtering() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Amount Range Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "10.00",
        None,
        Some("A1"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "50.25",
        None,
        Some("A2"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        Some("A3"),
    )
    .await;

    // Filter min 50.25, max 100.00 (inclusive)
    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            min_amount: Some("50.25".to_string()),
            max_amount: Some("100.00".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res.total, 2);
    let refs: Vec<&str> = res
        .items
        .iter()
        .map(|i| i.reference_no.as_deref().unwrap())
        .collect();
    assert!(refs.contains(&"A2"));
    assert!(refs.contains(&"A3"));
}

#[tokio::test]
async fn test_22_null_amount_handling_in_drafts() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, _, _, ser_id, _) = setup_test_company(pool, "Draft Null Co").await;

    // Create an empty draft with no lines (total_amount is NULL)
    let draft = VoucherDraftService::create_draft(
        pool,
        &comp.id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: ser_id,
            date: "2026-05-10".to_string(),
            effective_date: Some("2026-05-10".to_string()),
            reference_no: Some("NULL-DRAFT".to_string()),
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    assert!(draft.voucher.total_amount.is_none());

    // Query with min_amount: "0" and max_amount: "0" (NULL is coalesced to 0.0000)
    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("draft".to_string()),
            min_amount: Some("0".to_string()),
            max_amount: Some("0".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(res.items[0].reference_no.as_deref(), Some("NULL-DRAFT"));

    // Query with min_amount: "0.01" must exclude the NULL draft
    let res_pos = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("draft".to_string()),
            min_amount: Some("0.01".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res_pos.total, 0);
}

#[tokio::test]
async fn test_23_and_24_sorting_and_direction() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Sort Direction Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "0.05",
        None,
        Some("V-0.05"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "2.50",
        None,
        Some("V-2.50"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "10.00",
        None,
        Some("V-10.00"),
    )
    .await;
    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        Some("V-100.00"),
    )
    .await;

    // TC-SRCH-10: Ascending TotalAmount sort must produce numerical order (0.05 < 2.50 < 10.00 < 100.00)
    // NOT alphabetical order ("10.00" < "100.00" < "2.50")
    let res_asc = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let amounts: Vec<&str> = res_asc
        .items
        .iter()
        .map(|i| i.total_amount.as_deref().unwrap())
        .collect();
    assert_eq!(amounts, vec!["0.05", "2.50", "10.00", "100.00"]);

    // Descending TotalAmount sort
    let res_desc = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Desc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let amounts_desc: Vec<&str> = res_desc
        .items
        .iter()
        .map(|i| i.total_amount.as_deref().unwrap())
        .collect();
    assert_eq!(amounts_desc, vec!["100.00", "10.00", "2.50", "0.05"]);
}

#[tokio::test]
async fn test_25_deterministic_tie_breaking() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Tie Breaker Co").await;

    for i in 1..=5 {
        create_and_post_voucher(
            pool,
            &comp.id,
            &ser_id,
            &cash_id,
            &sales_id,
            "2026-05-10",
            "100.00",
            None,
            Some(&format!("TIE-{}", i)),
        )
        .await;
    }

    // Repeated executions of sort by Date DESC should always yield identical ordering
    let res1 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            sort_by: Some(VoucherSortField::Date),
            sort_direction: Some(SortDirection::Desc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let res2 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            sort_by: Some(VoucherSortField::Date),
            sort_direction: Some(SortDirection::Desc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let ids1: Vec<&str> = res1.items.iter().map(|i| i.id.as_str()).collect();
    let ids2: Vec<&str> = res2.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(
        ids1, ids2,
        "Tie breaking must produce 100% deterministic ordering"
    );
}

// ============================================================================
// 4. Regression & Audit Coverage Tests (Test 26 to 29)
// ============================================================================

#[tokio::test]
async fn test_26_cancellation_reason_extracted_from_audit_log() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Cancel Reason Co").await;

    let v_id = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "750.00",
        None,
        Some("CN-01"),
    )
    .await;

    VoucherCancellationService::cancel_voucher(
        pool,
        &comp.id,
        CancelVoucherRequest {
            voucher_id: v_id,
            reason: "Customer order rejected due to terms disagreement".to_string(),
            actor_id: Some("auditor".to_string()),
        },
    )
    .await
    .unwrap();

    let res = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            status: Some("cancelled".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(res.total, 1);
    assert_eq!(
        res.items[0].cancellation_reason.as_deref(),
        Some("Customer order rejected due to terms disagreement")
    );
}

#[tokio::test]
async fn test_27_lineage_recursive_cte_traversal() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _rev_ser_id) =
        setup_test_company(pool, "Lineage CTE Co").await;

    let v_orig = create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "1000.00",
        None,
        Some("ORIG-CHAIN"),
    )
    .await;

    let v_rev = VoucherReversalService::reverse_voucher(
        pool,
        &comp.id,
        ReverseVoucherRequest {
            original_voucher_id: v_orig.clone(),
            reversal_date: "2026-05-11".to_string(),
            reason: "Reversing test chain".to_string(),
            actor_id: Some("operator".to_string()),
        },
    )
    .await
    .unwrap();

    // Query lineage starting from original voucher
    let lineage_from_orig = VoucherSearchService::get_voucher_lineage(pool, &comp.id, &v_orig)
        .await
        .unwrap();
    assert_eq!(lineage_from_orig.items.len(), 2);
    assert_eq!(lineage_from_orig.max_depth_reached, 1);
    assert!(!lineage_from_orig.has_exceeded_depth_limit);

    // Query lineage starting from reversal voucher
    let lineage_from_rev = VoucherSearchService::get_voucher_lineage(
        pool,
        &comp.id,
        &v_rev.reversing_voucher.voucher.id,
    )
    .await
    .unwrap();
    assert_eq!(lineage_from_rev.items.len(), 2);
    assert_eq!(lineage_from_rev.max_depth_reached, 1);
    assert!(!lineage_from_rev.has_exceeded_depth_limit);
}

#[tokio::test]
async fn test_28_search_does_not_allocate_sequence_numbers() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) =
        setup_test_company(pool, "No Sequence Alloc Co").await;

    create_and_post_voucher(
        pool,
        &comp.id,
        &ser_id,
        &cash_id,
        &sales_id,
        "2026-05-10",
        "100.00",
        None,
        None,
    )
    .await;

    // Check sequence next_number before search
    let next_before: i64 = sqlx::query_scalar(
        "SELECT next_number FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&comp.id)
    .bind(&ser_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // Run 25 search queries
    for _ in 0..25 {
        let _ =
            VoucherSearchService::search_vouchers(pool, &comp.id, VoucherSearchFilter::default())
                .await
                .unwrap();
    }

    // Check sequence next_number after search
    let next_after: i64 = sqlx::query_scalar(
        "SELECT next_number FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
    )
    .bind(&comp.id)
    .bind(&ser_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(
        next_before, next_after,
        "Search operations must NEVER allocate sequence numbers"
    );
}

#[tokio::test]
async fn test_29_pagination_checked_arithmetic() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();
    let (comp, cash_id, sales_id, ser_id, _) = setup_test_company(pool, "Pagination Co").await;

    for i in 1..=5 {
        create_and_post_voucher(
            pool,
            &comp.id,
            &ser_id,
            &cash_id,
            &sales_id,
            "2026-05-10",
            &format!("{}.00", i * 10),
            None,
            Some(&format!("PAG-{}", i)),
        )
        .await;
    }

    // Page 1 with size 2
    let p1 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            page: Some(1),
            page_size: Some(2),
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(p1.total, 5);
    assert_eq!(p1.items.len(), 2);
    assert_eq!(p1.page, 1);
    assert_eq!(p1.page_size, 2);
    assert_eq!(p1.total_pages, 3);
    assert_eq!(p1.items[0].reference_no.as_deref(), Some("PAG-1"));
    assert_eq!(p1.items[1].reference_no.as_deref(), Some("PAG-2"));

    // Page 2 with size 2
    let p2 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            page: Some(2),
            page_size: Some(2),
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(p2.items.len(), 2);
    assert_eq!(p2.page, 2);
    assert_eq!(p2.items[0].reference_no.as_deref(), Some("PAG-3"));
    assert_eq!(p2.items[1].reference_no.as_deref(), Some("PAG-4"));

    // Page 3 with size 2 (only 1 remaining item)
    let p3 = VoucherSearchService::search_vouchers(
        pool,
        &comp.id,
        VoucherSearchFilter {
            page: Some(3),
            page_size: Some(2),
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(p3.items.len(), 1);
    assert_eq!(p3.page, 3);
    assert_eq!(p3.items[0].reference_no.as_deref(), Some("PAG-5"));
}
