use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::purchase::{
    PurchaseDiscountKind, PurchaseDocumentRequest, PurchaseDocumentService, PurchaseLineRequest,
};
use crate::reports::{
    BalanceSheetRequest, BalanceSheetService, ProfitAndLossRequest, ProfitAndLossService,
    ReportDateRange, TrialBalanceRequest, TrialBalanceService,
};
use crate::sales::{DiscountKind, SalesDocumentRequest, SalesDocumentService, SalesLineRequest};
use crate::vouchers::{
    CancelVoucherRequest, ReverseVoucherRequest, VoucherCancellationService, VoucherReversalService,
};
use rust_decimal::Decimal;
use std::str::FromStr;

fn billed_purchase_request(
    actual: &str,
    billed: Option<&str>,
    rate: &str,
) -> PurchaseDocumentRequest {
    serde_json::from_value(serde_json::json!({
        "series_id":"series_pur_a", "party_id":"party_supp_a", "bill_date":"2026-04-10",
        "document_discount_amount":"0", "additional_charges":"0", "lines":[{
            "description":"Actual/billed inventory", "stock_item_id":"item_widget", "unit_id":"unit_pcs",
            "godown_id":"godown_main", "quantity":actual, "billed_quantity":billed, "rate":rate,
            "discount_kind":"none", "discount_value":"0"
        }]
    })).unwrap()
}

fn billed_sales_request(actual: &str, billed: Option<&str>, rate: &str) -> SalesDocumentRequest {
    serde_json::from_value(serde_json::json!({
        "series_id":"series_sal_a", "party_id":"party_cust_a", "invoice_date":"2026-04-15",
        "document_discount_amount":"0", "additional_charges":"0", "lines":[{
            "description":"Actual/billed inventory", "stock_item_id":"item_widget", "unit_id":"unit_pcs",
            "godown_id":"godown_main", "quantity":actual, "billed_quantity":billed, "rate":rate,
            "discount_kind":"none", "discount_value":"0"
        }]
    })).unwrap()
}

#[tokio::test]
async fn test_actual_billed_purchase_draft_roundtrip_and_update() {
    let (_dir, db) = setup_fixture().await;
    let created = PurchaseDocumentService::create_draft(
        db.pool(),
        "cmp_a",
        billed_purchase_request("11", Some("10"), "55"),
    )
    .await
    .unwrap();
    assert_eq!(created.document.net_total, "550");
    let loaded = PurchaseDocumentService::get_draft(db.pool(), "cmp_a", &created.document.id)
        .await
        .unwrap();
    assert_eq!(loaded.lines[0].quantity, "11");
    assert_eq!(
        serde_json::to_value(&loaded.lines[0]).unwrap()["billed_quantity"],
        "10"
    );
    let updated = PurchaseDocumentService::update_draft(
        db.pool(),
        "cmp_a",
        &created.document.id,
        billed_purchase_request("11", Some("9"), "55"),
    )
    .await
    .unwrap();
    assert_eq!(updated.document.net_total, "495");
    assert_eq!(
        serde_json::to_value(&updated.lines[0]).unwrap()["billed_quantity"],
        "9"
    );
}

#[tokio::test]
async fn test_actual_billed_sales_draft_roundtrip_and_update() {
    let (_dir, db) = setup_fixture().await;
    let created = SalesDocumentService::create_draft(
        db.pool(),
        "cmp_a",
        billed_sales_request("11", Some("10"), "100"),
    )
    .await
    .unwrap();
    assert_eq!(created.document.net_total, "1000");
    let loaded = SalesDocumentService::get_draft(db.pool(), "cmp_a", &created.document.id)
        .await
        .unwrap();
    assert_eq!(loaded.lines[0].quantity, "11");
    assert_eq!(
        serde_json::to_value(&loaded.lines[0]).unwrap()["billed_quantity"],
        "10"
    );
    let updated = SalesDocumentService::update_draft(
        db.pool(),
        "cmp_a",
        &created.document.id,
        billed_sales_request("11", Some("9"), "100"),
    )
    .await
    .unwrap();
    assert_eq!(updated.document.net_total, "900");
}

