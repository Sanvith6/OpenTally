use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::interoperability::import::{execute_tally_xml_import, preview_tally_xml_import};
use crate::interoperability::parser::parse_tally_xml;
use tempfile::TempDir;

async fn setup_m21_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m21_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("init db");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M21 XML Corp".to_string(),
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

    (dir, db, comp.id)
}

#[tokio::test]
async fn test_m21_nested_4_dimension_xml_parsing_and_xxe_rejection() {
    let (_dir, db, company_id) = setup_m21_db().await;
    let xxe_xml = r#"<?xml version="1.0"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM "file:///etc/passwd">]><ENVELOPE></ENVELOPE>"#;
    let err = preview_tally_xml_import(db.pool(), &company_id, xxe_xml)
        .await
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("doctype")
            || err.to_string().to_lowercase().contains("xxe")
    );

    let nested_xml = r#"<ENVELOPE>
      <BODY><IMPORTDATA><REQUESTDATA>
        <TALLYMESSAGE>
          <VOUCHER VCHTYPE="Sales" ACTION="Create">
            <DATE>20260510</DATE>
            <VOUCHERNUMBER>SAL/001</VOUCHERNUMBER>
            <PARTYGSTIN>27AABCU9603R1ZN</PARTYGSTIN>
            <PLACEOFSUPPLY>Maharashtra</PLACEOFSUPPLY>
            <ALLLEDGERENTRIES.LIST>
              <LEDGERNAME>Customer A</LEDGERNAME>
              <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
              <AMOUNT>-1180.00</AMOUNT>
              <BILLALLOCATIONS.LIST>
                <NAME>BILL-001</NAME>
                <BILLTYPE>New Ref</BILLTYPE>
                <BILLCREDITPERIOD>30 Days</BILLCREDITPERIOD>
                <AMOUNT>1180.00</AMOUNT>
              </BILLALLOCATIONS.LIST>
            </ALLLEDGERENTRIES.LIST>
            <ALLLEDGERENTRIES.LIST>
              <LEDGERNAME>HDFC Bank</LEDGERNAME>
              <ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>
              <AMOUNT>-1180.00</AMOUNT>
              <BANKALLOCATIONS.LIST>
                <TRANSACTIONTYPE>NEFT</TRANSACTIONTYPE>
                <INSTRUMENTNUMBER>NEFT9988</INSTRUMENTNUMBER>
                <INSTRUMENTDATE>20260510</INSTRUMENTDATE>
                <BANKERSDATE>20260511</BANKERSDATE>
                <PAYMENTFAVOURING>Customer A</PAYMENTFAVOURING>
                <AMOUNT>1180.00</AMOUNT>
              </BANKALLOCATIONS.LIST>
            </ALLLEDGERENTRIES.LIST>
            <ALLLEDGERENTRIES.LIST>
              <LEDGERNAME>Sales Account</LEDGERNAME>
              <ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>
              <AMOUNT>2360.00</AMOUNT>
              <CATEGORYALLOCATIONS.LIST>
                <CATEGORY>Primary Cost Category</CATEGORY>
                <COSTCENTREALLOCATIONS.LIST>
                  <NAME>Mumbai Branch</NAME>
                  <AMOUNT>2360.00</AMOUNT>
                </COSTCENTREALLOCATIONS.LIST>
              </CATEGORYALLOCATIONS.LIST>
            </ALLLEDGERENTRIES.LIST>
            <ALLINVENTORYENTRIES.LIST>
              <STOCKITEMNAME>Widget X</STOCKITEMNAME>
              <ACTUALQTY>11 Nos</ACTUALQTY>
              <BILLEDQTY>10 Nos</BILLEDQTY>
              <RATE>100.00/Nos</RATE>
              <AMOUNT>1000.00</AMOUNT>
              <BATCHALLOCATIONS.LIST>
                <GODOWNNAME>Main Godown</GODOWNNAME>
                <BATCHNAME>B-01</BATCHNAME>
                <AMOUNT>1000.00</AMOUNT>
              </BATCHALLOCATIONS.LIST>
            </ALLINVENTORYENTRIES.LIST>
          </VOUCHER>
        </TALLYMESSAGE>
      </REQUESTDATA></IMPORTDATA></BODY>
    </ENVELOPE>"#;

    let doc = parse_tally_xml(nested_xml).expect("parse nested xml");
    assert_eq!(doc.vouchers.len(), 1);
    let v = &doc.vouchers[0];
    assert_eq!(v.lines[0].bill_allocations.len(), 1);
    assert_eq!(v.lines[0].bill_allocations[0].bill_type, "NewRef");
    assert_eq!(v.lines[0].bill_allocations[0].credit_period_days, Some(30));
    assert_eq!(v.lines[1].bank_allocations.len(), 1);
    assert_eq!(
        v.lines[1].bank_allocations[0].bank_date.as_deref(),
        Some("2026-05-11")
    );
    assert_eq!(v.lines[2].cost_allocations.len(), 1);
    assert_eq!(
        v.lines[2].cost_allocations[0].cost_centre_name,
        "Mumbai Branch"
    );
    assert_eq!(v.inventory_entries.len(), 1);
    assert_eq!(v.inventory_entries[0].actual_qty.to_string(), "11");
    assert_eq!(v.inventory_entries[0].billed_qty.to_string(), "10");
    assert_eq!(
        v.inventory_entries[0].batches[0].godown_name.as_deref(),
        Some("Main Godown")
    );
}

