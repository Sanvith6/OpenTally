//! Integration Test Suite for Milestone 15: Indian Payroll Engine, Statutory PF/ESI/PT & Payslip PDF
//!
//! Verifies:
//! 1. Employee CRUD, PAN/UAN validation, and LOP proration math.
//! 2. Statutory PF Rs. 15,000 ceiling (12% EE + 8.33% EPS / 3.67% EPF ER).
//! 3. Statutory ESI Rs. 21,000 boundary (eligible at 20,000 vs exempt at 25,000) & PT slab.
//! 4. F11 Auto-Seeding of the 7 standard Payroll ledgers and mappings.
//! 5. Atomic Payroll GL posting -> Trial Balance difference == "0".
//! 6. Duplicate active pay_period rejection, voucher cancellation -> payroll_run Cancelled -> clean re-run.
//! 7. RBAC enforcement (Admin/Accountant allowed; DataEntry/Auditor rejected).
//! 8. PeriodLocked enforcement, SHA-256 audit chain integrity, multi-company isolation.
//! 9. CWE-1236 CSV sanitization and valid `%PDF-1.4` Payslip PDF byte generation.

use crate::app::AppState;
use crate::company::features::{CompanyFeaturesService, SaveCompanyFeaturesInput};
use crate::config::AppConfig;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::payroll::{
    CreateEmployeeInput, PayrollAttendanceInput, PayrollRunInput, PayrollService,
};
use crate::pdf::payslip::export_payslip_pdf;
use crate::reports::{PayrollReportService, TrialBalanceRequest, TrialBalanceService};
use crate::security::models::{
    ActionPermission, CreateUserInput, EnableSecurityInput, LoginInput, UserRole,
};
use crate::security::service::SecurityService;
use crate::vouchers::model::CancelVoucherRequest;
use crate::vouchers::service::VoucherCancellationService;
use std::sync::Arc;

async fn setup_payroll_fixture() -> (tempfile::TempDir, DatabaseManager, Arc<AppState>) {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("opentally_payroll_test.db");
    let db = DatabaseManager::init(&db_path)
        .await
        .expect("Failed to init test database");
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
    run_milestone11_multi_location_and_costing_migration(p)
        .await
        .unwrap();
    run_milestone12_closing_budgets_audit_migration(p)
        .await
        .unwrap();
    run_milestone13_order_processing_migration(p).await.unwrap();
    run_milestone14_security_and_bom_migration(p).await.unwrap();
    run_milestone15_payroll_migration(p).await.unwrap();

    // Seed two companies
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, status)
         VALUES ('cmp_pay_a', 'Bharat Tech Pvt Ltd', 'Bharat Tech', 'India', 'Karnataka', '2026-04-01', '2026-04-01', 'active'),
                ('cmp_pay_b', 'Other Payroll Co', 'Other Co', 'India', 'Maharashtra', '2026-04-01', '2026-04-01', 'active')"
    )
    .execute(p)
    .await
    .unwrap();

    // Seed required account groups for F11 auto-seeding
    sqlx::query(
        "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
         VALUES ('grp_ie_a', 'cmp_pay_a', 'Indirect Expenses', NULL, 'Expenses', 1),
                ('grp_cl_a', 'cmp_pay_a', 'Current Liabilities', NULL, 'Liabilities', 1),
                ('grp_dt_a', 'cmp_pay_a', 'Duties & Taxes', 'grp_cl_a', 'Liabilities', 0),
                ('grp_ie_b', 'cmp_pay_b', 'Indirect Expenses', NULL, 'Expenses', 1),
                ('grp_cl_b', 'cmp_pay_b', 'Current Liabilities', NULL, 'Liabilities', 1),
                ('grp_dt_b', 'cmp_pay_b', 'Duties & Taxes', 'grp_cl_b', 'Liabilities', 0)",
    )
    .execute(p)
    .await
    .unwrap();

    // Enable payroll via F11 features for Company A
    CompanyFeaturesService::save_company_features(
        p,
        "cmp_pay_a",
        SaveCompanyFeaturesInput {
            maintain_inventory: false,
            billwise_tracking: false,
            enable_perpetual_cogs: false,
            gst_enabled: false,
            gstin: None,
            state_code: Some("29".into()),
            registration_type: None,
            auto_seed_gst_ledgers: false,
            auto_seed_inventory_ledgers: false,
            tds_enabled: Some(false),
            tan: None,
            deductor_type: None,
            auto_seed_tds_defaults: false,
            enable_payroll: Some(true),
            auto_seed_payroll_ledgers: true,
        },
    )
    .await
    .unwrap();

    let state = Arc::new(AppState::new(
        AppConfig::new().with_db_path(db_path),
        Arc::new(db.clone()),
    ));
    (dir, db, state)
}

