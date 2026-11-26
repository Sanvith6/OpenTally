use crate::banking::{BankAllocationInput, BankingService, ReconcileEntryInput};
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::vouchers::model::{CancelVoucherRequest, CreateVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::cancellation::VoucherCancellationService;
use crate::vouchers::service::draft::VoucherDraftService;
use crate::vouchers::service::posting::VoucherPostingService;

async fn setup_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("banking_test.db"))
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

    seed_fixture(p).await;
    (dir, db)
}

async fn seed_fixture(p: &sqlx::Pool<sqlx::Sqlite>) {
    // 1. Companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_bank_1', 'Banking Test Co 1', 'Banking Co 1', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                ('cmp_bank_2', 'Banking Test Co 2', 'Banking Co 2', 'India', 'Karnataka', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    // 2. Account Groups
    // Predefined hierarchy: Bank Accounts under Current Assets, Bank OD A/c under Loans (Liability)
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca_1', 'cmp_bank_1', 'Current Assets', NULL, 'Assets', 1),
                ('grp_bank_1', 'cmp_bank_1', 'Bank Accounts', 'grp_ca_1', 'Assets', 0),
                ('grp_loan_1', 'cmp_bank_1', 'Loans (Liability)', NULL, 'Liabilities', 1),
                ('grp_od_1', 'cmp_bank_1', 'Bank OD A/c', 'grp_loan_1', 'Liabilities', 0),
                ('grp_cl_1', 'cmp_bank_1', 'Current Liabilities', NULL, 'Liabilities', 1),
                ('grp_cred_1', 'cmp_bank_1', 'Sundry Creditors', 'grp_cl_1', 'Liabilities', 0),
                ('grp_debt_1', 'cmp_bank_1', 'Sundry Debtors', 'grp_ca_1', 'Assets', 0),
                
                ('grp_ca_2', 'cmp_bank_2', 'Current Assets', NULL, 'Assets', 1),
                ('grp_bank_2', 'cmp_bank_2', 'Bank Accounts', 'grp_ca_2', 'Assets', 0)",
    )
    .execute(p)
    .await
    .unwrap();

    // 3. Ledgers
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_hdfc_1', 'cmp_bank_1', 'grp_bank_1', 'HDFC Bank Regular A/c', 0, 'standard'),
                ('led_sbi_od_1', 'cmp_bank_1', 'grp_od_1', 'SBI Overdraft A/c', 0, 'standard'),
                ('led_vendor_1', 'cmp_bank_1', 'grp_cred_1', 'Precision Tools Ltd', 0, 'standard'),
                ('led_customer_1', 'cmp_bank_1', 'grp_debt_1', 'Apex Retailers', 0, 'standard'),
                
                ('led_icici_2', 'cmp_bank_2', 'grp_bank_2', 'ICICI Bank Co 2', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    // 4. Opening balances table
    sqlx::query(
        "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, amount, balance_type)
         VALUES ('ob_hdfc_1', 'cmp_bank_1', 'led_hdfc_1', '2026-04-01', '10000.00', 'Dr'),
                ('ob_sbi_1', 'cmp_bank_1', 'led_sbi_od_1', '2026-04-01', '5000.00', 'Cr'),
                ('ob_icici_2', 'cmp_bank_2', 'led_icici_2', '2026-04-01', '50000.00', 'Dr')",
    )
    .execute(p)
    .await
    .unwrap();

    // 5. Voucher Series (voucher_types are auto-seeded by trg_seed_predefined_voucher_types_on_company_insert)
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('ser_pmt_1', 'cmp_bank_1', 'vt_payment', 'Payment Series', 'PMT', 'automatic', 'yearly'),
                ('ser_rcpt_1', 'cmp_bank_1', 'vt_receipt', 'Receipt Series', 'RCT', 'automatic', 'yearly'),
                ('ser_contra_1', 'cmp_bank_1', 'vt_contra', 'Contra Series', 'CNT', 'automatic', 'yearly'),
                ('ser_pmt_2', 'cmp_bank_2', 'vt_payment', 'Payment Series 2', 'PMT2', 'automatic', 'yearly')",
    )
    .execute(p)
    .await
    .unwrap();

    // 6. Voucher Number Configs
    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_pmt_1', 'cmp_bank_1', 'vt_payment', 'ser_pmt_1', '2026-04-01', 'PMT/', '', 1, 4),
                ('cfg_rcpt_1', 'cmp_bank_1', 'vt_receipt', 'ser_rcpt_1', '2026-04-01', 'RCT/', '', 1, 4),
                ('cfg_contra_1', 'cmp_bank_1', 'vt_contra', 'ser_contra_1', '2026-04-01', 'CNT/', '', 1, 4),
                ('cfg_pmt_2', 'cmp_bank_2', 'vt_payment', 'ser_pmt_2', '2026-04-01', 'PMT2/', '', 1, 4)",
    )
    .execute(p)
    .await
    .unwrap();
}

