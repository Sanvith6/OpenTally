use crate::app::AppState;
use crate::billwise::{AllocationType, BillAllocationInput, BillWiseService};
use crate::config::AppConfig;
use crate::database::DatabaseManager;
use crate::navigation::NavigationSearchService;
use crate::vouchers::model::{SaveVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::{VoucherDraftService, VoucherPostingService, VoucherSeriesService};
use rust_decimal::Decimal;
use std::sync::Arc;

async fn setup_test_state() -> (tempfile::TempDir, AppState, String) {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("nav_test.db");
    let db = DatabaseManager::init(&db_path).await.unwrap();

    crate::database::migrations::run_sprint1_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_stage3_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint2_stage4_hardening_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint4_inventory_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint4_stage11_stock_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint4_stage12_valuation_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint5_stage14_integration_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint5_stage15_valuation_attribution_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint6_stage17_party_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_sprint6_stage18_sales_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_milestone1_commercial_posting_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_milestone2_billwise_migration(db.pool())
        .await
        .unwrap();
    crate::database::migrations::run_milestone3_gst_migration(db.pool())
        .await
        .unwrap();

    let config = AppConfig::default();
    let state = AppState::new(config, Arc::new(db));

    let comp = crate::company::service::CompanyService::create_company(
        state.db.pool(),
        crate::company::model::CreateCompanyRequest {
            name: "Navigation Test Corp".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    (temp_dir, state, comp.id)
}

#[tokio::test]
async fn test_global_goto_search_multi_company_isolation() {
    let (_temp_dir, state, comp1_id) = setup_test_state().await;

    // Create Company 2
    let comp2 = crate::company::service::CompanyService::create_company(
        state.db.pool(),
        crate::company::model::CreateCompanyRequest {
            name: "Other Company Ltd".to_string(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    let grp1_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors'",
    )
    .bind(&comp1_id)
    .fetch_one(state.db.pool())
    .await
    .unwrap();

    let grp2_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors'",
    )
    .bind(&comp2.id)
    .fetch_one(state.db.pool())
    .await
    .unwrap();

    // Insert a unique ledger in Company 1
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_alpha', ?, ?, 'Alpha Unique Supplier', 0, 'standard')",
    )
    .bind(&comp1_id)
    .bind(&grp1_id)
    .execute(state.db.pool())
    .await
    .unwrap();

    // Insert a unique ledger in Company 2
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_beta', ?, ?, 'Beta Unique Supplier', 0, 'standard')",
    )
    .bind(&comp2.id)
    .bind(&grp2_id)
    .execute(state.db.pool())
    .await
    .unwrap();

    // Search in Company 1
    let results_comp1 =
        NavigationSearchService::global_goto_search(state.db.pool(), &comp1_id, "Unique", 20)
            .await
            .unwrap();

    assert_eq!(results_comp1.len(), 1);
    assert_eq!(results_comp1[0].id, "led_alpha");
    assert_eq!(results_comp1[0].title, "Alpha Unique Supplier");
    assert_eq!(results_comp1[0].category, "Ledger");

    // Search in Company 2
    let results_comp2 =
        NavigationSearchService::global_goto_search(state.db.pool(), &comp2.id, "Unique", 20)
            .await
            .unwrap();

    assert_eq!(results_comp2.len(), 1);
    assert_eq!(results_comp2[0].id, "led_beta");
    assert_eq!(results_comp2[0].title, "Beta Unique Supplier");

    // Search empty query
    let empty_res =
        NavigationSearchService::global_goto_search(state.db.pool(), &comp1_id, "   ", 20)
            .await
            .unwrap();
    assert!(empty_res.is_empty());
}

#[tokio::test]
async fn test_get_voucher_full_detail_end_to_end() {
    let (_temp_dir, state, comp_id) = setup_test_state().await;

    // Create a series for payment
    let series = VoucherSeriesService::create_series(
        state.db.pool(),
        &comp_id,
        crate::vouchers::CreateVoucherSeriesRequest {
            voucher_type_id: "vt_payment".to_string(),
            series_name: "Bank Payments".to_string(),
            series_code: "BPAY".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("BPAY/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(3),
        },
    )
    .await
    .unwrap();

    let grp_id: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors'",
    )
    .bind(&comp_id)
    .fetch_one(state.db.pool())
    .await
    .unwrap();

    let cash_id: String =
        sqlx::query_scalar("SELECT id FROM ledgers WHERE company_id = ? AND name = 'Cash'")
            .bind(&comp_id)
            .fetch_one(state.db.pool())
            .await
            .unwrap();

    // Insert vendor ledger in Company 1
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_vendor_mega', ?, ?, 'Mega Hardware Vendor', 0, 'standard')",
    )
    .bind(&comp_id)
    .bind(&grp_id)
    .execute(state.db.pool())
    .await
    .unwrap();

    // Create draft payment voucher
    let draft = VoucherDraftService::save_draft(
        state.db.pool(),
        &comp_id,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_payment".to_string(),
            series_id: series.id.clone(),
            date: "2026-05-15".to_string(),
            effective_date: None,
            reference_no: Some("REF-BP-99".to_string()),
            reference_date: None,
            narration: Some("Payment for hardware supplies".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_vendor_mega".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "5000.00".to_string(),
                    narration: Some("Vendor settlement".to_string()),
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: cash_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: "5000.00".to_string(),
                    narration: Some("Cash outflow".to_string()),
                },
            ],
        },
    )
    .await
    .unwrap();

    // Post payment voucher
    let posted =
        VoucherPostingService::post_voucher(state.db.pool(), &comp_id, &draft.voucher.id, None)
            .await
            .unwrap();

    // Add bill allocation to the vendor line
    let vch_line_id = &posted.lines[0].id;
    let mut conn = state.db.pool().acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        &comp_id,
        &posted.voucher.id,
        vch_line_id,
        "led_vendor_mega",
        "Dr",
        Decimal::from_str_exact("5000.00").unwrap(),
        "2026-05-15",
        Some("2026-06-15"),
        &[BillAllocationInput {
            bill_name: "BILL-2026-001".to_string(),
            allocation_type: AllocationType::NewRef,
            dr_cr: Some("Dr".to_string()),
            amount: "5000.00".to_string(),
            due_date: Some("2026-06-15".to_string()),
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // Fetch full detail
    let detail = NavigationSearchService::get_voucher_full_detail(
        state.db.pool(),
        &comp_id,
        &posted.voucher.id,
    )
    .await
    .unwrap();

    assert_eq!(detail.voucher.id, posted.voucher.id);
    assert_eq!(detail.voucher.voucher_number, posted.voucher.voucher_number);
    assert_eq!(detail.voucher.status, "posted");
    assert_eq!(detail.lines.len(), 2);
    assert_eq!(detail.lines[0].ledger_name, "Mega Hardware Vendor");
    assert_eq!(detail.lines[0].classification, "Liabilities");
    assert_eq!(detail.lines[0].entry_type, "Dr");
    assert_eq!(detail.lines[1].ledger_name, "Cash");

    // Bill allocations verified
    assert_eq!(detail.bill_allocations.len(), 1);
    assert_eq!(detail.bill_allocations[0].bill_name, "BILL-2026-001");
    assert_eq!(detail.bill_allocations[0].allocation_type, "NewRef");
    assert!(
        detail.bill_allocations[0].amount == "5000"
            || detail.bill_allocations[0].amount == "5000.00"
    );

    // Search by voucher number in Go To
    let goto_results = NavigationSearchService::global_goto_search(
        state.db.pool(),
        &comp_id,
        &posted.voucher.voucher_number,
        10,
    )
    .await
    .unwrap();
    assert_eq!(goto_results.len(), 1);
    assert_eq!(goto_results[0].category, "Voucher");
    assert_eq!(goto_results[0].id, posted.voucher.id);
    assert_eq!(goto_results[0].target_view, "voucher_detail");
}
