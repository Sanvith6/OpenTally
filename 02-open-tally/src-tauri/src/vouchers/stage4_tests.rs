//! Stage 4: Voucher Creation & Draft Management Test Suite
//! Mapped to Scenarios TC-VCH-158 through TC-VCH-185.

use crate::database::migrations::{
    run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
    run_stage3_migration,
};
use crate::database::DatabaseManager;
use crate::vouchers::model::*;
use crate::vouchers::service::{VoucherDraftService, VoucherSeriesService};
use sqlx::{Pool, Sqlite};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new_migrated_sprint2() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage4_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");
        let pool = db.pool();
        run_sprint1_migration(pool)
            .await
            .expect("Sprint 1 migration failed");
        run_stage3_migration(pool)
            .await
            .expect("Stage 3 migration failed");
        run_sprint2_migration(pool)
            .await
            .expect("Sprint 2 migration failed");
        run_sprint2_stage4_hardening_migration(pool)
            .await
            .expect("Stage 4 hardening migration failed");
        Self {
            db,
            _temp_dir: temp_dir,
        }
    }
}

async fn insert_test_company(pool: &Pool<Sqlite>, company_id: &str, name: &str, precision: i64) {
    sqlx::query(
        "INSERT OR IGNORE INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_decimal_places)
         VALUES ($1, $2, $2, 'India', 'Maharashtra', '2026-04-01', '2026-04-01', $3)",
    )
    .bind(company_id)
    .bind(name)
    .bind(precision)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_test_group(
    pool: &Pool<Sqlite>,
    company_id: &str,
    group_id: &str,
    name: &str,
    nature: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined)
         VALUES ($1, $2, $3, NULL, $4, 1, 0)",
    )
    .bind(group_id)
    .bind(company_id)
    .bind(name)
    .bind(nature)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_test_ledger(
    pool: &Pool<Sqlite>,
    company_id: &str,
    ledger_id: &str,
    group_id: &str,
    name: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ($1, $2, $3, $4, 0, 'standard')",
    )
    .bind(ledger_id)
    .bind(company_id)
    .bind(group_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

async fn setup_company_and_series(
    pool: &Pool<Sqlite>,
    company_id: &str,
    series_code: &str,
) -> (VoucherNumberSeries, String, String) {
    insert_test_company(pool, company_id, &format!("Test Company {}", company_id), 2).await;
    insert_test_group(
        pool,
        company_id,
        &format!("grp_{}", company_id),
        "Current Assets",
        "Assets",
    )
    .await;

    let led1 = format!("led1_{}", company_id);
    let led2 = format!("led2_{}", company_id);
    insert_test_ledger(
        pool,
        company_id,
        &led1,
        &format!("grp_{}", company_id),
        "Bank A/c",
    )
    .await;
    insert_test_ledger(
        pool,
        company_id,
        &led2,
        &format!("grp_{}", company_id),
        "Customer A/c",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: format!("Sales Series {}", series_code),
            series_code: series_code.to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some(format!("{}/{{YY-YY}}/", series_code)),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    (series, led1, led2)
}

// -------------------------------------------------------------------------
// TC-VCH-158: Create draft with provisional DRAFT-{uuid} number
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_158_create_draft_provisional_identifier() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_158", "S158").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-05-10".to_string(),
        effective_date: Some("2026-05-10".to_string()),
        reference_no: Some("REF-158".to_string()),
        reference_date: Some("2026-05-09".to_string()),
        narration: Some("Draft invoice for May".to_string()),
        lines: None,
    };

    let result = VoucherDraftService::create_draft(pool, "comp_158", req)
        .await
        .unwrap();

    assert_eq!(result.voucher.status, "draft");
    assert!(result.voucher.voucher_number.starts_with("DRAFT-"));
    assert_eq!(result.voucher.sequence_number, None);
    assert_eq!(result.voucher.period_start, None);
    assert_eq!(result.voucher.date, "2026-05-10");
    assert_eq!(result.lines.len(), 0);
}

// -------------------------------------------------------------------------
// TC-VCH-159: Zero sequence numbers consumed during draft creation
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_159_zero_sequence_numbers_consumed() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_159", "S159").await;

    for i in 1..=5 {
        let req = CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: format!("2026-05-{:02}", i + 10),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        };
        VoucherDraftService::create_draft(pool, "comp_159", req)
            .await
            .unwrap();
    }

    let seq_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_number_sequences WHERE company_id = 'comp_159'",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(
        seq_count, 0,
        "No sequence rows should be created or consumed for drafts"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-160: Rejection of draft with invalid calendar date (2026-02-30)
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_160_rejection_invalid_calendar_date() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_160", "S160").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-02-30".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: None,
    };

    let res = VoucherDraftService::create_draft(pool, "comp_160", req).await;
    assert!(res.is_err(), "Should reject non-existent calendar date");
}

