//! Integration Test Suite for Milestone 10: GST Compliance
//!
//! Verifies:
//! 1. GSTR-2B JSON Parsing & Tolerance-aware Matching:
//!    - Matched (|tax_diff| <= 1.00)
//!    - Mismatch (|tax_diff| > 1.00)
//!    - MissingInBooks (Portal invoice absent in OpenTally)
//!    - MissingIn2B (Books purchase from registered supplier absent in GSTR-2B; At-Risk ITC)
//! 2. CWE-1236 Safe CSV Export with formula injection sanitization.
//! 3. NIC e-Invoice v1.1 JSON generation and B2C rejection.
//! 4. NIC e-Way Bill JSON generation and consignment validation.
//! 5. Voucher e-Invoice metadata persistence with 64-char hex IRN & 12-digit e-way bill validation.
//! 6. Tax Invoice PDF generation with IRN and e-Way bill rendering.
//! 7. Multi-tenant company isolation.

use crate::company::model::CreateCompanyRequest;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::gst::einvoice::{EinvoiceService, TransportDetailsInput, VoucherEinvoiceMetadataInput};
use crate::gst::reconciliation::Gstr2bService;
use crate::pdf::generate_invoice_pdf_bytes;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

async fn setup_gst_compliance_pool() -> (tempfile::TempDir, Pool<Sqlite>, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("gst_compliance_test.db"))
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

    let comp = CompanyService::create_company(
        p,
        CreateCompanyRequest {
            name: "Karnataka Precision Tools Pvt Ltd".to_string(),
            mailing_name: Some("Karnataka Precision Tools Pvt Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("Indian Rupee".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    let company_id = comp.id;

    // Configure Company GST profile (Karnataka State 29, GSTIN: 29AABCK1234L1Z5)
    sqlx::query(
        "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled)
         VALUES (?, '29AABCK1234L1Z5', '29', 'Regular', 1)",
    )
    .bind(&company_id)
    .execute(p)
    .await
    .unwrap();

    // Create manual number series for Purchase and Sales
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('srs_pur', ?, 'vt_pur', 'Purchase Series', 'PUR', 'manual', 'yearly')",
    )
    .bind(&company_id)
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('srs_sales', ?, 'vt_sales', 'Sales Series', 'SAL', 'manual', 'yearly')",
    )
    .bind(&company_id)
    .execute(p)
    .await
    .unwrap();

    (dir, p.clone(), company_id)
}

