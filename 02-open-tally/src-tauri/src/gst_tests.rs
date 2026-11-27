use crate::commercial::{
    BillAllocationPostingLine, CommercialDocumentType, CommercialLinePosting,
    CommercialPostingInput, CommercialPostingService, TaxPostingLine,
};
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::gst::*;
use crate::reports::GstReportService;
use crate::vouchers::model::CancelVoucherRequest;
use crate::vouchers::service::VoucherCancellationService;
use rust_decimal::Decimal;
use uuid::Uuid;

#[test]
fn test_gstin_validator_known_valid_formats() {
    // 27AAPFU0939F1ZV -> Maharashtra valid GSTIN
    let prefix = "27AAPFU0939F1Z";
    let check = calculate_gstin_check_digit(prefix).unwrap();
    assert_eq!(check, 'V');
    assert!(validate_gstin("27AAPFU0939F1ZV", Some("27")).is_ok());

    // Karnataka: 29ABCDE1234F1Z
    let prefix_ka = "29ABCDE1234F1Z";
    let check_ka = calculate_gstin_check_digit(prefix_ka).unwrap();
    let full_ka = format!("{}{}", prefix_ka, check_ka);
    assert!(validate_gstin(&full_ka, Some("29")).is_ok());

    // Delhi: 07AAAAA0000A1Z
    let prefix_dl = "07AAAAA0000A1Z";
    let check_dl = calculate_gstin_check_digit(prefix_dl).unwrap();
    let full_dl = format!("{}{}", prefix_dl, check_dl);
    assert!(validate_gstin(&full_dl, Some("07")).is_ok());
}

#[test]
fn test_gstin_validator_invalid_formats() {
    // Wrong length
    assert!(validate_gstin("27AAPFU0939F1Z", None).is_err());
    assert!(validate_gstin("27AAPFU0939F1ZVV", None).is_err());

    // Invalid state code
    assert!(validate_gstin("00AAPFU0939F1ZV", None).is_err());
    assert!(validate_gstin("99AAPFU0939F1ZV", None).is_err());

    // State mismatch
    assert!(validate_gstin("27AAPFU0939F1ZV", Some("29")).is_err());

    // Corrupted check digit
    assert!(validate_gstin("27AAPFU0939F1ZA", Some("27")).is_err());
}

#[test]
fn test_calculate_gst_intra_state_vs_inter_state() {
    // 1. Intra-State (MH to MH): 18% GST on 1000.00
    let input_intra = GstCalculationInput {
        company_state_code: "27".to_string(),
        place_of_supply_state_code: "27".to_string(),
        taxability: Taxability::Taxable,
        taxable_value: Decimal::new(100000, 2), // 1000.00
        gst_rate: Decimal::new(1800, 2),        // 18.00%
        cess_rate: Decimal::ZERO,
        is_reverse_charge: false,
    };
    let res_intra = calculate_gst(&input_intra);
    assert_eq!(res_intra.supply_type, SupplyType::IntraState);
    assert_eq!(res_intra.cgst_amount, Decimal::new(9000, 2)); // 90.00
    assert_eq!(res_intra.sgst_amount, Decimal::new(9000, 2)); // 90.00
    assert_eq!(res_intra.igst_amount, Decimal::ZERO);
    assert_eq!(res_intra.total_tax, Decimal::new(18000, 2)); // 180.00
    assert_eq!(res_intra.grand_total, Decimal::new(118000, 2)); // 1180.00

    // 2. Inter-State (MH to KA): 18% GST + 12% Cess on 1000.00
    let input_inter = GstCalculationInput {
        company_state_code: "27".to_string(),
        place_of_supply_state_code: "29".to_string(),
        taxability: Taxability::Taxable,
        taxable_value: Decimal::new(100000, 2), // 1000.00
        gst_rate: Decimal::new(1800, 2),        // 18.00%
        cess_rate: Decimal::new(1200, 2),       // 12.00%
        is_reverse_charge: false,
    };
    let res_inter = calculate_gst(&input_inter);
    assert_eq!(res_inter.supply_type, SupplyType::InterState);
    assert_eq!(res_inter.cgst_amount, Decimal::ZERO);
    assert_eq!(res_inter.sgst_amount, Decimal::ZERO);
    assert_eq!(res_inter.igst_amount, Decimal::new(18000, 2)); // 180.00
    assert_eq!(res_inter.cess_amount, Decimal::new(12000, 2)); // 120.00
    assert_eq!(res_inter.total_tax, Decimal::new(30000, 2)); // 300.00
    assert_eq!(res_inter.grand_total, Decimal::new(130000, 2)); // 1300.00

    // 3. Exempt supply: 0 tax
    let input_exempt = GstCalculationInput {
        company_state_code: "27".to_string(),
        place_of_supply_state_code: "27".to_string(),
        taxability: Taxability::Exempt,
        taxable_value: Decimal::new(100000, 2),
        gst_rate: Decimal::new(1800, 2),
        cess_rate: Decimal::ZERO,
        is_reverse_charge: false,
    };
    let res_exempt = calculate_gst(&input_exempt);
    assert_eq!(res_exempt.total_tax, Decimal::ZERO);
    assert_eq!(res_exempt.grand_total, Decimal::new(100000, 2));
}