// -------------------------------------------------------------------------
// TC-VCH-161: Rejection of draft date earlier than books_beginning_from
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_161_rejection_before_books_beginning() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_161", "S161").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-03-31".to_string(), // Books begin 2026-04-01
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: None,
    };

    let res = VoucherDraftService::create_draft(pool, "comp_161", req).await;
    assert!(
        res.is_err(),
        "Should reject date earlier than books beginning"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-162: Tenant isolation - series belonging to another company
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_162_tenant_isolation_cross_company_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series_a, _, _) = setup_company_and_series(pool, "comp_162_a", "S162A").await;
    setup_company_and_series(pool, "comp_162_b", "S162B").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series_a.id.clone(),
        date: "2026-05-15".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: None,
    };

    let res = VoucherDraftService::create_draft(pool, "comp_162_b", req).await;
    assert!(res.is_err(), "Company B cannot use Company A's series");
}

// -------------------------------------------------------------------------
// TC-VCH-163: Voucher type isolation - series belonging to another type
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_163_voucher_type_isolation_cross_type_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (sales_series, _, _) = setup_company_and_series(pool, "comp_163", "S163").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_purchase".to_string(), // Mismatched voucher type
        series_id: sales_series.id.clone(),
        date: "2026-05-15".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: None,
    };

    let res = VoucherDraftService::create_draft(pool, "comp_163", req).await;
    assert!(
        res.is_err(),
        "Cannot use Sales series with Purchase voucher type"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-164: Rejection of deactivated series (is_active = 0)
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_164_rejection_deactivated_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_164", "S164").await;

    VoucherSeriesService::deactivate_series(pool, "comp_164", &series.id)
        .await
        .unwrap();

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-05-15".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: None,
    };

    let res = VoucherDraftService::create_draft(pool, "comp_164", req).await;
    assert!(res.is_err(), "Cannot create draft using deactivated series");
}

// -------------------------------------------------------------------------
// TC-VCH-165: Get draft returns composite VoucherWithLines
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_165_get_draft_composite() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_165", "S165").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-06-01".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: Some("Composite draft test".to_string()),
        lines: Some(vec![
            VoucherLineInput {
                id: None,
                ledger_id: led1.clone(),
                entry_type: "Dr".to_string(),
                amount: "5000.00".to_string(),
                narration: Some("Debit leg".to_string()),
            },
            VoucherLineInput {
                id: None,
                ledger_id: led2.clone(),
                entry_type: "Cr".to_string(),
                amount: "5000.00".to_string(),
                narration: Some("Credit leg".to_string()),
            },
        ]),
    };

    let created = VoucherDraftService::create_draft(pool, "comp_165", req)
        .await
        .unwrap();

    let fetched = VoucherDraftService::get_draft(pool, "comp_165", &created.voucher.id)
        .await
        .unwrap();
    assert_eq!(fetched.voucher.id, created.voucher.id);
    assert_eq!(fetched.lines.len(), 2);
    assert_eq!(fetched.debit_total, "5000.00");
    assert_eq!(fetched.credit_total, "5000.00");
    assert_eq!(fetched.difference, "0.00");
    assert!(fetched.is_balanced);
}

// -------------------------------------------------------------------------
// TC-VCH-166: List drafts with pagination
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_166_list_drafts_pagination() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_166", "S166").await;

    for i in 1..=5 {
        VoucherDraftService::create_draft(
            pool,
            "comp_166",
            CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_id: series.id.clone(),
                date: format!("2026-06-{:02}", i),
                effective_date: None,
                reference_no: None,
                reference_date: None,
                narration: None,
                lines: None,
            },
        )
        .await
        .unwrap();
    }

    let filter = VoucherFilter {
        page: Some(1),
        page_size: Some(2),
        ..Default::default()
    };

    let page1 = VoucherDraftService::list_drafts(pool, "comp_166", filter)
        .await
        .unwrap();
    assert_eq!(page1.total, 5);
    assert_eq!(page1.items.len(), 2);
    assert_eq!(page1.total_pages, 3);
    assert_eq!(page1.page, 1);
}

