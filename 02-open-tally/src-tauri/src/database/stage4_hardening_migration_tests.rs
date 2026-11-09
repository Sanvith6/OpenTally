//! Stage 4 Hardening Forward Migration `0.3.1-sprint2` Test Suite
//! Tests the forward migration from `0.3.0-sprint2` to `0.3.1-sprint2`.

use crate::database::migrations::{
    run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
    run_stage3_migration, CANONICAL_STAGE4_DEPENDENT_TRIGGERS,
};
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::vouchers::model::{CreateVoucherDraftRequest, ReorderVoucherLinesRequest};
use crate::vouchers::service::VoucherDraftService;
use sqlx::{Pool, Sqlite};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_031_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");
        Self {
            db,
            _temp_dir: temp_dir,
        }
    }

    pub async fn new_migrated_030() -> Self {
        let test_db = Self::new().await;
        let pool = test_db.db.pool();
        run_sprint1_migration(pool)
            .await
            .expect("Sprint 1 migration failed");
        run_stage3_migration(pool)
            .await
            .expect("Stage 3 migration failed");
        run_sprint2_migration(pool)
            .await
            .expect("Sprint 2 migration failed");
        test_db
    }
}

async fn insert_test_company(pool: &Pool<Sqlite>, company_id: &str) {
    sqlx::query(
        "INSERT OR IGNORE INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ($1, $2, $2, 'India', 'Maharashtra', '2026-04-01', '2026-04-01')"
    )
    .bind(company_id)
    .bind(format!("Company {}", company_id))
    .execute(pool)
    .await
    .expect("Failed to insert company");

    sqlx::query(
        "INSERT OR IGNORE INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_test', $1, 'Direct Group', NULL, 'Expenses', 1)",
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert group");

    sqlx::query(
        "INSERT OR IGNORE INTO ledgers (id, company_id, group_id, name, ledger_classification)
         VALUES ('led_cash', $1, 'grp_test', 'Cash', 'standard')",
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert Cash ledger");

    sqlx::query(
        "INSERT OR IGNORE INTO ledgers (id, company_id, group_id, name, ledger_classification)
         VALUES ('led_pnl', $1, 'grp_test', 'Profit & Loss A/c', 'standard')",
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert PnL ledger");
}

async fn insert_test_series(
    pool: &Pool<Sqlite>,
    company_id: &str,
    voucher_type_id: &str,
    series_id: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ($1, $2, $3, $4, $4, 'automatic', 'yearly')"
    )
    .bind(series_id)
    .bind(company_id)
    .bind(voucher_type_id)
    .bind(format!("Series {}", series_id))
    .execute(pool)
    .await
    .expect("Failed to insert series");
}

#[tokio::test]
async fn test_mig_031_01_existing_030_upgrade_preserves_all_records_and_metadata() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_031_1";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_1").await;

    // Insert sample draft with multiple lines in unhardened 0.3.0 schema
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_draft_1', $1, 'vt_sales', 'ser_sales_1', 'automatic', NULL, NULL, 'DRAFT-001', '2026-11-05', '2026-11-05', 'draft', '1000.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert draft voucher");

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
         VALUES ('line_1', $1, 'vch_draft_1', 1, 'led_cash', 'Dr', '1000.00', 'Cash entry')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert line 1");

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
         VALUES ('line_2', $1, 'vch_draft_1', 2, 'led_pnl', 'Cr', '1000.00', 'Offset entry')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .expect("Failed to insert line 2");

    // Execute forward migration 0.3.1-sprint2
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Forward migration 0.3.1 failed");

    // Verify metadata version
    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.1-sprint2");

    // Verify record counts
    let vch_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vouchers")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(vch_count, 1);

    let line_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(line_count, 2);

    // Verify line details preserved byte-for-byte
    let line1: (i64, String, String) = sqlx::query_as(
        "SELECT line_number, entry_type, amount FROM voucher_lines WHERE id = 'line_1'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(line1, (1, "Dr".to_string(), "1000.00".to_string()));
}

#[tokio::test]
async fn test_mig_031_02_idempotent_031_skip() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();

    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Initial forward migration failed");

    // Re-run migration; must succeed idempotently
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Idempotent second run failed");

    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.1-sprint2");
}