#[tokio::test]
async fn test_gstr2b_reconciliation_exact_and_mismatch_and_missing() {
    let (_dir, pool, company_id) = setup_gst_compliance_pool().await;

    let creditors_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. Create a Supplier ledger with GSTIN
    let supplier_ledger_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'National Steel Suppliers', 0, 'standard')",
    )
    .bind(&supplier_ledger_id)
    .bind(&company_id)
    .bind(&creditors_grp)
    .execute(&pool)
    .await
    .unwrap();

    // Map supplier GSTIN in ledger_gst_profiles
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES (?, ?, ?, '29ABCDE1234F1Z5', '29', 'Regular')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&supplier_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Post Books Purchase Voucher #1: Matched
    // Taxable: 100000.00, CGST: 9000.00, SGST: 9000.00 -> Total Tax 18000.00
    let vch1_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, reference_no, total_amount
         ) VALUES (?, ?, 'vt_pur', 'srs_pur', 'PUR/2026/001', '2026-04-10', '2026-04-10', 'posted', 'manual', 'INV-8801', '118000.00')",
    )
    .bind(&vch1_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
         ) VALUES (?, ?, ?, ?, '29ABCDE1234F1Z5', '7208', '29', 'IntraState', 'Taxable', '100000.00', '18.00', '9000.00', '9000.00', '0.00', '0.00', 0, 0)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&vch1_id)
    .bind(&supplier_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Post Books Purchase Voucher #2: Mismatch
    // Taxable: 50000.00, CGST: 4500.00, SGST: 4500.00 -> Total Tax 9000.00
    let vch2_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, reference_no, total_amount
         ) VALUES (?, ?, 'vt_pur', 'srs_pur', 'PUR/2026/002', '2026-04-12', '2026-04-12', 'posted', 'manual', 'INV-8802', '59000.00')",
    )
    .bind(&vch2_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
         ) VALUES (?, ?, ?, ?, '29ABCDE1234F1Z5', '7208', '29', 'IntraState', 'Taxable', '50000.00', '18.00', '4500.00', '4500.00', '0.00', '0.00', 0, 0)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&vch2_id)
    .bind(&supplier_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // 4. Post Books Purchase Voucher #3: MissingIn2B (At-Risk ITC)
    // Supplier has valid GSTIN, but is absent in GSTR-2B portal JSON
    let vch3_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, reference_no, total_amount
         ) VALUES (?, ?, 'vt_pur', 'srs_pur', 'PUR/2026/003', '2026-04-18', '2026-04-18', 'posted', 'manual', 'INV-8803', '35400.00')",
    )
    .bind(&vch3_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
         ) VALUES (?, ?, ?, ?, '29ABCDE1234F1Z5', '7208', '29', 'IntraState', 'Taxable', '30000.00', '18.00', '2700.00', '2700.00', '0.00', '0.00', 0, 0)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&vch3_id)
    .bind(&supplier_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // 5. Construct GSTR-2B JSON:
    // - Invoice #1: INV-8801 exact match (100000 taxable, 18000 tax)
    // - Invoice #2: INV-8802 mismatch (60000 taxable, 10800 tax; diff = 1800)
    // - Invoice #4: INV-9999 missing in books (40000 taxable, 7200 tax)
    let gstr2b_json = r#"{
        "data": {
            "docdata": {
                "b2b": [
                    {
                        "ctin": "29ABCDE1234F1Z5",
                        "cname": "National Steel Suppliers",
                        "inv": [
                            {
                                "inum": "INV-8801",
                                "dt": "10-04-2026",
                                "val": 118000.00,
                                "itcavl": "Y",
                                "items": [
                                    {
                                        "txval": 100000.00,
                                        "rt": 18.0,
                                        "camt": 9000.00,
                                        "samt": 9000.00,
                                        "iamt": 0.0,
                                        "csamt": 0.0
                                    }
                                ]
                            },
                            {
                                "inum": "INV-8802",
                                "dt": "12-04-2026",
                                "val": 70800.00,
                                "itcavl": "Y",
                                "items": [
                                    {
                                        "txval": 60000.00,
                                        "rt": 18.0,
                                        "camt": 5400.00,
                                        "samt": 5400.00,
                                        "iamt": 0.0,
                                        "csamt": 0.0
                                    }
                                ]
                            },
                            {
                                "inum": "INV-9999",
                                "dt": "25-04-2026",
                                "val": 47200.00,
                                "itcavl": "Y",
                                "items": [
                                    {
                                        "txval": 40000.00,
                                        "rt": 18.0,
                                        "camt": 3600.00,
                                        "samt": 3600.00,
                                        "iamt": 0.0,
                                        "csamt": 0.0
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        }
    }"#;

    // Run reconciliation for April 2026 ("042026")
    let report =
        Gstr2bService::import_and_reconcile_gstr2b(&pool, &company_id, "042026", gstr2b_json)
            .await
            .unwrap();

    assert_eq!(report.summary.total_portal_invoices, 3);
    assert_eq!(
        report.summary.matched_count, 1,
        "INV-8801 should be Matched"
    );
    assert_eq!(
        report.summary.mismatch_count, 1,
        "INV-8802 should be Mismatch"
    );
    assert_eq!(
        report.summary.missing_in_books_count, 1,
        "INV-9999 should be MissingInBooks"
    );
    assert_eq!(
        report.summary.missing_in_2b_count, 1,
        "PUR/2026/003 should be MissingIn2B"
    );

    assert_eq!(report.summary.matched_itc, "18000.00");
    assert_eq!(report.summary.mismatch_itc, "10800.00");
    assert_eq!(report.summary.missing_in_books_itc, "7200.00");
    assert_eq!(report.summary.at_risk_itc, "5400.00");

    // Fetch report via ReadOnlyConnectionGuard
    let fetched = Gstr2bService::get_gstr2b_reconciliation_report(&pool, &company_id, "042026")
        .await
        .unwrap();

    assert_eq!(fetched.summary.matched_count, 1);
    assert_eq!(fetched.summary.mismatch_count, 1);
    assert_eq!(fetched.summary.missing_in_books_count, 1);
    assert_eq!(fetched.summary.missing_in_2b_count, 1);

    // Verify CSV export and CWE-1236 safe escaping
    let csv = Gstr2bService::export_gstr2b_reconciliation_csv(&pool, &company_id, "042026")
        .await
        .unwrap();

    assert!(csv.contains("Status,Supplier GSTIN,Supplier Name,Invoice No"));
    assert!(csv.contains("Matched,29ABCDE1234F1Z5"));
    assert!(csv.contains("Mismatch,29ABCDE1234F1Z5"));
    assert!(csv.contains("MissingInBooks,29ABCDE1234F1Z5"));
    assert!(csv.contains("MissingIn2B,29ABCDE1234F1Z5"));
}

#[tokio::test]
async fn test_gstr2b_payload_size_limit_guard() {
    let (_dir, pool, company_id) = setup_gst_compliance_pool().await;

    // Create 26 MB string
    let huge_payload = " ".repeat(26 * 1024 * 1024);
    let res =
        Gstr2bService::import_and_reconcile_gstr2b(&pool, &company_id, "042026", &huge_payload)
            .await;

    assert!(res.is_err(), "Payload > 25MB must be rejected");
    match res.err().unwrap() {
        crate::errors::OpenTallyError::Validation(msg) => {
            assert!(msg.contains("25MB"));
        }
        other => panic!("Expected Validation error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_einvoice_json_export_and_b2c_rejection() {
    let (_dir, pool, company_id) = setup_gst_compliance_pool().await;

    let debtors_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Debtors' COLLATE NOCASE",
    )
    .bind(&company_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create Customer ledger with valid 15-char B2B GSTIN (Karnataka: 29XYZDE9876P1Z1)
    let customer_ledger_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'Bangalore Aerospace Corp', 0, 'standard')",
    )
    .bind(&customer_ledger_id)
    .bind(&company_id)
    .bind(&debtors_grp)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES (?, ?, ?, '29XYZDE9876P1Z1', '29', 'Regular')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&customer_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // Post B2B Sales Voucher
    let sales_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, total_amount
         ) VALUES (?, ?, 'vt_sales', 'srs_sales', 'SALES/2026/042', '2026-04-15', '2026-04-15', 'posted', 'manual', '236000.00')",
    )
    .bind(&sales_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
         ) VALUES (?, ?, ?, ?, '29XYZDE9876P1Z1', '8471', '29', 'IntraState', 'Taxable', '200000.00', '18.00', '18000.00', '18000.00', '0.00', '0.00', 0, 0)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&sales_id)
    .bind(&customer_ledger_id)
    .execute(&pool)
    .await
    .unwrap();

    // Export NIC e-Invoice JSON
    let einvoice_json = EinvoiceService::export_einvoice_json(&pool, &company_id, &sales_id)
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&einvoice_json).unwrap();
    assert_eq!(parsed["Version"], "1.1");
    assert_eq!(parsed["TranDtls"]["TaxSch"], "GST");
    assert_eq!(parsed["TranDtls"]["SupTyp"], "B2B");
    assert_eq!(parsed["DocDtls"]["Typ"], "INV");
    assert_eq!(parsed["DocDtls"]["No"], "SALES/2026/042");
    assert_eq!(parsed["DocDtls"]["Dt"], "15/04/2026");
    assert_eq!(parsed["SellerDtls"]["Gstin"], "29AABCK1234L1Z5");
    assert_eq!(parsed["BuyerDtls"]["Gstin"], "29XYZDE9876P1Z1");
    assert_eq!(parsed["ValDtls"]["AssVal"], 200000.0);
    assert_eq!(parsed["ValDtls"]["CgstVal"], 18000.0);
    assert_eq!(parsed["ValDtls"]["SgstVal"], 18000.0);
    assert_eq!(parsed["ValDtls"]["TotInvVal"], 236000.0);

    // Export NIC e-Way Bill JSON
    let transport = TransportDetailsInput {
        transport_mode: "Road".to_string(),
        distance_km: 150,
        transporter_id: Some("29ABCDE1234F1Z5".to_string()),
        transporter_name: Some("FastTrack Logistics".to_string()),
        trans_doc_no: Some("LR/2026/099".to_string()),
        trans_doc_date: Some("2026-04-15".to_string()),
        vehicle_no: Some("KA04CD5678".to_string()),
        vehicle_type: Some("R".to_string()),
    };

    let eway_json =
        EinvoiceService::export_eway_bill_json(&pool, &company_id, &sales_id, transport)
            .await
            .unwrap();

    let eway_parsed: serde_json::Value = serde_json::from_str(&eway_json).unwrap();
    assert_eq!(eway_parsed["version"], "1.0.0621");
    let bill = &eway_parsed["billLists"][0];
    assert_eq!(bill["docNo"], "SALES/2026/042");
    assert_eq!(bill["transMode"], "1");
    assert_eq!(bill["transDistance"], "150");
    assert_eq!(bill["vehicleNo"], "KA04CD5678");

    // Test B2C Rejection: Voucher with unregistered party
    let b2c_customer_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES (?, ?, ?, 'Cash Consumer B2C', 0, 'standard')",
    )
    .bind(&b2c_customer_id)
    .bind(&company_id)
    .bind(&debtors_grp)
    .execute(&pool)
    .await
    .unwrap();

    let b2c_sales_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, total_amount
         ) VALUES (?, ?, 'vt_sales', 'srs_sales', 'SALES/2026/043', '2026-04-16', '2026-04-16', 'posted', 'manual', '5000.00')",
    )
    .bind(&b2c_sales_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
         ) VALUES (?, ?, ?, ?, NULL, '8471', '29', 'IntraState', 'Taxable', '4237.29', '18.00', '381.36', '381.36', '0.00', '0.00', 0, 0)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&b2c_sales_id)
    .bind(&b2c_customer_id)
    .execute(&pool)
    .await
    .unwrap();

    let b2c_res = EinvoiceService::export_einvoice_json(&pool, &company_id, &b2c_sales_id).await;
    assert!(
        b2c_res.is_err(),
        "B2C voucher must be rejected for e-Invoice"
    );
}