// -------------------------------------------------------------------------
// TC-VCH-167: Filter drafts by voucher_type_id, series_id, and date range
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_167_filter_drafts() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series1, _, _) = setup_company_and_series(pool, "comp_167", "S167A").await;

    let series2 = VoucherSeriesService::create_series(
        pool,
        "comp_167",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Series B".to_string(),
            series_code: "S167B".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("S167B/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Create draft in series1 in June
    VoucherDraftService::create_draft(
        pool,
        "comp_167",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series1.id.clone(),
            date: "2026-06-15".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    // Create draft in series2 in July
    VoucherDraftService::create_draft(
        pool,
        "comp_167",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series2.id.clone(),
            date: "2026-07-20".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: None,
        },
    )
    .await
    .unwrap();

    // Filter by series1
    let filter_s1 = VoucherFilter {
        series_id: Some(series1.id.clone()),
        ..Default::default()
    };
    let res_s1 = VoucherDraftService::list_drafts(pool, "comp_167", filter_s1)
        .await
        .unwrap();
    assert_eq!(res_s1.items.len(), 1);
    assert_eq!(res_s1.items[0].series_id, series1.id);

    // Filter by date range July
    let filter_date = VoucherFilter {
        date_from: Some("2026-07-01".to_string()),
        date_to: Some("2026-07-31".to_string()),
        ..Default::default()
    };
    let res_date = VoucherDraftService::list_drafts(pool, "comp_167", filter_date)
        .await
        .unwrap();
    assert_eq!(res_date.items.len(), 1);
    assert_eq!(res_date.items[0].series_id, series2.id);
}

// -------------------------------------------------------------------------
// TC-VCH-168: Update draft header details
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_168_update_draft_header() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_168", "S168").await;

    let created = VoucherDraftService::create_draft(
        pool,
        "comp_168",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: Some("OLD-REF".to_string()),
            reference_date: None,
            narration: Some("Old narration".to_string()),
            lines: None,
        },
    )
    .await
    .unwrap();

    let updated = VoucherDraftService::update_draft_header(
        pool,
        "comp_168",
        &created.voucher.id,
        UpdateVoucherDraftRequest {
            voucher_type_id: None,
            series_id: None,
            date: Some("2026-05-20".to_string()),
            effective_date: Some("2026-05-21".to_string()),
            reference_no: Some("NEW-REF".to_string()),
            reference_date: Some("2026-05-19".to_string()),
            narration: Some("New narration".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.date, "2026-05-20");
    assert_eq!(updated.effective_date, "2026-05-21");
    assert_eq!(updated.reference_no, Some("NEW-REF".to_string()));
    assert_eq!(updated.narration, Some("New narration".to_string()));
}

// -------------------------------------------------------------------------
// TC-VCH-169: Hard deletion of draft voucher with cascade line purge
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_169_delete_draft_cascade_lines() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_169", "S169").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_169",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: led1,
                entry_type: "Dr".to_string(),
                amount: "1000.00".to_string(),
                narration: None,
            }]),
        },
    )
    .await
    .unwrap();

    let v_id = draft.voucher.id.clone();
    VoucherDraftService::delete_draft(pool, "comp_169", &v_id)
        .await
        .unwrap();

    let v_exists: Option<String> = sqlx::query_scalar("SELECT id FROM vouchers WHERE id = $1")
        .bind(&v_id)
        .fetch_optional(pool)
        .await
        .unwrap();
    assert!(v_exists.is_none());

    let lines_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = $1")
            .bind(&v_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(lines_count, 0, "Lines must be cascaded on draft deletion");
}

// -------------------------------------------------------------------------
// TC-VCH-170: Immutability - attempt to update or delete non-draft voucher fails
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_170_immutability_non_draft_rejection() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_170", "S170").await;

    // Insert a posted voucher directly in database
    let v_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ($1, 'comp_170', 'vt_sales', $2, 'automatic', '2026-04-01', 1, 'S170/26-27/0001', '2026-05-10', '2026-05-10', 'posted', '1000.00')"
    )
    .bind(&v_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    // Attempt to update via draft service
    let update_res = VoucherDraftService::update_draft_header(
        pool,
        "comp_170",
        &v_id,
        UpdateVoucherDraftRequest {
            narration: Some("Hacked".to_string()),
            ..Default::default()
        },
    )
    .await;
    assert!(
        update_res.is_err(),
        "Must reject modifying posted voucher via draft service"
    );

    // Attempt to delete via draft service
    let delete_res = VoucherDraftService::delete_draft(pool, "comp_170", &v_id).await;
    assert!(
        delete_res.is_err(),
        "Must reject deleting posted voucher via draft service"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-171: Add line item with valid Dr entry and canonical decimal
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_171_add_line_dr() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_171", "S171").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_171",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let line = VoucherDraftService::add_line(
        pool,
        "comp_171",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led1.clone(),
            entry_type: "Dr".to_string(),
            amount: "1500.50".to_string(),
            narration: Some("Dr line".to_string()),
            line_number: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(line.line_number, 1);
    assert_eq!(line.entry_type, "Dr");
    assert_eq!(line.amount, "1500.50");
    assert_eq!(line.ledger_id, led1);
}

// -------------------------------------------------------------------------
// TC-VCH-172: Add line item with valid Cr entry
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_172_add_line_cr() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_172", "S172").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_172",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: led1,
                entry_type: "Dr".to_string(),
                amount: "1500.00".to_string(),
                narration: None,
            }]),
        },
    )
    .await
    .unwrap();

    let line2 = VoucherDraftService::add_line(
        pool,
        "comp_172",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led2.clone(),
            entry_type: "Cr".to_string(),
            amount: "1500.00".to_string(),
            narration: Some("Cr line".to_string()),
            line_number: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(line2.line_number, 2);
    assert_eq!(line2.entry_type, "Cr");
    assert_eq!(line2.amount, "1500.00");
}

