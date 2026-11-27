use crate::company::model::CreateCompanyRequest;
use crate::company::service::CompanyService;
use crate::database::DatabaseManager;
use crate::payroll::models::CreateEmployeeInput;
use crate::payroll::service::PayrollService;
use crate::reports::TdsReportService;
use rust_decimal::Decimal;
use std::str::FromStr;
use tempfile::TempDir;

async fn setup_m22_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m22_pan_india_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("db init");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "PanIndia Test Corp".to_string(),
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
async fn test_m22_state_wise_pt_slabs_and_mh_february() {
    let (_dir, db, company_id) = setup_m22_db().await;
    let slabs = PayrollService::list_pt_slabs(db.pool(), &company_id, None)
        .await
        .expect("list_pt_slabs");

    // Karnataka (29): >= 25,000 -> 200
    assert_eq!(
        PayrollService::compute_state_pt(&slabs, "29", 5, Decimal::from(30_000)),
        Decimal::from(200)
    );
    // Maharashtra (27): > 10,000 -> 200 in regular month, 300 in February (month 2)
    assert_eq!(
        PayrollService::compute_state_pt(&slabs, "27", 5, Decimal::from(15_000)),
        Decimal::from(200)
    );
    assert_eq!(
        PayrollService::compute_state_pt(&slabs, "27", 2, Decimal::from(15_000)),
        Decimal::from(300)
    );
    // Gujarat (24): <= 12,000 -> 0, > 12,000 -> 200
    assert_eq!(
        PayrollService::compute_state_pt(&slabs, "24", 5, Decimal::from(12_000)),
        Decimal::ZERO
    );
    assert_eq!(
        PayrollService::compute_state_pt(&slabs, "24", 5, Decimal::from(18_000)),
        Decimal::from(200)
    );
}

#[tokio::test]
async fn test_m22_salary_tds_sec192_new_vs_old_regime() {
    // New Regime: 1,00,000/month = 12,00,000/year -> after 75k std ded = 11,25,000 <= 12L -> 87A rebate -> 0
    assert_eq!(
        PayrollService::compute_salary_tds_sec192(Decimal::from(100_000), "New", Decimal::ZERO),
        Decimal::ZERO
    );
    // New Regime: 1,50,000/month = 18,00,000/year -> taxable 17,25,000 > 12L -> > 0 with 4% Cess
    let tds_new =
        PayrollService::compute_salary_tds_sec192(Decimal::from(150_000), "New", Decimal::ZERO);
    assert!(tds_new > Decimal::ZERO);

    // Old Regime: 1,50,000/month = 18,00,000/year with 1,50,000 80C -> taxable 16,00,000
    let tds_old = PayrollService::compute_salary_tds_sec192(
        Decimal::from(150_000),
        "Old",
        Decimal::from(150_000),
    );
    assert!(tds_old > tds_new);
}

#[tokio::test]
async fn test_m22_epfo_ecr_format_and_employee_fields() {
    let (_dir, db, company_id) = setup_m22_db().await;
    let emp = PayrollService::create_employee(
        db.pool(),
        &company_id,
        CreateEmployeeInput {
            employee_code: "EMP-M22-01".to_string(),
            name: "Asha Sharma".to_string(),
            department_group_id: None,
            designation: Some("Engineer".to_string()),
            date_of_joining: "2026-04-01".to_string(),
            date_of_exit: None,
            pan: Some("ABCPK1234F".to_string()),
            uan: Some("100123456789".to_string()),
            esic_number: None,
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "30000".to_string(),
            hra: "10000".to_string(),
            other_allowances: "5000".to_string(),
            pf_applicable: true,
            esi_applicable: false,
            pt_applicable: true,
            work_state_code: "27".to_string(),
            tax_regime: "New".to_string(),
            annual_declared_deductions: "0".to_string(),
        },
        "test-actor",
    )
    .await
    .expect("create employee");
    assert_eq!(emp.work_state_code, "27");
    assert_eq!(emp.tax_regime, "New");

    // Empty ECR when no posted run yet
    let ecr = PayrollService::export_epfo_ecr_file(db.pool(), &company_id, "2026-05")
        .await
        .expect("export_epfo_ecr_file");
    assert_eq!(ecr, "");
}

#[tokio::test]
async fn test_m22_form_27q_and_27eq_isolation() {
    let (_dir, db, company_id) = setup_m22_db().await;
    let q27 = TdsReportService::get_form_27q_summary(db.pool(), &company_id, "2026-2027", "Q1")
        .await
        .expect("27q");
    assert_eq!(q27.company_id, company_id);
    assert!(q27.deductions.is_empty());

    let q27eq = TdsReportService::get_form_27eq_summary(db.pool(), &company_id, "2026-2027", "Q1")
        .await
        .expect("27eq");
    assert_eq!(q27eq.company_id, company_id);
    assert!(q27eq.deductions.is_empty());

    let json27 = TdsReportService::export_form_27q_json(db.pool(), &company_id, "2026-2027", "Q1")
        .await
        .expect("27q json");
    assert!(json27.contains("2026-2027"));
    let _ = Decimal::from_str("0").unwrap();
}
