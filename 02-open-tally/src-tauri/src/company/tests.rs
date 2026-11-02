use crate::company::model::*;
use crate::company::service::CompanyService;
use crate::database::migrations::run_sprint1_migration;
use crate::database::DatabaseManager;
use sqlx::{Pool, Sqlite};

/// Test fixture managing an isolated temporary SQLite database
/// with Sprint 1 schema applied.
struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_company_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");

        // Run foundational Sprint 1 migrations so all 5 master tables exist
        run_sprint1_migration(db.pool())
            .await
            .expect("Failed to run sprint1 migration");

        Self {
            db,
            _temp_dir: temp_dir,
        }
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        self.db.pool()
    }
}

// ===========================================================================
// 1. Domain Model Validation Tests
// ===========================================================================

#[test]
fn test_val_01_valid_calendar_date() {
    let res = validate_calendar_date("2026-04-01");
    assert!(res.is_ok(), "2026-04-01 should be a valid calendar date");
    let date = res.unwrap();
    assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
}

#[test]
fn test_val_02_impossible_calendar_date() {
    let res = validate_calendar_date("2026-19-45");
    assert!(
        res.is_err(),
        "Impossible date 2026-19-45 must be rejected by validator"
    );
}

#[test]
fn test_val_03_non_leap_february_29() {
    let res = validate_calendar_date("2023-02-29");
    assert!(
        res.is_err(),
        "Non-leap year February 29 (2023-02-29) must be rejected"
    );
}

#[test]
fn test_val_04_leap_february_29() {
    let res = validate_calendar_date("2024-02-29");
    assert!(
        res.is_ok(),
        "Leap year February 29 (2024-02-29) must be accepted as valid"
    );
    let date = res.unwrap();
    assert_eq!(date, chrono::NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
}

#[test]
fn test_val_05_books_beginning_bounds_valid() {
    // Normal FY: 2026-04-01 to 2027-03-31
    assert!(validate_financial_year("2026-04-01", "2026-04-01").is_ok());
    assert!(validate_financial_year("2026-04-01", "2026-10-15").is_ok());
    assert!(validate_financial_year("2026-04-01", "2027-03-31").is_ok());

    // Leap year FY: 2023-04-01 to 2024-03-31 (Feb 2024 has 29 days)
    assert!(validate_financial_year("2023-04-01", "2024-02-29").is_ok());
}

#[test]
fn test_val_06_books_beginning_before_fy_rejected() {
    let res = validate_financial_year("2026-04-01", "2026-03-31");
    assert!(
        res.is_err(),
        "Books beginning before FY start (2026-03-31 < 2026-04-01) must be rejected"
    );
}

#[test]
fn test_val_07_books_beginning_after_fy_rejected() {
    let res = validate_financial_year("2026-04-01", "2027-04-01");
    assert!(
        res.is_err(),
        "Books beginning after FY end (2027-04-01 > 2027-03-31) must be rejected"
    );
}

#[test]
fn test_val_08_currency_decimals_bounds() {
    for valid_dec in 0..=4 {
        assert!(
            validate_currency_decimals(valid_dec).is_ok(),
            "Decimals {} should be valid",
            valid_dec
        );
    }

    assert!(
        validate_currency_decimals(-1).is_err(),
        "Negative decimals must be rejected"
    );
    assert!(
        validate_currency_decimals(5).is_err(),
        "Decimals > 4 must be rejected"
    );
}

// ===========================================================================
// 2. Transactional Company Creation & Seeding Tests
// ===========================================================================

#[tokio::test]
async fn test_create_company_seeds_29_groups_and_2_reserved_ledgers() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let req = CreateCompanyRequest {
        name: "Tata Consultancy Services Ltd".to_string(),
        mailing_name: Some("TCS Headquarters Mumbai".to_string()),
        country: Some("India".to_string()),
        state: Some("Maharashtra".to_string()),
        financial_year_begins: "2026-04-01".to_string(),
        books_beginning_from: Some("2026-04-01".to_string()),
        currency_symbol: Some("₹".to_string()),
        currency_formal_name: Some("INR".to_string()),
        currency_decimal_places: Some(2),
    };

    let company = CompanyService::create_company(pool, req)
        .await
        .expect("Company creation must succeed");

    assert_eq!(company.name, "Tata Consultancy Services Ltd");
    assert_eq!(company.status, CompanyStatus::Active);

    // 1. Verify default company settings seeded
    let settings_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM company_settings WHERE company_id = $1")
            .bind(&company.id)
            .fetch_one(pool)
            .await
            .expect("Fetch settings count");
    assert!(
        settings_count > 0,
        "Expected default company settings to be seeded"
    );

    // 2. Verify exactly 29 predefined account groups seeded
    let groups_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = $1")
            .bind(&company.id)
            .fetch_one(pool)
            .await
            .expect("Fetch groups count");
    assert_eq!(
        groups_count, 29,
        "Expected exactly 29 account groups (15 primary + 13 secondary + 1 hidden system group)"
    );

    // Verify 15 Primary groups
    let primary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_groups WHERE company_id = $1 AND is_primary = 1 AND is_hidden = 0",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .expect("Fetch primary groups count");
    assert_eq!(primary_count, 15, "Expected 15 primary user groups");

    // Verify 13 Secondary groups
    let secondary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_groups WHERE company_id = $1 AND is_primary = 0",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .expect("Fetch secondary groups count");
    assert_eq!(secondary_count, 13, "Expected 13 secondary user groups");

    // Verify 1 Hidden System group (Profit & Loss A/c System)
    let hidden_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_groups WHERE company_id = $1 AND is_hidden = 1",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .expect("Fetch hidden group count");
    assert_eq!(
        hidden_count, 1,
        "Expected 1 hidden system group for Profit & Loss A/c"
    );

    // 3. Verify exactly 2 reserved ledgers seeded
    let reserved_ledgers: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, name, ledger_classification FROM ledgers WHERE company_id = $1 AND is_reserved = 1",
    )
    .bind(&company.id)
    .fetch_all(pool)
    .await
    .expect("Fetch reserved ledgers");

    assert_eq!(
        reserved_ledgers.len(),
        2,
        "Expected exactly 2 reserved ledgers (Cash and Profit & Loss A/c)"
    );

    let classifications: Vec<String> = reserved_ledgers.iter().map(|l| l.2.clone()).collect();
    assert!(
        classifications.contains(&"cash".to_string()),
        "Must contain reserved 'cash' ledger"
    );
    assert!(
        classifications.contains(&"profit_and_loss".to_string()),
        "Must contain reserved 'profit_and_loss' ledger"
    );

    // Verify Cash is under Cash-in-Hand group
    let cash_group_name: String = sqlx::query_scalar(
        "SELECT g.name FROM ledgers l JOIN account_groups g ON l.company_id = g.company_id AND l.group_id = g.id
         WHERE l.company_id = $1 AND l.ledger_classification = 'cash'",
    )
    .bind(&company.id)
    .fetch_one(pool)
    .await
    .expect("Fetch Cash ledger group name");
    assert_eq!(cash_group_name, "Cash-in-Hand");
}