async fn post_bank_voucher(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
    voucher_type_id: &str,
    series_id: &str,
    date: &str,
    lines: Vec<(&str, &str, &str)>, // (ledger_id, entry_type, amount)
    bank_allocations: Option<Vec<BankAllocationInput>>,
) -> String {
    let line_inputs: Vec<VoucherLineInput> = lines
        .into_iter()
        .map(|(lid, entry_type, amt)| VoucherLineInput {
            id: None,
            ledger_id: lid.to_string(),
            entry_type: entry_type.to_string(),
            amount: amt.to_string(),
            narration: None,
        })
        .collect();

    let draft = VoucherDraftService::create_draft(
        pool,
        company_id,
        CreateVoucherDraftRequest {
            voucher_type_id: voucher_type_id.to_string(),
            series_id: series_id.to_string(),
            date: date.to_string(),
            effective_date: Some(date.to_string()),
            reference_no: None,
            reference_date: None,
            narration: Some("Banking test voucher".to_string()),
            lines: Some(line_inputs),
        },
    )
    .await
    .expect("create draft");

    let posted = VoucherPostingService::post_voucher_with_allocations(
        pool,
        company_id,
        &draft.voucher.id,
        Some("operator"),
        bank_allocations.as_deref(),
    )
    .await
    .expect("post voucher");

    posted.voucher.id
}

#[tokio::test]
async fn test_bank_ledger_listing_and_balance() {
    let (_dir, db) = setup_fixture().await;

    let ledgers = BankingService::list_bank_ledgers(db.pool(), "cmp_bank_1")
        .await
        .expect("list bank ledgers");

    // Must return both HDFC (under Bank Accounts) and SBI OD (under Bank OD A/c), but not Vendor or Customer
    assert_eq!(ledgers.len(), 2);
    let hdfc = ledgers.iter().find(|l| l.id == "led_hdfc_1").unwrap();
    assert_eq!(hdfc.name, "HDFC Bank Regular A/c");
    assert_eq!(hdfc.group_name, "Bank Accounts");
    assert_eq!(hdfc.current_balance, "10000.00");
    assert_eq!(hdfc.current_balance_dr_cr, "Dr");

    let sbi = ledgers.iter().find(|l| l.id == "led_sbi_od_1").unwrap();
    assert_eq!(sbi.name, "SBI Overdraft A/c");
    assert_eq!(sbi.group_name, "Bank OD A/c");
    assert_eq!(sbi.current_balance, "5000.00");
    assert_eq!(sbi.current_balance_dr_cr, "Cr");
}