#[tokio::test]
async fn test_mig_031_03_unsupported_source_version_rejected() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    // Currently at 0.1.0-sprint0
    let err = run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect_err("Should reject unsupported schema version");

    assert!(matches!(err, OpenTallyError::Migration(_)));
}

#[tokio::test]
async fn test_mig_031_04_target_constraints_enforced_post_migration() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_031_chk";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_chk").await;

    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration failed");

    // 1. Test line number > 10000 rejected by chk_voucher_line_number_bounds
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_test_bounds', $1, 'vt_sales', 'ser_sales_chk', 'automatic', NULL, NULL, 'DRAFT-BOUNDS', '2026-11-05', '2026-11-05', 'draft', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    let res_bad_line = sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('line_bad', $1, 'vch_test_bounds', 10001, 'led_cash', 'Dr', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await;

    assert!(
        res_bad_line.is_err(),
        "Line number 10001 must be rejected by chk_voucher_line_number_bounds"
    );

    // 2. Test draft sequence loophole rejected by hardened chk_voucher_sequence_metadata
    let res_bad_draft = sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_bad_draft', $1, 'vt_sales', 'ser_sales_chk', 'automatic', '2026-04-01', NULL, 'DRAFT-LOOPHOLE', '2026-11-05', '2026-11-05', 'draft', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await;

    assert!(
        res_bad_draft.is_err(),
        "Draft with period_start must be rejected by hardened chk_voucher_sequence_metadata"
    );
}

#[tokio::test]
async fn test_mig_031_05_pre_migration_validation_rejects_corrupted_lines_above_10000() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_corrupt";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_corrupt").await;

    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_corrupt', $1, 'vt_sales', 'ser_sales_corrupt', 'automatic', NULL, NULL, 'DRAFT-CORRUPT', '2026-11-05', '2026-11-05', 'draft', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    // Required fail-closed test values outside active 1..10000:
    // 10001, 1000000, 1000001, 1010000, 1010001, i64::MAX
    let test_values: [i64; 6] = [10001, 1000000, 1000001, 1010000, 1010001, i64::MAX];

    for (idx, &bad_line_num) in test_values.iter().enumerate() {
        let line_id = format!("line_corrupt_{}", idx);

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
             VALUES ($1, $2, 'vch_corrupt', $3, 'led_cash', 'Dr', '100.00')"
        )
        .bind(&line_id)
        .bind(company_id)
        .bind(bad_line_num)
        .execute(pool)
        .await
        .unwrap();

        let err = run_sprint2_stage4_hardening_migration(pool)
            .await
            .expect_err(&format!(
                "Pre-migration validation must reject out-of-bounds line_number {}",
                bad_line_num
            ));

        assert!(
            matches!(err, OpenTallyError::IntegrityViolation(_)),
            "Expected IntegrityViolation for line_number {}, got {:?}",
            bad_line_num,
            err
        );

        // Verify rollback: version remains 0.3.0-sprint2
        let version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(version, "0.3.0-sprint2");

        // Verify data immutability: record is preserved exactly as inserted (not repaired, not truncated, not renumbered)
        let persisted_line_num: i64 =
            sqlx::query_scalar("SELECT line_number FROM voucher_lines WHERE id = $1")
                .bind(&line_id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(
            persisted_line_num, bad_line_num,
            "Original line number must not be silently repaired or truncated"
        );

        // Clean up the line for the next test iteration
        sqlx::query("DELETE FROM voucher_lines WHERE id = $1")
            .bind(&line_id)
            .execute(pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn test_mig_031_06_pre_migration_validation_rejects_invalid_sequence_metadata() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_bad_seq";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_bad_seq").await;

    // Use dedicated connection to seed anomalous draft with non-null period_start
    let mut conn = pool.acquire().await.unwrap();
    sqlx::raw_sql("PRAGMA ignore_check_constraints = ON;")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_bad_meta', $1, 'vt_sales', 'ser_sales_bad_seq', 'automatic', '2026-04-01', NULL, 'DRAFT-BAD-META', '2026-11-05', '2026-11-05', 'draft', '100.00')"
    )
    .bind(company_id)
    .execute(&mut *conn)
    .await
    .unwrap();
    sqlx::raw_sql("PRAGMA ignore_check_constraints = OFF;")
        .execute(&mut *conn)
        .await
        .unwrap();
    drop(conn);

    let err = run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect_err("Pre-migration validation must reject invalid draft sequence metadata");

    assert!(matches!(err, OpenTallyError::IntegrityViolation(_)));

    // Verify rollback
    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.0-sprint2");
}