#[tokio::test]
async fn test_create_company_duplicate_name_rejected() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let req1 = CreateCompanyRequest {
        name: "Acme Enterprises".to_string(),
        mailing_name: None,
        country: None,
        state: None,
        financial_year_begins: "2026-04-01".to_string(),
        books_beginning_from: None,
        currency_symbol: None,
        currency_formal_name: None,
        currency_decimal_places: None,
    };

    let _ = CompanyService::create_company(pool, req1)
        .await
        .expect("First company creation must succeed");

    // Case-insensitive duplicate attempt
    let req2 = CreateCompanyRequest {
        name: "acme enterprises".to_string(),
        mailing_name: None,
        country: None,
        state: None,
        financial_year_begins: "2026-04-01".to_string(),
        books_beginning_from: None,
        currency_symbol: None,
        currency_formal_name: None,
        currency_decimal_places: None,
    };

    let res = CompanyService::create_company(pool, req2).await;
    assert!(
        res.is_err(),
        "Duplicate company name must be rejected by uniqueness constraint"
    );
}

#[tokio::test]
async fn test_create_company_rollback_on_failure() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    // Invalid calendar date should fail before or during transaction
    let req = CreateCompanyRequest {
        name: "Crash Test Corp".to_string(),
        mailing_name: None,
        country: None,
        state: None,
        financial_year_begins: "2026-19-45".to_string(), // Invalid date
        books_beginning_from: None,
        currency_symbol: None,
        currency_formal_name: None,
        currency_decimal_places: None,
    };

    let res = CompanyService::create_company(pool, req).await;
    assert!(res.is_err(), "Company creation with invalid date must fail");

    // Verify zero records were written to companies
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM companies WHERE name = 'Crash Test Corp'")
            .fetch_one(pool)
            .await
            .expect("Count companies");
    assert_eq!(
        count, 0,
        "No company record should exist after failed creation"
    );
}

// ===========================================================================
// 3. Company Operations Tests (List, Get, Update, Archive, Select)
// ===========================================================================

