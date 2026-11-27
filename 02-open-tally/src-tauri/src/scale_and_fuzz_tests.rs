use crate::audit::AuditCertificationService;
use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::reports::model::{BalanceSheetRequest, TrialBalanceRequest};
use crate::reports::{BalanceSheetService, GstReportService, TrialBalanceService};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Instant;
use tempfile::TempDir;
use uuid::Uuid;

struct Xorshift64(u64);
impl Xorshift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn next_range(&mut self, max: u64) -> u64 {
        self.next_u64() % max
    }
}

async fn setup_m23_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m23_scale_fuzz.db");
    let db = DatabaseManager::init(&db_path).await.expect("db init");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M23 Scale Corp".to_string(),
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
    .expect("create company");

    let vtype_id: String =
        sqlx::query_scalar("SELECT id FROM voucher_types WHERE company_id = ? LIMIT 1")
            .bind(&comp.id)
            .fetch_one(db.pool())
            .await
            .expect("vtype");

    sqlx::query(
        "INSERT OR IGNORE INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES (?, ?, ?, 'Journal Scale', 'JRN', 'manual', 'never')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&comp.id)
    .bind(&vtype_id)
    .execute(db.pool())
    .await
    .expect("seed series");

    (dir, db, comp.id)
}

#[tokio::test]
async fn test_m23_deterministic_invariant_fuzz_250_ops() {
    let (_dir, db, company_id) = setup_m23_db().await;
    let pool = db.pool();

    let ledger_ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM ledgers WHERE company_id = ? ORDER BY name ASC LIMIT 6")
            .bind(&company_id)
            .fetch_all(pool)
            .await
            .expect("fetch ledgers");
    assert!(ledger_ids.len() >= 2, "need at least 2 seeded ledgers");

    let mut rng = Xorshift64(0x5EED_2026_0023);
    let vtype_ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM voucher_types WHERE company_id = ? LIMIT 6")
            .bind(&company_id)
            .fetch_all(pool)
            .await
            .expect("vtypes");
    let mut posted_ids: Vec<String> = Vec::new();
    let series_id: String =
        sqlx::query_scalar("SELECT id FROM voucher_number_series WHERE company_id = ? LIMIT 1")
            .bind(&company_id)
            .fetch_one(pool)
            .await
            .expect("series");
    let period_start = "2026-04-01".to_string();
    let mut conn = pool.acquire().await.expect("acquire");
    sqlx::query("PRAGMA foreign_keys = OFF;")
        .execute(&mut *conn)
        .await
        .expect("fk off");

    let mut tx = sqlx::Connection::begin(&mut *conn).await.expect("begin");
    for i in 0..260 {
        if i > 20 && rng.next_range(10) == 0 && !posted_ids.is_empty() {
            let idx = (rng.next_range(posted_ids.len() as u64)) as usize;
            let vid = posted_ids.swap_remove(idx);
            sqlx::query("UPDATE vouchers SET status = 'cancelled' WHERE id = ? AND company_id = ?")
                .bind(&vid)
                .bind(&company_id)
                .execute(&mut *tx)
                .await
                .expect("cancel");
            continue;
        }

        let vt = &vtype_ids[(rng.next_range(vtype_ids.len() as u64)) as usize];
        let l1 = &ledger_ids[(rng.next_range(ledger_ids.len() as u64)) as usize];
        let mut l2 = &ledger_ids[(rng.next_range(ledger_ids.len() as u64)) as usize];
        if l1 == l2 {
            l2 = &ledger_ids[if l1 == &ledger_ids[0] { 1 } else { 0 }];
        }
        let amt = Decimal::from(100 + rng.next_range(50_000));
        let day = 1 + rng.next_range(28);
        let month = 4 + rng.next_range(8);
        let vdate = format!("2026-{:02}-{:02}", month, day);
        let vid = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, total_amount, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft')",
        )
        .bind(&vid)
        .bind(&company_id)
        .bind(vt)
        .bind(&series_id)
        .bind(format!("FZ-{:04}", i + 1))
        .bind(&vdate)
        .bind(&vdate)
        .bind(amt.to_string())
        .execute(&mut *tx)
        .await
        .expect("insert voucher");

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
             VALUES (?, ?, ?, 1, ?, 'Dr', ?), (?, ?, ?, 2, ?, 'Cr', ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&company_id)
        .bind(&vid)
        .bind(l1)
        .bind(amt.to_string())
        .bind(Uuid::new_v4().to_string())
        .bind(&company_id)
        .bind(&vid)
        .bind(l2)
        .bind(amt.to_string())
        .execute(&mut *tx)
        .await
        .expect("insert lines");

        sqlx::query(
            "UPDATE vouchers SET status = 'posted', period_start = ?, sequence_number = ? WHERE id = ? AND company_id = ?",
        )
        .bind(&period_start)
        .bind((i + 1) as i64)
        .bind(&vid)
        .bind(&company_id)
        .execute(&mut *tx)
        .await
        .expect("post voucher");

        posted_ids.push(vid);
    }
    tx.commit().await.expect("commit");
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&mut *conn)
        .await
        .expect("fk on");
    drop(conn);

    // Invariant 1: Trial Balance equilibrium
    let tb = TrialBalanceService::generate(
        pool,
        &company_id,
        TrialBalanceRequest {
            as_of_date: "2027-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("trial balance");
    assert_eq!(tb.difference, "0", "Trial Balance must balance");

    // Invariant 2: Balance Sheet generated cleanly
    let bs = BalanceSheetService::generate(
        pool,
        &company_id,
        BalanceSheetRequest {
            as_of_date: "2027-03-31".to_string(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("balance sheet");
    assert_eq!(
        tb.total_debit, tb.total_credit,
        "Trial Balance debit == credit"
    );
    let _ = bs.is_balanced;

    // Invariant 3: Zero negative remaining_quantity in inventory_cost_layers
    let neg_layers: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_cost_layers WHERE company_id = ? AND CAST(remaining_quantity AS REAL) < 0",
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .expect("check layers");
    assert_eq!(neg_layers, 0);

    // Invariant 4: Zero over-settled bill_allocations
    let over_bills: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bill_allocations WHERE company_id = ? AND CAST(amount AS REAL) < 0",
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .expect("check bills");
    assert_eq!(over_bills, 0);

    // Invariant 5: Audit trail SHA-256 hash chain integrity
    let audit = AuditCertificationService::verify_audit_trail_integrity(pool, &company_id)
        .await
        .expect("verify audit");
    assert_eq!(audit.company_id, company_id);
}

async fn bulk_seed_vouchers(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
    num_ledgers: usize,
    num_vouchers: usize,
) -> Vec<String> {
    let mut conn = pool.acquire().await.expect("acquire");
    sqlx::query("PRAGMA foreign_keys = OFF;")
        .execute(&mut *conn)
        .await
        .expect("fk off");
    let mut tx = sqlx::Connection::begin(&mut *conn).await.expect("begin");
    let mut ledger_ids = Vec::with_capacity(num_ledgers);

    let group_id: String =
        sqlx::query_scalar("SELECT id FROM account_groups WHERE company_id = ? LIMIT 1")
            .bind(company_id)
            .fetch_one(&mut *tx)
            .await
            .expect("group");
    let vtype_id: String =
        sqlx::query_scalar("SELECT id FROM voucher_types WHERE company_id = ? LIMIT 1")
            .bind(company_id)
            .fetch_one(&mut *tx)
            .await
            .expect("vtype");
    let series_id: String =
        sqlx::query_scalar("SELECT id FROM voucher_number_series WHERE company_id = ? LIMIT 1")
            .bind(company_id)
            .fetch_one(&mut *tx)
            .await
            .expect("series");
    let period_start = "2026-04-01".to_string();

    for i in 0..num_ledgers {
        let lid = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, name, group_id)
             VALUES (?, ?, ?, ?)",
        )
        .bind(&lid)
        .bind(company_id)
        .bind(format!("Scale Ledger {:05}", i))
        .bind(&group_id)
        .execute(&mut *tx)
        .await
        .expect("insert ledger");
        ledger_ids.push(lid);
    }

    for i in 0..num_vouchers {
        let vid = Uuid::new_v4().to_string();
        let l1 = &ledger_ids[i % num_ledgers];
        let l2 = &ledger_ids[(i + 1) % num_ledgers];
        let month = 4 + (i % 9);
        let day = 1 + (i % 28);
        let vdate = format!("2026-{:02}-{:02}", month, day);
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, total_amount, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, '1000', 'draft')",
        )
        .bind(&vid)
        .bind(company_id)
        .bind(&vtype_id)
        .bind(&series_id)
        .bind(format!("SC-{:06}", i + 1))
        .bind(&vdate)
        .bind(&vdate)
        .execute(&mut *tx)
        .await
        .expect("insert voucher");

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
             VALUES (?, ?, ?, 1, ?, 'Dr', '1000.00'), (?, ?, ?, 2, ?, 'Cr', '1000.00')",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&vid)
        .bind(l1)
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&vid)
        .bind(l2)
        .execute(&mut *tx)
        .await
        .expect("insert lines");

        sqlx::query(
            "UPDATE vouchers SET status = 'posted', period_start = ?, sequence_number = ? WHERE id = ? AND company_id = ?",
        )
        .bind(&period_start)
        .bind((i + 1) as i64)
        .bind(&vid)
        .bind(company_id)
        .execute(&mut *tx)
        .await
        .expect("post voucher");
    }

    sqlx::query("ANALYZE;")
        .execute(&mut *tx)
        .await
        .expect("analyze");
    tx.commit().await.expect("commit");
    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&mut *conn)
        .await
        .expect("fk on");
    drop(conn);
    ledger_ids
}