// -----------------------------------------------------------------------------
// Fixture Setup for GST Integration Tests
// -----------------------------------------------------------------------------

async fn setup_gst_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("gst_test.db"))
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

    seed_gst_fixture(p).await;
    (dir, db)
}

async fn seed_gst_fixture(p: &sqlx::Pool<sqlx::Sqlite>) {
    // 1. Companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_a', 'Company Alpha MH', 'Alpha Corp', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                ('cmp_b', 'Company Beta KA', 'Beta Corp', 'India', 'Karnataka', '2026-04-01', '2026-04-01')",
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
                ('grp_dt_a', 'cmp_a', 'Duties & Taxes', NULL, 'Liabilities', 1),
                ('grp_ca_b', 'cmp_b', 'Sundry Debtors', NULL, 'Assets', 1),
                ('grp_inc_b', 'cmp_b', 'Sales Accounts', NULL, 'Income', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // 3. Ledgers
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_cust_a', 'cmp_a', 'grp_ca_a', 'Customer MH Regular', 0, 'standard'),
                ('led_cust_inter', 'cmp_a', 'grp_ca_a', 'Customer KA Regular', 0, 'standard'),
                ('led_cust_b2c_large', 'cmp_a', 'grp_ca_a', 'Customer KA B2C Large', 0, 'standard'),
                ('led_cust_b2c_small', 'cmp_a', 'grp_ca_a', 'Customer MH B2C Small', 0, 'standard'),
                ('led_supp_a', 'cmp_a', 'grp_cl_a', 'Supplier MH Regular', 0, 'standard'),
                ('led_sales_a', 'cmp_a', 'grp_inc_a', 'Domestic Sales Account', 0, 'standard'),
                ('led_purchase_a', 'cmp_a', 'grp_exp_a', 'Domestic Purchase Account', 0, 'standard'),
                ('led_out_cgst', 'cmp_a', 'grp_dt_a', 'Output CGST', 0, 'standard'),
                ('led_out_sgst', 'cmp_a', 'grp_dt_a', 'Output SGST', 0, 'standard'),
                ('led_out_igst', 'cmp_a', 'grp_dt_a', 'Output IGST', 0, 'standard'),
                ('led_out_cess', 'cmp_a', 'grp_dt_a', 'Output Cess', 0, 'standard'),
                ('led_in_cgst', 'cmp_a', 'grp_dt_a', 'Input CGST', 0, 'standard'),
                ('led_in_sgst', 'cmp_a', 'grp_dt_a', 'Input SGST', 0, 'standard'),
                ('led_in_igst', 'cmp_a', 'grp_dt_a', 'Input IGST', 0, 'standard'),
                ('led_in_cess', 'cmp_a', 'grp_dt_a', 'Input Cess', 0, 'standard'),
                ('led_cust_b', 'cmp_b', 'grp_ca_b', 'Beta Customer', 0, 'standard'),
                ('led_sales_b', 'cmp_b', 'grp_inc_b', 'Beta Sales', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    // 4. Party Profiles
    sqlx::query(
        "INSERT INTO party_profiles (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled, credit_period_days, credit_limit)
         VALUES ('party_cust_a', 'cmp_a', 'led_cust_a', 'CUST-001', 'Acme Customer MH', 1, 0, 30, '100000'),
                ('party_cust_inter', 'cmp_a', 'led_cust_inter', 'CUST-002', 'Interstate Customer KA', 1, 0, 30, '500000'),
                ('party_cust_b2c_large', 'cmp_a', 'led_cust_b2c_large', 'CUST-003', 'Unregistered KA Customer', 1, 0, 30, '500000'),
                ('party_cust_b2c_small', 'cmp_a', 'led_cust_b2c_small', 'CUST-004', 'Unregistered MH Customer', 1, 0, 30, '100000'),
                ('party_supp_a', 'cmp_a', 'led_supp_a', 'SUPP-001', 'Acme Supplier MH', 0, 1, 30, '100000'),
                ('party_cust_b', 'cmp_b', 'led_cust_b', 'CUST-B01', 'Beta Customer', 1, 0, 15, '50000')",
    )
    .execute(p)
    .await
    .unwrap();

    // 5. Voucher Number Series & Configs
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('series_sal_a', 'cmp_a', 'vt_sales', 'Sales Series A', 'SAL', 'automatic', 'yearly'),
                ('series_pur_a', 'cmp_a', 'vt_pur', 'Purchase Series A', 'PUR', 'automatic', 'yearly'),
                ('series_sal_b', 'cmp_b', 'vt_sales', 'Sales Series B', 'SALB', 'automatic', 'yearly')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_sal_a', 'cmp_a', 'vt_sales', 'series_sal_a', '2026-04-01', 'SAL/2026/', '', 1, 4),
                ('cfg_pur_a', 'cmp_a', 'vt_pur', 'series_pur_a', '2026-04-01', 'PUR/2026/', '', 1, 4),
                ('cfg_sal_b', 'cmp_b', 'vt_sales', 'series_sal_b', '2026-04-01', 'SALB/2026/', '', 1, 4)",
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
    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value)
         VALUES ('item_widget', 'cmp_a', 'Widget', 'sg_a', 'unit_pcs', '1000', '100', '100000')",
    )
    .execute(p)
    .await
    .unwrap();

    // 7. Seed initial FIFO layer for item_widget so F8 sales has stock to consume
    sqlx::query(
        "INSERT INTO inventory_cost_layers (id, company_id, stock_item_id, godown_id, receipt_date, original_quantity, remaining_quantity, unit_cost, original_value)
         VALUES ('layer_init_widget', 'cmp_a', 'item_widget', 'godown_main', '2026-04-01', '1000', '1000', '100.00', '100000.00')",
    )
    .execute(p)
    .await
    .unwrap();

    // 8. GST Masters (Company GST Profile, Ledger Profiles, HSN/SAC, Item Mappings, Ledger Mappings)
    // Company Profile: MH (27)
    sqlx::query(
        "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled)
         VALUES ('cmp_a', '27AAPFU0939F1ZV', '27', 'Regular', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // Customer MH (27) Regular
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES ('lgp_cust_a', 'cmp_a', 'led_cust_a', '27AAPFU0939F1ZV', '27', 'Regular')",
    )
    .execute(p)
    .await
    .unwrap();

    // Customer KA (29) Regular (Interstate)
    let check_ka = calculate_gstin_check_digit("29ABCDE1234F1Z").unwrap();
    let gstin_ka = format!("29ABCDE1234F1Z{}", check_ka);
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES ('lgp_cust_inter', 'cmp_a', 'led_cust_inter', ?, '29', 'Regular')",
    )
    .bind(gstin_ka)
    .execute(p)
    .await
    .unwrap();

    // Customer KA B2C Large Unregistered
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES ('lgp_cust_b2c_large', 'cmp_a', 'led_cust_b2c_large', NULL, '29', 'Unregistered')",
    )
    .execute(p)
    .await
    .unwrap();

    // Customer MH B2C Small Unregistered
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES ('lgp_cust_b2c_small', 'cmp_a', 'led_cust_b2c_small', NULL, '27', 'Unregistered')",
    )
    .execute(p)
    .await
    .unwrap();

    // Supplier MH (27) Regular
    sqlx::query(
        "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
         VALUES ('lgp_supp_a', 'cmp_a', 'led_supp_a', '27AAPFU0939F1ZV', '27', 'Regular')",
    )
    .execute(p)
    .await
    .unwrap();

    // HSN Code
    sqlx::query(
        "INSERT INTO hsn_sac_codes (id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from)
         VALUES ('hsn_widget', 'cmp_a', '8504', 'Electrical Transformers & Converters', 'HSN', '18.00', '0.00', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    // Item GST Mapping
    sqlx::query(
        "INSERT INTO item_gst_mappings (id, company_id, stock_item_id, hsn_sac_code_id, gst_rate, cess_rate, taxability)
         VALUES ('igm_widget', 'cmp_a', 'item_widget', 'hsn_widget', '18.00', '0.00', 'Taxable')",
    )
    .execute(p)
    .await
    .unwrap();

    // GST Ledger Mapping
    sqlx::query(
        "INSERT INTO gst_ledger_mappings (company_id, output_cgst_ledger_id, output_sgst_ledger_id, output_igst_ledger_id, output_cess_ledger_id, input_cgst_ledger_id, input_sgst_ledger_id, input_igst_ledger_id, input_cess_ledger_id)
         VALUES ('cmp_a', 'led_out_cgst', 'led_out_sgst', 'led_out_igst', 'led_out_cess', 'led_in_cgst', 'led_in_sgst', 'led_in_igst', 'led_in_cess')",
    )
    .execute(p)
    .await
    .unwrap();
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct VLine {
    ledger_id: String,
    entry_type: String,
    amount: String,
}

#[tokio::test]
async fn test_intra_state_sales_posting_with_gst() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // 10 units @ 100 = 1000.00 taxable. 18% GST (9% CGST = 90.00, 9% SGST = 90.00). Invoice total = 1180.00.
    let input = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: Some("2026-06-10".to_string()),
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-001".to_string()),
        narration: Some("Intra-state sales with CGST + SGST".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget line".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-001".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: Some("2026-06-10".to_string()),
        }],
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
    };

    let result = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", input)
        .await
        .unwrap();

    assert_eq!(result.voucher.voucher.status, "posted");

    // Verify voucher lines
    let lines: Vec<VLine> = sqlx::query_as(
        "SELECT ledger_id, entry_type, amount FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number ASC",
    )
    .bind(&result.voucher.voucher.id)
    .fetch_all(pool)
    .await
    .unwrap();

    // Check party is debited with 1180.00
    let party_line = lines.iter().find(|l| l.ledger_id == "led_cust_a").unwrap();
    assert_eq!(party_line.entry_type, "Dr");
    assert_eq!(party_line.amount, "1180");

    // Check sales account is credited with 1000.00
    let sales_line = lines.iter().find(|l| l.ledger_id == "led_sales_a").unwrap();
    assert_eq!(sales_line.entry_type, "Cr");
    assert_eq!(sales_line.amount, "1000");

    // Check Output CGST is credited with 90.00
    let cgst_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_out_cgst")
        .unwrap();
    assert_eq!(cgst_line.entry_type, "Cr");
    assert_eq!(cgst_line.amount, "90");

    // Check Output SGST is credited with 90.00
    let sgst_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_out_sgst")
        .unwrap();
    assert_eq!(sgst_line.entry_type, "Cr");
    assert_eq!(sgst_line.amount, "90");

    // Verify voucher_gst_details record
    #[derive(Debug, sqlx::FromRow)]
    struct GstDetailRow {
        supply_type: String,
        taxable_value: String,
        cgst_amount: String,
        sgst_amount: String,
        igst_amount: String,
        is_cancelled: i64,
    }

    let gst_rec: GstDetailRow = sqlx::query_as(
        "SELECT supply_type, taxable_value, cgst_amount, sgst_amount, igst_amount, is_cancelled
         FROM voucher_gst_details WHERE voucher_id = ?",
    )
    .bind(&result.voucher.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(gst_rec.supply_type, "IntraState");
    assert_eq!(gst_rec.taxable_value, "1000.00");
    assert_eq!(gst_rec.cgst_amount, "90.00");
    assert_eq!(gst_rec.sgst_amount, "90.00");
    assert_eq!(gst_rec.igst_amount, "0.00");
    assert_eq!(gst_rec.is_cancelled, 0);
}