// -------------------------------------------------------------------------
// TC-VCH-173: Rejection of invalid entry_type ("Debit", "invalid")
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_173_rejection_invalid_entry_type() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_173", "S173").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_173",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let res = VoucherDraftService::add_line(
        pool,
        "comp_173",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led1,
            entry_type: "Debit".to_string(),
            amount: "100.00".to_string(),
            narration: None,
            line_number: None,
        },
    )
    .await;

    assert!(
        res.is_err(),
        "Must reject 'Debit'; only 'Dr' or 'Cr' accepted"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-174: Rejection of non-canonical amounts ("10.", ".50", "01.00")
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_174_rejection_non_canonical_amounts() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_174", "S174").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_174",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let invalid_amounts = vec!["10.", ".50", "01.00", "10..00", "abc"];
    for bad_amt in invalid_amounts {
        let res = VoucherDraftService::add_line(
            pool,
            "comp_174",
            &draft.voucher.id,
            AddVoucherLineRequest {
                ledger_id: led1.clone(),
                entry_type: "Dr".to_string(),
                amount: bad_amt.to_string(),
                narration: None,
                line_number: None,
            },
        )
        .await;
        assert!(
            res.is_err(),
            "Must reject non-canonical amount: {}",
            bad_amt
        );
    }
}

// -------------------------------------------------------------------------
// TC-VCH-175: Rejection of zero ("0", "0.00") and negative amounts
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_175_rejection_zero_and_negative_amounts() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_175", "S175").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_175",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let bad_amounts = vec!["0", "0.00", "-50.00", "-1"];
    for bad_amt in bad_amounts {
        let res = VoucherDraftService::add_line(
            pool,
            "comp_175",
            &draft.voucher.id,
            AddVoucherLineRequest {
                ledger_id: led1.clone(),
                entry_type: "Dr".to_string(),
                amount: bad_amt.to_string(),
                narration: None,
                line_number: None,
            },
        )
        .await;
        assert!(
            res.is_err(),
            "Must reject zero or negative amount: {}",
            bad_amt
        );
    }
}

// -------------------------------------------------------------------------
// TC-VCH-176: Rejection of amount with scale exceeding currency_decimal_places
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_176_rejection_scale_exceeding_precision() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    // Company 176 has currency_decimal_places = 2
    let (series, led1, _) = setup_company_and_series(pool, "comp_176", "S176").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_176",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let res = VoucherDraftService::add_line(
        pool,
        "comp_176",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led1,
            entry_type: "Dr".to_string(),
            amount: "150.123".to_string(), // 3 decimal places
            narration: None,
            line_number: None,
        },
    )
    .await;

    assert!(
        res.is_err(),
        "Must reject 3 decimal places for 2-decimal currency"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-177: Line item referencing ledger belonging to another company
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_177_cross_company_ledger_rejected() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series_a, _, _) = setup_company_and_series(pool, "comp_177_a", "S177A").await;
    let (_, led_b, _) = setup_company_and_series(pool, "comp_177_b", "S177B").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_177_a",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series_a.id.clone(),
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

    let res = VoucherDraftService::add_line(
        pool,
        "comp_177_a",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led_b, // Belongs to company B!
            entry_type: "Dr".to_string(),
            amount: "500.00".to_string(),
            narration: None,
            line_number: None,
        },
    )
    .await;

    assert!(
        res.is_err(),
        "Cannot add ledger belonging to different company"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-178: Line item referencing nonexistent ledger_id
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_178_nonexistent_ledger_rejected() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_178", "S178").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_178",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let res = VoucherDraftService::add_line(
        pool,
        "comp_178",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: "nonexistent_ledger_id".to_string(),
            entry_type: "Dr".to_string(),
            amount: "500.00".to_string(),
            narration: None,
            line_number: None,
        },
    )
    .await;

    assert!(res.is_err(), "Must reject nonexistent ledger ID");
}