#[tokio::test]
async fn test_m21_high_volume_2000_vouchers_streaming_reconciliation() {
    let (_dir, db, company_id) = setup_m21_db().await;

    let mut xml = String::with_capacity(512 * 1024);
    xml.push_str("<ENVELOPE><BODY><IMPORTDATA><REQUESTDATA>");
    xml.push_str("<TALLYMESSAGE><LEDGER NAME=\"Cash\" RESERVEDNAME=\"\"><PARENT>Cash-in-Hand</PARENT></LEDGER></TALLYMESSAGE>");
    xml.push_str("<TALLYMESSAGE><LEDGER NAME=\"Sales Account\" RESERVEDNAME=\"\"><PARENT>Sales Accounts</PARENT></LEDGER></TALLYMESSAGE>");
    for i in 1..=2000 {
        xml.push_str(&format!(
            "<TALLYMESSAGE><VOUCHER VCHTYPE=\"Sales\" ACTION=\"Create\"><DATE>20260515</DATE><VOUCHERNUMBER>HV/{:04}</VOUCHERNUMBER><ALLLEDGERENTRIES.LIST><LEDGERNAME>Cash</LEDGERNAME><ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE><AMOUNT>-100.00</AMOUNT></ALLLEDGERENTRIES.LIST><ALLLEDGERENTRIES.LIST><LEDGERNAME>Sales Account</LEDGERNAME><ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE><AMOUNT>100.00</AMOUNT></ALLLEDGERENTRIES.LIST></VOUCHER></TALLYMESSAGE>",
            i
        ));
    }
    xml.push_str("</REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>");

    let preview = preview_tally_xml_import(db.pool(), &company_id, &xml)
        .await
        .expect("preview 2000");
    assert!(preview.can_import);
    assert_eq!(preview.vouchers_to_create, 2000);

    let res = execute_tally_xml_import(db.pool(), &company_id, &xml)
        .await
        .expect("import 2000");
    assert!(res.success);
    assert_eq!(res.vouchers_created, 2000);
    let rec = res.reconciliation.expect("reconciliation report");
    assert_eq!(rec.source_voucher_count, 2000);
    assert_eq!(rec.imported_voucher_count, 2000);
    assert_eq!(rec.source_total_debit, "200000.00");
    assert_eq!(rec.imported_trial_balance_difference, "0.00");
}
