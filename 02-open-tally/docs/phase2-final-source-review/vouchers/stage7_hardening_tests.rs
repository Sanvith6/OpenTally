//! Stage 7A Phase 2 Targeted Hardening Tests
//!
//! Resolves independent verification findings:
//! - FIND-7A-P2-01: Async cancellation, unwind panics, and pool contamination hygiene
//! - FIND-7A-P2-02: Failure-path pool disposal, lineage depth/cycle limits, and monetary SQL boundaries
//! - FIND-7A-P2-04: Complete deterministic sort-order and tie-breaker coverage
//! - FIND-7A-P2-05: Repository-level pagination safety & checked total-page bounds
//! - FIND-7A-P2-06: Cross-company audit/lineage isolation & concurrent WAL snapshot consistency

use crate::accounting::{CreateLedgerRequest, GroupService, LedgerService};
use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    CancelVoucherRequest, CreateVoucherDraftRequest, CreateVoucherSeriesRequest, DateBasis,
    ReverseVoucherRequest, SortDirection, VoucherLineInput, VoucherSearchFilter, VoucherSortField,
};
use crate::vouchers::service::search::{
    compute_total_pages, execute_read_only_query, with_read_only_lifecycle_hook,
    ReadOnlyLifecyclePoint, SqliteVoucherSearchRepository, TestLifecycleHook,
    VoucherSearchRepository, VoucherSearchService,
};
use crate::vouchers::service::{
    VoucherCancellationService, VoucherDraftService, VoucherPostingService, VoucherReversalService,
    VoucherSeriesService,
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Notify;

struct CompanyFixture {
    company_id: String,
    dr_ledger_id: String,
    cr_ledger_id: String,
    jnl_auto_series_id: String,
    jnl_manual_series_id: String,
}

async fn setup_file_db() -> (TempDir, DatabaseManager, PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir for WAL database");
    let db_path = dir.path().join("stage7_hardening.db");
    let db = DatabaseManager::init(&db_path)
        .await
        .expect("init file-backed WAL database");
    crate::database::migrations::run_sprint1_migration(db.pool())
        .await
        .expect("run_sprint1_migration");
    crate::database::migrations::run_stage3_migration(db.pool())
        .await
        .expect("run_stage3_migration");
    crate::database::migrations::run_sprint2_migration(db.pool())
        .await
        .expect("run_sprint2_migration");
    crate::database::migrations::run_sprint2_stage4_hardening_migration(db.pool())
        .await
        .expect("run_sprint2_stage4_hardening_migration");
    (dir, db, db_path)
}

async fn setup_instrumented_single_conn_pool(db_path: &Path) -> (Pool<Sqlite>, Arc<AtomicUsize>) {
    let connect_count = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&connect_count);

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_millis(5000));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .min_connections(0)
        .after_connect(move |_conn, _meta| {
            let c = Arc::clone(&counter_clone);
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        })
        .connect_with(options)
        .await
        .expect("open instrumented 1-connection pool");

    (pool, connect_count)
}

