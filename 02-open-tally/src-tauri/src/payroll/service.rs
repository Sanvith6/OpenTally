use crate::errors::OpenTallyError;
use crate::payroll::models::*;
use crate::vouchers::model::{ValidationMode, VoucherLine};
use crate::vouchers::service::{
    execute_read_only_query, DoubleEntryValidator, VoucherSequenceService,
};
use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};
use std::collections::HashSet;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct EmployeeRow {
    id: String,
    company_id: String,
    employee_code: String,
    name: String,
    department_group_id: Option<String>,
    designation: Option<String>,
    date_of_joining: String,
    date_of_exit: Option<String>,
    pan: Option<String>,
    uan: Option<String>,
    esic_number: Option<String>,
    bank_account_no: Option<String>,
    bank_ifsc: Option<String>,
    basic_salary: String,
    hra: String,
    other_allowances: String,
    pf_applicable: i64,
    esi_applicable: i64,
    pt_applicable: i64,
    is_active: i64,
    created_at: String,
}

impl From<EmployeeRow> for Employee {
    fn from(r: EmployeeRow) -> Self {
        Self {
            id: r.id,
            company_id: r.company_id,
            employee_code: r.employee_code,
            name: r.name,
            department_group_id: r.department_group_id,
            designation: r.designation,
            date_of_joining: r.date_of_joining,
            date_of_exit: r.date_of_exit,
            pan: r.pan,
            uan: r.uan,
            esic_number: r.esic_number,
            bank_account_no: r.bank_account_no,
            bank_ifsc: r.bank_ifsc,
            basic_salary: r.basic_salary,
            hra: r.hra,
            other_allowances: r.other_allowances,
            pf_applicable: r.pf_applicable == 1,
            esi_applicable: r.esi_applicable == 1,
            pt_applicable: r.pt_applicable == 1,
            is_active: r.is_active == 1,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct PayrollLedgerMappingRow {
    salary_expense_ledger_id: String,
    employer_pf_expense_ledger_id: String,
    employer_esi_expense_ledger_id: String,
    salary_payable_ledger_id: String,
    pf_payable_ledger_id: String,
    esi_payable_ledger_id: String,
    pt_payable_ledger_id: String,
}

pub struct PayrollService;

impl PayrollService {
    /// Converts a Decimal to canonical string representation rounded to 2 decimal places.
    pub fn to_canonical_decimal(d: Decimal) -> String {
        let norm = d.round_dp(2).normalize();
        if norm.is_zero() {
            "0".to_string()
        } else {
            norm.to_string()
        }
    }

    fn parse_non_negative_decimal(raw: &str, field_name: &str) -> Result<Decimal, OpenTallyError> {
        let d = Decimal::from_str(raw.trim()).map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid decimal value '{}' for {}",
                raw, field_name
            ))
        })?;
        if d < Decimal::ZERO {
            return Err(OpenTallyError::Validation(format!(
                "{} cannot be negative",
                field_name
            )));
        }
        Ok(d)
    }

    pub fn validate_uan(uan: &str) -> Result<String, OpenTallyError> {
        let clean = uan.trim().to_string();
        if clean.len() != 12 || !clean.chars().all(|c| c.is_ascii_digit()) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid EPF UAN '{}': must be exactly 12 digits",
                uan
            )));
        }
        Ok(clean)
    }

    pub fn validate_pay_period(pay_period: &str) -> Result<String, OpenTallyError> {
        let clean = pay_period.trim();
        let parts: Vec<&str> = clean.split('-').collect();
        if parts.len() != 2 || parts[0].len() != 4 || parts[1].len() != 2 {
            return Err(OpenTallyError::Validation(format!(
                "Invalid pay_period '{}': expected YYYY-MM format",
                pay_period
            )));
        }
        let year: i32 = parts[0].parse().map_err(|_| {
            OpenTallyError::Validation(format!("Invalid year in pay_period '{}'", pay_period))
        })?;
        let month: u32 = parts[1].parse().map_err(|_| {
            OpenTallyError::Validation(format!("Invalid month in pay_period '{}'", pay_period))
        })?;
        if !(1900..=2100).contains(&year) || !(1..=12).contains(&month) {
            return Err(OpenTallyError::Validation(format!(
                "Invalid pay_period '{}': month must be 01..12",
                pay_period
            )));
        }
        Ok(clean.to_string())
    }

    /// Computes a single employee's statutory payslip using deterministic `rust_decimal` math.
    pub fn compute_employee_payslip(
        emp: &Employee,
        attendance: &PayrollAttendanceInput,
    ) -> Result<ComputedPayslipPreview, OpenTallyError> {
        let total_working_days =
            Self::parse_non_negative_decimal(&attendance.total_working_days, "total_working_days")?;
        if total_working_days <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "total_working_days must be strictly greater than zero".into(),
            ));
        }
        let days_present =
            Self::parse_non_negative_decimal(&attendance.days_present, "days_present")?;
        if days_present > total_working_days {
            return Err(OpenTallyError::Validation(format!(
                "days_present ({}) cannot exceed total_working_days ({}) for employee '{}'",
                days_present, total_working_days, emp.name
            )));
        }
        let lop_days = total_working_days - days_present;

        let basic = Self::parse_non_negative_decimal(&emp.basic_salary, "basic_salary")?;
        let hra = Self::parse_non_negative_decimal(&emp.hra, "hra")?;
        let allowances =
            Self::parse_non_negative_decimal(&emp.other_allowances, "other_allowances")?;
        let other_deductions =
            Self::parse_non_negative_decimal(&attendance.other_deductions, "other_deductions")?;

        let ratio = days_present / total_working_days;
        let prorated_basic =
            (basic * ratio).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let prorated_hra =
            (hra * ratio).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let prorated_allowances =
            (allowances * ratio).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
        let gross_earnings = prorated_basic + prorated_hra + prorated_allowances;

        // 1. EPF / EPS (Statutory Rs. 15,000 basic wage ceiling)
        let pf_ceiling = Decimal::new(1500000, 2); // 15000.00
        let (employee_pf, employer_epf, employer_eps) = if emp.pf_applicable {
            let pf_wage_base = prorated_basic.min(pf_ceiling);
            let ee_pf = (pf_wage_base * Decimal::new(12, 2))
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
            let er_eps = (pf_wage_base * Decimal::new(833, 4))
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .min(Decimal::new(1250, 0));
            let er_epf = ee_pf - er_eps;
            (ee_pf, er_epf, er_eps)
        } else {
            (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO)
        };

        // 2. ESI (Statutory Rs. 21,000 gross ceiling; ceil to whole rupee)
        let esi_ceiling = Decimal::new(2100000, 2); // 21000.00
        let (employee_esi, employer_esi) = if emp.esi_applicable
            && gross_earnings <= esi_ceiling
            && gross_earnings > Decimal::ZERO
        {
            let ee_esi = (gross_earnings * Decimal::new(75, 4))
                .round_dp_with_strategy(0, RoundingStrategy::ToPositiveInfinity);
            let er_esi = (gross_earnings * Decimal::new(325, 4))
                .round_dp_with_strategy(0, RoundingStrategy::ToPositiveInfinity);
            (ee_esi, er_esi)
        } else {
            (Decimal::ZERO, Decimal::ZERO)
        };

        // 3. Professional Tax (Karnataka default slab: >= Rs. 25,000 -> Rs. 200)
        let pt_threshold = Decimal::new(2500000, 2); // 25000.00
        let professional_tax = if emp.pt_applicable && gross_earnings >= pt_threshold {
            Decimal::new(200, 0)
        } else {
            Decimal::ZERO
        };

        let total_deductions = employee_pf + employee_esi + professional_tax + other_deductions;
        let net_pay = gross_earnings - total_deductions;
        if net_pay < Decimal::ZERO {
            return Err(OpenTallyError::Validation(format!(
                "Net pay cannot be negative for employee '{}' (Gross: {}, Deductions: {})",
                emp.name, gross_earnings, total_deductions
            )));
        }

        Ok(ComputedPayslipPreview {
            employee_id: emp.id.clone(),
            employee_code: emp.employee_code.clone(),
            employee_name: emp.name.clone(),
            designation: emp.designation.clone(),
            total_working_days: Self::to_canonical_decimal(total_working_days),
            days_present: Self::to_canonical_decimal(days_present),
            lop_days: Self::to_canonical_decimal(lop_days),
            prorated_basic: Self::to_canonical_decimal(prorated_basic),
            prorated_hra: Self::to_canonical_decimal(prorated_hra),
            prorated_allowances: Self::to_canonical_decimal(prorated_allowances),
            gross_earnings: Self::to_canonical_decimal(gross_earnings),
            employee_pf: Self::to_canonical_decimal(employee_pf),
            employer_epf: Self::to_canonical_decimal(employer_epf),
            employer_eps: Self::to_canonical_decimal(employer_eps),
            employee_esi: Self::to_canonical_decimal(employee_esi),
            employer_esi: Self::to_canonical_decimal(employer_esi),
            professional_tax: Self::to_canonical_decimal(professional_tax),
            other_deductions: Self::to_canonical_decimal(other_deductions),
            total_deductions: Self::to_canonical_decimal(total_deductions),
            net_pay: Self::to_canonical_decimal(net_pay),
        })
    }

    /// Lists all employees for a company (read-only).
    pub async fn list_employees(
        pool: &Pool<Sqlite>,
        company_id: &str,
        include_inactive: bool,
    ) -> Result<Vec<Employee>, OpenTallyError> {
        let cid = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let query = if include_inactive {
                    "SELECT id, company_id, employee_code, name, department_group_id, designation,
                            date_of_joining, date_of_exit, pan, uan, esic_number, bank_account_no,
                            bank_ifsc, basic_salary, hra, other_allowances,
                            pf_applicable, esi_applicable, pt_applicable, is_active, created_at
                     FROM employees
                     WHERE company_id = ?
                     ORDER BY employee_code ASC"
                } else {
                    "SELECT id, company_id, employee_code, name, department_group_id, designation,
                            date_of_joining, date_of_exit, pan, uan, esic_number, bank_account_no,
                            bank_ifsc, basic_salary, hra, other_allowances,
                            pf_applicable, esi_applicable, pt_applicable, is_active, created_at
                     FROM employees
                     WHERE company_id = ? AND is_active = 1
                     ORDER BY employee_code ASC"
                };

                let rows: Vec<EmployeeRow> = sqlx::query_as(query)
                    .bind(&cid)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                Ok(rows.into_iter().map(Employee::from).collect())
            })
        })
        .await
    }

    /// Creates a new employee master record with PAN/UAN and salary validation.
    pub async fn create_employee(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreateEmployeeInput,
        actor: &str,
    ) -> Result<Employee, OpenTallyError> {
        let code = input.employee_code.trim();
        let name = input.name.trim();
        if code.is_empty() || code.len() > 30 {
            return Err(OpenTallyError::Validation(
                "Employee code must be between 1 and 30 characters".into(),
            ));
        }
        if name.is_empty() || name.len() > 120 {
            return Err(OpenTallyError::Validation(
                "Employee name must be between 1 and 120 characters".into(),
            ));
        }

        NaiveDate::parse_from_str(&input.date_of_joining, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation("date_of_joining must be a valid YYYY-MM-DD date".into())
        })?;
        if let Some(ref exit) = input.date_of_exit {
            NaiveDate::parse_from_str(exit, "%Y-%m-%d").map_err(|_| {
                OpenTallyError::Validation("date_of_exit must be a valid YYYY-MM-DD date".into())
            })?;
            if exit < &input.date_of_joining {
                return Err(OpenTallyError::Validation(
                    "date_of_exit cannot precede date_of_joining".into(),
                ));
            }
        }

        let clean_pan = match input
            .pan
            .as_ref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
        {
            Some(p) => {
                crate::tds::validate_pan(&p)?;
                Some(p)
            }
            None => None,
        };

        let clean_uan = match input
            .uan
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        {
            Some(u) => Some(Self::validate_uan(&u)?),
            None => None,
        };

        let basic = Self::parse_non_negative_decimal(&input.basic_salary, "basic_salary")?;
        if basic <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "basic_salary must be strictly greater than zero".into(),
            ));
        }
        let hra = Self::parse_non_negative_decimal(&input.hra, "hra")?;
        let allowances =
            Self::parse_non_negative_decimal(&input.other_allowances, "other_allowances")?;

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res: Result<Employee, OpenTallyError> = async {
            let fy_begins: String = sqlx::query_scalar(
                "SELECT financial_year_begins FROM companies WHERE id = ?",
            )
            .bind(company_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| {
                OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
            })?;

            let existing: Option<String> = sqlx::query_scalar(
                "SELECT id FROM employees WHERE company_id = ? AND employee_code = ? COLLATE NOCASE",
            )
            .bind(company_id)
            .bind(code)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if existing.is_some() {
                return Err(OpenTallyError::Validation(format!(
                    "Employee code '{}' already exists in this company",
                    code
                )));
            }

            let emp_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO employees (
                    id, company_id, employee_code, name, department_group_id, designation,
                    date_of_joining, date_of_exit, pan, uan, esic_number, bank_account_no,
                    bank_ifsc, basic_salary, hra, other_allowances,
                    pf_applicable, esi_applicable, pt_applicable, is_active
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
            )
            .bind(&emp_id)
            .bind(company_id)
            .bind(code)
            .bind(name)
            .bind(input.department_group_id.as_deref())
            .bind(input.designation.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(&input.date_of_joining)
            .bind(input.date_of_exit.as_deref())
            .bind(&clean_pan)
            .bind(&clean_uan)
            .bind(input.esic_number.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(input.bank_account_no.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(input.bank_ifsc.as_deref().map(str::trim).filter(|s| !s.is_empty()))
            .bind(Self::to_canonical_decimal(basic))
            .bind(Self::to_canonical_decimal(hra))
            .bind(Self::to_canonical_decimal(allowances))
            .bind(if input.pf_applicable { 1i64 } else { 0i64 })
            .bind(if input.esi_applicable { 1i64 } else { 0i64 })
            .bind(if input.pt_applicable { 1i64 } else { 0i64 })
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let audit_payload = serde_json::json!({
                "employee_id": &emp_id,
                "employee_code": code,
                "name": name,
                "basic_salary": Self::to_canonical_decimal(basic),
            })
            .to_string();

            Self::record_payroll_audit_log(
                &mut conn,
                company_id,
                &fy_begins,
                "employees",
                &emp_id,
                "CREATE",
                actor,
                &audit_payload,
            )
            .await?;

            let row: EmployeeRow = sqlx::query_as(
                "SELECT id, company_id, employee_code, name, department_group_id, designation,
                        date_of_joining, date_of_exit, pan, uan, esic_number, bank_account_no,
                        bank_ifsc, basic_salary, hra, other_allowances,
                        pf_applicable, esi_applicable, pt_applicable, is_active, created_at
                 FROM employees WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&emp_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            Ok(Employee::from(row))
        }
        .await;

        match res {
            Ok(emp) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(emp)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    /// Deactivates an employee record.
    pub async fn deactivate_employee(
        pool: &Pool<Sqlite>,
        company_id: &str,
        employee_id: &str,
        actor: &str,
    ) -> Result<(), OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res: Result<(), OpenTallyError> = async {
            let fy_begins: String =
                sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                    .bind(company_id)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(|_| {
                        OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
                    })?;

            let affected =
                sqlx::query("UPDATE employees SET is_active = 0 WHERE company_id = ? AND id = ?")
                    .bind(company_id)
                    .bind(employee_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?
                    .rows_affected();

            if affected == 0 {
                return Err(OpenTallyError::NotFound(format!(
                    "Employee '{}' not found in company",
                    employee_id
                )));
            }

            Self::record_payroll_audit_log(
                &mut conn,
                company_id,
                &fy_begins,
                "employees",
                employee_id,
                "UPDATE",
                actor,
                r#"{"is_active":false}"#,
            )
            .await?;

            Ok(())
        }
        .await;

        match res {
            Ok(()) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(())
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    /// Computes a dry-run preview of a monthly payroll run without modifying the database.
    pub async fn preview_payroll_run(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: &PayrollRunInput,
    ) -> Result<PayrollPreviewResult, OpenTallyError> {
        let pay_period = Self::validate_pay_period(&input.pay_period)?;
        if input.attendances.is_empty() {
            return Err(OpenTallyError::Validation(
                "Payroll run must contain at least one employee attendance entry".into(),
            ));
        }

        let employees = Self::list_employees(pool, company_id, false).await?;
        let emp_map: std::collections::HashMap<String, Employee> =
            employees.into_iter().map(|e| (e.id.clone(), e)).collect();

        let mut seen = HashSet::new();
        let mut payslips = Vec::with_capacity(input.attendances.len());

        let mut total_gross = Decimal::ZERO;
        let mut total_deductions = Decimal::ZERO;
        let mut total_net_pay = Decimal::ZERO;
        let mut total_ee_pf = Decimal::ZERO;
        let mut total_er_epf = Decimal::ZERO;
        let mut total_er_eps = Decimal::ZERO;
        let mut total_ee_esi = Decimal::ZERO;
        let mut total_er_esi = Decimal::ZERO;
        let mut total_pt = Decimal::ZERO;

        for att in &input.attendances {
            if !seen.insert(&att.employee_id) {
                return Err(OpenTallyError::Validation(format!(
                    "Duplicate attendance entry for employee '{}'",
                    att.employee_id
                )));
            }
            let emp = emp_map.get(&att.employee_id).ok_or_else(|| {
                OpenTallyError::NotFound(format!(
                    "Active employee '{}' not found in company",
                    att.employee_id
                ))
            })?;
            let p = Self::compute_employee_payslip(emp, att)?;

            total_gross += Decimal::from_str(&p.gross_earnings).unwrap_or(Decimal::ZERO);
            total_deductions += Decimal::from_str(&p.total_deductions).unwrap_or(Decimal::ZERO);
            total_net_pay += Decimal::from_str(&p.net_pay).unwrap_or(Decimal::ZERO);
            total_ee_pf += Decimal::from_str(&p.employee_pf).unwrap_or(Decimal::ZERO);
            total_er_epf += Decimal::from_str(&p.employer_epf).unwrap_or(Decimal::ZERO);
            total_er_eps += Decimal::from_str(&p.employer_eps).unwrap_or(Decimal::ZERO);
            total_ee_esi += Decimal::from_str(&p.employee_esi).unwrap_or(Decimal::ZERO);
            total_er_esi += Decimal::from_str(&p.employer_esi).unwrap_or(Decimal::ZERO);
            total_pt += Decimal::from_str(&p.professional_tax).unwrap_or(Decimal::ZERO);

            payslips.push(p);
        }

        let total_employer_contributions = total_er_epf + total_er_eps + total_er_esi;

        Ok(PayrollPreviewResult {
            pay_period,
            total_gross: Self::to_canonical_decimal(total_gross),
            total_deductions: Self::to_canonical_decimal(total_deductions),
            total_employer_contributions: Self::to_canonical_decimal(total_employer_contributions),
            total_net_pay: Self::to_canonical_decimal(total_net_pay),
            total_employee_pf: Self::to_canonical_decimal(total_ee_pf),
            total_employer_epf: Self::to_canonical_decimal(total_er_epf),
            total_employer_eps: Self::to_canonical_decimal(total_er_eps),
            total_employee_esi: Self::to_canonical_decimal(total_ee_esi),
            total_employer_esi: Self::to_canonical_decimal(total_er_esi),
            total_pt: Self::to_canonical_decimal(total_pt),
            payslips,
        })
    }

    /// Executes an atomic monthly payroll run inside `BEGIN IMMEDIATE`, posting a balanced
    /// Payroll GL voucher (`vt_payroll`), persisting payslips, and logging a SHA-256 chained audit entry.
    pub async fn execute_payroll_run(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: PayrollRunInput,
        actor: &str,
    ) -> Result<PayrollRunResult, OpenTallyError> {
        let pay_period = Self::validate_pay_period(&input.pay_period)?;
        NaiveDate::parse_from_str(&input.run_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation("run_date must be a valid YYYY-MM-DD date".into())
        })?;

        let preview = Self::preview_payroll_run(pool, company_id, &input).await?;

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res: Result<PayrollRunResult, OpenTallyError> = async {
            // 1. Period lock check
            crate::company::FinancialYearClosingService::validate_period_not_locked(
                &mut *conn,
                company_id,
                &input.run_date,
            )
            .await?;

            // 2. Fetch company FY
            let fy_begins: String = sqlx::query_scalar(
                "SELECT financial_year_begins FROM companies WHERE id = ?",
            )
            .bind(company_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|_| {
                OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
            })?;

            // 3. Reject duplicate active Posted payroll run for same pay_period
            let existing_run: Option<String> = sqlx::query_scalar(
                "SELECT id FROM payroll_runs WHERE company_id = ? AND pay_period = ? AND status = 'Posted'",
            )
            .bind(company_id)
            .bind(&pay_period)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if existing_run.is_some() {
                return Err(OpenTallyError::Validation(format!(
                    "An active Posted payroll run already exists for period '{}'. Cancel its voucher first to re-run.",
                    pay_period
                )));
            }

            // 4. Load payroll ledger mappings (or return explicit error if not configured)
            let mappings: Option<PayrollLedgerMappingRow> = sqlx::query_as(
                "SELECT salary_expense_ledger_id, employer_pf_expense_ledger_id,
                        employer_esi_expense_ledger_id, salary_payable_ledger_id,
                        pf_payable_ledger_id, esi_payable_ledger_id, pt_payable_ledger_id
                 FROM payroll_ledger_mappings
                 WHERE company_id = ?",
            )
            .bind(company_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let mappings = mappings.ok_or_else(|| {
                OpenTallyError::Validation(
                    "Payroll ledger mappings are not configured. Enable Payroll in F11 Features first."
                        .into(),
                )
            })?;

            // 5. Resolve or allocate Payroll voucher series
            let series_row: Option<(String, String)> = sqlx::query_as(
                "SELECT id, numbering_method FROM voucher_number_series
                 WHERE company_id = ? AND voucher_type_id = 'vt_payroll' AND is_active = 1
                 ORDER BY (numbering_method = 'automatic') DESC, created_at ASC
                 LIMIT 1",
            )
            .bind(company_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let voucher_id = Uuid::new_v4().to_string();
            let (series_id, numbering_method, period_start, sequence_number, voucher_number) =
                match series_row {
                    Some((sid, method)) if method == "automatic" => {
                        let alloc = VoucherSequenceService::allocate_next_sequence(
                            &mut conn,
                            company_id,
                            "vt_payroll",
                            &sid,
                            &input.run_date,
                        )
                        .await?;
                        (
                            sid,
                            "automatic".to_string(),
                            Some(alloc.period_start),
                            Some(alloc.sequence_number),
                            alloc.voucher_number,
                        )
                    }
                    Some((sid, _)) => {
                        let vnum = format!("PAY-{}-{}", pay_period, voucher_id[..6].to_uppercase());
                        (sid, "manual".to_string(), None, None, vnum)
                    }
                    None => {
                        let sid = format!("vns_pay_{}", &Uuid::new_v4().to_string()[..8]);
                        sqlx::query(
                            "INSERT INTO voucher_number_series (
                                id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period
                             ) VALUES (?, ?, 'vt_payroll', 'Payroll Register', 'PAY', 'manual', 'never')",
                        )
                        .bind(&sid)
                        .bind(company_id)
                        .execute(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;
                        let vnum = format!("PAY-{}-{}", pay_period, voucher_id[..6].to_uppercase());
                        (sid, "manual".to_string(), None, None, vnum)
                    }
                };

            // 6. Build balanced GL lines
            let total_gross = Decimal::from_str(&preview.total_gross).unwrap_or(Decimal::ZERO);
            let total_er_pf = Decimal::from_str(&preview.total_employer_epf).unwrap_or(Decimal::ZERO)
                + Decimal::from_str(&preview.total_employer_eps).unwrap_or(Decimal::ZERO);
            let total_er_esi = Decimal::from_str(&preview.total_employer_esi).unwrap_or(Decimal::ZERO);
            let total_net_pay = Decimal::from_str(&preview.total_net_pay).unwrap_or(Decimal::ZERO);
            let total_pf_payable =
                Decimal::from_str(&preview.total_employee_pf).unwrap_or(Decimal::ZERO) + total_er_pf;
            let total_esi_payable =
                Decimal::from_str(&preview.total_employee_esi).unwrap_or(Decimal::ZERO) + total_er_esi;
            let total_pt = Decimal::from_str(&preview.total_pt).unwrap_or(Decimal::ZERO);

            let total_other_deductions: Decimal = preview
                .payslips
                .iter()
                .map(|p| Decimal::from_str(&p.other_deductions).unwrap_or(Decimal::ZERO))
                .sum();

            let mut gl_lines: Vec<VoucherLine> = Vec::new();
            let mut ln = 1i64;

            let mut push_line = |ledger_id: &str, entry_type: &str, amount: Decimal, narr: &str| {
                if amount > Decimal::ZERO {
                    gl_lines.push(VoucherLine {
                        id: Uuid::new_v4().to_string(),
                        company_id: company_id.to_string(),
                        voucher_id: voucher_id.clone(),
                        line_number: ln,
                        ledger_id: ledger_id.to_string(),
                        entry_type: entry_type.to_string(),
                        amount: Self::to_canonical_decimal(amount),
                        narration: Some(narr.to_string()),
                        created_at: String::new(),
                    });
                    ln += 1;
                }
            };

            push_line(
                &mappings.salary_expense_ledger_id,
                "Dr",
                total_gross,
                &format!("Gross Salary Expense ({})", pay_period),
            );
            push_line(
                &mappings.employer_pf_expense_ledger_id,
                "Dr",
                total_er_pf,
                &format!("Employer PF Contribution ({})", pay_period),
            );
            push_line(
                &mappings.employer_esi_expense_ledger_id,
                "Dr",
                total_er_esi,
                &format!("Employer ESI Contribution ({})", pay_period),
            );
            push_line(
                &mappings.salary_payable_ledger_id,
                "Cr",
                total_net_pay,
                &format!("Net Salary Payable ({})", pay_period),
            );
            push_line(
                &mappings.pf_payable_ledger_id,
                "Cr",
                total_pf_payable,
                &format!("EPF & EPS Statutory Payable ({})", pay_period),
            );
            push_line(
                &mappings.esi_payable_ledger_id,
                "Cr",
                total_esi_payable,
                &format!("ESI Statutory Payable ({})", pay_period),
            );
            push_line(
                &mappings.pt_payable_ledger_id,
                "Cr",
                total_pt,
                &format!("Professional Tax Payable ({})", pay_period),
            );
            push_line(
                &mappings.salary_expense_ledger_id,
                "Cr",
                total_other_deductions,
                &format!("Other Payroll Recoveries ({})", pay_period),
            );

            let val_res =
                DoubleEntryValidator::validate(&gl_lines, 2, ValidationMode::Posting);
            if !val_res.is_valid {
                return Err(OpenTallyError::Validation(format!(
                    "Payroll GL voucher failed double-entry validation: {:?}",
                    val_res.diagnostics
                )));
            }
            let total_dr = total_gross + total_er_pf + total_er_esi;

            let narration = input
                .narration
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Monthly Payroll Run for {}", pay_period));

            // 7. Insert voucher as 'draft', insert lines, then transition to 'posted'
            sqlx::query(
                "INSERT INTO vouchers (
                    id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    narration, status, total_amount, created_at, updated_at
                 ) VALUES (?, ?, 'vt_payroll', ?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            )
            .bind(&voucher_id)
            .bind(company_id)
            .bind(&series_id)
            .bind(&numbering_method)
            .bind(&period_start)
            .bind(sequence_number)
            .bind(&voucher_number)
            .bind(&input.run_date)
            .bind(&input.run_date)
            .bind(&narration)
            .bind(Self::to_canonical_decimal(total_dr))
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            for line in &gl_lines {
                sqlx::query(
                    "INSERT INTO voucher_lines (
                        id, company_id, voucher_id, line_number, ledger_id, amount, entry_type, narration
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&line.id)
                .bind(company_id)
                .bind(&voucher_id)
                .bind(line.line_number)
                .bind(&line.ledger_id)
                .bind(&line.amount)
                .bind(&line.entry_type)
                .bind(line.narration.as_deref())
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }

            sqlx::query("UPDATE vouchers SET status = 'posted' WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(&voucher_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

            // 8. Insert payroll_runs & payroll_payslips
            let run_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO payroll_runs (
                    id, company_id, pay_period, run_date, voucher_id,
                    total_gross, total_deductions, total_employer_contributions, total_net_pay, status
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'Posted')",
            )
            .bind(&run_id)
            .bind(company_id)
            .bind(&pay_period)
            .bind(&input.run_date)
            .bind(&voucher_id)
            .bind(&preview.total_gross)
            .bind(&preview.total_deductions)
            .bind(&preview.total_employer_contributions)
            .bind(&preview.total_net_pay)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let mut inserted_payslips = Vec::with_capacity(preview.payslips.len());
            for p in &preview.payslips {
                let ps_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO payroll_payslips (
                        id, company_id, payroll_run_id, employee_id,
                        total_working_days, days_present, lop_days,
                        prorated_basic, prorated_hra, prorated_allowances, gross_earnings,
                        employee_pf, employer_epf, employer_eps,
                        employee_esi, employer_esi, professional_tax,
                        other_deductions, total_deductions, net_pay
                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&ps_id)
                .bind(company_id)
                .bind(&run_id)
                .bind(&p.employee_id)
                .bind(&p.total_working_days)
                .bind(&p.days_present)
                .bind(&p.lop_days)
                .bind(&p.prorated_basic)
                .bind(&p.prorated_hra)
                .bind(&p.prorated_allowances)
                .bind(&p.gross_earnings)
                .bind(&p.employee_pf)
                .bind(&p.employer_epf)
                .bind(&p.employer_eps)
                .bind(&p.employee_esi)
                .bind(&p.employer_esi)
                .bind(&p.professional_tax)
                .bind(&p.other_deductions)
                .bind(&p.total_deductions)
                .bind(&p.net_pay)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let ps_row: PayrollPayslip = sqlx::query_as(
                    "SELECT id, company_id, payroll_run_id, employee_id,
                            total_working_days, days_present, lop_days,
                            prorated_basic, prorated_hra, prorated_allowances, gross_earnings,
                            employee_pf, employer_epf, employer_eps,
                            employee_esi, employer_esi, professional_tax,
                            other_deductions, total_deductions, net_pay, created_at
                     FROM payroll_payslips WHERE id = ?",
                )
                .bind(&ps_id)
                .fetch_one(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
                inserted_payslips.push(ps_row);
            }

            let run_row: PayrollRun = sqlx::query_as(
                "SELECT id, company_id, pay_period, run_date, voucher_id,
                        total_gross, total_deductions, total_employer_contributions, total_net_pay, status, created_at
                 FROM payroll_runs WHERE id = ?",
            )
            .bind(&run_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            // 9. Emit SHA-256 chained MCA Audit Log
            let audit_payload = serde_json::json!({
                "action": "EXECUTE_PAYROLL_RUN",
                "payroll_run_id": &run_id,
                "pay_period": &pay_period,
                "voucher_id": &voucher_id,
                "voucher_number": &voucher_number,
                "total_gross": &preview.total_gross,
                "total_net_pay": &preview.total_net_pay,
                "employee_count": inserted_payslips.len(),
            })
            .to_string();

            Self::record_payroll_audit_log(
                &mut conn,
                company_id,
                &fy_begins,
                "payroll_runs",
                &run_id,
                "CREATE",
                actor,
                &audit_payload,
            )
            .await?;

            Ok(PayrollRunResult {
                run: run_row,
                voucher_number,
                payslips: inserted_payslips,
            })
        }
        .await;

        match res {
            Ok(out) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(out)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    /// Lists all payroll runs for a company (read-only).
    pub async fn list_payroll_runs(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<PayrollRun>, OpenTallyError> {
        let cid = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let rows: Vec<PayrollRun> = sqlx::query_as(
                    "SELECT id, company_id, pay_period, run_date, voucher_id,
                            total_gross, total_deductions, total_employer_contributions, total_net_pay, status, created_at
                     FROM payroll_runs
                     WHERE company_id = ?
                     ORDER BY pay_period DESC, created_at DESC",
                )
                .bind(&cid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
                Ok(rows)
            })
        })
        .await
    }

    /// Marks any Posted payroll_run associated with a cancelled/reversed voucher as 'Cancelled'.
    pub async fn cancel_payroll_run_on_voucher_cancel(
        conn: &mut SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        actor_id: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        let table_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'payroll_runs'",
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(false);

        if !table_exists {
            return Ok(());
        }

        let run_opt: Option<(String, String)> = sqlx::query_as(
            "SELECT id, pay_period FROM payroll_runs WHERE company_id = ? AND voucher_id = ? AND status = 'Posted'",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((run_id, pay_period)) = run_opt {
            sqlx::query(
                "UPDATE payroll_runs SET status = 'Cancelled' WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&run_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let fy_begins: String =
                sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                    .bind(company_id)
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

            let payload = serde_json::json!({
                "action": "CANCEL_PAYROLL_RUN",
                "payroll_run_id": &run_id,
                "pay_period": &pay_period,
                "voucher_id": voucher_id,
            })
            .to_string();

            Self::record_payroll_audit_log(
                conn,
                company_id,
                &fy_begins,
                "payroll_runs",
                &run_id,
                "CANCEL",
                actor_id.unwrap_or("system"),
                &payload,
            )
            .await?;
        }

        Ok(())
    }

    /// Internal helper to record an immutable MCA audit_log row with SHA-256 hash chaining.
    #[allow(clippy::too_many_arguments)]
    async fn record_payroll_audit_log(
        conn: &mut SqliteConnection,
        company_id: &str,
        financial_year_begins: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        performed_by: &str,
        payload: &str,
    ) -> Result<(), OpenTallyError> {
        let prev_hash_opt: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev_hash = prev_hash_opt.unwrap_or_else(|| "GENESIS".to_string());
        let id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let entry_hash = crate::audit::certification::AuditCertificationService::compute_entry_hash(
            &prev_hash,
            &id,
            company_id,
            entity_type,
            entity_id,
            action,
            &timestamp,
            Some(payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, old_values, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(financial_year_begins)
        .bind(action)
        .bind(payload)
        .bind(performed_by)
        .bind(&timestamp)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }
}
