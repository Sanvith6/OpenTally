//! Integration Test Suite for Milestone 14: Security Control (RBAC) + Manufacturing BOM
//!
//! Verifies:
//! 1. Security Control setup, Argon2id cryptographic password hashing, and constant-time verification.
//! 2. Authentication sessions, role-based access control (Admin, Accountant, DataEntry, Auditor).
//! 3. Sole active Admin protection (preventing self-lockout or total loss of admin privileges).
//! 4. AppState actor resolution & backward compatibility (security_enabled == 0 operates as "system").
//! 5. BOM creation, validation, and self-referential cycle rejection.
//! 6. Proportional BOM scaling, raw-material FIFO layer consumption across godowns.
//! 7. Finished-goods exact landed unit costing with overhead expense absorption.
//! 8. Atomic rollback on insufficient raw stock.
//! 9. Period lock enforcement on production journals.
//! 10. Multi-company tenant isolation and MCA audit log SHA-256 hash chaining.

use crate::app::AppState;
use crate::config::AppConfig;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::inventory::bom::{
    AdditionalOverheadCostInput, BomService, CreateBomComponentInput, CreateBomInput,
    ExecuteManufacturingJournalInput,
};
use crate::security::models::{
    ActionPermission, CreateUserInput, DeactivateUserInput, EnableSecurityInput, LoginInput,
    UpdateUserRoleInput, UserRole,
};
use crate::security::service::SecurityService;
use rust_decimal::Decimal;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

async fn setup_security_bom_test_fixture() -> (tempfile::TempDir, DatabaseManager, Arc<AppState>) {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("opentally_sec_bom_test.db");
    let db = DatabaseManager::init(&db_path)
        .await
        .expect("Failed to init test database");
    let p = db.pool();

    run_sprint1_migration(p).await.unwrap();
    run_stage3_migration(p).await.unwrap();
    run_sprint2_migration(p).await.unwrap();
    run_sprint2_stage4_hardening_migration(p).await.unwrap();
    run_sprint4_inventory_migration(p).await.unwrap();
    run_sprint4_stage11_stock_migration(p).await.unwrap();
    run_sprint4_stage12_valuation_migration(p).await.unwrap();
    run_sprint5_stage14_integration_migration(p).await.unwrap();
    run_sprint5_stage15_valuation_attribution_migration(p)
        .await
        .unwrap();
    run_sprint6_stage17_party_migration(p).await.unwrap();
    run_sprint6_stage18_sales_migration(p).await.unwrap();
    run_milestone1_commercial_posting_migration(p)
        .await
        .unwrap();
    run_milestone2_billwise_migration(p).await.unwrap();
    run_milestone3_gst_migration(p).await.unwrap();
    run_milestone5_notes_migration(p).await.unwrap();
    run_milestone8_banking_migration(p).await.unwrap();
    run_milestone9_tds_migration(p).await.unwrap();
    run_milestone10_gst_compliance_migration(p).await.unwrap();
    run_milestone11_multi_location_and_costing_migration(p)
        .await
        .unwrap();
    run_milestone12_closing_budgets_audit_migration(p)
        .await
        .unwrap();
    run_milestone13_order_processing_migration(p).await.unwrap();
    run_milestone14_security_and_bom_migration(p).await.unwrap();

    // Seed companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_sec_a', 'Apex Manufacturing Ltd', 'Apex Mfg', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                ('cmp_sec_b', 'Zenith Motors Ltd', 'Zenith Motors', 'India', 'Gujarat', '2026-04-01', '2026-04-01')"
    )
    .execute(p)
    .await
    .unwrap();

    // Seed account groups
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca_a', 'cmp_sec_a', 'Stock-in-Hand', NULL, 'Assets', 1),
                ('grp_de_a', 'cmp_sec_a', 'Direct Expenses', NULL, 'Expenses', 1),
                ('grp_ca_b', 'cmp_sec_b', 'Stock-in-Hand', NULL, 'Assets', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // Seed ledgers (Stock-in-hand, Direct Wages/Overhead)
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_stock_a', 'cmp_sec_a', 'grp_ca_a', 'Stock-in-Hand', 0, 'standard'),
                ('led_overhead_a', 'cmp_sec_a', 'grp_de_a', 'Factory Wages & Power', 0, 'standard'),
                ('led_stock_b', 'cmp_sec_b', 'grp_ca_b', 'Stock-in-Hand', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    // Seed units
    sqlx::query(
        "INSERT INTO units (id, company_id, symbol, formal_name, decimal_precision)
         VALUES ('u_nos_a', 'cmp_sec_a', 'NOS', 'Numbers', 0),
                ('u_kg_a', 'cmp_sec_a', 'KG', 'Kilograms', 2),
                ('u_nos_b', 'cmp_sec_b', 'NOS', 'Numbers', 0)",
    )
    .execute(p)
    .await
    .unwrap();

    // Seed voucher_number_series for vt_jnl
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('vns_jnl_a', 'cmp_sec_a', 'vt_jnl', 'Journal Series', 'JNL', 'manual', 'never'),
                ('vns_jnl_b', 'cmp_sec_b', 'vt_jnl', 'Journal Series', 'JNL', 'manual', 'never')"
    )
    .execute(p)
    .await
    .unwrap();

    // Seed Stock Groups
    sqlx::query(
        "INSERT INTO stock_groups (id, company_id, name)
         VALUES ('grp_stock_a', 'cmp_sec_a', 'General Stock'),
                ('grp_stock_b', 'cmp_sec_b', 'General Stock')",
    )
    .execute(p)
    .await
    .unwrap();

    // Seed Godowns
    sqlx::query(
        "INSERT INTO godowns (id, company_id, name, is_active)
         VALUES ('g_main_a', 'cmp_sec_a', 'Main Factory Floor', 1),
                ('g_raw_a', 'cmp_sec_a', 'Raw Material Godown', 1),
                ('g_main_b', 'cmp_sec_b', 'Main Godown B', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // Seed Stock Items for Company A:
    // Raw item 1: Steel Rods
    // Raw item 2: Rubber Seals
    // Finished good: Industrial Valve Assembly
    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, is_active)
         VALUES ('it_steel_a', 'cmp_sec_a', 'Steel Rods 20mm', 'grp_stock_a', 'u_kg_a', 1),
                ('it_rubber_a', 'cmp_sec_a', 'Rubber Seals', 'grp_stock_a', 'u_nos_a', 1),
                ('it_valve_a', 'cmp_sec_a', 'Industrial Valve Assembly', 'grp_stock_a', 'u_nos_a', 1),
                ('it_steel_b', 'cmp_sec_b', 'Steel Rods 20mm', 'grp_stock_b', 'u_nos_b', 1)"
    )
    .execute(p)
    .await
    .unwrap();

    let state = Arc::new(AppState::new(AppConfig::default(), Arc::new(db.clone())));
    (dir, db, state)
}

