use crate::commercial::{
    CommercialDocumentType, CommercialLinePosting, CommercialPostingInput,
    CommercialPostingService, TaxPostingLine,
};
use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::gst::gstn_validator::{iso_to_gstn_date, validate_gstr1_gstn_schema};
use crate::gst::GstService;
use crate::reports::gst::GstReportService;
use rust_decimal::Decimal;
use std::str::FromStr;
use tempfile::TempDir;
use uuid::Uuid;

async fn setup_m19_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m19_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("init db");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M19 GSTN Corp".to_string(),
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
    .expect("create company");

    GstService::save_company_gst_profile(
        db.pool(),
        &comp.id,
        Some("27AABCU9603R1ZN"),
        "27",
        "Regular",
        true,
    )
    .await
    .expect("save gst profile");

    (dir, db, comp.id)
}

async fn seed_party_and_series(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
) -> (String, String, String, String, String, String) {
    let dr_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Debtors' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let cr_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let sal_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sales Accounts' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let pur_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Purchase Accounts' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let cust_led = Uuid::new_v4().to_string();
    let supp_led = Uuid::new_v4().to_string();
    let sal_led = Uuid::new_v4().to_string();
    let pur_led = Uuid::new_v4().to_string();

    for (id, grp, name) in [
        (&cust_led, &dr_grp, "Global Buyer"),
        (&supp_led, &cr_grp, "Legal Consultant"),
        (&sal_led, &sal_grp, "Sales Account"),
        (&pur_led, &pur_grp, "Purchase Account"),
    ] {
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES (?, ?, ?, ?, 0, 'standard')",
        )
        .bind(id)
        .bind(company_id)
        .bind(grp)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    let sal_ser = Uuid::new_v4().to_string();
    let pur_ser = Uuid::new_v4().to_string();
    let cn_ser = Uuid::new_v4().to_string();

    for (sid, vtype, scode, sname, pfx) in [
        (&sal_ser, "vt_sales", "SAL", "Sales Series", "SAL/"),
        (&pur_ser, "vt_pur", "PUR", "Purchase Series", "PUR/"),
        (&cn_ser, "vt_credit_note", "CN", "Credit Note Series", "CN/"),
    ] {
        sqlx::query(
            "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_code, series_name, numbering_method, restart_period)
             VALUES (?, ?, ?, ?, ?, 'automatic', 'yearly')",
        )
        .bind(sid)
        .bind(company_id)
        .bind(vtype)
        .bind(scode)
        .bind(sname)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
             VALUES (?, ?, ?, ?, '2026-04-01', ?, '', 1, 4)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(vtype)
        .bind(sid)
        .bind(pfx)
        .execute(pool)
        .await
        .unwrap();
    }

    (cust_led, supp_led, sal_led, pur_led, sal_ser, pur_ser)
}

#[tokio::test]
async fn test_m19_rcm_purchase_credits_supplier_taxable_only_and_posts_balanced_rcm_lines() {
    let (_dir, db, company_id) = setup_m19_db().await;
    let (_cust, supp_led, _sal, pur_led, _sal_ser, pur_ser) =
        seed_party_and_series(db.pool(), &company_id).await;

    let res = CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Purchase,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: pur_ser,
            date: "2026-05-10".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: supp_led.clone(),
            commercial_account_ledger_id: Some(pur_led.clone()),
            total_amount: Decimal::from_str("10000.00").unwrap(),
            document_reference: Some("RCM-01".to_string()),
            narration: Some("Legal services under RCM".to_string()),
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("10000.00").unwrap(),
                line_amount: Decimal::from_str("10000.00").unwrap(),
                description: "Legal fee".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![TaxPostingLine {
                tax_type: "IGST".to_string(),
                tax_ledger_id: pur_led.clone(),
                rate: Decimal::from_str("18.00").unwrap(),
                amount: Decimal::from_str("1800.00").unwrap(),
            }],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(true),
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
        },
    )
    .await
    .expect("RCM purchase");

    // Supplier credited ONLY 10000.00 (not 11800.00)
    let supp_line = res
        .voucher
        .lines
        .iter()
        .find(|l| l.ledger_id == supp_led)
        .expect("supplier line");
    assert_eq!(supp_line.amount, "10000");

    // Balanced RCM lines (Dr Input RCM ITC 1800.00, Cr Output RCM Liability 1800.00)
    let rcm_lines: Vec<(String, String)> = sqlx::query_as(
        "SELECT vl.entry_type, vl.amount FROM voucher_lines vl
         JOIN ledgers l ON l.id = vl.ledger_id
         WHERE vl.voucher_id = ? AND l.name LIKE '%RCM%'",
    )
    .bind(&res.voucher.voucher.id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(rcm_lines.len(), 2);
    assert!(rcm_lines.iter().any(|(d, a)| d == "Dr" && a == "1800.00"));
    assert!(rcm_lines.iter().any(|(d, a)| d == "Cr" && a == "1800.00"));

    // GSTR-3B Table 3.1(d) and Table 4(A)(3)
    let gstr3b =
        GstReportService::get_gstr3b_summary(db.pool(), &company_id, "2026-05-01", "2026-05-31")
            .await
            .unwrap();
    let row_31d = gstr3b
        .table_3_1
        .iter()
        .find(|r| r.nature_of_supplies.contains("(d)"))
        .unwrap();
    assert_eq!(row_31d.total_taxable_value, "10000.00");
    assert_eq!(row_31d.integrated_tax, "1800.00");
}

