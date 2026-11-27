use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::pdf::statements::{
    export_balance_sheet_pdf, export_ledger_statement_pdf, export_outstanding_aging_pdf,
    export_profit_and_loss_pdf, export_trial_balance_pdf,
};
use tempfile::TempDir;

async fn setup_pdf_db() -> (TempDir, DatabaseManager, String, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m24_statement_pdfs.db");
    let db = DatabaseManager::init(&db_path).await.expect("db init");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M24 Statement Corp".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .expect("create company");

    let ledger_id: String =
        sqlx::query_scalar("SELECT id FROM ledgers WHERE company_id = ? LIMIT 1")
            .bind(&comp.id)
            .fetch_one(db.pool())
            .await
            .expect("ledger");

    (dir, db, comp.id, ledger_id)
}

fn assert_valid_pdf(bytes: &[u8], label: &str) {
    let s = String::from_utf8_lossy(bytes);
    assert!(
        s.starts_with("%PDF-1.4"),
        "{} must start with %PDF-1.4",
        label
    );
    assert!(s.contains("%%EOF"), "{} must contain %%EOF", label);
    assert!(
        s.contains("M24 Statement Corp"),
        "{} must contain company name",
        label
    );
}

#[tokio::test]
async fn test_m24_all_5_financial_statement_pdfs() {
    let (_dir, db, company_id, ledger_id) = setup_pdf_db().await;
    let pool = db.pool();

    let tb = export_trial_balance_pdf(pool, &company_id, "2027-03-31")
        .await
        .expect("tb pdf");
    assert_valid_pdf(&tb, "Trial Balance PDF");

    let pl = export_profit_and_loss_pdf(pool, &company_id, "2026-04-01", "2027-03-31")
        .await
        .expect("pl pdf");
    assert_valid_pdf(&pl, "Profit & Loss PDF");

    let bs = export_balance_sheet_pdf(pool, &company_id, "2027-03-31")
        .await
        .expect("bs pdf");
    assert_valid_pdf(&bs, "Balance Sheet PDF");

    let ls = export_ledger_statement_pdf(pool, &company_id, &ledger_id, "2026-04-01", "2027-03-31")
        .await
        .expect("ledger pdf");
    assert_valid_pdf(&ls, "Ledger Statement PDF");

    let oa = export_outstanding_aging_pdf(pool, &company_id, "Receivable", "2027-03-31")
        .await
        .expect("aging pdf");
    assert_valid_pdf(&oa, "Outstanding Aging PDF");
}
