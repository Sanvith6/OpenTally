use crate::errors::OpenTallyError;
use crate::pdf::words::{format_indian_currency, number_to_indian_words};
use crate::pdf::writer::PdfWriter;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;

#[derive(Debug, FromRow)]
struct PayslipPdfRow {
    company_name: String,
    pay_period: String,
    run_date: String,
    employee_code: String,
    employee_name: String,
    designation: Option<String>,
    pan: Option<String>,
    uan: Option<String>,
    bank_account_no: Option<String>,
    total_working_days: String,
    days_present: String,
    lop_days: String,
    prorated_basic: String,
    prorated_hra: String,
    prorated_allowances: String,
    gross_earnings: String,
    employee_pf: String,
    employer_epf: String,
    employer_eps: String,
    employee_esi: String,
    employer_esi: String,
    professional_tax: String,
    other_deductions: String,
    total_deductions: String,
    net_pay: String,
}

/// Generates an A4 Salary Payslip PDF (`%PDF-1.4`) using a read-only connection guard.
pub async fn export_payslip_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    payslip_id: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = company_id.to_string();
    let ps_id = payslip_id.to_string();

    let row: PayslipPdfRow = execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            sqlx::query_as::<_, PayslipPdfRow>(
                "SELECT
                    c.name AS company_name,
                    pr.pay_period,
                    pr.run_date,
                    e.employee_code,
                    e.name AS employee_name,
                    e.designation,
                    e.pan,
                    e.uan,
                    e.bank_account_no,
                    ps.total_working_days,
                    ps.days_present,
                    ps.lop_days,
                    ps.prorated_basic,
                    ps.prorated_hra,
                    ps.prorated_allowances,
                    ps.gross_earnings,
                    ps.employee_pf,
                    ps.employer_epf,
                    ps.employer_eps,
                    ps.employee_esi,
                    ps.employer_esi,
                    ps.professional_tax,
                    ps.other_deductions,
                    ps.total_deductions,
                    ps.net_pay
                 FROM payroll_payslips ps
                 JOIN payroll_runs pr ON pr.company_id = ps.company_id AND pr.id = ps.payroll_run_id
                 JOIN employees e ON e.company_id = ps.company_id AND e.id = ps.employee_id
                 JOIN companies c ON c.id = ps.company_id
                 WHERE ps.company_id = ? AND ps.id = ?",
            )
            .bind(&cid)
            .bind(&ps_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
            .ok_or_else(|| {
                OpenTallyError::NotFound(format!("Payslip '{}' not found in company", ps_id))
            })
        })
    })
    .await?;

    Ok(render_payslip_pdf(&row))
}

fn parse_dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap_or(Decimal::ZERO)
}

