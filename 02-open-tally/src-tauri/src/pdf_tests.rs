use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::pdf::words::{format_indian_currency, number_to_indian_words};
use crate::pdf::{
    export_invoice_pdf_file, generate_invoice_pdf, generate_invoice_pdf_bytes, InvoiceHsnTaxRow,
    InvoicePdfItemLine, InvoicePdfModel,
};
use rust_decimal::Decimal;
use std::str::FromStr;

#[test]
fn test_number_to_indian_words_edge_cases() {
    // Zero
    assert_eq!(number_to_indian_words(Decimal::ZERO), "Rupees Zero Only");

    // Pure paise
    assert_eq!(
        number_to_indian_words(Decimal::from_str("0.75").unwrap()),
        "Seventy Five Paise Only"
    );

    // Single digit
    assert_eq!(
        number_to_indian_words(Decimal::from_str("5.00").unwrap()),
        "Rupees Five Only"
    );

    // Hundred and tens
    assert_eq!(
        number_to_indian_words(Decimal::from_str("125.00").unwrap()),
        "Rupees One Hundred Twenty Five Only"
    );

    // Thousands and lakhs with paise
    assert_eq!(
        number_to_indian_words(Decimal::from_str("125000.50").unwrap()),
        "Rupees One Lakh Twenty Five Thousand and Fifty Paise Only"
    );

    // One Crore
    assert_eq!(
        number_to_indian_words(Decimal::from_str("10000000.00").unwrap()),
        "Rupees One Crore Only"
    );

    // Complex Crore + Lakh + Thousand + Hundred + Paise
    assert_eq!(
        number_to_indian_words(Decimal::from_str("12345678.90").unwrap()),
        "Rupees One Crore Twenty Three Lakh Forty Five Thousand Six Hundred Seventy Eight and Ninety Paise Only"
    );
}

#[test]
fn test_format_indian_currency() {
    assert_eq!(format_indian_currency(Decimal::ZERO), "0.00");
    assert_eq!(
        format_indian_currency(Decimal::from_str("999.00").unwrap()),
        "999.00"
    );
    assert_eq!(
        format_indian_currency(Decimal::from_str("1000.00").unwrap()),
        "1,000.00"
    );
    assert_eq!(
        format_indian_currency(Decimal::from_str("125000.50").unwrap()),
        "1,25,000.50"
    );
    assert_eq!(
        format_indian_currency(Decimal::from_str("10000000.00").unwrap()),
        "1,00,00,000.00"
    );
    assert_eq!(
        format_indian_currency(Decimal::from_str("-5000.25").unwrap()),
        "-5,000.25"
    );
}

