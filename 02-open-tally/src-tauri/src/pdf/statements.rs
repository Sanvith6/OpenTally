use crate::errors::OpenTallyError;
use crate::pdf::words::format_indian_currency;
use crate::pdf::writer::PdfWriter;
use crate::reports::model::{
    BalanceSheetRequest, LedgerReportRequest, ProfitAndLossRequest, ReportDateRange, ReportFilter,
    TrialBalanceRequest,
};
use crate::reports::ReportPageRequest;
use crate::reports::{
    BalanceSheetService, LedgerReportService, ProfitAndLossService, TrialBalanceService,
};
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s.trim()).unwrap_or(Decimal::ZERO)
}

async fn fetch_company_name(
    pool: &Pool<Sqlite>,
    company_id: &str,
) -> Result<String, OpenTallyError> {
    let cid = company_id.to_string();
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            sqlx::query_scalar::<_, String>("SELECT name FROM companies WHERE id = ?")
                .bind(&cid)
                .fetch_one(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
        })
    })
    .await
}

fn render_header(pdf: &mut PdfWriter, company_name: &str, title: &str, subtitle: &str) -> f64 {
    let left = 40.0;
    let width = PdfWriter::PAGE_WIDTH - 80.0;
    pdf.filled_rect(left, 775.0, width, 40.0, 0.92, 0.75);
    pdf.text_center(company_name, left + width / 2.0, 798.0, "/F2", 13.0);
    pdf.text_center(title, left + width / 2.0, 783.0, "/F2", 10.0);
    pdf.text(subtitle, left, 760.0, "/F1", 8.5);
    742.0
}

/// Generates an A4 Trial Balance PDF (`%PDF-1.4`) under ReadOnlyConnectionGuard.
pub async fn export_trial_balance_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    as_of_date: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let company_name = fetch_company_name(pool, company_id).await?;
    let report = TrialBalanceService::generate(
        pool,
        company_id,
        TrialBalanceRequest {
            as_of_date: as_of_date.to_string(),
            account_group_id: None,
            ledger_id: None,
            account_search: None,
            include_zero_balances: false,
        },
    )
    .await?;

    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let mut y = render_header(
        &mut pdf,
        &company_name,
        "TRIAL BALANCE",
        &format!("As of: {}", as_of_date),
    );

    pdf.filled_rect(left, y - 4.0, right - left, 18.0, 0.90, 0.6);
    pdf.text(
        "Particulars (Ledger / Group)",
        left + 6.0,
        y + 1.0,
        "/F2",
        8.5,
    );
    pdf.text_right("Debit (INR)", right - 110.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Credit (INR)", right - 6.0, y + 1.0, "/F2", 8.5);
    y -= 20.0;

    for row in report.rows.iter().take(34) {
        pdf.text(&row.ledger_name, left + 6.0, y, "/F1", 8.0);
        pdf.text_right(
            &format_indian_currency(dec(&row.closing_debit)),
            right - 110.0,
            y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(dec(&row.closing_credit)),
            right - 6.0,
            y,
            "/F1",
            8.0,
        );
        y -= 14.0;
    }

    pdf.line(left, y + 6.0, right, y + 6.0, 0.75);
    pdf.text("Grand Total", left + 6.0, y - 6.0, "/F2", 9.0);
    pdf.text_right(
        &format_indian_currency(dec(&report.total_debit)),
        right - 110.0,
        y - 6.0,
        "/F2",
        9.0,
    );
    pdf.text_right(
        &format_indian_currency(dec(&report.total_credit)),
        right - 6.0,
        y - 6.0,
        "/F2",
        9.0,
    );
    pdf.line(left, y - 10.0, right, y - 10.0, 0.75);
    pdf.line(left, y - 12.5, right, y - 12.5, 0.75);
    pdf.text(
        &format!("Difference: {}", report.difference),
        left + 6.0,
        y - 26.0,
        "/F1",
        8.0,
    );

    Ok(pdf.build())
}

