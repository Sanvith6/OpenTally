use crate::accounting::model::*;
use crate::accounting::service::{GroupService, LedgerService, OpeningBalanceService, TreeService};
use crate::accounting::validator::validate_opening_balance_amount;
use crate::company::model::*;
use crate::company::service::CompanyService;
use crate::database::migrations::{run_sprint1_migration, run_stage3_migration};
use crate::database::DatabaseManager;
use sqlx::{Pool, Sqlite};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_accounting_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");

        run_sprint1_migration(db.pool())
            .await
            .expect("Failed to run sprint1 migration");

        run_stage3_migration(db.pool())
            .await
            .expect("Failed to run stage3 migration");

        Self {
            db,
            _temp_dir: temp_dir,
        }
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        self.db.pool()
    }
}

async fn create_seeded_company(pool: &Pool<Sqlite>, name: &str) -> String {
    let req = CreateCompanyRequest {
        name: name.to_string(),
        mailing_name: None,
        country: Some("India".to_string()),
        state: Some("Maharashtra".to_string()),
        financial_year_begins: "2025-04-01".to_string(),
        books_beginning_from: Some("2025-04-01".to_string()),
        currency_symbol: Some("₹".to_string()),
        currency_formal_name: Some("Indian Rupee".to_string()),
        currency_decimal_places: Some(2),
    };

    let company = CompanyService::create_company(pool, req)
        .await
        .expect("Failed to create company");

    company.id
}

#[tokio::test]
async fn test_tc_grp_01_multi_company_account_group_listing() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let alpha_id = create_seeded_company(pool, "Company Alpha").await;
    let beta_id = create_seeded_company(pool, "Company Beta").await;

    // Retrieve Sundry Debtors in Alpha
    let alpha_groups = GroupService::list_groups(pool, &alpha_id, false)
        .await
        .unwrap();
    let debtors_alpha = alpha_groups
        .iter()
        .find(|g| g.name == "Sundry Debtors")
        .unwrap();

    let beta_groups = GroupService::list_groups(pool, &beta_id, false)
        .await
        .unwrap();
    let debtors_beta = beta_groups
        .iter()
        .find(|g| g.name == "Sundry Debtors")
        .unwrap();

    // Create Northern Debtors in Alpha
    let g1 = GroupService::create_group(
        pool,
        &alpha_id,
        CreateAccountGroupRequest {
            name: "Northern Debtors".to_string(),
            parent_id: debtors_alpha.id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(g1.name, "Northern Debtors");

    // Create Southern Debtors in Beta
    let g2 = GroupService::create_group(
        pool,
        &beta_id,
        CreateAccountGroupRequest {
            name: "Southern Debtors".to_string(),
            parent_id: debtors_beta.id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(g2.name, "Southern Debtors");

    // Assert visible count and company isolation: exactly 28 predefined + 1 custom = 29
    let list_alpha = GroupService::list_groups(pool, &alpha_id, false)
        .await
        .unwrap();
    assert_eq!(list_alpha.len(), 29);
    assert!(list_alpha.iter().any(|g| g.name == "Northern Debtors"));
    assert!(!list_alpha.iter().any(|g| g.name == "Southern Debtors"));

    let list_beta = GroupService::list_groups(pool, &beta_id, false)
        .await
        .unwrap();
    assert_eq!(list_beta.len(), 29);
    assert!(list_beta.iter().any(|g| g.name == "Southern Debtors"));
    assert!(!list_beta.iter().any(|g| g.name == "Northern Debtors"));

    // Verify hidden P&L group is excluded when include_hidden = false
    assert!(!list_alpha
        .iter()
        .any(|g| g.name == "Profit & Loss A/c (System)"));
}

#[tokio::test]
async fn test_tc_grp_02_create_user_defined_group() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let parent = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    let created = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Special Clients".to_string(),
            parent_id: parent.id.clone(),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.name, "Special Clients");
    assert_eq!(created.parent_id, Some(parent.id.clone()));
    assert_eq!(created.nature, "Assets");
    assert!(!created.is_primary);
    assert!(!created.is_predefined);
    assert!(!created.is_hidden);
    assert!(!created.affects_gross_profit);
}

#[tokio::test]
async fn test_tc_grp_03_edit_user_defined_group_attributes() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();
    let current_assets = groups.iter().find(|g| g.name == "Current Assets").unwrap();

    let created = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Special Clients".to_string(),
            parent_id: debtors.id.clone(),
        },
    )
    .await
    .unwrap();

    let updated = GroupService::update_group(
        pool,
        &company_id,
        &created.id,
        UpdateAccountGroupRequest {
            name: "VIP Clients".to_string(),
            parent_id: Some(current_assets.id.clone()),
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.name, "VIP Clients");
    assert_eq!(updated.parent_id, Some(current_assets.id.clone()));
    assert_eq!(updated.nature, "Assets");
}

