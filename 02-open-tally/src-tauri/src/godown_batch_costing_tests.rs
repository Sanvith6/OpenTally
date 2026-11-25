//! Integration Test Suite for Milestone 11: Multi-Location Godowns, Stock Journals, Batches & Cost Centres
//!
//! Verifies:
//! 1. Multi-location Godowns (including address and external warehouse flags).
//! 2. Stock Item Batches: creation, uniqueness, and expiry date validation on sales.
//! 3. Batch Expiry Report with ReadOnlyConnectionGuard / execute_read_only_query.
//! 4. Stock Journal Inter-Godown Transfers (Alt+F7): FIFO layer consumption, destination layer creation,
//!    and total valuation invariance (total_dest_value == total_source_value).
//! 5. Godown Summary Report: warehouse-wise balances and valuations.
//! 6. Cost Categories & Cost Centres: hierarchy, allocation sum matching line_amount, and breakup report.
//! 7. Multi-tenant company isolation across all godowns, batches, and cost centres.

use crate::accounting::cost_centres::{
    CostCategoryRequest, CostCentreRequest, CostCentreService, VoucherCostAllocationInput,
};
use crate::company::model::CreateCompanyRequest;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::inventory::batches::{BatchService, StockItemBatchRequest};
use crate::inventory::stock_journal::{StockJournalLine, StockJournalRequest, StockJournalService};
use crate::inventory::{
    GodownRequest, InventoryService, StockGroupRequest, StockItemRequest, UnitRequest,
};
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

async fn setup_test_pool() -> (tempfile::TempDir, Pool<Sqlite>, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("m11_godown_costing_test.db"))
        .await
        .unwrap();
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

    let comp = CompanyService::create_company(
        p,
        CreateCompanyRequest {
            name: "Hindustan Logistics & Precision Pvt Ltd".to_string(),
            mailing_name: Some("Hindustan Logistics & Precision Pvt Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2025-04-01".to_string(),
            books_beginning_from: Some("2025-04-01".to_string()),
            currency_symbol: Some("INR".to_string()),
            currency_formal_name: Some("Indian Rupee".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    (dir, p.clone(), comp.id)
}

#[tokio::test]
async fn test_godown_creation_and_alter_extension() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    let godown_req = GodownRequest {
        name: "Whitefield Warehouse 1".to_string(),
        parent_id: None,
        description: None,
        address: Some("Survey No 88, Whitefield Main Rd, Bengaluru".to_string()),
        is_external: true,
    };

    let created = InventoryService::create_godown(&pool, &company_id, godown_req)
        .await
        .unwrap();
    assert_eq!(created.name, "Whitefield Warehouse 1");
    assert_eq!(
        created.address.as_deref(),
        Some("Survey No 88, Whitefield Main Rd, Bengaluru")
    );
    assert!(created.is_external);

    let list = InventoryService::list_godowns(&pool, &company_id, None)
        .await
        .unwrap();
    let found = list.iter().find(|g| g.id == created.id).unwrap();
    assert_eq!(found.address, created.address);
    assert_eq!(found.is_external, created.is_external);
}

#[tokio::test]
async fn test_batch_creation_uniqueness_and_expiry_validation() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    let unit = InventoryService::create_unit(
        &pool,
        &company_id,
        UnitRequest {
            symbol: "NOS".to_string(),
            formal_name: "Numbers".to_string(),
            unit_type: Some("simple".to_string()),
            base_unit_id: None,
            secondary_unit_id: None,
            conversion_factor: None,
            decimal_precision: 0,
        },
    )
    .await
    .unwrap();

    let grp = InventoryService::create_stock_group(
        &pool,
        &company_id,
        StockGroupRequest {
            name: "Bearings".to_string(),
            parent_id: None,
            description: None,
        },
    )
    .await
    .unwrap();

    let item = InventoryService::create_stock_item(
        &pool,
        &company_id,
        StockItemRequest {
            name: "Industrial Bearing B-100".to_string(),
            stock_group_id: grp.id.clone(),
            stock_category_id: None,
            base_unit_id: unit.id.clone(),
            alternate_unit_id: None,
            sku: Some("BEAR-100".to_string()),
            description: None,
            opening_quantity: "0".to_string(),
            opening_rate: "0".to_string(),
            opening_value: "0".to_string(),
            reorder_level: "0".to_string(),
        },
    )
    .await
    .unwrap();

    let batch_req = StockItemBatchRequest {
        stock_item_id: item.id.clone(),
        batch_number: "BATCH-2025-001".to_string(),
        mfg_date: Some("2025-01-15".to_string()),
        expiry_date: Some("2025-06-30".to_string()),
    };

    let batch = BatchService::create_batch(&pool, &company_id, batch_req.clone())
        .await
        .unwrap();
    assert_eq!(batch.batch_number, "BATCH-2025-001");
    assert_eq!(batch.expiry_date.as_deref(), Some("2025-06-30"));

    // Duplicate batch number for same item in same company must fail
    let dup_res = BatchService::create_batch(&pool, &company_id, batch_req).await;
    assert!(dup_res.is_err(), "Duplicate batch number must be rejected");

    // Test expiry validation on sale:
    let mut conn = pool.acquire().await.unwrap();

    // 1. Sale before expiry date should succeed
    let valid_sale = BatchService::validate_batch_expiry_for_sale(
        &mut conn,
        &company_id,
        &item.id,
        &batch.batch_number,
        "2025-05-10",
    )
    .await;
    assert!(valid_sale.is_ok(), "Sale before expiry must succeed");

    // 2. Sale on expiry date should succeed
    let boundary_sale = BatchService::validate_batch_expiry_for_sale(
        &mut conn,
        &company_id,
        &item.id,
        &batch.batch_number,
        "2025-06-30",
    )
    .await;
    assert!(boundary_sale.is_ok(), "Sale on expiry date must succeed");

    // 3. Sale after expiry date must fail with validation error
    let expired_sale = BatchService::validate_batch_expiry_for_sale(
        &mut conn,
        &company_id,
        &item.id,
        &batch.batch_number,
        "2025-07-01",
    )
    .await;
    assert!(
        expired_sale.is_err(),
        "Sale after expiry date must be rejected"
    );

    // 4. Batch Expiry Report check
    let report = BatchService::get_batch_expiry_report(&pool, &company_id, None, "2025-05-01")
        .await
        .unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].batch_number, "BATCH-2025-001");
    assert_eq!(report[0].status, "ExpiringIn60Days");
}