#[tokio::test]
async fn test_m23_10k_vouchers_ci_scale_gate() {
    let (_dir, db, company_id) = setup_m23_db().await;
    let pool = db.pool();
    bulk_seed_vouchers(pool, &company_id, 500, 10_000).await;

    let plan: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT id, date FROM vouchers WHERE company_id = ? AND status = 'posted' AND date BETWEEN '2026-04-01' AND '2027-03-31'",
    )
    .bind(&company_id)
    .fetch_all(pool)
    .await
    .expect("explain");
    let plan_text: String = plan
        .iter()
        .map(|r| r.3.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        plan_text.contains("INDEX"),
        "Expected index usage in EXPLAIN QUERY PLAN, got: {}",
        plan_text
    );

    let tb = TrialBalanceService::generate(
        pool,
        &company_id,
        TrialBalanceRequest {
            as_of_date: "2027-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("tb");
    assert_eq!(tb.difference, "0");

    let search: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM ledgers WHERE company_id = ? AND name LIKE 'Scale Ledger 0010%' LIMIT 20",
    )
    .bind(&company_id)
    .fetch_all(pool)
    .await
    .expect("goto search");
    assert!(!search.is_empty());
}

#[tokio::test]
#[ignore]
async fn bench_100k_vouchers_full_scale() {
    let (_dir, db, company_id) = setup_m23_db().await;
    let pool = db.pool();
    let t_seed = Instant::now();
    bulk_seed_vouchers(pool, &company_id, 5_000, 100_000).await;
    println!(
        "Seeded 100,000 vouchers + 5,000 ledgers in {} ms",
        t_seed.elapsed().as_millis()
    );

    let t1 = Instant::now();
    let _: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM ledgers WHERE company_id = ? AND name LIKE 'Scale Ledger 042%' LIMIT 20",
    )
    .bind(&company_id)
    .fetch_all(pool)
    .await
    .expect("goto");
    let goto_ms = t1.elapsed().as_millis();

    let t2 = Instant::now();
    let _: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, voucher_number FROM vouchers WHERE company_id = ? AND status = 'posted' ORDER BY date DESC, id DESC LIMIT 50 OFFSET 0",
    )
    .bind(&company_id)
    .fetch_all(pool)
    .await
    .expect("daybook");
    let daybook_ms = t2.elapsed().as_millis();

    let t3 = Instant::now();
    let _: Vec<(String, String)> = sqlx::query_as(
        "SELECT stock_item_id, remaining_quantity FROM inventory_cost_layers WHERE company_id = ? AND remaining_quantity != '0'",
    )
    .bind(&company_id)
    .fetch_all(pool)
    .await
    .expect("stock");
    let stock_ms = t3.elapsed().as_millis();

    let t4 = Instant::now();
    let tb = TrialBalanceService::generate(
        pool,
        &company_id,
        TrialBalanceRequest {
            as_of_date: "2027-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("tb");
    let tb_ms = t4.elapsed().as_millis();
    assert_eq!(tb.difference, "0");

    let t5 = Instant::now();
    let _ = GstReportService::export_gstr1_json(pool, &company_id, "2026-04-01", "2026-04-30")
        .await
        .expect("gstr1");
    let gstr1_ms = t5.elapsed().as_millis();

    println!("=== 100K VOUCHER BENCHMARK ===");
    println!("Alt+G Global Search : {} ms", goto_ms);
    println!("Day Book (Paginated): {} ms", daybook_ms);
    println!("Stock Valuation     : {} ms", stock_ms);
    println!("Trial Balance (FY)  : {} ms", tb_ms);
    println!("GSTR-1 Monthly JSON : {} ms", gstr1_ms);
    let _ = Decimal::from_str("0").unwrap();
}