#[tokio::test]
async fn test_inter_state_sales_posting_with_igst() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // 20 units @ 100 = 2000.00 taxable. 18% IGST = 360.00. Invoice total = 2360.00.
    let input = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-12".to_string(),
        due_date: Some("2026-06-12".to_string()),
        party_id: "party_cust_inter".to_string(),
        party_ledger_id: "led_cust_inter".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(2000, 0),
        document_reference: Some("INV-002".to_string()),
        narration: Some("Inter-state sales with IGST".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget line".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(20, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(2000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-002".to_string(),
            amount: Decimal::new(2360, 0),
            due_date: Some("2026-06-12".to_string()),
        }],
        place_of_supply_state_code: Some("29".to_string()), // Karnataka POS
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
    };

    let result = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", input)
        .await
        .unwrap();

    assert_eq!(result.voucher.voucher.status, "posted");

    // Verify voucher lines: Dr Party = 2360.00, Cr Sales = 2000.00, Cr Output IGST = 360.00
    let lines: Vec<VLine> = sqlx::query_as(
        "SELECT ledger_id, entry_type, amount FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number ASC",
    )
    .bind(&result.voucher.voucher.id)
    .fetch_all(pool)
    .await
    .unwrap();

    let party_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_cust_inter")
        .unwrap();
    assert_eq!(party_line.entry_type, "Dr");
    assert_eq!(party_line.amount, "2360");

    let sales_line = lines.iter().find(|l| l.ledger_id == "led_sales_a").unwrap();
    assert_eq!(sales_line.entry_type, "Cr");
    assert_eq!(sales_line.amount, "2000");

    let igst_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_out_igst")
        .unwrap();
    assert_eq!(igst_line.entry_type, "Cr");
    assert_eq!(igst_line.amount, "360");

    // Check voucher_gst_details
    let gst_rec: (String, String, String, i64) = sqlx::query_as(
        "SELECT supply_type, taxable_value, igst_amount, is_cancelled
         FROM voucher_gst_details WHERE voucher_id = ?",
    )
    .bind(&result.voucher.voucher.id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(gst_rec.0, "InterState");
    assert_eq!(gst_rec.1, "2000.00");
    assert_eq!(gst_rec.2, "360.00");
    assert_eq!(gst_rec.3, 0);
}

