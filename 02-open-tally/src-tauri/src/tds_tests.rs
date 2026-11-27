use crate::commercial::{
    CommercialDocumentType, CommercialLinePosting, CommercialPostingInput,
    CommercialPostingService, TaxPostingLine,
};
use crate::company::features::{CompanyFeaturesService, SaveCompanyFeaturesInput};
use crate::company::model::CreateCompanyRequest;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::reports::tds::TdsReportService;
use crate::tds::models::*;
use crate::tds::service::TdsService;
use crate::tds::validators::{validate_pan, validate_tan};
use crate::vouchers::model::CancelVoucherRequest;
use crate::vouchers::service::VoucherCancellationService;
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

#[test]
fn test_tan_validation() {
    assert!(validate_tan("BLRP12345A").is_ok());
    assert!(validate_tan("MUMP09876B").is_ok());
    assert!(validate_tan("DELA54321C").is_ok());

    // Invalid format
    assert!(validate_tan("BLRP123456").is_err()); // 6 digits
    assert!(validate_tan("BLR12345A").is_err()); // 3 letters
    assert!(validate_tan("BLRP1234AA").is_err()); // 2 trailing letters
    assert!(validate_tan("").is_err());
}

#[test]
fn test_pan_validation() {
    // 4th char entity types: P (Individual), C (Company), H (HUF), F (Firm), A (AOP), T (Trust), B (BOI), L (Local), J (AJP), G (Govt)
    assert!(validate_pan("ABCCD1234F").is_ok()); // Company 'C'
    assert!(validate_pan("AAAPH1234K").is_ok()); // HUF 'H'
    assert!(validate_pan("BKRPP5678Q").is_ok()); // Individual 'P'
    assert!(validate_pan("AAATF9999Z").is_ok()); // Firm 'F'

    // Invalid 4th char
    assert!(validate_pan("ABCDX1234F").is_err()); // 'X' is invalid entity
    assert!(validate_pan("ABCDE1234F").is_err()); // 'D' is invalid entity
    assert!(validate_pan("12345ABCDE").is_err());
    assert!(validate_pan("ABCDE12345").is_err());
    assert!(validate_pan("ABCDE123").is_err());
}

#[test]
fn test_due_date_computation() {
    assert_eq!(
        TdsReportService::compute_tds_due_date("2026-05-15"),
        "2026-06-07"
    );
    assert_eq!(
        TdsReportService::compute_tds_due_date("2026-12-01"),
        "2027-01-07"
    );
    assert_eq!(
        TdsReportService::compute_tds_due_date("2027-03-25"),
        "2027-04-30"
    );
}

#[test]
fn test_quarter_bounds() {
    let (s, e) = TdsReportService::get_quarter_date_bounds("2026-2027", "Q1").unwrap();
    assert_eq!(s, "2026-04-01");
    assert_eq!(e, "2026-06-30");

    let (s4, e4) = TdsReportService::get_quarter_date_bounds("2026-27", "Q4").unwrap();
    assert_eq!(s4, "2027-01-01");
    assert_eq!(e4, "2027-03-31");
}

