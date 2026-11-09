use crate::errors::OpenTallyError;
use crate::vouchers::model::ParsedManualVoucherNumber;

/// Authoritative canonical voucher number formatter and inverse manual parser
pub struct CanonicalVoucherFormatter;

impl CanonicalVoucherFormatter {
    /// Resolves period-based dynamic format tokens strictly from period_start
    pub fn resolve_period_tokens(template: &str, period_start: &str) -> String {
        let parts: Vec<&str> = period_start.split('-').collect();
        if parts.len() != 3 {
            return template.to_string();
        }
        let ps_year: i32 = parts[0].parse().unwrap_or(2026);
        let ps_month: u32 = parts[1].parse().unwrap_or(4);
        let ps_day: u32 = parts[2].parse().unwrap_or(1);

        let fy_start_year = ps_year;
        let fy_end_year = fy_start_year + 1;
        let fy_token_yy = format!(
            "{:02}-{:02}",
            (fy_start_year % 100).abs(),
            (fy_end_year % 100).abs()
        );
        let fy_token_yyyy = format!("{:04}-{:04}", fy_start_year, fy_end_year);

        let mut result = template.to_string();
        result = result.replace("{YY-YY}", &fy_token_yy);
        result = result.replace("{YYYY-YYYY}", &fy_token_yyyy);
        result = result.replace("{YYYY}", &format!("{:04}", ps_year));
        result = result.replace("{YYYY-MM}", &format!("{:04}-{:02}", ps_year, ps_month));
        result = result.replace(
            "{YYYYMMDD}",
            &format!("{:04}{:02}{:02}", ps_year, ps_month, ps_day),
        );
        result = result.replace(
            "{YYYY-MM-DD}",
            &format!("{:04}-{:02}-{:02}", ps_year, ps_month, ps_day),
        );
        result
    }

    /// Authoritative formatting function for automatic vouchers
    pub fn format_automatic(
        _series_code: &str,
        prefix_template: &str,
        suffix_template: &str,
        sequence_number: i64,
        width: usize,
        period_start: &str,
    ) -> String {
        let num_str = format!("{:0width$}", sequence_number, width = width);
        let resolved_prefix = Self::resolve_period_tokens(prefix_template, period_start);
        let resolved_suffix = if suffix_template.is_empty() {
            String::new()
        } else {
            Self::resolve_period_tokens(suffix_template, period_start)
        };
        format!("{}{}{}", resolved_prefix, num_str, resolved_suffix)
    }

    /// Authoritative inverse parser and validator for user-entered manual voucher numbers
    #[allow(clippy::too_many_arguments)]
    pub fn parse_and_validate_manual_voucher_number(
        voucher_number: &str,
        series_code: &str,
        prefix_template: &str,
        suffix_template: &str,
        width: usize,
        _restart_period: &str,
        period_start: &str,
        all_series_codes: &[String],
    ) -> Result<ParsedManualVoucherNumber, OpenTallyError> {
        let expected_prefix = Self::resolve_period_tokens(prefix_template, period_start);
        let expected_suffix = if suffix_template.is_empty() {
            String::new()
        } else {
            Self::resolve_period_tokens(suffix_template, period_start)
        };

        // Reject namespace usurpation of any other series in the voucher type
        for other_code in all_series_codes {
            if other_code != series_code
                && (voucher_number.contains(&format!("/{}/", other_code))
                    || voucher_number.starts_with(&format!("{}/", other_code))
                    || voucher_number.contains(&format!("-{}-", other_code))
                    || voucher_number.starts_with(&format!("{}-", other_code)))
            {
                return Err(OpenTallyError::Validation(format!(
                    "Voucher number '{}' usurps series namespace '{}'",
                    voucher_number, other_code
                )));
            }
        }

        // Validate prefix match
        if !voucher_number.starts_with(&expected_prefix) {
            return Err(OpenTallyError::Validation(format!(
                "Manual voucher number '{}' does not match expected series prefix '{}'",
                voucher_number, expected_prefix
            )));
        }

        // Validate suffix match
        if !expected_suffix.is_empty() && !voucher_number.ends_with(&expected_suffix) {
            return Err(OpenTallyError::Validation(format!(
                "Manual voucher number '{}' does not match expected series suffix '{}'",
                voucher_number, expected_suffix
            )));
        }

        // Bounds guard: ensure voucher number is long enough to contain both non-overlapping prefix and suffix
        if voucher_number.len() < expected_prefix.len() + expected_suffix.len() {
            return Err(OpenTallyError::Validation(format!(
                "Manual voucher number '{}' is too short to contain both expected prefix '{}' and suffix '{}'",
                voucher_number, expected_prefix, expected_suffix
            )));
        }

        // Extract core component
        let core =
            &voucher_number[expected_prefix.len()..voucher_number.len() - expected_suffix.len()];
        if core.is_empty() {
            return Err(OpenTallyError::Validation(
                "Manual voucher number has empty numeric/identifier component".into(),
            ));
        }

        if core.chars().all(|c| c.is_ascii_digit()) {
            if width > 0 && core.len() < width {
                return Err(OpenTallyError::Validation(format!(
                    "Numeric component '{}' violates minimum zero-padding width of {}",
                    core, width
                )));
            }
            let numeric_val: i64 = core.parse().map_err(|_| {
                OpenTallyError::Validation(format!("Invalid integer numeric component '{}'", core))
            })?;
            if numeric_val <= 0 {
                return Err(OpenTallyError::Validation(format!(
                    "Numeric component '{}' must be strictly positive",
                    core
                )));
            }
            Ok(ParsedManualVoucherNumber {
                series_code: series_code.to_string(),
                prefix: expected_prefix,
                suffix: expected_suffix,
                sequence_number: Some(numeric_val),
                is_numeric: true,
                core: core.to_string(),
            })
        } else {
            // Alphanumeric identifier: must contain only ASCII alphanumeric, hyphens, or underscores
            if !core
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(OpenTallyError::Validation(format!(
                    "Identifier component '{}' contains invalid characters; must be alphanumeric, '-' or '_'",
                    core
                )));
            }
            Ok(ParsedManualVoucherNumber {
                series_code: series_code.to_string(),
                prefix: expected_prefix,
                suffix: expected_suffix,
                sequence_number: None,
                is_numeric: false,
                core: core.to_string(),
            })
        }
    }
}