#[tokio::test]
async fn test_f9_purchase_posting_with_input_itc_and_fifo_layer_protection() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // Purchase 30 widgets @ 150.00 = 4500.00 taxable.
    // 18% GST (9% CGST = 405.00, 9% SGST = 405.00). Invoice total = 5310.00.
    let input = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Purchase,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_pur_a".to_string(),
        date: "2026-05-15".to_string(),
        due_date: Some("2026-06-15".to_string()),
        party_id: "party_supp_a".to_string(),
        party_ledger_id: "led_supp_a".to_string(),
        commercial_account_ledger_id: Some("led_purchase_a".to_string()),
        total_amount: Decimal::new(4500, 0),
        document_reference: Some("SUPP-INV-101".to_string()),
        narration: Some("Purchase with Input Tax Credit".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget purchase".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(30, 0),
            rate: Decimal::new(150, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(4500, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "SUPP-INV-101".to_string(),
            amount: Decimal::new(5310, 0),
            due_date: Some("2026-06-15".to_string()),
        }],
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
    };

    let result = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", input)
        .await
        .unwrap();

    assert_eq!(result.voucher.voucher.status, "posted");

    // Verify voucher lines:
    // Dr Purchase Account = 4500.00
    // Dr Input CGST = 405.00
    // Dr Input SGST = 405.00
    // Cr Supplier = 5310.00
    let lines: Vec<VLine> = sqlx::query_as(
        "SELECT ledger_id, entry_type, amount FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number ASC",
    )
    .bind(&result.voucher.voucher.id)
    .fetch_all(pool)
    .await
    .unwrap();

    let pur_line = lines
        .iter()
        .find(|l| l.ledger_id == "led_purchase_a")
        .unwrap();
    assert_eq!(pur_line.entry_type, "Dr");
    assert_eq!(pur_line.amount, "4500");

    let in_cgst = lines.iter().find(|l| l.ledger_id == "led_in_cgst").unwrap();
    assert_eq!(in_cgst.entry_type, "Dr");
    assert_eq!(in_cgst.amount, "405");

    let in_sgst = lines.iter().find(|l| l.ledger_id == "led_in_sgst").unwrap();
    assert_eq!(in_sgst.entry_type, "Dr");
    assert_eq!(in_sgst.amount, "405");

    let supp_line = lines.iter().find(|l| l.ledger_id == "led_supp_a").unwrap();
    assert_eq!(supp_line.entry_type, "Cr");
    assert_eq!(supp_line.amount, "5310");

    // CRITICAL: Verify FIFO inventory cost layer protection.
    // The created layer unit_cost must be 150.00 (net taxable cost), NOT 177.00 (gross with tax)
    #[derive(Debug, sqlx::FromRow)]
    struct LayerRow {
        original_quantity: String,
        unit_cost: String,
        original_value: String,
    }

    let layer: LayerRow = sqlx::query_as(
        "SELECT original_quantity, unit_cost, original_value
         FROM inventory_cost_layers
         WHERE company_id = 'cmp_a' AND source_voucher_id = ?",
    )
    .bind(result.stock_voucher_id.as_ref().unwrap())
    .fetch_one(pool)
    .await
    .unwrap();

    assert_eq!(layer.original_quantity, "30");
    assert_eq!(layer.unit_cost, "150");
    assert_eq!(layer.original_value, "4500");
}