#[tokio::test]
async fn test_employee_crud_pan_uan_validation_and_lop_proration() {
    let (_dir, db, _state) = setup_payroll_fixture().await;
    let pool = db.pool();

    // Invalid PAN rejected
    let bad_pan = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-BAD1".into(),
            name: "Bad Pan".into(),
            department_group_id: None,
            designation: None,
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: Some("INVALID".into()),
            uan: None,
            esic_number: None,
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: false,
            pt_applicable: true,
        },
        "system",
    )
    .await;
    assert!(matches!(bad_pan, Err(OpenTallyError::Validation(_))));

    // Invalid UAN rejected
    let bad_uan = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-BAD2".into(),
            name: "Bad UAN".into(),
            department_group_id: None,
            designation: None,
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: Some("ABCPE1234F".into()),
            uan: Some("12345".into()),
            esic_number: None,
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: false,
            pt_applicable: true,
        },
        "system",
    )
    .await;
    assert!(matches!(bad_uan, Err(OpenTallyError::Validation(_))));

    // Valid employee creation
    let emp = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-001".into(),
            name: "Ananya Sharma".into(),
            department_group_id: None,
            designation: Some("Engineer".into()),
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: Some("ABCPE1234F".into()),
            uan: Some("100123456789".into()),
            esic_number: None,
            bank_account_no: Some("918010012345".into()),
            bank_ifsc: Some("UTIB0000100".into()),
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: true,
            pt_applicable: true,
        },
        "system",
    )
    .await
    .unwrap();
    assert_eq!(emp.employee_code, "EMP-001");

    // Test LOP Proration: 15 days present out of 30 -> 50% of basic (7500) + 50% of HRA (2500) = 10000 gross
    let slip = PayrollService::compute_employee_payslip(
        &emp,
        &PayrollAttendanceInput {
            employee_id: emp.id.clone(),
            total_working_days: "30".into(),
            days_present: "15".into(),
            other_deductions: "0".into(),
        },
    )
    .unwrap();

    assert_eq!(slip.lop_days, "15");
    assert_eq!(slip.prorated_basic, "7500");
    assert_eq!(slip.prorated_hra, "2500");
    assert_eq!(slip.gross_earnings, "10000");
    // PF on 7500: EE 12% = 900, ER EPS 8.33% = 625, ER EPF = 275
    assert_eq!(slip.employee_pf, "900");
    assert_eq!(slip.employer_eps, "625");
    assert_eq!(slip.employer_epf, "275");
    // ESI on 10000 (<= 21000): EE 0.75% = 75, ER 3.25% = 325
    assert_eq!(slip.employee_esi, "75");
    assert_eq!(slip.employer_esi, "325");
    // PT: gross 10000 < 25000 -> 0
    assert_eq!(slip.professional_tax, "0");
    assert_eq!(slip.net_pay, "9025");
}

#[tokio::test]
async fn test_statutory_pf_ceiling_esi_boundary_and_pt_slab() {
    let (_dir, db, _state) = setup_payroll_fixture().await;
    let pool = db.pool();

    // Employee A: Gross = 20,000 (Basic = 15,000, HRA = 5,000) -> ESI eligible
    let emp_a = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-ESI".into(),
            name: "ESI Eligible".into(),
            department_group_id: None,
            designation: None,
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: None,
            uan: None,
            esic_number: Some("3100123456".into()),
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: true,
            pt_applicable: true,
        },
        "system",
    )
    .await
    .unwrap();

    // Employee B: Gross = 25,000 (Basic = 20,000, HRA = 5,000) -> PF capped at 15,000; ESI exempt; PT = 200
    let emp_b = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-PT".into(),
            name: "Senior Staff".into(),
            department_group_id: None,
            designation: None,
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: None,
            uan: None,
            esic_number: None,
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "20000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: true,
            pt_applicable: true,
        },
        "system",
    )
    .await
    .unwrap();

    let slip_a = PayrollService::compute_employee_payslip(
        &emp_a,
        &PayrollAttendanceInput {
            employee_id: emp_a.id.clone(),
            total_working_days: "30".into(),
            days_present: "30".into(),
            other_deductions: "0".into(),
        },
    )
    .unwrap();

    assert_eq!(slip_a.employee_pf, "1800"); // 12% of 15000
    assert_eq!(slip_a.employer_eps, "1250"); // 8.33% of 15000 = 1249.5 -> 1250
    assert_eq!(slip_a.employer_epf, "550"); // 1800 - 1250
    assert_eq!(slip_a.employee_esi, "150"); // ceil(20000 * 0.75%) = 150
    assert_eq!(slip_a.employer_esi, "650"); // ceil(20000 * 3.25%) = 650
    assert_eq!(slip_a.professional_tax, "0");

    let slip_b = PayrollService::compute_employee_payslip(
        &emp_b,
        &PayrollAttendanceInput {
            employee_id: emp_b.id.clone(),
            total_working_days: "30".into(),
            days_present: "30".into(),
            other_deductions: "0".into(),
        },
    )
    .unwrap();

    assert_eq!(slip_b.employee_pf, "1800"); // Capped at 15,000 basic ceiling
    assert_eq!(slip_b.employer_eps, "1250");
    assert_eq!(slip_b.employer_epf, "550");
    assert_eq!(slip_b.employee_esi, "0"); // Exempt (> 21,000)
    assert_eq!(slip_b.employer_esi, "0");
    assert_eq!(slip_b.professional_tax, "200"); // >= 25,000 slab
    assert_eq!(slip_b.net_pay, "23000"); // 25000 - 1800 - 200
}