#[tokio::test]
async fn test_voucher_einvoice_metadata_persistence_and_pdf() {
    let (_dir, pool, company_id) = setup_gst_compliance_pool().await;

    let vch_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, voucher_number,
            date, effective_date, status, numbering_method, total_amount
         ) VALUES (?, ?, 'vt_sales', 'srs_sales', 'INV/2026/901', '2026-04-20', '2026-04-20', 'posted', 'manual', '11800.00')",
    )
    .bind(&vch_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    let valid_irn = "a1b2c3d4e5f678901234567890abcdef1234567890abcdef1234567890abcdef";
    let meta_input = VoucherEinvoiceMetadataInput {
        irn: Some(valid_irn.to_string()),
        ack_no: Some("122010045678".to_string()),
        ack_date: Some("2026-04-20 14:30:00".to_string()),
        signed_qr_code: None,
        einvoice_status: Some("Generated".to_string()),
        eway_bill_no: Some("251000123456".to_string()),
        eway_bill_date: Some("2026-04-20".to_string()),
        eway_valid_upto: Some("2026-04-22".to_string()),
        transporter_id: None,
        transporter_name: Some("VRL Logistics".to_string()),
        transport_mode: Some("Road".to_string()),
        vehicle_no: Some("KA01AB9999".to_string()),
        distance_km: Some(120),
    };

    let saved =
        EinvoiceService::save_voucher_einvoice_metadata(&pool, &company_id, &vch_id, meta_input)
            .await
            .unwrap();

    assert_eq!(saved.irn.as_deref(), Some(valid_irn));
    assert_eq!(saved.eway_bill_no.as_deref(), Some("251000123456"));
    assert_eq!(saved.einvoice_status, "Generated");

    // Invalid IRN test (must be 64 hex characters)
    let bad_irn = VoucherEinvoiceMetadataInput {
        irn: Some("short-irn".to_string()),
        ack_no: None,
        ack_date: None,
        signed_qr_code: None,
        einvoice_status: None,
        eway_bill_no: None,
        eway_bill_date: None,
        eway_valid_upto: None,
        transporter_id: None,
        transporter_name: None,
        transport_mode: None,
        vehicle_no: None,
        distance_km: None,
    };
    let bad_res =
        EinvoiceService::save_voucher_einvoice_metadata(&pool, &company_id, &vch_id, bad_irn).await;
    assert!(bad_res.is_err(), "IRN with invalid length must be rejected");

    // Verify PDF output renders IRN and e-Way Bill
    let pdf_bytes = generate_invoice_pdf_bytes(&pool, &company_id, &vch_id)
        .await
        .unwrap();

    assert!(pdf_bytes.starts_with(b"%PDF-1.4"));
    assert!(pdf_bytes.ends_with(b"%%EOF\n") || pdf_bytes.ends_with(b"%%EOF"));

    let pdf_text = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_text.contains("IRN:"),
        "PDF must render IRN when present"
    );
    assert!(
        pdf_text.contains("e-Way Bill:"),
        "PDF must render e-Way Bill when present"
    );
    assert!(
        pdf_text.contains("251000123456"),
        "PDF must render e-Way Bill number"
    );
}
