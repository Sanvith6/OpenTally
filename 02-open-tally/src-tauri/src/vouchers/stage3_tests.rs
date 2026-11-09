use crate::database::migrations::{
    run_sprint1_migration, run_sprint2_migration, run_stage3_migration,
};
use crate::database::DatabaseManager;
use crate::vouchers::model::{CreateVoucherNumberConfigRequest, CreateVoucherSeriesRequest};
use crate::vouchers::service::period::{derive_period_start, validate_period_consistency};
use crate::vouchers::service::sequence::VoucherSequenceService;
use crate::vouchers::service::series::VoucherSeriesService;
use sqlx::{Pool, Sqlite};

struct TestDb {
    db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    async fn new_migrated_sprint2() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_stage3.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init DB");
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
        Self {
            db,
            _temp_dir: temp_dir,
        }
    }
}

async fn insert_test_company(
    pool: &Pool<Sqlite>,
    company_id: &str,
    name: &str,
    fy_begins: &str,
    books_begins: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ($1, $2, $2, 'India', 'Maharashtra', $3, $4)"
    )
    .bind(company_id)
    .bind(name)
    .bind(fy_begins)
    .bind(books_begins)
    .execute(pool)
    .await
    .unwrap();
}

// =========================================================================
// SECTION 1: Authoritative Period Derivation Tests
// =========================================================================

#[test]
fn test_derive_period_start_yearly_indian_fy() {
    let fy = "2026-04-01";
    let books = "2026-04-01";

    // Standard date in FY 2026-27
    assert_eq!(
        derive_period_start("2026-05-15", "yearly", fy, books).unwrap(),
        "2026-04-01"
    );

    // Date on FY start boundary
    assert_eq!(
        derive_period_start("2026-04-01", "yearly", fy, books).unwrap(),
        "2026-04-01"
    );

    // Backdated date in Jan-Mar 2027 (still FY 2026-27!)
    assert_eq!(
        derive_period_start("2027-01-15", "yearly", fy, books).unwrap(),
        "2026-04-01"
    );
    assert_eq!(
        derive_period_start("2027-03-31", "yearly", fy, books).unwrap(),
        "2026-04-01"
    );

    // Rollover to next financial year FY 2027-28
    assert_eq!(
        derive_period_start("2027-04-01", "yearly", fy, books).unwrap(),
        "2027-04-01"
    );
}

#[test]
fn test_derive_period_start_yearly_calendar_fy() {
    let fy = "2026-01-01";
    let books = "2026-01-01";

    assert_eq!(
        derive_period_start("2026-08-15", "yearly", fy, books).unwrap(),
        "2026-01-01"
    );
    assert_eq!(
        derive_period_start("2027-01-15", "yearly", fy, books).unwrap(),
        "2027-01-01"
    );
}

#[test]
fn test_derive_period_start_monthly() {
    let fy = "2026-04-01";
    let books = "2026-04-01";

    assert_eq!(
        derive_period_start("2026-08-15", "monthly", fy, books).unwrap(),
        "2026-08-01"
    );
    assert_eq!(
        derive_period_start("2026-08-31", "monthly", fy, books).unwrap(),
        "2026-08-01"
    );
    assert_eq!(
        derive_period_start("2026-09-01", "monthly", fy, books).unwrap(),
        "2026-09-01"
    );
}

#[test]
fn test_derive_period_start_daily() {
    let fy = "2026-04-01";
    let books = "2026-04-01";

    assert_eq!(
        derive_period_start("2026-08-15", "daily", fy, books).unwrap(),
        "2026-08-15"
    );
    assert_eq!(
        derive_period_start("2026-08-16", "daily", fy, books).unwrap(),
        "2026-08-16"
    );
}

#[test]
fn test_derive_period_start_never() {
    let fy = "2026-04-01";
    let books = "2026-04-01";

    assert_eq!(
        derive_period_start("2026-05-01", "never", fy, books).unwrap(),
        "2026-04-01"
    );
    assert_eq!(
        derive_period_start("2028-11-20", "never", fy, books).unwrap(),
        "2026-04-01"
    );
    assert_eq!(
        derive_period_start("2032-03-31", "never", fy, books).unwrap(),
        "2026-04-01"
    );
}