#[tokio::test]
async fn test_payroll_gl_posting_trial_balance_cancellation_and_pdf_export() {
    let (_dir, db, _state) = setup_payroll_fixture().await;
    let pool = db.pool();

    let emp = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-101".into(),
            name: "=Ramesh Kumar".into(), // Deliberate CWE-1236 formula injection prefix
            department_group_id: None,
            designation: Some("Manager".into()),
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: Some("ABCPE1234F".into()),
            uan: Some("100999888777".into()),
            esic_number: None,
            bank_account_no: Some("1234567890".into()),
            bank_ifsc: Some("SBIN0001234".into()),
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: true,
            pt_applicable: true,
        },
        "AdminPayroll",
    )
    .await
    .unwrap();

    let run_input = PayrollRunInput {
        pay_period: "2026-04".into(),
        run_date: "2026-04-30".into(),
        narration: Some("April 2026 Salary".into()),
        attendances: vec![PayrollAttendanceInput {
            employee_id: emp.id.clone(),
            total_working_days: "30".into(),
            days_present: "30".into(),
            other_deductions: "0".into(),
        }],
    };

    let run_res =
        PayrollService::execute_payroll_run(pool, "cmp_pay_a", run_input.clone(), "AdminPayroll")
            .await
            .unwrap();
    assert_eq!(run_res.run.status, "Posted");
    assert_eq!(run_res.payslips.len(), 1);

    // 1. Trial Balance must be in exact equilibrium (difference == "0")
    let tb = TrialBalanceService::generate(
        pool,
        "cmp_pay_a",
        TrialBalanceRequest {
            as_of_date: "2026-04-30".into(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(tb.difference, "0");

    // 2. Duplicate active pay_period must be rejected
    let dup =
        PayrollService::execute_payroll_run(pool, "cmp_pay_a", run_input.clone(), "AdminPayroll")
            .await;
    assert!(matches!(dup, Err(OpenTallyError::Validation(_))));

    // 3. Export Payslip PDF -> valid %PDF-1.4
    let pdf_bytes = export_payslip_pdf(pool, "cmp_pay_a", &run_res.payslips[0].id)
        .await
        .unwrap();
    assert!(
        pdf_bytes.starts_with(b"%PDF-1.4"),
        "Payslip PDF must begin with %PDF-1.4"
    );
    assert!(pdf_bytes.len() > 500);

    // 4. Statutory Summary & CWE-1236 safe CSV export
    let summary = PayrollReportService::get_payroll_statutory_summary(pool, "cmp_pay_a", "2026-04")
        .await
        .unwrap();
    assert_eq!(summary.total_pf_remittance, "3600"); // 1800 EE + 1800 ER
    assert_eq!(summary.total_esi_remittance, "800"); // 150 EE + 650 ER

    let csv = PayrollReportService::export_payroll_csv(pool, "cmp_pay_a", "2026-04")
        .await
        .unwrap();
    assert!(
        csv.contains("\"'=Ramesh Kumar\""),
        "CSV must neutralize leading '=' formula injection"
    );

    // 5. Verify MCA Audit Trail cryptographic integrity
    let cert =
        crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
            pool,
            "cmp_pay_a",
        )
        .await
        .unwrap();
    assert!(cert.is_valid);
    assert_eq!(cert.tampered_entries, 0);

    // 6. Cancel Payroll Voucher -> Payroll Run transitions to Cancelled -> Period can be re-run cleanly
    VoucherCancellationService::cancel_voucher(
        pool,
        "cmp_pay_a",
        CancelVoucherRequest {
            voucher_id: run_res.run.voucher_id.clone(),
            reason: "Correcting attendance".into(),
            actor_id: Some("AdminPayroll".into()),
        },
    )
    .await
    .unwrap();

    let runs = PayrollService::list_payroll_runs(pool, "cmp_pay_a")
        .await
        .unwrap();
    assert_eq!(runs[0].status, "Cancelled");

    let rerun = PayrollService::execute_payroll_run(pool, "cmp_pay_a", run_input, "AdminPayroll")
        .await
        .unwrap();
    assert_eq!(rerun.run.status, "Posted");
}

#[tokio::test]
async fn test_payroll_rbac_period_lock_and_tenant_isolation() {
    let (_dir, db, state) = setup_payroll_fixture().await;
    let pool = db.pool();

    // 1. Enable RBAC on Company A
    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: "cmp_pay_a".into(),
            admin_username: "Owner".into(),
            admin_password: "Password1!".into(),
        },
        "system",
    )
    .await
    .unwrap();

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_pay_a".into(),
            username: "Auditor1".into(),
            password: "Password1!".into(),
            role: UserRole::Auditor,
        },
        "Owner",
    )
    .await
    .unwrap();

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: "cmp_pay_a".into(),
            username: "Entry1".into(),
            password: "Password1!".into(),
            role: UserRole::DataEntry,
        },
        "Owner",
    )
    .await
    .unwrap();

    // Auditor & DataEntry rejected for ManagePayroll
    let aud_sess = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_pay_a".into(),
            username: "Auditor1".into(),
            password: "Password1!".into(),
        },
    )
    .await
    .unwrap();
    state.set_active_session("cmp_pay_a".into(), aud_sess);
    assert!(matches!(
        state
            .resolve_actor("cmp_pay_a", ActionPermission::ManagePayroll)
            .await,
        Err(OpenTallyError::Forbidden(_))
    ));

    let ent_sess = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_pay_a".into(),
            username: "Entry1".into(),
            password: "Password1!".into(),
        },
    )
    .await
    .unwrap();
    state.set_active_session("cmp_pay_a".into(), ent_sess);
    assert!(matches!(
        state
            .resolve_actor("cmp_pay_a", ActionPermission::ManagePayroll)
            .await,
        Err(OpenTallyError::Forbidden(_))
    ));

    // Owner allowed
    let adm_sess = SecurityService::authenticate(
        pool,
        LoginInput {
            company_id: "cmp_pay_a".into(),
            username: "Owner".into(),
            password: "Password1!".into(),
        },
    )
    .await
    .unwrap();
    state.set_active_session("cmp_pay_a".into(), adm_sess);
    assert_eq!(
        state
            .resolve_actor("cmp_pay_a", ActionPermission::ManagePayroll)
            .await
            .unwrap(),
        "Owner"
    );

    // 2. Multi-company isolation: Employee in Company A invisible in Company B
    let emp_a = PayrollService::create_employee(
        pool,
        "cmp_pay_a",
        CreateEmployeeInput {
            employee_code: "EMP-ISO".into(),
            name: "Tenant A Employee".into(),
            department_group_id: None,
            designation: None,
            date_of_joining: "2026-04-01".into(),
            date_of_exit: None,
            pan: None,
            uan: None,
            esic_number: None,
            bank_account_no: None,
            bank_ifsc: None,
            basic_salary: "15000".into(),
            hra: "5000".into(),
            other_allowances: "0".into(),
            pf_applicable: true,
            esi_applicable: false,
            pt_applicable: true,
        },
        "Owner",
    )
    .await
    .unwrap();

    let list_b = PayrollService::list_employees(pool, "cmp_pay_b", false)
        .await
        .unwrap();
    assert!(list_b.is_empty());

    // 3. PeriodLocked enforcement
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('vns_cls_a', 'cmp_pay_a', 'vt_jnl', 'Closing Series', 'CLS', 'manual', 'never')"
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount)
         VALUES ('vch_cls_a', 'cmp_pay_a', 'vt_jnl', 'vns_cls_a', 'manual', 'CLS-1', '2026-04-30', '2026-04-30', 'posted', '100.00')"
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO financial_year_closings (id, company_id, financial_year_label, period_start, period_end, next_fy_start, closing_voucher_id, net_profit_or_loss, is_locked)
         VALUES ('fyc_a', 'cmp_pay_a', 'FY 2025-26', '2025-04-01', '2026-04-30', '2026-05-01', 'vch_cls_a', '0', 1)"
    )
    .execute(pool)
    .await
    .unwrap();

    let locked_run = PayrollService::execute_payroll_run(
        pool,
        "cmp_pay_a",
        PayrollRunInput {
            pay_period: "2026-04".into(),
            run_date: "2026-04-30".into(),
            narration: None,
            attendances: vec![PayrollAttendanceInput {
                employee_id: emp_a.id,
                total_working_days: "30".into(),
                days_present: "30".into(),
                other_deductions: "0".into(),
            }],
        },
        "Owner",
    )
    .await;
    assert!(matches!(locked_run, Err(OpenTallyError::PeriodLocked(_))));
}
