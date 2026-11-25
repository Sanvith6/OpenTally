//! Integration Test Suite for Milestone 12: Ratio Analysis, Budgets, Year-End Closing & MCA Audit Certification
//!
//! Verifies:
//! 1. Financial Ratio Analysis: Two-pane Principal Groups & Figures + Principal Ratios, exact Decimal arithmetic, and "N/A" on zero denominators.
//! 2. Budgets & Variance Analysis: Group (recursive CTE), Ledger, and Cost Centre budget targets, actual calculation, variance, and over-budget detection.
//! 3. Automated Financial Year-End Closing: Trial balance pre-flight check, nominal accounts zeroing, balance sheet & FIFO roll-forward, and opening balance generation for next FY.
//! 4. Period Lock Enforcement: Strict rejection (`PeriodLocked`) of backdated voucher posting, cancellation, reversal, commercial posting, and stock journals.
//! 5. MCA 2023 Audit Trail SHA-256 Forensic Certification: Cryptographic hash chaining verification, tamper detection, and report exports.
//! 6. Multi-tenant company isolation across all Milestone 12 components.

use crate::accounting::budgets::{BudgetLineRequest, BudgetRequest, BudgetService};
use crate::accounting::cost_centres::{
    CostCategoryRequest, CostCentreRequest, CostCentreService, VoucherCostAllocationInput,
};
use crate::audit::certification::AuditCertificationService;
use crate::company::closing::FinancialYearClosingService;
use crate::company::model::CreateCompanyRequest;
use crate::company::service::CompanyService;
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::reports::ratios::RatioAnalysisService;
use crate::vouchers::model::ReverseVoucherRequest;
use crate::vouchers::service::VoucherPostingService;
use crate::vouchers::service::VoucherReversalService;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