#[tokio::test]
async fn test_m19_exports_lut_and_tax_route_to_table_6a_and_3_1_b() {
    let (_dir, db, company_id) = setup_m19_db().await;
    let (cust_led, _supp, sal_led, _pur, sal_ser, _pur_ser) =
        seed_party_and_series(db.pool(), &company_id).await;

    // ExportWithLUT (WOPAY -> 0.00 tax)
    CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser.clone(),
            date: "2026-05-12".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led.clone(),
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("50000.00").unwrap(),
            document_reference: Some("EXP-LUT-1".to_string()),
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("50000.00").unwrap(),
                line_amount: Decimal::from_str("50000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("96".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: Some("ExportWithLUT".to_string()),
            itc_eligibility: None,
            port_code: Some("INBOM4".to_string()),
            shipping_bill_no: Some("SB12345".to_string()),
            shipping_bill_date: Some("2026-05-12".to_string()),
            allow_post_cutoff_note: None,
        },
    )
    .await
    .expect("Export LUT");

    // ExportWithTax (WPAY -> IGST)
    CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser,
            date: "2026-05-14".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led,
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("20000.00").unwrap(),
            document_reference: Some("EXP-TAX-1".to_string()),
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("20000.00").unwrap(),
                line_amount: Decimal::from_str("20000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![TaxPostingLine {
                tax_type: "IGST".to_string(),
                tax_ledger_id: sal_led.clone(),
                rate: Decimal::from_str("18.00").unwrap(),
                amount: Decimal::from_str("3600.00").unwrap(),
            }],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("96".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(true),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: Some("ExportWithTax".to_string()),
            itc_eligibility: None,
            port_code: Some("INMAA1".to_string()),
            shipping_bill_no: Some("SB99887".to_string()),
            shipping_bill_date: Some("2026-05-14".to_string()),
            allow_post_cutoff_note: None,
        },
    )
    .await
    .expect("Export With Tax");

    let gstr1 =
        GstReportService::get_gstr1_summary(db.pool(), &company_id, "2026-05-01", "2026-05-31")
            .await
            .unwrap();
    assert_eq!(gstr1.exports_table_6a.len(), 2);
    assert!(gstr1
        .exports_table_6a
        .iter()
        .any(|e| e.exp_typ == "WOPAY" && e.taxable_value == "50000.00"));
    assert!(gstr1
        .exports_table_6a
        .iter()
        .any(|e| e.exp_typ == "WPAY" && e.igst_amount == "3600.00"));
    assert!(!gstr1.doc_issue_table_13.is_empty());

    let gstr3b =
        GstReportService::get_gstr3b_summary(db.pool(), &company_id, "2026-05-01", "2026-05-31")
            .await
            .unwrap();
    let row_31b = gstr3b
        .table_3_1
        .iter()
        .find(|r| r.nature_of_supplies.contains("(b)"))
        .unwrap();
    assert_eq!(row_31b.total_taxable_value, "70000.00");
    assert_eq!(row_31b.integrated_tax, "3600.00");
}