#[test]
fn test_derive_period_start_leap_year_handling() {
    // Leap year Feb 29 voucher date
    let fy = "2024-01-01";
    let books = "2024-01-01";
    assert_eq!(
        derive_period_start("2024-02-29", "yearly", fy, books).unwrap(),
        "2024-01-01"
    );

    // FY begins Feb 29 (leap year 2024-02-29) evaluated in non-leap year 2025 -> normalized to Feb 28
    let fy_leap = "2024-02-29";
    let books_leap = "2024-02-29";
    assert_eq!(
        derive_period_start("2025-05-15", "yearly", fy_leap, books_leap).unwrap(),
        "2025-02-28"
    );

    // Voucher before FY Feb 29 in leap year 2024 -> start_year is 2023 (non-leap), normalized to 2023-02-28
    let fy_2020 = "2020-02-29";
    let books_2020 = "2020-02-29";
    assert_eq!(
        derive_period_start("2024-01-15", "yearly", fy_2020, books_2020).unwrap(),
        "2023-02-28"
    );

    // Voucher before FY Feb 29 in non-leap year 2025 -> start_year is 2024 (leap year), keeps 2024-02-29
    assert_eq!(
        derive_period_start("2025-01-15", "yearly", fy_2020, books_2020).unwrap(),
        "2024-02-29"
    );
}

#[test]
fn test_derive_period_start_validates_calendar_dates() {
    let fy = "2026-04-01";
    let books = "2026-04-01";

    // Non-existent calendar date
    assert!(derive_period_start("2026-02-30", "yearly", fy, books).is_err());
    // Non-leap Feb 29
    assert!(derive_period_start("2023-02-29", "yearly", fy, books).is_err());
    // Malformed string
    assert!(derive_period_start("invalid-date", "yearly", fy, books).is_err());
}

#[test]
fn test_derive_period_start_rejects_date_before_books_beginning() {
    let fy = "2026-04-01";
    let books = "2026-08-01";

    // Date earlier than books_beginning_from is strictly rejected
    let res = derive_period_start("2026-05-15", "yearly", fy, books);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("cannot be earlier than company books beginning date"));
}

// -------------------------------------------------------------------------
// TC-VCH-126: Authoritative derive_period_start rejects inconsistent period_start
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_126_rejects_caller_supplied_inconsistent_period_start() {
    let derived = "2026-04-01";
    let voucher_date = "2026-08-15";
    let restart_period = "yearly";

    // Matching period passes
    assert!(
        validate_period_consistency(Some("2026-04-01"), derived, voucher_date, restart_period)
            .is_ok()
    );

    // Inconsistent caller-supplied period strictly rejected
    let res =
        validate_period_consistency(Some("2025-04-01"), derived, voucher_date, restart_period);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Period/date inconsistency"));
}

// =========================================================================
// SECTION 2: Effective Numbering Configuration Resolution Tests
// =========================================================================

// -------------------------------------------------------------------------
// TC-VCH-151: Single-pass configuration resolver with zero circularity
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_151_single_pass_configuration_resolver() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_151";
    insert_test_company(pool, company_id, "Company 151", "2026-04-01", "2026-04-01").await;

    // Create series with initial config
    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Tax Invoice 151".into(),
            series_code: "T151".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("T151/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let effective = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series.id,
        "2026-08-15",
    )
    .await
    .unwrap();

    assert_eq!(effective.series_id, series.id);
    assert_eq!(effective.restart_period, "yearly");
    assert_eq!(effective.period_start, "2026-04-01");
    assert_eq!(effective.start_number, 1);
    assert_eq!(effective.prefix, "T151/{YY-YY}/");
}