#[tokio::test]
async fn test_security_enable_and_argon2id_hashing() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // Initially security is disabled
    let status_before = SecurityService::get_security_status(pool, "cmp_sec_a", None)
        .await
        .unwrap();
    assert!(!status_before.security_enabled);
    assert!(!status_before.has_users);

    // Enable security with admin
    let enable_input = EnableSecurityInput {
        company_id: "cmp_sec_a".to_string(),
        admin_username: "AdminMaster".to_string(),
        admin_password: "SuperSecretPassword123!".to_string(),
    };
    let session = SecurityService::enable_company_security(pool, enable_input, "system")
        .await
        .unwrap();
    assert_eq!(session.username, "AdminMaster");
    assert_eq!(session.role, UserRole::Admin);

    // Check status after
    let status_after = SecurityService::get_security_status(pool, "cmp_sec_a", None)
        .await
        .unwrap();
    assert!(status_after.security_enabled);
    assert!(status_after.has_users);

    // Verify stored password hash format in DB
    let user_row: (String, String) = sqlx::query_as(
        "SELECT username, password_hash FROM company_users WHERE company_id = 'cmp_sec_a' AND username = 'AdminMaster'"
    )
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(user_row.0, "AdminMaster");
    assert!(user_row.1.starts_with("$argon2id$"));

    // Verify audit log has company_security_config event
    let audit_event: (String, String) = sqlx::query_as(
        "SELECT entity_type, performed_by FROM audit_log WHERE company_id = 'cmp_sec_a' AND entity_type = 'company_security_config'"
    )
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(audit_event.0, "company_security_config");
    assert_eq!(audit_event.1, "system");
}