#[tokio::test]
async fn test_actual_billed_posting_finance_uses_billed_stock_uses_actual() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();
    let purchase = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        billed_purchase_request("22", Some("20"), "55"),
    )
    .await
    .unwrap();
    let receipt = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_a",
        &purchase.document.id,
        Some("led_purchase_a".into()),
    )
    .await
    .unwrap();
    assert!(receipt.voucher.is_balanced);
    let layer: (String, String, String) = sqlx::query_as("SELECT remaining_quantity, unit_cost, original_value FROM inventory_cost_layers WHERE id=?")
        .bind(&receipt.cost_layer_ids[0]).fetch_one(pool).await.unwrap();
    assert_eq!(layer, ("22".into(), "50".into(), "1100".into()));
    let sale = SalesDocumentService::create_draft(
        pool,
        "cmp_a",
        billed_sales_request("11", Some("10"), "100"),
    )
    .await
    .unwrap();
    let dispatch = SalesDocumentService::post_sales_document(
        pool,
        "cmp_a",
        &sale.document.id,
        Some("led_sales_a".into()),
    )
    .await
    .unwrap();
    assert!(dispatch.voucher.is_balanced);
    assert_eq!(dispatch.cogs_amount, Some(Decimal::new(550, 0)));
    let remaining: String =
        sqlx::query_scalar("SELECT remaining_quantity FROM inventory_cost_layers WHERE id=?")
            .bind(&receipt.cost_layer_ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(remaining, "11");
    let customer_amount: String =
        sqlx::query_scalar("SELECT amount FROM voucher_lines WHERE voucher_id=? AND line_number=1")
            .bind(&dispatch.voucher.voucher.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(customer_amount, "1000");
}

#[tokio::test]
async fn test_actual_billed_rejects_invalid_billed_quantities_before_writing() {
    let (_dir, db) = setup_fixture().await;
    for billed in ["-1", "12", "NaN", "1e3"] {
        assert!(PurchaseDocumentService::create_draft(
            db.pool(),
            "cmp_a",
            billed_purchase_request("11", Some(billed), "55")
        )
        .await
        .is_err());
        assert!(SalesDocumentService::create_draft(
            db.pool(),
            "cmp_a",
            billed_sales_request("11", Some(billed), "55")
        )
        .await
        .is_err());
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sales_documents")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

async fn setup_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("commercial_test.db"))
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

    seed_fixture(p).await;
    (dir, db)
}

async fn seed_fixture(p: &sqlx::Pool<sqlx::Sqlite>) {
    // 1. Companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_a', 'Company Alpha', 'Alpha Corp', 'India', 'Delhi', '2026-04-01', '2026-04-01'),
                ('cmp_b', 'Company Beta', 'Beta Corp', 'India', 'Maharashtra', '2026-04-01', '2026-04-01')"
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
         VALUES ('led_cust_a', 'cmp_a', 'grp_ca_a', 'Customer Ledger', 0, 'standard'),
                ('led_supp_a', 'cmp_a', 'grp_cl_a', 'Supplier Ledger', 0, 'standard'),
                ('led_sales_a', 'cmp_a', 'grp_inc_a', 'Sales Account', 0, 'standard'),
                ('led_purchase_a', 'cmp_a', 'grp_exp_a', 'Purchase Account', 0, 'standard'),
                ('led_asset_a', 'cmp_a', 'grp_ca_a', 'Stock-in-Hand', 0, 'standard'),
                ('led_cogs_a', 'cmp_a', 'grp_dexp_a', 'Cost of Goods Sold', 0, 'standard'),
                ('led_adj_exp_a', 'cmp_a', 'grp_dexp_a', 'Stock Adjustment Expense', 0, 'standard'),
                ('led_adj_gain_a', 'cmp_a', 'grp_inc_a', 'Stock Adjustment Gain', 0, 'standard'),
                ('led_offset_a', 'cmp_a', 'grp_ca_a', 'Stock Receipt Offset', 0, 'standard'),
                ('led_cust_b', 'cmp_b', 'grp_ca_b', 'Customer Ledger B', 0, 'standard')",
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

    sqlx::query("INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision) VALUES ('unit_pcs', 'cmp_a', 'PCS', 'Pieces', 'simple', 2)")
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
         VALUES ('item_widget', 'cmp_a', 'Widget Pro', 'sg_a', 'unit_pcs', '0', '0', '0', '0'),
                ('item_empty', 'cmp_a', 'Empty Stock Item', 'sg_a', 'unit_pcs', '0', '0', '0', '0')"
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
}

#[tokio::test]
async fn test_commercial_e2e_purchase_and_sales_posting_with_fifo_and_cogs() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // Step 1: Create F9 Purchase Draft: 10 PCS @ 50.00 = 500.00
    let purchase_req = PurchaseDocumentRequest {
        series_id: "series_pur_a".to_string(),
        party_id: "party_supp_a".to_string(),
        bill_date: "2026-04-10".to_string(),
        supplier_invoice_no: Some("INV-SUPP-99".to_string()),
        due_date_override: None,
        due_date_override_reason: None,
        narration: Some("Bulk inventory purchase".to_string()),
        document_discount_amount: "0".to_string(),
        additional_charges: "0".to_string(),
        lines: vec![PurchaseLineRequest {
            description: "Widget Pro Initial Stock".to_string(),
            stock_item_id: Some("item_widget".to_string()),
            unit_id: Some("unit_pcs".to_string()),
            godown_id: Some("godown_main".to_string()),
            quantity: "10".to_string(),
            billed_quantity: None,
            rate: "50".to_string(),
            discount_kind: PurchaseDiscountKind::None,
            discount_value: "0".to_string(),
        }],
    };

    let p_draft = PurchaseDocumentService::create_draft(pool, "cmp_a", purchase_req)
        .await
        .expect("Failed to create purchase draft");
    assert_eq!(p_draft.document.net_total, "500");

    // Step 2: Post Purchase Document
    let p_res = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_a",
        &p_draft.document.id,
        Some("led_purchase_a".to_string()),
    )
    .await
    .expect("Failed to post purchase document");

    // Verify Purchase GL Voucher
    assert!(p_res.voucher.is_balanced);
    assert_eq!(p_res.voucher.lines.len(), 2);
    // Dr Purchase Account 500, Cr Supplier 500
    assert_eq!(p_res.cost_layer_ids.len(), 1);
    assert!(p_res.stock_voucher_id.is_some());

    // Verify Purchase Document Status
    let p_posted = PurchaseDocumentService::get_draft(pool, "cmp_a", &p_draft.document.id)
        .await
        .unwrap();
    assert_eq!(p_posted.document.status, "posted");
    assert_eq!(p_posted.document.voucher_id, p_res.voucher.voucher.id);

    // Verify FIFO Cost Layer created
    let layer: (String, String, String) = sqlx::query_as(
        "SELECT remaining_quantity, unit_cost, original_value FROM inventory_cost_layers WHERE id = ?",
    )
    .bind(&p_res.cost_layer_ids[0])
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(layer.0, "10");
    assert_eq!(layer.1, "50");
    assert_eq!(layer.2, "500");

    // Step 3: Create F8 Sales Draft: 6 PCS @ 100.00 = 600.00
    let sales_req = SalesDocumentRequest {
        series_id: "series_sal_a".to_string(),
        party_id: "party_cust_a".to_string(),
        invoice_date: "2026-04-15".to_string(),
        document_reference: Some("SO-001".to_string()),
        due_date_override: None,
        due_date_override_reason: None,
        narration: Some("First commercial sale".to_string()),
        document_discount_amount: "0".to_string(),
        additional_charges: "0".to_string(),
        lines: vec![SalesLineRequest {
            description: "Widget Pro Commercial Sale".to_string(),
            stock_item_id: Some("item_widget".to_string()),
            unit_id: Some("unit_pcs".to_string()),
            godown_id: Some("godown_main".to_string()),
            quantity: "6".to_string(),
            billed_quantity: None,
            rate: "100".to_string(),
            discount_kind: DiscountKind::None,
            discount_value: "0".to_string(),
        }],
    };

    let s_draft = SalesDocumentService::create_draft(pool, "cmp_a", sales_req)
        .await
        .expect("Failed to create sales draft");
    assert_eq!(s_draft.document.net_total, "600");

    // Step 4: Post Sales Document
    let s_res = SalesDocumentService::post_sales_document(
        pool,
        "cmp_a",
        &s_draft.document.id,
        Some("led_sales_a".to_string()),
    )
    .await
    .expect("Failed to post sales document");

    // Verify Sales GL Voucher
    assert!(s_res.voucher.is_balanced);
    // Lines:
    // Line 1: Dr Customer 600
    // Line 2: Cr Sales 600
    // Line 3: Dr COGS 300 (6 * 50)
    // Line 4: Cr Inventory Asset 300
    assert_eq!(s_res.voucher.lines.len(), 4);
    assert_eq!(s_res.cogs_amount, Some(Decimal::from_str("300").unwrap()));
    assert_eq!(s_res.consumed_layer_ids.len(), 1);

    // Verify FIFO Cost Layer remaining quantity is now 4
    let rem_qty: String =
        sqlx::query_scalar("SELECT remaining_quantity FROM inventory_cost_layers WHERE id = ?")
            .bind(&p_res.cost_layer_ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(rem_qty, "4");

    // Step 5: Verify Financial Statement Equilibrium
    let period = ReportDateRange {
        date_from: "2026-04-01".to_string(),
        date_to: "2026-04-30".to_string(),
    };

    // Trial Balance
    let tb = TrialBalanceService::generate(
        pool,
        "cmp_a",
        TrialBalanceRequest {
            as_of_date: "2026-04-30".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Trial balance generation failed");

    assert_eq!(Decimal::from_str(&tb.difference).unwrap(), Decimal::ZERO);
    assert_eq!(tb.total_debit, tb.total_credit);

    // Profit & Loss Report
    let pnl = ProfitAndLossService::generate(
        pool,
        "cmp_a",
        ProfitAndLossRequest {
            period: period.clone(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Profit & Loss generation failed");

    // Revenue = 600, COGS = 300 (Direct Expense), Purchase = 500 (Expense) -> Net result calculated correctly
    assert!(!pnl.net_profit_or_loss.is_empty());

    // Balance Sheet Report
    let bs = BalanceSheetService::generate(
        pool,
        "cmp_a",
        BalanceSheetRequest {
            as_of_date: "2026-04-30".to_string(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Balance Sheet generation failed");

    assert!(bs.is_balanced);
    assert_eq!(
        Decimal::from_str(&bs.balance_difference).unwrap(),
        Decimal::ZERO
    );
}

#[tokio::test]
async fn test_commercial_insufficient_stock_atomic_rollback() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // Attempt to post Sales draft for an item with 0 stock
    let sales_req = SalesDocumentRequest {
        series_id: "series_sal_a".to_string(),
        party_id: "party_cust_a".to_string(),
        invoice_date: "2026-04-15".to_string(),
        document_reference: Some("SO-FAIL".to_string()),
        due_date_override: None,
        due_date_override_reason: None,
        narration: Some("Should fail due to insufficient stock".to_string()),
        document_discount_amount: "0".to_string(),
        additional_charges: "0".to_string(),
        lines: vec![SalesLineRequest {
            description: "Item with zero stock".to_string(),
            stock_item_id: Some("item_empty".to_string()),
            unit_id: Some("unit_pcs".to_string()),
            godown_id: Some("godown_main".to_string()),
            quantity: "5".to_string(),
            billed_quantity: None,
            rate: "100".to_string(),
            discount_kind: DiscountKind::None,
            discount_value: "0".to_string(),
        }],
    };

    let s_draft = SalesDocumentService::create_draft(pool, "cmp_a", sales_req)
        .await
        .unwrap();

    let err = SalesDocumentService::post_sales_document(
        pool,
        "cmp_a",
        &s_draft.document.id,
        Some("led_sales_a".to_string()),
    )
    .await
    .expect_err("Expected post_sales_document to fail due to insufficient stock");

    match err {
        OpenTallyError::Validation(msg) => {
            assert!(
                msg.contains("Insufficient stock"),
                "Expected insufficient stock validation error, got: {}",
                msg
            );
        }
        other => panic!("Unexpected error variant: {:?}", other),
    }

    // Verify 0 posted vouchers and 0 stock vouchers created
    let v_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id = 'cmp_a' AND status = 'posted'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(v_count, 0, "No posted vouchers should have been created");

    let sv_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM stock_vouchers WHERE company_id = 'cmp_a'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(sv_count, 0, "No stock vouchers should have been created");

    // Verify sales document remains in draft status
    let draft_check = SalesDocumentService::get_draft(pool, "cmp_a", &s_draft.document.id)
        .await
        .unwrap();
    assert_eq!(draft_check.document.status, "draft");
}

#[tokio::test]
async fn test_commercial_purchase_cancellation_fifo_guardrail() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Purchase 10 PCS @ 50
    let p_draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        PurchaseDocumentRequest {
            series_id: "series_pur_a".to_string(),
            party_id: "party_supp_a".to_string(),
            bill_date: "2026-04-10".to_string(),
            supplier_invoice_no: Some("PUR-FIFO-1".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                description: "Purchase Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "10".to_string(),
                billed_quantity: None,
                rate: "50".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let p_res = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_a",
        &p_draft.document.id,
        Some("led_purchase_a".to_string()),
    )
    .await
    .unwrap();

    // 2. Sale of 4 PCS @ 100 (consumes 4 PCS from Purchase layer)
    let s_draft = SalesDocumentService::create_draft(
        pool,
        "cmp_a",
        SalesDocumentRequest {
            series_id: "series_sal_a".to_string(),
            party_id: "party_cust_a".to_string(),
            invoice_date: "2026-04-12".to_string(),
            document_reference: Some("SAL-FIFO-1".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![SalesLineRequest {
                description: "Sales Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "4".to_string(),
                billed_quantity: None,
                rate: "100".to_string(),
                discount_kind: DiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let _s_res = SalesDocumentService::post_sales_document(
        pool,
        "cmp_a",
        &s_draft.document.id,
        Some("led_sales_a".to_string()),
    )
    .await
    .unwrap();

    // 3. Attempt to cancel the F9 Purchase voucher -> FIFO guardrail must reject!
    let cancel_err = VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: p_res.voucher.voucher.id.clone(),
            reason: "Trying to cancel consumed purchase".to_string(),
            actor_id: Some("tester".to_string()),
        },
    )
    .await
    .expect_err("Expected cancellation to fail due to FIFO consumption");

    match cancel_err {
        OpenTallyError::Validation(msg) => {
            assert!(
                msg.contains("consumed by subsequent sale"),
                "Error message should mention FIFO consumption, got: {}",
                msg
            );
        }
        other => panic!("Unexpected error variant: {:?}", other),
    }

    // 4. Attempt to reverse the F9 Purchase voucher -> FIFO guardrail must also reject!
    let reverse_err = VoucherReversalService::reverse_voucher(
        pool,
        "cmp_a",
        ReverseVoucherRequest {
            original_voucher_id: p_res.voucher.voucher.id.clone(),
            reversal_date: "2026-04-20".to_string(),
            reason: "Trying to reverse consumed purchase".to_string(),
            actor_id: Some("tester".to_string()),
        },
    )
    .await
    .expect_err("Expected reversal to fail due to FIFO consumption");

    match reverse_err {
        OpenTallyError::Validation(msg) => {
            assert!(
                msg.contains("consumed by subsequent sale"),
                "Error message should mention FIFO consumption, got: {}",
                msg
            );
        }
        other => panic!("Unexpected error variant: {:?}", other),
    }
}

#[tokio::test]
async fn test_commercial_unconsumed_purchase_cancellation_and_sales_cancellation() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // Scenario A: Unconsumed purchase cancellation
    let p_draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        PurchaseDocumentRequest {
            series_id: "series_pur_a".to_string(),
            party_id: "party_supp_a".to_string(),
            bill_date: "2026-04-10".to_string(),
            supplier_invoice_no: Some("PUR-UNCONSUMED".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                description: "Purchase Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "5".to_string(),
                billed_quantity: None,
                rate: "40".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let p_res = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_a",
        &p_draft.document.id,
        Some("led_purchase_a".to_string()),
    )
    .await
    .unwrap();

    // Cancel unconsumed purchase voucher -> should succeed!
    let cancel_res = VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: p_res.voucher.voucher.id.clone(),
            reason: "Cancelling unconsumed purchase".to_string(),
            actor_id: Some("tester".to_string()),
        },
    )
    .await
    .expect("Expected unconsumed purchase cancellation to succeed");

    assert_eq!(cancel_res.voucher.status, "cancelled");

    // Cost layer remaining_quantity is 0
    let rem: String =
        sqlx::query_scalar("SELECT remaining_quantity FROM inventory_cost_layers WHERE id = ?")
            .bind(&p_res.cost_layer_ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(rem, "0");

    // Scenario B: Sale consumes purchase, Sale is cancelled, Purchase can now be cancelled!
    let p2_draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        PurchaseDocumentRequest {
            series_id: "series_pur_a".to_string(),
            party_id: "party_supp_a".to_string(),
            bill_date: "2026-04-11".to_string(),
            supplier_invoice_no: Some("PUR-RESTORE".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                description: "Purchase Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "8".to_string(),
                billed_quantity: None,
                rate: "60".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let p2_res = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_a",
        &p2_draft.document.id,
        Some("led_purchase_a".to_string()),
    )
    .await
    .unwrap();

    let s2_draft = SalesDocumentService::create_draft(
        pool,
        "cmp_a",
        SalesDocumentRequest {
            series_id: "series_sal_a".to_string(),
            party_id: "party_cust_a".to_string(),
            invoice_date: "2026-04-12".to_string(),
            document_reference: Some("SAL-RESTORE".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![SalesLineRequest {
                description: "Sales Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "3".to_string(),
                billed_quantity: None,
                rate: "120".to_string(),
                discount_kind: DiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let s2_res = SalesDocumentService::post_sales_document(
        pool,
        "cmp_a",
        &s2_draft.document.id,
        Some("led_sales_a".to_string()),
    )
    .await
    .unwrap();

    // Verify layer has 5 left
    let rem2: String =
        sqlx::query_scalar("SELECT remaining_quantity FROM inventory_cost_layers WHERE id = ?")
            .bind(&p2_res.cost_layer_ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(rem2, "5");

    // Cancel the Sales Voucher
    let s_cancel_res = VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: s2_res.voucher.voucher.id.clone(),
            reason: "Cancelling sales voucher".to_string(),
            actor_id: Some("tester".to_string()),
        },
    )
    .await
    .expect("Expected sales cancellation to succeed");
    assert_eq!(s_cancel_res.voucher.status, "cancelled");

    // The consumed 3 units should now be restored to the layer (5 + 3 = 8)
    let rem2_restored: String =
        sqlx::query_scalar("SELECT remaining_quantity FROM inventory_cost_layers WHERE id = ?")
            .bind(&p2_res.cost_layer_ids[0])
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(rem2_restored, "8");

    // Now purchase voucher can be cancelled without error
    let p2_cancel_res = VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: p2_res.voucher.voucher.id.clone(),
            reason: "Now cancelling restored purchase".to_string(),
            actor_id: Some("tester".to_string()),
        },
    )
    .await
    .expect("Expected purchase cancellation to succeed after sale cancellation");
    assert_eq!(p2_cancel_res.voucher.status, "cancelled");
}

#[tokio::test]
async fn test_commercial_multi_company_isolation() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // Company A creates a purchase draft
    let p_draft_a = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        PurchaseDocumentRequest {
            series_id: "series_pur_a".to_string(),
            party_id: "party_supp_a".to_string(),
            bill_date: "2026-04-10".to_string(),
            supplier_invoice_no: Some("ISOLATE-1".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                description: "Purchase Line".to_string(),
                stock_item_id: Some("item_widget".to_string()),
                unit_id: Some("unit_pcs".to_string()),
                godown_id: Some("godown_main".to_string()),
                quantity: "5".to_string(),
                billed_quantity: None,
                rate: "50".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    // Company B tries to get Company A's draft -> NotFound
    let get_err = PurchaseDocumentService::get_draft(pool, "cmp_b", &p_draft_a.document.id)
        .await
        .expect_err("Company B should not see Company A's draft");
    assert!(matches!(get_err, OpenTallyError::NotFound(_)));

    // Company B tries to update Company A's draft -> NotFound
    let update_err = PurchaseDocumentService::update_draft(
        pool,
        "cmp_b",
        &p_draft_a.document.id,
        PurchaseDocumentRequest {
            series_id: "series_pur_b".to_string(),
            party_id: "party_cust_b".to_string(),
            bill_date: "2026-04-10".to_string(),
            supplier_invoice_no: None,
            due_date_override: None,
            due_date_override_reason: None,
            narration: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![],
        },
    )
    .await
    .expect_err("Company B should not update Company A's draft");
    assert!(matches!(update_err, OpenTallyError::NotFound(_)));

    // Company B tries to delete Company A's draft -> NotFound
    let del_err = PurchaseDocumentService::delete_draft(pool, "cmp_b", &p_draft_a.document.id)
        .await
        .expect_err("Company B should not delete Company A's draft");
    assert!(matches!(del_err, OpenTallyError::NotFound(_)));

    // Company B tries to post Company A's draft -> NotFound
    let post_err = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_b",
        &p_draft_a.document.id,
        Some("led_cust_b".to_string()),
    )
    .await
    .expect_err("Company B should not post Company A's draft");
    assert!(matches!(post_err, OpenTallyError::NotFound(_)));
}
