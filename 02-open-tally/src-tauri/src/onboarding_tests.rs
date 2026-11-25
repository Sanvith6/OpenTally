//! Integration Tests for Milestone 7: TallyPrime Onboarding & F11 Features
//!
//! Verifies:
//! 1. `save_company_features` auto-seeding idempotency (8 GST ledgers + Stock-in-Hand/COGS mappings, zero duplicates on re-save).
//! 2. `create_sample_company` mathematical invariants:
//!    - Double-entry Trial Balance difference == 0
//!    - Statutory GSTR-1 and GSTR-3B non-zero reports
//!    - Outstanding Bills Receivable and Payable pending amounts
//!    - Invoice PDF generation success on sales voucher
//! 3. Multi-company tenant isolation between sample company and newly created companies.

use crate::company::features::{CompanyFeaturesService, SaveCompanyFeaturesInput};
use crate::company::model::CreateCompanyRequest;
use crate::company::sample::SampleCompanyService;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::pdf::generate_invoice_pdf_bytes;
use crate::reports::billwise::BillWiseReportService;
use crate::reports::gst::GstReportService;
use crate::reports::model::TrialBalanceRequest;
use crate::reports::trial_balance::TrialBalanceService;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

async fn setup_test_pool() -> (tempfile::TempDir, Pool<Sqlite>) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("onboarding_test.db"))
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
    run_milestone12_closing_budgets_audit_migration(p)
        .await
        .unwrap();
    run_milestone12_closing_budgets_audit_migration(p)
        .await
        .unwrap();

    (dir, p.clone())
}

