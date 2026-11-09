use crate::errors::OpenTallyError;
use crate::vouchers::model::SeriesNamespaceInfo;

/// Validates that a series name is non-empty and does not exceed 100 characters.
pub fn validate_series_name(name: &str) -> Result<(), OpenTallyError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(
            "Series name cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 100 {
        return Err(OpenTallyError::Validation(
            "Series name cannot exceed 100 characters".to_string(),
        ));
    }
    Ok(())
}

/// Validates that a series code is non-empty, alphanumeric/hyphen/underscore, and does not exceed 20 characters.
pub fn validate_series_code(code: &str) -> Result<(), OpenTallyError> {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(
            "Series code cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 20 {
        return Err(OpenTallyError::Validation(
            "Series code cannot exceed 20 characters".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(OpenTallyError::Validation(
            "Series code must contain only ASCII alphanumeric characters, hyphens, or underscores"
                .to_string(),
        ));
    }
    Ok(())
}

/// Validates that a restart period is one of ('yearly', 'monthly', 'daily', 'never').
pub fn validate_restart_period(period: &str) -> Result<(), OpenTallyError> {
    match period {
        "yearly" | "monthly" | "daily" | "never" => Ok(()),
        other => Err(OpenTallyError::Validation(format!(
            "Invalid restart period '{}'; must be one of 'yearly', 'monthly', 'daily', 'never'",
            other
        ))),
    }
}

/// Validates that a numbering method is one of ('automatic', 'manual').
pub fn validate_numbering_method(method: &str) -> Result<(), OpenTallyError> {
    match method {
        "automatic" | "manual" => Ok(()),
        other => Err(OpenTallyError::Validation(format!(
            "Invalid numbering method '{}'; must be 'automatic' or 'manual'",
            other
        ))),
    }
}

/// Validates that new_series does not overlap in namespace with any existing_series.
pub fn validate_series_namespace_isolation(
    existing_series: &[SeriesNamespaceInfo],
    new_series: &SeriesNamespaceInfo,
) -> Result<(), OpenTallyError> {
    for ex in existing_series {
        // 1. Series code uniqueness check
        if ex.series_code.eq_ignore_ascii_case(&new_series.series_code) {
            return Err(OpenTallyError::Validation(format!(
                "Series code '{}' already exists in this voucher type",
                new_series.series_code
            )));
        }

        // 2. Series name uniqueness check
        if ex.series_name.eq_ignore_ascii_case(&new_series.series_name) {
            return Err(OpenTallyError::Validation(format!(
                "Series name '{}' already exists in this voucher type",
                new_series.series_name
            )));
        }

        // 3. Identical templates check
        if ex.prefix == new_series.prefix && ex.suffix == new_series.suffix {
            return Err(OpenTallyError::Validation(format!(
                "Series '{}' has identical prefix '{}' and suffix '{}' to existing series '{}'. Namespaces must be distinct.",
                new_series.series_name, new_series.prefix, new_series.suffix, ex.series_name
            )));
        }

        // 4. Prefix overlap check
        if ex.prefix == new_series.prefix {
            if ex.suffix.is_empty() || new_series.suffix.is_empty() {
                return Err(OpenTallyError::Validation(format!(
                    "Series '{}' shares prefix '{}' with '{}' but lacks distinct mandatory suffixes.",
                    new_series.series_name, new_series.prefix, ex.series_name
                )));
            }
        } else if ex.prefix.starts_with(&new_series.prefix)
            || new_series.prefix.starts_with(&ex.prefix)
        {
            let (shorter, longer) = if new_series.prefix.len() < ex.prefix.len() {
                (new_series.prefix.as_str(), ex.prefix.as_str())
            } else {
                (ex.prefix.as_str(), new_series.prefix.as_str())
            };
            let diff = &longer[shorter.len()..];
            if !diff.is_empty() && diff.chars().all(|c| c.is_ascii_digit()) {
                return Err(OpenTallyError::Validation(format!(
                    "Ambiguous series prefixes: '{}' and '{}' can generate identical numbers with digit sequences.",
                    shorter, longer
                )));
            }
        }
    }
    Ok(())
}

/// Validates format tokens and ensures periodic templates are not static-only.
pub fn validate_configuration_tokens(
    restart_period: &str,
    financial_year_begins: &str,
    prefix_template: &str,
    suffix_template: &str,
) -> Result<(), OpenTallyError> {
    let template = format!("{}{}", prefix_template, suffix_template);
    let fy_parts: Vec<&str> = financial_year_begins.split('-').collect();
    let is_calendar_year = fy_parts.len() == 3 && fy_parts[1] == "01" && fy_parts[2] == "01";

    match restart_period {
        "yearly" => {
            if is_calendar_year {
                if !template.contains("{YYYY}")
                    && !template.contains("{YY-YY}")
                    && !template.contains("{YYYY-YYYY}")
                {
                    return Err(OpenTallyError::Validation(
                        "Yearly calendar-year reset requires at least one periodic token from {YYYY}, {YY-YY}, or {YYYY-YYYY} in prefix or suffix. Static-only templates are prohibited for periodic resets.".into()
                    ));
                }
            } else {
                if template.contains("{YYYY}")
                    && !template.contains("{YY-YY}")
                    && !template.contains("{YYYY-YYYY}")
                {
                    return Err(OpenTallyError::Validation(
                        "Yearly financial-year reset prohibits bare {YYYY} token because financial years cross calendar boundaries (e.g. 15 Jan 2027 and 15 Apr 2027 share calendar year 2027 but belong to FY 26-27 and FY 27-28). Require {YY-YY} or {YYYY-YYYY}.".into()
                    ));
                }
                if !template.contains("{YY-YY}") && !template.contains("{YYYY-YYYY}") {
                    return Err(OpenTallyError::Validation(
                        "Yearly financial-year reset requires {YY-YY} or {YYYY-YYYY} token in prefix or suffix. Static-only templates are prohibited for periodic resets.".into()
                    ));
                }
            }
        }
        "monthly" => {
            if !template.contains("{YYYY-MM}") {
                return Err(OpenTallyError::Validation(
                    "Monthly reset requires {YYYY-MM} token in prefix or suffix. Static-only templates are prohibited for periodic resets.".into()
                ));
            }
        }
        "daily" => {
            if !template.contains("{YYYYMMDD}") && !template.contains("{YYYY-MM-DD}") {
                return Err(OpenTallyError::Validation(
                    "Daily reset requires {YYYYMMDD} or {YYYY-MM-DD} token in prefix or suffix. Static-only templates are prohibited for periodic resets.".into()
                ));
            }
        }
        "never" => {
            // Static templates are strictly permitted for never-reset mode
        }
        other => {
            return Err(OpenTallyError::Validation(format!(
                "Unsupported restart period: {}",
                other
            )))
        }
    }
    Ok(())
}

// ============================================================================
// Sprint 2 Stage 4: Voucher Header & Line Validators
// ============================================================================

/// Validates that a voucher date string is a valid ISO 8601 YYYY-MM-DD calendar date
/// and is not earlier than the company's books_beginning_from date.
pub fn validate_voucher_date(date: &str, books_beginning_from: &str) -> Result<(), OpenTallyError> {
    let parsed_date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| {
        OpenTallyError::Validation(format!(
            "Invalid calendar date '{}'. Must be a valid date in YYYY-MM-DD format.",
            date
        ))
    })?;

    let parsed_books = chrono::NaiveDate::parse_from_str(books_beginning_from, "%Y-%m-%d")
        .map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid company books beginning date '{}'.",
                books_beginning_from
            ))
        })?;

    if parsed_date < parsed_books {
        return Err(OpenTallyError::Validation(format!(
            "Voucher date '{}' cannot be earlier than company books beginning date '{}'",
            date, books_beginning_from
        )));
    }

    Ok(())
}