#[tokio::test]
async fn test_voucher_auto_allocations() {
    let (_dir, db) = setup_fixture().await;

    // 1. Post F5 Payment: Dr Vendor 15,000, Cr HDFC Bank 15,000 (No explicit bank_allocations passed)
    let pmt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-05",
        vec![
            ("led_vendor_1", "Dr", "15000.00"),
            ("led_hdfc_1", "Cr", "15000.00"),
        ],
        None,
    )
    .await;

    // Verify auto-created allocation
    type AutoAllocRow = (String, String, Option<String>, Option<String>, String, i64);
    let allocs: Vec<AutoAllocRow> = sqlx::query_as(
        "SELECT transaction_type, amount, instrument_number, bank_date, dr_cr, is_cancelled 
         FROM bank_allocations WHERE company_id = 'cmp_bank_1' AND voucher_id = ?",
    )
    .bind(&pmt_id)
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(allocs.len(), 1);
    assert_eq!(allocs[0].0, "Other");
    assert_eq!(allocs[0].1, "15000.00");
    assert!(allocs[0].2.is_none());
    assert!(allocs[0].3.is_none()); // Unreconciled
    assert_eq!(allocs[0].4, "Cr");
    assert_eq!(allocs[0].5, 0); // is_cancelled = 0

    // 2. Post F6 Receipt: Dr HDFC Bank 25,000, Cr Customer 25,000 with explicit BankAllocationInput
    let rcpt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_receipt",
        "ser_rcpt_1",
        "2026-04-08",
        vec![
            ("led_hdfc_1", "Dr", "25000.00"),
            ("led_customer_1", "Cr", "25000.00"),
        ],
        Some(vec![BankAllocationInput {
            voucher_line_id: None,
            bank_ledger_id: Some("led_hdfc_1".to_string()),
            transaction_type: Some("Cheque".to_string()),
            instrument_number: Some("CHQ-987654".to_string()),
            instrument_date: Some("2026-04-07".to_string()),
            favouring_name: Some("Apex Retailers".to_string()),
            bank_date: None,
        }]),
    )
    .await;

    type ExplicitAllocRow = (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    );
    let r_allocs: Vec<ExplicitAllocRow> = sqlx::query_as(
        "SELECT transaction_type, amount, instrument_number, instrument_date, favouring_name, dr_cr 
         FROM bank_allocations WHERE company_id = 'cmp_bank_1' AND voucher_id = ?",
    )
    .bind(&rcpt_id)
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(r_allocs.len(), 1);
    assert_eq!(r_allocs[0].0, "Cheque");
    assert_eq!(r_allocs[0].1, "25000.00");
    assert_eq!(r_allocs[0].2.as_deref(), Some("CHQ-987654"));
    assert_eq!(r_allocs[0].3.as_deref(), Some("2026-04-07"));
    assert_eq!(r_allocs[0].4.as_deref(), Some("Apex Retailers"));
    assert_eq!(r_allocs[0].5, "Dr");
}

#[tokio::test]
async fn test_brs_statement_equilibrium() {
    let (_dir, db) = setup_fixture().await;

    // Post 3 vouchers:
    // 1. Payment PMT-01: Cr HDFC 12,000 (voucher date 2026-04-02)
    // 2. Receipt RCT-01: Dr HDFC 30,000 (voucher date 2026-04-03)
    // 3. Payment PMT-02: Cr HDFC 8,000 (voucher date 2026-04-04)
    let pmt1_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-02",
        vec![
            ("led_vendor_1", "Dr", "12000.00"),
            ("led_hdfc_1", "Cr", "12000.00"),
        ],
        None,
    )
    .await;

    let _rct1_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_receipt",
        "ser_rcpt_1",
        "2026-04-03",
        vec![
            ("led_hdfc_1", "Dr", "30000.00"),
            ("led_customer_1", "Cr", "30000.00"),
        ],
        None,
    )
    .await;

    let _pmt2_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-04",
        vec![
            ("led_vendor_1", "Dr", "8000.00"),
            ("led_hdfc_1", "Cr", "8000.00"),
        ],
        None,
    )
    .await;

    // Reconcile PMT-01 (12,000 Cr) with bank_date = "2026-04-10"
    let pmt1_alloc_id: String =
        sqlx::query_scalar("SELECT id FROM bank_allocations WHERE voucher_id = ?")
            .bind(&pmt1_id)
            .fetch_one(db.pool())
            .await
            .unwrap();

    BankingService::reconcile_bank_entries(
        db.pool(),
        "cmp_bank_1",
        vec![ReconcileEntryInput {
            bank_allocation_id: pmt1_alloc_id,
            bank_date: Some("2026-04-10".to_string()),
        }],
        Some("operator"),
    )
    .await
    .unwrap();

    // Query BRS for period 2026-04-01 to 2026-04-30
    let brs = BankingService::get_bank_reconciliation(
        db.pool(),
        "cmp_bank_1",
        "led_hdfc_1",
        "2026-04-01",
        "2026-04-30",
        true,
    )
    .await
    .unwrap();

    // Book Balance:
    // Opening Dr: 10,000.00
    // + Receipts (Dr): 30,000.00
    // - Payments (Cr): 12,000.00 + 8,000.00 = 20,000.00
    // Net Book Balance: 20,000.00 Dr
    assert_eq!(brs.summary.book_balance, "20000.00");
    assert_eq!(brs.summary.book_balance_dr_cr, "Dr");

    // Unreconciled:
    // RCT-01 (30,000 Dr) is unreconciled (bank_date is NULL)
    // PMT-02 (8,000 Cr) is unreconciled (bank_date is NULL)
    // PMT-01 (12,000 Cr) is reconciled on 2026-04-10
    assert_eq!(brs.summary.unreconciled_debit, "30000.00");
    assert_eq!(brs.summary.unreconciled_credit, "8000.00");

    // Balance as per Bank:
    // Formula: Book Balance (+20,000) - Unreconciled Dr (30,000) + Unreconciled Cr (8,000) = -2,000.00 (2,000.00 Cr)
    assert_eq!(brs.summary.bank_balance, "2000.00");
    assert_eq!(brs.summary.bank_balance_dr_cr, "Cr");

    // 3 rows returned
    assert_eq!(brs.rows.len(), 3);
}