// -------------------------------------------------------------------------
// TC-VCH-179: Update line item amount and narration
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_179_update_line_item() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_179", "S179").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_179",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: led1,
                entry_type: "Dr".to_string(),
                amount: "1000.00".to_string(),
                narration: Some("Initial".to_string()),
            }]),
        },
    )
    .await
    .unwrap();

    let line_id = draft.lines[0].id.clone();
    let updated_line = VoucherDraftService::update_line(
        pool,
        "comp_179",
        &draft.voucher.id,
        &line_id,
        UpdateVoucherLineRequest {
            amount: Some("2500.00".to_string()),
            narration: Some("Updated narration".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(updated_line.amount, "2500.00");
    assert_eq!(
        updated_line.narration,
        Some("Updated narration".to_string())
    );
}

// -------------------------------------------------------------------------
// TC-VCH-180: Delete line item automatically renumbers remaining lines
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_180_delete_line_renumbers_contiguously() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_180", "S180").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_180",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: led1.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: led2.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "200.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: led1.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "300.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Delete middle line (line 2)
    let middle_line_id = draft.lines[1].id.clone();
    let third_line_id = draft.lines[2].id.clone();

    VoucherDraftService::delete_line(pool, "comp_180", &draft.voucher.id, &middle_line_id)
        .await
        .unwrap();

    let fetched = VoucherDraftService::get_draft(pool, "comp_180", &draft.voucher.id)
        .await
        .unwrap();

    assert_eq!(fetched.lines.len(), 2);
    assert_eq!(fetched.lines[0].line_number, 1);
    assert_eq!(fetched.lines[1].id, third_line_id);
    assert_eq!(
        fetched.lines[1].line_number, 2,
        "Third line should be renumbered to 2"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-181: Reorder lines preserves deterministic 1-based numbering
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_181_reorder_lines_deterministic() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_181", "S181").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_181",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: led1.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: led2.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "200.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: led1.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "300.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let l1 = draft.lines[0].id.clone();
    let l2 = draft.lines[1].id.clone();
    let l3 = draft.lines[2].id.clone();

    // Invert order: [l3, l2, l1]
    let reordered = VoucherDraftService::reorder_lines(
        pool,
        "comp_181",
        &draft.voucher.id,
        ReorderVoucherLinesRequest {
            line_ids: vec![l3.clone(), l2.clone(), l1.clone()],
        },
    )
    .await
    .unwrap();

    assert_eq!(reordered.len(), 3);
    assert_eq!(reordered[0].id, l3);
    assert_eq!(reordered[0].line_number, 1);
    assert_eq!(reordered[1].id, l2);
    assert_eq!(reordered[1].line_number, 2);
    assert_eq!(reordered[2].id, l1);
    assert_eq!(reordered[2].line_number, 3);
}

// -------------------------------------------------------------------------
// TC-VCH-182: Reorder rejects mismatched line ID list
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_182_reorder_rejects_mismatched_ids() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_182", "S182").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_182",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: led1,
                entry_type: "Dr".to_string(),
                amount: "100.00".to_string(),
                narration: None,
            }]),
        },
    )
    .await
    .unwrap();

    let res = VoucherDraftService::reorder_lines(
        pool,
        "comp_182",
        &draft.voucher.id,
        ReorderVoucherLinesRequest {
            line_ids: vec!["bogus_id".to_string()],
        },
    )
    .await;

    assert!(
        res.is_err(),
        "Must reject reordering with non-existent line IDs"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-183: Temporary imbalance permitted in drafts
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_183_temporary_imbalance_permitted_in_drafts() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_183", "S183").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_183",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: led1,
                    entry_type: "Dr".to_string(),
                    amount: "1000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: led2,
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
    assert_eq!(draft.debit_total, "1000.00");
    assert_eq!(draft.credit_total, "500.00");
    assert_eq!(draft.difference, "500.00");
    assert!(
        !draft.is_balanced,
        "Draft with Dr 1000 and Cr 500 must show is_balanced = false without error"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-184: Atomic draft creation rolls back on line failure
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_184_atomic_draft_creation_rollback() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_184", "S184").await;

    let req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-05-10".to_string(),
        effective_date: None,
        reference_no: None,
        reference_date: None,
        narration: None,
        lines: Some(vec![
            VoucherLineInput {
                id: None,
                ledger_id: led1,
                entry_type: "Dr".to_string(),
                amount: "1000.00".to_string(),
                narration: None,
            },
            VoucherLineInput {
                id: None,
                ledger_id: "nonexistent_ledger".to_string(), // Injected error on line 2!
                entry_type: "Cr".to_string(),
                amount: "1000.00".to_string(),
                narration: None,
            },
        ]),
    };

    let res = VoucherDraftService::create_draft(pool, "comp_184", req).await;
    assert!(
        res.is_err(),
        "Must fail due to nonexistent ledger on line 2"
    );

    let v_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id = 'comp_184'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        v_count, 0,
        "No orphaned voucher header should exist after rollback"
    );

    let l_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines WHERE company_id = 'comp_184'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(l_count, 0, "No orphaned lines should exist after rollback");
}