#[tokio::test]
async fn test_tc_grp_04_reject_edits_to_protected_predefined_groups() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let current_assets = groups.iter().find(|g| g.name == "Current Assets").unwrap();
    let capital = groups.iter().find(|g| g.name == "Capital Account").unwrap();

    // Attempt to update predefined group name
    let res = GroupService::update_group(
        pool,
        &company_id,
        &current_assets.id,
        UpdateAccountGroupRequest {
            name: "Liquid Assets".to_string(),
            parent_id: None,
        },
    )
    .await;

    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("protected and cannot be altered"));

    // Attempt to delete predefined group
    let del_res = GroupService::delete_group(pool, &company_id, &current_assets.id).await;
    assert!(del_res.is_err());
    assert!(del_res
        .unwrap_err()
        .to_string()
        .contains("Predefined system account group"));

    // Attempt to reparent across nature
    let created = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Asset Sub".to_string(),
            parent_id: current_assets.id.clone(),
        },
    )
    .await
    .unwrap();

    let cross_res = GroupService::update_group(
        pool,
        &company_id,
        &created.id,
        UpdateAccountGroupRequest {
            name: "Asset Sub".to_string(),
            parent_id: Some(capital.id.clone()), // Capital is Liabilities, Asset Sub is Assets
        },
    )
    .await;

    assert!(cross_res.is_err());
    assert!(cross_res
        .unwrap_err()
        .to_string()
        .contains("Cannot reparent group across differing natures"));
}

#[tokio::test]
async fn test_tc_grp_05_reject_circular_group_hierarchy() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let root_parent = groups.iter().find(|g| g.name == "Current Assets").unwrap();

    // Group 1 -> under Current Assets
    let g1 = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Group 1".to_string(),
            parent_id: root_parent.id.clone(),
        },
    )
    .await
    .unwrap();

    // Group 2 -> under Group 1
    let g2 = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Group 2".to_string(),
            parent_id: g1.id.clone(),
        },
    )
    .await
    .unwrap();

    // Group 3 -> under Group 2
    let g3 = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Group 3".to_string(),
            parent_id: g2.id.clone(),
        },
    )
    .await
    .unwrap();

    // Attempt to reparent Group 1 under Group 3 (would create cycle: G1 -> G3 -> G2 -> G1)
    let res = GroupService::update_group(
        pool,
        &company_id,
        &g1.id,
        UpdateAccountGroupRequest {
            name: "Group 1".to_string(),
            parent_id: Some(g3.id.clone()),
        },
    )
    .await;

    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Circular hierarchy detected"));
}

#[tokio::test]
async fn test_tc_grp_06_reject_cross_company_parent_assignment() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let alpha_id = create_seeded_company(pool, "Company Alpha").await;
    let beta_id = create_seeded_company(pool, "Company Beta").await;

    let beta_groups = GroupService::list_groups(pool, &beta_id, false)
        .await
        .unwrap();
    let beta_debtors = beta_groups
        .iter()
        .find(|g| g.name == "Sundry Debtors")
        .unwrap();

    // Attempt to create group in Alpha pointing to parent in Beta
    let res = GroupService::create_group(
        pool,
        &alpha_id,
        CreateAccountGroupRequest {
            name: "Illicit Group".to_string(),
            parent_id: beta_debtors.id.clone(),
        },
    )
    .await;

    assert!(res.is_err());
}

#[tokio::test]
async fn test_tc_grp_07_reject_deletion_of_non_empty_groups() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    let parent_group = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "Regional Debtors".to_string(),
            parent_id: debtors.id.clone(),
        },
    )
    .await
    .unwrap();

    // Create child sub-group under Regional Debtors
    let _child_subgroup = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "North Branch Debtors".to_string(),
            parent_id: parent_group.id.clone(),
        },
    )
    .await
    .unwrap();

    // Attempt to delete parent_group
    let res = GroupService::delete_group(pool, &company_id, &parent_group.id).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("contains 1 child group"));
}