#[tokio::test]
async fn test_reconciliation_workflow_and_validation() {
    let (_dir, db) = setup_fixture().await;

    // Post Payment voucher on 2026-04-10
    let pmt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-10",
        vec![
            ("led_vendor_1", "Dr", "5000.00"),
            ("led_hdfc_1", "Cr", "5000.00"),
        ],
        None,
    )
    .await;

    let alloc_id: String =
        sqlx::query_scalar("SELECT id FROM bank_allocations WHERE voucher_id = ?")
            .bind(&pmt_id)
            .fetch_one(db.pool())
            .await
            .unwrap();

    // 1. Validation failure: bank_date < voucher_date (2026-04-08 < 2026-04-10)
    let err = BankingService::reconcile_bank_entries(
        db.pool(),
        "cmp_bank_1",
        vec![ReconcileEntryInput {
            bank_allocation_id: alloc_id.clone(),
            bank_date: Some("2026-04-08".to_string()),
        }],
        Some("operator"),
    )
    .await
    .expect_err("should reject bank date prior to voucher date");

    match err {
        OpenTallyError::Validation(msg) => {
            assert!(
                msg.contains("cannot precede voucher date"),
                "Error message: {}",
                msg
            );
        }
        other => panic!("Expected Validation error, got: {:?}", other),
    }

    // 2. Successful reconciliation: bank_date = "2026-04-12"
    let ok_res = BankingService::reconcile_bank_entries(
        db.pool(),
        "cmp_bank_1",
        vec![ReconcileEntryInput {
            bank_allocation_id: alloc_id.clone(),
            bank_date: Some("2026-04-12".to_string()),
        }],
        Some("operator"),
    )
    .await
    .expect("reconciliation should succeed");

    assert_eq!(ok_res.updated_count, 1);
    assert!(ok_res.success);

    // Verify MCA audit_log recorded the reconciliation
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_bank_1' AND action = 'UPDATE' AND entity_type = 'bank_reconciliation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert!(
        audit_count >= 1,
        "Audit log must record RECONCILE_BANK_ALLOCATION"
    );
}

#[tokio::test]
async fn test_bank_voucher_cancellation_audit() {
    let (_dir, db) = setup_fixture().await;

    // Post Payment voucher
    let pmt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-15",
        vec![
            ("led_vendor_1", "Dr", "7500.00"),
            ("led_hdfc_1", "Cr", "7500.00"),
        ],
        None,
    )
    .await;

    // Cancel voucher
    VoucherCancellationService::cancel_voucher(
        db.pool(),
        "cmp_bank_1",
        CancelVoucherRequest {
            voucher_id: pmt_id.clone(),
            reason: "Cheque stopped by company".to_string(),
            actor_id: Some("operator".to_string()),
        },
    )
    .await
    .unwrap();

    // Verify bank_allocations is_cancelled = 1
    let is_cancelled: i64 =
        sqlx::query_scalar("SELECT is_cancelled FROM bank_allocations WHERE voucher_id = ?")
            .bind(&pmt_id)
            .fetch_one(db.pool())
            .await
            .unwrap();

    assert_eq!(is_cancelled, 1);

    // Verify MCA audit_log recorded CANCEL_BANK_ALLOCATIONS
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_bank_1' AND action = 'CANCEL' AND entity_type = 'bank_allocation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    assert!(
        audit_count >= 1,
        "Audit log must record CANCEL_BANK_ALLOCATIONS"
    );
}