#[tokio::test]
async fn test_gstr1_summary_b2b_b2c_and_hsn() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // 1. Post B2B Intra-State (10 units @ 100 = 1000.00 taxable, 90 CGST, 90 SGST)
    let inv1 = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: Some("2026-06-10".to_string()),
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-B2B-1".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-B2B-1".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: None,
        }],
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv1)
        .await
        .unwrap();

    // 2. Post B2C Large (> 2.5L inter-state unregistered: 260 units @ 1000 = 260,000 taxable, 46,800 IGST)
    let inv_b2c_l = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-12".to_string(),
        due_date: None,
        party_id: "party_cust_b2c_large".to_string(),
        party_ledger_id: "led_cust_b2c_large".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(260000, 0),
        document_reference: Some("INV-B2CL-1".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget bulk".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(260, 0),
            rate: Decimal::new(1000, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(260000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-B2CL-1".to_string(),
            amount: Decimal::new(306800, 0),
            due_date: None,
        }],
        place_of_supply_state_code: Some("29".to_string()),
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv_b2c_l)
        .await
        .unwrap();

    // 3. Post B2C Small (Intra-state unregistered: 5 units @ 100 = 500 taxable, 45 CGST, 45 SGST)
    let inv_b2c_s = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-14".to_string(),
        due_date: None,
        party_id: "party_cust_b2c_small".to_string(),
        party_ledger_id: "led_cust_b2c_small".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(500, 0),
        document_reference: Some("INV-B2CS-1".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget retail".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(5, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(500, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-B2CS-1".to_string(),
            amount: Decimal::new(590, 0),
            due_date: None,
        }],
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv_b2c_s)
        .await
        .unwrap();

    // Generate GSTR-1 Report
    let gstr1 = GstReportService::get_gstr1_summary(pool, "cmp_a", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    assert_eq!(gstr1.b2b_invoices.len(), 1);
    assert_eq!(gstr1.b2b_invoices[0].taxable_value, "1000.00");
    assert_eq!(gstr1.b2b_invoices[0].cgst_amount, "90.00");
    assert_eq!(gstr1.b2b_invoices[0].sgst_amount, "90.00");

    assert_eq!(gstr1.b2c_large_invoices.len(), 1);
    assert_eq!(gstr1.b2c_large_invoices[0].taxable_value, "260000.00");
    assert_eq!(gstr1.b2c_large_invoices[0].igst_amount, "46800.00");

    assert_eq!(gstr1.b2c_small_summary.len(), 1);
    assert_eq!(gstr1.b2c_small_summary[0].taxable_value, "500.00");
    assert_eq!(gstr1.b2c_small_summary[0].cgst_amount, "45.00");
    assert_eq!(gstr1.b2c_small_summary[0].sgst_amount, "45.00");

    // HSN Summary check: total taxable = 1000 + 260000 + 500 = 261,500.00
    assert_eq!(gstr1.hsn_summary.len(), 1);
    assert_eq!(gstr1.hsn_summary[0].hsn_sac_code, "8504");
    assert_eq!(gstr1.hsn_summary[0].total_taxable_value, "261500.00");
    assert_eq!(gstr1.hsn_summary[0].igst_amount, "46800.00");
    assert_eq!(gstr1.hsn_summary[0].cgst_amount, "135.00");
    assert_eq!(gstr1.hsn_summary[0].sgst_amount, "135.00");
}

