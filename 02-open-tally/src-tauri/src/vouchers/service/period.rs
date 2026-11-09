use crate::company::validate_calendar_date;
use crate::errors::OpenTallyError;

/// Authoritative helper: Derives the discrete sequence period start date (YYYY-MM-DD)
/// from voucher date, reset period, and fiscal boundaries.
/// Enforces period/date consistency across voucher posting, manual parsing, token evaluation,
/// and sequence recovery.
pub fn derive_period_start(
    voucher_date: &str,
    restart_period: &str,
    financial_year_begins: &str,
    books_beginning_from: &str,
) -> Result<String, OpenTallyError> {
    // 1. Strict calendar date validation of voucher date and books_beginning_from
    let v_date = validate_calendar_date(voucher_date)?;
    let books_start = validate_calendar_date(books_beginning_from)?;

    // Reject voucher date before company books_beginning_from
    if v_date < books_start {
        return Err(OpenTallyError::Validation(format!(
            "Voucher date '{}' cannot be earlier than company books beginning date '{}'",
            voucher_date, books_beginning_from
        )));
    }

    let v_parts: Vec<&str> = voucher_date.split('-').collect();
    if v_parts.len() != 3 {
        return Err(OpenTallyError::Validation(format!(
            "Invalid voucher date format: {}",
            voucher_date
        )));
    }
    let v_year: i32 = v_parts[0]
        .parse()
        .map_err(|_| OpenTallyError::Validation("Invalid voucher year".into()))?;
    let v_month: u32 = v_parts[1]
        .parse()
        .map_err(|_| OpenTallyError::Validation("Invalid voucher month".into()))?;
    let v_day: u32 = v_parts[2]
        .parse()
        .map_err(|_| OpenTallyError::Validation("Invalid voucher day".into()))?;

    match restart_period {
        "yearly" => {
            validate_calendar_date(financial_year_begins)?;
            let fy_parts: Vec<&str> = financial_year_begins.split('-').collect();
            if fy_parts.len() != 3 {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid financial_year_begins format: {}",
                    financial_year_begins
                )));
            }
            let fy_m: u32 = fy_parts[1]
                .parse()
                .map_err(|_| OpenTallyError::Validation("Invalid FY month".into()))?;
            let fy_d: u32 = fy_parts[2]
                .parse()
                .map_err(|_| OpenTallyError::Validation("Invalid FY day".into()))?;

            // Leap-year normalization: If FY begins on Feb 29, normalize to Feb 28 in non-leap years
            let is_leap_year = |y: i32| (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            let bound_d = if fy_m == 2 && fy_d == 29 && !is_leap_year(v_year) {
                28
            } else {
                fy_d
            };

            let start_year = if (v_month, v_day) >= (fy_m, bound_d) {
                v_year
            } else {
                v_year - 1
            };

            // Final day normalization based on start_year leap year status
            let actual_fy_d = if fy_m == 2 && fy_d == 29 && !is_leap_year(start_year) {
                28
            } else {
                fy_d
            };

            Ok(format!("{:04}-{:02}-{:02}", start_year, fy_m, actual_fy_d))
        }
        "monthly" => Ok(format!("{:04}-{:02}-01", v_year, v_month)),
        "daily" => Ok(voucher_date.to_string()),
        "never" => {
            // Fixed sequence period anchored to company books_beginning_from
            Ok(books_beginning_from.to_string())
        }
        other => Err(OpenTallyError::Validation(format!(
            "Unsupported restart period: {}",
            other
        ))),
    }
}

/// Enforces period/date consistency by verifying that an optional caller-supplied `period_start`
/// matches the authoritatively derived `period_start`.
pub fn validate_period_consistency(
    supplied_period: Option<&str>,
    derived_period: &str,
    voucher_date: &str,
    restart_period: &str,
) -> Result<(), OpenTallyError> {
    if let Some(supplied) = supplied_period {
        if supplied != derived_period {
            return Err(OpenTallyError::Validation(format!(
                "Period/date inconsistency: supplied period_start '{}' does not match derived period_start '{}' for voucher date '{}' (restart_period: '{}')",
                supplied, derived_period, voucher_date, restart_period
            )));
        }
    }
    Ok(())
}