#[tokio::test]
async fn test_resolve_effective_config_inaugural_partial_financial_year() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_inaugural";
    // Company incorporated on August 1, 2026
    insert_test_company(
        pool,
        company_id,
        "Company Inaugural",
        "2026-04-01",
        "2026-08-01",
    )
    .await;

    // Config legitimately effective from books_beginning_from (2026-08-01)
    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Inaugural Series".into(),
            series_code: "INAUG".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("INAUG/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-08-01".into()),
            start_number: Some(500),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();

    // Effective anchor formula: max('2026-04-01', '2026-08-01') = '2026-08-01'
    // Matches config effective 2026-08-01 seamlessly!
    let effective = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series.id,
        "2026-09-15",
    )
    .await
    .unwrap();

    assert_eq!(effective.applicable_from, "2026-08-01");
    assert_eq!(effective.start_number, 500);
}

#[tokio::test]
async fn test_resolve_effective_config_multi_version_chronological_selection() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_multiversion";
    insert_test_company(
        pool,
        company_id,
        "Company Multi",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    // Version 1 for FY 26-27 (effective 2026-04-01)
    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Evolving Series".into(),
            series_code: "EVO".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("EVO-V1/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Version 2 for FY 27-28 (effective 2027-04-01)
    VoucherSeriesService::create_configuration(
        pool,
        company_id,
        &series.id,
        CreateVoucherNumberConfigRequest {
            applicable_from: "2027-04-01".into(),
            prefix: Some("EVO-V2/{YY-YY}/".into()),
            suffix: None,
            start_number: Some(100),
            width: Some(5),
        },
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();

    // Voucher dated 2026-10-15 resolves Version 1
    let cfg1 = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series.id,
        "2026-10-15",
    )
    .await
    .unwrap();
    assert_eq!(cfg1.prefix, "EVO-V1/{YY-YY}/");
    assert_eq!(cfg1.start_number, 1);
    assert_eq!(cfg1.width, 4);

    // Voucher dated 2027-05-20 resolves Version 2
    let cfg2 = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series.id,
        "2027-05-20",
    )
    .await
    .unwrap();
    assert_eq!(cfg2.prefix, "EVO-V2/{YY-YY}/");
    assert_eq!(cfg2.start_number, 100);
    assert_eq!(cfg2.width, 5);
}

#[tokio::test]
async fn test_resolve_effective_config_rejects_missing_configuration_zero_fallback() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_noconfig";
    insert_test_company(
        pool,
        company_id,
        "Company NoConfig",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    // Insert series directly into table without any configuration rows
    let series_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
         VALUES ($1, $2, 'vt_sales', 'Unconfigured Series', 'UNCFG', 'automatic', 'yearly', 1)"
    )
    .bind(&series_id)
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let res = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series_id,
        "2026-08-15",
    )
    .await;

    // Must be rejected with Validation error; ZERO silent fallback to start_number = 1
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Missing numbering configuration for series"));
    assert!(err_msg.contains(
        "Implicit fallback to default configuration (start_number = 1) is strictly prohibited"
    ));
}

#[tokio::test]
async fn test_resolve_effective_config_rejects_inactive_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_inactive";
    insert_test_company(
        pool,
        company_id,
        "Company Inactive",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Deactivated Series".into(),
            series_code: "DEACT".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("DEACT/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Deactivate series
    VoucherSeriesService::deactivate_series(pool, company_id, &series.id)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let res = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        company_id,
        &series.id,
        "2026-08-15",
    )
    .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("is inactive; posting blocked"));
}

// =========================================================================
// SECTION 3: Atomic Sequence Allocation Tests
// =========================================================================

