use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::interoperability::{
    execute_tally_xml_import, export_company_tally_xml, preview_tally_xml_import,
};
use crate::vouchers::model::{CreateVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::{VoucherDraftService, VoucherPostingService};

async fn setup_interop_fixture() -> (tempfile::TempDir, DatabaseManager) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("interop_test.db"))
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

    seed_interop_fixture(p).await;
    (dir, db)
}

async fn seed_interop_fixture(p: &sqlx::Pool<sqlx::Sqlite>) {
    // 1. Companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_export_a', 'Exporting Alpha Ltd', 'Alpha Ltd', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                ('cmp_import_b', 'Importing Beta Ltd', 'Beta Ltd', 'India', 'Karnataka', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    // 2. Account Groups in cmp_export_a
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ca_a', 'cmp_export_a', 'Sundry Debtors', NULL, 'Assets', 1),
                ('grp_sales_a', 'cmp_export_a', 'Sales Accounts', NULL, 'Income', 1),
                ('grp_bank_a', 'cmp_export_a', 'Bank Accounts', NULL, 'Assets', 1)",
    )
    .execute(p)
    .await
    .unwrap();

    // 3. Ledgers in cmp_export_a
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
         VALUES ('led_cust_a', 'cmp_export_a', 'grp_ca_a', 'Acme Corp Client', 0, 'standard'),
                ('led_sales_a', 'cmp_export_a', 'grp_sales_a', 'Domestic Sales Account', 0, 'standard'),
                ('led_bank_a', 'cmp_export_a', 'grp_bank_a', 'State Bank Current A/c', 0, 'standard')",
    )
    .execute(p)
    .await
    .unwrap();

    // 4. Units in cmp_export_a
    sqlx::query(
        "INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision)
         VALUES ('unit_nos_a', 'cmp_export_a', 'NOS', 'Numbers', 'simple', 0)",
    )
    .execute(p)
    .await
    .unwrap();

    // 5. Godowns in cmp_export_a
    sqlx::query(
        "INSERT INTO godowns (id, company_id, name)
         VALUES ('godown_main_a', 'cmp_export_a', 'Main Godown Hub')",
    )
    .execute(p)
    .await
    .unwrap();

    // 6. Stock Groups in cmp_export_a
    sqlx::query(
        "INSERT INTO stock_groups (id, company_id, name)
         VALUES ('stk_grp_hw_a', 'cmp_export_a', 'Computer Hardware')",
    )
    .execute(p)
    .await
    .unwrap();

    // 7. Stock Items in cmp_export_a
    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, opening_quantity, opening_rate, opening_value)
         VALUES ('item_server_a', 'cmp_export_a', 'Rack Server X1', 'stk_grp_hw_a', 'unit_nos_a', '10', '40000.00', '400000.00')",
    )
    .execute(p)
    .await
    .unwrap();

    // 8. Voucher Number Series & Configs in cmp_export_a
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('series_jnl_a', 'cmp_export_a', 'vt_jnl', 'General Journal Series', 'JNL', 'automatic', 'yearly')",
    )
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_jnl_a', 'cmp_export_a', 'vt_jnl', 'series_jnl_a', '2026-04-01', 'JNL-', '', 1, 4)",
    )
    .execute(p)
    .await
    .unwrap();

    // 9. Post a balanced voucher in cmp_export_a
    let draft_req = CreateVoucherDraftRequest {
        voucher_type_id: "vt_jnl".to_string(),
        series_id: "series_jnl_a".to_string(),
        date: "2026-05-10".to_string(),
        effective_date: None,
        reference_no: Some("REF-XML-001".to_string()),
        reference_date: None,
        narration: Some("Initial export test transaction".to_string()),
        lines: Some(vec![
            VoucherLineInput {
                id: None,
                ledger_id: "led_cust_a".to_string(),
                entry_type: "Dr".to_string(),
                amount: "50000.00".to_string(),
                narration: Some("Debit client for hardware supply".to_string()),
            },
            VoucherLineInput {
                id: None,
                ledger_id: "led_sales_a".to_string(),
                entry_type: "Cr".to_string(),
                amount: "50000.00".to_string(),
                narration: Some("Credit domestic sales".to_string()),
            },
        ]),
    };

    let draft = VoucherDraftService::create_draft(p, "cmp_export_a", draft_req)
        .await
        .unwrap();

    VoucherPostingService::post_voucher(p, "cmp_export_a", &draft.voucher.id, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_xml_security_xxe_and_entity_expansion_rejection() {
    let (_dir, db) = setup_interop_fixture().await;
    let pool = db.pool();

    // 1. XXE attack attempt with DOCTYPE
    let xxe_payload = r#"<?xml version="1.0" encoding="utf-8"?>
    <!DOCTYPE foo [ <!ENTITY xxe SYSTEM "file:///etc/passwd"> ]>
    <ENVELOPE>
      <HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>
      <BODY><IMPORTDATA><REQUESTDATA>&xxe;</REQUESTDATA></IMPORTDATA></BODY>
    </ENVELOPE>"#;

    let preview_res = preview_tally_xml_import(pool, "cmp_import_b", xxe_payload).await;
    assert!(preview_res.is_err());
    let err_msg = format!("{}", preview_res.unwrap_err());
    assert!(err_msg.contains("<!DOCTYPE") || err_msg.contains("prohibited"));

    let exec_res = execute_tally_xml_import(pool, "cmp_import_b", xxe_payload).await;
    assert!(exec_res.is_err());

    // 2. Entity expansion attack attempt
    let entity_payload = r#"<?xml version="1.0"?>
    <!ENTITY lol "lol">
    <ENVELOPE><BODY></BODY></ENVELOPE>"#;

    let res2 = preview_tally_xml_import(pool, "cmp_import_b", entity_payload).await;
    assert!(res2.is_err());
}

#[tokio::test]
async fn test_xml_roundtrip_masters_and_vouchers() {
    let (_dir, db) = setup_interop_fixture().await;
    let pool = db.pool();

    // Step 1: Export company cmp_export_a data as canonical Tally XML
    let exported_xml =
        export_company_tally_xml(pool, "cmp_export_a", Some("2026-04-01"), Some("2027-03-31"))
            .await
            .unwrap();

    assert!(exported_xml.contains("<TALLYREQUEST>Import Data</TALLYREQUEST>"));
    assert!(exported_xml.contains("NAME=\"Sundry Debtors\""));
    assert!(exported_xml.contains("NAME=\"Acme Corp Client\""));
    assert!(exported_xml.contains("NAME=\"NOS\""));
    assert!(exported_xml.contains("NAME=\"Main Godown Hub\""));
    assert!(exported_xml.contains("NAME=\"Computer Hardware\""));
    assert!(exported_xml.contains("NAME=\"Rack Server X1\""));
    assert!(exported_xml.contains("<VOUCHER"));
    assert!(exported_xml.contains("<DATE>20260510</DATE>"));
    assert!(exported_xml.contains("50000.00"));

    // Step 2: Dry-run preview import into clean company cmp_import_b
    let preview = preview_tally_xml_import(pool, "cmp_import_b", &exported_xml)
        .await
        .unwrap();

    assert!(preview.can_import);
    assert_eq!(
        preview
            .errors
            .iter()
            .filter(|e| e.severity == "FATAL")
            .count(),
        0
    );
    assert!(preview.groups_to_create >= 3);
    assert!(preview.ledgers_to_create >= 3);
    assert!(preview.stock_items_to_create >= 1);
    assert_eq!(preview.vouchers_to_create, 1);
    assert_eq!(preview.vouchers_skipped, 0);

    // Step 3: Execute atomic import into cmp_import_b
    let import_result = execute_tally_xml_import(pool, "cmp_import_b", &exported_xml)
        .await
        .unwrap();

    assert!(import_result.success);
    assert_eq!(import_result.vouchers_created, 1);
    assert!(import_result.groups_created >= 3);
    assert!(import_result.ledgers_created >= 3);

    // Step 4: Verify masters and voucher exist in cmp_import_b
    let client_ledger_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = 'cmp_import_b' AND name = 'Acme Corp Client'",
    )
    .fetch_optional(pool)
    .await
    .unwrap();
    assert!(client_ledger_id.is_some());

    let sales_ledger_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = 'cmp_import_b' AND name = 'Domestic Sales Account'",
    )
    .fetch_optional(pool)
    .await
    .unwrap();
    assert!(sales_ledger_id.is_some());

    let imported_voucher_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id = 'cmp_import_b' AND status = 'posted'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(imported_voucher_count, 1);

    // Verify double-entry equilibrium of lines in imported voucher
    let lines_sum: (String, String) = sqlx::query_as(
        "SELECT
           CAST(COALESCE(SUM(CASE WHEN entry_type = 'Dr' THEN CAST(amount AS DECIMAL) ELSE 0 END), 0) AS TEXT),
           CAST(COALESCE(SUM(CASE WHEN entry_type = 'Cr' THEN CAST(amount AS DECIMAL) ELSE 0 END), 0) AS TEXT)
         FROM voucher_lines WHERE company_id = 'cmp_import_b'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(lines_sum.0, "50000");
    assert_eq!(lines_sum.1, "50000");

    // Verify MCA audit_log entry was stamped
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_import_b' AND entity_type = 'tally_xml_import'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn test_unbalanced_voucher_rejection_no_partial_writes() {
    let (_dir, db) = setup_interop_fixture().await;
    let pool = db.pool();

    // XML with unbalanced voucher: Dr 1000, Cr 900
    let unbalanced_xml = r#"<ENVELOPE>
      <HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>
      <BODY>
        <IMPORTDATA>
          <REQUESTDATA>
            <TALLYMESSAGE>
              <GROUP NAME="Test Receivables"><PARENT/></GROUP>
            </TALLYMESSAGE>
            <TALLYMESSAGE>
              <LEDGER NAME="Client Unbalanced">
                <PARENT>Test Receivables</PARENT>
              </LEDGER>
            </TALLYMESSAGE>
            <TALLYMESSAGE>
              <LEDGER NAME="Revenue Unbalanced">
                <PARENT>Sales Accounts</PARENT>
              </LEDGER>
            </TALLYMESSAGE>
            <TALLYMESSAGE>
              <VOUCHER VCHTYPE="Journal" ACTION="Create">
                <DATE>20260515</DATE>
                <VOUCHERNUMBER>VCH-UNBAL-01</VOUCHERNUMBER>
                <ALLLEDGERENTRIES.LIST>
                  <LEDGERNAME>Client Unbalanced</LEDGERNAME>
                  <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
                  <AMOUNT>-1000.00</AMOUNT>
                </ALLLEDGERENTRIES.LIST>
                <ALLLEDGERENTRIES.LIST>
                  <LEDGERNAME>Revenue Unbalanced</LEDGERNAME>
                  <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                  <AMOUNT>900.00</AMOUNT>
                </ALLLEDGERENTRIES.LIST>
              </VOUCHER>
            </TALLYMESSAGE>
          </REQUESTDATA>
        </IMPORTDATA>
      </BODY>
    </ENVELOPE>"#;

    // 1. Dry run should detect fatal equilibrium error
    let preview = preview_tally_xml_import(pool, "cmp_import_b", unbalanced_xml)
        .await
        .unwrap();

    assert!(!preview.can_import);
    let fatal_errs: Vec<_> = preview
        .errors
        .iter()
        .filter(|e| e.severity == "FATAL")
        .collect();
    assert!(!fatal_errs.is_empty());
    assert!(fatal_errs
        .iter()
        .any(|e| e.message.contains("Double-entry equilibrium violation")));

    // 2. Execution should fail cleanly and rollback all writes
    let exec_res = execute_tally_xml_import(pool, "cmp_import_b", unbalanced_xml).await;
    assert!(exec_res.is_err());

    // 3. Confirm zero partial writes: no vouchers, no groups created in cmp_import_b
    let vch_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id = 'cmp_import_b' AND voucher_number = 'VCH-UNBAL-01'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(vch_count, 0);

    let grp_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM account_groups WHERE company_id = 'cmp_import_b' AND name = 'Test Receivables'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(grp_count, 0);
}