/// Validates that an entry_type is strictly 'Dr' or 'Cr'.
pub fn validate_entry_type(entry_type: &str) -> Result<(), OpenTallyError> {
    match entry_type {
        "Dr" | "Cr" => Ok(()),
        other => Err(OpenTallyError::Validation(format!(
            "Invalid entry type '{}'; must be strictly 'Dr' or 'Cr'",
            other
        ))),
    }
}

/// Validates that a line amount string satisfies positive canonical monetary grammar
/// (matching SQLite chk_voucher_line_amount_canonical) and currency scale limits.
pub fn validate_canonical_amount(
    amount: &str,
    currency_decimal_places: u32,
) -> Result<rust_decimal::Decimal, OpenTallyError> {
    if amount.is_empty() || amount.len() > 25 {
        return Err(OpenTallyError::Validation(
            "Amount length must be between 1 and 25 characters".to_string(),
        ));
    }

    // Check characters: only digits and at most one dot
    let mut dot_count = 0;
    for c in amount.chars() {
        if c == '.' {
            dot_count += 1;
        } else if !c.is_ascii_digit() {
            return Err(OpenTallyError::Validation(format!(
                "Amount '{}' contains invalid character '{}'",
                amount, c
            )));
        }
    }

    if dot_count > 1 {
        return Err(OpenTallyError::Validation(format!(
            "Amount '{}' contains multiple decimal points",
            amount
        )));
    }

    if amount.starts_with('.') || amount.ends_with('.') {
        return Err(OpenTallyError::Validation(format!(
            "Amount '{}' cannot start or end with a decimal point",
            amount
        )));
    }

    if dot_count == 1 {
        let mut parts = amount.split('.');
        let int_part = parts.next().unwrap_or("");
        let frac_part = parts.next().unwrap_or("");

        if int_part.is_empty() {
            return Err(OpenTallyError::Validation(format!(
                "Amount '{}' missing integer part before decimal point",
                amount
            )));
        }

        if int_part.starts_with('0') && int_part.len() > 1 {
            return Err(OpenTallyError::Validation(format!(
                "Amount '{}' has invalid leading zero in integer part",
                amount
            )));
        }

        if frac_part.is_empty() || frac_part.len() > 4 {
            return Err(OpenTallyError::Validation(format!(
                "Amount '{}' fractional part must be between 1 and 4 decimal places",
                amount
            )));
        }

        if int_part == "0" && !frac_part.chars().any(|c| c != '0') {
            return Err(OpenTallyError::Validation(
                "Amount cannot be zero".to_string(),
            ));
        }
    } else {
        // No dot
        if amount.starts_with('0') {
            return Err(OpenTallyError::Validation(format!(
                "Amount '{}' cannot have leading zero or be zero",
                amount
            )));
        }
    }

    let decimal: rust_decimal::Decimal = amount.parse().map_err(|e| {
        OpenTallyError::Validation(format!("Failed to parse amount '{}': {}", amount, e))
    })?;

    if decimal <= rust_decimal::Decimal::ZERO {
        return Err(OpenTallyError::Validation(
            "Amount must be strictly positive".to_string(),
        ));
    }

    if decimal.scale() > currency_decimal_places {
        return Err(OpenTallyError::Validation(format!(
            "Amount '{}' has {} decimal places, exceeding company currency precision of {}",
            amount,
            decimal.scale(),
            currency_decimal_places
        )));
    }

    Ok(decimal)
}