#[tokio::test]
async fn test_mig_031_07_all_dependent_indexes_and_triggers_preserved() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_objects";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_objects").await;

    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration failed");

    // Verify canonical indexes exist
    let expected_indexes = [
        "idx_vouchers_date",
        "idx_vouchers_type_status",
        "idx_vouchers_series_ownership",
        "uq_vouchers_permanent_number",
        "uq_vouchers_series_sequence",
        "uq_vouchers_reversal",
        "idx_voucher_lines_ledger",
    ];

    for idx_name in expected_indexes {
        let exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'index' AND name = $1",
        )
        .bind(idx_name)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(
            exists,
            "Expected index '{}' must exist after migration",
            idx_name
        );
    }

    // Verify all 14 canonical triggers exist
    for trg_name in CANONICAL_STAGE4_DEPENDENT_TRIGGERS {
        let exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'trigger' AND name = $1",
        )
        .bind(trg_name)
        .fetch_one(pool)
        .await
        .unwrap();
        assert!(
            exists,
            "Expected trigger '{}' must exist after migration",
            trg_name
        );
    }
}

#[tokio::test]
async fn test_mig_031_08_empty_tables_handled_gracefully() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();

    // Tables are empty; run migration
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration on empty tables must succeed");

    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.1-sprint2");
}

#[tokio::test]
async fn test_mig_031_09_fresh_installation_chain_convergence() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    // Full fresh chain: 0.1.0 -> 0.2.0 -> 0.2.1 -> 0.3.0 -> 0.3.1
    run_sprint1_migration(pool).await.unwrap();
    run_stage3_migration(pool).await.unwrap();
    run_sprint2_migration(pool).await.unwrap();
    run_sprint2_stage4_hardening_migration(pool).await.unwrap();

    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.1-sprint2");
}