#[tokio::test]
async fn test_tc_led_01_create_standard_ledger_under_valid_group() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let bank_group = groups.iter().find(|g| g.name == "Bank Accounts").unwrap();

    let ledger = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "ICICI Bank Operating A/c".to_string(),
            group_id: bank_group.id.clone(),
            description: Some("Main operating account".to_string()),
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(ledger.name, "ICICI Bank Operating A/c");
    assert_eq!(ledger.group_id, bank_group.id);
    assert_eq!(ledger.ledger_classification, "standard");
    assert!(!ledger.is_reserved);
    assert_eq!(
        ledger.description.as_deref(),
        Some("Main operating account")
    );

    // Retrieve via get_ledger
    let fetched = LedgerService::get_ledger(pool, &company_id, &ledger.id)
        .await
        .unwrap();
    assert_eq!(fetched.id, ledger.id);
}

#[tokio::test]
async fn test_tc_led_02_reject_standard_ledger_in_hidden_system_group() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    // Retrieve the hidden system group
    let all_groups = GroupService::list_groups(pool, &company_id, true)
        .await
        .unwrap();
    let pnl_group = all_groups
        .iter()
        .find(|g| g.name == "Profit & Loss A/c (System)")
        .unwrap();
    assert!(pnl_group.is_hidden);

    let res = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Custom Expense".to_string(),
            group_id: pnl_group.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await;

    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Standard ledgers cannot belong to hidden system groups"));
}

#[tokio::test]
async fn test_tc_led_03_protect_reserved_cash_and_pnl_ledgers() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let ledgers = LedgerService::list_ledgers(pool, &company_id, None, None)
        .await
        .unwrap();
    let cash_ledger = ledgers.iter().find(|l| l.name == "Cash").unwrap();
    assert!(cash_ledger.is_reserved);

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors_group = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    // 1. Attempt to rename Cash
    let rename_res = LedgerService::update_ledger(
        pool,
        &company_id,
        &cash_ledger.id,
        UpdateLedgerRequest {
            name: "Petty Cash".to_string(),
            group_id: None,
            description: None,
        },
    )
    .await;
    assert!(rename_res.is_err());
    assert!(rename_res
        .unwrap_err()
        .to_string()
        .contains("Reserved system ledger 'Cash' cannot be renamed"));

    // 2. Attempt to reparent Cash
    let reparent_res = LedgerService::update_ledger(
        pool,
        &company_id,
        &cash_ledger.id,
        UpdateLedgerRequest {
            name: "Cash".to_string(),
            group_id: Some(debtors_group.id.clone()),
            description: None,
        },
    )
    .await;
    assert!(reparent_res.is_err());
    assert!(reparent_res
        .unwrap_err()
        .to_string()
        .contains("Reserved system ledger 'Cash' cannot be reparented"));

    // 3. Attempt to delete Cash
    let del_res = LedgerService::delete_ledger(pool, &company_id, &cash_ledger.id).await;
    assert!(del_res.is_err());
    assert!(del_res
        .unwrap_err()
        .to_string()
        .contains("Reserved system ledger 'Cash' cannot be deleted"));
}

#[tokio::test]
async fn test_tc_led_04_prevent_deletion_of_referenced_ledgers() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let creditor_group = groups
        .iter()
        .find(|g| g.name == "Sundry Creditors")
        .unwrap();

    let ledger = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Supplier Alpha".to_string(),
            group_id: creditor_group.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // Insert an opening balance referencing this ledger
    sqlx::query(
        "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
         VALUES ('opb-1', ?, ?, '2025-04-01', 'Cr', '10000.00')"
    )
    .bind(&company_id)
    .bind(&ledger.id)
    .execute(pool)
    .await
    .unwrap();

    // Attempt to delete ledger
    let res = LedgerService::delete_ledger(pool, &company_id, &ledger.id).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("opening balance exists"));
}

#[tokio::test]
async fn test_tc_opb_01_opening_balance_validation_and_precision_enforcement() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    let ledger = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Customer Alpha".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // 1. Valid balance
    let saved = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: ledger.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "50000.00".to_string(),
        },
    )
    .await
    .unwrap();

    assert!(saved.is_some());
    let opb = saved.unwrap();
    assert_eq!(opb.amount, "50000.00");
    assert_eq!(opb.balance_type, "Dr");

    // Check audit_log entry exists
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = ? AND entity_type = 'opening_balance' AND action = 'CREATE'"
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    // 2. Negative amount rejected
    let neg_res = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: ledger.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "-500.00".to_string(),
        },
    )
    .await;
    assert!(neg_res.is_err());
    assert!(neg_res
        .unwrap_err()
        .to_string()
        .contains("must be non-negative"));

    // 3. Exceeds decimal places rejected (no silent rounding)
    let dec_res = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: ledger.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "100.235".to_string(),
        },
    )
    .await;
    assert!(dec_res.is_err());
    assert!(dec_res
        .unwrap_err()
        .to_string()
        .contains("exceeds company currency precision"));

    // 4. Zero amount clears opening balance
    let clear_res = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: ledger.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "0.00".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(clear_res.is_none());

    // Row in opening_balances must be gone
    let fetched = OpeningBalanceService::get_opening_balance(pool, &company_id, &ledger.id, None)
        .await
        .unwrap();
    assert!(fetched.is_none());

    // Audit log must record CLEAR action
    let clear_audit: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = ? AND entity_type = 'opening_balance' AND action = 'CLEAR'"
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(clear_audit, 1);
}