#[tokio::test]
async fn test_csv_bank_statement_auto_match() {
    let (_dir, db) = setup_fixture().await;

    // 1. Post Receipt (Dr Bank 45,000) with instrument number "NEFT-IN-555" on 2026-04-10
    let _rct_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_receipt",
        "ser_rcpt_1",
        "2026-04-10",
        vec![
            ("led_hdfc_1", "Dr", "45000.00"),
            ("led_customer_1", "Cr", "45000.00"),
        ],
        Some(vec![BankAllocationInput {
            voucher_line_id: None,
            bank_ledger_id: Some("led_hdfc_1".to_string()),
            transaction_type: Some("NEFT".to_string()),
            instrument_number: Some("NEFT-IN-555".to_string()),
            instrument_date: Some("2026-04-10".to_string()),
            favouring_name: None,
            bank_date: None,
        }]),
    )
    .await;

    // 2. Post Payment (Cr Bank 18,000) on 2026-04-12 without instrument number
    let _pmt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-12",
        vec![
            ("led_vendor_1", "Dr", "18000.00"),
            ("led_hdfc_1", "Cr", "18000.00"),
        ],
        None,
    )
    .await;

    // CSV Bank Statement
    // Polarity:
    // Deposit (Credit in bank) = 45000 -> matches Book Debit (Receipt)
    // Withdrawal (Debit in bank) = 18000 -> matches Book Credit (Payment)
    let csv = r#"Date,Description,Reference,Withdrawal,Deposit
2026-04-11,Customer NEFT Inflow,NEFT-IN-555,0.00,45000.00
2026-04-14,Vendor Wire Out,REF-UNKNOWN,18000.00,0.00
2026-04-20,Interest Credit,INT-001,0.00,250.00
"#;

    let preview = BankingService::preview_bank_statement_auto_match(
        db.pool(),
        "cmp_bank_1",
        "led_hdfc_1",
        csv,
    )
    .await
    .expect("statement auto match preview");

    assert_eq!(preview.total_statement_rows, 3);
    assert_eq!(preview.exact_matches, 1);
    assert_eq!(preview.suggested_matches, 1);
    assert_eq!(preview.unmatched, 1);

    // Row 0: Exact match on NEFT-IN-555 and 45000
    let r0 = &preview.rows[0];
    assert_eq!(r0.match_type, "exact");
    assert_eq!(r0.confidence, "HIGH");
    assert!(r0.matched_voucher_number.is_some());

    // Row 1: Suggested match on amount 18000 within +2 days (2026-04-14 vs 2026-04-12)
    let r1 = &preview.rows[1];
    assert_eq!(r1.match_type, "suggested");
    assert_eq!(r1.confidence, "MEDIUM");
    assert!(r1.matched_voucher_number.is_some());

    // Row 2: Unmatched
    let r2 = &preview.rows[2];
    assert_eq!(r2.match_type, "unmatched");
    assert_eq!(r2.confidence, "NONE");
}

#[tokio::test]
async fn test_multi_company_isolation() {
    let (_dir, db) = setup_fixture().await;

    // Post payment in cmp_bank_1
    let _pmt_id = post_bank_voucher(
        db.pool(),
        "cmp_bank_1",
        "vt_payment",
        "ser_pmt_1",
        "2026-04-05",
        vec![
            ("led_vendor_1", "Dr", "5000.00"),
            ("led_hdfc_1", "Cr", "5000.00"),
        ],
        None,
    )
    .await;

    // List bank ledgers for cmp_bank_2 must only return led_icici_2
    let ledgers_cmp2 = BankingService::list_bank_ledgers(db.pool(), "cmp_bank_2")
        .await
        .unwrap();
    assert_eq!(ledgers_cmp2.len(), 1);
    assert_eq!(ledgers_cmp2[0].id, "led_icici_2");

    // Attempting to query BRS for led_hdfc_1 under cmp_bank_2 must return NotFound
    let err = BankingService::get_bank_reconciliation(
        db.pool(),
        "cmp_bank_2",
        "led_hdfc_1",
        "2026-04-01",
        "2026-04-30",
        true,
    )
    .await
    .expect_err("should isolate ledgers across companies");

    match err {
        OpenTallyError::NotFound(msg) => {
            assert!(msg.contains("led_hdfc_1"), "Message was: {}", msg);
        }
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}