#[tokio::test]
async fn test_mig_031_10_tc_vch_194_line_mutation_invariant_zero_staging_values_persisted() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_invariants_194";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_invariants").await;

    run_sprint2_stage4_hardening_migration(pool).await.unwrap();

    let draft = VoucherDraftService::create_draft(
        pool,
        company_id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_sales".into(),
            series_id: "ser_invariants".into(),
            date: "2026-11-05".into(),
            effective_date: Some("2026-11-05".into()),
            reference_no: None,
            reference_date: None,
            narration: None,
            lines: Some(vec![
                crate::vouchers::model::VoucherLineInput {
                    id: Some("l1".into()),
                    ledger_id: "led_cash".into(),
                    entry_type: "Dr".into(),
                    amount: "100.00".into(),
                    narration: None,
                },
                crate::vouchers::model::VoucherLineInput {
                    id: Some("l2".into()),
                    ledger_id: "led_pnl".into(),
                    entry_type: "Cr".into(),
                    amount: "100.00".into(),
                    narration: None,
                },
                crate::vouchers::model::VoucherLineInput {
                    id: Some("l3".into()),
                    ledger_id: "led_cash".into(),
                    entry_type: "Dr".into(),
                    amount: "50.00".into(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    // 1. Reorder lines
    let reordered_ids = vec![
        draft.lines[2].id.clone(),
        draft.lines[0].id.clone(),
        draft.lines[1].id.clone(),
    ];
    VoucherDraftService::reorder_lines(
        pool,
        company_id,
        &draft.voucher.id,
        ReorderVoucherLinesRequest {
            line_ids: reordered_ids,
        },
    )
    .await
    .unwrap();

    // Invariant Check 1: Zero staging values exist
    let staging_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = $1 AND (line_number < 1 OR line_number > 10000)"
    ).bind(&draft.voucher.id).fetch_one(pool).await.unwrap();
    assert_eq!(
        staging_count, 0,
        "Invariant violation: staging line numbers persisted after reorder"
    );

    // 2. Delete middle line
    VoucherDraftService::delete_line(pool, company_id, &draft.voucher.id, &draft.lines[0].id)
        .await
        .unwrap();

    // Invariant Check 2: Zero staging values and contiguous 1..N
    let staging_count_post_del: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = $1 AND (line_number < 1 OR line_number > 10000)"
    ).bind(&draft.voucher.id).fetch_one(pool).await.unwrap();
    assert_eq!(
        staging_count_post_del, 0,
        "Invariant violation: staging line numbers persisted after delete"
    );

    let max_line: i64 =
        sqlx::query_scalar("SELECT MAX(line_number) FROM voucher_lines WHERE voucher_id = $1")
            .bind(&draft.voucher.id)
            .fetch_one(pool)
            .await
            .unwrap();
    let total_lines: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = $1")
            .bind(&draft.voucher.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(max_line, total_lines);
}

#[tokio::test]
async fn test_mig_031_11_application_startup_sequence_idempotent_restart() {
    let test_db = TestDb::new().await;
    let pool = test_db.db.pool();

    // Pass 1: Run the full startup migration sequence
    run_sprint1_migration(pool)
        .await
        .expect("Sprint 1 migration failed on pass 1");
    run_stage3_migration(pool)
        .await
        .expect("Stage 3 migration failed on pass 1");
    run_sprint2_migration(pool)
        .await
        .expect("Sprint 2 migration failed on pass 1");
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Sprint 2 Stage 4 hardening migration failed on pass 1");

    let version_pass1: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version_pass1, "0.3.1-sprint2");

    // Pass 2: Simulate application relaunch where the same startup chain executes again
    run_sprint1_migration(pool)
        .await
        .expect("Sprint 1 migration must be idempotent on app relaunch");
    run_stage3_migration(pool)
        .await
        .expect("Stage 3 migration must be idempotent on app relaunch");
    run_sprint2_migration(pool)
        .await
        .expect("Sprint 2 migration must be idempotent on app relaunch");
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Stage 4 hardening migration must be idempotent on app relaunch");

    let version_pass2: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version_pass2, "0.3.1-sprint2");
}

#[tokio::test]
async fn test_mig_031_12_fail_closed_rejection_of_committed_staging_value() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_mig_staging";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_sales_staging").await;

    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_staging', $1, 'vt_sales', 'ser_sales_staging', 'automatic', NULL, NULL, 'DRAFT-STAGING', '2026-11-05', '2026-11-05', 'draft', '200.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    // Inject valid line 1
    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('line_ok_1', $1, 'vch_staging', 1, 'led_cash', 'Dr', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    // Inject committed staging line in staging range [1000001, 1010000]
    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('line_staging_committed', $1, 'vch_staging', 1000005, 'led_pnl', 'Cr', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    let err = run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect_err("Pre-migration validation must reject committed staging-range value");

    assert!(
        matches!(err, OpenTallyError::IntegrityViolation(_)),
        "Expected IntegrityViolation, got {:?}",
        err
    );

    // Verify rollback: version remains 0.3.0-sprint2
    let version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(version, "0.3.0-sprint2");

    // Verify original data is preserved untouched (zero truncation, zero silent repair)
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = 'vch_staging'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(count, 2);

    let staging_val: i64 = sqlx::query_scalar(
        "SELECT line_number FROM voucher_lines WHERE id = 'line_staging_committed'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(staging_val, 1000005);
}

#[tokio::test]
async fn test_mig_031_13_external_table_trigger_referencing_voucher_lines_dropped_and_restored() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_ext_trg";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_ext_trg").await;

    // Verify trg_prevent_ledger_delete_with_vouchers is on table ledgers and references voucher_lines
    let trg_info: (String, String) = sqlx::query_as(
        "SELECT tbl_name, sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_prevent_ledger_delete_with_vouchers'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(trg_info.0, "ledgers");
    assert!(trg_info.1.contains("voucher_lines"));
    let canonical_sql = trg_info.1;

    // Create a voucher and line referencing led_cash
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_ext', $1, 'vt_sales', 'ser_ext_trg', 'automatic', NULL, NULL, 'DRAFT-EXT', '2026-11-05', '2026-11-05', 'draft', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES ('line_ext', $1, 'vch_ext', 1, 'led_cash', 'Dr', '100.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    // Run forward migration 0.3.1-sprint2
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration must succeed on valid database");

    // Verify the trigger was restored and its SQL definition matches canonical definition
    let restored_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_prevent_ledger_delete_with_vouchers'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        restored_sql, canonical_sql,
        "Restored SQL definition for trg_prevent_ledger_delete_with_vouchers must match canonical definition"
    );

    // Verify trigger behavior after migration: deleting led_cash must be aborted
    let del_err = sqlx::query("DELETE FROM ledgers WHERE company_id = $1 AND id = 'led_cash'")
        .bind(company_id)
        .execute(pool)
        .await
        .expect_err("Trigger must abort deleting ledger referenced by voucher line");
    assert!(
        del_err
            .to_string()
            .contains("Cannot delete ledger: it is referenced in existing voucher lines"),
        "Unexpected error: {:?}",
        del_err
    );
}