async fn setup_company_fixture(pool: &Pool<Sqlite>, suffix: &str) -> CompanyFixture {
    let company = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: format!("Hardening Co {}", suffix),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(4),
        },
    )
    .await
    .expect("create company");

    let groups = GroupService::list_groups(pool, &company.id, false)
        .await
        .expect("list groups");
    let debtor_group = groups
        .iter()
        .find(|g| g.name == "Sundry Debtors")
        .unwrap_or(&groups[0]);
    let sales_group = groups
        .iter()
        .find(|g| g.name == "Sales Accounts")
        .unwrap_or(&groups[0]);

    let dr_ledger = LedgerService::create_ledger(
        pool,
        &company.id,
        CreateLedgerRequest {
            name: format!("Debtor {}", suffix),
            group_id: debtor_group.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .expect("create debtor ledger");

    let cr_ledger = LedgerService::create_ledger(
        pool,
        &company.id,
        CreateLedgerRequest {
            name: format!("Sales {}", suffix),
            group_id: sales_group.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .expect("create sales ledger");

    sqlx::query(
        "INSERT OR IGNORE INTO voucher_types (
            id, company_id, name, category, parent_type,
            affects_ledger_balance, affects_inventory, is_predefined, is_active, numbering_method
        ) VALUES
            ('vt_jnl', $1, 'Journal', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
            ('vt_reversing_journal', $1, 'Reversing Journal', 'scenario', NULL, 0, 0, 1, 1, 'automatic')",
    )
    .bind(&company.id)
    .execute(pool)
    .await
    .expect("ensure voucher types exist");

    let jnl_auto = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_jnl".to_string(),
            series_name: format!("Journal Hardening Auto {}", suffix),
            series_code: "HAUT".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("HAUT/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("create automatic jnl series");

    let jnl_manual = VoucherSeriesService::create_series(
        pool,
        &company.id,
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_jnl".to_string(),
            series_name: format!("Journal Hardening Manual {}", suffix),
            series_code: "HMAN".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "never".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("HMAN-".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("create manual jnl series");

    let existing_rev_series: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_number_series WHERE company_id = $1 AND voucher_type_id = 'vt_reversing_journal'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .expect("count rev series");

    if existing_rev_series == 0 {
        VoucherSeriesService::create_series(
            pool,
            &company.id,
            CreateVoucherSeriesRequest {
                voucher_type_id: "vt_reversing_journal".to_string(),
                series_name: format!("Reversing Default {}", suffix),
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
        .expect("create reversing series");
    }

    CompanyFixture {
        company_id: company.id,
        dr_ledger_id: dr_ledger.id,
        cr_ledger_id: cr_ledger.id,
        jnl_auto_series_id: jnl_auto.id,
        jnl_manual_series_id: jnl_manual.id,
    }
}

async fn post_balanced_voucher(
    pool: &Pool<Sqlite>,
    fix: &CompanyFixture,
    date: &str,
    amount: &str,
    narration: Option<&str>,
) -> String {
    let draft = VoucherDraftService::create_draft(
        pool,
        &fix.company_id,
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_jnl".to_string(),
            series_id: fix.jnl_auto_series_id.clone(),
            date: date.to_string(),
            effective_date: Some(date.to_string()),
            reference_no: None,
            reference_date: None,
            narration: narration.map(str::to_string),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: fix.dr_ledger_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: amount.to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: fix.cr_ledger_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: amount.to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .expect("create balanced draft");

    let posted = VoucherPostingService::post_voucher(
        pool,
        &fix.company_id,
        &draft.voucher.id,
        Some("tester"),
    )
    .await
    .expect("post voucher");
    posted.voucher.id
}

async fn assert_pool_writable_and_clean(pool: &Pool<Sqlite>, marker: i64) {
    let mut conn = pool.acquire().await.expect("re-acquire pooled connection");
    let q_only: i64 = sqlx::query_scalar("PRAGMA query_only;")
        .fetch_one(&mut *conn)
        .await
        .expect("read PRAGMA query_only");
    assert_eq!(
        q_only, 0,
        "Pool returned contaminated connection with PRAGMA query_only = 1"
    );

    sqlx::query("CREATE TABLE IF NOT EXISTS _pool_hygiene_probe (marker INTEGER NOT NULL);")
        .execute(&mut *conn)
        .await
        .expect("write DDL must succeed on clean pooled connection");
    sqlx::query("INSERT INTO _pool_hygiene_probe (marker) VALUES ($1);")
        .bind(marker)
        .execute(&mut *conn)
        .await
        .expect("write DML must succeed on clean pooled connection");
}

// =============================================================================
// FIND-7A-P2-01: Async Cancellation, Panic Unwinding & Pool Hygiene
// =============================================================================

#[tokio::test]
async fn test_h01_cancel_inside_closure_discards_contaminated_connection() {
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;

    // Two clean queries must reuse the same single connection without reopening.
    for _ in 0..2 {
        let v = execute_read_only_query(&pool, |conn| {
            Box::pin(async move {
                let q: i64 = sqlx::query_scalar("PRAGMA query_only;")
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                assert_eq!(q, 1);
                Ok(q)
            })
        })
        .await
        .expect("clean read-only query");
        assert_eq!(v, 1);
    }
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        1,
        "Clean read-only queries must reuse the pooled connection"
    );

    let entered = Arc::new(Notify::new());
    let entered_tx = Arc::clone(&entered);
    let pool_clone = pool.clone();

    let handle = tokio::spawn(async move {
        execute_read_only_query(&pool_clone, |conn| {
            Box::pin(async move {
                let q: i64 = sqlx::query_scalar("PRAGMA query_only;")
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                assert_eq!(q, 1);
                entered_tx.notify_one();
                std::future::pending::<()>().await;
                Ok(0_i64)
            })
        })
        .await
    });

    entered.notified().await;
    handle.abort();
    let join_err = handle.await.expect_err("task should be cancelled");
    assert!(join_err.is_cancelled());

    // Contaminated connection #1 must have been discarded; next acquire opens #2 cleanly.
    assert_pool_writable_and_clean(&pool, 101).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Cancelled query inside closure must discard connection #1 and force fresh connection #2"
    );
}

#[tokio::test]
async fn test_h02_cancel_after_commit_before_pragma_reset_discards_connection() {
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;

    // Warm connection #1 cleanly first.
    let ok = execute_read_only_query(&pool, |_conn| Box::pin(async { Ok(1_i64) }))
        .await
        .expect("warmup");
    assert_eq!(ok, 1);
    assert_eq!(connect_count.load(Ordering::SeqCst), 1);

    let entered = Arc::new(Notify::new());
    let entered_hook = Arc::clone(&entered);
    let hook: TestLifecycleHook = Arc::new(move |point| {
        let entered_hook = Arc::clone(&entered_hook);
        Box::pin(async move {
            if point == ReadOnlyLifecyclePoint::AfterCommitBeforeReset {
                entered_hook.notify_one();
                std::future::pending::<()>().await;
            }
        })
    });

    let pool_clone = pool.clone();
    let handle = tokio::spawn(with_read_only_lifecycle_hook(hook, async move {
        execute_read_only_query(&pool_clone, |_conn| Box::pin(async { Ok(7_i64) })).await
    }));

    entered.notified().await;
    handle.abort();
    let join_err = handle
        .await
        .expect_err("task cancelled right before PRAGMA OFF");
    assert!(join_err.is_cancelled());

    // Even after tx.commit() succeeded, query_only was still ON -> must discard connection #1!
    assert_pool_writable_and_clean(&pool, 102).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Cancellation between COMMIT and PRAGMA query_only = OFF must discard connection #1"
    );
}

#[tokio::test]
async fn test_h03_cancel_after_pragma_on_before_begin_discards_connection() {
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;
    execute_read_only_query(&pool, |_conn| Box::pin(async { Ok(()) }))
        .await
        .expect("warmup");
    assert_eq!(connect_count.load(Ordering::SeqCst), 1);

    let entered = Arc::new(Notify::new());
    let entered_hook = Arc::clone(&entered);
    let hook: TestLifecycleHook = Arc::new(move |point| {
        let entered_hook = Arc::clone(&entered_hook);
        Box::pin(async move {
            if point == ReadOnlyLifecyclePoint::AfterPragmaOnBeforeBegin {
                entered_hook.notify_one();
                std::future::pending::<()>().await;
            }
        })
    });

    let pool_clone = pool.clone();
    let handle = tokio::spawn(with_read_only_lifecycle_hook(hook, async move {
        execute_read_only_query(&pool_clone, |_conn| Box::pin(async { Ok(()) })).await
    }));

    entered.notified().await;
    handle.abort();
    assert!(handle.await.expect_err("task aborted").is_cancelled());

    assert_pool_writable_and_clean(&pool, 103).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Cancellation between PRAGMA ON and BEGIN must discard connection #1"
    );
}

#[tokio::test]
async fn test_h04_panic_inside_closure_discards_contaminated_connection() {
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;
    execute_read_only_query(&pool, |_conn| Box::pin(async { Ok(()) }))
        .await
        .expect("warmup");
    assert_eq!(connect_count.load(Ordering::SeqCst), 1);

    let pool_clone = pool.clone();
    let handle = tokio::spawn(async move {
        execute_read_only_query(&pool_clone, |_conn| {
            Box::pin(async {
                panic!("deliberate unwind panic inside read-only closure");
            })
        })
        .await as Result<(), OpenTallyError>
    });

    let join_err = handle.await.expect_err("closure panic must abort task");
    assert!(join_err.is_panic());

    assert_pool_writable_and_clean(&pool, 104).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Unwind panic inside read-only closure must discard connection #1"
    );
}

#[tokio::test]
async fn test_h05_database_manager_after_release_rejects_dirtied_connection() {
    let (_dir, db, _db_path) = setup_file_db().await;

    // Directly contaminate a pooled connection via PRAGMA query_only = ON and drop it back.
    {
        let mut conn = db.pool().acquire().await.expect("acquire");
        sqlx::query("PRAGMA query_only = ON;")
            .execute(&mut *conn)
            .await
            .expect("dirty connection");
        // Drop without resetting query_only to 0.
    }

    // DatabaseManager::init configures after_release to reject any query_only != 0 connection.
    assert_pool_writable_and_clean(db.pool(), 105).await;
}

// =============================================================================
// FIND-7A-P2-02: Failure Paths, Lineage Depths/Cycles & Monetary SQL Boundaries
// =============================================================================

#[tokio::test]
async fn test_h06_closure_error_and_readonly_dml_failure_rollback_and_discard() {
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;

    // 1. DML write inside read-only session must fail with SQLITE_READONLY and discard conn.
    let write_res: Result<(), OpenTallyError> = execute_read_only_query(&pool, |conn| {
        Box::pin(async move {
            sqlx::query("INSERT INTO companies (id, name, financial_year_begins) VALUES ('bad', 'Bad Co', '2026-04-01');")
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(())
        })
    })
    .await;
    let write_err = write_res.expect_err("DML inside read-only session must fail");
    assert!(
        matches!(write_err, OpenTallyError::Database(_)),
        "Expected Database error, got {:?}",
        write_err
    );
    assert!(
        write_err.to_string().to_lowercase().contains("readonly"),
        "Expected SQLITE_READONLY error, got: {}",
        write_err
    );
    assert_pool_writable_and_clean(&pool, 201).await;
    assert_eq!(connect_count.load(Ordering::SeqCst), 2);

    // 2. Domain Err returned from closure must also discard connection.
    let domain_res: Result<(), OpenTallyError> = execute_read_only_query(&pool, |_conn| {
        Box::pin(async {
            Err(OpenTallyError::Validation(
                "explicit domain failure".to_string(),
            ))
        })
    })
    .await;
    assert!(matches!(domain_res, Err(OpenTallyError::Validation(_))));
    assert_pool_writable_and_clean(&pool, 202).await;
    assert_eq!(connect_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_h07_real_sqlite_rollback_failure_still_discards_connection() {
    // Note: SQLite does not support fault-injecting ROLLBACK via PRAGMA without closing
    // the worker. Closing the transaction manually inside the closure forces SQLx/SQLite
    // `tx.rollback().await` to fail with "cannot rollback - no transaction is active" while
    // PRAGMA query_only = ON remains active, verifying unconditional disposal on rollback error.
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;

    let res: Result<(), OpenTallyError> = execute_read_only_query(&pool, |conn| {
        Box::pin(async move {
            sqlx::query("ROLLBACK;")
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Err(OpenTallyError::Validation(
                "outer query error after tx already ended".to_string(),
            ))
        })
    })
    .await;

    assert!(matches!(res, Err(OpenTallyError::Validation(_))));
    assert_pool_writable_and_clean(&pool, 203).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Failed rollback must still discard connection #1"
    );
}

#[tokio::test]
async fn test_h08_real_sqlite_commit_failure_discards_connection() {
    // Closing the transaction manually via COMMIT inside closure forces SQLx `tx.commit().await`
    // to fail genuinely inside SQLite while query_only = 1 is still set, proving disposal.
    let (_dir, db, db_path) = setup_file_db().await;
    db.pool().close().await;

    let (pool, connect_count) = setup_instrumented_single_conn_pool(&db_path).await;

    let res: Result<i64, OpenTallyError> = execute_read_only_query(&pool, |conn| {
        Box::pin(async move {
            sqlx::query("COMMIT;")
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(42)
        })
    })
    .await;

    assert!(matches!(res, Err(OpenTallyError::Database(_))));
    assert_pool_writable_and_clean(&pool, 204).await;
    assert_eq!(
        connect_count.load(Ordering::SeqCst),
        2,
        "Commit failure must discard connection #1"
    );
}

async fn build_reversal_chain(
    pool: &Pool<Sqlite>,
    fix: &CompanyFixture,
    hops: usize,
) -> Vec<String> {
    let root_id =
        post_balanced_voucher(pool, fix, "2026-04-10", "500.00", Some("Chain Root")).await;
    let mut ids = vec![root_id];

    for i in 1..=hops {
        let prev_id = ids.last().expect("prev id").clone();
        let rev = VoucherReversalService::reverse_voucher(
            pool,
            &fix.company_id,
            ReverseVoucherRequest {
                original_voucher_id: prev_id,
                reversal_date: "2026-04-11".to_string(),
                reason: format!("Hop {}", i),
                actor_id: Some("tester".to_string()),
            },
        )
        .await
        .expect("chain reversal hop");
        ids.push(rev.reversing_voucher.voucher.id);
    }

    ids
}

#[tokio::test]
async fn test_h09_lineage_depth_under_ten_exact_boundaries() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "lin_lt10").await;

    // 4 hops => 5 nodes (depths 0..=4)
    let ids = build_reversal_chain(db.pool(), &fix, 4).await;
    assert_eq!(ids.len(), 5);

    let from_root = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[0])
        .await
        .expect("lineage from root");
    assert_eq!(from_root.items.len(), 5);
    assert!(!from_root.has_exceeded_depth_limit);
    assert_eq!(from_root.max_depth_reached, 4);

    let from_leaf = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[4])
        .await
        .expect("lineage from leaf");
    assert_eq!(from_leaf.items.len(), 5);
    assert!(!from_leaf.has_exceeded_depth_limit);
    assert_eq!(from_leaf.max_depth_reached, 4);
}

#[tokio::test]
async fn test_h10_lineage_depth_exactly_ten_not_truncated() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "lin_eq10").await;

    // 10 hops => 11 nodes (depths 0..=10). Must NOT mark exceeded!
    let ids = build_reversal_chain(db.pool(), &fix, 10).await;
    assert_eq!(ids.len(), 11);

    let lin = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[0])
        .await
        .expect("lineage depth 10");
    assert_eq!(lin.items.len(), 11);
    assert_eq!(lin.max_depth_reached, 10);
    assert!(
        !lin.has_exceeded_depth_limit,
        "Depth exactly 10 must not trigger has_exceeded_depth_limit"
    );
}