#[tokio::test]
async fn test_gstr3b_summary_tables() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // 1. Sales: 10 units @ 100 = 1000 taxable. 90 CGST, 90 SGST.
    let inv1 = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-001".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-001".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: None,
        }],
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv1)
        .await
        .unwrap();

    // 2. Purchase: 5 units @ 100 = 500 taxable. 45 Input CGST, 45 Input SGST.
    let pur1 = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Purchase,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_pur_a".to_string(),
        date: "2026-05-12".to_string(),
        due_date: None,
        party_id: "party_supp_a".to_string(),
        party_ledger_id: "led_supp_a".to_string(),
        commercial_account_ledger_id: Some("led_purchase_a".to_string()),
        total_amount: Decimal::new(500, 0),
        document_reference: Some("PUR-001".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(5, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(500, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "PUR-001".to_string(),
            amount: Decimal::new(590, 0),
            due_date: None,
        }],
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", pur1)
        .await
        .unwrap();

    // Generate GSTR-3B
    let gstr3b = GstReportService::get_gstr3b_summary(pool, "cmp_a", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    // Table 3.1: (a) Outward taxable supplies
    let table31a = &gstr3b.table_3_1[0];
    assert_eq!(table31a.total_taxable_value, "1000.00");
    assert_eq!(table31a.central_tax, "90.00");
    assert_eq!(table31a.state_ut_tax, "90.00");

    // Table 4: (A)(5) All other ITC
    let table4a5 = &gstr3b.table_4_itc[1];
    assert_eq!(table4a5.central_tax, "45.00");
    assert_eq!(table4a5.state_ut_tax, "45.00");

    // Net Tax Payable: 90.00 - 45.00 = 45.00 CGST and SGST
    assert_eq!(gstr3b.net_tax_payable.central_tax, "45.00");
    assert_eq!(gstr3b.net_tax_payable.state_ut_tax, "45.00");
    assert_eq!(gstr3b.net_tax_payable.total, "90.00");
}

#[tokio::test]
async fn test_voucher_cancellation_gst_neutralization() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    let inv = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-CANCEL".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-CANCEL".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: None,
        }],
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
    };

    let posted = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv)
        .await
        .unwrap();

    // Verify voucher_gst_details is_cancelled is 0
    let cancelled_before: i64 =
        sqlx::query_scalar("SELECT is_cancelled FROM voucher_gst_details WHERE voucher_id = ?")
            .bind(&posted.voucher.voucher.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(cancelled_before, 0);

    // Cancel voucher
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: posted.voucher.voucher.id.clone(),
            reason: "Customer cancelled order".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // Verify voucher_gst_details is_cancelled is 1
    let cancelled_after: i64 =
        sqlx::query_scalar("SELECT is_cancelled FROM voucher_gst_details WHERE voucher_id = ?")
            .bind(&posted.voucher.voucher.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(cancelled_after, 1);

    // Verify GSTR-1 summary excludes this cancelled invoice
    let gstr1 = GstReportService::get_gstr1_summary(pool, "cmp_a", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(gstr1.b2b_invoices.len(), 0);
}

#[tokio::test]
async fn test_anti_drift_tax_guard_and_audit_override() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // Attempt to post with manual tax differing from computed (CGST: 120 vs computed 90) without override
    let drift_input = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-DRIFT".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_out_cgst".to_string(),
                rate: Decimal::new(9, 0),
                amount: Decimal::new(120, 0), // drifted from 90.00!
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_out_sgst".to_string(),
                rate: Decimal::new(9, 0),
                amount: Decimal::new(90, 0),
            },
        ],
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-DRIFT".to_string(),
            amount: Decimal::new(1210, 0),
            due_date: None,
        }],
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
    };

    let err = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", drift_input)
        .await
        .unwrap_err();

    match err {
        OpenTallyError::Validation(msg) => {
            assert!(msg.contains("Tax drift detected"));
        }
        _ => panic!("Expected Validation error for tax drift, got {:?}", err),
    }

    // Now permit override with allow_tax_override: Some(true)
    let allowed_input = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-DRIFT-OK".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: vec![
            TaxPostingLine {
                tax_type: "CGST".to_string(),
                tax_ledger_id: "led_out_cgst".to_string(),
                rate: Decimal::new(9, 0),
                amount: Decimal::new(120, 0),
            },
            TaxPostingLine {
                tax_type: "SGST".to_string(),
                tax_ledger_id: "led_out_sgst".to_string(),
                rate: Decimal::new(9, 0),
                amount: Decimal::new(90, 0),
            },
        ],
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-DRIFT-OK".to_string(),
            amount: Decimal::new(1210, 0),
            due_date: None,
        }],
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
    };

    let success = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", allowed_input)
        .await
        .unwrap();

    assert_eq!(success.voucher.voucher.status, "posted");

    // Check audit_log
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_a' AND entity_type = 'voucher_tax' AND action = 'UPDATE'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(count >= 1);
}