#[tokio::test]
async fn test_atomic_sequence_allocation_first_allocation_equals_start_number() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_alloc_first";
    insert_test_company(
        pool,
        company_id,
        "Company Alloc First",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Start 100 Series".into(),
            series_code: "S100".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("INV/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(100), // Explicit starting baseline S_0 = 100
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let alloc = VoucherSequenceService::allocate_next_sequence(
        &mut tx,
        company_id,
        "vt_sales",
        &series.id,
        "2026-05-10",
    )
    .await
    .unwrap();

    assert_eq!(alloc.sequence_number, 100);
    assert_eq!(alloc.voucher_number, "INV/26-27/0100");
    assert_eq!(alloc.period_start, "2026-04-01");
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn test_atomic_sequence_allocation_monotonic_increment() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_alloc_mono";
    insert_test_company(
        pool,
        company_id,
        "Company Alloc Mono",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Monotonic Series".into(),
            series_code: "MONO".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("MONO/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let a1 = VoucherSequenceService::allocate_next_sequence(
        &mut tx,
        company_id,
        "vt_sales",
        &series.id,
        "2026-05-01",
    )
    .await
    .unwrap();
    let a2 = VoucherSequenceService::allocate_next_sequence(
        &mut tx,
        company_id,
        "vt_sales",
        &series.id,
        "2026-05-02",
    )
    .await
    .unwrap();
    let a3 = VoucherSequenceService::allocate_next_sequence(
        &mut tx,
        company_id,
        "vt_sales",
        &series.id,
        "2026-05-03",
    )
    .await
    .unwrap();

    assert_eq!(a1.sequence_number, 1);
    assert_eq!(a2.sequence_number, 2);
    assert_eq!(a3.sequence_number, 3);
    assert_eq!(a1.voucher_number, "MONO/26-27/0001");
    assert_eq!(a2.voucher_number, "MONO/26-27/0002");
    assert_eq!(a3.voucher_number, "MONO/26-27/0003");
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn test_atomic_sequence_allocation_transaction_rollback_reverts_counter() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_alloc_rollback";
    insert_test_company(
        pool,
        company_id,
        "Company Alloc Rollback",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Rollback Series".into(),
            series_code: "ROLL".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("ROLL/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // 1. Transaction 1 allocates sequence 1, but rolls back!
    {
        let mut tx = pool.begin().await.unwrap();
        let a1 = VoucherSequenceService::allocate_next_sequence(
            &mut tx,
            company_id,
            "vt_sales",
            &series.id,
            "2026-05-01",
        )
        .await
        .unwrap();
        assert_eq!(a1.sequence_number, 1);
        tx.rollback().await.unwrap(); // Explicit rollback
    }

    // 2. Transaction 2 allocates: Must re-acquire sequence 1 (zero burned numbers, zero gaps!)
    {
        let mut tx = pool.begin().await.unwrap();
        let a2 = VoucherSequenceService::allocate_next_sequence(
            &mut tx,
            company_id,
            "vt_sales",
            &series.id,
            "2026-05-01",
        )
        .await
        .unwrap();
        assert_eq!(a2.sequence_number, 1);
        assert_eq!(a2.voucher_number, "ROLL/26-27/0001");
        tx.commit().await.unwrap();
    }
}

#[tokio::test]
async fn test_atomic_sequence_allocation_rejects_manual_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_alloc_manual";
    insert_test_company(
        pool,
        company_id,
        "Company Alloc Manual",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Manual Series".into(),
            series_code: "MANUAL".into(),
            numbering_method: "manual".into(), // Manual numbering!
            restart_period: "never".into(),
            prefix: Some("MANUAL/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let res = VoucherSequenceService::allocate_next_sequence(
        &mut tx,
        company_id,
        "vt_sales",
        &series.id,
        "2026-05-01",
    )
    .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Cannot allocate sequence for non-automatic series"));
}

// =========================================================================
// SECTION 4: Sequence Recovery & Governance Tests
// =========================================================================

#[tokio::test]
async fn test_sequence_recovery_normal_aligned_counter() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_aligned";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Aligned",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Aligned Series".into(),
            series_code: "ALN".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("ALN/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Insert 3 permanent posted vouchers (seq 1, 2, 3)
    for seq in 1..=3 {
        let v_id = format!("v_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("ALN/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    // Sequence table next_number is 4
    let seq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
         VALUES ($1, $2, 'vt_sales', $3, '2026-04-01', 4)"
    )
    .bind(&seq_id)
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let rep = VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        company_id,
        &series.id,
        "2026-04-01",
        "admin_01",
        "strict",
        None,
    )
    .await
    .unwrap();

    assert_eq!(rep.start_number_baseline, 1);
    assert_eq!(rep.max_allocated, 3);
    assert_eq!(rep.previous_next_number, 4);
    assert_eq!(rep.new_next_number, 4);
    assert!(!rep.counter_updated);
    assert!(rep.historical_gaps.is_empty());
    assert!(rep.counter_tail_gaps.is_empty());
    tx.commit().await.unwrap();
}

// -------------------------------------------------------------------------
// TC-VCH-100: Sequence recovery advances lagged sequence counter
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_100_sequence_recovery_advances_lagged_counter() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_lag";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Lag",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Lag Series".into(),
            series_code: "LAG".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("LAG/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Vouchers exist up to sequence 4
    for seq in 1..=4 {
        let v_id = format!("v_lag_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("LAG/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    // Sequence table lags at next_number = 2
    let seq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
         VALUES ($1, $2, 'vt_sales', $3, '2026-04-01', 2)"
    )
    .bind(&seq_id)
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let rep = VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        company_id,
        &series.id,
        "2026-04-01",
        "admin_01",
        "strict",
        None,
    )
    .await
    .unwrap();

    assert_eq!(rep.previous_next_number, 2);
    assert_eq!(rep.new_next_number, 5); // Advanced to M + 1 = 5
    assert!(rep.counter_updated);
    tx.commit().await.unwrap();
}

// -------------------------------------------------------------------------
// TC-VCH-120: Sequence recovery preserves ahead counter (never lowered)
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_120_sequence_recovery_preserves_ahead_counter_never_decrements() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_ahead";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Ahead",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Ahead Series".into(),
            series_code: "AHD".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("AHD/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Vouchers exist up to sequence 2
    for seq in 1..=2 {
        let v_id = format!("v_ahd_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("AHD/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    // Sequence table counter is ahead at 6
    let seq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
         VALUES ($1, $2, 'vt_sales', $3, '2026-04-01', 6)"
    )
    .bind(&seq_id)
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    // Under allow_historical_gaps with justification, counter is verified and NEVER lowered
    let mut tx = pool.begin().await.unwrap();
    let rep = VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        company_id,
        &series.id,
        "2026-04-01",
        "admin_01",
        "allow_historical_gaps",
        Some("Approved gap jump during system testing"),
    )
    .await
    .unwrap();

    assert_eq!(rep.previous_next_number, 6);
    assert_eq!(rep.new_next_number, 6); // Preserved at 6, NOT lowered to 3!
    assert!(!rep.counter_updated);
    assert_eq!(rep.counter_tail_gaps, vec![3, 4, 5]);
    tx.commit().await.unwrap();
}

// -------------------------------------------------------------------------
// TC-VCH-118: Sequence recovery baseline with arbitrary start_number = 100
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_118_sequence_recovery_arbitrary_start_number_baseline() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_100";
    insert_test_company(
        pool,
        company_id,
        "Company Rec 100",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Base 100 Series".into(),
            series_code: "B100".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("B100/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(100), // Starting baseline 100
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Insert vouchers with sequence numbers 100, 101, 102
    for seq in 100..=102 {
        let v_id = format!("v_100_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("B100/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    let mut conn = pool.acquire().await.unwrap();
    let rep =
        VoucherSequenceService::inspect_sequence(&mut conn, company_id, &series.id, "2026-04-01")
            .await
            .unwrap();

    // Baseline is 100; numbers 1..99 must NOT be flagged as gaps!
    assert_eq!(rep.start_number_baseline, 100);
    assert_eq!(rep.max_allocated, 102);
    assert!(rep.historical_gaps.is_empty());
}

#[tokio::test]
async fn test_sequence_recovery_detects_allocations_below_baseline() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_below";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Below",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Base 100 Below Series".into(),
            series_code: "BBLW".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("BBLW/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(100),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Illegally inserted voucher with sequence_number = 42 (< 100 baseline!)
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('v_below_42', $1, 'vt_sales', $2, 'automatic', '2026-04-01', 42, 'BBLW/26-27/0042', '2026-05-01', '2026-05-01', 'posted', '100.00')"
    )
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let res =
        VoucherSequenceService::inspect_sequence(&mut conn, company_id, &series.id, "2026-04-01")
            .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Allocations detected below configured start_number"));
}

// -------------------------------------------------------------------------
// TC-VCH-112: Sequence recovery detects historical gaps in strict mode
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_112_sequence_recovery_detects_historical_gaps_in_strict_mode() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_hist_gap";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Hist Gap",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Gap Series".into(),
            series_code: "GAP".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("GAP/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Vouchers exist for 1 and 3 (gap at sequence 2!)
    for seq in [1, 3] {
        let v_id = format!("v_gap_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("GAP/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    let mut tx = pool.begin().await.unwrap();
    let res = VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        company_id,
        &series.id,
        "2026-04-01",
        "admin_01",
        "strict", // Strict mode
        None,
    )
    .await;

    // Strict mode strictly aborts upon detecting sequence gaps!
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Sequence gaps detected"));
    assert!(err_msg.contains("historical_gaps=[2]"));
}

// -------------------------------------------------------------------------
// TC-VCH-123: Sequence recovery detects counter-tail gaps in strict mode
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_123_sequence_recovery_detects_counter_tail_gaps_in_strict_mode() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_tail_gap";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Tail Gap",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Tail Gap Series".into(),
            series_code: "TAIL".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("TAIL/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Vouchers exist for 1 and 2 (M = 2)
    for seq in 1..=2 {
        let v_id = format!("v_tail_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("TAIL/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    // Sequence table next_number is 6 -> tail gaps [3, 4, 5]
    let seq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
         VALUES ($1, $2, 'vt_sales', $3, '2026-04-01', 6)"
    )
    .bind(&seq_id)
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let res = VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        company_id,
        &series.id,
        "2026-04-01",
        "admin_01",
        "strict",
        None,
    )
    .await;

    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Sequence gaps detected"));
    assert!(err_msg.contains("counter_tail_gaps=[3, 4, 5]"));
}

// -------------------------------------------------------------------------
// TC-VCH-124: Administrative override of gaps with audit trail
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_124_sequence_recovery_allow_historical_gaps_mode_with_audit_trail() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_audit";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Audit",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Audit Gap Series".into(),
            series_code: "AUDG".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("AUDG/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    for seq in [1, 3] {
        let v_id = format!("v_aud_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', 'posted', '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("AUDG/26-27/{:04}", seq))
        .execute(pool)
        .await
        .unwrap();
    }

    // 1. Without reason -> rejected
    {
        let mut tx = pool.begin().await.unwrap();
        let res_no_reason = VoucherSequenceService::resync_voucher_sequence(
            &mut tx,
            company_id,
            &series.id,
            "2026-04-01",
            "admin_01",
            "allow_historical_gaps",
            None, // Missing reason!
        )
        .await;
        assert!(res_no_reason.is_err());
        assert!(res_no_reason
            .unwrap_err()
            .to_string()
            .contains("resolution_reason is mandatory"));
    }

    // 2. With reason -> succeeds and writes to audit_log
    {
        let mut tx = pool.begin().await.unwrap();
        let rep = VoucherSequenceService::resync_voucher_sequence(
            &mut tx,
            company_id,
            &series.id,
            "2026-04-01",
            "admin_01",
            "allow_historical_gaps",
            Some("Approved gap 2 by Statutory Auditor ref #AUD-88"),
        )
        .await
        .unwrap();

        assert_eq!(rep.historical_gaps, vec![2]);
        tx.commit().await.unwrap();

        // Verify audit log record exists
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE company_id = $1 AND entity_type = 'voucher_number_sequences' AND performed_by = 'admin_01'"
        )
        .bind(company_id)
        .fetch_one(pool)
        .await
        .unwrap();

        assert_eq!(audit_count, 1);
    }
}

// -------------------------------------------------------------------------
// TC-VCH-133: Cancelled and reversed vouchers retain sequence numbers
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_133_cancelled_and_reversed_vouchers_included_in_recovery() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_cancelled";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Cancelled",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series = VoucherSeriesService::create_series(
        pool,
        company_id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Status Series".into(),
            series_code: "STAT".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("STAT/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // V1 = posted, V2 = cancelled, V3 = reversed, V4 = posted, V5 = cancelled
    let statuses = [
        (1, "posted"),
        (2, "cancelled"),
        (3, "reversed"),
        (4, "posted"),
        (5, "cancelled"),
    ];
    for (seq, st) in statuses {
        let v_id = format!("v_stat_{}", seq);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
             VALUES ($1, $2, 'vt_sales', $3, 'automatic', '2026-04-01', $4, $5, '2026-05-01', '2026-05-01', $6, '100.00')"
        )
        .bind(&v_id)
        .bind(company_id)
        .bind(&series.id)
        .bind(seq)
        .bind(format!("STAT/26-27/{:04}", seq))
        .bind(st)
        .execute(pool)
        .await
        .unwrap();
    }

    // Sequence table holds next_number = 6
    let seq_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
         VALUES ($1, $2, 'vt_sales', $3, '2026-04-01', 6)"
    )
    .bind(&seq_id)
    .bind(company_id)
    .bind(&series.id)
    .execute(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let rep =
        VoucherSequenceService::inspect_sequence(&mut conn, company_id, &series.id, "2026-04-01")
            .await
            .unwrap();

    // Cancelled and reversed vouchers are retained in evidence! M = 5, expected_next = 6, ZERO false gaps!
    assert_eq!(rep.max_allocated, 5);
    assert_eq!(rep.total_permanent_allocations, 5);
    assert_eq!(rep.previous_next_number, 6);
    assert!(rep.historical_gaps.is_empty());
    assert!(rep.counter_tail_gaps.is_empty());
}

// -------------------------------------------------------------------------
// TC-VCH-122: Missing numbering configuration rejection on sequence recovery
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_122_sequence_recovery_rejects_missing_configuration() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_rec_missing_cfg";
    insert_test_company(
        pool,
        company_id,
        "Company Rec Missing Cfg",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    // Series without configuration
    let series_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
         VALUES ($1, $2, 'vt_sales', 'Uncfg Series', 'UNCFG2', 'automatic', 'yearly', 1)"
    )
    .bind(&series_id)
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let res =
        VoucherSequenceService::inspect_sequence(&mut conn, company_id, &series_id, "2026-04-01")
            .await;

    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Missing numbering configuration for series"));
}

#[tokio::test]
async fn test_cross_company_isolation_on_sequence_allocation_and_recovery() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();

    insert_test_company(
        pool,
        "comp_iso_a",
        "Company ISO A",
        "2026-04-01",
        "2026-04-01",
    )
    .await;
    insert_test_company(
        pool,
        "comp_iso_b",
        "Company ISO B",
        "2026-04-01",
        "2026-04-01",
    )
    .await;

    let series_a = VoucherSeriesService::create_series(
        pool,
        "comp_iso_a",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".into(),
            series_name: "Series A".into(),
            series_code: "SERA".into(),
            numbering_method: "automatic".into(),
            restart_period: "yearly".into(),
            prefix: Some("A/{YY-YY}/".into()),
            suffix: None,
            applicable_from: Some("2026-04-01".into()),
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();

    // Attempting to resolve or allocate series_a under comp_iso_b must fail with NotFound
    let res_resolve = VoucherSequenceService::resolve_effective_config(
        &mut conn,
        "comp_iso_b", // Cross-company!
        &series_a.id,
        "2026-05-01",
    )
    .await;
    assert!(res_resolve.is_err());

    let res_rec = VoucherSequenceService::inspect_sequence(
        &mut conn,
        "comp_iso_b", // Cross-company!
        &series_a.id,
        "2026-04-01",
    )
    .await;
    assert!(res_rec.is_err());
}