#[tokio::test]
async fn test_authentication_success_failure_and_role_matrix() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: "cmp_sec_a".to_string(),
            admin_username: "sysadmin".to_string(),
            admin_password: "CorrectPassword123!".to_string(),
        },
        "system",
    )
    .await
    .unwrap();

    // 1. Success login
    let session = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_sec_a".to_string(),
            username: "SYSADMIN".to_string(), // case-insensitive username match
            password: "CorrectPassword123!".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(session.username, "sysadmin");
    assert_eq!(session.role, UserRole::Admin);

    // 2. Wrong password
    let err_wrong_pass = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_sec_a".to_string(),
            username: "sysadmin".to_string(),
            password: "WrongPassword!".to_string(),
        },
    )
    .await
    .unwrap_err();

    match err_wrong_pass {
        OpenTallyError::Unauthorized(_) => (),
        other => panic!("Expected Unauthorized error, got {:?}", other),
    }

    // 3. Create other roles
    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_sec_a".to_string(),
            username: "acct_john".to_string(),
            password: "JohnPassword123!".to_string(),
            role: UserRole::Accountant,
        },
        "sysadmin",
    )
    .await
    .unwrap();

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_sec_a".to_string(),
            username: "data_mary".to_string(),
            password: "MaryPassword123!".to_string(),
            role: UserRole::DataEntry,
        },
        "sysadmin",
    )
    .await
    .unwrap();

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_sec_a".to_string(),
            username: "audit_bob".to_string(),
            password: "BobPassword123!".to_string(),
            role: UserRole::Auditor,
        },
        "sysadmin",
    )
    .await
    .unwrap();

    // Verify RBAC Permissions Matrix
    assert!(UserRole::Admin.can_perform(ActionPermission::PostVoucher));
    assert!(UserRole::Admin.can_perform(ActionPermission::CancelOrReverseVoucher));
    assert!(UserRole::Admin.can_perform(ActionPermission::ViewFinancialStatements));
    assert!(UserRole::Admin.can_perform(ActionPermission::YearEndClosing));
    assert!(UserRole::Admin.can_perform(ActionPermission::BackupDatabase));
    assert!(UserRole::Admin.can_perform(ActionPermission::RestoreDatabase));
    assert!(UserRole::Admin.can_perform(ActionPermission::ManageUsers));
    assert!(UserRole::Admin.can_perform(ActionPermission::ManageBom));
    assert!(UserRole::Admin.can_perform(ActionPermission::ExecuteManufacturingJournal));

    assert!(UserRole::Accountant.can_perform(ActionPermission::PostVoucher));
    assert!(UserRole::Accountant.can_perform(ActionPermission::CancelOrReverseVoucher));
    assert!(UserRole::Accountant.can_perform(ActionPermission::ViewFinancialStatements));
    assert!(UserRole::Accountant.can_perform(ActionPermission::BackupDatabase));
    assert!(!UserRole::Accountant.can_perform(ActionPermission::RestoreDatabase));
    assert!(!UserRole::Accountant.can_perform(ActionPermission::ManageUsers));
    assert!(UserRole::Accountant.can_perform(ActionPermission::ManageBom));
    assert!(UserRole::Accountant.can_perform(ActionPermission::ExecuteManufacturingJournal));

    assert!(UserRole::DataEntry.can_perform(ActionPermission::PostVoucher));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::CancelOrReverseVoucher));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::ViewFinancialStatements));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::YearEndClosing));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::BackupDatabase));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::ManageUsers));

    assert!(!UserRole::Auditor.can_perform(ActionPermission::PostVoucher));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::CancelOrReverseVoucher));
    assert!(UserRole::Auditor.can_perform(ActionPermission::ViewFinancialStatements));
    assert!(UserRole::Auditor.can_perform(ActionPermission::ViewStandardReports));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::ManageUsers));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::ManageBom));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::ExecuteManufacturingJournal));
}