/// Validates optional narration field (max 500 characters).
pub fn validate_narration(narration: Option<&str>) -> Result<(), OpenTallyError> {
    if let Some(narr) = narration {
        if narr.len() > 500 {
            return Err(OpenTallyError::Validation(
                "Narration cannot exceed 500 characters".to_string(),
            ));
        }
    }
    Ok(())
}

/// Maximum allowable line items per voucher (Sprint 2 Stage 4 Hardening TD-S2-002).
pub const MAX_VOUCHER_LINES: usize = 10_000;
pub const MAX_LINE_NUMBER: i64 = 10_000;
pub const LINE_RENUMBER_OFFSET: i64 = 1_000_000;

/// Validates total line count for a voucher (max 10,000 lines).
pub fn validate_voucher_line_count(count: usize) -> Result<(), OpenTallyError> {
    if count > MAX_VOUCHER_LINES {
        return Err(OpenTallyError::Validation(format!(
            "Voucher line count {} exceeds maximum allowed limit of {}",
            count, MAX_VOUCHER_LINES
        )));
    }
    Ok(())
}

/// Validates line number (> 0 and <= 10,000).
pub fn validate_line_number(line_number: i64) -> Result<(), OpenTallyError> {
    if line_number <= 0 {
        return Err(OpenTallyError::Validation(
            "Line number must be strictly positive (> 0)".to_string(),
        ));
    }
    if line_number > MAX_LINE_NUMBER {
        return Err(OpenTallyError::Validation(format!(
            "Line number {} exceeds maximum allowed line number of {}",
            line_number, MAX_LINE_NUMBER
        )));
    }
    Ok(())
}