// -------------------------------------------------------------------------
// TC-VCH-185: Atomic draft sync (header + lines replacement)
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_185_atomic_save_draft_replacement() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_185", "S185").await;

    // 1. Initial draft creation with 1 line
    let initial = VoucherDraftService::create_draft(
        pool,
        "comp_185",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: Some("Draft v1".to_string()),
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: led1.clone(),
                entry_type: "Dr".to_string(),
                amount: "100.00".to_string(),
                narration: None,
            }]),
        },
    )
    .await
    .unwrap();

    // 2. Save draft replacing with 2 new balanced lines and updated date
    let save_req = SaveVoucherDraftRequest {
        voucher_id: Some(initial.voucher.id.clone()),
        voucher_type_id: "vt_sales".to_string(),
        series_id: series.id.clone(),
        date: "2026-05-15".to_string(),
        effective_date: Some("2026-05-15".to_string()),
        reference_no: Some("REF-V2".to_string()),
        reference_date: None,
        narration: Some("Draft v2 replaced".to_string()),
        lines: vec![
            VoucherLineInput {
                id: None,
                ledger_id: led1.clone(),
                entry_type: "Dr".to_string(),
                amount: "7500.00".to_string(),
                narration: Some("Dr line".to_string()),
            },
            VoucherLineInput {
                id: None,
                ledger_id: led2.clone(),
                entry_type: "Cr".to_string(),
                amount: "7500.00".to_string(),
                narration: Some("Cr line".to_string()),
            },
        ],
    };

    let saved = VoucherDraftService::save_draft(pool, "comp_185", save_req)
        .await
        .unwrap();

    assert_eq!(saved.voucher.id, initial.voucher.id);
    assert_eq!(saved.voucher.date, "2026-05-15");
    assert_eq!(
        saved.voucher.narration,
        Some("Draft v2 replaced".to_string())
    );
    assert_eq!(saved.lines.len(), 2);
    assert_eq!(saved.debit_total, "7500.00");
    assert_eq!(saved.credit_total, "7500.00");
    assert_eq!(saved.difference, "0.00");
    assert!(saved.is_balanced);
}

// =========================================================================
// Sprint 2 Stage 4 Database Integrity Hardening Tests (TD-S2-002 & TD-S2-003)
// =========================================================================

// -------------------------------------------------------------------------
// TC-VCH-186: Voucher line limit exactly 10,000 lines allowed
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_186_voucher_line_limit_exactly_10000_allowed() {
    use crate::vouchers::validator::{validate_voucher_line_count, MAX_VOUCHER_LINES};

    assert_eq!(MAX_VOUCHER_LINES, 10_000);
    assert!(validate_voucher_line_count(10_000).is_ok());

    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_186", "S186").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_186",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    // Fast batch-insert exactly 10,000 lines
    let mut tx = pool.begin().await.unwrap();
    for i in 1..=10_000 {
        let lid = format!("line_186_{}", i);
        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
             VALUES ($1, 'comp_186', $2, $3, $4, 'Dr', '1.00', NULL)",
        )
        .bind(&lid)
        .bind(&draft.voucher.id)
        .bind(i as i64)
        .bind(&led1)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE company_id = 'comp_186' AND voucher_id = $1",
    )
    .bind(&draft.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(count, 10_000);
}

// -------------------------------------------------------------------------
// TC-VCH-187: Voucher line limit 10,001 rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_187_voucher_line_limit_10001_rejected() {
    use crate::vouchers::validator::{validate_line_number, validate_voucher_line_count};

    // 1. Validator function checks
    let res_count = validate_voucher_line_count(10_001);
    assert!(res_count.is_err());
    let err_msg = res_count.unwrap_err().to_string();
    assert!(err_msg.contains("exceeds maximum allowed limit of 10000"));

    let res_ln = validate_line_number(10_001);
    assert!(res_ln.is_err());
    let ln_err = res_ln.unwrap_err().to_string();
    assert!(ln_err.contains("exceeds maximum allowed line number of 10000"));

    // 2. Service level check on add_line when 10,000 lines exist
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_187", "S187").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_187",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    // Insert 10,000 lines directly into database
    let mut tx = pool.begin().await.unwrap();
    for i in 1..=10_000 {
        let lid = format!("line_187_{}", i);
        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
             VALUES ($1, 'comp_187', $2, $3, $4, 'Dr', '1.00', NULL)",
        )
        .bind(&lid)
        .bind(&draft.voucher.id)
        .bind(i as i64)
        .bind(&led1)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    // Attempt to add line 10,001 via VoucherDraftService
    let add_res = VoucherDraftService::add_line(
        pool,
        "comp_187",
        &draft.voucher.id,
        AddVoucherLineRequest {
            ledger_id: led1.clone(),
            entry_type: "Cr".to_string(),
            amount: "100.00".to_string(),
            narration: None,
            line_number: None,
        },
    )
    .await;

    assert!(add_res.is_err());
    let err_str = add_res.unwrap_err().to_string();
    assert!(
        err_str.contains("maximum allowed lines (10000)"),
        "Unexpected error: {}",
        err_str
    );
}