#[tokio::test]
async fn test_sole_admin_protection_guardrails() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: "cmp_sec_a".to_string(),
            admin_username: "SoleAdmin".to_string(),
            admin_password: "AdminPassword123!".to_string(),
        },
        "system",
    )
    .await
    .unwrap();

    let users = SecurityService::list_users(pool, "cmp_sec_a")
        .await
        .unwrap();
    let sole_admin = &users[0];

    // Attempting to demote sole admin -> Error
    let demote_res = SecurityService::update_user_role(
        pool,
        UpdateUserRoleInput {
            user_id: sole_admin.id.clone(),
            company_id: "cmp_sec_a".to_string(),
            role: UserRole::Accountant,
        },
        "SoleAdmin",
    )
    .await;
    assert!(demote_res.is_err());

    // Attempting to deactivate sole admin -> Error
    let deact_res = SecurityService::deactivate_user(
        pool,
        DeactivateUserInput {
            company_id: "cmp_sec_a".to_string(),
            user_id: sole_admin.id.clone(),
        },
        "SoleAdmin",
    )
    .await;
    assert!(deact_res.is_err());

    // Create a second admin
    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_sec_a".to_string(),
            username: "BackupAdmin".to_string(),
            password: "BackupPassword123!".to_string(),
            role: UserRole::Admin,
        },
        "SoleAdmin",
    )
    .await
    .unwrap();

    // Now deactivating the first admin succeeds
    let deact_success = SecurityService::deactivate_user(
        pool,
        DeactivateUserInput {
            company_id: "cmp_sec_a".to_string(),
            user_id: sole_admin.id.clone(),
        },
        "BackupAdmin",
    )
    .await;
    assert!(deact_success.is_ok());

    // Trying to login as deactivated user fails
    let login_deactivated = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_sec_a".to_string(),
            username: "SoleAdmin".to_string(),
            password: "AdminPassword123!".to_string(),
        },
    )
    .await;
    assert!(login_deactivated.is_err());
}

#[tokio::test]
async fn test_appstate_actor_resolution_and_backward_compatibility() {
    let (_dir, db, state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // 1. Security disabled: resolve_actor returns "system" without session
    let actor_system = state
        .resolve_actor("cmp_sec_a", ActionPermission::ViewFinancialStatements)
        .await
        .unwrap();
    assert_eq!(actor_system, "system");

    // 2. Enable security
    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: "cmp_sec_a".to_string(),
            admin_username: "chief_admin".to_string(),
            admin_password: "Password123!".to_string(),
        },
        "system",
    )
    .await
    .unwrap();

    // 3. Clear sessions; resolve_actor must now fail with Unauthorized
    state.logout_session("cmp_sec_a");
    let actor_err = state
        .resolve_actor("cmp_sec_a", ActionPermission::ViewFinancialStatements)
        .await;
    assert!(matches!(actor_err, Err(OpenTallyError::Unauthorized(_))));

    // 4. Authenticate as DataEntry
    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_sec_a".to_string(),
            username: "data_user".to_string(),
            password: "UserPassword123!".to_string(),
            role: UserRole::DataEntry,
        },
        "chief_admin",
    )
    .await
    .unwrap();

    let session = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_sec_a".to_string(),
            username: "data_user".to_string(),
            password: "UserPassword123!".to_string(),
        },
    )
    .await
    .unwrap();

    state.set_active_session("cmp_sec_a".to_string(), session);

    // Can create vouchers
    let voucher_actor = state
        .resolve_actor("cmp_sec_a", ActionPermission::PostVoucher)
        .await
        .unwrap();
    assert_eq!(voucher_actor, "data_user");

    // Blocked from ViewFinancialStatements -> Forbidden
    let forbidden_err = state
        .resolve_actor("cmp_sec_a", ActionPermission::ViewFinancialStatements)
        .await;
    assert!(matches!(forbidden_err, Err(OpenTallyError::Forbidden(_))));
}