#[tokio::test]
async fn test_inter_godown_stock_journal_transfer_valuation_invariance() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // Create Godowns
    let godown_a = InventoryService::create_godown(
        &pool,
        &company_id,
        GodownRequest {
            name: "Central Godown".to_string(),
            parent_id: None,
            description: None,
            address: Some("Central Road".to_string()),
            is_external: false,
        },
    )
    .await
    .unwrap();

    let godown_b = InventoryService::create_godown(
        &pool,
        &company_id,
        GodownRequest {
            name: "Suburban Godown".to_string(),
            parent_id: None,
            description: None,
            address: Some("Suburban Road".to_string()),
            is_external: false,
        },
    )
    .await
    .unwrap();

    let unit = InventoryService::create_unit(
        &pool,
        &company_id,
        UnitRequest {
            symbol: "PCS".to_string(),
            formal_name: "Pieces".to_string(),
            unit_type: Some("simple".to_string()),
            base_unit_id: None,
            secondary_unit_id: None,
            conversion_factor: None,
            decimal_precision: 0,
        },
    )
    .await
    .unwrap();

    let grp = InventoryService::create_stock_group(
        &pool,
        &company_id,
        StockGroupRequest {
            name: "Fittings".to_string(),
            parent_id: None,
            description: None,
        },
    )
    .await
    .unwrap();

    let item = InventoryService::create_stock_item(
        &pool,
        &company_id,
        StockItemRequest {
            name: "Copper Fitting 25mm".to_string(),
            stock_group_id: grp.id.clone(),
            stock_category_id: None,
            base_unit_id: unit.id.clone(),
            alternate_unit_id: None,
            sku: Some("CU-25".to_string()),
            description: None,
            opening_quantity: "0".to_string(),
            opening_rate: "0".to_string(),
            opening_value: "0".to_string(),
            reorder_level: "0".to_string(),
        },
    )
    .await
    .unwrap();

    // Create stock voucher and line to satisfy foreign keys
    let init_sv_id = Uuid::new_v4().to_string();
    let init_sv_line_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO stock_vouchers (id, company_id, voucher_number, voucher_type, voucher_date, narration, status)
         VALUES (?, ?, 'INIT-001', 'receipt', '2025-05-01', 'Initial stock seed', 'posted')",
    )
    .bind(&init_sv_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO stock_voucher_lines (id, company_id, voucher_id, line_number, stock_item_id, unit_id, destination_godown_id, quantity, description)
         VALUES (?, ?, ?, 1, ?, ?, ?, '100', 'Initial receipt')",
    )
    .bind(&init_sv_line_id)
    .bind(&company_id)
    .bind(&init_sv_id)
    .bind(&item.id)
    .bind(&unit.id)
    .bind(&godown_a.id)
    .execute(&pool)
    .await
    .unwrap();

    // Directly seed 100 units @ 150.00 in Central Godown (A) via cost layer and movement
    let layer_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO inventory_cost_layers (id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id, batch_id, receipt_date, original_quantity, remaining_quantity, unit_cost, original_value)
         VALUES (?, ?, ?, ?, ?, ?, NULL, '2025-05-01', '100', '100', '150.00', '15000.00')",
    )
    .bind(&layer_id)
    .bind(&company_id)
    .bind(&item.id)
    .bind(&godown_a.id)
    .bind(&init_sv_id)
    .bind(&init_sv_line_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO stock_movements (id, company_id, movement_date, stock_item_id, godown_id, batch_id, signed_quantity, movement_type, voucher_id, voucher_line_id)
         VALUES (?, ?, '2025-05-01', ?, ?, NULL, '100', 'receipt', ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&item.id)
    .bind(&godown_a.id)
    .bind(&init_sv_id)
    .bind(&init_sv_line_id)
    .execute(&pool)
    .await
    .unwrap();

    // Check Godown Summary before transfer
    let summary_a_before = StockJournalService::get_godown_summary_report(
        &pool,
        &company_id,
        &godown_a.id,
        "2025-05-01",
    )
    .await
    .unwrap();

    assert_eq!(summary_a_before.total_godown_value, "15000");
    assert_eq!(summary_a_before.items.len(), 1);
    assert_eq!(summary_a_before.items[0].closing_quantity, "100");

    // Execute Stock Journal Transfer: 40 units from Godown A to Godown B
    let sj_req = StockJournalRequest {
        voucher_number: Some("STK-JRN-001".to_string()),
        series_id: None,
        voucher_date: "2025-05-05".to_string(),
        narration: Some("Inter-godown transfer of 40 units".to_string()),
        source_lines: vec![StockJournalLine {
            stock_item_id: item.id.clone(),
            godown_id: godown_a.id.clone(),
            batch_number: None,
            quantity: Decimal::new(40, 0),
            rate: Decimal::new(150, 0),
            amount: Decimal::new(6000, 0),
        }],
        destination_lines: vec![StockJournalLine {
            stock_item_id: item.id.clone(),
            godown_id: godown_b.id.clone(),
            batch_number: None,
            quantity: Decimal::new(40, 0),
            rate: Decimal::new(150, 0),
            amount: Decimal::new(6000, 0),
        }],
    };

    let post_res = StockJournalService::post_stock_journal(&pool, &company_id, sj_req)
        .await
        .unwrap();
    assert_eq!(
        Decimal::from_str(&post_res.total_source_value).unwrap(),
        Decimal::new(6000, 0)
    );
    assert_eq!(
        Decimal::from_str(&post_res.total_destination_value).unwrap(),
        Decimal::new(6000, 0)
    );

    // Check Godown Summary after transfer
    let summary_a_after = StockJournalService::get_godown_summary_report(
        &pool,
        &company_id,
        &godown_a.id,
        "2025-05-06",
    )
    .await
    .unwrap();

    let summary_b_after = StockJournalService::get_godown_summary_report(
        &pool,
        &company_id,
        &godown_b.id,
        "2025-05-06",
    )
    .await
    .unwrap();

    assert_eq!(summary_a_after.total_godown_value, "9000");
    assert_eq!(summary_a_after.items[0].closing_quantity, "60");

    assert_eq!(summary_b_after.total_godown_value, "6000");
    assert_eq!(summary_b_after.items[0].closing_quantity, "40");

    // Total inventory value invariance: 9000 + 6000 == 15000
    let total_val_after = Decimal::from_str(&summary_a_after.total_godown_value).unwrap()
        + Decimal::from_str(&summary_b_after.total_godown_value).unwrap();
    assert_eq!(total_val_after, Decimal::new(15000, 0));
}

