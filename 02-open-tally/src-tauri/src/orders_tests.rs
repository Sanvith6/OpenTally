//! Integration Test Suite for Milestone 13: Order Processing
//!
//! Verifies:
//! 1. Strict Non-GL / Non-Stock Pre-Billing Invariant: Creating Purchase Orders and Sales Orders
//!    creates zero General Ledger entries, zero stock cost layers, zero GST lines, and zero Bill-Wise allocations.
//! 2. Partial vs. Full Fulfillment Lifecycle: Tracking fulfilled quantities per line item, transitioning
//!    order status from Open -> PartiallyFulfilled -> Fulfilled, and rejecting over-fulfillment attempts.
//! 3. Atomic Order-to-Invoice Conversion: Calling `convert_purchase_order_to_invoice` and
//!    `convert_sales_order_to_invoice` within a single `BEGIN IMMEDIATE` transaction, creating real F9/F8
//!    vouchers, posting GL movements, creating/consuming FIFO layers, generating Bill-Wise entries, and chaining audit logs.
//! 4. Cancellation Guardrails & Automatic Stock/Fulfillment Restoration:
//!    - Preventing order cancellation when active invoice fulfillments exist.
//!    - Cancelling a converted commercial invoice automatically rolls back `order_fulfillments` and
//!      restores order line fulfilled quantities and order status.
//! 5. Pending Orders & Overdue Tracking: Outstanding report calculations, remaining pending amounts,
//!    and overdue classification based on delivery dates.
//! 6. Multi-Tenant Isolation & Closed Period Enforcement: Orders are strictly bounded to the authenticated
//!    company, and order creation inside locked financial periods is blocked.

use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::orders::models::{
    ConvertPurchaseOrderInput, ConvertSalesOrderInput, CreateOrderLineInput,
    CreatePurchaseOrderInput, CreateSalesOrderInput, FulfillOrderLineInput,
};
use crate::orders::service::OrdersService;
use crate::reports::orders::OrderOutstandingReportService;
use crate::vouchers::model::CancelVoucherRequest;
use crate::vouchers::service::VoucherCancellationService;
use rust_decimal::Decimal;
use std::str::FromStr;