#[tokio::test]
async fn test_bom_creation_and_self_referential_cycle_rejection() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // 1. Valid BOM
    let valid_input = CreateBomInput {
        company_id: "cmp_sec_a".to_string(),
        finished_stock_item_id: "it_valve_a".to_string(),
        bom_name: "Standard Valve Assembly 2026".to_string(),
        base_output_qty: "10".to_string(),
        default_godown_id: Some("g_main_a".to_string()),
        components: vec![
            CreateBomComponentInput {
                raw_stock_item_id: "it_steel_a".to_string(),
                source_godown_id: Some("g_raw_a".to_string()),
                required_qty: "20".to_string(),
            },
            CreateBomComponentInput {
                raw_stock_item_id: "it_rubber_a".to_string(),
                source_godown_id: Some("g_raw_a".to_string()),
                required_qty: "10".to_string(),
            },
        ],
    };

    let bom = BomService::create_bom(pool, valid_input, "sysadmin")
        .await
        .unwrap();
    assert_eq!(bom.bom_name, "Standard Valve Assembly 2026");
    assert_eq!(bom.components.len(), 2);

    // 2. Reject self-referential cycle: raw item same as finished good
    let invalid_cycle_input = CreateBomInput {
        company_id: "cmp_sec_a".to_string(),
        finished_stock_item_id: "it_valve_a".to_string(),
        bom_name: "Self-Referential Circular BOM".to_string(),
        base_output_qty: "1".to_string(),
        default_godown_id: None,
        components: vec![CreateBomComponentInput {
            raw_stock_item_id: "it_valve_a".to_string(), // SAME ITEM!
            source_godown_id: None,
            required_qty: "1".to_string(),
        }],
    };

    let cycle_res = BomService::create_bom(pool, invalid_cycle_input, "sysadmin").await;
    assert!(cycle_res.is_err());
    assert!(matches!(
        cycle_res.unwrap_err(),
        OpenTallyError::Validation(_)
    ));
}