async fn setup_tds_test_db() -> (
    tempfile::TempDir,
    DatabaseManager,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("tds_test.db"))
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

    // 1. Company via CompanyService (seeds all predefined groups and voucher types)
    let comp = CompanyService::create_company(
        p,
        CreateCompanyRequest {
            name: "Apex Electronics Pvt Ltd".to_string(),
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
    let company_id = comp.id;

    // 2. F11 Features
    CompanyFeaturesService::save_company_features(
        p,
        &company_id,
        SaveCompanyFeaturesInput {
            maintain_inventory: true,
            billwise_tracking: true,
            enable_perpetual_cogs: true,
            gst_enabled: true,
            gstin: Some("27AABCA1234A1ZJ".to_string()),
            state_code: Some("27".to_string()),
            registration_type: Some("Regular".to_string()),
            auto_seed_gst_ledgers: true,
            auto_seed_inventory_ledgers: true,
            tds_enabled: Some(true),
            tan: Some("BLRP12345A".to_string()),
            deductor_type: Some("Company".to_string()),
            auto_seed_tds_defaults: true,
            enable_payroll: Some(false),
            auto_seed_payroll_ledgers: false,
            enable_multi_currency: Some(false),
            auto_seed_currency_defaults: false,
        },
    )
    .await
    .unwrap();

    // 3. Voucher Series for Purchase
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('series_pur_tds', ?, 'vt_pur', 'Purchase TDS', 'PUR', 'automatic', 'yearly')",
    )
    .bind(&company_id)
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_pur_tds', ?, 'vt_pur', 'series_pur_tds', '2026-04-01', 'PUR/26/', '', 1, 4)",
    )
    .bind(&company_id)
    .execute(p)
    .await
    .unwrap();

    // 4. Ledgers: Professional Fees (Expense), Apex Consultant (Creditor)
    let exp_ledger_id = "led_prof_fees".to_string();
    let supp_ledger_id = "led_supp_consultant".to_string();
    let supp_party_id = "party_supp_consultant".to_string();

    let indirect_exp_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Indirect Expenses' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(p)
    .await
    .unwrap();

    let creditors_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'Professional Fees', 0, 'standard')",
    )
    .bind(&exp_ledger_id)
    .bind(&company_id)
    .bind(&indirect_exp_grp)
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'TechServe Solutions', 0, 'standard')",
    )
    .bind(&supp_ledger_id)
    .bind(&company_id)
    .bind(&creditors_grp)
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO party_profiles (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled, credit_period_days, credit_limit)
         VALUES (?, ?, ?, 'SUPP-001', 'TechServe Solutions', 0, 1, 30, '500000')",
    )
    .bind(&supp_party_id)
    .bind(&company_id)
    .bind(&supp_ledger_id)
    .execute(p)
    .await
    .unwrap();

    // Supplier PAN profile under 194J
    TdsService::save_ledger_tds_profile(
        p,
        SaveLedgerTdsProfileInput {
            company_id: company_id.clone(),
            ledger_id: supp_ledger_id.clone(),
            pan: Some("ABCCD1234F".to_string()),
            deductee_type: "Company_Domestic".to_string(),
            lower_deduction_cert_no: None,
            lower_deduction_rate: None,
        },
        Some("test_user"),
    )
    .await
    .unwrap();

    // Tax ledgers (CGST, SGST are auto-seeded by ensure_gst_ledgers_and_mappings)
    let cgst_ledger_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Input CGST' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(p)
    .await
    .unwrap();

    let sgst_ledger_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Input SGST' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(p)
    .await
    .unwrap();

    (
        dir,
        db,
        company_id,
        exp_ledger_id,
        supp_ledger_id,
        supp_party_id,
        "series_pur_tds".to_string(),
        cgst_ledger_id,
        sgst_ledger_id,
    )
}

