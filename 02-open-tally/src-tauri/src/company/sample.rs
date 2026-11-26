//! One-Click "Load Sample Indian SME Company" (Bharath Precision Components Pvt Ltd)
//!
//! Creates an unrestricted, realistic Indian SME demo company with:
//! - Realistic Karnataka SME profile (State 29, FY 2025-04-01, GST enabled)
//! - Auto-seeded GST ledgers & perpetual inventory account mappings
//! - 2 Sundry Debtors (1 intra-state KA, 1 inter-state MH) with valid Mod-36 GSTINs
//! - 1 Sundry Creditor with valid Mod-36 GSTIN
//! - Bank Account (HDFC Bank Current A/c) with initial capital introduction
//! - 3 HSN-mapped Stock Items (HSN 8481, 8482, 4016)
//! - 2 posted F9 Purchases (creating FIFO cost layers + input ITC + Bills Payable)
//! - 2 posted F8 Sales (1 intra-state, 1 inter-state, consuming FIFO + Output GST + Bills Receivable)
//! - 1 partial F6 Receipt (AgstRef settlement, leaving pending bill in aging reports)
//! - 1 F5 Payment (AgstRef payment, leaving pending bill in aging reports)
//! - Sets active company so Gateway and reports are immediately populated.

use crate::billwise::{AllocationType, BillAllocationInput, BillWiseService};
use crate::commercial::{
    BillAllocationPostingLine, CommercialDocumentType, CommercialLinePosting,
    CommercialPostingInput, CommercialPostingService,
};
use crate::company::features::{CompanyFeaturesService, SaveCompanyFeaturesInput};
use crate::company::model::{Company, CreateCompanyRequest};
use crate::company::service::CompanyService;
use crate::errors::OpenTallyError;
use crate::gst::calculate_gstin_check_digit;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

pub struct SampleCompanyService;

impl SampleCompanyService {
    /// Helper to format valid Mod-36 GSTIN from a 14-char prefix
    fn format_valid_gstin(prefix_14: &str) -> String {
        let check_char = calculate_gstin_check_digit(prefix_14).unwrap_or('0');
        format!("{}{}", prefix_14, check_char)
    }