#[tokio::test]
async fn test_h11_lineage_depth_exceeds_ten_truncates_forward_and_backward() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "lin_gt10").await;

    // 12 hops => 13 nodes (v0 .. v12).
    let ids = build_reversal_chain(db.pool(), &fix, 12).await;
    assert_eq!(ids.len(), 13);

    // Forward from v0: reaches depth 10 (11 nodes v0..v10), detects depth 11 -> truncated!
    let from_root = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[0])
        .await
        .expect("forward lineage from v0");
    assert_eq!(from_root.items.len(), 11);
    assert_eq!(from_root.max_depth_reached, 10);
    assert!(from_root.has_exceeded_depth_limit);
    assert!(from_root.items.iter().any(|i| i.id == ids[10]));
    assert!(!from_root.items.iter().any(|i| i.id == ids[11]));
    assert!(!from_root.items.iter().any(|i| i.id == ids[12]));

    // Backward from v12: reaches depth 10 (11 nodes v2..v12), detects depth 11 -> truncated!
    let from_leaf = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[12])
        .await
        .expect("backward lineage from v12");
    assert_eq!(from_leaf.items.len(), 11);
    assert_eq!(from_leaf.max_depth_reached, 10);
    assert!(from_leaf.has_exceeded_depth_limit);
    assert!(from_leaf.items.iter().any(|i| i.id == ids[2]));
    assert!(!from_leaf.items.iter().any(|i| i.id == ids[1]));
    assert!(!from_leaf.items.iter().any(|i| i.id == ids[0]));

    // Middle node v6: max distance is 6 in each direction -> all 13 nodes returned, not exceeded!
    let from_mid = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, &ids[6])
        .await
        .expect("lineage from midpoint v6");
    assert_eq!(from_mid.items.len(), 13);
    assert_eq!(from_mid.max_depth_reached, 6);
    assert!(!from_mid.has_exceeded_depth_limit);
}