#[tokio::test]
async fn test_mig_031_14_all_14_canonical_triggers_sql_restored_identically() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();

    // Collect pre-migration canonical SQL definitions for all 14 triggers
    let mut before_definitions = std::collections::HashMap::new();
    for trg_name in CANONICAL_STAGE4_DEPENDENT_TRIGGERS {
        let sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = $1",
        )
        .bind(trg_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "Trigger '{}' must exist in unhardened 0.3.0 baseline: {:?}",
                trg_name, e
            )
        });
        before_definitions.insert(trg_name, sql);
    }

    assert_eq!(before_definitions.len(), 14);

    // Execute forward migration 0.3.1-sprint2
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration failed");

    // Verify all 14 triggers are restored and their SQL definitions match canonical definitions
    for trg_name in CANONICAL_STAGE4_DEPENDENT_TRIGGERS {
        let after_sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = $1",
        )
        .bind(trg_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "Trigger '{}' was not restored after migration: {:?}",
                trg_name, e
            )
        });

        assert_eq!(
            after_sql, before_definitions[trg_name],
            "Restored SQL definition for trigger '{}' must match canonical definition byte-for-byte",
            trg_name
        );
    }
}

#[tokio::test]
async fn test_mig_031_15_successful_migration_confirms_zero_staging_values_post_migration() {
    let test_db = TestDb::new_migrated_030().await;
    let pool = test_db.db.pool();
    let company_id = "comp_zero_staging";
    insert_test_company(pool, company_id).await;
    insert_test_series(pool, company_id, "vt_sales", "ser_zero_staging").await;

    // Seed multiple valid vouchers and lines in unhardened 0.3.0
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_zs_1', $1, 'vt_sales', 'ser_zero_staging', 'automatic', NULL, NULL, 'DRAFT-ZS-1', '2026-11-05', '2026-11-05', 'draft', '300.00')"
    )
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    for line_no in [1, 2, 3, 50, 100, 10000] {
        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
             VALUES ($1, $2, 'vch_zs_1', $3, 'led_cash', 'Dr', '50.00')"
        )
        .bind(format!("line_zs_{}", line_no))
        .bind(company_id)
        .bind(line_no)
        .execute(pool)
        .await
        .unwrap();
    }

    // Run forward migration 0.3.1-sprint2
    run_sprint2_stage4_hardening_migration(pool)
        .await
        .expect("Migration failed");

    // Invariant Check 1: Zero values in staging range [1000001, 1010000]
    let staging_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE line_number BETWEEN 1000001 AND 1010000",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        staging_count, 0,
        "No staging-range line numbers may exist post-migration"
    );

    // Invariant Check 2: Zero values outside [1, 10000]
    let invalid_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE COALESCE(line_number BETWEEN 1 AND 10000, 0) <> 1"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        invalid_count, 0,
        "No out-of-bounds line numbers may exist post-migration"
    );

    // Invariant Check 3: Verified row count preserved
    let total_lines: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = 'vch_zs_1'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(total_lines, 6);
}
