use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EmployeeGroup {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Employee {
    pub id: String,
    pub company_id: String,
    pub employee_code: String,
    pub name: String,
    pub department_group_id: Option<String>,
    pub designation: Option<String>,
    pub date_of_joining: String,
    pub date_of_exit: Option<String>,
    pub pan: Option<String>,
    pub uan: Option<String>,
    pub esic_number: Option<String>,
    pub bank_account_no: Option<String>,
    pub bank_ifsc: Option<String>,
    pub basic_salary: String,
    pub hra: String,
    pub other_allowances: String,
    pub pf_applicable: bool,
    pub esi_applicable: bool,
    pub pt_applicable: bool,
    pub is_active: bool,
    pub work_state_code: String,
    pub tax_regime: String,
    pub annual_declared_deductions: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateEmployeeInput {
    pub employee_code: String,
    pub name: String,
    #[serde(default)]
    pub department_group_id: Option<String>,
    #[serde(default)]
    pub designation: Option<String>,
    pub date_of_joining: String,
    #[serde(default)]
    pub date_of_exit: Option<String>,
    #[serde(default)]
    pub pan: Option<String>,
    #[serde(default)]
    pub uan: Option<String>,
    #[serde(default)]
    pub esic_number: Option<String>,
    #[serde(default)]
    pub bank_account_no: Option<String>,
    #[serde(default)]
    pub bank_ifsc: Option<String>,
    pub basic_salary: String,
    #[serde(default = "default_zero")]
    pub hra: String,
    #[serde(default = "default_zero")]
    pub other_allowances: String,
    #[serde(default = "default_true")]
    pub pf_applicable: bool,
    #[serde(default)]
    pub esi_applicable: bool,
    #[serde(default = "default_true")]
    pub pt_applicable: bool,
    #[serde(default = "default_state_ka")]
    pub work_state_code: String,
    #[serde(default = "default_tax_regime")]
    pub tax_regime: String,
    #[serde(default = "default_zero")]
    pub annual_declared_deductions: String,
}

fn default_state_ka() -> String {
    "29".to_string()
}

fn default_tax_regime() -> String {
    "New".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProfessionalTaxSlab {
    pub id: String,
    pub company_id: String,
    pub state_code: String,
    pub min_gross: String,
    pub max_gross: Option<String>,
    pub monthly_tax: String,
    pub february_tax: Option<String>,
    pub created_at: String,
}

fn default_zero() -> String {
    "0".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollAttendanceInput {
    pub employee_id: String,
    pub total_working_days: String,
    pub days_present: String,
    #[serde(default = "default_zero")]
    pub other_deductions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollRunInput {
    pub pay_period: String,
    pub run_date: String,
    #[serde(default)]
    pub narration: Option<String>,
    pub attendances: Vec<PayrollAttendanceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedPayslipPreview {
    pub employee_id: String,
    pub employee_code: String,
    pub employee_name: String,
    pub designation: Option<String>,
    pub total_working_days: String,
    pub days_present: String,
    pub lop_days: String,
    pub prorated_basic: String,
    pub prorated_hra: String,
    pub prorated_allowances: String,
    pub gross_earnings: String,
    pub employee_pf: String,
    pub employer_epf: String,
    pub employer_eps: String,
    pub employee_esi: String,
    pub employer_esi: String,
    pub professional_tax: String,
    pub tds_sec192: String,
    pub other_deductions: String,
    pub total_deductions: String,
    pub net_pay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollPreviewResult {
    pub pay_period: String,
    pub total_gross: String,
    pub total_deductions: String,
    pub total_employer_contributions: String,
    pub total_net_pay: String,
    pub total_employee_pf: String,
    pub total_employer_epf: String,
    pub total_employer_eps: String,
    pub total_employee_esi: String,
    pub total_employer_esi: String,
    pub total_pt: String,
    pub total_tds_sec192: String,
    pub payslips: Vec<ComputedPayslipPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PayrollRun {
    pub id: String,
    pub company_id: String,
    pub pay_period: String,
    pub run_date: String,
    pub voucher_id: String,
    pub total_gross: String,
    pub total_deductions: String,
    pub total_employer_contributions: String,
    pub total_net_pay: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PayrollPayslip {
    pub id: String,
    pub company_id: String,
    pub payroll_run_id: String,
    pub employee_id: String,
    pub total_working_days: String,
    pub days_present: String,
    pub lop_days: String,
    pub prorated_basic: String,
    pub prorated_hra: String,
    pub prorated_allowances: String,
    pub gross_earnings: String,
    pub employee_pf: String,
    pub employer_epf: String,
    pub employer_eps: String,
    pub employee_esi: String,
    pub employer_esi: String,
    pub professional_tax: String,
    pub tds_sec192: String,
    pub other_deductions: String,
    pub total_deductions: String,
    pub net_pay: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollRunResult {
    pub run: PayrollRun,
    pub voucher_number: String,
    pub payslips: Vec<PayrollPayslip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollStatutoryRow {
    pub payslip_id: String,
    pub employee_id: String,
    pub employee_code: String,
    pub employee_name: String,
    pub pan: Option<String>,
    pub uan: Option<String>,
    pub esic_number: Option<String>,
    pub total_working_days: String,
    pub days_present: String,
    pub lop_days: String,
    pub gross_earnings: String,
    pub employee_pf: String,
    pub employer_epf: String,
    pub employer_eps: String,
    pub employee_esi: String,
    pub employer_esi: String,
    pub professional_tax: String,
    pub other_deductions: String,
    pub total_deductions: String,
    pub net_pay: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayrollStatutorySummary {
    pub pay_period: String,
    pub run_id: Option<String>,
    pub run_status: Option<String>,
    pub voucher_id: Option<String>,
    pub total_employees: usize,
    pub total_gross: String,
    pub total_employee_pf: String,
    pub total_employer_epf: String,
    pub total_employer_eps: String,
    pub total_pf_remittance: String,
    pub total_employee_esi: String,
    pub total_employer_esi: String,
    pub total_esi_remittance: String,
    pub total_pt_remittance: String,
    pub total_net_pay: String,
    pub rows: Vec<PayrollStatutoryRow>,
}