#[tokio::test]
async fn test_manufacturing_journal_proportional_scaling_and_landed_costing() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // 1. Seed raw material stock in g_raw_a:
    // Create stock voucher & lines
    let sv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_vouchers (id, company_id, voucher_number, voucher_type, voucher_date, narration, status)
         VALUES (?, 'cmp_sec_a', 'RAW-SEED-01', 'receipt', '2026-04-10', 'Opening raw stock', 'posted')"
    )
    .bind(&sv_id)
    .execute(pool)
    .await
    .unwrap();

    let sv_line_steel = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_voucher_lines (id, company_id, voucher_id, line_number, stock_item_id, unit_id, destination_godown_id, quantity, description)
         VALUES (?, 'cmp_sec_a', ?, 1, 'it_steel_a', 'u_kg_a', 'g_raw_a', '100', 'Raw steel receipt')"
    )
    .bind(&sv_line_steel)
    .bind(&sv_id)
    .execute(pool)
    .await
    .unwrap();

    let sv_line_rubber = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_voucher_lines (id, company_id, voucher_id, line_number, stock_item_id, unit_id, destination_godown_id, quantity, description)
         VALUES (?, 'cmp_sec_a', ?, 2, 'it_rubber_a', 'u_nos_a', 'g_raw_a', '50', 'Rubber seals receipt')"
    )
    .bind(&sv_line_rubber)
    .bind(&sv_id)
    .execute(pool)
    .await
    .unwrap();

    // 100 kg Steel @ 50.00/kg in g_raw_a
    let steel_layer_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO inventory_cost_layers (id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id, receipt_date, original_quantity, remaining_quantity, unit_cost, original_value)
         VALUES (?, 'cmp_sec_a', 'it_steel_a', 'g_raw_a', ?, ?, '2026-04-10', '100', '100', '50.00', '5000.00')"
    )
    .bind(&steel_layer_id)
    .bind(&sv_id)
    .bind(&sv_line_steel)
    .execute(pool)
    .await
    .unwrap();

    // 50 nos Rubber Seals @ 20.00/each in g_raw_a
    let rubber_layer_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO inventory_cost_layers (id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id, receipt_date, original_quantity, remaining_quantity, unit_cost, original_value)
         VALUES (?, 'cmp_sec_a', 'it_rubber_a', 'g_raw_a', ?, ?, '2026-04-10', '50', '50', '20.00', '1000.00')"
    )
    .bind(&rubber_layer_id)
    .bind(&sv_id)
    .bind(&sv_line_rubber)
    .execute(pool)
    .await
    .unwrap();

    // 2. Create BOM: Base 10 finished valves requires 20 kg steel + 10 nos rubber seals
    let bom = BomService::create_bom(
        pool,
        CreateBomInput {
            company_id: "cmp_sec_a".to_string(),
            finished_stock_item_id: "it_valve_a".to_string(),
            bom_name: "Standard Valve Assembly".to_string(),
            base_output_qty: "10".to_string(),
            default_godown_id: Some("g_main_a".to_string()),
            components: vec![
                CreateBomComponentInput {
                    raw_stock_item_id: "it_steel_a".to_string(),
                    source_godown_id: Some("g_raw_a".to_string()),
                    required_qty: "20".to_string(),
                },
                CreateBomComponentInput {
                    raw_stock_item_id: "it_rubber_a".to_string(),
                    source_godown_id: Some("g_raw_a".to_string()),
                    required_qty: "10".to_string(),
                },
            ],
        },
        "system",
    )
    .await
    .unwrap();

    // 3. Execute Manufacturing Journal producing 20 valves (Scale factor = 20 / 10 = 2)
    // Steel required = 2 * 20 = 40 kg @ 50 = 2000.00
    // Rubber required = 2 * 10 = 20 nos @ 20 = 400.00
    // Raw material cost = 2400.00
    // Additional overhead expense (Factory Wages) = 600.00
    // Total production cost = 3000.00
    // Finished Goods Landed Unit Cost = 3000.00 / 20 = 150.0000 per valve!
    let mfg_input = ExecuteManufacturingJournalInput {
        company_id: "cmp_sec_a".to_string(),
        bom_id: bom.id.clone(),
        produced_qty: "20".to_string(),
        destination_godown_id: "g_main_a".to_string(),
        production_date: "2026-04-15".to_string(),
        narration: Some("Manufactured Batch #001 valves".to_string()),
        batch_number: Some("BAT-2026-001".to_string()),
        mfg_date: Some("2026-04-15".to_string()),
        expiry_date: Some("2028-04-15".to_string()),
        additional_overheads: vec![AdditionalOverheadCostInput {
            overhead_ledger_id: "led_overhead_a".to_string(),
            amount: "600".to_string(),
            description: Some("Direct Labor & Power allocation".to_string()),
        }],
    };

    let result = BomService::execute_manufacturing_journal(pool, mfg_input, "system")
        .await
        .unwrap();

    assert_eq!(result.produced_qty, "20");
    assert_eq!(
        Decimal::from_str(&result.total_raw_material_cost).unwrap(),
        Decimal::from(2400)
    );
    assert_eq!(
        Decimal::from_str(&result.total_overhead_cost).unwrap(),
        Decimal::from(600)
    );
    assert_eq!(
        Decimal::from_str(&result.total_manufacturing_cost).unwrap(),
        Decimal::from(3000)
    );
    assert_eq!(
        Decimal::from_str(&result.landed_unit_cost).unwrap(),
        Decimal::from_str("150.0000").unwrap()
    );

    // 4. Verify remaining raw material stock in g_raw_a
    let steel_rem: String = sqlx::query_scalar(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_steel_a' AND godown_id = 'g_raw_a'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(steel_rem, "60"); // 100 - 40 = 60

    let rubber_rem: String = sqlx::query_scalar(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_rubber_a' AND godown_id = 'g_raw_a'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rubber_rem, "30"); // 50 - 20 = 30

    // 5. Verify finished goods layer in g_main_a
    let fg_row: (String, String) = sqlx::query_as(
        "SELECT remaining_quantity, unit_cost FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_valve_a' AND godown_id = 'g_main_a'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(fg_row.0, "20");
    assert_eq!(fg_row.1, "150");

    // 6. Verify overhead GL lines posted
    let gl_vch_id: String = sqlx::query_scalar(
        "SELECT id FROM vouchers WHERE company_id = 'cmp_sec_a' AND narration = 'BOM Manufacturing Overhead Capitalization'"
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let gl_lines: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT ledger_id, amount, entry_type FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number"
    )
    .bind(&gl_vch_id)
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(gl_lines.len(), 2);
    // Line 1: Dr Stock-in-Hand 600
    assert_eq!(gl_lines[0].0, "led_stock_a");
    assert_eq!(gl_lines[0].1, "600");
    assert_eq!(gl_lines[0].2, "Dr");

    // Line 2: Cr Overhead Ledger 600
    assert_eq!(gl_lines[1].0, "led_overhead_a");
    assert_eq!(gl_lines[1].1, "600");
    assert_eq!(gl_lines[1].2, "Cr");

    // 7. Verify batch record created in stock_item_batches and linked to finished goods layer
    let batch_row: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT batch_number, mfg_date, expiry_date FROM stock_item_batches WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_valve_a'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(batch_row.0, "BAT-2026-001");
    assert_eq!(batch_row.1.as_deref(), Some("2026-04-15"));
    assert_eq!(batch_row.2.as_deref(), Some("2028-04-15"));

    let fg_batch_id: Option<String> = sqlx::query_scalar(
        "SELECT batch_id FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_valve_a' AND godown_id = 'g_main_a'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        fg_batch_id.is_some(),
        "Batch ID must be linked to finished goods cost layer"
    );

    // 8. Verify MCA Audit Trail cryptographic integrity
    let cert_report =
        crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
            pool,
            "cmp_sec_a",
        )
        .await
        .unwrap();
    assert!(
        cert_report.is_valid,
        "MCA Audit Trail SHA-256 chain must be fully valid"
    );
    assert!(
        cert_report.total_entries >= 2,
        "Expected audit entries for stock journal and overhead voucher"
    );
    assert_eq!(
        cert_report.tampered_entries, 0,
        "Zero tampered entries expected in audit trail"
    );
}

#[tokio::test]
async fn test_manufacturing_journal_insufficient_stock_atomic_rollback() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // Seed only 5 kg steel (BOM requires 20 kg for 10 units)
    let sv_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_vouchers (id, company_id, voucher_number, voucher_type, voucher_date, narration, status)
         VALUES (?, 'cmp_sec_a', 'RAW-SEED-02', 'receipt', '2026-04-10', 'Opening raw steel', 'posted')"
    )
    .bind(&sv_id)
    .execute(pool)
    .await
    .unwrap();

    let sv_line_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_voucher_lines (id, company_id, voucher_id, line_number, stock_item_id, unit_id, destination_godown_id, quantity, description)
         VALUES (?, 'cmp_sec_a', ?, 1, 'it_steel_a', 'u_kg_a', 'g_raw_a', '5', '5kg steel')"
    )
    .bind(&sv_line_id)
    .bind(&sv_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO inventory_cost_layers (id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id, receipt_date, original_quantity, remaining_quantity, unit_cost, original_value)
         VALUES (?, 'cmp_sec_a', 'it_steel_a', 'g_raw_a', ?, ?, '2026-04-10', '5', '5', '50.00', '250.00')"
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&sv_id)
    .bind(&sv_line_id)
    .execute(pool)
    .await
    .unwrap();

    let bom = BomService::create_bom(
        pool,
        CreateBomInput {
            company_id: "cmp_sec_a".to_string(),
            finished_stock_item_id: "it_valve_a".to_string(),
            bom_name: "Valve BOM".to_string(),
            base_output_qty: "10".to_string(),
            default_godown_id: Some("g_main_a".to_string()),
            components: vec![CreateBomComponentInput {
                raw_stock_item_id: "it_steel_a".to_string(),
                source_godown_id: Some("g_raw_a".to_string()),
                required_qty: "20".to_string(),
            }],
        },
        "system",
    )
    .await
    .unwrap();

    // Attempting to produce 10 valves requires 20 kg steel, but only 5 kg exists!
    let mfg_input = ExecuteManufacturingJournalInput {
        company_id: "cmp_sec_a".to_string(),
        bom_id: bom.id,
        produced_qty: "10".to_string(),
        destination_godown_id: "g_main_a".to_string(),
        production_date: "2026-04-15".to_string(),
        narration: None,
        batch_number: None,
        mfg_date: None,
        expiry_date: None,
        additional_overheads: vec![],
    };

    let res = BomService::execute_manufacturing_journal(pool, mfg_input, "system").await;
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), OpenTallyError::Validation(_)));

    // Verify steel stock is still untouched (5 kg)
    let steel_rem: String = sqlx::query_scalar(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_steel_a' AND godown_id = 'g_raw_a'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(steel_rem, "5");

    // Zero finished goods layers exist
    let fg_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_cost_layers WHERE company_id = 'cmp_sec_a' AND stock_item_id = 'it_valve_a'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(fg_count, 0);
}