#[tokio::test]
async fn test_h12_lineage_cyclic_graph_and_prefix_id_collision() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "lin_cycle").await;

    // Insert three valid 'manual' 'draft' rows with prefix-colliding IDs ("v1", "v10", "v100")
    // and wire a genuine cycle v1 -> v10 -> v1 (plus v100 -> v10) while in 'draft' status.
    for id in ["v1", "v10", "v100"] {
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, total_amount, reversal_of_voucher_id
            ) VALUES ($1, $2, 'vt_jnl', $3, 'manual', NULL, NULL, 'DRAFT', '2026-04-10', '2026-04-10', 'draft', '10.00', NULL)",
        )
        .bind(id)
        .bind(&fix.company_id)
        .bind(&fix.jnl_manual_series_id)
        .execute(db.pool())
        .await
        .expect("insert draft node");
    }

    sqlx::query(
        "UPDATE vouchers SET reversal_of_voucher_id = 'v1' WHERE company_id = $1 AND id = 'v10'",
    )
    .bind(&fix.company_id)
    .execute(db.pool())
    .await
    .expect("link v10 -> v1");
    sqlx::query(
        "UPDATE vouchers SET reversal_of_voucher_id = 'v10' WHERE company_id = $1 AND id = 'v1'",
    )
    .bind(&fix.company_id)
    .execute(db.pool())
    .await
    .expect("close cycle v1 -> v10");
    sqlx::query(
        "UPDATE vouchers SET reversal_of_voucher_id = 'v100' WHERE company_id = $1 AND id = 'v100'",
    )
    .bind(&fix.company_id)
    .execute(db.pool())
    .await
    .ok(); // ignored if self or let's link a non-conflicting edge v100 -> v10 via another parent?
           // Replace v100 edge: link v100 -> v10? uq_vouchers_reversal is on reversal_of_voucher_id.
           // Since 'v10' is already reversed by 'v1', point 'v1' -> 'v100' and 'v100' -> 'v10' (true 3-cycle!).
    sqlx::query(
        "UPDATE vouchers SET reversal_of_voucher_id = 'v100' WHERE company_id = $1 AND id = 'v1'",
    )
    .bind(&fix.company_id)
    .execute(db.pool())
    .await
    .expect("link v1 -> v100");
    sqlx::query(
        "UPDATE vouchers SET reversal_of_voucher_id = 'v10' WHERE company_id = $1 AND id = 'v100'",
    )
    .bind(&fix.company_id)
    .execute(db.pool())
    .await
    .expect("close 3-cycle v100 -> v10 -> v1 -> v100");

    let lin = VoucherSearchService::get_voucher_lineage(db.pool(), &fix.company_id, "v1")
        .await
        .expect("cyclic lineage must terminate without infinite recursion");
    assert_eq!(lin.items.len(), 3);
    assert!(!lin.has_exceeded_depth_limit);
    let returned_ids: Vec<&str> = lin.items.iter().map(|v| v.id.as_str()).collect();
    assert!(returned_ids.contains(&"v1"));
    assert!(returned_ids.contains(&"v10"));
    assert!(returned_ids.contains(&"v100"));
}

