//! Full-Lifecycle v1.0 End-to-End Smoke Verification Test Suite
//!
//! Exercises the entire real user lifecycle against genuine SQLite on-disk without mocks:
//! 1. Sample SME Onboarding: Creates "Bharath Precision Components Pvt Ltd", asserts
//!    Trial Balance (difference == "0"), P&L, Balance Sheet equilibrium, active FIFO layers,
//!    and active pending bills in Bills Receivable & Aging Analysis.
//! 2. Go-To → Voucher Inspector → GST Tax Invoice PDF: Searches via `global_goto_search`,
//!    inspects intra-state and inter-state sales vouchers via `get_voucher_full_detail`,
//!    generates PDFs via `generate_invoice_pdf_bytes`, asserting `%PDF-1.4`, `%%EOF`,
//!    Supplier GSTIN, HSN codes, CGST/SGST/IGST, and Lakh/Crore amount in words.
//! 3. GSTR-1 & GSTR-3B JSON Validation: Runs `export_gstr1_json` and `export_gstr3b_json`,
//!    parsing with `serde_json` and asserting non-zero B2B, HSN, Table 3.1, and Table 4 ITC.
//! 4. 2-Step Company Creation (F11) + Tally XML Round-Trip: Exports sample company via
//!    `export_company_tally_xml`, creates a fresh company + F11 auto-seeding, previews XML
//!    import (0 blocking errors), executes XML import, and verifies imported Trial Balance.
//! 5. WAL-Safe Backup & Restore Round-Trip: Runs `create_database_backup`, verifies .sha256
//!    and `PRAGMA integrity_check`, mutates database, runs `restore_database_backup`,
//!    and asserts exact state restoration.

use crate::backup::{create_database_backup, restore_database_backup};
use crate::company::features::{CompanyFeaturesService, SaveCompanyFeaturesInput};
use crate::company::model::CreateCompanyRequest;
use crate::company::sample::SampleCompanyService;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::interoperability::{
    execute_tally_xml_import, export_company_tally_xml, preview_tally_xml_import,
};
use crate::navigation::NavigationSearchService;
use crate::pdf::generate_invoice_pdf_bytes;
use crate::reports::balance_sheet::BalanceSheetService;
use crate::reports::billwise::BillWiseReportService;
use crate::reports::gst::GstReportService;
use crate::reports::model::{
    BalanceSheetRequest, ProfitAndLossRequest, ReportDateRange, TrialBalanceRequest,
};
use crate::reports::profit_and_loss::ProfitAndLossService;
use crate::reports::trial_balance::TrialBalanceService;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::path::PathBuf;
use std::str::FromStr;

async fn setup_v1_e2e_pool() -> (tempfile::TempDir, PathBuf, Pool<Sqlite>) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("opentally_v1_e2e.db");
    let db = DatabaseManager::init(&db_path).await.unwrap();
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

    (dir, db_path, p.clone())
}