/// Generates an A4 Profit & Loss Statement PDF (`%PDF-1.4`) in classic T-Format.
pub async fn export_profit_and_loss_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let company_name = fetch_company_name(pool, company_id).await?;
    let report = ProfitAndLossService::generate(
        pool,
        company_id,
        ProfitAndLossRequest {
            period: ReportDateRange {
                date_from: from_date.to_string(),
                date_to: to_date.to_string(),
            },
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await?;

    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let mid = (left + right) / 2.0;
    let mut y = render_header(
        &mut pdf,
        &company_name,
        "PROFIT & LOSS ACCOUNT",
        &format!("Period: {} to {}", from_date, to_date),
    );

    pdf.filled_rect(left, y - 4.0, right - left, 18.0, 0.90, 0.6);
    pdf.text("Expenses (Dr)", left + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Amount", mid - 8.0, y + 1.0, "/F2", 8.5);
    pdf.text("Incomes (Cr)", mid + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Amount", right - 6.0, y + 1.0, "/F2", 8.5);
    y -= 20.0;

    let max_rows = report
        .direct_expense_rows
        .len()
        .max(report.direct_income_rows.len())
        .min(30);
    for i in 0..max_rows {
        if let Some(exp) = report.direct_expense_rows.get(i) {
            pdf.text(&exp.ledger_name, left + 6.0, y, "/F1", 8.0);
            pdf.text_right(
                &format_indian_currency(dec(&exp.contribution)),
                mid - 8.0,
                y,
                "/F1",
                8.0,
            );
        }
        if let Some(inc) = report.direct_income_rows.get(i) {
            pdf.text(&inc.ledger_name, mid + 6.0, y, "/F1", 8.0);
            pdf.text_right(
                &format_indian_currency(dec(&inc.contribution)),
                right - 6.0,
                y,
                "/F1",
                8.0,
            );
        }
        y -= 14.0;
    }

    pdf.line(left, y + 4.0, right, y + 4.0, 0.75);
    pdf.text("Direct Expenses", left + 6.0, y - 8.0, "/F2", 8.5);
    pdf.text_right(
        &format_indian_currency(dec(&report.direct_expense_subtotal)),
        mid - 8.0,
        y - 8.0,
        "/F2",
        8.5,
    );
    pdf.text("Direct Incomes", mid + 6.0, y - 8.0, "/F2", 8.5);
    pdf.text_right(
        &format_indian_currency(dec(&report.direct_income_subtotal)),
        right - 6.0,
        y - 8.0,
        "/F2",
        8.5,
    );
    pdf.text(
        &format!("Net Profit / (Loss): {}", report.net_profit_or_loss),
        left + 6.0,
        y - 26.0,
        "/F2",
        9.0,
    );

    Ok(pdf.build())
}

/// Generates an A4 Balance Sheet PDF (`%PDF-1.4`) in classic T-Format.
pub async fn export_balance_sheet_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    as_of_date: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let company_name = fetch_company_name(pool, company_id).await?;
    let report = BalanceSheetService::generate(
        pool,
        company_id,
        BalanceSheetRequest {
            as_of_date: as_of_date.to_string(),
            account_group_id: None,
            ledger_search: None,
            include_zero_balances: false,
        },
    )
    .await?;

    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let mid = (left + right) / 2.0;
    let mut y = render_header(
        &mut pdf,
        &company_name,
        "BALANCE SHEET",
        &format!("As of: {}", as_of_date),
    );

    pdf.filled_rect(left, y - 4.0, right - left, 18.0, 0.90, 0.6);
    pdf.text("Capital & Liabilities", left + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Amount", mid - 8.0, y + 1.0, "/F2", 8.5);
    pdf.text("Assets", mid + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Amount", right - 6.0, y + 1.0, "/F2", 8.5);
    y -= 20.0;

    let max_rows = report
        .liability_rows
        .len()
        .max(report.asset_rows.len())
        .min(30);
    for i in 0..max_rows {
        if let Some(liab) = report.liability_rows.get(i) {
            pdf.text(&liab.ledger_name, left + 6.0, y, "/F1", 8.0);
            pdf.text_right(
                &format_indian_currency(dec(&liab.presentation_amount)),
                mid - 8.0,
                y,
                "/F1",
                8.0,
            );
        }
        if let Some(asset) = report.asset_rows.get(i) {
            pdf.text(&asset.ledger_name, mid + 6.0, y, "/F1", 8.0);
            pdf.text_right(
                &format_indian_currency(dec(&asset.presentation_amount)),
                right - 6.0,
                y,
                "/F1",
                8.0,
            );
        }
        y -= 14.0;
    }

    pdf.line(left, y + 4.0, right, y + 4.0, 0.75);
    pdf.text("Total Liabilities", left + 6.0, y - 8.0, "/F2", 8.5);
    pdf.text_right(
        &format_indian_currency(dec(&report.total_liabilities_and_equity)),
        mid - 8.0,
        y - 8.0,
        "/F2",
        8.5,
    );
    pdf.text("Total Assets", mid + 6.0, y - 8.0, "/F2", 8.5);
    pdf.text_right(
        &format_indian_currency(dec(&report.total_assets)),
        right - 6.0,
        y - 8.0,
        "/F2",
        8.5,
    );

    Ok(pdf.build())
}

/// Generates an A4 Ledger Statement PDF (`%PDF-1.4`).
pub async fn export_ledger_statement_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    ledger_id: &str,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let company_name = fetch_company_name(pool, company_id).await?;
    let report = LedgerReportService::generate(
        pool,
        company_id,
        LedgerReportRequest {
            filter: ReportFilter {
                period: ReportDateRange {
                    date_from: from_date.to_string(),
                    date_to: to_date.to_string(),
                },
                ledger_id: Some(ledger_id.to_string()),
            },
            page: ReportPageRequest {
                page: 1,
                page_size: 100,
            },
        },
    )
    .await?;

    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let mut y = render_header(
        &mut pdf,
        &company_name,
        &format!("LEDGER STATEMENT: {}", report.ledger_name),
        &format!("Period: {} to {}", from_date, to_date),
    );

    pdf.filled_rect(left, y - 4.0, right - left, 18.0, 0.90, 0.6);
    pdf.text("Date", left + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text("Vch Type / No", left + 75.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Debit", right - 150.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Credit", right - 75.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Balance", right - 6.0, y + 1.0, "/F2", 8.5);
    y -= 20.0;

    for row in report.transactions.items.iter().take(32) {
        pdf.text(&row.date, left + 6.0, y, "/F1", 8.0);
        pdf.text(
            &format!("{} {}", row.voucher_type, row.voucher_number),
            left + 75.0,
            y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(dec(&row.debit)),
            right - 150.0,
            y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(dec(&row.credit)),
            right - 75.0,
            y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format!("{} {}", row.running_balance, row.running_balance_type),
            right - 6.0,
            y,
            "/F1",
            8.0,
        );
        y -= 14.0;
    }

    pdf.line(left, y + 4.0, right, y + 4.0, 0.75);
    pdf.text(
        &format!(
            "Closing Balance: {} {}",
            report.closing_balance, report.closing_balance_type
        ),
        left + 6.0,
        y - 10.0,
        "/F2",
        9.0,
    );

    Ok(pdf.build())
}

/// Generates an A4 Outstanding Aging PDF (`%PDF-1.4`) under ReadOnlyConnectionGuard.
pub async fn export_outstanding_aging_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    report_type: &str,
    as_of_date: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let company_name = fetch_company_name(pool, company_id).await?;
    let cid = company_id.to_string();
    let rtype = report_type.to_string();

    #[derive(Debug, sqlx::FromRow)]
    struct BillRow {
        bill_name: String,
        due_date: Option<String>,
        amount: String,
    }

    let rows: Vec<BillRow> = execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            sqlx::query_as(
                "SELECT bill_name, due_date, amount
                 FROM bill_allocations
                 WHERE company_id = ? AND allocation_type = 'NewRef'
                 ORDER BY due_date ASC
                 LIMIT 32",
            )
            .bind(&cid)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)
        })
    })
    .await?;

    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let mut y = render_header(
        &mut pdf,
        &company_name,
        &format!("OUTSTANDING AGING ({})", rtype.to_uppercase()),
        &format!("As of: {}", as_of_date),
    );

    pdf.filled_rect(left, y - 4.0, right - left, 18.0, 0.90, 0.6);
    pdf.text("Bill Reference", left + 6.0, y + 1.0, "/F2", 8.5);
    pdf.text("Due Date", left + 210.0, y + 1.0, "/F2", 8.5);
    pdf.text_right("Amount (INR)", right - 6.0, y + 1.0, "/F2", 8.5);
    y -= 20.0;

    for r in rows {
        pdf.text(&r.bill_name, left + 6.0, y, "/F1", 8.0);
        pdf.text(
            r.due_date.as_deref().unwrap_or("-"),
            left + 210.0,
            y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(dec(&r.amount)),
            right - 6.0,
            y,
            "/F1",
            8.0,
        );
        y -= 14.0;
    }

    Ok(pdf.build())
}