#[tokio::test]
async fn test_h13_monetary_sql_boundaries_and_invalid_query_rejections() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "money").await;

    // Seed schema-valid rows spanning 0.0001 up to 25-char column maxes:
    // chk_voucher_total_amount_canonical enforces length(total_amount) <= 25.
    let cases = [
        ("m_0001", "M-001", "0.0001"),
        ("m_one", "M-002", "1"),
        ("m_20int", "M-003", "99999999999999999999"),
        ("m_max25_dec", "M-004", "99999999999999999999.9999"), // 25 chars (max 4-dec stored)
        ("m_max25_int", "M-005", "9999999999999999999999999"), // 25 chars (max integer stored)
    ];

    for (idx, (id, vnum, amt)) in cases.iter().enumerate() {
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, total_amount
            ) VALUES ($1, $2, 'vt_jnl', $3, 'manual', NULL, NULL, $4, '2026-04-10', '2026-04-10', 'posted', $5)",
        )
        .bind(id)
        .bind(&fix.company_id)
        .bind(&fix.jnl_manual_series_id)
        .bind(vnum)
        .bind(amt)
        .execute(db.pool())
        .await
        .unwrap_or_else(|e| panic!("insert boundary row {}: {}", idx, e));
    }

    // 1. Ascending TotalAmount sort proves canonical 30-char SQL ordering across all magnitudes
    let all_asc = VoucherSearchService::search_vouchers(
        db.pool(),
        &fix.company_id,
        VoucherSearchFilter {
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .expect("total_amount asc");
    let ordered_ids: Vec<&str> = all_asc.items.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(
        ordered_ids,
        vec!["m_0001", "m_one", "m_20int", "m_max25_dec", "m_max25_int"]
    );

    // 2. Exact match for "1.0000" matches stored unpadded "1"
    let eq_one = VoucherSearchService::search_vouchers(
        db.pool(),
        &fix.company_id,
        VoucherSearchFilter {
            min_amount: Some("1.0000".to_string()),
            max_amount: Some("1.0000".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("exact 1.0000 range");
    assert_eq!(eq_one.items.len(), 1);
    assert_eq!(eq_one.items[0].id, "m_one");

    // 3. Query upper bound at full 25+4 (30-char canonical) limit "9999999999999999999999999.9999"
    let max_range = VoucherSearchService::search_vouchers(
        db.pool(),
        &fix.company_id,
        VoucherSearchFilter {
            min_amount: Some("99999999999999999999.9999".to_string()),
            max_amount: Some("9999999999999999999999999.9999".to_string()),
            sort_by: Some(VoucherSortField::TotalAmount),
            sort_direction: Some(SortDirection::Asc),
            ..Default::default()
        },
    )
    .await
    .expect("max 30-char query range");
    let max_ids: Vec<&str> = max_range.items.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(max_ids, vec!["m_max25_dec", "m_max25_int"]);

    // 4. Invalid monetary query inputs rejected cleanly
    for bad in ["-0.0001", "1.00001", "10000000000000000000000000", "abc"] {
        let err = VoucherSearchService::search_vouchers(
            db.pool(),
            &fix.company_id,
            VoucherSearchFilter {
                min_amount: Some(bad.to_string()),
                ..Default::default()
            },
        )
        .await
        .expect_err("invalid amount must fail validation");
        assert!(matches!(err, OpenTallyError::Validation(_)));
    }
}

// =============================================================================
// FIND-7A-P2-04: Complete Sort Coverage & Tie-Breakers
// =============================================================================

#[tokio::test]
async fn test_h14_sort_all_fields_asc_desc_tiebreakers_nulls_and_stability() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "sort").await;

    // Seed controlled rows exercising:
    // - Null total_amount (draft s_null) vs 2.50 vs tied 10.00 across dates & IDs
    // - Same date + differing automatic sequence_number (1 vs 2)
    // - Same date + NULL manual sequence_number + differing IDs (v.id ASC tie-breaker)
    // - Distinct created_at timestamps + tied created_at pair (v.id ASC tie-breaker)
    let rows = [
        (
            "s_auto_1",
            &fix.jnl_auto_series_id,
            "automatic",
            Some("2026-04-01"),
            Some(1_i64),
            "JNL-0001",
            "2026-04-10",
            Some("10.00"),
            "posted",
            "2026-04-10T09:00:00Z",
        ),
        (
            "s_auto_2",
            &fix.jnl_auto_series_id,
            "automatic",
            Some("2026-04-01"),
            Some(2_i64),
            "JNL-0002",
            "2026-04-10",
            Some("2.50"),
            "posted",
            "2026-04-10T10:00:00Z",
        ),
        (
            "s_man_b",
            &fix.jnl_manual_series_id,
            "manual",
            None,
            None,
            "MAN-0002",
            "2026-04-05",
            Some("10.00"),
            "posted",
            "2026-04-10T08:00:00Z",
        ),
        (
            "s_man_a",
            &fix.jnl_manual_series_id,
            "manual",
            None,
            None,
            "MAN-0001",
            "2026-04-05",
            Some("100.00"),
            "posted",
            "2026-04-10T08:00:00Z",
        ),
        (
            "s_null",
            &fix.jnl_manual_series_id,
            "manual",
            None,
            None,
            "DRAFT",
            "2026-04-12",
            None,
            "draft",
            "2026-04-10T11:00:00Z",
        ),
        (
            "s_draft_tie",
            &fix.jnl_manual_series_id,
            "manual",
            None,
            None,
            "DRAFT",
            "2026-04-02",
            Some("5.00"),
            "draft",
            "2026-04-10T07:00:00Z",
        ),
    ];

    for (id, sid, method, pstart, seq, vnum, date, amt, status, created_at) in rows {
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, total_amount, created_at, updated_at
            ) VALUES ($1, $2, 'vt_jnl', $3, $4, $5, $6, $7, $8, $8, $9, $10, $11, $11)",
        )
        .bind(id)
        .bind(&fix.company_id)
        .bind(sid)
        .bind(method)
        .bind(pstart)
        .bind(seq)
        .bind(vnum)
        .bind(date)
        .bind(status)
        .bind(amt)
        .bind(created_at)
        .execute(db.pool())
        .await
        .expect("insert sort row");
    }

    let query_ids = |sort_by: VoucherSortField, sort_dir: SortDirection| {
        let pool = db.pool().clone();
        let cid = fix.company_id.clone();
        async move {
            let res = VoucherSearchService::search_vouchers(
                &pool,
                &cid,
                VoucherSearchFilter {
                    status: Some("all".to_string()),
                    date_basis: Some(DateBasis::TransactionDate),
                    sort_by: Some(sort_by),
                    sort_direction: Some(sort_dir),
                    ..Default::default()
                },
            )
            .await
            .expect("sorted search");
            res.items.into_iter().map(|i| i.id).collect::<Vec<String>>()
        }
    };

    // Date Desc: date DESC, sequence_number DESC, id ASC
    // 2026-04-12: s_null
    // 2026-04-10: s_auto_2 (seq 2), s_auto_1 (seq 1)
    // 2026-04-05 (NULL seq tied): s_man_a < s_man_b (id ASC!)
    // 2026-04-02: s_draft_tie
    assert_eq!(
        query_ids(VoucherSortField::Date, SortDirection::Desc).await,
        vec![
            "s_null",
            "s_auto_2",
            "s_auto_1",
            "s_man_a",
            "s_man_b",
            "s_draft_tie"
        ]
    );
    // Date Asc: date ASC, sequence_number ASC, id ASC (NULL < non-null; tied NULLs by id ASC)
    assert_eq!(
        query_ids(VoucherSortField::Date, SortDirection::Asc).await,
        vec![
            "s_draft_tie",
            "s_man_a",
            "s_man_b",
            "s_auto_1",
            "s_auto_2",
            "s_null"
        ]
    );

    // VoucherNumber Asc & Desc (including date DESC tie-break on identical "DRAFT" number)
    assert_eq!(
        query_ids(VoucherSortField::VoucherNumber, SortDirection::Asc).await,
        vec![
            "s_null",
            "s_draft_tie",
            "s_auto_1",
            "s_auto_2",
            "s_man_a",
            "s_man_b"
        ]
    );
    assert_eq!(
        query_ids(VoucherSortField::VoucherNumber, SortDirection::Desc).await,
        vec![
            "s_man_b",
            "s_man_a",
            "s_auto_2",
            "s_auto_1",
            "s_null",
            "s_draft_tie"
        ]
    );

    // TotalAmount Asc: NULL (0) < 2.50 < 5.00 < 10.00 (date DESC tie: s_auto_1 04-10 before s_man_b 04-05) < 100.00
    let amt_asc = query_ids(VoucherSortField::TotalAmount, SortDirection::Asc).await;
    assert_eq!(
        amt_asc,
        vec![
            "s_null",
            "s_auto_2",
            "s_draft_tie",
            "s_auto_1",
            "s_man_b",
            "s_man_a"
        ]
    );
    assert_eq!(
        query_ids(VoucherSortField::TotalAmount, SortDirection::Desc).await,
        vec![
            "s_man_a",
            "s_auto_1",
            "s_man_b",
            "s_draft_tie",
            "s_auto_2",
            "s_null"
        ]
    );

    // CreatedAt Asc & Desc: tied 08:00:00Z must sort by v.id ASC ("s_man_a" before "s_man_b")
    assert_eq!(
        query_ids(VoucherSortField::CreatedAt, SortDirection::Asc).await,
        vec![
            "s_draft_tie",
            "s_man_a",
            "s_man_b",
            "s_auto_1",
            "s_auto_2",
            "s_null"
        ]
    );
    assert_eq!(
        query_ids(VoucherSortField::CreatedAt, SortDirection::Desc).await,
        vec![
            "s_null",
            "s_auto_2",
            "s_auto_1",
            "s_man_a",
            "s_man_b",
            "s_draft_tie"
        ]
    );

    // Determinism across repeated runs
    assert_eq!(
        query_ids(VoucherSortField::TotalAmount, SortDirection::Asc).await,
        amt_asc
    );
}

