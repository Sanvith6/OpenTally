use crate::billwise::{AllocationType, BillAllocationInput, BillWiseService};
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::purchase::{
    PurchaseDiscountKind, PurchaseDocumentRequest, PurchaseDocumentService, PurchaseLineRequest,
};
use crate::reports::BillWiseReportService;
use crate::sales::{DiscountKind, SalesDocumentRequest, SalesDocumentService, SalesLineRequest};
use crate::vouchers::{
    CancelVoucherRequest, CreateVoucherDraftRequest, ReverseVoucherRequest,
    VoucherCancellationService, VoucherDraftService, VoucherLineInput, VoucherPostingService,
    VoucherReversalService,
};
use rust_decimal::Decimal;
use uuid::Uuid;

async fn setup_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("billwise_test.db"))
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
         VALUES ('grp_ca_a', 'cmp_a', 'Sundry Debtors', NULL, 'Assets', 1),
                ('grp_cl_a', 'cmp_a', 'Sundry Creditors', NULL, 'Liabilities', 1),
                ('grp_inc_a', 'cmp_a', 'Sales Accounts', NULL, 'Income', 1),
                ('grp_exp_a', 'cmp_a', 'Purchase Accounts', NULL, 'Expenses', 1),
                ('grp_bank_a', 'cmp_a', 'Bank Accounts', NULL, 'Assets', 1),
                ('grp_ca_b', 'cmp_b', 'Sundry Debtors', NULL, 'Assets', 1),
                ('grp_bank_b', 'cmp_b', 'Bank Accounts', NULL, 'Assets', 1)",
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
                ('led_bank_a', 'cmp_a', 'grp_bank_a', 'HDFC Bank A', 0, 'standard'),
                ('led_cust_b', 'cmp_b', 'grp_ca_b', 'Customer Ledger B', 0, 'standard'),
                ('led_bank_b', 'cmp_b', 'grp_bank_b', 'HDFC Bank B', 0, 'standard')",
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
                ('series_rcpt_a', 'cmp_a', 'vt_receipt', 'Receipt Series', 'RCPT', 'automatic', 'yearly'),
                ('series_pmt_a', 'cmp_a', 'vt_payment', 'Payment Series', 'PMT', 'automatic', 'yearly'),
                ('series_sal_b', 'cmp_b', 'vt_sales', 'Sales Series B', 'SALB', 'automatic', 'yearly')"
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_sal_a', 'cmp_a', 'vt_sales', 'series_sal_a', '2026-04-01', 'SAL/2026/', '', 1, 4),
                ('cfg_pur_a', 'cmp_a', 'vt_pur', 'series_pur_a', '2026-04-01', 'PUR/2026/', '', 1, 4),
                ('cfg_rcpt_a', 'cmp_a', 'vt_receipt', 'series_rcpt_a', '2026-04-01', 'RCPT/2026/', '', 1, 4),
                ('cfg_pmt_a', 'cmp_a', 'vt_payment', 'series_pmt_a', '2026-04-01', 'PMT/2026/', '', 1, 4),
                ('cfg_sal_b', 'cmp_b', 'vt_sales', 'series_sal_b', '2026-04-01', 'SALB/2026/', '', 1, 4)"
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
    sqlx::query("INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision) VALUES ('unit_pcs', 'cmp_a', 'PCS', 'Pieces', 'simple', 2)").execute(p).await.unwrap();
    sqlx::query(
        "INSERT INTO godowns (id, company_id, name) VALUES ('godown_main', 'cmp_a', 'Main Godown')",
    )
    .execute(p)
    .await
    .unwrap();
    sqlx::query("INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value) VALUES ('item_widget', 'cmp_a', 'Widget', 'sg_a', 'unit_pcs', '100', '10', '1000')").execute(p).await.unwrap();
}