fn render_payslip_pdf(row: &PayslipPdfRow) -> Vec<u8> {
    let mut pdf = PdfWriter::new();
    let left = 40.0;
    let right = PdfWriter::PAGE_WIDTH - 40.0;
    let width = right - left;

    // Header Banner
    pdf.filled_rect(left, 780.0, width, 34.0, 0.92, 0.75);
    pdf.text_center(&row.company_name, left + (width / 2.0), 798.0, "/F2", 13.0);
    pdf.text_center(
        &format!("SALARY PAYSLIP - PAY PERIOD: {}", row.pay_period),
        left + (width / 2.0),
        785.0,
        "/F2",
        9.5,
    );

    // Employee & Attendance Info Box
    let info_y = 685.0;
    let info_h = 88.0;
    pdf.rect(left, info_y, width, info_h, 0.75);
    let mid_x = left + (width / 2.0);
    pdf.line(mid_x, info_y, mid_x, info_y + info_h, 0.75);

    pdf.text(
        &format!("Employee Name: {}", row.employee_name),
        left + 8.0,
        info_y + 72.0,
        "/F2",
        9.0,
    );
    pdf.text(
        &format!("Employee Code: {}", row.employee_code),
        left + 8.0,
        info_y + 57.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!("Designation: {}", row.designation.as_deref().unwrap_or("-")),
        left + 8.0,
        info_y + 42.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!(
            "PAN: {} | UAN: {}",
            row.pan.as_deref().unwrap_or("-"),
            row.uan.as_deref().unwrap_or("-")
        ),
        left + 8.0,
        info_y + 27.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!(
            "Bank A/c: {}",
            row.bank_account_no.as_deref().unwrap_or("-")
        ),
        left + 8.0,
        info_y + 12.0,
        "/F1",
        8.5,
    );

    pdf.text(
        &format!("Pay Run Date: {}", row.run_date),
        mid_x + 8.0,
        info_y + 72.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!("Working Days: {}", row.total_working_days),
        mid_x + 8.0,
        info_y + 57.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!("Days Present: {}", row.days_present),
        mid_x + 8.0,
        info_y + 42.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!("LOP Days: {}", row.lop_days),
        mid_x + 8.0,
        info_y + 27.0,
        "/F1",
        8.5,
    );

    // Two-column Earnings vs Deductions Table
    let tbl_top = 670.0;
    let tbl_h = 150.0;
    let tbl_bottom = tbl_top - tbl_h;
    pdf.rect(left, tbl_bottom, width, tbl_h, 0.75);
    pdf.filled_rect(left, tbl_top - 20.0, width, 20.0, 0.92, 0.75);
    pdf.line(mid_x, tbl_bottom, mid_x, tbl_top, 0.75);

    pdf.text("EARNINGS", left + 8.0, tbl_top - 14.0, "/F2", 9.0);
    pdf.text_right("AMOUNT (INR)", mid_x - 8.0, tbl_top - 14.0, "/F2", 9.0);
    pdf.text("DEDUCTIONS", mid_x + 8.0, tbl_top - 14.0, "/F2", 9.0);
    pdf.text_right("AMOUNT (INR)", right - 8.0, tbl_top - 14.0, "/F2", 9.0);

    // Earnings rows
    let mut ey = tbl_top - 38.0;
    let earnings = [
        ("Basic Salary (Prorated)", &row.prorated_basic),
        ("House Rent Allowance (HRA)", &row.prorated_hra),
        ("Other Allowances", &row.prorated_allowances),
    ];
    for (label, val) in earnings {
        pdf.text(label, left + 8.0, ey, "/F1", 8.5);
        pdf.text_right(
            &format_indian_currency(parse_dec(val)),
            mid_x - 8.0,
            ey,
            "/F1",
            8.5,
        );
        ey -= 18.0;
    }

    // Deductions rows
    let mut dy = tbl_top - 38.0;
    let deductions = [
        ("Employee Provident Fund (EPF)", &row.employee_pf),
        ("Employee State Insurance (ESI)", &row.employee_esi),
        ("Professional Tax (PT)", &row.professional_tax),
        ("Other Deductions", &row.other_deductions),
    ];
    for (label, val) in deductions {
        pdf.text(label, mid_x + 8.0, dy, "/F1", 8.5);
        pdf.text_right(
            &format_indian_currency(parse_dec(val)),
            right - 8.0,
            dy,
            "/F1",
            8.5,
        );
        dy -= 18.0;
    }

    // Totals row
    let tot_y = tbl_bottom + 8.0;
    pdf.line(left, tbl_bottom + 24.0, right, tbl_bottom + 24.0, 0.75);
    pdf.text("Gross Earnings", left + 8.0, tot_y, "/F2", 9.0);
    pdf.text_right(
        &format_indian_currency(parse_dec(&row.gross_earnings)),
        mid_x - 8.0,
        tot_y,
        "/F2",
        9.0,
    );
    pdf.text("Total Deductions", mid_x + 8.0, tot_y, "/F2", 9.0);
    pdf.text_right(
        &format_indian_currency(parse_dec(&row.total_deductions)),
        right - 8.0,
        tot_y,
        "/F2",
        9.0,
    );

    // Net Pay Highlight Box
    let net_dec = parse_dec(&row.net_pay);
    let net_box_y = tbl_bottom - 52.0;
    pdf.filled_rect(left, net_box_y, width, 42.0, 0.95, 0.75);
    pdf.text(
        &format!("NET PAYABLE: {}", format_indian_currency(net_dec)),
        left + 10.0,
        net_box_y + 25.0,
        "/F2",
        11.0,
    );
    pdf.text(
        &format!("Amount in Words: {}", number_to_indian_words(net_dec)),
        left + 10.0,
        net_box_y + 9.0,
        "/F1",
        8.5,
    );

    // Employer Statutory Contributions Footer
    let foot_y = net_box_y - 38.0;
    pdf.rect(left, foot_y, width, 28.0, 0.5);
    pdf.text(
        &format!(
            "Employer Contributions (Non-Deductible): EPF: {} | EPS: {} | ESI: {}",
            format_indian_currency(parse_dec(&row.employer_epf)),
            format_indian_currency(parse_dec(&row.employer_eps)),
            format_indian_currency(parse_dec(&row.employer_esi)),
        ),
        left + 8.0,
        foot_y + 10.0,
        "/F1",
        8.0,
    );

    pdf.text(
        "Generated by OpenTally Payroll Engine (System Generated Payslip)",
        left,
        foot_y - 20.0,
        "/F1",
        7.5,
    );

    pdf.build()
}