// =============================================================================
// FIND-7A-P2-05: Repository-Level Pagination Input Safety
// =============================================================================

#[tokio::test]
async fn test_h15_repository_pagination_guards_direct_calls_and_total_overflow() {
    let (_dir, db, _) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "paging").await;

    // Test direct repository calls bypassing VoucherSearchService, first with empty table (total == 0),
    // then with 1 posted voucher (total == 1).
    for populated in [false, true] {
        if populated {
            post_balanced_voucher(db.pool(), &fix, "2026-04-10", "50.00", Some("Row")).await;
        }

        for bad_filter in [
            VoucherSearchFilter {
                page: Some(0),
                ..Default::default()
            },
            VoucherSearchFilter {
                page_size: Some(0),
                ..Default::default()
            },
            VoucherSearchFilter {
                page_size: Some(101),
                ..Default::default()
            },
            VoucherSearchFilter {
                page: Some(u32::MAX),
                page_size: Some(2),
                ..Default::default()
            },
        ] {
            let cid = fix.company_id.clone();
            let res = execute_read_only_query(db.pool(), |conn| {
                Box::pin(async move {
                    let repo = SqliteVoucherSearchRepository;
                    repo.search_vouchers(conn, &cid, &bad_filter).await
                })
            })
            .await;
            assert!(
                matches!(res, Err(OpenTallyError::Validation(_))),
                "Direct repository call must reject invalid pagination even when populated={}",
                populated
            );
        }
    }

    // Checked total_pages computation without u32 truncation or panics
    assert_eq!(compute_total_pages(0, 20).expect("empty"), 1);
    assert_eq!(compute_total_pages(20, 20).expect("exact"), 1);
    assert_eq!(compute_total_pages(21, 20).expect("ceil"), 2);
    assert_eq!(
        compute_total_pages(i64::from(u32::MAX) * 2, 2).expect("max valid"),
        u32::MAX
    );

    assert!(matches!(
        compute_total_pages(i64::from(u32::MAX) + 1, 1),
        Err(OpenTallyError::Validation(_))
    ));
    assert!(matches!(
        compute_total_pages(-1, 20),
        Err(OpenTallyError::Internal(_))
    ));
    assert!(matches!(
        compute_total_pages(10, 0),
        Err(OpenTallyError::Validation(_))
    ));
}