// -------------------------------------------------------------------------
// TC-VCH-188: Large and corrupted line numbers rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_188_large_and_corrupted_line_numbers_rejected() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_188", "S188").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_188",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    // 1. Database check constraint rejects line_number = 10001
    let db_res_10001 = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l_bad_10001', 'comp_188', $1, 10001, $2, 'Dr', '50.00')",
    )
    .bind(&draft.voucher.id)
    .bind(&led1)
    .execute(pool)
    .await;
    assert!(
        db_res_10001.is_err(),
        "line_number 10001 must violate database check constraint"
    );

    // 2. Database check constraint rejects line_number = 50000
    let db_res_50000 = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l_bad_50000', 'comp_188', $1, 50000, $2, 'Dr', '50.00')",
    )
    .bind(&draft.voucher.id)
    .bind(&led1)
    .execute(pool)
    .await;
    assert!(
        db_res_50000.is_err(),
        "line_number 50000 must violate database check constraint"
    );

    // 3. Database check constraint rejects line_number = 0
    let db_res_0 = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l_bad_0', 'comp_188', $1, 0, $2, 'Dr', '50.00')",
    )
    .bind(&draft.voucher.id)
    .bind(&led1)
    .execute(pool)
    .await;
    assert!(
        db_res_0.is_err(),
        "line_number 0 must violate check constraint"
    );

    // 4. Database check constraint rejects line_number = -1
    let db_res_neg = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l_bad_neg', 'comp_188', $1, -1, $2, 'Dr', '50.00')",
    )
    .bind(&draft.voucher.id)
    .bind(&led1)
    .execute(pool)
    .await;
    assert!(
        db_res_neg.is_err(),
        "negative line_number must violate check constraint"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-189: Near-i64 maximum staging values rejected at database level
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_189_near_i64_max_staging_values_prevented() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, _) = setup_company_and_series(pool, "comp_189", "S189").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_189",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
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

    let near_i64_max: i64 = i64::MAX - 10;
    let res = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('l_near_max', 'comp_189', $1, $2, $3, 'Dr', '10.00')",
    )
    .bind(&draft.voucher.id)
    .bind(near_i64_max)
    .bind(&led1)
    .execute(pool)
    .await;

    assert!(
        res.is_err(),
        "Near-i64 maximum line_number must be rejected by check constraint"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-190: Failed renumbering rollback restores lines
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_190_failed_renumbering_rollback_preserves_lines() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, led1, led2) = setup_company_and_series(pool, "comp_190", "S190").await;

    let draft = VoucherDraftService::create_draft(
        pool,
        "comp_190",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-10".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: Some("line_190_1".to_string()),
                    ledger_id: led1.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: Some("line_190_2".to_string()),
                    ledger_id: led2.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "100.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // Start manual transaction simulating failed Phase 2
    let mut tx = pool.begin().await.unwrap();

    // Phase 1 shift
    sqlx::query(
        "UPDATE voucher_lines SET line_number = line_number + 1000000 
         WHERE company_id = 'comp_190' AND voucher_id = $1",
    )
    .bind(&draft.voucher.id)
    .execute(&mut *tx)
    .await
    .unwrap();

    // Deliberate error: abort transaction without committing
    tx.rollback().await.unwrap();

    // Verify lines are restored to original 1 and 2
    let lines: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, line_number FROM voucher_lines WHERE voucher_id = $1 ORDER BY line_number ASC",
    )
    .bind(&draft.voucher.id)
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].1, 1);
    assert_eq!(lines[1].1, 2);
}

