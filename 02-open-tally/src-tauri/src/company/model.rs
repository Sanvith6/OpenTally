use crate::errors::OpenTallyError;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::Sqlite;

/// Company status enum representing the lifecycle state of a company.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanyStatus {
    Active,
    Archived,
    Closed,
}

impl CompanyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompanyStatus::Active => "active",
            CompanyStatus::Archived => "archived",
            CompanyStatus::Closed => "closed",
        }
    }
}

impl std::str::FromStr for CompanyStatus {
    type Err = OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(CompanyStatus::Active),
            "archived" => Ok(CompanyStatus::Archived),
            "closed" => Ok(CompanyStatus::Closed),
            other => Err(OpenTallyError::Validation(format!(
                "Invalid company status: '{}'. Must be 'active', 'archived', or 'closed'",
                other
            ))),
        }
    }
}

impl std::fmt::Display for CompanyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl<'r> sqlx::Decode<'r, Sqlite> for CompanyStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <&str as sqlx::Decode<Sqlite>>::decode(value)?;
        s.parse()
            .map_err(|e: OpenTallyError| Box::new(e) as sqlx::error::BoxDynError)
    }
}

impl sqlx::Type<Sqlite> for CompanyStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <&str as sqlx::Type<Sqlite>>::type_info()
    }
}

/// Core Company domain model matching the `companies` database entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::FromRow)]
pub struct Company {
    pub id: String,
    pub name: String,
    pub mailing_name: String,
    pub country: String,
    pub state: String,
    pub financial_year_begins: String,
    pub books_beginning_from: String,
    pub currency_symbol: String,
    pub currency_formal_name: String,
    pub currency_decimal_places: i64,
    pub status: CompanyStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// Request payload for creating a new company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyRequest {
    pub name: String,
    pub mailing_name: Option<String>,
    pub country: Option<String>,
    pub state: Option<String>,
    pub financial_year_begins: String,
    pub books_beginning_from: Option<String>,
    pub currency_symbol: Option<String>,
    pub currency_formal_name: Option<String>,
    pub currency_decimal_places: Option<i64>,
}

/// Request payload for updating permitted company fields.
/// In accordance with ADR/Spec, invariant fields (id, name, financial_year_begins,
/// books_beginning_from) cannot be altered after creation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCompanyRequest {
    pub mailing_name: Option<String>,
    pub country: Option<String>,
    pub state: Option<String>,
    pub currency_symbol: Option<String>,
    pub currency_formal_name: Option<String>,
}

/// Company settings key-value record matching the `company_settings` entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompanySettings {
    pub company_id: String,
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Validation Functions
// ---------------------------------------------------------------------------

/// Strict calendar date parsing with leap-year awareness.
pub fn validate_calendar_date(date_str: &str) -> Result<NaiveDate, OpenTallyError> {
    NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d").map_err(|e| {
        OpenTallyError::Validation(format!(
            "Invalid calendar date '{}'. Must be a valid date in YYYY-MM-DD format: {}",
            date_str, e
        ))
    })
}

/// Strict financial year boundary and leap-year validation.
pub fn validate_financial_year(
    fy_begins_str: &str,
    books_beginning_str: &str,
) -> Result<(NaiveDate, NaiveDate), OpenTallyError> {
    let fy_start = validate_calendar_date(fy_begins_str)?;
    let books_start = validate_calendar_date(books_beginning_str)?;

    let next_year = fy_start.year() + 1;
    let fy_end = if fy_start.month() == 2 && fy_start.day() == 29 {
        // If FY starts on leap day (Feb 29), the fiscal period ends on Feb 28 of next year
        NaiveDate::from_ymd_opt(next_year, 2, 28).ok_or_else(|| {
            OpenTallyError::Validation(
                "Failed to compute FY end date for leap day start".to_string(),
            )
        })?
    } else {
        let next_year_start = NaiveDate::from_ymd_opt(next_year, fy_start.month(), fy_start.day())
            .ok_or_else(|| {
                OpenTallyError::Validation("Failed to compute FY end date".to_string())
            })?;
        next_year_start - chrono::Duration::days(1)
    };

    if books_start < fy_start || books_start > fy_end {
        return Err(OpenTallyError::Validation(format!(
            "Books beginning date '{}' must be within financial year range [{} to {}]",
            books_beginning_str, fy_start, fy_end
        )));
    }

    Ok((fy_start, fy_end))
}

/// Validate currency decimal places (allowed range: 0..=4).
pub fn validate_currency_decimals(decimals: i64) -> Result<(), OpenTallyError> {
    if (0..=4).contains(&decimals) {
        Ok(())
    } else {
        Err(OpenTallyError::Validation(format!(
            "Currency decimal places must be between 0 and 4, got: {}",
            decimals
        )))
    }
}