#[tokio::test]
async fn test_manufacturing_journal_period_lock_rejection() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // Lock period up to 2026-04-30 by creating dummy closing voucher and closing record
    let dummy_vch_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            voucher_number, date, effective_date, status, total_amount
        ) VALUES (?, 'cmp_sec_a', 'vt_jnl', 'vns_jnl_a', 'manual', 'CLS-DUMMY', '2026-04-30', '2026-04-30', 'posted', '100.00')"
    )
    .bind(&dummy_vch_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO financial_year_closings (
            id, company_id, financial_year_label, period_start, period_end, next_fy_start, closing_voucher_id, net_profit_or_loss, is_locked
        ) VALUES ('lock_1', 'cmp_sec_a', 'FY 2025-26', '2025-04-01', '2026-04-30', '2026-05-01', ?, '0', 1)"
    )
    .bind(&dummy_vch_id)
    .execute(pool)
    .await
    .unwrap();

    let bom = BomService::create_bom(
        pool,
        CreateBomInput {
            company_id: "cmp_sec_a".to_string(),
            finished_stock_item_id: "it_valve_a".to_string(),
            bom_name: "Valve BOM".to_string(),
            base_output_qty: "10".to_string(),
            default_godown_id: Some("g_main_a".to_string()),
            components: vec![CreateBomComponentInput {
                raw_stock_item_id: "it_steel_a".to_string(),
                source_godown_id: Some("g_raw_a".to_string()),
                required_qty: "20".to_string(),
            }],
        },
        "system",
    )
    .await
    .unwrap();

    let mfg_input = ExecuteManufacturingJournalInput {
        company_id: "cmp_sec_a".to_string(),
        bom_id: bom.id,
        produced_qty: "10".to_string(),
        destination_godown_id: "g_main_a".to_string(),
        production_date: "2026-04-15".to_string(), // In locked period!
        narration: None,
        batch_number: None,
        mfg_date: None,
        expiry_date: None,
        additional_overheads: vec![],
    };

    let res = BomService::execute_manufacturing_journal(pool, mfg_input, "system").await;
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), OpenTallyError::PeriodLocked(_)));
}