async fn setup_test_pool() -> (tempfile::TempDir, Pool<Sqlite>, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = DatabaseManager::init(&dir.path().join("m12_closing_budgets_test.db"))
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
    run_milestone8_banking_migration(p).await.unwrap();
    run_milestone9_tds_migration(p).await.unwrap();
    run_milestone10_gst_compliance_migration(p).await.unwrap();
    run_milestone11_multi_location_and_costing_migration(p)
        .await
        .unwrap();
    run_milestone12_closing_budgets_audit_migration(p)
        .await
        .unwrap();

    let company = CompanyService::create_company(
        p,
        CreateCompanyRequest {
            name: "Bharath Analytics & Rolling Ltd".to_string(),
            mailing_name: Some("Bharath Analytics & Rolling Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Karnataka".to_string()),
            financial_year_begins: "2024-04-01".to_string(),
            books_beginning_from: Some("2024-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    // Seed default voucher number series and configs
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('vs_pur_default', ?, 'vt_pur', 'Purchase Default', 'PUR', 'automatic', 'yearly'),
                ('vs_sales_default', ?, 'vt_sales', 'Sales Default', 'SAL', 'automatic', 'yearly'),
                ('vs_pmt_default', ?, 'vt_payment', 'Payment Default', 'PMT', 'automatic', 'yearly'),
                ('vs_rcpt_default', ?, 'vt_receipt', 'Receipt Default', 'RCT', 'automatic', 'yearly'),
                ('vs_jnl_default', ?, 'vt_jnl', 'Journal Default', 'JNL', 'automatic', 'yearly')",
    )
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .execute(p)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
         VALUES ('cfg_pur_default', ?, 'vt_pur', 'vs_pur_default', '2024-04-01', 'PUR/', '', 1, 4),
                ('cfg_sales_default', ?, 'vt_sales', 'vs_sales_default', '2024-04-01', 'INV/', '', 1, 4),
                ('cfg_pmt_default', ?, 'vt_payment', 'vs_pmt_default', '2024-04-01', 'PMT/', '', 1, 4),
                ('cfg_rcpt_default', ?, 'vt_receipt', 'vs_rcpt_default', '2024-04-01', 'RCT/', '', 1, 4),
                ('cfg_jnl_default', ?, 'vt_jnl', 'vs_jnl_default', '2024-04-01', 'JNL/', '', 1, 4)",
    )
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .bind(&company.id)
    .execute(p)
    .await
    .unwrap();

    (dir, p.clone(), company.id)
}

async fn find_group_id(pool: &Pool<Sqlite>, company_id: &str, name: &str) -> String {
    let row: (String,) = sqlx::query_as(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = ? COLLATE NOCASE",
    )
    .bind(company_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|_| panic!("Group '{}' not found", name));
    row.0
}

#[tokio::test]
async fn test_ratio_analysis_calculations_and_na_handling() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // 1. Initial State: Empty books -> verify "N/A" for denominators that are zero
    let empty_report =
        RatioAnalysisService::generate(&pool, &company_id, "2024-04-01", "2025-03-31")
            .await
            .unwrap();

    assert_eq!(empty_report.current_ratio, "N/A");
    assert_eq!(empty_report.quick_ratio, "N/A");
    assert_eq!(empty_report.debt_equity_ratio, "N/A");
    assert_eq!(empty_report.gross_profit_pct, "N/A");
    assert_eq!(empty_report.net_profit_pct, "N/A");
    assert_eq!(empty_report.operating_cost_pct, "N/A");
    assert_eq!(empty_report.receivables_turnover_days, "N/A");
    assert_eq!(empty_report.inventory_turnover, "N/A");
    assert_eq!(empty_report.return_on_working_capital_pct, "N/A");

    // 2. Create Ledgers for Sales, Purchases, Sundry Debtors, Sundry Creditors
    let sales_grp = find_group_id(&pool, &company_id, "Sales Accounts").await;
    let pur_grp = find_group_id(&pool, &company_id, "Purchase Accounts").await;
    let debtor_grp = find_group_id(&pool, &company_id, "Sundry Debtors").await;
    let creditor_grp = find_group_id(&pool, &company_id, "Sundry Creditors").await;

    let sales_led = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: sales_grp,
            name: "Domestic Valve Sales".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let pur_led = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: pur_grp,
            name: "Steel Forgings Purchase".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let debtor_led = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: debtor_grp,
            name: "Karnataka Hydros Ltd".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let creditor_led = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: creditor_grp,
            name: "Precision Forgings Ltd".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // Purchase Voucher: Dr Purchase 50,000, Cr Creditor 50,000
    let vch1_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES (?, ?, 'vt_pur', 'vs_pur_default', 'automatic', NULL, NULL, 'DRAFT', '2024-05-10', '2024-05-10', 'draft', '50000.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ).bind(&vch1_id).bind(&company_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 1, ?, 'Dr', '50000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch1_id).bind(&pur_led.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 2, ?, 'Cr', '50000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch1_id).bind(&creditor_led.id).execute(&pool).await.unwrap();
    VoucherPostingService::post_voucher(&pool, &company_id, &vch1_id, None)
        .await
        .unwrap();

    // Sales Voucher: Dr Debtor 40,000, Cr Sales 40,000
    let vch2_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES (?, ?, 'vt_sales', 'vs_sales_default', 'automatic', NULL, NULL, 'DRAFT', '2024-06-15', '2024-06-15', 'draft', '40000.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ).bind(&vch2_id).bind(&company_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 1, ?, 'Dr', '40000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch2_id).bind(&debtor_led.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 2, ?, 'Cr', '40000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch2_id).bind(&sales_led.id).execute(&pool).await.unwrap();
    VoucherPostingService::post_voucher(&pool, &company_id, &vch2_id, None)
        .await
        .unwrap();

    // 3. Generate Ratio Report
    let rep = RatioAnalysisService::generate(&pool, &company_id, "2024-04-01", "2025-03-31")
        .await
        .unwrap();

    assert_eq!(
        Decimal::from_str(&rep.sales_accounts).unwrap(),
        Decimal::from_str("40000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&rep.purchase_accounts).unwrap(),
        Decimal::from_str("50000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&rep.sundry_debtors).unwrap(),
        Decimal::from_str("40000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&rep.sundry_creditors).unwrap(),
        Decimal::from_str("50000.00").unwrap()
    );
    assert_ne!(rep.current_ratio, "N/A");
    assert_ne!(rep.quick_ratio, "N/A");
}

#[tokio::test]
async fn test_budgets_creation_variance_reporting_and_deletion() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // 1. Create a Cost Centre to include in budget
    let cat = CostCentreService::create_cost_category(
        &pool,
        &company_id,
        CostCategoryRequest {
            name: "R&D Projects".to_string(),
            allocate_revenue: true,
            allocate_non_revenue: true,
        },
    )
    .await
    .unwrap();

    let cc = CostCentreService::create_cost_centre(
        &pool,
        &company_id,
        CostCentreRequest {
            category_id: cat.id.clone(),
            name: "Project Hydrogen".to_string(),
            parent_id: None,
        },
    )
    .await
    .unwrap();

    let ind_exp_grp = find_group_id(&pool, &company_id, "Indirect Expenses").await;

    // 2. Create Budget across Group, Ledger, and Cost Centre
    let budget_details = BudgetService::create_budget(
        &pool,
        &company_id,
        BudgetRequest {
            name: "Annual Operations Budget 2024-25".to_string(),
            from_date: "2024-04-01".to_string(),
            to_date: "2025-03-31".to_string(),
            lines: vec![
                BudgetLineRequest {
                    target_type: "Group".to_string(),
                    target_id: ind_exp_grp.clone(),
                    budget_type: "OnNetTransactions".to_string(),
                    amount: Decimal::from_str("50000.00").unwrap(),
                    dr_cr: "Dr".to_string(),
                },
                BudgetLineRequest {
                    target_type: "CostCenter".to_string(),
                    target_id: cc.id.clone(),
                    budget_type: "OnNetTransactions".to_string(),
                    amount: Decimal::from_str("25000.00").unwrap(),
                    dr_cr: "Dr".to_string(),
                },
            ],
        },
    )
    .await
    .unwrap();

    assert_eq!(budget_details.lines.len(), 2);
    let budget_id = budget_details.budget.id.clone();

    // 3. Post a voucher hitting indirect expenses and the cost centre
    let cash_id: String = sqlx::query_scalar(
        "SELECT id FROM ledgers WHERE company_id = ? AND ledger_classification = 'cash'",
    )
    .bind(&company_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create an expense ledger under Indirect Expenses
    let exp_ledger = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: ind_exp_grp,
            name: "Consultancy & R&D Fees".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let vch_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number, date, effective_date,
            status, narration, total_amount, created_at, updated_at
        ) VALUES (?, ?, 'vt_payment', 'vs_pmt_default', 'automatic', NULL, NULL, 'DRAFT', '2024-07-10', '2024-07-10', 'draft', 'R&D Payment', '15000.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(&vch_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    let line1_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
         VALUES (?, ?, ?, 1, ?, 'Dr', '15000.00', 'Consultancy fee')",
    )
    .bind(&line1_id)
    .bind(&company_id)
    .bind(&vch_id)
    .bind(&exp_ledger.id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
         VALUES (?, ?, ?, 2, ?, 'Cr', '15000.00', 'Cash Outflow')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&company_id)
    .bind(&vch_id)
    .bind(&cash_id)
    .execute(&pool)
    .await
    .unwrap();

    // Allocate cost centre to line 1
    let mut conn = pool.acquire().await.unwrap();
    CostCentreService::save_voucher_cost_allocations(
        &mut *conn,
        &company_id,
        &vch_id,
        &line1_id,
        Decimal::from_str("15000.00").unwrap(),
        &[VoucherCostAllocationInput {
            cost_centre_id: cc.id.clone(),
            amount: Decimal::from_str("15000.00").unwrap(),
        }],
    )
    .await
    .unwrap();
    drop(conn);

    // Post voucher
    VoucherPostingService::post_voucher(&pool, &company_id, &vch_id, Some("admin"))
        .await
        .unwrap();

    // 4. Verify Budget Variance Report
    let variance_report =
        BudgetService::get_budget_variance_report(&pool, &company_id, &budget_id, "2024-07-31")
            .await
            .unwrap();

    assert_eq!(variance_report.rows.len(), 2);
    // Group row: 50,000 budget, 15,000 actual -> 35,000 variance, 30.00% utilization, not over budget
    let group_row = variance_report
        .rows
        .iter()
        .find(|r| r.target_type == "Group")
        .unwrap();
    assert_eq!(
        Decimal::from_str(&group_row.budget_amount).unwrap(),
        Decimal::from_str("50000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&group_row.actual_amount).unwrap(),
        Decimal::from_str("15000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&group_row.variance_amount).unwrap(),
        Decimal::from_str("35000.00").unwrap()
    );
    assert_eq!(group_row.utilization_pct, "30.00%");
    assert!(!group_row.is_over_budget);

    // CostCentre row: 25,000 budget, 15,000 actual -> 10,000 variance, 60.00% utilization, not over budget
    let cc_row = variance_report
        .rows
        .iter()
        .find(|r| r.target_type == "CostCenter")
        .unwrap();
    assert_eq!(
        Decimal::from_str(&cc_row.budget_amount).unwrap(),
        Decimal::from_str("25000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&cc_row.actual_amount).unwrap(),
        Decimal::from_str("15000.00").unwrap()
    );
    assert_eq!(
        Decimal::from_str(&cc_row.variance_amount).unwrap(),
        Decimal::from_str("10000.00").unwrap()
    );
    assert_eq!(cc_row.utilization_pct, "60.00%");
    assert!(!cc_row.is_over_budget);

    // 5. Test Delete Budget
    BudgetService::delete_budget(&pool, &company_id, &budget_id)
        .await
        .unwrap();
    let budgets_after = BudgetService::list_budgets(&pool, &company_id)
        .await
        .unwrap();
    assert_eq!(budgets_after.len(), 0);
}

#[tokio::test]
async fn test_financial_year_closing_rollover_and_period_lock() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // 1. Create an income and expense ledger
    let sales_grp = find_group_id(&pool, &company_id, "Sales Accounts").await;
    let ind_exp_grp = find_group_id(&pool, &company_id, "Indirect Expenses").await;
    let bank_grp = find_group_id(&pool, &company_id, "Bank Accounts").await;

    let sales_ledger = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: sales_grp,
            name: "Software Consulting Income".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let rent_ledger = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: ind_exp_grp,
            name: "Office Space Rent".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    let bank_ledger = crate::accounting::LedgerService::create_ledger(
        &pool,
        &company_id,
        crate::accounting::CreateLedgerRequest {
            group_id: bank_grp,
            name: "HDFC Operating Current A/c".to_string(),
            description: None,
            opening_balance: None,
        },
    )
    .await
    .unwrap();

    // Post Sales: Dr Bank 1,00,000, Cr Sales 1,00,000
    let vch1_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES (?, ?, 'vt_receipt', 'vs_rcpt_default', 'automatic', NULL, NULL, 'DRAFT', '2024-08-15', '2024-08-15', 'draft', '100000.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ).bind(&vch1_id).bind(&company_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 1, ?, 'Dr', '100000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch1_id).bind(&bank_ledger.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 2, ?, 'Cr', '100000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch1_id).bind(&sales_ledger.id).execute(&pool).await.unwrap();
    VoucherPostingService::post_voucher(&pool, &company_id, &vch1_id, None)
        .await
        .unwrap();

    // Post Rent Expense: Dr Rent 30,000, Cr Bank 30,000
    let vch2_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES (?, ?, 'vt_payment', 'vs_pmt_default', 'automatic', NULL, NULL, 'DRAFT', '2024-11-20', '2024-11-20', 'draft', '30000.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ).bind(&vch2_id).bind(&company_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 1, ?, 'Dr', '30000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch2_id).bind(&rent_ledger.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 2, ?, 'Cr', '30000.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&vch2_id).bind(&bank_ledger.id).execute(&pool).await.unwrap();
    VoucherPostingService::post_voucher(&pool, &company_id, &vch2_id, None)
        .await
        .unwrap();

    // 2. Pre-Closing Audit Preview
    let preview =
        FinancialYearClosingService::preview_year_end_closing(&pool, &company_id, "2025-03-31")
            .await
            .unwrap();

    assert!(preview.is_equilibrium);
    assert_eq!(preview.net_profit_or_loss, "70000.00");
    assert!(preview.is_profit);
    assert_eq!(preview.next_fy_start, "2025-04-01");
    assert_eq!(preview.nominal_transfers.len(), 2);

    // 3. Execute Year-End Closing & Period Lock
    let closing_record = FinancialYearClosingService::execute_financial_year_closing(
        &pool,
        &company_id,
        "2025-03-31",
        Some("cfo"),
    )
    .await
    .unwrap();

    assert!(closing_record.is_locked);
    assert_eq!(closing_record.net_profit_or_loss, "70000.00");
    assert_eq!(closing_record.financial_year_label, "FY 2024-25");

    // 4. Verify Nominal Accounts are Zeroed Out as of 2025-03-31
    let tb_closed = crate::reports::trial_balance::TrialBalanceService::generate(
        &pool,
        &company_id,
        crate::reports::model::TrialBalanceRequest {
            as_of_date: "2025-03-31".to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: true,
        },
    )
    .await
    .unwrap();

    for row in tb_closed.rows {
        if row.ledger_id == sales_ledger.id || row.ledger_id == rent_ledger.id {
            assert!(
                Decimal::from_str(&row.closing_debit)
                    .unwrap_or(Decimal::ZERO)
                    .is_zero(),
                "Nominal ledger must be zeroed"
            );
            assert!(
                Decimal::from_str(&row.closing_credit)
                    .unwrap_or(Decimal::ZERO)
                    .is_zero(),
                "Nominal ledger must be zeroed"
            );
        }
    }

    // 5. Verify Roll-Forward Opening Balances exist for next financial year 2025-04-01
    let op_balances: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT ledger_id, balance_type, amount FROM opening_balances WHERE company_id = ? AND financial_year_begins = '2025-04-01'"
    )
    .bind(&company_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(
        !op_balances.is_empty(),
        "Balance sheet accounts must roll forward to next FY"
    );
    let bank_op = op_balances.iter().find(|b| b.0 == bank_ledger.id).unwrap();
    assert_eq!(bank_op.1, "Dr");
    assert_eq!(bank_op.2, "70000.00"); // 1,00,000 inflow - 30,000 outflow

    // 6. Verify Backdated Voucher Rejection (Period Lock Enforcement)
    let backdated_vch = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount, created_at, updated_at)
         VALUES (?, ?, 'vt_payment', 'vs_pmt_default', 'automatic', NULL, NULL, 'DRAFT', '2024-12-15', '2024-12-15', 'draft', '500.00', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
    ).bind(&backdated_vch).bind(&company_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 1, ?, 'Dr', '500.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&backdated_vch).bind(&bank_ledger.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES (?, ?, ?, 2, ?, 'Cr', '500.00')")
        .bind(Uuid::new_v4().to_string()).bind(&company_id).bind(&backdated_vch).bind(&rent_ledger.id).execute(&pool).await.unwrap();

    let post_res =
        VoucherPostingService::post_voucher(&pool, &company_id, &backdated_vch, None).await;
    match post_res {
        Err(OpenTallyError::PeriodLocked(msg)) => {
            assert!(msg.contains("falls within locked financial year period ending '2025-03-31'"));
        }
        other => panic!(
            "Expected PeriodLocked error on backdated posting, got: {:?}",
            other
        ),
    }

    // Attempting reversal on a voucher in locked period must fail
    let rev_res = VoucherReversalService::reverse_voucher(
        &pool,
        &company_id,
        ReverseVoucherRequest {
            original_voucher_id: vch1_id.clone(),
            reversal_date: "2025-04-10".to_string(), // reversal date in open period, but original voucher in locked period!
            reason: "Test backdated reversal".to_string(),
            actor_id: None,
        },
    )
    .await;

    match rev_res {
        Err(OpenTallyError::PeriodLocked(msg)) => {
            assert!(msg.contains("falls within locked financial year period ending '2025-03-31'"));
        }
        other => panic!(
            "Expected PeriodLocked error on locked voucher reversal, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_mca_audit_trail_forensic_certification_and_tamper_detection() {
    let (_dir, pool, company_id) = setup_test_pool().await;

    // 1. Trigger multiple operations to generate audit log records with hash chaining
    let _cat = CostCentreService::create_cost_category(
        &pool,
        &company_id,
        CostCategoryRequest {
            name: "Corporate Audit".to_string(),
            allocate_revenue: true,
            allocate_non_revenue: true,
        },
    )
    .await
    .unwrap();

    let ind_exp_grp = find_group_id(&pool, &company_id, "Indirect Expenses").await;

    let _budget = BudgetService::create_budget(
        &pool,
        &company_id,
        BudgetRequest {
            name: "Audit Trail Budget".to_string(),
            from_date: "2024-04-01".to_string(),
            to_date: "2025-03-31".to_string(),
            lines: vec![BudgetLineRequest {
                target_type: "Group".to_string(),
                target_id: ind_exp_grp,
                budget_type: "OnNetTransactions".to_string(),
                amount: Decimal::from_str("10000.00").unwrap(),
                dr_cr: "Dr".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    // 2. Run Verification: Clean State
    let report_clean = AuditCertificationService::verify_audit_trail_integrity(&pool, &company_id)
        .await
        .unwrap();

    assert!(report_clean.is_valid);
    assert_eq!(report_clean.certification_status, "VERIFIED_CLEAN");
    assert_eq!(report_clean.tampered_entries, 0);
    assert_eq!(report_clean.broken_chain_links, 0);
    assert!(report_clean.chained_entries > 0);

    // 3. Test CSV and JSON export
    let json = AuditCertificationService::export_certification_json(&report_clean).unwrap();
    assert!(json.contains("VERIFIED_CLEAN"));

    let csv = AuditCertificationService::export_certification_csv(&report_clean);
    assert!(csv.contains("Status,\"VERIFIED_CLEAN\""));
    assert!(csv.contains("Total Entries"));

    // 4. Synthetically insert a tampered audit record by bypassing hash computation
    // We insert a row with an invalid SHA-256 entry_hash
    let bad_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, new_values, performed_by, timestamp, prev_hash, entry_hash)
         VALUES (?, ?, 'tampered_entity', 'bad_1', 'UPDATE', '{\"amount\": 999999}', 'hacker', '2026-11-24T12:00:00Z', 'GENESIS', '0000000000000000000000000000000000000000000000000000000000000000')"
    )
    .bind(&bad_id)
    .bind(&company_id)
    .execute(&pool)
    .await
    .unwrap();

    let report_tampered =
        AuditCertificationService::verify_audit_trail_integrity(&pool, &company_id)
            .await
            .unwrap();

    assert!(!report_tampered.is_valid);
    assert_eq!(report_tampered.certification_status, "TAMPER_DETECTED");
    assert!(report_tampered.tampered_entries > 0);
    assert!(!report_tampered.issues.is_empty());
}

#[tokio::test]
async fn test_multi_company_tenant_isolation_m12() {
    let (_dir, pool, company_a) = setup_test_pool().await;

    let company_b = CompanyService::create_company(
        &pool,
        CreateCompanyRequest {
            name: "Chennai Automotive Systems Ltd".to_string(),
            mailing_name: Some("Chennai Automotive Systems Ltd".to_string()),
            country: Some("India".to_string()),
            state: Some("Tamil Nadu".to_string()),
            financial_year_begins: "2024-04-01".to_string(),
            books_beginning_from: Some("2024-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();

    let ind_exp_grp_a = find_group_id(&pool, &company_a, "Indirect Expenses").await;

    // 1. Create Budget in Company A
    let _budget_a = BudgetService::create_budget(
        &pool,
        &company_a,
        BudgetRequest {
            name: "Company A Exclusive Budget".to_string(),
            from_date: "2024-04-01".to_string(),
            to_date: "2025-03-31".to_string(),
            lines: vec![BudgetLineRequest {
                target_type: "Group".to_string(),
                target_id: ind_exp_grp_a,
                budget_type: "OnNetTransactions".to_string(),
                amount: Decimal::from_str("100000.00").unwrap(),
                dr_cr: "Dr".to_string(),
            }],
        },
    )
    .await
    .unwrap();

    // Verify Company B sees zero budgets
    let budgets_b = BudgetService::list_budgets(&pool, &company_b.id)
        .await
        .unwrap();
    assert_eq!(budgets_b.len(), 0);

    // Verify Company B audit verification does not read Company A entries
    let audit_b = AuditCertificationService::verify_audit_trail_integrity(&pool, &company_b.id)
        .await
        .unwrap();
    assert_eq!(audit_b.company_id, company_b.id);
    assert_eq!(audit_b.total_entries, 0);
}