// -------------------------------------------------------------------------
// TC-VCH-191: Direct SQL insertion with invalid draft sequence metadata rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_191_direct_sql_invalid_draft_sequence_metadata_rejected() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_191", "S191").await;

    // 1. Draft with period_start populated and sequence_number = NULL must be REJECTED
    let res1 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
         VALUES ('v_inv_draft_1', 'comp_191', 'vt_sales', $1, 'automatic', '2026-04-01', NULL, 'DRAFT-INV-1', '2026-05-10', '2026-05-10', 'draft')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res1.is_err(),
        "Draft with non-null period_start must be rejected by chk_voucher_sequence_metadata"
    );

    // 2. Draft with sequence_number populated and period_start = NULL must be REJECTED
    let res2 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
         VALUES ('v_inv_draft_2', 'comp_191', 'vt_sales', $1, 'automatic', NULL, 1, 'DRAFT-INV-2', '2026-05-10', '2026-05-10', 'draft')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res2.is_err(),
        "Draft with non-null sequence_number must be rejected by chk_voucher_sequence_metadata"
    );

    // 3. Draft with both sequence_number and period_start populated must be REJECTED
    let res3 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
         VALUES ('v_inv_draft_3', 'comp_191', 'vt_sales', $1, 'automatic', '2026-04-01', 1, 'DRAFT-INV-3', '2026-05-10', '2026-05-10', 'draft')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res3.is_err(),
        "Draft with sequence metadata must be rejected by chk_voucher_sequence_metadata"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-192: Direct SQL insertion with inconsistent permanent voucher metadata rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_192_direct_sql_inconsistent_permanent_metadata_rejected() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_192", "S192").await;

    // 1. Posted automatic voucher with NULL sequence_number must be REJECTED
    let res1 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_inv_post_1', 'comp_192', 'vt_sales', $1, 'automatic', '2026-04-01', NULL, 'GEN/0001', '2026-05-10', '2026-05-10', 'posted', '100.00')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res1.is_err(),
        "Posted automatic voucher with NULL sequence_number must fail check constraint"
    );

    // 2. Posted automatic voucher with NULL period_start must be REJECTED
    let res2 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_inv_post_2', 'comp_192', 'vt_sales', $1, 'automatic', NULL, 1, 'GEN/0002', '2026-05-10', '2026-05-10', 'posted', '100.00')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res2.is_err(),
        "Posted automatic voucher with NULL period_start must fail check constraint"
    );

    // Create a manual series for manual voucher tests
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
         VALUES ('ser_man_192', 'comp_192', 'vt_sales', 'Manual Sales', 'MAN', 'manual', 'never', 1)",
    )
    .execute(pool)
    .await
    .unwrap();

    // 3. Posted manual voucher with non-NULL sequence_number must be REJECTED
    let res3 = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_inv_post_3', 'comp_192', 'vt_sales', 'ser_man_192', 'manual', NULL, 5, 'MAN/0005', '2026-05-10', '2026-05-10', 'posted', '100.00')",
    )
    .execute(pool)
    .await;
    assert!(
        res3.is_err(),
        "Posted manual voucher with sequence_number must fail check constraint"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-193: Valid draft and permanent voucher metadata accepted
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_193_valid_draft_and_permanent_metadata_accepted() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let (series, _, _) = setup_company_and_series(pool, "comp_193", "S193").await;

    // 1. Valid draft (NULL period_start, NULL sequence_number) accepted
    let res_draft = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
         VALUES ('v_valid_draft', 'comp_193', 'vt_sales', $1, 'automatic', NULL, NULL, 'DRAFT-VALID-1', '2026-05-10', '2026-05-10', 'draft')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res_draft.is_ok(),
        "Valid draft must be accepted: {:?}",
        res_draft.err()
    );

    // 2. Valid posted automatic voucher accepted
    let res_auto = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_valid_auto', 'comp_193', 'vt_sales', $1, 'automatic', '2026-04-01', 1, 'GEN/0001', '2026-05-10', '2026-05-10', 'posted', '500.00')",
    )
    .bind(&series.id)
    .execute(pool)
    .await;
    assert!(
        res_auto.is_ok(),
        "Valid posted automatic voucher must be accepted: {:?}",
        res_auto.err()
    );

    // Create manual series
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
         VALUES ('ser_man_193', 'comp_193', 'vt_sales', 'Manual Sales', 'MAN', 'manual', 'never', 1)",
    )
    .execute(pool)
    .await
    .unwrap();

    // 3. Valid posted manual voucher accepted
    let res_man = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_valid_man', 'comp_193', 'vt_sales', 'ser_man_193', 'manual', NULL, NULL, 'MAN-001', '2026-05-10', '2026-05-10', 'posted', '500.00')",
    )
    .execute(pool)
    .await;
    assert!(
        res_man.is_ok(),
        "Valid posted manual voucher must be accepted: {:?}",
        res_man.err()
    );
}