// =============================================================================
// FIND-7A-P2-06: Cross-Company Audit/Lineage Isolation & WAL Snapshot Isolation
// =============================================================================

#[tokio::test]
async fn test_h16_cross_company_audit_log_collision_and_isolation() {
    let (_dir, db, _) = setup_file_db().await;
    let comp_a = setup_company_fixture(db.pool(), "audit_a").await;
    let comp_b = setup_company_fixture(db.pool(), "audit_b").await;

    // 1. Both companies share an identical voucher id ("shared_v1") that is cancelled,
    //    with Company B having a LATER cancellation audit entry.
    for (fix, vnum) in [(&comp_a, "CA-001"), (&comp_b, "CB-001")] {
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, total_amount
            ) VALUES ('shared_v1', $1, 'vt_jnl', $2, 'manual', NULL, NULL, $3, '2026-04-10', '2026-04-10', 'posted', '100.00')",
        )
        .bind(&fix.company_id)
        .bind(&fix.jnl_manual_series_id)
        .bind(vnum)
        .execute(db.pool())
        .await
        .expect("insert shared_v1");
    }

    VoucherCancellationService::cancel_voucher(
        db.pool(),
        &comp_a.company_id,
        CancelVoucherRequest {
            voucher_id: "shared_v1".to_string(),
            reason: "Company A Legitimate Cancellation".to_string(),
            actor_id: Some("user_a".to_string()),
        },
    )
    .await
    .expect("cancel comp_a shared_v1");

    VoucherCancellationService::cancel_voucher(
        db.pool(),
        &comp_b.company_id,
        CancelVoucherRequest {
            voucher_id: "shared_v1".to_string(),
            reason: "Company B Secret Reason".to_string(),
            actor_id: Some("user_b".to_string()),
        },
    )
    .await
    .expect("cancel comp_b shared_v1");

    // 2. Also add a synthetic cancelled voucher in Comp A with NO Comp A audit row while Comp B
    //    has a CANCEL audit_log row pointing to that exact same entity_id.
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount
        ) VALUES ('comp_a_no_audit', $1, 'vt_jnl', $2, 'manual', NULL, NULL, 'CA-002', '2026-04-10', '2026-04-10', 'cancelled', '50.00')",
    )
    .bind(&comp_a.company_id)
    .bind(&comp_a.jnl_manual_series_id)
    .execute(db.pool())
    .await
    .expect("insert comp_a cancelled row without audit");

    sqlx::query(
        "INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, new_values, performed_by, timestamp)
         VALUES ('audit_cross_leak_probe', $1, 'voucher', 'comp_a_no_audit', 'CANCEL', '{\"reason\":\"Leaked From B\"}', 'attacker', '2099-01-01T00:00:00.000Z')",
    )
    .bind(&comp_b.company_id)
    .execute(db.pool())
    .await
    .expect("insert comp_b audit row colliding on entity_id");

    let res_a = VoucherSearchService::search_vouchers(
        db.pool(),
        &comp_a.company_id,
        VoucherSearchFilter::default(),
    )
    .await
    .expect("search comp_a");
    assert_eq!(res_a.total, 2);

    let shared = res_a
        .items
        .iter()
        .find(|i| i.id == "shared_v1")
        .expect("comp_a shared_v1");
    assert_eq!(
        shared.cancellation_reason.as_deref(),
        Some("Company A Legitimate Cancellation")
    );

    let no_audit = res_a
        .items
        .iter()
        .find(|i| i.id == "comp_a_no_audit")
        .expect("comp_a_no_audit");
    assert_eq!(no_audit.cancellation_reason, None);
}