#[tokio::test]
async fn test_f8_sales_new_ref_f6_receipt_agst_ref_and_over_settlement() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Post F8 Sales invoice for 1000.00
    let draft = SalesDocumentService::create_draft(
        pool,
        "cmp_a",
        SalesDocumentRequest {
            series_id: "series_sal_a".to_string(),
            party_id: "party_cust_a".to_string(),
            invoice_date: "2026-04-10".to_string(),
            document_reference: Some("INV-1001".to_string()),
            due_date_override: Some("2026-05-10".to_string()),
            due_date_override_reason: Some("Net 30 terms".to_string()),
            narration: Some("Test sale 1".to_string()),
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![SalesLineRequest {
                description: "Widget item".to_string(),
                stock_item_id: None,
                unit_id: None,
                godown_id: None,
                quantity: "10".to_string(),
                rate: "100.00".to_string(),
                discount_kind: DiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let post_res =
        SalesDocumentService::post_sales_document(pool, "cmp_a", &draft.document.id, None)
            .await
            .unwrap();

    let voucher_number = post_res.voucher.voucher.voucher_number;
    assert_eq!(voucher_number, "SAL/2026/0001");

    // Verify auto-generated NewRef
    let pending = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-15")
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].party_name, "Customer Ledger A");
    assert_eq!(pending[0].bill_name, "INV-1001");
    assert_eq!(pending[0].original_amount, "1000.00");
    assert_eq!(pending[0].settled_amount, "0.00");
    assert_eq!(pending[0].pending_amount, "1000.00");
    assert_eq!(pending[0].dr_cr, "Dr");
    assert_eq!(pending[0].overdue_days, 0); // As of 2026-04-15, due date 2026-05-10 is in future

    // 2. Post partial F6 Receipt for 400.00 with AgstRef
    let rcpt_draft = VoucherDraftService::create_draft(
        pool,
        "cmp_a",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_receipt".to_string(),
            series_id: "series_rcpt_a".to_string(),
            date: "2026-04-20".to_string(),
            effective_date: None,
            reference_no: Some("RCPT-01".to_string()),
            reference_date: None,
            narration: Some("Partial payment for INV-1001".to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_bank_a".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "400.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_cust_a".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "400.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let rcpt_posted =
        VoucherPostingService::post_voucher(pool, "cmp_a", &rcpt_draft.voucher.id, None)
            .await
            .unwrap();

    let cust_line = rcpt_posted
        .lines
        .iter()
        .find(|l| l.ledger_id == "led_cust_a")
        .unwrap();

    // Allocate AgstRef 400.00
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &rcpt_posted.voucher.id,
        &cust_line.id,
        "led_cust_a",
        "Cr",
        Decimal::new(40000, 2),
        "2026-04-20",
        None,
        &[BillAllocationInput {
            bill_name: "INV-1001".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "400.00".to_string(),
            dr_cr: Some("Cr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // Verify settled = 400.00, pending = 600.00
    let pending2 = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-25")
        .await
        .unwrap();
    assert_eq!(pending2.len(), 1);
    assert_eq!(pending2[0].settled_amount, "400.00");
    assert_eq!(pending2[0].pending_amount, "600.00");

    // 3. Test Over-Settlement Rejection (Attempting to settle 700.00 when only 600.00 remains)
    let over_alloc = [BillAllocationInput {
        bill_name: "INV-1001".to_string(),
        allocation_type: AllocationType::AgstRef,
        amount: "700.00".to_string(),
        dr_cr: Some("Cr".to_string()),
        due_date: None,
    }];
    let mut conn2 = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn2)
        .await
        .unwrap();
    let over_err = BillWiseService::validate_and_post_allocations(
        &mut conn2,
        "cmp_a",
        &rcpt_posted.voucher.id,
        &cust_line.id,
        "led_cust_a",
        "Cr",
        Decimal::new(70000, 2),
        "2026-04-26",
        None,
        &over_alloc,
    )
    .await;
    let _ = sqlx::query("ROLLBACK").execute(&mut *conn2).await;

    assert!(over_err.is_err());
    let err_msg = over_err.unwrap_err().to_string();
    assert!(
        err_msg.contains("Over-settlement of bill 'INV-1001'"),
        "Expected over-settlement error, got: {}",
        err_msg
    );

    // 4. Final settlement with exact remaining 600.00
    let mut conn3 = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn3)
        .await
        .unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn3,
        "cmp_a",
        &rcpt_posted.voucher.id,
        &cust_line.id,
        "led_cust_a",
        "Cr",
        Decimal::new(60000, 2),
        "2026-04-26",
        None,
        &[BillAllocationInput {
            bill_name: "INV-1001".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "600.00".to_string(),
            dr_cr: Some("Cr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn3).await.unwrap();

    // Now completely settled -> 0 pending bills returned
    let pending3 = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-30")
        .await
        .unwrap();
    assert_eq!(pending3.len(), 0);
}

#[tokio::test]
async fn test_f9_purchase_new_ref_f5_payment_agst_ref_flow() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Post F9 Purchase document for 2500.00
    let draft = PurchaseDocumentService::create_draft(
        pool,
        "cmp_a",
        PurchaseDocumentRequest {
            series_id: "series_pur_a".to_string(),
            party_id: "party_supp_a".to_string(),
            bill_date: "2026-04-05".to_string(),
            supplier_invoice_no: Some("BILL-5001".to_string()),
            due_date_override: Some("2026-05-05".to_string()),
            due_date_override_reason: Some("Net 30 terms".to_string()),
            narration: Some("Purchase supplies".to_string()),
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![PurchaseLineRequest {
                description: "Raw material".to_string(),
                stock_item_id: None,
                unit_id: None,
                godown_id: None,
                quantity: "25".to_string(),
                rate: "100.00".to_string(),
                discount_kind: PurchaseDiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let post_res =
        PurchaseDocumentService::post_purchase_document(pool, "cmp_a", &draft.document.id, None)
            .await
            .unwrap();

    assert_eq!(post_res.voucher.voucher.voucher_number, "PUR/2026/0001");

    // Verify auto-generated NewRef on Bills Payable
    let payables = BillWiseReportService::get_bills_payable(pool, "cmp_a", "2026-04-10")
        .await
        .unwrap();
    assert_eq!(payables.len(), 1);
    assert_eq!(payables[0].party_name, "Supplier Ledger A");
    assert_eq!(payables[0].bill_name, "BILL-5001");
    assert_eq!(payables[0].dr_cr, "Cr");
    assert_eq!(payables[0].pending_amount, "2500.00");

    // 2. Settle via F5 Payment for full 2500.00
    let pmt_draft = VoucherDraftService::create_draft(
        pool,
        "cmp_a",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_payment".to_string(),
            series_id: "series_pmt_a".to_string(),
            date: "2026-04-15".to_string(),
            effective_date: None,
            reference_no: Some("PMT-01".to_string()),
            reference_date: None,
            narration: Some("Pay supplier in full".to_string()),
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_supp_a".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "2500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_bank_a".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "2500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let pmt_posted =
        VoucherPostingService::post_voucher(pool, "cmp_a", &pmt_draft.voucher.id, None)
            .await
            .unwrap();

    let supp_line = pmt_posted
        .lines
        .iter()
        .find(|l| l.ledger_id == "led_supp_a")
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &pmt_posted.voucher.id,
        &supp_line.id,
        "led_supp_a",
        "Dr",
        Decimal::new(250000, 2),
        "2026-04-15",
        None,
        &[BillAllocationInput {
            bill_name: "BILL-5001".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "2500.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // Verify 0 pending payables
    let payables2 = BillWiseReportService::get_bills_payable(pool, "cmp_a", "2026-04-20")
        .await
        .unwrap();
    assert_eq!(payables2.len(), 0);
}

#[tokio::test]
async fn test_cancellation_reversal_guardrail_and_balance_restoration() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Post Sales Invoice for 1500.00
    let draft = SalesDocumentService::create_draft(
        pool,
        "cmp_a",
        SalesDocumentRequest {
            series_id: "series_sal_a".to_string(),
            party_id: "party_cust_a".to_string(),
            invoice_date: "2026-04-01".to_string(),
            document_reference: Some("INV-GUARD-1".to_string()),
            due_date_override: Some("2026-04-30".to_string()),
            due_date_override_reason: Some("Payment terms agreement".to_string()),
            narration: Some("Guardrail test".to_string()),
            document_discount_amount: "0".to_string(),
            additional_charges: "0".to_string(),
            lines: vec![SalesLineRequest {
                description: "Service item".to_string(),
                stock_item_id: None,
                unit_id: None,
                godown_id: None,
                quantity: "15".to_string(),
                rate: "100.00".to_string(),
                discount_kind: DiscountKind::None,
                discount_value: "0".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    let sales_posted =
        SalesDocumentService::post_sales_document(pool, "cmp_a", &draft.document.id, None)
            .await
            .unwrap();
    let sales_voucher_id = sales_posted.voucher.voucher.id;

    // 2. Post Receipt AgstRef for 500.00
    let rcpt_draft = VoucherDraftService::create_draft(
        pool,
        "cmp_a",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_receipt".to_string(),
            series_id: "series_rcpt_a".to_string(),
            date: "2026-04-05".to_string(),
            effective_date: None,
            reference_no: Some("RCPT-G1".to_string()),
            reference_date: None,
            narration: None,
            lines: Some(vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_bank_a".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_cust_a".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "500.00".to_string(),
                    narration: None,
                },
            ]),
        },
    )
    .await
    .unwrap();

    let rcpt_posted =
        VoucherPostingService::post_voucher(pool, "cmp_a", &rcpt_draft.voucher.id, None)
            .await
            .unwrap();
    let rcpt_voucher_id = rcpt_posted.voucher.id;

    let cust_line = rcpt_posted
        .lines
        .iter()
        .find(|l| l.ledger_id == "led_cust_a")
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &rcpt_voucher_id,
        &cust_line.id,
        "led_cust_a",
        "Cr",
        Decimal::new(50000, 2),
        "2026-04-05",
        None,
        &[BillAllocationInput {
            bill_name: "INV-GUARD-1".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "500.00".to_string(),
            dr_cr: Some("Cr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // 3. Attempting to Cancel or Reverse Sales Invoice must be BLOCKED
    let cancel_res = VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: sales_voucher_id.clone(),
            reason: "Attempted illegal cancel".to_string(),
            actor_id: None,
        },
    )
    .await;
    assert!(cancel_res.is_err());
    let cancel_err_msg = cancel_res.unwrap_err().to_string();
    assert!(
        cancel_err_msg.contains("has active settlements"),
        "Expected settlement block error, got: {}",
        cancel_err_msg
    );

    let rev_res = VoucherReversalService::reverse_voucher(
        pool,
        "cmp_a",
        ReverseVoucherRequest {
            original_voucher_id: sales_voucher_id.clone(),
            reversal_date: "2026-04-10".to_string(),
            reason: "Attempted illegal reverse".to_string(),
            actor_id: None,
        },
    )
    .await;
    assert!(rev_res.is_err());
    let rev_err_msg = rev_res.unwrap_err().to_string();
    assert!(
        rev_err_msg.contains("has active settlements"),
        "Expected settlement block error, got: {}",
        rev_err_msg
    );

    // 4. Cancel the Receipt voucher
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: rcpt_voucher_id,
            reason: "Customer cheque bounced".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Verify balance is restored back to full 1500.00
    let restored_pending = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-15")
        .await
        .unwrap();
    assert_eq!(restored_pending.len(), 1);
    assert_eq!(restored_pending[0].settled_amount, "0.00");
    assert_eq!(restored_pending[0].pending_amount, "1500.00");

    // 5. Now cancel the Sales invoice: must SUCCEED cleanly
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: sales_voucher_id,
            reason: "Order cancelled by customer".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    let final_pending = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-15")
        .await
        .unwrap();
    assert_eq!(final_pending.len(), 0);
}

#[tokio::test]
async fn test_aging_analysis_bucket_categorization() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // Create 4 bills for Acme Customer with different dates and due dates:
    // As of date: 2026-05-01
    // Bill 1: date 2026-04-10, due 2026-05-15 -> Not Due (100.00)
    // Bill 2: date 2026-04-01, due 2026-04-15 -> Overdue by 16 days -> 0-30 Days (200.00)
    // Bill 3: date 2026-02-15, due 2026-03-15 -> Overdue by 47 days -> 31-60 Days (300.00)
    // Bill 4: date 2025-12-01, due 2026-01-01 -> Overdue by 120 days -> 90+ Days (400.00)

    let test_cases = vec![
        ("BILL-NOTDUE", "2026-04-10", Some("2026-05-15"), "100.00"),
        ("BILL-NODUE-TODAY", "2026-05-01", None, "50.00"),
        ("BILL-0-30", "2026-04-01", Some("2026-04-15"), "200.00"),
        ("BILL-31-60", "2026-02-15", Some("2026-03-15"), "300.00"),
        ("BILL-90PLUS", "2025-12-01", Some("2026-01-01"), "400.00"),
    ];

    let mut conn = pool.acquire().await.unwrap();
    for (i, (bname, bdate, ddate, amt)) in test_cases.into_iter().enumerate() {
        let v_id = Uuid::new_v4().to_string();
        let vl_id = Uuid::new_v4().to_string();
        let parsed_amt = Decimal::from_str_exact(amt).unwrap();

        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
             VALUES (?, 'cmp_a', 'vt_sales', 'series_sal_a', 'automatic', ?, ?, ?, 'draft', ?)"
        )
        .bind(&v_id)
        .bind(bname)
        .bind(bdate)
        .bind(bdate)
        .bind(amt)
        .execute(&mut *conn)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
             VALUES (?, 'cmp_a', ?, 1, 'led_cust_a', 'Dr', ?)"
        )
        .bind(&vl_id)
        .bind(&v_id)
        .bind(amt)
        .execute(&mut *conn)
        .await
        .unwrap();

        let seq_num = (i + 1) as i64;
        sqlx::query(
            "UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = ? WHERE id = ?"
        )
        .bind(seq_num)
        .bind(&v_id)
        .execute(&mut *conn)
        .await
        .unwrap();

        BillWiseService::validate_and_post_allocations(
            &mut conn,
            "cmp_a",
            &v_id,
            &vl_id,
            "led_cust_a",
            "Dr",
            parsed_amt,
            bdate,
            ddate,
            &[BillAllocationInput {
                bill_name: bname.to_string(),
                allocation_type: AllocationType::NewRef,
                amount: amt.to_string(),
                dr_cr: Some("Dr".to_string()),
                due_date: ddate.map(ToString::to_string),
            }],
        )
        .await
        .unwrap();
        sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();
    }

    let report =
        BillWiseReportService::get_aging_analysis(pool, "cmp_a", "receivable", "2026-05-01")
            .await
            .unwrap();

    assert_eq!(report.grand_total.not_due, "150.00");
    assert_eq!(report.grand_total.days_0_30, "200.00");
    assert_eq!(report.grand_total.days_31_60, "300.00");
    assert_eq!(report.grand_total.days_61_90, "0.00");
    assert_eq!(report.grand_total.days_90_plus, "400.00");
    assert_eq!(report.grand_total.total, "1050.00");

    assert_eq!(report.parties.len(), 1);
    assert_eq!(report.parties[0].party_name, "Customer Ledger A");
    assert_eq!(report.parties[0].buckets.not_due, "150.00");
    assert_eq!(report.parties[0].buckets.days_0_30, "200.00");
    assert_eq!(report.parties[0].buckets.days_31_60, "300.00");
    assert_eq!(report.parties[0].buckets.days_61_90, "0.00");
    assert_eq!(report.parties[0].buckets.days_90_plus, "400.00");
    assert_eq!(report.parties[0].buckets.total, "1050.00");
    assert_eq!(report.parties[0].bills.len(), 5);
}

#[tokio::test]
async fn test_multi_company_isolation() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();

    // 1. Post Bill for Company Alpha (Customer A)
    let v_id_a = Uuid::new_v4().to_string();
    let vl_id_a = Uuid::new_v4().to_string();
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
         VALUES (?, 'cmp_a', 'vt_sales', 'series_sal_a', 'automatic', 'SAL-A-01', '2026-04-10', '2026-04-10', 'draft', '800.00')"
    )
    .bind(&v_id_a)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES (?, 'cmp_a', ?, 1, 'led_cust_a', 'Dr', '800.00')"
    )
    .bind(&vl_id_a)
    .bind(&v_id_a)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 1 WHERE id = ?"
    )
    .bind(&v_id_a)
    .execute(&mut *conn)
    .await
    .unwrap();

    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &v_id_a,
        &vl_id_a,
        "led_cust_a",
        "Dr",
        Decimal::new(80000, 2),
        "2026-04-10",
        None,
        &[BillAllocationInput {
            bill_name: "ALPHA-BILL-1".to_string(),
            allocation_type: AllocationType::NewRef,
            amount: "800.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // 2. Post Bill for Company Beta (Customer B)
    let v_id_b = Uuid::new_v4().to_string();
    let vl_id_b = Uuid::new_v4().to_string();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
         VALUES (?, 'cmp_b', 'vt_sales', 'series_sal_b', 'automatic', 'SAL-B-01', '2026-04-12', '2026-04-12', 'draft', '950.00')"
    )
    .bind(&v_id_b)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES (?, 'cmp_b', ?, 1, 'led_cust_b', 'Dr', '950.00')"
    )
    .bind(&vl_id_b)
    .bind(&v_id_b)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 1 WHERE id = ?"
    )
    .bind(&v_id_b)
    .execute(&mut *conn)
    .await
    .unwrap();

    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_b",
        &v_id_b,
        &vl_id_b,
        "led_cust_b",
        "Dr",
        Decimal::new(95000, 2),
        "2026-04-12",
        None,
        &[BillAllocationInput {
            bill_name: "BETA-BILL-1".to_string(),
            allocation_type: AllocationType::NewRef,
            amount: "950.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // 3. Verify Isolation
    let pending_a = BillWiseReportService::get_bills_receivable(pool, "cmp_a", "2026-04-20")
        .await
        .unwrap();
    assert_eq!(pending_a.len(), 1);
    assert_eq!(pending_a[0].bill_name, "ALPHA-BILL-1");
    assert_eq!(pending_a[0].pending_amount, "800.00");

    let pending_b = BillWiseReportService::get_bills_receivable(pool, "cmp_b", "2026-04-20")
        .await
        .unwrap();
    assert_eq!(pending_b.len(), 1);
    assert_eq!(pending_b[0].bill_name, "BETA-BILL-1");
    assert_eq!(pending_b[0].pending_amount, "950.00");

    // Attempt to settle ALPHA-BILL-1 inside Company Beta must fail (no active NewRef in Beta)
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    let cross_settle_res = BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_b",
        &v_id_b,
        &vl_id_b,
        "led_cust_b",
        "Cr",
        Decimal::new(80000, 2),
        "2026-04-15",
        None,
        &[BillAllocationInput {
            bill_name: "ALPHA-BILL-1".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "800.00".to_string(),
            dr_cr: Some("Cr".to_string()),
            due_date: None,
        }],
    )
    .await;
    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

    assert!(cross_settle_res.is_err());
    let cross_err = cross_settle_res.unwrap_err().to_string();
    assert!(
        cross_err.contains("no active NewRef or Advance exists for party 'led_cust_b'"),
        "Expected missing bill error, got: {}",
        cross_err
    );
}

#[tokio::test]
async fn test_batch_oversettlement_opposing_drcr_and_voucher_status_guardrails() {
    let (_dir, db) = setup_fixture().await;
    let pool = db.pool();
    let mut conn = pool.acquire().await.unwrap();

    // 1. Setup Draft Voucher (unposted)
    let v_draft_id = Uuid::new_v4().to_string();
    let vl_draft_id = Uuid::new_v4().to_string();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
         VALUES (?, 'cmp_a', 'vt_sales', 'series_sal_a', 'automatic', 'SAL-DRAFT-01', '2026-04-10', '2026-04-10', 'draft', '100.00')",
    )
    .bind(&v_draft_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES (?, 'cmp_a', ?, 1, 'led_cust_a', 'Dr', '100.00')",
    )
    .bind(&vl_draft_id)
    .bind(&v_draft_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    // Attempting to post allocations to draft voucher MUST fail
    let draft_alloc_res = BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &v_draft_id,
        &vl_draft_id,
        "led_cust_a",
        "Dr",
        Decimal::new(10000, 2),
        "2026-04-10",
        None,
        &[BillAllocationInput {
            bill_name: "BILL-DRAFT".to_string(),
            allocation_type: AllocationType::NewRef,
            amount: "100.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: None,
        }],
    )
    .await;
    assert!(draft_alloc_res.is_err());
    let err_msg = draft_alloc_res.unwrap_err().to_string();
    assert!(
        err_msg.contains("is not posted"),
        "Expected not posted error, got: {}",
        err_msg
    );

    // Now transition to posted
    sqlx::query(
        "UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 100 WHERE id = ?",
    )
    .bind(&v_draft_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    // Now NewRef succeeds
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &v_draft_id,
        &vl_draft_id,
        "led_cust_a",
        "Dr",
        Decimal::new(10000, 2),
        "2026-04-10",
        None,
        &[BillAllocationInput {
            bill_name: "BILL-TIGHT-100".to_string(),
            allocation_type: AllocationType::NewRef,
            amount: "100.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: None,
        }],
    )
    .await
    .unwrap();
    sqlx::query("COMMIT").execute(&mut *conn).await.unwrap();

    // 2. Setup Receipt Voucher for settlement tests
    let v_rcpt_id = Uuid::new_v4().to_string();
    let vl_rcpt_id = Uuid::new_v4().to_string();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
         VALUES (?, 'cmp_a', 'vt_receipt', 'series_rcpt_a', 'automatic', 'RCPT-TEST-01', '2026-04-15', '2026-04-15', 'draft', '140.00')",
    )
    .bind(&v_rcpt_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
         VALUES (?, 'cmp_a', ?, 1, 'led_cust_a', 'Cr', '140.00')",
    )
    .bind(&vl_rcpt_id)
    .bind(&v_rcpt_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 101 WHERE id = ?",
    )
    .bind(&v_rcpt_id)
    .execute(&mut *conn)
    .await
    .unwrap();

    // 2a. Guardrail: Opposing dr_cr validation (Attempting to settle Dr bill with Dr allocation must fail)
    let bad_drcr_res = BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &v_rcpt_id,
        &vl_rcpt_id,
        "led_cust_a",
        "Cr",
        Decimal::new(14000, 2),
        "2026-04-15",
        None,
        &[BillAllocationInput {
            bill_name: "BILL-TIGHT-100".to_string(),
            allocation_type: AllocationType::AgstRef,
            amount: "140.00".to_string(),
            dr_cr: Some("Dr".to_string()), // Invalid! Original bill is Dr, settlement must be Cr
            due_date: None,
        }],
    )
    .await;
    assert!(bad_drcr_res.is_err());
    let bad_drcr_err = bad_drcr_res.unwrap_err().to_string();
    assert!(
        bad_drcr_err.contains("Cannot settle Dr bill 'BILL-TIGHT-100' with a Dr allocation"),
        "Expected opposing entry error, got: {}",
        bad_drcr_err
    );

    // 2b. Guardrail: Batch Over-settlement loop hole prevention
    // Two allocations of 70.00 in same batch against 100.00 bill must be rejected
    let batch_oversettle_res = BillWiseService::validate_and_post_allocations(
        &mut conn,
        "cmp_a",
        &v_rcpt_id,
        &vl_rcpt_id,
        "led_cust_a",
        "Cr",
        Decimal::new(14000, 2),
        "2026-04-15",
        None,
        &[
            BillAllocationInput {
                bill_name: "BILL-TIGHT-100".to_string(),
                allocation_type: AllocationType::AgstRef,
                amount: "70.00".to_string(),
                dr_cr: Some("Cr".to_string()),
                due_date: None,
            },
            BillAllocationInput {
                bill_name: "BILL-TIGHT-100".to_string(),
                allocation_type: AllocationType::AgstRef,
                amount: "70.00".to_string(),
                dr_cr: Some("Cr".to_string()),
                due_date: None,
            },
        ],
    )
    .await;
    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

    assert!(batch_oversettle_res.is_err());
    let batch_err = batch_oversettle_res.unwrap_err().to_string();
    assert!(
        batch_err.contains("Over-settlement of bill 'BILL-TIGHT-100'"),
        "Expected batch oversettlement error, got: {}",
        batch_err
    );
    assert!(
        batch_err.contains("only 30.00 remains unpaid"),
        "Expected remaining calculation with in-batch accumulation, got: {}",
        batch_err
    );
}