#[tokio::test]
async fn test_idempotent_master_skipping() {
    let (_dir, db) = setup_interop_fixture().await;
    let pool = db.pool();

    let export_xml =
        export_company_tally_xml(pool, "cmp_export_a", Some("2026-04-01"), Some("2027-03-31"))
            .await
            .unwrap();

    // First import
    let res1 = execute_tally_xml_import(pool, "cmp_import_b", &export_xml)
        .await
        .unwrap();
    assert!(res1.success);
    assert_eq!(res1.vouchers_created, 1);

    // Second import of same XML: all masters and existing vouchers should be skipped
    let preview2 = preview_tally_xml_import(pool, "cmp_import_b", &export_xml)
        .await
        .unwrap();
    assert!(preview2.can_import);
    assert!(preview2.groups_skipped > 0);
    assert!(preview2.ledgers_skipped > 0);
    assert_eq!(preview2.vouchers_skipped, 1);
    assert_eq!(preview2.vouchers_to_create, 0);

    let res2 = execute_tally_xml_import(pool, "cmp_import_b", &export_xml)
        .await
        .unwrap();
    assert!(res2.success);
    assert_eq!(res2.groups_created, 0);
    assert_eq!(res2.ledgers_created, 0);
    assert_eq!(res2.stock_items_created, 0);
    assert_eq!(res2.vouchers_created, 0);
}