#[tokio::test]
async fn test_list_and_get_company() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let c1 = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Alpha Corp".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Alpha");

    let _c2 = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Beta Corp".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Beta");

    // List companies
    let companies = CompanyService::list_companies(pool)
        .await
        .expect("List companies");
    assert!(companies.len() >= 2);
    let names: Vec<String> = companies.iter().map(|c| c.name.clone()).collect();
    assert!(names.contains(&"Alpha Corp".to_string()));
    assert!(names.contains(&"Beta Corp".to_string()));

    // Get company by ID
    let fetched = CompanyService::get_company(pool, &c1.id)
        .await
        .expect("Get company by ID");
    assert_eq!(fetched.id, c1.id);
    assert_eq!(fetched.name, "Alpha Corp");

    // Get non-existent ID fails
    let non_existent = CompanyService::get_company(pool, "non_existent_id").await;
    assert!(non_existent.is_err(), "Non-existent ID must return Err");
}

#[tokio::test]
async fn test_update_company() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let c = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Gamma Industries".to_string(),
            mailing_name: Some("Initial Mailing".to_string()),
            country: Some("India".to_string()),
            state: Some("Delhi".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .expect("Create Gamma");

    let update_req = UpdateCompanyRequest {
        mailing_name: Some("Updated Mailing Address".to_string()),
        country: Some("India".to_string()),
        state: Some("Haryana".to_string()),
        currency_symbol: Some("Rs.".to_string()),
        currency_formal_name: Some("Rupees".to_string()),
    };

    let updated = CompanyService::update_company(pool, &c.id, update_req)
        .await
        .expect("Update company");

    assert_eq!(updated.mailing_name, "Updated Mailing Address");
    assert_eq!(updated.state, "Haryana");
    assert_eq!(updated.currency_symbol, "Rs.");
    assert_eq!(updated.currency_formal_name, "Rupees");

    // Invariant fields remain unchanged
    assert_eq!(updated.name, "Gamma Industries");
    assert_eq!(updated.financial_year_begins, "2026-04-01");
}

#[tokio::test]
async fn test_archive_company_and_prevent_selection() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let c = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Delta Logistics".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Delta");

    // Selecting active company should succeed
    let selected = CompanyService::select_active_company(pool, &c.id)
        .await
        .expect("Select active company");
    assert_eq!(selected.id, c.id);

    // Soft archive the company
    let archived = CompanyService::archive_company(pool, &c.id)
        .await
        .expect("Archive company");
    assert_eq!(archived.status, CompanyStatus::Archived);

    // Attempting to select an archived company must fail
    let select_archived = CompanyService::select_active_company(pool, &c.id).await;
    assert!(
        select_archived.is_err(),
        "Attempting to select archived company must return Err"
    );
    let err_msg = select_archived.unwrap_err().to_string();
    assert!(
        err_msg.contains("archived") || err_msg.contains("closed"),
        "Error message should mention archived/closed status, got: {}",
        err_msg
    );
}

// ===========================================================================
// 4. Multi-Company Isolation Tests
// ===========================================================================

#[tokio::test]
async fn test_multi_company_isolation() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    let comp_a = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Company Alpha".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Company Alpha");

    let comp_b = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Company Beta".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Company Beta");

    // 1. Verify Company Alpha has 29 groups scoped to comp_a.id
    let a_groups: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = $1")
            .bind(&comp_a.id)
            .fetch_one(pool)
            .await
            .expect("Count A groups");
    assert_eq!(a_groups, 29);

    // 2. Verify Company Beta has 29 groups scoped to comp_b.id
    let b_groups: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = $1")
            .bind(&comp_b.id)
            .fetch_one(pool)
            .await
            .expect("Count B groups");
    assert_eq!(b_groups, 29);

    // 3. Total groups in database is exactly 58 (29 * 2)
    let total_groups: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account_groups")
        .fetch_one(pool)
        .await
        .expect("Count total groups");
    assert_eq!(total_groups, 58);

    // 4. Archiving Company Alpha does not affect Company Beta
    let _ = CompanyService::archive_company(pool, &comp_a.id)
        .await
        .expect("Archive Alpha");

    let beta_check = CompanyService::get_company(pool, &comp_b.id)
        .await
        .expect("Get Beta");
    assert_eq!(beta_check.status, CompanyStatus::Active);

    // Beta can still be selected as active company
    let beta_selected = CompanyService::select_active_company(pool, &comp_b.id).await;
    assert!(
        beta_selected.is_ok(),
        "Company Beta should still be selectable"
    );
}