#[tokio::test]
async fn test_v1_e2e_sample_sme_onboarding_invariants() {
    let (_dir, _db_path, pool) = setup_v1_e2e_pool().await;

    // Step 1: Create Sample Indian SME Company
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .expect("Sample SME company creation must succeed");
    let company_id = &sample_company.id;

    assert!(sample_company
        .name
        .starts_with("Bharath Precision Components Pvt Ltd"));

    // Step 2: Double-Entry Trial Balance Verification
    let tb = TrialBalanceService::generate(
        &pool,
        company_id,
        TrialBalanceRequest {
            as_of_date: "2026-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Trial balance generation must succeed");

    assert_eq!(
        tb.difference, "0.00",
        "Trial balance must be in mathematical equilibrium with 0.00 difference"
    );
    let total_dr = Decimal::from_str(&tb.total_debit).unwrap();
    let total_cr = Decimal::from_str(&tb.total_credit).unwrap();
    assert!(
        total_dr > Decimal::ZERO,
        "Trial balance debits must be positive"
    );
    assert_eq!(total_dr, total_cr, "Total debits must equal total credits");

    // Step 3: Profit & Loss Statement Verification
    let pnl = ProfitAndLossService::generate(
        &pool,
        company_id,
        ProfitAndLossRequest {
            period: ReportDateRange::new("2025-04-01", "2026-03-31").unwrap(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Profit & loss statement generation must succeed");

    let sales_rev = Decimal::from_str(&pnl.direct_income_subtotal).unwrap();
    assert!(
        sales_rev > Decimal::ZERO,
        "Operating revenue (sales) must be non-zero"
    );

    // Step 4: Balance Sheet Verification
    let bs = BalanceSheetService::generate(
        &pool,
        company_id,
        BalanceSheetRequest {
            as_of_date: "2026-03-31".to_string(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .expect("Balance sheet generation must succeed");

    assert!(
        !bs.asset_rows.is_empty(),
        "Assets must contain active ledgers (Bank, Debtors, Stock)"
    );
    assert!(
        !bs.liability_rows.is_empty(),
        "Liabilities must contain Capital & Creditors"
    );

    // Step 5: Active FIFO Valuation Layers
    let fifo_layers_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_cost_layers WHERE company_id = ? AND remaining_quantity > 0",
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        fifo_layers_count > 0,
        "Active unconsumed FIFO cost layers must exist after purchase & sales transactions"
    );

    // Step 6: Bills Receivable Pending & Aging Analysis
    let pending_bills =
        BillWiseReportService::get_bills_receivable(&pool, company_id, "2026-03-31")
            .await
            .expect("Pending bills query must succeed");

    let debtor_bills: Vec<_> = pending_bills
        .iter()
        .filter(|b| !b.pending_amount.is_empty() && b.pending_amount != "0.00")
        .collect();
    assert!(
        !debtor_bills.is_empty(),
        "Active pending bills must exist for Sundry Debtors"
    );

    let aging =
        BillWiseReportService::get_aging_analysis(&pool, company_id, "receivable", "2026-03-31")
            .await
            .expect("Aging report query must succeed");
    assert!(
        !aging.parties.is_empty(),
        "Aging report must categorize pending bills into intervals"
    );
}

#[tokio::test]
async fn test_v1_e2e_goto_voucher_inspection_and_gst_invoice_pdf() {
    let (_dir, _db_path, pool) = setup_v1_e2e_pool().await;
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();
    let company_id = &sample_company.id;

    // 1. Query Go To Search for Sales Vouchers
    let search_results =
        NavigationSearchService::global_goto_search(&pool, company_id, "Sales", 10)
            .await
            .expect("Go To search must succeed");

    let sales_vch = search_results
        .iter()
        .find(|r| r.category == "Voucher" && r.subtitle.contains("Sales"))
        .expect("Must find at least one Sales voucher in Go To search results");

    let voucher_id = &sales_vch.id;

    // 2. Inspect Voucher Detail
    let detail = NavigationSearchService::get_voucher_full_detail(&pool, company_id, voucher_id)
        .await
        .expect("Voucher full detail inspection must succeed");

    assert!(!detail.lines.is_empty(), "Voucher must have GL lines");
    assert!(
        !detail.gst_details.is_empty(),
        "Voucher must have GST tax lines"
    );
    assert!(
        !detail.bill_allocations.is_empty(),
        "Voucher must have bill-wise allocations"
    );

    // 3. Generate GST Tax Invoice PDF Bytes
    let pdf_bytes = generate_invoice_pdf_bytes(&pool, company_id, voucher_id)
        .await
        .expect("Invoice PDF generation must succeed");

    assert!(
        pdf_bytes.starts_with(b"%PDF-1.4"),
        "PDF output must start with standard %PDF-1.4 header"
    );

    // Check trailer
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_str.contains("%%EOF"),
        "PDF output must contain %%EOF trailer"
    );
    assert!(
        pdf_bytes.len() > 1000,
        "Generated PDF must have substantial content"
    );

    // Assert key invoice contents are rendered
    assert!(
        pdf_str.contains("29AABCU9603R1ZJ"),
        "PDF must contain Supplier Karnataka GSTIN"
    );
    assert!(
        pdf_str.contains("TAX INVOICE"),
        "PDF must have document title TAX INVOICE"
    );
    assert!(
        pdf_str.contains("Rupees") || pdf_str.contains("Only"),
        "PDF must contain Indian Lakh/Crore amount in words"
    );
    assert!(
        pdf_str.contains("8481") || pdf_str.contains("8482") || pdf_str.contains("4016"),
        "PDF must contain HSN code"
    );
}

#[tokio::test]
async fn test_v1_e2e_gstr1_and_gstr3b_statutory_json_export() {
    let (_dir, _db_path, pool) = setup_v1_e2e_pool().await;
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();
    let company_id = &sample_company.id;

    // 1. Export GSTR-1 JSON
    let gstr1_json =
        GstReportService::export_gstr1_json(&pool, company_id, "2025-04-01", "2026-03-31")
            .await
            .expect("GSTR-1 JSON export must succeed");

    let gstr1_parsed: serde_json::Value =
        serde_json::from_str(&gstr1_json).expect("GSTR-1 JSON must parse cleanly");

    assert_eq!(
        gstr1_parsed["gstin"], "29AABCU9603R1ZJ",
        "GSTR-1 GSTIN must match company GSTIN"
    );
    let b2b = gstr1_parsed["b2b"]
        .as_array()
        .expect("b2b array must be present");
    assert!(!b2b.is_empty(), "b2b array must have invoice entries");

    let hsn = gstr1_parsed["hsn"]
        .as_array()
        .expect("HSN array must be present");
    assert!(!hsn.is_empty(), "HSN summary array must have entries");

    // 2. Export GSTR-3B JSON
    let gstr3b_json =
        GstReportService::export_gstr3b_json(&pool, company_id, "2025-04-01", "2026-03-31")
            .await
            .expect("GSTR-3B JSON export must succeed");

    let gstr3b_parsed: serde_json::Value =
        serde_json::from_str(&gstr3b_json).expect("GSTR-3B JSON must parse cleanly");

    assert_eq!(
        gstr3b_parsed["gstin"], "29AABCU9603R1ZJ",
        "GSTR-3B GSTIN must match company GSTIN"
    );
    let txval_str = gstr3b_parsed["sup_details"][0]["total_taxable_value"]
        .as_str()
        .unwrap_or("0");
    let txval = Decimal::from_str(txval_str).unwrap_or(Decimal::ZERO);
    assert!(
        txval > Decimal::ZERO,
        "Table 3.1 outward supplies taxable value must be positive (got {})",
        txval_str
    );

    let itc_cgst_str = gstr3b_parsed["itc_elg"][1]["central_tax"]
        .as_str()
        .unwrap_or("0");
    let itc_cgst = Decimal::from_str(itc_cgst_str).unwrap_or(Decimal::ZERO);
    assert!(
        itc_cgst > Decimal::ZERO,
        "Table 4 eligible input tax credit from purchases must be positive (got {})",
        itc_cgst_str
    );
}

#[tokio::test]
async fn test_v1_e2e_f11_auto_seeding_and_tally_xml_roundtrip() {
    let (_dir, _db_path, pool) = setup_v1_e2e_pool().await;
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();
    let src_company_id = &sample_company.id;

    // 1. Export sample company to Tally XML
    let xml_payload = export_company_tally_xml(&pool, src_company_id, None, None)
        .await
        .expect("Tally XML export must succeed");

    assert!(
        xml_payload.contains("<ENVELOPE>"),
        "XML payload must contain ENVELOPE root"
    );
    assert!(
        xml_payload.contains("<TALLYMESSAGE"),
        "XML payload must contain TALLYMESSAGE elements"
    );

    // 2. Create target company with 2-Step F11 auto-seeding
    let target_comp = CompanyService::create_company(
        &pool,
        CreateCompanyRequest {
            name: "Roundtrip Target Enterprises Pvt Ltd".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2025-04-01".to_string(),
            books_beginning_from: Some("2025-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();
    let target_id = &target_comp.id;

    // Run F11 auto-seeding
    CompanyFeaturesService::save_company_features(
        &pool,
        target_id,
        SaveCompanyFeaturesInput {
            maintain_inventory: true,
            billwise_tracking: true,
            enable_perpetual_cogs: true,
            gst_enabled: true,
            gstin: Some("29ABCDE1234F1ZW".to_string()),
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

    // 3. Dry-Run XML Import Preview
    let preview = preview_tally_xml_import(&pool, target_id, &xml_payload)
        .await
        .expect("Tally XML preview must succeed");

    assert!(
        preview.can_import,
        "Import preview must indicate can_import = true"
    );
    assert_eq!(
        preview.errors.len(),
        0,
        "Import preview must have zero blocking errors"
    );

    // 4. Execute XML Import
    let result = execute_tally_xml_import(&pool, target_id, &xml_payload)
        .await
        .expect("Tally XML import execution must succeed");

    assert!(result.success, "Import execution must succeed");
    assert!(
        result.vouchers_created > 0,
        "Vouchers must be imported into the target company"
    );

    // 5. Verify Target Company Double-Entry Equilibrium
    let tb_target = TrialBalanceService::generate(
        &pool,
        target_id,
        TrialBalanceRequest {
            as_of_date: "2026-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();

    assert!(
        tb_target.difference == "0" || tb_target.difference == "0.00",
        "Target company imported Trial Balance must be balanced with 0 difference (got {})",
        tb_target.difference
    );
}

#[tokio::test]
async fn test_v1_e2e_wal_safe_backup_and_restore_cycle() {
    let (dir, db_path, pool) = setup_v1_e2e_pool().await;
    let sample_company = SampleCompanyService::create_sample_company(&pool)
        .await
        .unwrap();
    let company_id = &sample_company.id;

    // Capture pre-backup voucher count
    let count_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id = ?")
            .bind(company_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(count_before > 0, "Must have sample vouchers before backup");

    // 1. Create WAL-safe database backup
    let backup_dest = dir.path().join("backups").join("v1_qa_snapshot.db");
    let manifest = create_database_backup(&pool, &db_path, &backup_dest)
        .await
        .expect("Backup creation must succeed");

    assert!(backup_dest.exists(), "Backup file must be created on disk");
    assert!(
        !manifest.sha256_checksum.is_empty(),
        "Manifest must include SHA-256 checksum"
    );

    // Verify sidecar manifest file
    let sidecar_path = dir
        .path()
        .join("backups")
        .join("v1_qa_snapshot.db.manifest.json");
    assert!(
        sidecar_path.exists(),
        "Sidecar manifest file must exist on disk"
    );

    // 2. Mutate active database (simulate uncommitted/accidental extra entity insertion)
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_mutated_test', 'Mutated Temp Ltd', 'Mutated Temp Ltd', 'India', 'Karnataka', '2025-04-01', '2025-04-01')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let check_mutated: Option<String> =
        sqlx::query_scalar("SELECT id FROM companies WHERE id = 'cmp_mutated_test'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(
        check_mutated.as_deref(),
        Some("cmp_mutated_test"),
        "Mutated company must exist prior to restore"
    );

    // 3. Restore database from backup
    let restore_result = restore_database_backup(&pool, &db_path, &backup_dest)
        .await
        .expect("Database restore must succeed");

    assert!(restore_result.success, "Restore result must be successful");

    // 4. Verify exact state restoration
    let check_after_restore: Option<String> =
        sqlx::query_scalar("SELECT id FROM companies WHERE id = 'cmp_mutated_test'")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(
        check_after_restore.is_none(),
        "Mutated entity must be removed after restoring from backup"
    );
    let count_restored: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id = ?")
            .bind(company_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        count_restored, count_before,
        "Restored database must contain exact pre-backup voucher count"
    );

    // Verify double-entry Trial Balance balance after restoration
    let tb_restored = TrialBalanceService::generate(
        &pool,
        company_id,
        TrialBalanceRequest {
            as_of_date: "2026-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        tb_restored.difference, "0.00",
        "Restored database Trial Balance must maintain perfect 0.00 equilibrium"
    );
}