#[tokio::test]
async fn test_h17_cross_company_lineage_edges_and_shared_ids() {
    let (_dir, db, _) = setup_file_db().await;
    let comp_a = setup_company_fixture(db.pool(), "edge_a").await;
    let comp_b = setup_company_fixture(db.pool(), "edge_b").await;

    // Both companies contain identical voucher IDs ("edge_0", "edge_1"); Comp B additionally has "edge_2".
    for (fix, prefix) in [(&comp_a, "A"), (&comp_b, "B")] {
        for (id, rev_of, vnum) in [
            ("edge_0", None, format!("{}-0", prefix)),
            ("edge_1", Some("edge_0"), format!("{}-1", prefix)),
        ] {
            sqlx::query(
                "INSERT INTO vouchers (
                    id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, total_amount, reversal_of_voucher_id
                ) VALUES ($1, $2, 'vt_jnl', $3, 'manual', NULL, NULL, $4, '2026-04-10', '2026-04-10', 'posted', '10.00', $5)",
            )
            .bind(id)
            .bind(&fix.company_id)
            .bind(&fix.jnl_manual_series_id)
            .bind(vnum)
            .bind(rev_of)
            .execute(db.pool())
            .await
            .expect("insert shared edge id");
        }
    }

    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, total_amount, reversal_of_voucher_id
        ) VALUES ('edge_2', $1, 'vt_jnl', $2, 'manual', NULL, NULL, 'B-2', '2026-04-10', '2026-04-10', 'posted', '10.00', 'edge_1')",
    )
    .bind(&comp_b.company_id)
    .bind(&comp_b.jnl_manual_series_id)
    .execute(db.pool())
    .await
    .expect("insert comp_b extra hop edge_2");

    // Lineage on comp_a must contain strictly comp_a's 2 items ("A-0", "A-1"), never edge_2 or B-*!
    let lin_a = VoucherSearchService::get_voucher_lineage(db.pool(), &comp_a.company_id, "edge_0")
        .await
        .expect("comp_a lineage");
    assert_eq!(lin_a.items.len(), 2);
    assert!(lin_a
        .items
        .iter()
        .all(|v| v.company_id == comp_a.company_id));
    assert!(lin_a
        .items
        .iter()
        .all(|v| v.voucher_number.starts_with("A-")));

    // Comp A cannot look up Comp B's unique voucher edge_2
    let err_b = VoucherSearchService::get_voucher_lineage(db.pool(), &comp_a.company_id, "edge_2")
        .await
        .expect_err("comp_a must not access comp_b voucher");
    assert!(matches!(err_b, OpenTallyError::NotFound(_)));

    // Comp B returns all 3 Comp B vouchers
    let lin_b = VoucherSearchService::get_voucher_lineage(db.pool(), &comp_b.company_id, "edge_0")
        .await
        .expect("comp_b lineage");
    assert_eq!(lin_b.items.len(), 3);
    assert!(lin_b
        .items
        .iter()
        .all(|v| v.company_id == comp_b.company_id));

    // All filtered search paths in Comp A return strictly Comp A rows
    let filtered_a = VoucherSearchService::search_vouchers(
        db.pool(),
        &comp_a.company_id,
        VoucherSearchFilter {
            ledger_id: Some(comp_b.dr_ledger_id.clone()),
            ..Default::default()
        },
    )
    .await
    .expect("search comp_a using comp_b ledger_id");
    assert_eq!(filtered_a.total, 0);
}

#[tokio::test]
async fn test_h18_true_concurrent_write_snapshot_isolation_count_and_select() {
    // DatabaseManager::init on file path configures max_connections = 10 + WAL mode,
    // allowing a concurrent writer connection to commit while a reader transaction stays open.
    let (_dir, db, _db_path) = setup_file_db().await;
    let fix = setup_company_fixture(db.pool(), "snapshot").await;

    for i in 1..=3 {
        post_balanced_voucher(
            db.pool(),
            &fix,
            "2026-04-10",
            "100.00",
            Some(&format!("SnapRow {}", i)),
        )
        .await;
    }

    let count_done = Arc::new(Notify::new());
    let write_committed = Arc::new(Notify::new());
    let count_done_hook = Arc::clone(&count_done);
    let write_committed_hook = Arc::clone(&write_committed);

    let hook: TestLifecycleHook = Arc::new(move |point| {
        let count_done_hook = Arc::clone(&count_done_hook);
        let write_committed_hook = Arc::clone(&write_committed_hook);
        Box::pin(async move {
            if point == ReadOnlyLifecyclePoint::AfterSearchCountBeforeSelect {
                count_done_hook.notify_one();
                write_committed_hook.notified().await;
            }
        })
    });

    let pool_reader = db.pool().clone();
    let cid_reader = fix.company_id.clone();
    let filter_reader = VoucherSearchFilter {
        search_query: Some("SnapRow".to_string()),
        ..Default::default()
    };

    let reader_handle = tokio::spawn(with_read_only_lifecycle_hook(hook, async move {
        VoucherSearchService::search_vouchers(&pool_reader, &cid_reader, filter_reader).await
    }));

    // Wait until COUNT(*) inside reader's BEGIN transaction has completed and snapshot is pinned.
    count_done.notified().await;

    // Concurrently insert and COMMIT a 4th matching voucher on another connection in WAL mode.
    let fourth_id = post_balanced_voucher(
        db.pool(),
        &fix,
        "2026-04-10",
        "100.00",
        Some("SnapRow 4 Concurrent"),
    )
    .await;

    // Release reader to execute its SELECT statement inside the original transaction snapshot.
    write_committed.notify_one();

    let snapshot_page = reader_handle
        .await
        .expect("reader task join")
        .expect("reader search success");
    assert_eq!(snapshot_page.total, 3);
    assert_eq!(
        snapshot_page.items.len(),
        3,
        "COUNT(*) and SELECT in execute_read_only_query must observe the identical WAL snapshot"
    );
    assert!(
        !snapshot_page.items.iter().any(|v| v.id == fourth_id),
        "Phantom concurrent commit must not leak between COUNT(*) and SELECT"
    );

    // Next fresh read after snapshot closes observes all 4 committed vouchers.
    let after_page = VoucherSearchService::search_vouchers(
        db.pool(),
        &fix.company_id,
        VoucherSearchFilter {
            search_query: Some("SnapRow".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("fresh post-commit search");
    assert_eq!(after_page.total, 4);
    assert_eq!(after_page.items.len(), 4);
}
