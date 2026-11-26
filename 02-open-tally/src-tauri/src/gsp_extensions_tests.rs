//! Integration Test Suite for Milestone 18: BYO-GSP Connector & Declarative Custom Reports

use crate::audit::certification::AuditCertificationService;
use crate::database::migrations::run_all_migrations;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::extensions::{CustomReportService, SaveCustomReportInput};
use crate::gst::gsp_connector::{GspConnectorService, HttpGspProvider, SaveCompanyGspConfigInput};
use crate::pdf::generate_invoice_pdf_bytes;
use crate::security::models::{ActionPermission, UserRole};
use crate::vouchers::model::{SaveVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::draft::VoucherDraftService;
use crate::vouchers::service::posting::VoucherPostingService;

async fn setup_m18_db() -> (tempfile::TempDir, DatabaseManager, String, String, String) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("m18_gsp_ext_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("init db");
    let pool = db.pool();

    run_all_migrations(pool).await.expect("run all migrations");

    let comp1 = "cmp_m18_01".to_string();
    let comp2 = "cmp_m18_02".to_string();

    sqlx::query(
        r#"
        INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_symbol, currency_formal_name, currency_decimal_places)
        VALUES (?, 'Enterprise Exports Ltd', 'Enterprise Exports', 'India', 'Karnataka', '2026-04-01', '2026-04-01', '₹', 'INR', 2),
               (?, 'Other Tenant Corp', 'Other Tenant', 'India', 'Maharashtra', '2026-04-01', '2026-04-01', '₹', 'INR', 2)
        "#,
    )
    .bind(&comp1)
    .bind(&comp2)
    .execute(pool)
    .await
    .expect("seed companies");

    // Seed GST profile, groups, ledgers, party, and a posted sales voucher for comp1
    sqlx::query(
        r#"
        INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type)
        VALUES (?, '29AABCU9603R1ZM', '29', 'Regular')
        "#,
    )
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed gst profile");

    sqlx::query(
        r#"
        INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
        VALUES ('grp_ca_m18', ?, 'Current Assets', NULL, 'Assets', 1),
               ('grp_deb_m18', ?, 'Sundry Debtors', 'grp_ca_m18', 'Assets', 0),
               ('grp_sal_m18', ?, 'Sales Accounts', NULL, 'Income', 1)
        "#,
    )
    .bind(&comp1)
    .bind(&comp1)
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed groups");

    sqlx::query(
        r#"
        INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
        VALUES ('led_party_m18', ?, 'grp_deb_m18', 'Acme Buyer Pvt Ltd', 0, 'standard'),
               ('led_sales_m18', ?, 'grp_sal_m18', 'Local Sales', 0, 'standard')
        "#,
    )
    .bind(&comp1)
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed ledgers");

    sqlx::query(
        r#"
        INSERT INTO ledger_gst_profiles (id, company_id, ledger_id, gstin, registration_type, state_code)
        VALUES ('lgstp_m18', ?, 'led_party_m18', '29AAACB1234F1Z5', 'Regular', '29')
        "#,
    )
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed party gst");

    sqlx::query(
        r#"
        INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
        VALUES ('ser_sal_m18', ?, 'vt_sales', 'Sales Series', 'SAL', 'automatic', 'yearly')
        "#,
    )
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed series");

    sqlx::query(
        r#"
        INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
        VALUES ('cfg_sal_m18', ?, 'vt_sales', 'ser_sal_m18', '2026-04-01', 'SAL/', '', 1, 4)
        "#,
    )
    .bind(&comp1)
    .execute(pool)
    .await
    .expect("seed config");

    let draft = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_sales".to_string(),
            series_id: "ser_sal_m18".to_string(),
            date: "2026-04-15".to_string(),
            effective_date: None,
            reference_no: None,
            reference_date: None,
            narration: Some("=SUM(A1)".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_party_m18".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "11800.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_m18".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "11800.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .expect("save draft");

    let posted =
        VoucherPostingService::post_voucher(pool, &comp1, &draft.voucher.id, Some("admin"))
            .await
            .expect("post voucher");

    sqlx::query(
        r#"
        INSERT INTO voucher_gst_details (
            id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
            place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
            cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
        )
        VALUES ('vgst_m18_1', ?, ?, 'led_party_m18', '29AAACB1234F1Z5', '7208', '29', 'IntraState', 'Taxable', '10000.00', '18.00', '900.00', '900.00', '0.00', '0.00', 0, 0)
        "#,
    )
    .bind(&comp1)
    .bind(&posted.voucher.id)
    .execute(pool)
    .await
    .expect("seed voucher_gst_details");

    (dir, db, comp1, comp2, posted.voucher.id)
}

#[tokio::test]
async fn test_sandbox_gsp_einvoice_and_eway_bill_persistence_and_pdf() {
    let (_dir, db, comp1, _comp2, vch_id) = setup_m18_db().await;
    let pool = db.pool();

    // 1. Push e-Invoice via default Sandbox GSP
    let einv_resp = GspConnectorService::push_einvoice_to_gsp(pool, &comp1, &vch_id, "admin")
        .await
        .expect("sandbox einvoice push");
    assert_eq!(einv_resp.irn.len(), 64);
    assert_eq!(einv_resp.provider_mode, "Sandbox");
    assert!(!einv_resp.ack_no.is_empty());

    // 2. Push e-Way Bill via Sandbox GSP
    let ewb_resp = GspConnectorService::push_eway_bill_to_gsp(pool, &comp1, &vch_id, "admin")
        .await
        .expect("sandbox eway bill push");
    assert_eq!(ewb_resp.ewb_no.len(), 12);
    assert_eq!(ewb_resp.provider_mode, "Sandbox");

    // 3. Verify persisted in voucher_einvoice_details and rendered in Ctrl+P Tax Invoice PDF
    let pdf_bytes = generate_invoice_pdf_bytes(pool, &comp1, &vch_id)
        .await
        .expect("generate invoice pdf");
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_str.contains(&einv_resp.irn));
    assert!(pdf_str.contains(&ewb_resp.ewb_no));
}