#[tokio::test]
async fn test_multi_company_tenant_isolation_m14() {
    let (_dir, db, _state) = setup_security_bom_test_fixture().await;
    let pool = db.pool();

    // Enable security for Company A
    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: "cmp_sec_a".to_string(),
            admin_username: "AdminA".to_string(),
            admin_password: "PassA123!".to_string(),
        },
        "system",
    )
    .await
    .unwrap();

    // Verify Company B security remains disabled
    let status_b = SecurityService::get_security_status(pool, "cmp_sec_b", None)
        .await
        .unwrap();
    assert!(!status_b.security_enabled);
    assert!(!status_b.has_users);

    // Create BOM for Company A
    let bom_a = BomService::create_bom(
        pool,
        CreateBomInput {
            company_id: "cmp_sec_a".to_string(),
            finished_stock_item_id: "it_valve_a".to_string(),
            bom_name: "Apex Proprietary Valve".to_string(),
            base_output_qty: "10".to_string(),
            default_godown_id: Some("g_main_a".to_string()),
            components: vec![CreateBomComponentInput {
                raw_stock_item_id: "it_steel_a".to_string(),
                source_godown_id: Some("g_raw_a".to_string()),
                required_qty: "20".to_string(),
            }],
        },
        "AdminA",
    )
    .await
    .unwrap();

    // Company B lists BOMs -> should be empty
    let boms_b = BomService::list_boms(pool, "cmp_sec_b", None)
        .await
        .unwrap();
    assert!(boms_b.is_empty());

    // Company B cannot execute Company A's BOM
    let mfg_cross_company = ExecuteManufacturingJournalInput {
        company_id: "cmp_sec_b".to_string(),
        bom_id: bom_a.id,
        produced_qty: "10".to_string(),
        destination_godown_id: "g_main_b".to_string(),
        production_date: "2026-05-01".to_string(),
        narration: None,
        batch_number: None,
        mfg_date: None,
        expiry_date: None,
        additional_overheads: vec![],
    };

    let cross_res =
        BomService::execute_manufacturing_journal(pool, mfg_cross_company, "system").await;
    assert!(cross_res.is_err());
    assert!(matches!(
        cross_res.unwrap_err(),
        OpenTallyError::NotFound(_)
    ));
}
