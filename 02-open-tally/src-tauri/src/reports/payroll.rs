use crate::errors::OpenTallyError;
use crate::payroll::models::{PayrollStatutoryRow, PayrollStatutorySummary};
use crate::payroll::PayrollService;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;

#[derive(Debug, FromRow)]
struct RunHeaderRow {
    id: String,
    status: String,
    voucher_id: String,
}

#[derive(Debug, FromRow)]
struct PayslipJoinRow {
    payslip_id: String,
    employee_id: String,
    employee_code: String,
    employee_name: String,
    pan: Option<String>,
    uan: Option<String>,
    esic_number: Option<String>,
    total_working_days: String,
    days_present: String,
    lop_days: String,
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

pub struct PayrollReportService;

impl PayrollReportService {
    /// Returns employee-wise pay sheet and statutory PF/ESI/PT remittance summary using `ReadOnlyConnectionGuard`.
    pub async fn get_payroll_statutory_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        pay_period: &str,
    ) -> Result<PayrollStatutorySummary, OpenTallyError> {
        let clean_period = PayrollService::validate_pay_period(pay_period)?;
        let cid = company_id.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let run_opt: Option<RunHeaderRow> = sqlx::query_as(
                    "SELECT id, status, voucher_id
                     FROM payroll_runs
                     WHERE company_id = ? AND pay_period = ?
                     ORDER BY (status = 'Posted') DESC, created_at DESC
                     LIMIT 1",
                )
                .bind(&cid)
                .bind(&clean_period)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let Some(run) = run_opt else {
                    return Ok(PayrollStatutorySummary {
                        pay_period: clean_period,
                        run_id: None,
                        run_status: None,
                        voucher_id: None,
                        total_employees: 0,
                        total_gross: "0".into(),
                        total_employee_pf: "0".into(),
                        total_employer_epf: "0".into(),
                        total_employer_eps: "0".into(),
                        total_pf_remittance: "0".into(),
                        total_employee_esi: "0".into(),
                        total_employer_esi: "0".into(),
                        total_esi_remittance: "0".into(),
                        total_pt_remittance: "0".into(),
                        total_net_pay: "0".into(),
                        rows: vec![],
                    });
                };

                let joined: Vec<PayslipJoinRow> = sqlx::query_as(
                    "SELECT
                        ps.id AS payslip_id,
                        ps.employee_id,
                        e.employee_code,
                        e.name AS employee_name,
                        e.pan,
                        e.uan,
                        e.esic_number,
                        ps.total_working_days,
                        ps.days_present,
                        ps.lop_days,
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
                     JOIN employees e ON e.company_id = ps.company_id AND e.id = ps.employee_id
                     WHERE ps.company_id = ? AND ps.payroll_run_id = ?
                     ORDER BY e.employee_code ASC",
                )
                .bind(&cid)
                .bind(&run.id)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut total_gross = Decimal::ZERO;
                let mut total_ee_pf = Decimal::ZERO;
                let mut total_er_epf = Decimal::ZERO;
                let mut total_er_eps = Decimal::ZERO;
                let mut total_ee_esi = Decimal::ZERO;
                let mut total_er_esi = Decimal::ZERO;
                let mut total_pt = Decimal::ZERO;
                let mut total_net = Decimal::ZERO;

                let rows: Vec<PayrollStatutoryRow> = joined
                    .into_iter()
                    .map(|r| {
                        total_gross +=
                            Decimal::from_str(&r.gross_earnings).unwrap_or(Decimal::ZERO);
                        total_ee_pf += Decimal::from_str(&r.employee_pf).unwrap_or(Decimal::ZERO);
                        total_er_epf += Decimal::from_str(&r.employer_epf).unwrap_or(Decimal::ZERO);
                        total_er_eps += Decimal::from_str(&r.employer_eps).unwrap_or(Decimal::ZERO);
                        total_ee_esi += Decimal::from_str(&r.employee_esi).unwrap_or(Decimal::ZERO);
                        total_er_esi += Decimal::from_str(&r.employer_esi).unwrap_or(Decimal::ZERO);
                        total_pt += Decimal::from_str(&r.professional_tax).unwrap_or(Decimal::ZERO);
                        total_net += Decimal::from_str(&r.net_pay).unwrap_or(Decimal::ZERO);

                        PayrollStatutoryRow {
                            payslip_id: r.payslip_id,
                            employee_id: r.employee_id,
                            employee_code: r.employee_code,
                            employee_name: r.employee_name,
                            pan: r.pan,
                            uan: r.uan,
                            esic_number: r.esic_number,
                            total_working_days: r.total_working_days,
                            days_present: r.days_present,
                            lop_days: r.lop_days,
                            gross_earnings: r.gross_earnings,
                            employee_pf: r.employee_pf,
                            employer_epf: r.employer_epf,
                            employer_eps: r.employer_eps,
                            employee_esi: r.employee_esi,
                            employer_esi: r.employer_esi,
                            professional_tax: r.professional_tax,
                            other_deductions: r.other_deductions,
                            total_deductions: r.total_deductions,
                            net_pay: r.net_pay,
                        }
                    })
                    .collect();

                let pf_remit = total_ee_pf + total_er_epf + total_er_eps;
                let esi_remit = total_ee_esi + total_er_esi;

                Ok(PayrollStatutorySummary {
                    pay_period: clean_period,
                    run_id: Some(run.id),
                    run_status: Some(run.status),
                    voucher_id: Some(run.voucher_id),
                    total_employees: rows.len(),
                    total_gross: PayrollService::to_canonical_decimal(total_gross),
                    total_employee_pf: PayrollService::to_canonical_decimal(total_ee_pf),
                    total_employer_epf: PayrollService::to_canonical_decimal(total_er_epf),
                    total_employer_eps: PayrollService::to_canonical_decimal(total_er_eps),
                    total_pf_remittance: PayrollService::to_canonical_decimal(pf_remit),
                    total_employee_esi: PayrollService::to_canonical_decimal(total_ee_esi),
                    total_employer_esi: PayrollService::to_canonical_decimal(total_er_esi),
                    total_esi_remittance: PayrollService::to_canonical_decimal(esi_remit),
                    total_pt_remittance: PayrollService::to_canonical_decimal(total_pt),
                    total_net_pay: PayrollService::to_canonical_decimal(total_net),
                    rows,
                })
            })
        })
        .await
    }

    /// Exports payroll register CSV with CWE-1236 formula-injection sanitization.
    pub async fn export_payroll_csv(
        pool: &Pool<Sqlite>,
        company_id: &str,
        pay_period: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_payroll_statutory_summary(pool, company_id, pay_period).await?;
        let mut csv = String::from(
            "Employee Code,Employee Name,PAN,UAN,ESIC,Working Days,Days Present,LOP Days,Gross Earnings,Employee PF,Employer EPF,Employer EPS,Employee ESI,Employer ESI,Professional Tax,Other Deductions,Total Deductions,Net Pay\n",
        );

        for r in &summary.rows {
            let line = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                csv_safe(&r.employee_code),
                csv_safe(&r.employee_name),
                csv_safe(r.pan.as_deref().unwrap_or("")),
                csv_safe(r.uan.as_deref().unwrap_or("")),
                csv_safe(r.esic_number.as_deref().unwrap_or("")),
                r.total_working_days,
                r.days_present,
                r.lop_days,
                r.gross_earnings,
                r.employee_pf,
                r.employer_epf,
                r.employer_eps,
                r.employee_esi,
                r.employer_esi,
                r.professional_tax,
                r.other_deductions,
                r.total_deductions,
                r.net_pay,
            );
            csv.push_str(&line);
        }

        Ok(csv)
    }
}

fn csv_safe(val: &str) -> String {
    let trimmed = val.trim();
    let needs_prefix = trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@')
        || trimmed.starts_with('\t')
        || trimmed.starts_with('\r');
    let escaped = if needs_prefix {
        format!("'{}", trimmed)
    } else {
        trimmed.to_string()
    };
    format!("\"{}\"", escaped.replace('"', "\"\""))
}