#[tokio::test]
async fn test_tc_opb_02_opening_balance_equilibrium_rollup() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();
    let creditors = groups
        .iter()
        .find(|g| g.name == "Sundry Creditors")
        .unwrap();

    let l1 = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "L1".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let l2 = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "L2".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let l3 = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "L3".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let l4 = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "L4".to_string(),
            group_id: creditors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // L1: 500 Dr, L2: 300 Dr, L3: 200 Dr, L4: 800 Cr
    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l1.id,
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "500.00".to_string(),
        },
    )
    .await
    .unwrap();

    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l2.id,
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "300.00".to_string(),
        },
    )
    .await
    .unwrap();

    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l3.id,
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "200.00".to_string(),
        },
    )
    .await
    .unwrap();

    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l4.id,
            financial_year_begins: None,
            balance_type: BalanceType::Cr,
            amount: "800.00".to_string(),
        },
    )
    .await
    .unwrap();

    let eq = OpeningBalanceService::get_equilibrium(pool, &company_id, None)
        .await
        .unwrap();

    assert_eq!(eq.total_debit, "1000.00");
    assert_eq!(eq.total_credit, "800.00");
    assert_eq!(eq.difference, "200.00");
    assert_eq!(eq.difference_type, Some(BalanceType::Dr));
    assert!(!eq.is_balanced);
}

#[tokio::test]
async fn test_tc_opb_03_decimal_normalization_and_exact_arithmetic() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Precision Test Co").await;

    // 1. Direct validator verification
    // 100.2300 normalizes to 100.23 (scale 2 <= 2)
    let norm = validate_opening_balance_amount("100.2300", 2).unwrap();
    assert_eq!(norm.to_string(), "100.23");
    assert_eq!(norm.scale(), 2);

    // 100.234 is rejected (scale 3 > 2)
    let rej = validate_opening_balance_amount("100.234", 2);
    assert!(rej.is_err());
    assert!(rej
        .unwrap_err()
        .to_string()
        .contains("exceeds company currency precision"));

    // 0 and 0.000 normalize to 0
    let zero = validate_opening_balance_amount("0.000", 2).unwrap();
    assert!(zero.is_zero());

    // 2. Service-level normalization and persistence
    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();
    let creditors = groups
        .iter()
        .find(|g| g.name == "Sundry Creditors")
        .unwrap();

    let l_dr = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Customer Alpha".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let l_cr = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Supplier Beta".to_string(),
            group_id: creditors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // Set 100.2300 on l_dr -> normalizes to 100.23
    let saved_dr = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l_dr.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "100.2300".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("Saved opening balance");

    assert_eq!(saved_dr.amount, "100.23");

    // Setting 100.234 on l_dr must fail
    let err_res = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l_dr.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "100.234".to_string(),
        },
    )
    .await;
    assert!(err_res.is_err());

    // Set 100.23 on l_cr
    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l_cr.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Cr,
            amount: "100.23".to_string(),
        },
    )
    .await
    .unwrap();

    // Verify exact equilibrium: 100.23 Dr == 100.23 Cr
    let eq = OpeningBalanceService::get_equilibrium(pool, &company_id, None)
        .await
        .unwrap();
    assert_eq!(eq.total_debit, "100.23");
    assert_eq!(eq.total_credit, "100.23");
    assert_eq!(eq.difference, "0.00");
    assert_eq!(eq.difference_type, None);
    assert!(eq.is_balanced);

    // Explicit clearing via zero
    let clear_dr = OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: l_dr.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "0.000".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(clear_dr.is_none());

    // Verify deleted
    let fetched = OpeningBalanceService::get_opening_balance(pool, &company_id, &l_dr.id, None)
        .await
        .unwrap();
    assert!(fetched.is_none());

    // Verify CLEAR logged in audit_log
    let clear_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = ? AND ledger_id = ? AND action = 'CLEAR'"
    )
    .bind(&company_id)
    .bind(&l_dr.id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(clear_count, 1);
}