#[tokio::test]
async fn test_active_company_persistence_across_app_restarts() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    // 1. Create Company A and Company B
    let comp_a = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Persistent Corp A".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Comp A");

    let comp_b = CompanyService::create_company(
        pool,
        CreateCompanyRequest {
            name: "Persistent Corp B".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: None,
        },
    )
    .await
    .expect("Create Comp B");

    // Initially, no active company is persisted
    let initial_active = CompanyService::get_persisted_active_company_id(pool)
        .await
        .expect("Fetch persisted");
    assert_eq!(initial_active, None);

    // 2. Select Company A and persist
    let selected_a = CompanyService::select_active_company(pool, &comp_a.id)
        .await
        .expect("Select A");
    CompanyService::set_persisted_active_company_id(pool, Some(&selected_a.id))
        .await
        .expect("Persist A");

    // 3. Simulate App Restart: read persisted active company from DB
    let restored_id = CompanyService::get_persisted_active_company_id(pool)
        .await
        .expect("Fetch persisted")
        .expect("Must have persisted active ID");
    assert_eq!(restored_id, comp_a.id);
    let restored_comp = CompanyService::get_company(pool, &restored_id)
        .await
        .expect("Fetch restored company");
    assert_eq!(restored_comp.name, "Persistent Corp A");

    // 4. Switch to Company B
    let selected_b = CompanyService::select_active_company(pool, &comp_b.id)
        .await
        .expect("Select B");
    CompanyService::set_persisted_active_company_id(pool, Some(&selected_b.id))
        .await
        .expect("Persist B");

    // 5. Verify Company B is now restored
    let restored_b_id = CompanyService::get_persisted_active_company_id(pool)
        .await
        .expect("Fetch persisted")
        .expect("Must have persisted active ID");
    assert_eq!(restored_b_id, comp_b.id);

    // 6. Archive Company B and verify active company is cleared
    let _ = CompanyService::archive_company(pool, &comp_b.id)
        .await
        .expect("Archive B");
    CompanyService::set_persisted_active_company_id(pool, None)
        .await
        .expect("Clear persisted");

    let post_archive_active = CompanyService::get_persisted_active_company_id(pool)
        .await
        .expect("Fetch persisted");
    assert_eq!(post_archive_active, None);
}

#[tokio::test]
async fn test_injected_seeding_failure_rolls_back_entire_transaction() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();

    // Begin an explicit transaction
    let mut tx = pool.begin().await.expect("Begin tx");

    let fake_id = uuid::Uuid::new_v4().to_string();

    // 1. Insert company row inside transaction
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_symbol, currency_formal_name, currency_decimal_places, status, created_at, updated_at) \
         VALUES ($1, 'Crash Canary Corp', 'Canary', 'India', 'Goa', '2026-04-01', '2026-04-01', '₹', 'INR', 2, 'active', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(&fake_id)
    .execute(&mut *tx)
    .await
    .expect("Insert canary company");

    // 2. Insert partial settings inside transaction
    sqlx::query(
        "INSERT INTO company_settings (company_id, key, value, updated_at) VALUES ($1, 'canary_key', 'canary_val', CURRENT_TIMESTAMP)"
    )
    .bind(&fake_id)
    .execute(&mut *tx)
    .await
    .expect("Insert canary setting");

    // 3. Inject intentional failure (foreign key constraint violation or duplicate primary key)
    let injected_err = sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at) \
         VALUES ('duplicate_gid', $1, 'Duplicate Group', 'NON_EXISTENT_PARENT', 'Assets', 0, 1, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    )
    .bind(&fake_id)
    .execute(&mut *tx)
    .await;

    assert!(
        injected_err.is_err(),
        "Injected foreign key violation must fail"
    );

    // Rollback transaction (explicitly or via drop)
    tx.rollback().await.expect("Rollback tx");

    // 4. Verify complete transaction rollback: zero rows left in ANY table for fake_id
    let company_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companies WHERE id = $1")
        .bind(&fake_id)
        .fetch_one(pool)
        .await
        .expect("Count companies");
    assert_eq!(
        company_count, 0,
        "Transaction rollback must leave 0 rows in companies"
    );

    let settings_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM company_settings WHERE company_id = $1")
            .bind(&fake_id)
            .fetch_one(pool)
            .await
            .expect("Count settings");
    assert_eq!(
        settings_count, 0,
        "Transaction rollback must leave 0 rows in company_settings"
    );

    let groups_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = $1")
            .bind(&fake_id)
            .fetch_one(pool)
            .await
            .expect("Count groups");
    assert_eq!(
        groups_count, 0,
        "Transaction rollback must leave 0 rows in account_groups"
    );
}
