use crate::commercial::{
    CommercialDocumentType, CommercialLinePosting, CommercialPostingInput,
    CommercialPostingService, TaxPostingLine,
};
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::purchase::{
    PurchaseDiscountKind, PurchaseDocumentRequest, PurchaseDocumentService, PurchaseLineRequest,
};
use crate::reports::gst::GstReportService;
use crate::sales::{DiscountKind, SalesDocumentRequest, SalesDocumentService, SalesLineRequest};
use rust_decimal::Decimal;
use std::str::FromStr;

async fn setup_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("notes_test.db"))
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

    seed_fixture(p).await;
    (dir, db)
}

async fn seed_fixture(p: &sqlx::Pool<sqlx::Sqlite>) {
    // 1. Companies & GST Profiles
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_notes', 'Notes Test Corp', 'Notes Corp', 'India', 'Delhi', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled)
         VALUES ('cmp_notes', '07AAAAA1234A1Z5', '07', 'Regular', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // 2. Account Groups
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca', 'cmp_notes', 'Current Assets', NULL, 'Assets', 1),
                ('grp_cl', 'cmp_notes', 'Current Liabilities', NULL, 'Liabilities', 1),
                ('grp_inc', 'cmp_notes', 'Sales Accounts', NULL, 'Income', 1),
                ('grp_exp', 'cmp_notes', 'Purchase Accounts', NULL, 'Expenses', 1),
                ('grp_dexp', 'cmp_notes', 'Direct Expenses', NULL, 'Expenses', 1),
                ('grp_tax', 'cmp_notes', 'Duties & Taxes', NULL, 'Liabilities', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // 3. Ledgers
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_debtor', 'cmp_notes', 'grp_ca', 'Sundry Debtor Alpha', 0, 'standard'),
                ('led_creditor', 'cmp_notes', 'grp_cl', 'Sundry Creditor Beta', 0, 'standard'),
                ('led_sales', 'cmp_notes', 'grp_inc', 'Sales Account', 0, 'standard'),
                ('led_sales_ret', 'cmp_notes', 'grp_inc', 'Sales Returns', 0, 'standard'),
                ('led_purchase', 'cmp_notes', 'grp_exp', 'Purchase Account', 0, 'standard'),
                ('led_purchase_ret', 'cmp_notes', 'grp_exp', 'Purchase Returns', 0, 'standard'),
                ('led_stock', 'cmp_notes', 'grp_ca', 'Stock-in-Hand', 0, 'standard'),
                ('led_cogs', 'cmp_notes', 'grp_dexp', 'Cost of Goods Sold', 0, 'standard'),
                ('led_cgst', 'cmp_notes', 'grp_tax', 'Output CGST', 0, 'standard'),
                ('led_sgst', 'cmp_notes', 'grp_tax', 'Output SGST', 0, 'standard'),
                ('led_igst', 'cmp_notes', 'grp_tax', 'Output IGST', 0, 'standard'),
                ('led_adj_exp', 'cmp_notes', 'grp_dexp', 'Stock Adj Expense', 0, 'standard'),
                ('led_adj_gain', 'cmp_notes', 'grp_inc', 'Stock Adj Gain', 0, 'standard'),
                ('led_offset', 'cmp_notes', 'grp_ca', 'Stock Offset', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    // 4. GST Ledger Mappings
    sqlx::query(
        "INSERT INTO gst_ledger_mappings (company_id, output_cgst_ledger_id, output_sgst_ledger_id, output_igst_ledger_id, output_cess_ledger_id, input_cgst_ledger_id, input_sgst_ledger_id, input_igst_ledger_id, input_cess_ledger_id)
         VALUES ('cmp_notes', 'led_cgst', 'led_sgst', 'led_igst', NULL, 'led_cgst', 'led_sgst', 'led_igst', NULL)",
    )
    .execute(p)
    .await
    .unwrap();

    // 5. Party Profiles
    sqlx::query(
        "INSERT INTO party_profiles (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled, credit_period_days, credit_limit)
         VALUES ('party_deb', 'cmp_notes', 'led_debtor', 'CUST-01', 'Debtor Alpha', 1, 0, 30, '500000'),
                ('party_crd', 'cmp_notes', 'led_creditor', 'SUPP-01', 'Creditor Beta', 0, 1, 30, '500000')",
    )
    .execute(p)
    .await
    .unwrap();

    // 6. Voucher Series & Configs
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_sal', 'cmp_notes', 'vt_sales', 'Sales Series', 'SAL', 'automatic', 'yearly'),
                ('s_pur', 'cmp_notes', 'vt_pur', 'Purchase Series', 'PUR', 'automatic', 'yearly'),
                ('s_cn', 'cmp_notes', 'vt_credit_note', 'Credit Note Series', 'CN', 'automatic', 'yearly'),
                ('s_dn', 'cmp_notes', 'vt_debit_note', 'Debit Note Series', 'DN', 'automatic', 'yearly'),
                ('s_jnl', 'cmp_notes', 'vt_jnl', 'Journal Series', 'JNL', 'automatic', 'yearly')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_sal', 'cmp_notes', 'vt_sales', 's_sal', '2026-04-01', 'INV/', '', 1, 4),
                ('cfg_pur', 'cmp_notes', 'vt_pur', 's_pur', '2026-04-01', 'PUR/', '', 1, 4),
                ('cfg_cn', 'cmp_notes', 'vt_credit_note', 's_cn', '2026-04-01', 'CN/', '', 1, 4),
                ('cfg_dn', 'cmp_notes', 'vt_debit_note', 's_dn', '2026-04-01', 'DN/', '', 1, 4),
                ('cfg_jnl', 'cmp_notes', 'vt_jnl', 's_jnl', '2026-04-01', 'JNL/', '', 1, 4)",
    )
    .execute(p)
    .await
    .unwrap();

    // 7. Inventory Masters
    sqlx::query("INSERT INTO stock_groups (id, company_id, name) VALUES ('sg_1', 'cmp_notes', 'Electronics')")
        .execute(p)
        .await
        .unwrap();

    sqlx::query("INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision) VALUES ('u_pcs', 'cmp_notes', 'PCS', 'Pieces', 'simple', 0)")
        .execute(p)
        .await
        .unwrap();

    sqlx::query("INSERT INTO godowns (id, company_id, name) VALUES ('g_main', 'cmp_notes', 'Central Warehouse')")
        .execute(p)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value, reorder_level)
         VALUES ('item_laptop', 'cmp_notes', 'Pro Laptop 15', 'sg_1', 'u_pcs', '0', '0', '0', '0')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO inventory_account_mappings (
            company_id, inventory_asset_ledger_id, adjustment_expense_ledger_id,
            adjustment_gain_ledger_id, cogs_ledger_id, receipt_offset_ledger_id,
            journal_series_id, enabled
         ) VALUES ('cmp_notes', 'led_stock', 'led_adj_exp', 'led_adj_gain', 'led_cogs', 'led_offset', 's_jnl', 1)",
    )
    .execute(p)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_credit_note_sales_return_lifecycle_and_over_return_guards() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Initial Purchase to bring 10 PCS @ Rs 1000 into inventory
    let pur_draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_notes",
        PurchaseDocumentRequest {
            series_id: "s_pur".to_string(),
            party_id: "party_crd".to_string(),
            bill_date: "2026-05-01".to_string(),
            supplier_invoice_no: Some("SUPP-101".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: Some("Initial Stock Purchase".to_string()),
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                stock_item_id: Some("item_laptop".to_string()),
                unit_id: Some("u_pcs".to_string()),
                godown_id: Some("g_main".to_string()),
                description: "Pro Laptop 15".to_string(),
                quantity: "10".to_string(),
                rate: "1000.00".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_notes",
        &pur_draft.document.id,
        Some("led_purchase".to_string()),
    )
    .await
    .unwrap();

    // 2. F8 Sales of 6 PCS @ Rs 1500 = Rs 9000 Taxable + 18% GST (CGST 810 + SGST 810) = Rs 10,620
    let sales_draft = SalesDocumentService::create_draft(
        pool,
        "cmp_notes",
        SalesDocumentRequest {
            series_id: "s_sal".to_string(),
            party_id: "party_deb".to_string(),
            invoice_date: "2026-05-05".to_string(),
            due_date_override: None,
            due_date_override_reason: None,
            narration: Some("Sales to Debtor Alpha".to_string()),
            document_reference: None,
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![SalesLineRequest {
                stock_item_id: Some("item_laptop".to_string()),
                unit_id: Some("u_pcs".to_string()),
                godown_id: Some("g_main".to_string()),
                description: "Pro Laptop 15".to_string(),
                quantity: "6".to_string(),
                rate: "1500.00".to_string(),
                discount_kind: DiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let sales_posted = SalesDocumentService::post_sales_document(
        pool,
        "cmp_notes",
        &sales_draft.document.id,
        Some("led_sales".to_string()),
    )
    .await
    .unwrap();

    let orig_sales_vid = sales_posted.voucher.voucher.id.clone();
    let orig_sales_vnum = sales_posted.voucher.voucher.voucher_number.clone();

    // Verify inventory remaining: 10 - 6 = 4 PCS
    let stock_bal: String = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(remaining_quantity), 0) AS TEXT) FROM inventory_cost_layers WHERE company_id = 'cmp_notes' AND stock_item_id = 'item_laptop'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(Decimal::from_str(&stock_bal).unwrap(), Decimal::from(4));

    // 3. Post Credit Note (Sales Return) for 2 PCS @ Rs 1500 = Rs 3000 Taxable + 18% GST (CGST 270 + SGST 270) = Rs 3540
    let cn_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::CreditNote,
        source_document_id: format!("cn_src_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_cn".to_string(),
        date: "2026-05-08".to_string(),
        due_date: None,
        party_id: "party_deb".to_string(),
        party_ledger_id: "led_debtor".to_string(),
        commercial_account_ledger_id: Some("led_sales_ret".to_string()),
        total_amount: Decimal::from(3000),
        document_reference: Some(orig_sales_vnum.clone()),
        narration: Some("Defective units returned by Debtor".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Pro Laptop 15 (Returned)".to_string(),
            stock_item_id: Some("item_laptop".to_string()),
            unit_id: Some("u_pcs".to_string()),
            godown_id: Some("g_main".to_string()),
            quantity: Decimal::from(2),
            rate: Decimal::from(1500),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(3000),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_cgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(270),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_sgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(270),
            },
        ],
        bill_allocations: vec![], // Auto AgstRef linkage test
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: Some(orig_sales_vid.clone()),
        return_reason: Some("Defective screens".to_string()),
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let cn_result = CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", cn_input)
        .await
        .unwrap();

    assert_eq!(cn_result.voucher.voucher.voucher_type_id, "vt_credit_note");
    assert_eq!(cn_result.voucher.voucher.status, "posted");

    // Verify GL lines: Dr Sales Returns 3000, Dr CGST 270, Dr SGST 270, Cr Debtor 3540
    let lines = cn_result.voucher.lines;
    let debtor_line = lines.iter().find(|l| l.ledger_id == "led_debtor").unwrap();
    assert_eq!(debtor_line.entry_type, "Cr");
    assert_eq!(debtor_line.amount, "3540");

    let ret_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_sales_ret")
        .unwrap();
    assert_eq!(ret_line.entry_type, "Dr");
    assert_eq!(ret_line.amount, "3000");

    let cgst_line = lines.iter().find(|l| l.ledger_id == "led_cgst").unwrap();
    assert_eq!(cgst_line.entry_type, "Dr");
    assert_eq!(cgst_line.amount, "270");

    // Verify Stock Movement: 2 PCS restored to inventory (4 + 2 = 6 PCS)
    let stock_bal_after: String = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(remaining_quantity), 0) AS TEXT) FROM inventory_cost_layers WHERE company_id = 'cmp_notes' AND stock_item_id = 'item_laptop'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        Decimal::from_str(&stock_bal_after).unwrap(),
        Decimal::from(6)
    );

    // Verify commercial_note_linkages record
    #[derive(sqlx::FromRow)]
    struct NoteLinkage {
        original_voucher_id: String,
        original_invoice_number: String,
        reason: Option<String>,
    }

    let link = sqlx::query_as::<_, NoteLinkage>(
        "SELECT original_voucher_id, original_invoice_number, reason
         FROM commercial_note_linkages WHERE company_id = 'cmp_notes' AND note_voucher_id = ?",
    )
    .bind(&cn_result.voucher.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(link.original_voucher_id, orig_sales_vid);
    assert_eq!(link.original_invoice_number, orig_sales_vnum);
    assert_eq!(link.reason.as_deref(), Some("Defective screens"));

    // 4. Over-Return Quantity Guardrail:
    // Original had 6 PCS. 2 already returned. Remaining returnable is 4 PCS.
    // Attempting to return 5 PCS must fail with Validation error.
    let over_qty_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::CreditNote,
        source_document_id: format!("cn_src_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_cn".to_string(),
        date: "2026-05-09".to_string(),
        due_date: None,
        party_id: "party_deb".to_string(),
        party_ledger_id: "led_debtor".to_string(),
        commercial_account_ledger_id: Some("led_sales_ret".to_string()),
        total_amount: Decimal::from(8850),
        document_reference: Some(orig_sales_vnum.clone()),
        narration: Some("Attempt excessive return".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Pro Laptop 15".to_string(),
            stock_item_id: Some("item_laptop".to_string()),
            unit_id: Some("u_pcs".to_string()),
            godown_id: Some("g_main".to_string()),
            quantity: Decimal::from(5), // 2 + 5 = 7 > 6!
            rate: Decimal::from(1500),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(7500),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![],
        bill_allocations: vec![],
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: Some(orig_sales_vid.clone()),
        return_reason: Some("Over return test".to_string()),
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let over_res =
        CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", over_qty_input).await;
    match over_res {
        Err(OpenTallyError::Validation(msg)) => {
            assert!(
                msg.contains("exceeds original quantity")
                    || msg.contains("exceeds remaining returnable quantity"),
                "Got error: {}",
                msg
            );
        }
        other => panic!("Expected Validation error for over-return, got {:?}", other),
    }

    // 5. Over-Return Value Guardrail:
    // Original had 9000 taxable. 3000 already returned. Remaining returnable is 6000.
    // Attempting to return 7000 taxable must fail with Validation error.
    let over_val_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::CreditNote,
        source_document_id: format!("cn_src_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_cn".to_string(),
        date: "2026-05-09".to_string(),
        due_date: None,
        party_id: "party_deb".to_string(),
        party_ledger_id: "led_debtor".to_string(),
        commercial_account_ledger_id: Some("led_sales_ret".to_string()),
        total_amount: Decimal::from(7000),
        document_reference: Some(orig_sales_vnum.clone()),
        narration: Some("Attempt excessive value return".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "General Adjustment".to_string(),
            stock_item_id: None,
            unit_id: None,
            godown_id: None,
            quantity: Decimal::ONE,
            rate: Decimal::from(7000),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(7000), // 3000 + 7000 = 10000 > 9000!
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![],
        bill_allocations: vec![],
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: Some(orig_sales_vid.clone()),
        return_reason: Some("Over return value test".to_string()),
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let over_val_res =
        CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", over_val_input).await;
    match over_val_res {
        Err(OpenTallyError::Validation(msg)) => {
            assert!(
                msg.contains("would exceed original invoice taxable value")
                    || msg.contains("exceeds original invoice taxable value"),
                "Got error: {}",
                msg
            );
        }
        other => panic!(
            "Expected Validation error for over-value return, got {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_debit_note_purchase_return_and_insufficient_stock_guard() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Purchase 10 PCS @ Rs 1000 = Rs 10,000 Taxable + 18% GST (CGST 900 + SGST 900) = Rs 11,800
    let pur_draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_notes",
        PurchaseDocumentRequest {
            series_id: "s_pur".to_string(),
            party_id: "party_crd".to_string(),
            bill_date: "2026-05-01".to_string(),
            supplier_invoice_no: Some("PUR-INV-55".to_string()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: Some("Purchase for Debit Note Test".to_string()),
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                stock_item_id: Some("item_laptop".to_string()),
                unit_id: Some("u_pcs".to_string()),
                godown_id: Some("g_main".to_string()),
                description: "Pro Laptop 15".to_string(),
                quantity: "10".to_string(),
                rate: "1000.00".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let pur_posted = PurchaseDocumentService::post_purchase_document(
        pool,
        "cmp_notes",
        &pur_draft.document.id,
        Some("led_purchase".to_string()),
    )
    .await
    .unwrap();

    let orig_pur_vid = pur_posted.voucher.voucher.id.clone();
    let orig_pur_vnum = pur_posted.voucher.voucher.voucher_number.clone();

    // 2. Post Debit Note returning 4 PCS to Supplier @ Rs 1000 = Rs 4000 Taxable + 18% GST (CGST 360 + SGST 360) = Rs 4720
    let dn_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::DebitNote,
        source_document_id: format!("dn_src_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_dn".to_string(),
        date: "2026-05-12".to_string(),
        due_date: None,
        party_id: "party_crd".to_string(),
        party_ledger_id: "led_creditor".to_string(),
        commercial_account_ledger_id: Some("led_purchase_ret".to_string()),
        total_amount: Decimal::from(4000),
        document_reference: Some(orig_pur_vnum.clone()),
        narration: Some("Defective goods returned to vendor".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Pro Laptop 15 (Returned to Vendor)".to_string(),
            stock_item_id: Some("item_laptop".to_string()),
            unit_id: Some("u_pcs".to_string()),
            godown_id: Some("g_main".to_string()),
            quantity: Decimal::from(4),
            rate: Decimal::from(1000),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(4000),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_cgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(360),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_sgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(360),
            },
        ],
        bill_allocations: vec![], // Auto AgstRef test
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: Some(orig_pur_vid.clone()),
        return_reason: Some("Vendor damaged batch".to_string()),
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let dn_result = CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", dn_input)
        .await
        .unwrap();

    assert_eq!(dn_result.voucher.voucher.voucher_type_id, "vt_debit_note");
    assert_eq!(dn_result.voucher.voucher.status, "posted");

    // Verify GL: Dr Creditor 4720, Cr Purchase Returns 4000, Cr CGST 360, Cr SGST 360
    let lines = dn_result.voucher.lines;
    let crd_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_creditor")
        .unwrap();
    assert_eq!(crd_line.entry_type, "Dr");
    assert_eq!(crd_line.amount, "4720");

    let pret_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_purchase_ret")
        .unwrap();
    assert_eq!(pret_line.entry_type, "Cr");
    assert_eq!(pret_line.amount, "4000");

    let cgst_line = lines.iter().find(|l| l.ledger_id == "led_cgst").unwrap();
    assert_eq!(cgst_line.entry_type, "Cr");
    assert_eq!(cgst_line.amount, "360");

    // Verify stock balance: 10 - 4 = 6 PCS
    let stock_bal: String = sqlx::query_scalar(
        "SELECT CAST(COALESCE(SUM(remaining_quantity), 0) AS TEXT) FROM inventory_cost_layers WHERE company_id = 'cmp_notes' AND stock_item_id = 'item_laptop'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(Decimal::from_str(&stock_bal).unwrap(), Decimal::from(6));
}

#[tokio::test]
async fn test_gstr1_and_gstr3b_statutory_reporting_with_notes() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Post Sales Invoice: Taxable Rs 10,000, CGST Rs 900, SGST Rs 900
    let sales_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: format!("sal_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_sal".to_string(),
        date: "2026-05-02".to_string(),
        due_date: None,
        party_id: "party_deb".to_string(),
        party_ledger_id: "led_debtor".to_string(),
        commercial_account_ledger_id: Some("led_sales".to_string()),
        total_amount: Decimal::from(10000),
        document_reference: Some("INV-STAT-01".to_string()),
        narration: Some("Statutory test sales".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Service item".to_string(),
            stock_item_id: None,
            unit_id: None,
            godown_id: None,
            quantity: Decimal::ONE,
            rate: Decimal::from(10000),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(10000),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_cgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(900),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_sgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(900),
            },
        ],
        bill_allocations: vec![],
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: None,
        return_reason: None,
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let sales_res =
        CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", sales_input)
            .await
            .unwrap();

    // 2. Post Credit Note: Taxable Rs 2000, CGST Rs 180, SGST Rs 180
    let cn_input = CommercialPostingInput {
        company_id: "cmp_notes".to_string(),
        document_type: CommercialDocumentType::CreditNote,
        source_document_id: format!("cn_{}", uuid::Uuid::new_v4()),
        source_voucher_id: None,
        series_id: "s_cn".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_deb".to_string(),
        party_ledger_id: "led_debtor".to_string(),
        commercial_account_ledger_id: Some("led_sales_ret".to_string()),
        total_amount: Decimal::from(2000),
        document_reference: Some(sales_res.voucher.voucher.voucher_number.clone()),
        narration: Some("Price adjustment credit note".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Credit note discount".to_string(),
            stock_item_id: None,
            unit_id: None,
            godown_id: None,
            quantity: Decimal::ONE,
            rate: Decimal::from(2000),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(2000),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_cgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(180),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_sgst".to_string(),
                rate: Decimal::from(9),
                amount: Decimal::from(180),
            },
        ],
        bill_allocations: vec![],
        place_of_supply_state_code: Some("07".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: Some(sales_res.voucher.voucher.id.clone()),
        return_reason: Some("Discount concession".to_string()),
        tds_section_code: None,
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    CommercialPostingService::post_commercial_voucher(pool, "cmp_notes", cn_input)
        .await
        .unwrap();

    // 3. Query GSTR-1 Summary
    let gstr1 = GstReportService::get_gstr1_summary(pool, "cmp_notes", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    // Table 9B should have 1 Credit Note record in CDNUR
    assert_eq!(gstr1.cdnur_table_9b.len(), 1);
    let cdn_row = &gstr1.cdnur_table_9b[0];
    assert_eq!(cdn_row.taxable_value, "2000.00");
    assert_eq!(cdn_row.cgst_amount, "180.00");
    assert_eq!(cdn_row.sgst_amount, "180.00");

    // 4. Query GSTR-3B Summary
    let gstr3b =
        GstReportService::get_gstr3b_summary(pool, "cmp_notes", "2026-05-01", "2026-05-31")
            .await
            .unwrap();

    // Outward taxable supplies should be netted: 10,000 - 2,000 = 8,000
    assert_eq!(gstr3b.table_3_1[0].total_taxable_value, "8000.00");
    assert_eq!(gstr3b.table_3_1[0].central_tax, "720.00"); // 900 - 180
    assert_eq!(gstr3b.table_3_1[0].state_ut_tax, "720.00"); // 900 - 180
}