/// Validates that all ledger references in voucher lines exist in the specified company,
/// and do not violate reserved ledger protections (e.g. system Profit & Loss A/c).
pub async fn validate_voucher_ledger_integrity<L: crate::vouchers::model::DoubleEntryLine>(
    conn: &mut sqlx::SqliteConnection,
    company_id: &str,
    lines: &[L],
) -> Result<(), OpenTallyError> {
    if lines.is_empty() {
        return Ok(());
    }

    let mut ledger_ids: Vec<&str> = lines.iter().map(|l| l.ledger_id().trim()).collect();
    ledger_ids.sort_unstable();
    ledger_ids.dedup();

    for ledger_id in ledger_ids {
        if ledger_id.is_empty() {
            return Err(OpenTallyError::Validation(
                "Voucher line item missing ledger reference".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT ledger_classification, is_reserved FROM ledgers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(ledger_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match row {
            Some((classification, _)) => {
                if classification == "profit_and_loss" {
                    return Err(OpenTallyError::Validation(format!(
                        "Ledger '{}' is the reserved Profit & Loss system ledger and cannot be directly posted to in standard vouchers",
                        ledger_id
                    )));
                }
            }
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Ledger '{}' not found for company '{}'",
                    ledger_id, company_id
                )));
            }
        }
    }

    Ok(())
}

// ============================================================================
// Sprint 2 Stage 7: Voucher Search & Filter Validation Functions
// ============================================================================

/// Normalizes an exact monetary amount string for canonical SQLite string comparison and sorting.
///
/// Format: 25-digit zero-padded integer + '.' + 4-digit zero-padded fraction (exactly 30 characters).
/// Example: "100" -> "0000000000000000000000100.0000"
/// Example: "0.05" -> "0000000000000000000000000.0500"
///
/// Supports the full schema range: up to 25 integer digits and up to 4 fractional digits.
/// Uses exact string validation without floating-point arithmetic or Decimal 96-bit overflow limits.
pub fn normalize_canonical_amount_for_query(
    input: &str,
    field_name: &str,
) -> Result<String, OpenTallyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(format!(
            "{} cannot be empty",
            field_name
        )));
    }

    // Reject explicit signs ('+' or '-') to avoid normalization padding anomalies
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        return Err(OpenTallyError::Validation(format!(
            "{} cannot contain explicit sign: '{}'",
            field_name, input
        )));
    }

    // Split into integer and fractional components without heap allocation
    let (int_part, frac_part) = match trimmed.split_once('.') {
        Some((i, f)) => {
            if f.contains('.') {
                return Err(OpenTallyError::Validation(format!(
                    "{} contains multiple decimal points: '{}'",
                    field_name, input
                )));
            }
            if f.is_empty() {
                return Err(OpenTallyError::Validation(format!(
                    "{} has trailing decimal point without fractional digits: '{}'",
                    field_name, input
                )));
            }
            (i, f)
        }
        None => (trimmed, ""),
    };

    if int_part.is_empty() {
        return Err(OpenTallyError::Validation(format!(
            "{} missing integer part before decimal: '{}'",
            field_name, input
        )));
    }

    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part contains non-digit characters: '{}'",
            field_name, input
        )));
    }

    // Bound raw integer string length to prevent DoS via unbounded leading zeros
    if int_part.len() > 35 {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part length {} exceeds maximum allowed length of 35 characters: '{}'",
            field_name,
            int_part.len(),
            input
        )));
    }

    // Extract significant integer digits (handling leading zeros)
    let significant_int = int_part.trim_start_matches('0');
    let effective_int = if significant_int.is_empty() {
        "0"
    } else {
        significant_int
    };

    if effective_int.len() > 25 {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part exceeds maximum 25 digits: '{}'",
            field_name, input
        )));
    }

    if !frac_part.is_empty() {
        if !frac_part.chars().all(|c| c.is_ascii_digit()) {
            return Err(OpenTallyError::Validation(format!(
                "{} fractional part contains non-digit characters: '{}'",
                field_name, input
            )));
        }
        if frac_part.len() > 4 {
            return Err(OpenTallyError::Validation(format!(
                "{} exceeds maximum 4 decimal places: '{}'",
                field_name, input
            )));
        }
    }

    // Format: 25-digit zero-padded integer + '.' + 4-digit zero-padded fraction (exactly 30 characters)
    let padded_int = format!("{:0>25}", effective_int);
    let padded_frac = format!("{:0<4}", frac_part);

    Ok(format!("{}.{}", padded_int, padded_frac))
}