async fn setup_orders_test_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("opentally_orders_test.db");
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

    // 1. Companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_a', 'Alpha Orders Corp', 'Alpha Corp', 'India', 'Delhi', '2026-04-01', '2026-04-01'),
                ('cmp_b', 'Beta Orders Corp', 'Beta Corp', 'India', 'Maharashtra', '2026-04-01', '2026-04-01')"
    )
    .execute(p)
    .await
    .unwrap();

    // 2. Account Groups
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca_a', 'cmp_a', 'Current Assets', NULL, 'Assets', 1),
                ('grp_cl_a', 'cmp_a', 'Current Liabilities', NULL, 'Liabilities', 1),
                ('grp_inc_a', 'cmp_a', 'Sales Accounts', NULL, 'Income', 1),
                ('grp_exp_a', 'cmp_a', 'Purchase Accounts', NULL, 'Expenses', 1),
                ('grp_dexp_a', 'cmp_a', 'Direct Expenses', NULL, 'Expenses', 1),
                ('grp_ca_b', 'cmp_b', 'Current Assets', NULL, 'Assets', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // 3. Ledgers
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_cust_a', 'cmp_a', 'grp_ca_a', 'Customer Ledger A', 0, 'standard'),
                ('led_supp_a', 'cmp_a', 'grp_cl_a', 'Supplier Ledger A', 0, 'standard'),
                ('led_sales_a', 'cmp_a', 'grp_inc_a', 'Sales Account A', 0, 'standard'),
                ('led_purchase_a', 'cmp_a', 'grp_exp_a', 'Purchase Account A', 0, 'standard'),
                ('led_asset_a', 'cmp_a', 'grp_ca_a', 'Stock-in-Hand A', 0, 'standard'),
                ('led_cogs_a', 'cmp_a', 'grp_dexp_a', 'Cost of Goods Sold A', 0, 'standard'),
                ('led_adj_exp_a', 'cmp_a', 'grp_dexp_a', 'Stock Adjustment Expense A', 0, 'standard'),
                ('led_adj_gain_a', 'cmp_a', 'grp_inc_a', 'Stock Adjustment Gain A', 0, 'standard'),
                ('led_offset_a', 'cmp_a', 'grp_ca_a', 'Stock Receipt Offset A', 0, 'standard'),
                ('led_cust_b', 'cmp_b', 'grp_ca_b', 'Customer Ledger B', 0, 'standard')"
    )
    .execute(p)
    .await
    .unwrap();

    // 4. Party Profiles
    sqlx::query(
        "INSERT INTO party_profiles (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled, credit_period_days, credit_limit)
         VALUES ('party_cust_a', 'cmp_a', 'led_cust_a', 'CUST-001', 'Acme Customer', 1, 0, 30, '100000'),
                ('party_supp_a', 'cmp_a', 'led_supp_a', 'SUPP-001', 'Acme Supplier', 0, 1, 30, '100000'),
                ('party_cust_b', 'cmp_b', 'led_cust_b', 'CUST-B01', 'Beta Customer', 1, 0, 15, '50000')"
    )
    .execute(p)
    .await
    .unwrap();

    // 5. Voucher Number Series & Configs
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('series_sal_a', 'cmp_a', 'vt_sales', 'Sales Series', 'SAL', 'automatic', 'yearly'),
                ('series_pur_a', 'cmp_a', 'vt_pur', 'Purchase Series', 'PUR', 'automatic', 'yearly'),
                ('series_jnl_a', 'cmp_a', 'vt_jnl', 'Journal Series', 'JNL', 'automatic', 'yearly'),
                ('series_rev_jnl_a', 'cmp_a', 'vt_reversing_journal', 'Reversing Journal Series', 'REV', 'automatic', 'yearly'),
                ('series_pur_b', 'cmp_b', 'vt_pur', 'Purchase Series B', 'PURB', 'automatic', 'yearly')"
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_sal_a', 'cmp_a', 'vt_sales', 'series_sal_a', '2026-04-01', 'SAL/2026/', '', 1, 4),
                ('cfg_pur_a', 'cmp_a', 'vt_pur', 'series_pur_a', '2026-04-01', 'PUR/2026/', '', 1, 4),
                ('cfg_jnl_a', 'cmp_a', 'vt_jnl', 'series_jnl_a', '2026-04-01', 'JNL/2026/', '', 1, 4),
                ('cfg_rev_jnl_a', 'cmp_a', 'vt_reversing_journal', 'series_rev_jnl_a', '2026-04-01', 'REV/2026/', '', 1, 4),
                ('cfg_pur_b', 'cmp_b', 'vt_pur', 'series_pur_b', '2026-04-01', 'PURB/2026/', '', 1, 4)"
    )
    .execute(p)
    .await
    .unwrap();

    // 6. Inventory Masters
    sqlx::query(
        "INSERT INTO stock_groups (id, company_id, name) VALUES ('sg_a', 'cmp_a', 'General Goods')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision) VALUES ('unit_pcs', 'cmp_a', 'PCS', 'Pieces', 'simple', 2)"
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO godowns (id, company_id, name) VALUES ('godown_main', 'cmp_a', 'Main Godown')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value, reorder_level)
         VALUES ('item_widget', 'cmp_a', 'Industrial Widget', 'sg_a', 'unit_pcs', '0', '0', '0', '0')"
    )
    .execute(p)
    .await
    .unwrap();

    // 7. Inventory Accounting Mappings
    sqlx::query(
        "INSERT INTO inventory_account_mappings (
            company_id, inventory_asset_ledger_id, adjustment_expense_ledger_id,
            adjustment_gain_ledger_id, cogs_ledger_id, receipt_offset_ledger_id,
            journal_series_id, enabled
         ) VALUES ('cmp_a', 'led_asset_a', 'led_adj_exp_a', 'led_adj_gain_a', 'led_cogs_a', 'led_offset_a', 'series_jnl_a', 1)"
    )
    .execute(p)
    .await
    .unwrap();

    (dir, db)
}