    /// Create and populate the Indian SME Sample Company.
    pub async fn create_sample_company(pool: &Pool<Sqlite>) -> Result<Company, OpenTallyError> {
        // Step 1: Idempotent naming
        let base_name = "Bharath Precision Components Pvt Ltd";
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM companies WHERE name LIKE 'Bharath Precision Components Pvt Ltd%'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let company_name = if count == 0 {
            base_name.to_string()
        } else {
            format!("{} (Demo {})", base_name, count + 1)
        };

        // Step 2: Create base company
        let req = CreateCompanyRequest {
            name: company_name,
            mailing_name: Some("Bharath Precision Components Pvt Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2025-04-01".to_string(),
            books_beginning_from: Some("2025-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        };
        let company = CompanyService::create_company(pool, req).await?;
        let company_id = &company.id;

        // Step 3: Configure F11 Features with auto-seeding
        let comp_gstin = Self::format_valid_gstin("29AABCU9603R1Z");
        CompanyFeaturesService::save_company_features(
            pool,
            company_id,
            SaveCompanyFeaturesInput {
                maintain_inventory: true,
                billwise_tracking: true,
                enable_perpetual_cogs: true,
                gst_enabled: true,
                gstin: Some(comp_gstin),
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
                enable_multi_currency: Some(false),
                auto_seed_currency_defaults: false,
            },
        )
        .await?;

        // Step 4: Seed Masters & Setup Series in a dedicated connection
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;

        // Find standard seeded group IDs
        let debtors_grp_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Debtors' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let creditors_grp_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let bank_grp_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Bank Accounts' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let capital_grp_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Capital Account' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let pur_account_id: String = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Purchase Account' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let sales_account_id: String = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = 'Sales Account' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // A. Bank & Capital Accounts
        let bank_ledger_id = Self::create_ledger_with_opbal(
            &mut conn,
            company_id,
            "HDFC Bank Current A/c",
            &bank_grp_id,
            "Dr",
        )
        .await?;

        let capital_ledger_id = Self::create_ledger_with_opbal(
            &mut conn,
            company_id,
            "Share Capital",
            &capital_grp_id,
            "Cr",
        )
        .await?;

        // B. Sundry Debtors & Creditors
        let cust1_ledger_id = Self::create_ledger_with_opbal(
            &mut conn,
            company_id,
            "Apex Automations Pvt Ltd",
            &debtors_grp_id,
            "Dr",
        )
        .await?;
        let cust1_gstin = Self::format_valid_gstin("29AABCA1234D1Z");
        let cust1_party_id = Self::create_party_and_gst_profile(
            &mut conn,
            company_id,
            &cust1_ledger_id,
            "CUST-001",
            "Apex Automations Pvt Ltd",
            true,
            false,
            &cust1_gstin,
            "29",
            "Regular",
            30,
        )
        .await?;

        let cust2_ledger_id = Self::create_ledger_with_opbal(
            &mut conn,
            company_id,
            "Metro Mechanics Ltd",
            &debtors_grp_id,
            "Dr",
        )
        .await?;
        let cust2_gstin = Self::format_valid_gstin("27AABCM5678E1Z");
        let cust2_party_id = Self::create_party_and_gst_profile(
            &mut conn,
            company_id,
            &cust2_ledger_id,
            "CUST-002",
            "Metro Mechanics Ltd",
            true,
            false,
            &cust2_gstin,
            "27",
            "Regular",
            45,
        )
        .await?;

        let supp1_ledger_id = Self::create_ledger_with_opbal(
            &mut conn,
            company_id,
            "Precision Steel Works",
            &creditors_grp_id,
            "Cr",
        )
        .await?;
        let supp1_gstin = Self::format_valid_gstin("29AABCP9999P1Z");
        let supp1_party_id = Self::create_party_and_gst_profile(
            &mut conn,
            company_id,
            &supp1_ledger_id,
            "SUPP-001",
            "Precision Steel Works",
            false,
            true,
            &supp1_gstin,
            "29",
            "Regular",
            30,
        )
        .await?;

        // C. Inventory Units, Godowns, Groups & HSN Codes
        let unit_pcs_id = format!("unit_pcs_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision)
             VALUES (?, ?, 'Pcs', 'Pieces', 'simple', 0)",
        )
        .bind(&unit_pcs_id)
        .bind(company_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let godown_main_id = format!("godown_main_{}", Uuid::new_v4().simple());
        sqlx::query("INSERT INTO godowns (id, company_id, name) VALUES (?, ?, 'Main Warehouse')")
            .bind(&godown_main_id)
            .bind(company_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let stock_grp_id = format!("sg_eng_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO stock_groups (id, company_id, name) VALUES (?, ?, 'Precision Components')",
        )
        .bind(&stock_grp_id)
        .bind(company_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let hsn_8481_id = format!("hsn_8481_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO hsn_sac_codes (id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from)
             VALUES (?, ?, '8481', 'Taps, cocks, valves for pipes', 'HSN', '18.00', '0.00', '2025-04-01')",
        )
        .bind(&hsn_8481_id)
        .bind(company_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let hsn_8482_id = format!("hsn_8482_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO hsn_sac_codes (id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from)
             VALUES (?, ?, '8482', 'Ball or roller bearings', 'HSN', '18.00', '0.00', '2025-04-01')",
        )
        .bind(&hsn_8482_id)
        .bind(company_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let hsn_4016_id = format!("hsn_4016_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO hsn_sac_codes (id, company_id, code, description, code_type, default_gst_rate, cess_rate, effective_from)
             VALUES (?, ?, '4016', 'Articles of vulcanised rubber', 'HSN', '12.00', '0.00', '2025-04-01')",
        )
        .bind(&hsn_4016_id)
        .bind(company_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Stock Items with Item GST Mappings
        let item_valve_id = Self::create_stock_item_with_gst(
            &mut conn,
            company_id,
            "CNC Machined Valve",
            &stock_grp_id,
            &unit_pcs_id,
            &hsn_8481_id,
            "18.00",
        )
        .await?;

        let item_sleeve_id = Self::create_stock_item_with_gst(
            &mut conn,
            company_id,
            "Alloy Bearing Sleeve",
            &stock_grp_id,
            &unit_pcs_id,
            &hsn_8482_id,
            "18.00",
        )
        .await?;

        let item_gasket_id = Self::create_stock_item_with_gst(
            &mut conn,
            company_id,
            "Precision Gasket Ring",
            &stock_grp_id,
            &unit_pcs_id,
            &hsn_4016_id,
            "12.00",
        )
        .await?;

        // D. Setup Series for Sales, Purchase, Payment, Receipt
        let sales_series_id =
            Self::ensure_series(&mut conn, company_id, "vt_sales", "Sales Series", "SAL").await?;
        let pur_series_id =
            Self::ensure_series(&mut conn, company_id, "vt_pur", "Purchase Series", "PUR").await?;
        let pmt_series_id =
            Self::ensure_series(&mut conn, company_id, "vt_payment", "Payment Series", "PMT")
                .await?;
        let rcpt_series_id = Self::ensure_series(
            &mut conn,
            company_id,
            "vt_receipt",
            "Receipt Series",
            "RCPT",
        )
        .await?;

        // E. Capital Introduction Voucher (₹500,000 Receipt on 2025-04-01)
        let cap_vch_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method,
                                  voucher_number, date, effective_date, narration, status)
             VALUES (?, ?, 'vt_receipt', ?, 'automatic', 'RCPT/2025/0001', '2025-04-01', '2025-04-01',
                     'Initial Share Capital deposited in Bank', 'draft')",
        )
        .bind(&cap_vch_id)
        .bind(company_id)
        .bind(&rcpt_series_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
             VALUES (?, ?, ?, 1, ?, 'Dr', '500000.00', 'Deposit into HDFC Bank'),
                    (?, ?, ?, 2, ?, 'Cr', '500000.00', 'Share Capital introduced')",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&cap_vch_id)
        .bind(&bank_ledger_id)
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&cap_vch_id)
        .bind(&capital_ledger_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "UPDATE vouchers
             SET status = 'posted', sequence_number = 1, period_start = '2025-04-01', total_amount = '500000.00'
             WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&cap_vch_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        drop(conn);

        // Step 5: Post Commercial Transactions through Production CommercialPostingService
        // ---------------------------------------------------------------------------------
        // Purchase 1: PO-2025-001 on 2025-04-10
        // 100 Valves @ 200 = 20,000 (18% GST: 1800 CGST + 1800 SGST)
        // 50 Sleeves @ 300 = 15,000 (18% GST: 1350 CGST + 1350 SGST)
        // Total Taxable = 35,000; Total Grand = 41,300
        let pur1_input = CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: CommercialDocumentType::Purchase,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: pur_series_id.clone(),
            date: "2025-04-10".to_string(),
            due_date: Some("2025-05-10".to_string()),
            party_id: supp1_party_id.clone(),
            party_ledger_id: supp1_ledger_id.clone(),
            commercial_account_ledger_id: Some(pur_account_id.clone()),
            total_amount: Decimal::new(35000, 0),
            document_reference: Some("PO-2025-001".to_string()),
            narration: Some("Purchase of valves and bearing sleeves".to_string()),
            lines: vec![
                CommercialLinePosting {
                    line_number: 1,
                    description: "CNC Machined Valve".to_string(),
                    stock_item_id: Some(item_valve_id.clone()),
                    unit_id: Some(unit_pcs_id.clone()),
                    godown_id: Some(godown_main_id.clone()),
                    quantity: Decimal::new(100, 0),
                    rate: Decimal::new(200, 0),
                    discount_amount: Decimal::ZERO,
                    line_amount: Decimal::new(20000, 0),
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                },
                CommercialLinePosting {
                    line_number: 2,
                    description: "Alloy Bearing Sleeve".to_string(),
                    stock_item_id: Some(item_sleeve_id.clone()),
                    unit_id: Some(unit_pcs_id.clone()),
                    godown_id: Some(godown_main_id.clone()),
                    quantity: Decimal::new(50, 0),
                    rate: Decimal::new(300, 0),
                    discount_amount: Decimal::ZERO,
                    line_amount: Decimal::new(15000, 0),
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                },
            ],
            tax_lines: vec![],
            bill_allocations: vec![BillAllocationPostingLine {
                bill_type: "New Ref".to_string(),
                bill_name: "PO-2025-001".to_string(),
                amount: Decimal::new(41300, 0),
                due_date: Some("2025-05-10".to_string()),
            }],
            place_of_supply_state_code: Some("29".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
        };
        CommercialPostingService::post_commercial_voucher(pool, company_id, pur1_input).await?;

        // Purchase 2: PO-2025-002 on 2025-04-15
        // 100 Gaskets @ 100 = 10,000 (12% GST: 600 CGST + 600 SGST)
        // Total Taxable = 10,000; Total Grand = 11,200
        let pur2_input = CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: CommercialDocumentType::Purchase,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: pur_series_id.clone(),
            date: "2025-04-15".to_string(),
            due_date: Some("2025-05-15".to_string()),
            party_id: supp1_party_id.clone(),
            party_ledger_id: supp1_ledger_id.clone(),
            commercial_account_ledger_id: Some(pur_account_id.clone()),
            total_amount: Decimal::new(10000, 0),
            document_reference: Some("PO-2025-002".to_string()),
            narration: Some("Purchase of gasket rings".to_string()),
            lines: vec![CommercialLinePosting {
                line_number: 1,
                description: "Precision Gasket Ring".to_string(),
                stock_item_id: Some(item_gasket_id.clone()),
                unit_id: Some(unit_pcs_id.clone()),
                godown_id: Some(godown_main_id.clone()),
                quantity: Decimal::new(100, 0),
                rate: Decimal::new(100, 0),
                discount_amount: Decimal::ZERO,
                line_amount: Decimal::new(10000, 0),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![BillAllocationPostingLine {
                bill_type: "New Ref".to_string(),
                bill_name: "PO-2025-002".to_string(),
                amount: Decimal::new(11200, 0),
                due_date: Some("2025-05-15".to_string()),
            }],
            place_of_supply_state_code: Some("29".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
        };
        CommercialPostingService::post_commercial_voucher(pool, company_id, pur2_input).await?;

        // Sales 1: INV-2025-001 on 2025-04-20
        // Customer: Apex Automations (Intra-state KA 29)
        // 40 Valves @ 400 = 16,000 (18% GST: 1440 CGST + 1440 SGST)
        // Total Taxable = 16,000; Total Grand = 18,880
        let sal1_input = CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sales_series_id.clone(),
            date: "2025-04-20".to_string(),
            due_date: Some("2025-05-20".to_string()),
            party_id: cust1_party_id.clone(),
            party_ledger_id: cust1_ledger_id.clone(),
            commercial_account_ledger_id: Some(sales_account_id.clone()),
            total_amount: Decimal::new(16000, 0),
            document_reference: Some("INV-2025-001".to_string()),
            narration: Some("Sales of precision machined valves".to_string()),
            lines: vec![CommercialLinePosting {
                line_number: 1,
                description: "CNC Machined Valve".to_string(),
                stock_item_id: Some(item_valve_id.clone()),
                unit_id: Some(unit_pcs_id.clone()),
                godown_id: Some(godown_main_id.clone()),
                quantity: Decimal::new(40, 0),
                rate: Decimal::new(400, 0),
                discount_amount: Decimal::ZERO,
                line_amount: Decimal::new(16000, 0),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![BillAllocationPostingLine {
                bill_type: "New Ref".to_string(),
                bill_name: "INV-2025-001".to_string(),
                amount: Decimal::new(18880, 0),
                due_date: Some("2025-05-20".to_string()),
            }],
            place_of_supply_state_code: Some("29".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
        };
        CommercialPostingService::post_commercial_voucher(pool, company_id, sal1_input).await?;

        // Sales 2: INV-2025-002 on 2025-04-25
        // Customer: Metro Mechanics (Inter-state MH 27)
        // 20 Sleeves @ 600 = 12,000 (18% IGST: 2160)
        // 30 Gaskets @ 200 = 6,000 (12% IGST: 720)
        // Total Taxable = 18,000; Total IGST = 2,880; Total Grand = 20,880
        let sal2_input = CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sales_series_id.clone(),
            date: "2025-04-25".to_string(),
            due_date: Some("2025-06-09".to_string()),
            party_id: cust2_party_id.clone(),
            party_ledger_id: cust2_ledger_id.clone(),
            commercial_account_ledger_id: Some(sales_account_id.clone()),
            total_amount: Decimal::new(18000, 0),
            document_reference: Some("INV-2025-002".to_string()),
            narration: Some("Interstate supply of bearing sleeves and gasket rings".to_string()),
            lines: vec![
                CommercialLinePosting {
                    line_number: 1,
                    description: "Alloy Bearing Sleeve".to_string(),
                    stock_item_id: Some(item_sleeve_id.clone()),
                    unit_id: Some(unit_pcs_id.clone()),
                    godown_id: Some(godown_main_id.clone()),
                    quantity: Decimal::new(20, 0),
                    rate: Decimal::new(600, 0),
                    discount_amount: Decimal::ZERO,
                    line_amount: Decimal::new(12000, 0),
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                },
                CommercialLinePosting {
                    line_number: 2,
                    description: "Precision Gasket Ring".to_string(),
                    stock_item_id: Some(item_gasket_id.clone()),
                    unit_id: Some(unit_pcs_id.clone()),
                    godown_id: Some(godown_main_id.clone()),
                    quantity: Decimal::new(30, 0),
                    rate: Decimal::new(200, 0),
                    discount_amount: Decimal::ZERO,
                    line_amount: Decimal::new(6000, 0),
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                },
            ],
            tax_lines: vec![],
            bill_allocations: vec![BillAllocationPostingLine {
                bill_type: "New Ref".to_string(),
                bill_name: "INV-2025-002".to_string(),
                amount: Decimal::new(20880, 0),
                due_date: Some("2025-06-09".to_string()),
            }],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
        };
        CommercialPostingService::post_commercial_voucher(pool, company_id, sal2_input).await?;

        // Step 6: Post Partial F6 Receipt on 2025-05-02 (settles ₹10,000 of INV-2025-001)
        let mut conn2 = pool.acquire().await.map_err(OpenTallyError::Database)?;
        let rcpt2_vch_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method,
                                  voucher_number, date, effective_date, narration, status)
             VALUES (?, ?, 'vt_receipt', ?, 'automatic', 'RCPT/2025/0002', '2025-05-02', '2025-05-02',
                     'Partial receipt from Apex Automations', 'draft')",
        )
        .bind(&rcpt2_vch_id)
        .bind(company_id)
        .bind(&rcpt_series_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        let rcpt_line_bank = Uuid::new_v4().to_string();
        let rcpt_line_cust = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
             VALUES (?, ?, ?, 1, ?, 'Dr', '10000.00', 'Payment received in HDFC Bank'),
                    (?, ?, ?, 2, ?, 'Cr', '10000.00', 'Partial settlement of INV-2025-001')",
        )
        .bind(&rcpt_line_bank)
        .bind(company_id)
        .bind(&rcpt2_vch_id)
        .bind(&bank_ledger_id)
        .bind(&rcpt_line_cust)
        .bind(company_id)
        .bind(&rcpt2_vch_id)
        .bind(&cust1_ledger_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "UPDATE vouchers
             SET status = 'posted', sequence_number = 2, period_start = '2025-04-01', total_amount = '10000.00'
             WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&rcpt2_vch_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        // Allocate AgstRef 10,000.00
        BillWiseService::validate_and_post_allocations(
            &mut conn2,
            company_id,
            &rcpt2_vch_id,
            &rcpt_line_cust,
            &cust1_ledger_id,
            "Cr",
            Decimal::new(10000, 0),
            "2025-05-02",
            None,
            &[BillAllocationInput {
                bill_name: "INV-2025-001".to_string(),
                allocation_type: AllocationType::AgstRef,
                amount: "10000.00".to_string(),
                dr_cr: Some("Cr".to_string()),
                due_date: None,
            }],
        )
        .await?;

        // Step 7: Post F5 Payment on 2025-05-05 (settles ₹20,000 of PO-2025-001)
        let pmt1_vch_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method,
                                  voucher_number, date, effective_date, narration, status)
             VALUES (?, ?, 'vt_payment', ?, 'automatic', 'PMT/2025/0001', '2025-05-05', '2025-05-05',
                     'Partial payment to Precision Steel Works', 'draft')",
        )
        .bind(&pmt1_vch_id)
        .bind(company_id)
        .bind(&pmt_series_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        let pmt_line_supp = Uuid::new_v4().to_string();
        let pmt_line_bank = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
             VALUES (?, ?, ?, 1, ?, 'Dr', '20000.00', 'Supplier bill settlement PO-2025-001'),
                    (?, ?, ?, 2, ?, 'Cr', '20000.00', 'Paid from HDFC Bank')",
        )
        .bind(&pmt_line_supp)
        .bind(company_id)
        .bind(&pmt1_vch_id)
        .bind(&supp1_ledger_id)
        .bind(&pmt_line_bank)
        .bind(company_id)
        .bind(&pmt1_vch_id)
        .bind(&bank_ledger_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "UPDATE vouchers
             SET status = 'posted', sequence_number = 1, period_start = '2025-04-01', total_amount = '20000.00'
             WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&pmt1_vch_id)
        .execute(&mut *conn2)
        .await
        .map_err(OpenTallyError::Database)?;

        // Allocate AgstRef 20,000.00
        BillWiseService::validate_and_post_allocations(
            &mut conn2,
            company_id,
            &pmt1_vch_id,
            &pmt_line_supp,
            &supp1_ledger_id,
            "Dr",
            Decimal::new(20000, 0),
            "2025-05-05",
            None,
            &[BillAllocationInput {
                bill_name: "PO-2025-001".to_string(),
                allocation_type: AllocationType::AgstRef,
                amount: "20000.00".to_string(),
                dr_cr: Some("Dr".to_string()),
                due_date: None,
            }],
        )
        .await?;

        drop(conn2);

        // Step 8: Persist active company selection
        CompanyService::set_persisted_active_company_id(pool, Some(company_id)).await?;

        Ok(company)
    }

    // Helper functions
    async fn create_ledger_with_opbal(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        name: &str,
        group_id: &str,
        balance_type: &str,
    ) -> Result<String, OpenTallyError> {
        let lid = format!(
            "led_{}_{}",
            name.to_lowercase().replace([' ', '/'], "_"),
            Uuid::new_v4().simple()
        );
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES (?, ?, ?, ?, 0, 'standard')",
        )
        .bind(&lid)
        .bind(company_id)
        .bind(group_id)
        .bind(name)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let opb_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES (?, ?, ?, '2025-04-01', ?, '0.00')",
        )
        .bind(&opb_id)
        .bind(company_id)
        .bind(&lid)
        .bind(balance_type)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(lid)
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_party_and_gst_profile(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        ledger_id: &str,
        party_code: &str,
        display_name: &str,
        cust_en: bool,
        supp_en: bool,
        gstin: &str,
        state_code: &str,
        reg_type: &str,
        credit_period: i64,
    ) -> Result<String, OpenTallyError> {
        let party_id = format!(
            "party_{}_{}",
            party_code.to_lowercase(),
            Uuid::new_v4().simple()
        );
        sqlx::query(
            "INSERT INTO party_profiles (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled, credit_period_days, credit_limit)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, '1000000')",
        )
        .bind(&party_id)
        .bind(company_id)
        .bind(ledger_id)
        .bind(party_code)
        .bind(display_name)
        .bind(if cust_en { 1 } else { 0 })
        .bind(if supp_en { 1 } else { 0 })
        .bind(credit_period)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let lgp_id = format!("lgp_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, state_code, registration_type)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&lgp_id)
        .bind(company_id)
        .bind(ledger_id)
        .bind(gstin)
        .bind(state_code)
        .bind(reg_type)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(party_id)
    }

    async fn create_stock_item_with_gst(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        name: &str,
        stock_group_id: &str,
        unit_id: &str,
        hsn_sac_id: &str,
        gst_rate: &str,
    ) -> Result<String, OpenTallyError> {
        let item_id = format!(
            "item_{}_{}",
            name.to_lowercase().replace(' ', "_"),
            Uuid::new_v4().simple()
        );
        sqlx::query(
            "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value)
             VALUES (?, ?, ?, ?, ?, '0', '0.00', '0.00')",
        )
        .bind(&item_id)
        .bind(company_id)
        .bind(name)
        .bind(stock_group_id)
        .bind(unit_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let mapping_id = format!("igm_{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO item_gst_mappings (id, company_id, stock_item_id, hsn_sac_code_id, gst_rate, cess_rate, taxability)
             VALUES (?, ?, ?, ?, ?, '0.00', 'Taxable')",
        )
        .bind(&mapping_id)
        .bind(company_id)
        .bind(&item_id)
        .bind(hsn_sac_id)
        .bind(gst_rate)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(item_id)
    }

    async fn ensure_series(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        vt_id: &str,
        name: &str,
        code: &str,
    ) -> Result<String, OpenTallyError> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = ? LIMIT 1",
        )
        .bind(company_id)
        .bind(vt_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(sid) = existing {
            return Ok(sid);
        }

        let sid = format!("series_{}_{}", code.to_lowercase(), Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
             VALUES (?, ?, ?, ?, ?, 'automatic', 'yearly')",
        )
        .bind(&sid)
        .bind(company_id)
        .bind(vt_id)
        .bind(name)
        .bind(code)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let cfg_id = format!("cfg_{}_{}", code.to_lowercase(), Uuid::new_v4().simple());
        let prefix = format!("{}/2025/", code);
        sqlx::query(
            "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
             VALUES (?, ?, ?, ?, '2025-04-01', ?, '', 1, 4)",
        )
        .bind(&cfg_id)
        .bind(company_id)
        .bind(vt_id)
        .bind(&sid)
        .bind(&prefix)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(sid)
    }
}