#[tokio::test]
async fn test_gsp_https_enforcement_secret_redaction_and_rbac() {
    let (_dir, db, comp1, _comp2, _vch_id) = setup_m18_db().await;
    let pool = db.pool();

    // 1. Reject insecure external http:// URL
    let err = GspConnectorService::save_config(
        pool,
        &comp1,
        SaveCompanyGspConfigInput {
            provider_mode: "HttpGsp".to_string(),
            provider_name: "Custom GSP".to_string(),
            base_url: Some("http://insecure-gsp.example.com".to_string()),
            client_id: Some("cid_123".to_string()),
            api_secret_ref: Some("super_secret_token_999".to_string()),
            is_enabled: true,
        },
        "admin",
    )
    .await
    .unwrap_err();
    assert!(matches!(err, OpenTallyError::Validation(_)));

    // 2. Accept https:// URL and verify secret is redacted from DTO and audit_log
    assert!(HttpGspProvider::validate_url("http://127.0.0.1:8080").is_ok());
    let saved = GspConnectorService::save_config(
        pool,
        &comp1,
        SaveCompanyGspConfigInput {
            provider_mode: "HttpGsp".to_string(),
            provider_name: "ClearTax BYO".to_string(),
            base_url: Some("https://gsp.example.com/v1".to_string()),
            client_id: Some("cid_123".to_string()),
            api_secret_ref: Some("super_secret_token_999".to_string()),
            is_enabled: true,
        },
        "admin",
    )
    .await
    .expect("save https gsp config");
    assert!(saved.has_api_secret);

    let audit_details: Vec<String> =
        sqlx::query_scalar("SELECT COALESCE(new_values, '') FROM audit_log WHERE company_id = ?")
            .bind(&comp1)
            .fetch_all(pool)
            .await
            .unwrap();
    for d in &audit_details {
        assert!(!d.contains("super_secret_token_999"));
    }

    // 3. RBAC check: Auditor & DataEntry cannot ManageUsers; Auditor cannot PostVoucher
    assert!(!UserRole::Auditor.can_perform(ActionPermission::PostVoucher));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::ManageUsers));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::ManageUsers));
}

#[tokio::test]
async fn test_custom_reports_allowlist_totals_csv_and_template_roundtrip() {
    let (_dir, db, comp1, comp2, _vch_id) = setup_m18_db().await;
    let pool = db.pool();

    // 1. Reject SQL injection in column / filter / sort_by
    let inj_err = CustomReportService::save_report(
        pool,
        &comp1,
        SaveCustomReportInput {
            id: None,
            name: "Bad Report".to_string(),
            description: None,
            data_source: "Vouchers".to_string(),
            columns: vec!["voucher_number; DROP TABLE vouchers;".to_string()],
            filters: None,
            sort_by: None,
            sort_dir: Some("ASC".to_string()),
        },
        "admin",
    )
    .await
    .unwrap_err();
    assert!(matches!(inj_err, OpenTallyError::Validation(_)));

    // 2. Create valid declarative Vouchers report
    let def = CustomReportService::save_report(
        pool,
        &comp1,
        SaveCustomReportInput {
            id: None,
            name: "Sales Register Custom".to_string(),
            description: Some("Posted sales vouchers".to_string()),
            data_source: "Vouchers".to_string(),
            columns: vec![
                "voucher_number".to_string(),
                "date".to_string(),
                "narration".to_string(),
                "total_amount".to_string(),
            ],
            filters: None,
            sort_by: Some("date".to_string()),
            sort_dir: Some("ASC".to_string()),
        },
        "admin",
    )
    .await
    .expect("save custom report");

    // 3. Run report and verify exact rust_decimal total
    let exec = CustomReportService::run_report(pool, &comp1, &def.id)
        .await
        .expect("run custom report");
    assert_eq!(exec.row_count, 1);
    assert_eq!(exec.numeric_totals.get("total_amount").unwrap(), "11800.00");

    // 4. Verify CWE-1236 CSV sanitization (=SUM(A1) -> '=SUM(A1))
    let csv = CustomReportService::export_report_csv(pool, &comp1, &def.id)
        .await
        .expect("export csv");
    assert!(csv.contains("'=SUM(A1)"));

    // 5. Export template & import into comp2 (multi-company isolation)
    let tmpl_json = CustomReportService::export_template(pool, &comp1, &def.id)
        .await
        .expect("export template");
    let imported = CustomReportService::import_template(pool, &comp2, &tmpl_json, "admin")
        .await
        .expect("import template into comp2");
    assert_eq!(imported.company_id, comp2);
    assert_eq!(imported.name, "Sales Register Custom");

    // comp2 has 0 vouchers -> row_count == 0
    let exec_comp2 = CustomReportService::run_report(pool, &comp2, &imported.id)
        .await
        .expect("run in comp2");
    assert_eq!(exec_comp2.row_count, 0);

    // 6. Verify audit_log SHA-256 hash chain integrity
    let chain = AuditCertificationService::verify_audit_trail_integrity(pool, &comp1)
        .await
        .expect("verify audit chain");
    assert!(chain.is_valid);
}