#[tokio::test]
async fn test_orders_strict_non_gl_non_stock_invariant() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // Step 1: Create Purchase Order (20 PCS @ 100.00 = 2000.00)
    let po_lines = vec![CreateOrderLineInput {
        stock_item_id: "item_widget".to_string(),
        godown_id: Some("godown_main".to_string()),
        ordered_qty: Decimal::from_str("20").unwrap(),
        unit_rate: Decimal::from_str("100.00").unwrap(),
        discount_kind: None,
        discount_value: None,
    }];

    let po = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-2026-001".to_string(),
            order_date: "2026-04-02".to_string(),
            due_date: Some("2026-04-15".to_string()),
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: Some("REF-PO-01".to_string()),
            narration: Some("Initial PO batch".to_string()),
            lines: po_lines,
        },
        Some("admin"),
    )
    .await
    .expect("Failed to create PO");

    assert_eq!(po.order.order_number, "PO-2026-001");
    assert_eq!(po.order.status, "Open");
    assert_eq!(po.order.total_amount, "2000.00");
    assert_eq!(po.lines.len(), 1);
    assert_eq!(po.lines[0].fulfilled_qty, "0");

    // Step 2: Create Sales Order (15 PCS @ 150.00 = 2250.00)
    let so_lines = vec![CreateOrderLineInput {
        stock_item_id: "item_widget".to_string(),
        godown_id: Some("godown_main".to_string()),
        ordered_qty: Decimal::from_str("15").unwrap(),
        unit_rate: Decimal::from_str("150.00").unwrap(),
        discount_kind: None,
        discount_value: None,
    }];

    let so = OrdersService::create_sales_order(
        pool,
        "cmp_a",
        CreateSalesOrderInput {
            order_number: "SO-2026-001".to_string(),
            order_date: "2026-04-03".to_string(),
            due_date: Some("2026-04-20".to_string()),
            party_ledger_id: "led_cust_a".to_string(),
            reference_no: Some("REF-SO-01".to_string()),
            narration: Some("Initial SO batch".to_string()),
            lines: so_lines,
        },
        Some("admin"),
    )
    .await
    .expect("Failed to create SO");

    assert_eq!(so.order.order_number, "SO-2026-001");
    assert_eq!(so.order.status, "Open");
    assert_eq!(so.order.total_amount, "2250.00");

    // Step 3: Verify Strict Non-GL / Non-Stock Invariant:
    // Zero rows in accounting and stock ledger tables!
    let voucher_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM vouchers WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        voucher_count.0, 0,
        "Pre-billing orders must create 0 vouchers"
    );

    let line_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM voucher_lines WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        line_count.0, 0,
        "Pre-billing orders must create 0 voucher lines"
    );

    let layer_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM inventory_cost_layers WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        layer_count.0, 0,
        "Pre-billing orders must create 0 FIFO cost layers"
    );

    let bill_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bill_allocations WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        bill_count.0, 0,
        "Pre-billing orders must create 0 bill allocations"
    );

    let gst_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM voucher_gst_details WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        gst_count.0, 0,
        "Pre-billing orders must create 0 voucher GST details"
    );

    // But audit log records exist and have sha256 hashes!
    let audit_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(
        audit_count.0 >= 2,
        "Orders must emit immutable audit log entries"
    );

    let audit_cert =
        crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
            pool, "cmp_a",
        )
        .await
        .unwrap();
    assert!(
        audit_cert.is_valid,
        "Audit trail must be valid with no broken links: {:?}",
        audit_cert.issues
    );
    assert_eq!(audit_cert.tampered_entries, 0);
    assert_eq!(audit_cert.broken_chain_links, 0);
}