#[tokio::test]
async fn test_cost_centres_allocation_and_breakup_report() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // 1. Create Cost Category
    let cat_req = CostCategoryRequest {
        name: "Projects".to_string(),
        allocate_revenue: true,
        allocate_non_revenue: false,
    };
    let cat = CostCentreService::create_cost_category(&pool, &company_id, cat_req)
        .await
        .unwrap();
    assert_eq!(cat.name, "Projects");
    assert!(cat.allocate_revenue);

    // 2. Create Cost Centres under this category
    let cc_alpha = CostCentreService::create_cost_centre(
        &pool,
        &company_id,
        CostCentreRequest {
            category_id: cat.id.clone(),
            name: "Project Alpha".to_string(),
            parent_id: None,
        },
    )
    .await
    .unwrap();

    let cc_beta = CostCentreService::create_cost_centre(
        &pool,
        &company_id,
        CostCentreRequest {
            category_id: cat.id.clone(),
            name: "Project Beta".to_string(),
            parent_id: None,
        },
    )
    .await
    .unwrap();

    // 3. Test Cost Allocation balancing enforcement
    let dummy_voucher_id = Uuid::new_v4().to_string();
    let dummy_line_id = Uuid::new_v4().to_string();

    // Create a voucher and line to satisfy foreign keys
    let series_id = "s_jnl_test".to_string();
    sqlx::query(
        "INSERT OR IGNORE INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES (?, ?, 'vt_jnl', 'Journal Series', 'JNL', 'manual', 'never')",
    )
    .bind(&series_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    let voucher_type_id = "vt_jnl".to_string();

    let ledger_id =
        sqlx::query_scalar::<_, String>("SELECT id FROM ledgers WHERE company_id = ? LIMIT 1")
            .bind(&company_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, numbering_method, total_amount)
         VALUES (?, ?, ?, ?, 'VCH-CC-001', '2025-05-10', '2025-05-10', 'draft', 'manual', '10000.00')",
    )
    .bind(&dummy_voucher_id)
    .bind(&company_id)
    .bind(&voucher_type_id)
    .bind(&series_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, created_at)
         VALUES (?, ?, ?, 1, ?, 'Dr', '10000.00', CURRENT_TIMESTAMP)",
    )
    .bind(&dummy_line_id)
    .bind(&company_id)
    .bind(&dummy_voucher_id)
    .bind(&ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("UPDATE vouchers SET status = 'posted' WHERE id = ?")
        .bind(&dummy_voucher_id)
        .execute(&pool)
        .await
        .unwrap();

    // Sum is 4000 + 6000 = 10000. Line amount is 10000. Should succeed.
    let balanced_allocations = vec![
        VoucherCostAllocationInput {
            cost_centre_id: cc_alpha.id.clone(),
            amount: Decimal::new(4000, 0),
        },
        VoucherCostAllocationInput {
            cost_centre_id: cc_beta.id.clone(),
            amount: Decimal::new(6000, 0),
        },
    ];

    let mut conn = pool.acquire().await.unwrap();
    let save_ok = CostCentreService::save_voucher_cost_allocations(
        &mut conn,
        &company_id,
        &dummy_voucher_id,
        &dummy_line_id,
        Decimal::new(10000, 0),
        &balanced_allocations,
    )
    .await;
    assert!(save_ok.is_ok(), "Balanced allocations must succeed");

    // Imbalanced allocations (4000 + 5000 != 10000) must fail
    let imbalanced_allocations = vec![
        VoucherCostAllocationInput {
            cost_centre_id: cc_alpha.id.clone(),
            amount: Decimal::new(4000, 0),
        },
        VoucherCostAllocationInput {
            cost_centre_id: cc_beta.id.clone(),
            amount: Decimal::new(5000, 0),
        },
    ];

    let mut conn2 = pool.acquire().await.unwrap();
    let save_err = CostCentreService::save_voucher_cost_allocations(
        &mut conn2,
        &company_id,
        &dummy_voucher_id,
        &dummy_line_id,
        Decimal::new(10000, 0),
        &imbalanced_allocations,
    )
    .await;
    assert!(
        save_err.is_err(),
        "Imbalanced cost allocations must be rejected"
    );

    // 4. Test Cost Centre Breakup Report
    let breakup_rep = CostCentreService::get_cost_centre_breakup_report(
        &pool,
        &company_id,
        None,
        None,
        Some(&cat.id),
        None,
    )
    .await
    .unwrap();

    assert_eq!(breakup_rep.centres.len(), 2);
    let row_alpha = breakup_rep
        .centres
        .iter()
        .find(|r| r.cost_centre_id == cc_alpha.id)
        .unwrap();
    assert_eq!(
        Decimal::from_str(&row_alpha.debit_total).unwrap(),
        Decimal::new(4000, 0)
    );

    let row_beta = breakup_rep
        .centres
        .iter()
        .find(|r| r.cost_centre_id == cc_beta.id)
        .unwrap();
    assert_eq!(
        Decimal::from_str(&row_beta.debit_total).unwrap(),
        Decimal::new(6000, 0)
    );
}