#[tokio::test]
async fn test_tc_tre_01_multi_level_recursive_tree_rollup() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    // Create sub-group North Region under Sundry Debtors
    let north_region = GroupService::create_group(
        pool,
        &company_id,
        CreateAccountGroupRequest {
            name: "North Region".to_string(),
            parent_id: debtors.id.clone(),
        },
    )
    .await
    .unwrap();

    // Create ledger Client A under North Region
    let client_a = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Client A".to_string(),
            group_id: north_region.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // Set opening balance of 10000.00 Dr on Client A
    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: client_a.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "10000.00".to_string(),
        },
    )
    .await
    .unwrap();

    // Get chart of accounts tree
    let tree = TreeService::get_chart_of_accounts_tree(pool, &company_id)
        .await
        .unwrap();

    // Root group: Current Assets
    let current_assets_node = tree
        .nodes
        .iter()
        .find(|n| n.name == "Current Assets")
        .expect("Current Assets group missing");
    assert_eq!(current_assets_node.debit_total, "10000.00");

    // Secondary group: Sundry Debtors under Current Assets
    let debtors_node = current_assets_node
        .children
        .iter()
        .find(|n| n.name == "Sundry Debtors")
        .expect("Sundry Debtors node missing");
    assert_eq!(debtors_node.debit_total, "10000.00");

    // Sub-group: North Region under Sundry Debtors
    let north_node = debtors_node
        .children
        .iter()
        .find(|n| n.name == "North Region")
        .expect("North Region node missing");
    assert_eq!(north_node.debit_total, "10000.00");

    // Ledger: Client A under North Region
    let client_node = north_node
        .children
        .iter()
        .find(|n| n.name == "Client A")
        .expect("Client A node missing");
    assert_eq!(client_node.debit_total, "10000.00");
}

#[tokio::test]
async fn test_audit_log_immutable_triggers() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    let groups = GroupService::list_groups(pool, &company_id, false)
        .await
        .unwrap();
    let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

    let ledger = LedgerService::create_ledger(
        pool,
        &company_id,
        CreateLedgerRequest {
            name: "Audit Test Ledger".to_string(),
            group_id: debtors.id.clone(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    OpeningBalanceService::set_opening_balance(
        pool,
        &company_id,
        SetOpeningBalanceRequest {
            ledger_id: ledger.id.clone(),
            financial_year_begins: None,
            balance_type: BalanceType::Dr,
            amount: "1500.00".to_string(),
        },
    )
    .await
    .unwrap();

    let audit_entry: (String, String) = sqlx::query_as(
        "SELECT id, action FROM audit_log WHERE company_id = ? AND entity_type = 'opening_balance'",
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .unwrap();

    // 1. Direct UPDATE on audit_log should be rejected by trg_prevent_audit_log_mutation
    let update_res = sqlx::query("UPDATE audit_log SET action = 'ALTERED' WHERE id = ?")
        .bind(&audit_entry.0)
        .execute(pool)
        .await;

    assert!(update_res.is_err());
    let err_msg = update_res.unwrap_err().to_string();
    assert!(err_msg.contains("Audit log entries are immutable and cannot be updated"));

    // 2. Direct DELETE on audit_log should be rejected by trg_prevent_audit_log_deletion
    let delete_res = sqlx::query("DELETE FROM audit_log WHERE id = ?")
        .bind(&audit_entry.0)
        .execute(pool)
        .await;

    assert!(delete_res.is_err());
    let del_err = delete_res.unwrap_err().to_string();
    assert!(del_err.contains("Audit log entries are permanent and cannot be deleted"));
}

#[tokio::test]
async fn test_predefined_group_deletion_trigger() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    // Direct DELETE on predefined group should be rejected by trg_prevent_predefined_group_deletion
    let delete_res =
        sqlx::query("DELETE FROM account_groups WHERE company_id = ? AND name = 'Current Assets'")
            .bind(&company_id)
            .execute(pool)
            .await;

    assert!(delete_res.is_err());
    let err_msg = delete_res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Predefined system account groups cannot be deleted")
            || err_msg.contains("Cannot delete account group")
    );
}

#[tokio::test]
async fn test_tc_sec_01_company_isolation_and_unselected_state() {
    let fixture = TestDb::new().await;
    let pool = fixture.pool();
    let company_id = create_seeded_company(pool, "Test Co").await;

    // Direct invocation with no company ID or invalid active company ID should return Validation error
    let fake_id = "00000000-0000-0000-0000-000000000000";
    let res = TreeService::get_chart_of_accounts_tree(pool, fake_id).await;
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Company with ID") || err.contains("not found"));

    // Verify valid company succeeds
    let tree = TreeService::get_chart_of_accounts_tree(pool, &company_id)
        .await
        .unwrap();
    assert!(!tree.nodes.is_empty());
}