#[test]
fn test_generate_invoice_pdf_structure() {
    let model = InvoicePdfModel {
        title: "TAX INVOICE".to_string(),
        voucher_id: "v_test_01".to_string(),
        voucher_number: "INV/2026/001".to_string(),
        voucher_date: "2026-05-15".to_string(),
        original_invoice_no: None,
        original_invoice_date: None,
        place_of_supply: "State Code 07 (Delhi)".to_string(),
        is_reverse_charge: false,
        supplier_name: "Acme Enterprises Pvt Ltd".to_string(),
        supplier_address: "123 Business Hub, Connaught Place, New Delhi".to_string(),
        supplier_gstin: "07AAAAA0000A1Z5".to_string(),
        supplier_state: "Delhi".to_string(),
        supplier_state_code: "07".to_string(),
        buyer_name: "Global Traders LLP".to_string(),
        buyer_gstin: "07BBBBB1111B1Z2".to_string(),
        buyer_state: "Delhi".to_string(),
        buyer_state_code: "07".to_string(),
        items: vec![InvoicePdfItemLine {
            sl_no: 1,
            description: "Widget Model Alpha".to_string(),
            hsn_sac: "8471".to_string(),
            quantity: Decimal::from(10),
            unit: "NOS".to_string(),
            rate: Decimal::from(1500),
            discount: Decimal::ZERO,
            taxable_value: Decimal::from(15000),
            gst_rate: Decimal::from(18),
            tax_amount: Decimal::from(2700),
            total: Decimal::from(17700),
        }],
        hsn_summary: vec![InvoiceHsnTaxRow {
            hsn_sac: "8471".to_string(),
            taxable_value: Decimal::from(15000),
            cgst_rate: Decimal::from(9),
            cgst_amount: Decimal::from(1350),
            sgst_rate: Decimal::from(9),
            sgst_amount: Decimal::from(1350),
            igst_rate: Decimal::ZERO,
            igst_amount: Decimal::ZERO,
            cess_amount: Decimal::ZERO,
            total_tax: Decimal::from(2700),
        }],
        total_taxable: Decimal::from(15000),
        total_tax: Decimal::from(2700),
        grand_total: Decimal::from(17700),
        amount_in_words: number_to_indian_words(Decimal::from(17700)),
        irn: None,
        ack_no: None,
        ack_date: None,
        eway_bill_no: None,
        eway_valid_upto: None,
    };

    let bytes = generate_invoice_pdf(&model);
    assert!(!bytes.is_empty(), "Generated PDF bytes should not be empty");
    assert!(
        bytes.starts_with(b"%PDF-1.4"),
        "PDF must begin with %PDF-1.4 header"
    );
    assert!(
        bytes.ends_with(b"%%EOF\n") || bytes.ends_with(b"%%EOF"),
        "PDF must terminate with %%EOF marker"
    );

    let pdf_str = String::from_utf8_lossy(&bytes);
    assert!(pdf_str.contains("/F1"), "PDF should reference font /F1");
    assert!(pdf_str.contains("/F2"), "PDF should reference font /F2");
    assert!(
        pdf_str.contains("/Type /Page"),
        "PDF should contain page dictionary"
    );
}

#[tokio::test]
async fn test_export_invoice_pdf_file_and_read_only_guard() {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("pdf_test.db"))
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

    // Seed test company & voucher
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_pdf', 'PDF Test Corp', 'PDF Corp Ltd', 'India', 'Delhi', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled)
         VALUES ('cmp_pdf', '07AAAAA1234A1Z5', '07', 'Regular', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca', 'cmp_pdf', 'Current Assets', NULL, 'Assets', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_sales', 'cmp_pdf', 'vt_sales', 'Sales Series', 'SAL', 'manual', 'yearly')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, numbering_method, total_amount)
         VALUES ('v_pdf_01', 'cmp_pdf', 'vt_sales', 's_sales', 'INV-2026-001', '2026-05-10', '2026-05-10', 'posted', 'manual', '11800.00')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_p1', 'cmp_pdf', 'grp_ca', 'Alpha Buyer Ltd', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, place_of_supply_state_code,
            supply_type, taxability, is_reverse_charge, hsn_sac_code, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount
         ) VALUES (
            'vgd_01', 'cmp_pdf', 'v_pdf_01', 'led_p1', '07BBBBB5678B1Z9', '07',
            'InterState', 'Taxable', 0, '8471', '10000.00', '18.00', '900.00', '900.00', '0.00', '0.00'
         )",
    )
    .execute(p)
    .await
    .unwrap();

    // 1. Generate bytes
    let bytes = generate_invoice_pdf_bytes(p, "cmp_pdf", "v_pdf_01")
        .await
        .unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF-1.4"));

    // 2. Export to disk
    let output_pdf = dir.path().join("invoices").join("tax_invoice_001.pdf");
    let res_path = export_invoice_pdf_file(p, "cmp_pdf", "v_pdf_01", output_pdf.to_str().unwrap())
        .await
        .unwrap();
    assert!(std::path::Path::new(&res_path).exists());
    let disk_bytes = std::fs::read(&res_path).unwrap();
    assert_eq!(bytes, disk_bytes);

    // 3. Multi-company tenant isolation check
    let bad_company = generate_invoice_pdf_bytes(p, "other_company", "v_pdf_01").await;
    match bad_company {
        Err(OpenTallyError::NotFound(_)) => {}
        other => panic!("Expected NotFound for foreign company, got {:?}", other),
    }
}
