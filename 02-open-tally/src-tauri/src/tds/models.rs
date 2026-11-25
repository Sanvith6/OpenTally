use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeductorType {
    Company,
    IndividualHuf,
    Firm,
    Government,
    Other,
}

impl DeductorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Company => "Company",
            Self::IndividualHuf => "Individual_HUF",
            Self::Firm => "Firm",
            Self::Government => "Government",
            Self::Other => "Other",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().replace('_', "").as_str() {
            "company" => Self::Company,
            "individualhuf" | "individual" | "huf" => Self::IndividualHuf,
            "firm" => Self::Firm,
            "government" | "govt" => Self::Government,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeducteeType {
    IndividualHuf,
    CompanyDomestic,
    CompanyForeign,
    Firm,
    Other,
}

impl DeducteeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::IndividualHuf => "Individual_HUF",
            Self::CompanyDomestic => "Company_Domestic",
            Self::CompanyForeign => "Company_Foreign",
            Self::Firm => "Firm",
            Self::Other => "Other",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().replace('_', "").as_str() {
            "individualhuf" | "individual" | "huf" => Self::IndividualHuf,
            "companydomestic" | "domesticcompany" | "company" => Self::CompanyDomestic,
            "companyforeign" | "foreigncompany" => Self::CompanyForeign,
            "firm" => Self::Firm,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyTdsProfile {
    pub company_id: String,
    pub tan: Option<String>,
    pub deductor_type: String,
    pub tds_enabled: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveCompanyTdsProfileInput {
    pub company_id: String,
    pub tan: Option<String>,
    pub deductor_type: String,
    pub tds_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LedgerTdsProfile {
    pub id: String,
    pub company_id: String,
    pub ledger_id: String,
    pub pan: Option<String>,
    pub deductee_type: String,
    pub lower_deduction_cert_no: Option<String>,
    pub lower_deduction_rate: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveLedgerTdsProfileInput {
    pub company_id: String,
    pub ledger_id: String,
    pub pan: Option<String>,
    pub deductee_type: String,
    pub lower_deduction_cert_no: Option<String>,
    pub lower_deduction_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TdsNatureOfPayment {
    pub id: String,
    pub company_id: String,
    pub section_code: String,
    pub name: String,
    pub rate_individual_huf: String,
    pub rate_others: String,
    pub rate_without_pan: String,
    pub single_threshold_limit: String,
    pub aggregate_threshold_limit: String,
    pub tds_payable_ledger_id: Option<String>,
    pub is_tcs: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTdsNatureOfPaymentInput {
    pub company_id: String,
    pub section_code: String,
    pub name: String,
    pub rate_individual_huf: String,
    pub rate_others: String,
    pub rate_without_pan: Option<String>,
    pub single_threshold_limit: Option<String>,
    pub aggregate_threshold_limit: Option<String>,
    pub tds_payable_ledger_id: Option<String>,
    pub is_tcs: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherTdsDetail {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub party_ledger_id: String,
    pub party_pan: Option<String>,
    pub section_code: String,
    pub deductee_type: String,
    pub gross_amount: String,
    pub tds_rate: String,
    pub tds_amount: String,
    pub net_payable_amount: String,
    pub is_pan_missing_higher_rate: i64,
    pub challan_bsr_code: Option<String>,
    pub challan_date: Option<String>,
    pub challan_serial_no: Option<String>,
    pub payment_voucher_id: Option<String>,
    pub is_paid: i64,
    pub is_cancelled: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdsCalculationPreviewInput {
    pub company_id: String,
    pub party_ledger_id: String,
    pub section_code: String,
    pub taxable_amount: String,
    pub voucher_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdsCalculationPreviewResult {
    pub section_code: String,
    pub section_name: String,
    pub deductee_type: String,
    pub party_pan: Option<String>,
    pub is_pan_missing: bool,
    pub is_pan_missing_higher_rate: bool,
    pub taxable_base: String,
    pub applicable_rate: String,
    pub threshold_exceeded: bool,
    pub single_threshold: String,
    pub aggregate_threshold: String,
    pub prior_aggregate_amount: String,
    pub current_aggregate_amount: String,
    pub tds_amount: String,
    pub net_payable_amount: String,
    pub tds_payable_ledger_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordTdsChallanPaymentInput {
    pub company_id: String,
    pub voucher_tds_detail_ids: Vec<String>,
    pub challan_bsr_code: String,
    pub challan_date: String,
    pub challan_serial_no: String,
    pub payment_voucher_id: Option<String>,
    pub actor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdsOutstandingVoucherItem {
    pub voucher_tds_id: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub party_pan: Option<String>,
    pub section_code: String,
    pub gross_amount: String,
    pub tds_rate: String,
    pub tds_amount: String,
    pub due_date: String,
    pub status: String, // "Due" or "Overdue"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdsOutstandingSectionSummary {
    pub section_code: String,
    pub section_name: String,
    pub unpaid_tds_amount: String,
    pub voucher_count: usize,
    pub vouchers: Vec<TdsOutstandingVoucherItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TdsOutstandingReport {
    pub as_of_date: String,
    pub total_unpaid_tds: String,
    pub sections: Vec<TdsOutstandingSectionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form26qChallanItem {
    pub challan_bsr_code: String,
    pub challan_date: String,
    pub challan_serial_no: String,
    pub payment_voucher_id: Option<String>,
    pub total_tds_deposited: String,
    pub voucher_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form26qDeductionItem {
    pub voucher_tds_id: String,
    pub voucher_number: String,
    pub deductee_pan: Option<String>,
    pub deductee_name: String,
    pub deductee_type: String,
    pub section_code: String,
    pub payment_date: String,
    pub amount_paid: String,
    pub tds_rate: String,
    pub tds_deducted: String,
    pub is_pan_missing_higher_rate: bool,
    pub challan_serial_no: Option<String>,
    pub challan_date: Option<String>,
    pub challan_bsr_code: Option<String>,
    pub status: String, // "Deposited" or "Unpaid"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Form26qSummary {
    pub company_id: String,
    pub financial_year: String,
    pub quarter: String,
    pub deductor_tan: Option<String>,
    pub deductor_name: String,
    pub total_amount_paid: String,
    pub total_tds_deducted: String,
    pub total_tds_deposited: String,
    pub total_tds_unpaid: String,
    pub challans: Vec<Form26qChallanItem>,
    pub deductions: Vec<Form26qDeductionItem>,
}