#[tokio::test]
async fn test_multi_company_isolation() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // Post in cmp_a
    let inv = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-10".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-ISO".to_string()),
        narration: None,
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-ISO".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: None,
        }],
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
    };
    CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv)
        .await
        .unwrap();

    // Query for cmp_b
    let gstr1_b = GstReportService::get_gstr1_summary(pool, "cmp_b", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(gstr1_b.b2b_invoices.len(), 0);
    assert_eq!(gstr1_b.b2c_large_invoices.len(), 0);
    assert_eq!(gstr1_b.b2c_small_summary.len(), 0);
    assert_eq!(gstr1_b.hsn_summary.len(), 0);

    let gstr3b_b = GstReportService::get_gstr3b_summary(pool, "cmp_b", "2026-05-01", "2026-05-31")
        .await
        .unwrap();
    assert_eq!(gstr3b_b.net_tax_payable.total, "0.00");
}

#[tokio::test]
async fn test_export_gstr1_and_gstr3b_json() {
    let (_dir, db) = setup_gst_fixture().await;
    let pool = db.pool();

    // 1. Post a valid B2B invoice in cmp_a
    let inv = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-15".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(1000, 0),
        document_reference: Some("INV-GSTR-JSON-01".to_string()),
        narration: Some("Test JSON export sale".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(10, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(1000, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-GSTR-JSON-01".to_string(),
            amount: Decimal::new(1180, 0),
            due_date: None,
        }],
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
    };
    let posted = CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv)
        .await
        .unwrap();

    // 2. Post a second voucher that we will cancel to verify cancellation exclusion
    let inv_to_cancel = CommercialPostingInput {
        company_id: "cmp_a".to_string(),
        document_type: CommercialDocumentType::Sales,
        source_document_id: Uuid::new_v4().to_string(),
        source_voucher_id: None,
        series_id: "series_sal_a".to_string(),
        date: "2026-05-16".to_string(),
        due_date: None,
        party_id: "party_cust_a".to_string(),
        party_ledger_id: "led_cust_a".to_string(),
        commercial_account_ledger_id: Some("led_sales_a".to_string()),
        total_amount: Decimal::new(500, 0),
        document_reference: Some("INV-GSTR-CANCEL".to_string()),
        narration: Some("Sale to cancel".to_string()),
        lines: vec![CommercialLinePosting {
            line_number: 1,
            stock_item_id: Some("item_widget".to_string()),
            description: "Widget item".to_string(),
            unit_id: Some("unit_pcs".to_string()),
            quantity: Decimal::new(5, 0),
            rate: Decimal::new(100, 0),
            discount_amount: Decimal::ZERO,
            line_amount: Decimal::new(500, 0),
            godown_id: Some("godown_main".to_string()),
            batch_number: None,
            mfg_date: None,
            expiry_date: None,
            billed_quantity: None,
            entered_unit_id: None,
        }],
        tax_lines: Vec::new(),
        bill_allocations: vec![BillAllocationPostingLine {
            bill_type: "New Ref".to_string(),
            bill_name: "INV-GSTR-CANCEL".to_string(),
            amount: Decimal::new(590, 0),
            due_date: None,
        }],
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
    };
    let posted_cancel =
        CommercialPostingService::post_commercial_voucher(pool, "cmp_a", inv_to_cancel)
            .await
            .unwrap();

    // Cancel the second voucher
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_a",
        CancelVoucherRequest {
            voucher_id: posted_cancel.voucher.voucher.id.clone(),
            reason: "Cancelled test voucher".to_string(),
            actor_id: None,
        },
    )
    .await
    .unwrap();

    // 3. Export GSTR-1 JSON
    let gstr1_json = GstReportService::export_gstr1_json(pool, "cmp_a", "2026-05-01", "2026-05-31")
        .await
        .unwrap();

    let v1: serde_json::Value = serde_json::from_str(&gstr1_json).unwrap();
    assert_eq!(v1["fp"], "052026");
    assert_eq!(v1["gstin"], "27AAPFU0939F1ZV");
    let b2b = v1["b2b"].as_array().unwrap();
    assert_eq!(b2b.len(), 1);
    assert_eq!(
        b2b[0]["inv"][0]["inum"],
        posted.voucher.voucher.voucher_number
    );
    // Ensure cancelled voucher is excluded
    for invoice in b2b {
        assert_ne!(
            invoice["voucher_number"],
            posted_cancel.voucher.voucher.voucher_number
        );
    }
    assert_eq!(v1["total_taxable_value"], "1000.00");
    assert_eq!(v1["total_tax"], "180.00");
    assert_eq!(v1["total_invoice_value"], "1180.00");

    // 4. Export GSTR-3B JSON
    let gstr3b_json =
        GstReportService::export_gstr3b_json(pool, "cmp_a", "2026-05-01", "2026-05-31")
            .await
            .unwrap();

    let v3: serde_json::Value = serde_json::from_str(&gstr3b_json).unwrap();
    assert_eq!(v3["ret_period"], "052026");
    assert_eq!(v3["gstin"], "27AAPFU0939F1ZV");
    assert!(v3["sup_details"].is_array());
    assert!(v3["itc_elg"].is_array());
    assert_eq!(v3["tax_payable"]["central_tax"], "90.00");
    assert_eq!(v3["tax_payable"]["state_ut_tax"], "90.00");
    assert_eq!(v3["tax_payable"]["total"], "180.00");
}