#[tokio::test]
async fn test_multi_company_tenant_isolation() {
    let (_dir, pool, company_a) = setup_test_pool().await;

    // Create Company B
    let comp_b = CompanyService::create_company(
        &pool,
        CreateCompanyRequest {
            name: "Mumbai Spares Ltd".to_string(),
            mailing_name: Some("Mumbai Spares Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Maharashtra".to_string()),
            financial_year_begins: "2025-04-01".to_string(),
            books_beginning_from: Some("2025-04-01".to_string()),
            currency_symbol: Some("INR".to_string()),
            currency_formal_name: Some("Indian Rupee".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();
    let company_b = comp_b.id;

    // Create Godown in Company A
    let godown_a = InventoryService::create_godown(
        &pool,
        &company_a,
        GodownRequest {
            name: "Bengaluru Central".to_string(),
            parent_id: None,
            description: None,
            address: Some("MG Road, Bengaluru".to_string()),
            is_external: false,
        },
    )
    .await
    .unwrap();

    // Verify Company B cannot see Company A's godown
    let godowns_b = InventoryService::list_godowns(&pool, &company_b, None)
        .await
        .unwrap();
    assert!(
        godowns_b.iter().all(|g| g.id != godown_a.id),
        "Company B must not see Company A godowns"
    );

    // Create Cost Category in Company A
    let cat_a = CostCentreService::create_cost_category(
        &pool,
        &company_a,
        CostCategoryRequest {
            name: "Internal Projects".to_string(),
            allocate_revenue: true,
            allocate_non_revenue: false,
        },
    )
    .await
    .unwrap();

    // Verify Company B cannot see Company A's cost category
    let cats_b = CostCentreService::list_cost_categories(&pool, &company_b)
        .await
        .unwrap();
    assert!(
        cats_b.iter().all(|c| c.id != cat_a.id),
        "Company B must not see Company A cost categories"
    );
}