#[tokio::test]
async fn test_m19_composition_dealer_blocks_gst_on_sales_and_computes_cmp08() {
    let (_dir, db, company_id) = setup_m19_db().await;
    let (cust_led, _supp, sal_led, _pur, sal_ser, _pur_ser) =
        seed_party_and_series(db.pool(), &company_id).await;

    GstService::save_company_gst_profile(
        db.pool(),
        &company_id,
        Some("27AABCU9603R1ZN"),
        "27",
        "Composition",
        true,
    )
    .await
    .unwrap();

    // Reject charging GST on Sales for Composition dealer
    let err = CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser.clone(),
            date: "2026-05-15".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led.clone(),
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("10000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("10000.00").unwrap(),
                line_amount: Decimal::from_str("10000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![TaxPostingLine {
                tax_type: "IGST".to_string(),
                tax_ledger_id: sal_led.clone(),
                rate: Decimal::from_str("18.00").unwrap(),
                amount: Decimal::from_str("1800.00").unwrap(),
            }],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
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
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Composition dealer"));

    // Valid Composition Sale (0.00 tax)
    CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser,
            date: "2026-05-15".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led,
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("100000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("100000.00").unwrap(),
                line_amount: Decimal::from_str("100000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        },
    )
    .await
    .expect("Composition sale");

    let cmp08 = GstReportService::get_cmp08_quarterly_summary(db.pool(), &company_id, 2026, 1)
        .await
        .unwrap();
    assert_eq!(cmp08.outward_turnover, "100000.00");
    assert_eq!(cmp08.composition_tax_payable, "1000.00");
    assert_eq!(cmp08.total_cmp08_liability, "1000.00");
}

#[tokio::test]
async fn test_m19_ineligible_sec17_5_and_rule_42_43_itc_segregation() {
    let (_dir, db, company_id) = setup_m19_db().await;
    let (_cust, supp_led, _sal, pur_led, _sal_ser, pur_ser) =
        seed_party_and_series(db.pool(), &company_id).await;

    for (elig, amt) in [
        ("IneligibleSec17_5", "900.00"),
        ("Rule42_43Reversed", "450.00"),
    ] {
        CommercialPostingService::post_commercial_voucher(
            db.pool(),
            &company_id,
            CommercialPostingInput {
                company_id: company_id.clone(),
                document_type: CommercialDocumentType::Purchase,
                source_document_id: Uuid::new_v4().to_string(),
                source_voucher_id: None,
                series_id: pur_ser.clone(),
                date: "2026-05-18".to_string(),
                due_date: None,
                party_id: Uuid::new_v4().to_string(),
                party_ledger_id: supp_led.clone(),
                commercial_account_ledger_id: Some(pur_led.clone()),
                total_amount: Decimal::from_str("5000.00").unwrap(),
                document_reference: None,
                narration: None,
                lines: vec![CommercialLinePosting {
                    line_number: 1,
                    stock_item_id: None,
                    godown_id: None,
                    unit_id: None,
                    discount_amount: Decimal::ZERO,
                    batch_number: None,
                    mfg_date: None,
                    quantity: Decimal::ONE,
                    rate: Decimal::from_str("5000.00").unwrap(),
                    line_amount: Decimal::from_str("5000.00").unwrap(),
                    description: "Item".to_string(),
                    expiry_date: None,
                    billed_quantity: None,
                    entered_unit_id: None,
                }],
                tax_lines: vec![TaxPostingLine {
                    tax_type: "IGST".to_string(),
                    tax_ledger_id: pur_led.clone(),
                    rate: Decimal::from_str("18.00").unwrap(),
                    amount: Decimal::from_str(amt).unwrap(),
                }],
                bill_allocations: vec![],
                place_of_supply_state_code: Some("27".to_string()),
                is_reverse_charge: Some(false),
                allow_tax_override: Some(true),
                original_voucher_id: None,
                return_reason: None,
                tds_section_code: None,
                supply_category: None,
                itc_eligibility: Some(elig.to_string()),
                port_code: None,
                shipping_bill_no: None,
                shipping_bill_date: None,
                allow_post_cutoff_note: None,
            },
        )
        .await
        .unwrap();
    }

    let gstr3b =
        GstReportService::get_gstr3b_summary(db.pool(), &company_id, "2026-05-01", "2026-05-31")
            .await
            .unwrap();
    let rev = gstr3b
        .table_4_itc
        .iter()
        .find(|r| r.itc_details.contains("(B)"))
        .unwrap();
    let inel = gstr3b
        .table_4_itc
        .iter()
        .find(|r| r.itc_details.contains("(D)"))
        .unwrap();
    assert_eq!(rev.integrated_tax, "450.00");
    assert_eq!(inel.integrated_tax, "900.00");
    assert_eq!(gstr3b.total_eligible_itc.integrated_tax, "0.00");
}

#[tokio::test]
async fn test_m19_sec34_2_cross_fy_credit_note_cutoff_guard() {
    let (_dir, db, company_id) = setup_m19_db().await;
    let (cust_led, _supp, sal_led, _pur, sal_ser, _pur_ser) =
        seed_party_and_series(db.pool(), &company_id).await;

    let inv = CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser.clone(),
            date: "2026-04-10".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led.clone(),
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("10000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("10000.00").unwrap(),
                line_amount: Decimal::from_str("10000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        },
    )
    .await
    .unwrap();

    let cn_ser: String = sqlx::query_scalar(
        "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = 'vt_credit_note' LIMIT 1",
    )
    .bind(&company_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    // After Nov 30, 2027 (FY 2026-27 cutoff is 2027-11-30) -> rejected by default
    let err = CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::CreditNote,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: cn_ser.clone(),
            date: "2027-12-05".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led.clone(),
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("2000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("2000.00").unwrap(),
                line_amount: Decimal::from_str("2000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: Some(inv.voucher.voucher.id.clone()),
            return_reason: Some("Late return".to_string()),
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: Some(false),
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Sec 34(2)"));

    // Override with allow_post_cutoff_note: true -> succeeds & logs in audit_log
    CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::CreditNote,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: cn_ser,
            date: "2027-12-05".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led,
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("2000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                stock_item_id: None,
                godown_id: None,
                unit_id: None,
                discount_amount: Decimal::ZERO,
                batch_number: None,
                mfg_date: None,
                quantity: Decimal::ONE,
                rate: Decimal::from_str("2000.00").unwrap(),
                line_amount: Decimal::from_str("2000.00").unwrap(),
                description: "Item".to_string(),
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: Some(inv.voucher.voucher.id),
            return_reason: Some("Late return".to_string()),
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: Some(true),
        },
    )
    .await
    .expect("override allowed");

    let audit_cnt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = ? AND entity_type = 'gst_sec34_cutoff_override'",
    )
    .bind(&company_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(audit_cnt, 1);
}

#[tokio::test]
async fn test_m19_gstn_schema_validator_dates_and_doc_num() {
    assert_eq!(iso_to_gstn_date("2026-05-12"), "12-05-2026");

    let valid_payload = serde_json::json!({
        "gstin": "27AABCU9603R1ZN",
        "fp": "052026",
        "b2b": [{
            "ctin": "29ABCDE1234F1ZW",
            "inv": [{
                "inum": "INV/26-27/001",
                "idt": "12-05-2026",
                "val": "11800.00",
                "pos": "29",
                "itms": [{
                    "itm_det": {
                        "txval": "10000.00",
                        "camt": "0.00",
                        "samt": "0.00",
                        "iamt": "1800.00",
                        "csamt": "0.00"
                    }
                }]
            }]
        }],
        "doc_issue": { "doc_det": [] }
    });
    let rep = validate_gstr1_gstn_schema(&valid_payload);
    assert!(rep.is_valid, "Expected valid, got {:?}", rep.errors);

    let invalid_payload = serde_json::json!({
        "gstin": "27AABCU9603R1ZN",
        "fp": "052026",
        "b2b": [{
            "ctin": "29ABCDE1234F1ZW",
            "inv": [{
                "inum": "INVOICE-NUMBER-LONGER-THAN-16",
                "idt": "2026-05-12",
                "val": "11800.00",
                "pos": "29",
                "itms": []
            }]
        }]
    });
    let rep_bad = validate_gstr1_gstn_schema(&invalid_payload);
    assert!(!rep_bad.is_valid);
    assert!(rep_bad.error_count >= 2);
}