/// Validates a voucher search request according to the Sprint 2 Stage 7 domain specifications.
pub fn validate_search_filter(
    filter: &crate::vouchers::model::VoucherSearchFilter,
) -> Result<(), OpenTallyError> {
    // 1. Status validation
    if let Some(ref status) = filter.status {
        let s = status.trim().to_lowercase();
        match s.as_str() {
            "posted" | "reversed" | "cancelled" | "draft" | "all" => {}
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid voucher search status '{}'; must be one of 'posted', 'reversed', 'cancelled', 'draft', or 'all'",
                    other
                )));
            }
        }
    }

    // 2. Date parsing and range validation
    let date_from_parsed = if let Some(ref df) = filter.date_from {
        let trimmed = df.trim();
        if trimmed.is_empty() {
            return Err(OpenTallyError::Validation(
                "date_from cannot be empty string".to_string(),
            ));
        }
        let parsed = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|e| {
            OpenTallyError::Validation(format!(
                "Invalid date_from format '{}'; expected YYYY-MM-DD ({})",
                df, e
            ))
        })?;
        Some(parsed)
    } else {
        None
    };

    let date_to_parsed = if let Some(ref dt) = filter.date_to {
        let trimmed = dt.trim();
        if trimmed.is_empty() {
            return Err(OpenTallyError::Validation(
                "date_to cannot be empty string".to_string(),
            ));
        }
        let parsed = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|e| {
            OpenTallyError::Validation(format!(
                "Invalid date_to format '{}'; expected YYYY-MM-DD ({})",
                dt, e
            ))
        })?;
        Some(parsed)
    } else {
        None
    };

    if let (Some(df), Some(dt)) = (date_from_parsed, date_to_parsed) {
        if df > dt {
            return Err(OpenTallyError::Validation(format!(
                "date_from '{}' cannot be later than date_to '{}'",
                df, dt
            )));
        }
    }

    // 3. Amount parsing and range validation
    let min_canonical = if let Some(ref min_str) = filter.min_amount {
        Some(normalize_canonical_amount_for_query(min_str, "min_amount")?)
    } else {
        None
    };

    let max_canonical = if let Some(ref max_str) = filter.max_amount {
        Some(normalize_canonical_amount_for_query(max_str, "max_amount")?)
    } else {
        None
    };

    if let (Some(ref min_c), Some(ref max_c)) = (min_canonical, max_canonical) {
        if min_c > max_c {
            return Err(OpenTallyError::Validation(format!(
                "min_amount '{}' cannot be greater than max_amount '{}'",
                filter.min_amount.as_deref().unwrap_or(""),
                filter.max_amount.as_deref().unwrap_or("")
            )));
        }
    }

    // 4. Pagination validation & checked arithmetic
    if let Some(page) = filter.page {
        if page == 0 {
            return Err(OpenTallyError::Validation(
                "Page number must be greater than or equal to 1".to_string(),
            ));
        }
    }
    if let Some(page_size) = filter.page_size {
        if page_size == 0 {
            return Err(OpenTallyError::Validation(
                "Page size must be greater than or equal to 1".to_string(),
            ));
        }
        if page_size > 100 {
            return Err(OpenTallyError::Validation(format!(
                "Page size {} exceeds maximum permitted limit of 100",
                page_size
            )));
        }
    }
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    if (page - 1).checked_mul(page_size).is_none() {
        return Err(OpenTallyError::Validation(
            "Pagination offset calculation overflows u32 range".to_string(),
        ));
    }

    // 5. Identifier validation
    if let Some(ref id) = filter.voucher_type_id {
        if id.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "voucher_type_id cannot be empty or whitespace".to_string(),
            ));
        }
    }
    if let Some(ref id) = filter.series_id {
        if id.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "series_id cannot be empty or whitespace".to_string(),
            ));
        }
    }
    if let Some(ref id) = filter.ledger_id {
        if id.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "ledger_id cannot be empty or whitespace".to_string(),
            ));
        }
    }
    if let Some(ref id) = filter.reversal_of_voucher_id {
        if id.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "reversal_of_voucher_id cannot be empty or whitespace".to_string(),
            ));
        }
    }

    // 6. Text search query validation
    if let Some(ref sq) = filter.search_query {
        if sq.len() > 200 {
            return Err(OpenTallyError::Validation(
                "search_query cannot exceed 200 characters".to_string(),
            ));
        }
    }

    Ok(())
}