#[tokio::test]
async fn test_f11_features_auto_seeding_and_idempotency() {
    let (_dir, pool) = setup_test_pool().await;

    // 1. Create a fresh test company
    let comp = CompanyService::create_company(
        &pool,
        CreateCompanyRequest {
            name: "Test SME Auto Seed".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    let company_id = &comp.id;

    // Initially, GST and Inventory mappings should not exist
    let initial_features = CompanyFeaturesService::get_company_features(&pool, company_id)
        .await
        .unwrap();
    assert!(!initial_features.has_gst_ledgers_mapped);
    assert!(!initial_features.has_inventory_ledgers_mapped);

    // 2. First Save: Enable GST + Perpetual Inventory with auto-seeding
    let valid_gstin = "29ABCDE1234F1ZW"; // Valid Mod-36 Karnataka GSTIN
    let saved1 = CompanyFeaturesService::save_company_features(
        &pool,
        company_id,
        SaveCompanyFeaturesInput {
            maintain_inventory: true,
            billwise_tracking: true,
            enable_perpetual_cogs: true,
            gst_enabled: true,
            gstin: Some(valid_gstin.to_string()),
            state_code: Some("29".to_string()),
            registration_type: Some("Regular".to_string()),
            auto_seed_gst_ledgers: true,
            auto_seed_inventory_ledgers: true,
            tds_enabled: Some(false),
            tan: None,
            deductor_type: None,
            auto_seed_tds_defaults: false,
            enable_payroll: Some(false),
            auto_seed_payroll_ledgers: false,
        },
    )
    .await
    .unwrap();

    assert!(saved1.gst_enabled);
    assert!(saved1.maintain_inventory);
    assert!(saved1.enable_perpetual_cogs);
    assert!(saved1.has_gst_ledgers_mapped);
    assert!(saved1.has_inventory_ledgers_mapped);

    // Verify exactly 8 GST ledgers exist under Duties & Taxes
    let duties_taxes_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Duties & Taxes' COLLATE NOCASE",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let gst_ledgers_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ledgers WHERE company_id = ? AND group_id = ?")
            .bind(company_id)
            .bind(&duties_taxes_grp)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(gst_ledgers_count, 8);

    // Verify Stock-in-Hand and COGS ledgers exist
    let stock_in_hand_exists: Option<String> = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Stock-in-Hand' COLLATE NOCASE",
    )
    .bind(company_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(stock_in_hand_exists.is_some());

    let cogs_exists: Option<String> = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Cost of Goods Sold' COLLATE NOCASE",
    )
    .bind(company_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(cogs_exists.is_some());

    // 3. Second Save: Re-run save_company_features (Idempotency check)
    let saved2 = CompanyFeaturesService::save_company_features(
        &pool,
        company_id,
        SaveCompanyFeaturesInput {
            maintain_inventory: true,
            billwise_tracking: true,
            enable_perpetual_cogs: true,
            gst_enabled: true,
            gstin: Some(valid_gstin.to_string()),
            state_code: Some("29".to_string()),
            registration_type: Some("Regular".to_string()),
            auto_seed_gst_ledgers: true,
            auto_seed_inventory_ledgers: true,
            tds_enabled: Some(false),
            tan: None,
            deductor_type: None,
            auto_seed_tds_defaults: false,
            enable_payroll: Some(false),
            auto_seed_payroll_ledgers: false,
        },
    )
    .await
    .unwrap();

    assert!(saved2.has_gst_ledgers_mapped);
    assert!(saved2.has_inventory_ledgers_mapped);

    // Verify count did not double (still exactly 8 GST ledgers)
    let gst_ledgers_count2: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ledgers WHERE company_id = ? AND group_id = ?")
            .bind(company_id)
            .bind(&duties_taxes_grp)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(gst_ledgers_count2, 8);
}

#[tokio::test]
async fn test_create_sample_company_end_to_end_invariants() {
    let (_dir, pool) = setup_test_pool().await;

    // 1. Create Sample Company
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();

    assert!(sample_company
        .name
        .starts_with("Bharath Precision Components Pvt Ltd"));
    assert_eq!(sample_company.state, "Karnataka");

    let cid = &sample_company.id;

    // 2. Trial Balance Check: Mathematical Equilibrium Invariant
    let tb_req = TrialBalanceRequest {
        as_of_date: "2025-05-31".to_string(),
        account_group_id: None,
        ledger_id: None,
        account_search: None,
        include_zero_balances: true,
    };
    let tb = TrialBalanceService::generate(&pool, cid, tb_req)
        .await
        .unwrap();

    assert_eq!(
        tb.difference, "0.00",
        "Trial balance difference must be exactly 0.00"
    );
    let total_dr = Decimal::from_str(&tb.total_debit).unwrap();
    let total_cr = Decimal::from_str(&tb.total_credit).unwrap();
    assert_eq!(total_dr, total_cr);
    assert!(total_dr > Decimal::ZERO, "Total debit must be non-zero");

    // 3. Statutory GST Reports Check: GSTR-1 and GSTR-3B must return non-zero statutory figures
    let gstr1 = GstReportService::get_gstr1_summary(&pool, cid, "2025-04-01", "2025-05-31")
        .await
        .unwrap();

    assert_eq!(
        gstr1.b2b_invoices.len(),
        2,
        "GSTR-1 must contain 2 B2B invoices"
    );
    let gstr1_taxable = Decimal::from_str(&gstr1.total_taxable_value).unwrap();
    assert_eq!(
        gstr1_taxable,
        Decimal::new(34000, 0),
        "Taxable sales = 16000 + 18000 = 34000"
    );
    let gstr1_cgst = Decimal::from_str(&gstr1.total_cgst).unwrap();
    assert_eq!(gstr1_cgst, Decimal::new(1440, 0));
    let gstr1_sgst = Decimal::from_str(&gstr1.total_sgst).unwrap();
    assert_eq!(gstr1_sgst, Decimal::new(1440, 0));
    let gstr1_igst = Decimal::from_str(&gstr1.total_igst).unwrap();
    assert_eq!(gstr1_igst, Decimal::new(2880, 0));

    let gstr3b = GstReportService::get_gstr3b_summary(&pool, cid, "2025-04-01", "2025-05-31")
        .await
        .unwrap();

    assert!(
        !gstr3b.table_3_1.is_empty(),
        "GSTR-3B Table 3.1 must have outward supply rows"
    );
    assert!(
        !gstr3b.table_4_itc.is_empty(),
        "GSTR-3B Table 4 must have eligible ITC rows"
    );

    // 4. Outstanding AR/AP Aging Reports Check
    // Bills Receivable: INV-2025-001 (original 18,880, settled 10,000, pending 8,880)
    //                   INV-2025-002 (original 20,880, settled 0, pending 20,880)
    let ar_bills = BillWiseReportService::get_bills_receivable(&pool, cid, "2025-05-31")
        .await
        .unwrap();
    assert_eq!(ar_bills.len(), 2, "Expected 2 active receivables");

    let inv1 = ar_bills
        .iter()
        .find(|b| b.bill_name == "INV-2025-001")
        .unwrap();
    assert_eq!(inv1.settled_amount, "10000.00");
    assert_eq!(inv1.pending_amount, "8880.00");

    let inv2 = ar_bills
        .iter()
        .find(|b| b.bill_name == "INV-2025-002")
        .unwrap();
    assert_eq!(inv2.settled_amount, "0.00");
    assert_eq!(inv2.pending_amount, "20880.00");

    // Bills Payable: PO-2025-001 (original 41,300, settled 20,000, pending 21,300)
    //                PO-2025-002 (original 11,200, settled 0, pending 11,200)
    let ap_bills = BillWiseReportService::get_bills_payable(&pool, cid, "2025-05-31")
        .await
        .unwrap();
    assert_eq!(ap_bills.len(), 2, "Expected 2 active payables");

    let po1 = ap_bills
        .iter()
        .find(|b| b.bill_name == "PO-2025-001")
        .unwrap();
    assert_eq!(po1.settled_amount, "20000.00");
    assert_eq!(po1.pending_amount, "21300.00");

    // 5. PDF Invoice Generation Check on Sales Voucher
    let sales_vch_id: String = sqlx::query_scalar(
        "SELECT id FROM vouchers WHERE company_id = ? AND voucher_type_id = 'vt_sales' LIMIT 1",
    )
    .bind(cid)
    .fetch_one(&pool)
    .await
    .unwrap();

    let pdf_bytes = generate_invoice_pdf_bytes(&pool, cid, &sales_vch_id)
        .await
        .unwrap();
    assert!(
        !pdf_bytes.is_empty(),
        "Generated invoice PDF bytes must not be empty"
    );
    assert!(
        pdf_bytes.starts_with(b"%PDF"),
        "Must be a valid PDF document"
    );
}

#[tokio::test]
async fn test_multi_company_isolation_between_sample_and_user_company() {
    let (_dir, pool) = setup_test_pool().await;

    // Create Sample Company
    let sample = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();

    // Create independent User Company
    let user_comp = CompanyService::create_company(
        &pool,
        CreateCompanyRequest {
            name: "Independent Manufacturing Pvt Ltd".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Maharashtra".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    // Verify voucher counts are isolated
    let sample_vouchers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id = ?")
            .bind(&sample.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let user_vouchers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id = ?")
            .bind(&user_comp.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(
        sample_vouchers >= 5,
        "Sample company must have posted vouchers"
    );
    assert_eq!(user_vouchers, 0, "User company must have 0 vouchers");

    // Verify ledger isolation
    let sample_ledgers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ledgers WHERE company_id = ?")
            .bind(&sample.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let user_ledgers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ledgers WHERE company_id = ?")
        .bind(&user_comp.id)
        .fetch_one(&pool)
        .await
        .unwrap();

    // User company only has the 2 reserved ledgers
    assert_eq!(user_ledgers, 2);
    assert!(sample_ledgers > 10);
}