#[tokio::test]
async fn test_partial_fulfillment_and_invoice_conversion() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // 1. Create Purchase Order for 10 PCS @ 50.00
    let po = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-2026-002".to_string(),
            order_date: "2026-04-02".to_string(),
            due_date: Some("2026-04-10".to_string()),
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("10").unwrap(),
                unit_rate: Decimal::from_str("50.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    let po_line_id = po.lines[0].id.clone();

    // 2. Partial Conversion: Convert 4 units
    let conv1 = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-05".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po_line_id.clone(),
                fulfill_qty: Decimal::from_str("4").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-INV-101".to_string()),
        },
        Some("admin"),
    )
    .await
    .expect("Partial PO conversion failed");

    assert_eq!(conv1.order_status, "PartiallyFulfilled");

    // Verify PO state in DB
    let reloaded_po = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(reloaded_po.order.status, "PartiallyFulfilled");
    assert_eq!(reloaded_po.lines[0].fulfilled_qty, "4");

    // Verify converted F9 voucher created real GL movements and inventory layer (+4 units)
    let v_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM vouchers WHERE id = ? AND voucher_type_id = 'vt_pur'")
            .bind(&conv1.voucher_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(v_count.0, 1);

    let layer_qty: (String,) = sqlx::query_as(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_a' AND stock_item_id = 'item_widget'"
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        Decimal::from_str(&layer_qty.0).unwrap(),
        Decimal::from_str("4").unwrap()
    );

    // 3. Over-fulfillment rejection: Attempt to convert 7 units (only 6 remaining)
    let overfill_res = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-06".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po_line_id.clone(),
                fulfill_qty: Decimal::from_str("7").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-INV-102".to_string()),
        },
        Some("admin"),
    )
    .await;

    match overfill_res {
        Err(OpenTallyError::Validation(msg)) => {
            assert!(
                msg.contains("remaining on order") || msg.contains("Cannot fulfill"),
                "Expected overfulfillment error, got: {}",
                msg
            );
        }
        other => panic!("Expected OpenTallyError::Validation, got {:?}", other),
    }

    // 4. Remaining conversion: Convert remaining 6 units
    let conv2 = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-07".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po_line_id.clone(),
                fulfill_qty: Decimal::from_str("6").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-INV-103".to_string()),
        },
        Some("admin"),
    )
    .await
    .expect("Remaining PO conversion failed");

    assert_eq!(conv2.order_status, "Fulfilled");

    let final_po = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(final_po.order.status, "Fulfilled");
    assert_eq!(final_po.lines[0].fulfilled_qty, "10");

    // Total stock in hand is now 10 PCS @ 50.00
    let layers: Vec<(String,)> = sqlx::query_as(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_a' AND stock_item_id = 'item_widget'"
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let total_stock: Decimal = layers
        .iter()
        .map(|(q,)| Decimal::from_str(q).unwrap())
        .sum();
    assert_eq!(total_stock, Decimal::from_str("10").unwrap());

    // 5. Sales Order Conversion: 4 units @ 120.00
    let so = OrdersService::create_sales_order(
        pool,
        "cmp_a",
        CreateSalesOrderInput {
            order_number: "SO-2026-002".to_string(),
            order_date: "2026-04-08".to_string(),
            due_date: Some("2026-04-12".to_string()),
            party_ledger_id: "led_cust_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("4").unwrap(),
                unit_rate: Decimal::from_str("120.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    let so_conv = OrdersService::convert_sales_order_to_invoice(
        pool,
        "cmp_a",
        ConvertSalesOrderInput {
            order_id: so.order.id.clone(),
            invoice_date: "2026-04-09".to_string(),
            series_id: "series_sal_a".to_string(),
            commercial_account_ledger_id: Some("led_sales_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: so.lines[0].id.clone(),
                fulfill_qty: Decimal::from_str("4").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: None,
        },
        Some("admin"),
    )
    .await
    .expect("Sales Order conversion failed");

    assert_eq!(so_conv.order_status, "Fulfilled");

    // Remaining stock must now be 6 units (10 - 4 consumed via FIFO)
    let rem_layers: Vec<(String,)> = sqlx::query_as(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = 'cmp_a' AND stock_item_id = 'item_widget'"
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let remaining_stock: Decimal = rem_layers
        .iter()
        .map(|(q,)| Decimal::from_str(q).unwrap())
        .sum();
    assert_eq!(remaining_stock, Decimal::from_str("6").unwrap());
}

#[tokio::test]
async fn test_order_cancellation_guardrails() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // 1. Create Purchase Order
    let po = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-2026-003".to_string(),
            order_date: "2026-04-02".to_string(),
            due_date: Some("2026-04-10".to_string()),
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("10").unwrap(),
                unit_rate: Decimal::from_str("50.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Convert 3 units to invoice
    let conv = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-05".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po.lines[0].id.clone(),
                fulfill_qty: Decimal::from_str("3").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-INV-201".to_string()),
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // 2. Cancellation Guardrail: Cannot cancel order with active fulfillments
    let cancel_order_res = OrdersService::cancel_purchase_order(
        pool,
        "cmp_a",
        &po.order.id,
        "Attempted cancel",
        Some("admin"),
    )
    .await;

    match cancel_order_res {
        Err(OpenTallyError::Validation(msg)) => {
            assert!(
                msg.contains("Cannot cancel order with active fulfillments"),
                "Expected active fulfillments error, got: {}",
                msg
            );
        }
        other => panic!("Expected OpenTallyError::Validation, got {:?}", other),
    }

    // 3. Cancelling the converted invoice restores order line fulfilled_qty and order status
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: conv.voucher_id,
            reason: "Supplier sent wrong specifications".to_string(),
            actor_id: Some("admin".to_string()),
        },
    )
    .await
    .expect("Failed to cancel voucher");

    // Verify order line fulfilled_qty is restored to 0 and status is restored to Open
    let restored_po = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(restored_po.order.status, "Open");
    assert_eq!(restored_po.lines[0].fulfilled_qty, "0");

    // Verify order_fulfillments record is marked is_cancelled = 1
    let is_cancelled: (i64,) =
        sqlx::query_as("SELECT is_cancelled FROM order_fulfillments WHERE order_id = ? LIMIT 1")
            .bind(&po.order.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(is_cancelled.0, 1);

    // 4. Now that active fulfillments are restored/cancelled, order cancellation succeeds!
    OrdersService::cancel_purchase_order(
        pool,
        "cmp_a",
        &po.order.id,
        "Order aborted by supplier agreement",
        Some("admin"),
    )
    .await
    .expect("Order cancellation failed after voucher cancellation");

    let final_cancelled = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(final_cancelled.order.status, "Cancelled");
}

#[tokio::test]
async fn test_pending_orders_outstanding_report_and_overdue() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // 1. Create overdue PO: Delivery date 2026-04-05
    let po = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-OVERDUE-01".to_string(),
            order_date: "2026-04-01".to_string(),
            due_date: Some("2026-04-05".to_string()),
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("10").unwrap(),
                unit_rate: Decimal::from_str("75.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Partially fulfill 3 units
    OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-04".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po.lines[0].id.clone(),
                fulfill_qty: Decimal::from_str("3").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-301".to_string()),
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // 2. Create future SO: Delivery date 2026-04-25
    OrdersService::create_sales_order(
        pool,
        "cmp_a",
        CreateSalesOrderInput {
            order_number: "SO-FUTURE-01".to_string(),
            order_date: "2026-04-08".to_string(),
            due_date: Some("2026-04-25".to_string()),
            party_ledger_id: "led_cust_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("5").unwrap(),
                unit_rate: Decimal::from_str("120.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Query pending purchase orders as of 2026-04-10
    let po_report =
        OrderOutstandingReportService::get_pending_purchase_orders(pool, "cmp_a", "2026-04-10")
            .await
            .expect("Failed to get pending PO report");

    assert_eq!(po_report.items.len(), 1);
    let item = &po_report.items[0];
    assert_eq!(item.order_number, "PO-OVERDUE-01");
    assert_eq!(
        Decimal::from_str(&item.ordered_qty).unwrap(),
        Decimal::from_str("10").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&item.fulfilled_qty).unwrap(),
        Decimal::from_str("3").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&item.pending_qty).unwrap(),
        Decimal::from_str("7").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&item.pending_amount).unwrap(),
        Decimal::from_str("525.00").unwrap()
    ); // 7 * 75.00
    assert!(item.is_overdue, "Expected PO to be marked overdue");
    assert_eq!(
        Decimal::from_str(&po_report.total_pending_amount).unwrap(),
        Decimal::from_str("525.00").unwrap()
    );

    // Query pending sales orders as of 2026-04-10
    let so_report =
        OrderOutstandingReportService::get_pending_sales_orders(pool, "cmp_a", "2026-04-10")
            .await
            .expect("Failed to get pending SO report");

    assert_eq!(so_report.items.len(), 1);
    let so_item = &so_report.items[0];
    assert_eq!(so_item.order_number, "SO-FUTURE-01");
    assert_eq!(
        Decimal::from_str(&so_item.pending_qty).unwrap(),
        Decimal::from_str("5").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&so_item.pending_amount).unwrap(),
        Decimal::from_str("600.00").unwrap()
    ); // 5 * 120.00
    assert!(
        !so_item.is_overdue,
        "Future SO should not be marked overdue"
    );
}

#[tokio::test]
async fn test_orders_multi_company_isolation() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // Create PO in Company Alpha
    let po_a = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-ALPHA-001".to_string(),
            order_date: "2026-04-02".to_string(),
            due_date: None,
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("10").unwrap(),
                unit_rate: Decimal::from_str("50.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Query Company Beta -> should be empty
    let list_b = OrdersService::list_purchase_orders(pool, "cmp_b", None)
        .await
        .unwrap();
    assert_eq!(
        list_b.len(),
        0,
        "Company Beta must not see Company Alpha POs"
    );

    // Fetching Company Alpha's PO using Company Beta's ID must fail
    let get_b_res = OrdersService::get_purchase_order(pool, "cmp_b", &po_a.order.id).await;
    match get_b_res {
        Err(OpenTallyError::NotFound(msg)) => {
            assert!(msg.contains("not found"));
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }

    // Attempting to convert Company Alpha's PO using Company Beta's ID must fail
    let conv_b_res = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_b",
        ConvertPurchaseOrderInput {
            order_id: po_a.order.id.clone(),
            invoice_date: "2026-04-05".to_string(),
            series_id: "series_pur_b".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po_a.lines[0].id.clone(),
                fulfill_qty: Decimal::from_str("5").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("INV-CROSS".to_string()),
        },
        Some("admin"),
    )
    .await;

    match conv_b_res {
        Err(OpenTallyError::NotFound(msg)) => {
            assert!(msg.contains("not found"));
        }
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_orders_period_lock_rejection() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // Lock period for Apr 2026 (2026-04-01 to 2026-04-30)
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES ('vch_lock_close', 'cmp_a', 'vt_jnl', 'series_jnl_a', 'automatic', '2026-04-01', 9999, 'JNL/LOCK/01', '2026-04-30', '2026-04-30', 'posted', '100.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO financial_year_closings (id, company_id, financial_year_label, period_start, period_end, next_fy_start, closing_voucher_id, net_profit_or_loss, is_locked)
         VALUES ('close_fy26_apr', 'cmp_a', 'FY 2026-27 (Apr)', '2026-04-01', '2026-04-30', '2026-05-01', 'vch_lock_close', '0.00', 1)"
    )
    .execute(pool)
    .await
    .unwrap();

    // Try to create PO with date in locked period ("2026-04-15")
    let po_locked_res = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-LOCKED-001".to_string(),
            order_date: "2026-04-15".to_string(),
            due_date: None,
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("5").unwrap(),
                unit_rate: Decimal::from_str("50.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await;

    match po_locked_res {
        Err(OpenTallyError::PeriodLocked(msg)) => {
            assert!(msg.contains("locked financial year period"));
        }
        other => panic!("Expected OpenTallyError::PeriodLocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_order_restoration_on_invoice_reversal() {
    let (_dir, db) = setup_orders_test_fixture().await;
    let pool = db.pool();

    // 1. Create Purchase Order
    let po = OrdersService::create_purchase_order(
        pool,
        "cmp_a",
        CreatePurchaseOrderInput {
            order_number: "PO-REV-001".to_string(),
            order_date: "2026-04-02".to_string(),
            due_date: Some("2026-04-10".to_string()),
            party_ledger_id: "led_supp_a".to_string(),
            reference_no: None,
            narration: None,
            lines: vec![CreateOrderLineInput {
                stock_item_id: "item_widget".to_string(),
                godown_id: Some("godown_main".to_string()),
                ordered_qty: Decimal::from_str("10").unwrap(),
                unit_rate: Decimal::from_str("50.00").unwrap(),
                discount_kind: None,
                discount_value: None,
            }],
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Convert 4 units to invoice
    let conv = OrdersService::convert_purchase_order_to_invoice(
        pool,
        "cmp_a",
        ConvertPurchaseOrderInput {
            order_id: po.order.id.clone(),
            invoice_date: "2026-04-05".to_string(),
            series_id: "series_pur_a".to_string(),
            commercial_account_ledger_id: Some("led_purchase_a".to_string()),
            lines: vec![FulfillOrderLineInput {
                order_line_id: po.lines[0].id.clone(),
                fulfill_qty: Decimal::from_str("4").unwrap(),
                unit_rate: None,
                godown_id: Some("godown_main".to_string()),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: None,
            narration: None,
            reference_no: Some("SUPP-INV-REV".to_string()),
        },
        Some("admin"),
    )
    .await
    .unwrap();

    // Verify order is partially fulfilled
    let partial_po = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(partial_po.order.status, "PartiallyFulfilled");
    assert_eq!(partial_po.lines[0].fulfilled_qty, "4");

    // 2. Reverse the invoice voucher via VoucherReversalService
    use crate::vouchers::model::ReverseVoucherRequest;
    use crate::vouchers::service::VoucherReversalService;

    VoucherReversalService::reverse_voucher(
        pool,
        "cmp_a",
        ReverseVoucherRequest {
            original_voucher_id: conv.voucher_id,
            reversal_date: "2026-04-06".to_string(),
            reason: "Audit reversal of duplicate purchase invoice".to_string(),
            actor_id: Some("admin".to_string()),
        },
    )
    .await
    .expect("Failed to reverse voucher");

    // Verify order line fulfilled_qty is restored to 0 and status is restored to Open
    let restored_po = OrdersService::get_purchase_order(pool, "cmp_a", &po.order.id)
        .await
        .unwrap();
    assert_eq!(restored_po.order.status, "Open");
    assert_eq!(restored_po.lines[0].fulfilled_qty, "0");

    // 3. Verify audit trail integrity after all order events and reversals
    let audit_cert =
        crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
            pool, "cmp_a",
        )
        .await
        .unwrap();
    assert!(
        audit_cert.is_valid,
        "Audit trail must be valid with no broken links: {:?}",
        audit_cert.issues
    );
    assert_eq!(audit_cert.tampered_entries, 0);
    assert_eq!(audit_cert.broken_chain_links, 0);
}