#[tokio::test]
async fn test_preview_tds_calculation_threshold_and_higher_rate() {
    let (_dir, db, company_id, _exp_id, supp_ledger_id, _party_id, _series_id, _cgst, _sgst) =
        setup_tds_test_db().await;
    let pool = db.pool();

    // 1. Transaction under single threshold for 194C (Threshold = ₹30,000)
    let preview_under = TdsService::preview_tds_calculation(
        pool,
        TdsCalculationPreviewInput {
            company_id: company_id.clone(),
            party_ledger_id: supp_ledger_id.clone(),
            section_code: "194C".to_string(),
            taxable_amount: "25000.00".to_string(),
            voucher_date: "2026-05-10".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(!preview_under.threshold_exceeded);
    assert_eq!(preview_under.tds_amount, "0.00");

    // 2. Transaction exceeding single threshold for 194C (₹35,000 > ₹30,000)
    let preview_over = TdsService::preview_tds_calculation(
        pool,
        TdsCalculationPreviewInput {
            company_id: company_id.clone(),
            party_ledger_id: supp_ledger_id.clone(),
            section_code: "194C".to_string(),
            taxable_amount: "35000.00".to_string(),
            voucher_date: "2026-05-10".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(preview_over.threshold_exceeded);
    // Company deductee under 194C = 2% of 35000 = 700.00
    assert_eq!(preview_over.applicable_rate, "2.00");
    assert_eq!(preview_over.tds_amount, "700.00");

    // 3. Section 206AA Higher Rate when PAN is missing
    let party_no_pan_ledger = "led_supp_no_pan".to_string();
    let creditors_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'Vendor Without PAN', 0, 'standard')",
    )
    .bind(&party_no_pan_ledger)
    .bind(&company_id)
    .bind(&creditors_grp)
    .execute(pool)
    .await
    .unwrap();

    TdsService::save_ledger_tds_profile(
        pool,
        SaveLedgerTdsProfileInput {
            company_id: company_id.clone(),
            ledger_id: party_no_pan_ledger.clone(),
            pan: None,
            deductee_type: "Company_Domestic".to_string(),
            lower_deduction_cert_no: None,
            lower_deduction_rate: None,
        },
        Some("test_user"),
    )
    .await
    .unwrap();

    let preview_missing_pan = TdsService::preview_tds_calculation(
        pool,
        TdsCalculationPreviewInput {
            company_id: company_id.clone(),
            party_ledger_id: party_no_pan_ledger.clone(),
            section_code: "194J".to_string(),
            taxable_amount: "100000.00".to_string(),
            voucher_date: "2026-05-15".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(preview_missing_pan.is_pan_missing_higher_rate);
    assert_eq!(preview_missing_pan.applicable_rate, "20.00");
    // 20% on 100,000 = 20,000.00
    assert_eq!(preview_missing_pan.tds_amount, "20000.00");
}

#[tokio::test]
async fn test_f9_purchase_with_gst_and_tds_atomic_posting() {
    let (
        _dir,
        db,
        company_id,
        exp_id,
        supp_ledger_id,
        party_id,
        series_id,
        cgst_ledger_id,
        sgst_ledger_id,
    ) = setup_tds_test_db().await;
    let pool = db.pool();

    // Purchase Invoice: Taxable ₹1,00,000, 18% GST (CGST ₹9,000 + SGST ₹9,000)
    // TDS under 194J (10% on GST-exclusive ₹1,00,000) = ₹10,000
    // Party Net Payable = (100,000 + 18,000) - 10,000 = 108,000.00
    let input = CommercialPostingInput {
        company_id: company_id.clone(),
        document_type: CommercialDocumentType::Purchase,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id,
        date: "2026-05-20".to_string(),
        due_date: Some("2026-06-20".to_string()),
        party_id,
        party_ledger_id: supp_ledger_id.clone(),
        commercial_account_ledger_id: Some(exp_id),
        total_amount: Decimal::from(100000), // GST-exclusive taxable
        document_reference: Some("INV-TDS-001".to_string()),
        narration: Some("Consulting services with 18% GST and 10% TDS".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            description: "Software Architecture Consulting".to_string(),
            stock_item_id: None,
            unit_id: None,
            godown_id: None,
            quantity: Decimal::ONE,
            rate: Decimal::from(100000),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::from(100000),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: cgst_ledger_id,
                rate: Decimal::from(9),
                amount: Decimal::from(9000),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: sgst_ledger_id,
                rate: Decimal::from(9),
                amount: Decimal::from(9000),
            },
        ],
        bill_allocations: vec![], // Will auto-derive from net total
        place_of_supply_state_code: Some("27".to_string()),
        is_reverse_charge: Some(false),
        allow_tax_override: Some(true),
        original_voucher_id: None,
        return_reason: None,
        tds_section_code: Some("194J".to_string()),
        supply_category: None,
        itc_eligibility: None,
        port_code: None,
        shipping_bill_no: None,
        shipping_bill_date: None,
        allow_post_cutoff_note: None,
    };

    let result = CommercialPostingService::post_commercial_voucher(pool, &company_id, input)
        .await
        .unwrap();

    let voucher = result.voucher;
    assert_eq!(voucher.voucher.status, "posted");

    // Verify GL lines
    // 1. Dr Expense 100,000
    // 2. Dr CGST 9,000
    // 3. Dr SGST 9,000
    // 4. Cr TDS Payable 10,000
    // 5. Cr Party 108,000
    let mut total_dr = Decimal::ZERO;
    let mut total_cr = Decimal::ZERO;
    let mut found_tds_line = false;
    let mut party_cr_amount = Decimal::ZERO;

    for line in &voucher.lines {
        let amt = Decimal::from_str(&line.amount).unwrap();
        if line.entry_type == "Dr" {
            total_dr += amt;
        } else {
            total_cr += amt;
            if line.ledger_id == supp_ledger_id {
                party_cr_amount = amt;
            }
            if line.narration.as_deref().unwrap_or("").contains("TDS") {
                found_tds_line = true;
                assert_eq!(amt, Decimal::from(10000));
            }
        }
    }

    assert_eq!(total_dr, total_cr);
    assert_eq!(total_dr, Decimal::from(118000));
    assert_eq!(party_cr_amount, Decimal::from(108000));
    assert!(found_tds_line, "TDS line must be present in voucher lines");

    // Verify Bill-Wise Pending Bill amount is net payable (108,000.00)
    let bill_amount: String = sqlx::query_scalar(
        "SELECT amount FROM bill_allocations WHERE company_id = ? AND party_ledger_id = ?",
    )
    .bind(&company_id)
    .bind(&supp_ledger_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        Decimal::from_str(&bill_amount).unwrap(),
        Decimal::from(108000)
    );

    // Verify voucher_tds_details
    let tds_details = sqlx::query_as::<_, VoucherTdsDetail>(
        "SELECT * FROM voucher_tds_details WHERE voucher_id = ?",
    )
    .bind(&voucher.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(tds_details.section_code, "194J");
    assert_eq!(tds_details.gross_amount, "100000.00");
    assert_eq!(tds_details.tds_amount, "10000.00");
    assert_eq!(tds_details.is_paid, 0);

    // Verify TDS Outstanding Report
    let outstanding = TdsReportService::get_tds_outstanding(pool, &company_id, "2026-05-31")
        .await
        .unwrap();
    assert_eq!(outstanding.total_unpaid_tds, "10000.00");
    assert_eq!(outstanding.sections.len(), 1);
    assert_eq!(outstanding.sections[0].vouchers.len(), 1);
    assert_eq!(outstanding.sections[0].vouchers[0].due_date, "2026-06-07");

    // Step 4: Challan Payment Tagging
    TdsService::record_tds_challan_payment(
        pool,
        RecordTdsChallanPaymentInput {
            company_id: company_id.clone(),
            voucher_tds_detail_ids: vec![tds_details.id.clone()],
            challan_bsr_code: "0210012".to_string(),
            challan_date: "2026-06-05".to_string(),
            challan_serial_no: "00042".to_string(),
            payment_voucher_id: None,
            actor_id: Some("test_user".to_string()),
        },
    )
    .await
    .unwrap();

    // Verify is_paid updated to 1
    let is_paid: i64 = sqlx::query_scalar("SELECT is_paid FROM voucher_tds_details WHERE id = ?")
        .bind(&tds_details.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(is_paid, 1);

    // Verify Cancellation Guardrail: cannot cancel paid TDS voucher
    let cancel_res = VoucherCancellationService::cancel_voucher(
        pool,
        &company_id,
        CancelVoucherRequest {
            voucher_id: voucher.voucher.id.clone(),
            reason: "Attempt cancellation of paid TDS voucher".to_string(),
            actor_id: Some("test_user".to_string()),
        },
    )
    .await;
    assert!(cancel_res.is_err());
    match cancel_res.err().unwrap() {
        OpenTallyError::Validation(msg) => {
            assert!(msg.contains("remitted"));
        }
        other => panic!("Expected Validation, got {:?}", other),
    }

    // Form 26Q Summary
    let f26q = TdsReportService::get_form_26q_summary(pool, &company_id, "2026-2027", "Q1")
        .await
        .unwrap();
    assert_eq!(f26q.deductor_tan.as_deref().unwrap(), "BLRP12345A");
    assert_eq!(f26q.deductions.len(), 1);
    assert_eq!(f26q.total_amount_paid, "100000.00");
    assert_eq!(f26q.total_tds_deducted, "10000.00");
    assert_eq!(f26q.total_tds_deposited, "10000.00");
    assert_eq!(f26q.challans.len(), 1);
    assert_eq!(f26q.challans[0].challan_bsr_code, "0210012");

    // Form 26Q JSON Export
    let json_export = TdsReportService::export_form_26q_json(pool, &company_id, "2026-2027", "Q1")
        .await
        .unwrap();
    assert!(json_export.contains("BLRP12345A"));
    assert!(json_export.contains("10000.00"));

    // Form 26Q CSV Export (CWE-1236 check)
    let csv_export = TdsReportService::export_form_26q_csv(pool, &company_id, "2026-2027", "Q1")
        .await
        .unwrap();
    assert!(csv_export.contains("BLRP12345A"));
    assert!(csv_export.contains("0210012"));
}