#[tokio::test]
async fn test_voucher_before_books_beginning_rejected() {
    let (_dir, db) = setup_interop_fixture().await;
    let pool = db.pool();

    // Voucher with date 20250101 (before books beginning 2026-04-01)
    let pre_date_xml = r#"<ENVELOPE>
      <HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER>
      <BODY>
        <IMPORTDATA>
          <REQUESTDATA>
            <TALLYMESSAGE>
              <GROUP NAME="Current Assets"><PARENT/></GROUP>
            </TALLYMESSAGE>
            <TALLYMESSAGE>
              <LEDGER NAME="Bank Account">
                <PARENT>Current Assets</PARENT>
              </LEDGER>
            </TALLYMESSAGE>
            <TALLYMESSAGE>
              <VOUCHER VCHTYPE="Receipt" ACTION="Create">
                <DATE>20250101</DATE>
                <VOUCHERNUMBER>RCPT-OLD-01</VOUCHERNUMBER>
                <ALLLEDGERENTRIES.LIST>
                  <LEDGERNAME>Bank Account</LEDGERNAME>
                  <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
                  <AMOUNT>-500.00</AMOUNT>
                </ALLLEDGERENTRIES.LIST>
                <ALLLEDGERENTRIES.LIST>
                  <LEDGERNAME>Bank Account</LEDGERNAME>
                  <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
                  <AMOUNT>500.00</AMOUNT>
                </ALLLEDGERENTRIES.LIST>
              </VOUCHER>
            </TALLYMESSAGE>
          </REQUESTDATA>
        </IMPORTDATA>
      </BODY>
    </ENVELOPE>"#;

    let preview = preview_tally_xml_import(pool, "cmp_import_b", pre_date_xml)
        .await
        .unwrap();

    assert!(!preview.can_import);
    assert!(preview
        .errors
        .iter()
        .any(|e| e.message.contains("is before company books beginning date")));
}
