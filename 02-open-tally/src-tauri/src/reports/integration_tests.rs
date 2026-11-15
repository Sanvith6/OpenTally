use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::reports::{
    BalanceSheetRequest, BalanceSheetService, CashFlowRequest, CashFlowService, DayBookRequest,
    DayBookService, LedgerReportRequest, LedgerReportService, ProfitAndLossRequest,
    ProfitAndLossService, ReportDateRange, ReportFilter, ReportPageRequest, TrialBalanceRequest,
    TrialBalanceService,
};
use crate::vouchers::{
    CancelVoucherRequest, CreateVoucherDraftRequest, CreateVoucherSeriesRequest,
    ReverseVoucherRequest, VoucherCancellationService, VoucherDraftService, VoucherLineInput,
    VoucherPostingService, VoucherReversalService, VoucherSeriesService,
};
use std::path::Path;

struct Fixture {
    db: DatabaseManager,
    company_id: String,
    cash_id: String,
    sales_id: String,
    expense_id: String,
    series_id: String,
}

impl Fixture {
    async fn new(path: &Path) -> Self {
        let db = DatabaseManager::init(path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_migration(db.pool())
            .await
            .unwrap();
        let company = CompanyService::create_company(
            db.pool(),
            CreateCompanyRequest {
                name: "Reporting Fixture Co".into(),
                mailing_name: None,
                country: None,
                state: None,
                financial_year_begins: "2026-04-01".into(),
                books_beginning_from: None,
                currency_symbol: None,
                currency_formal_name: None,
                currency_decimal_places: Some(2),
            },
        )
        .await
        .unwrap();
        let assets = find_group(&db, &company.id, "Cash-in-Hand").await;
        let capital = find_group(&db, &company.id, "Capital Account").await;
        let income = find_group(&db, &company.id, "Sales Accounts").await;
        let expenses = find_group(&db, &company.id, "Purchase Accounts").await;
        for (id, group, name) in [
            ("fixture_cash", assets.as_str(), "Fixture Cash"),
            ("fixture_capital", capital.as_str(), "Fixture Capital"),
            ("fixture_sales", income.as_str(), "Fixture Sales"),
            ("fixture_expense", expenses.as_str(), "Fixture Expense"),
        ] {
            sqlx::query("INSERT INTO ledgers (id, company_id, group_id, name, description, is_reserved, ledger_classification) VALUES (?, ?, ?, ?, NULL, 0, 'standard')")
                .bind(id).bind(&company.id).bind(group).bind(name).execute(db.pool()).await.unwrap();
        }
        sqlx::query("INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount) VALUES ('fixture_open_cash', ?, 'fixture_cash', '2026-04-01', 'Dr', '100.00'), ('fixture_open_capital', ?, 'fixture_capital', '2026-04-01', 'Cr', '100.00')")
            .bind(&company.id).bind(&company.id).execute(db.pool()).await.unwrap();
        let series = VoucherSeriesService::create_series(
            db.pool(),
            &company.id,
            CreateVoucherSeriesRequest {
                voucher_type_id: "vt_jnl".into(),
                series_name: "Fixture Journal".into(),
                series_code: "FIX".into(),
                numbering_method: "automatic".into(),
                restart_period: "yearly".into(),
                applicable_from: Some("2026-04-01".into()),
                prefix: Some("FIX/{YY-YY}/".into()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();
        VoucherSeriesService::create_series(
            db.pool(),
            &company.id,
            CreateVoucherSeriesRequest {
                voucher_type_id: "vt_reversing_journal".into(),
                series_name: "Fixture Reversal".into(),
                series_code: "FREV".into(),
                numbering_method: "automatic".into(),
                restart_period: "yearly".into(),
                applicable_from: Some("2026-04-01".into()),
                prefix: Some("FREV/{YY-YY}/".into()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();
        let fixture = Self {
            db,
            company_id: company.id,
            cash_id: "fixture_cash".into(),
            sales_id: "fixture_sales".into(),
            expense_id: "fixture_expense".into(),
            series_id: series.id,
        };
        fixture
            .post(
                "2026-04-05",
                &fixture.cash_id,
                "Dr",
                &fixture.sales_id,
                "Cr",
                "50.00",
            )
            .await;
        fixture
            .post(
                "2026-04-10",
                &fixture.expense_id,
                "Dr",
                &fixture.cash_id,
                "Cr",
                "20.00",
            )
            .await;
        let cancelled = fixture
            .post(
                "2026-04-14",
                &fixture.cash_id,
                "Dr",
                &fixture.sales_id,
                "Cr",
                "30.00",
            )
            .await;
        VoucherCancellationService::cancel_voucher(
            fixture.db.pool(),
            &fixture.company_id,
            CancelVoucherRequest {
                voucher_id: cancelled,
                reason: "Fixture cancellation".into(),
                actor_id: Some("test".into()),
            },
        )
        .await
        .unwrap();
        let reversed = fixture
            .post(
                "2026-04-15",
                &fixture.cash_id,
                "Dr",
                &fixture.sales_id,
                "Cr",
                "40.00",
            )
            .await;
        VoucherReversalService::reverse_voucher(
            fixture.db.pool(),
            &fixture.company_id,
            ReverseVoucherRequest {
                original_voucher_id: reversed,
                reversal_date: "2026-04-16".into(),
                reason: "Fixture correction".into(),
                actor_id: Some("test".into()),
            },
        )
        .await
        .unwrap();
        let _ = fixture
            .draft("2026-04-12", &fixture.cash_id, &fixture.sales_id, "500.00")
            .await;
        fixture
    }

    async fn draft(&self, date: &str, debit: &str, credit: &str, amount: &str) -> String {
        VoucherDraftService::create_draft(
            self.db.pool(),
            &self.company_id,
            CreateVoucherDraftRequest {
                voucher_type_id: "vt_jnl".into(),
                series_id: self.series_id.clone(),
                date: date.into(),
                effective_date: None,
                reference_no: None,
                reference_date: None,
                narration: Some("Fixture voucher".into()),
                lines: Some(vec![
                    VoucherLineInput {
                        id: None,
                        ledger_id: debit.into(),
                        entry_type: "Dr".into(),
                        amount: amount.into(),
                        narration: None,
                    },
                    VoucherLineInput {
                        id: None,
                        ledger_id: credit.into(),
                        entry_type: "Cr".into(),
                        amount: amount.into(),
                        narration: None,
                    },
                ]),
            },
        )
        .await
        .unwrap()
        .voucher
        .id
    }

    async fn post(
        &self,
        date: &str,
        debit: &str,
        debit_type: &str,
        credit: &str,
        credit_type: &str,
        amount: &str,
    ) -> String {
        let draft = self.draft(date, debit, credit, amount).await;
        assert_eq!(debit_type, "Dr");
        assert_eq!(credit_type, "Cr");
        VoucherPostingService::post_voucher(self.db.pool(), &self.company_id, &draft, Some("test"))
            .await
            .unwrap()
            .voucher
            .id
    }
}

async fn find_group(db: &DatabaseManager, company_id: &str, name: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = ?",
    )
    .bind(company_id)
    .bind(name)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn reports_agree_on_realistic_lifecycle_fixture() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(&temp.path().join("reports.db")).await;
    let period = ReportDateRange::new("2026-04-01", "2026-04-20").unwrap();
    let trial = TrialBalanceService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        TrialBalanceRequest {
            as_of_date: "2026-04-20".into(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    let cash_trial = trial
        .rows
        .iter()
        .find(|row| row.ledger_id == fixture.cash_id)
        .unwrap();
    assert_eq!(cash_trial.closing_debit, "130.00");
    let ledger = LedgerReportService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        LedgerReportRequest {
            filter: ReportFilter {
                period: period.clone(),
                ledger_id: Some(fixture.cash_id.clone()),
            },
            page: ReportPageRequest {
                page: 1,
                page_size: 25,
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(ledger.closing_balance, "130.00");
    let pnl = ProfitAndLossService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        ProfitAndLossRequest {
            period: period.clone(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(pnl.net_profit_or_loss, "30.00");
    assert_eq!(pnl.net_result_type, "Profit");
    let balance = BalanceSheetService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        BalanceSheetRequest {
            as_of_date: "2026-04-20".into(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(balance.total_assets, "130.00");
    assert_eq!(balance.total_equity, "130.00");
    assert!(balance.is_balanced);
    assert_eq!(balance.balance_difference, "0.00");
    let cash_flow = CashFlowService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        CashFlowRequest {
            period: period.clone(),
            cash_ledger_id: None,
            activity_filter: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(cash_flow.opening_cash_balance, "100.00");
    assert_eq!(cash_flow.net_change_in_cash, "30.00");
    assert_eq!(cash_flow.closing_cash_balance, "130.00");
    assert!(cash_flow.is_reconciled);
    let day_book = DayBookService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        DayBookRequest {
            filter: ReportFilter {
                period,
                ledger_id: None,
            },
            voucher_type_id: None,
            voucher_number_query: None,
            page: ReportPageRequest {
                page: 1,
                page_size: 25,
            },
        },
    )
    .await
    .unwrap();
    assert_eq!(day_book.total, 4);
}

#[tokio::test]
async fn reports_enforce_company_isolation_and_as_of_boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(&temp.path().join("reports_isolation.db")).await;
    let other = CompanyService::create_company(
        fixture.db.pool(),
        CreateCompanyRequest {
            name: "Other Reporting Co".into(),
            mailing_name: None,
            country: None,
            state: None,
            financial_year_begins: "2026-04-01".into(),
            books_beginning_from: None,
            currency_symbol: None,
            currency_formal_name: None,
            currency_decimal_places: Some(2),
        },
    )
    .await
    .unwrap();
    let other_report = BalanceSheetService::generate(
        fixture.db.pool(),
        &other.id,
        BalanceSheetRequest {
            as_of_date: "2026-04-20".into(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(other_report.ledger_count, 0);
    let before_expense = ProfitAndLossService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        ProfitAndLossRequest {
            period: ReportDateRange::new("2026-04-01", "2026-04-05").unwrap(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(before_expense.net_profit_or_loss, "50.00");
    assert!(BalanceSheetService::generate(
        fixture.db.pool(),
        &fixture.company_id,
        BalanceSheetRequest {
            as_of_date: "2026-03-31".into(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false
        }
    )
    .await
    .is_err());
}
