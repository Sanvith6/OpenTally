use crate::vouchers::model::{
    DoubleEntryLine, DoubleEntryValidationResult, ValidationDiagnostic, ValidationMode,
    ValidationSeverity,
};
use crate::vouchers::validator::validate_canonical_amount;
use rust_decimal::Decimal;

/// Pure, deterministic double-entry validation engine.
/// Operates strictly in-memory without database access or floating-point arithmetic.
pub struct DoubleEntryValidator;

impl DoubleEntryValidator {
    /// Validates a collection of voucher lines enforcing double-entry arithmetic invariants.
    pub fn validate<L: DoubleEntryLine>(
        lines: &[L],
        currency_decimal_places: u32,
        mode: ValidationMode,
    ) -> DoubleEntryValidationResult {
        let mut diagnostics = Vec::new();
        let mut debit_total = Decimal::ZERO;
        let mut credit_total = Decimal::ZERO;
        let mut debit_line_count = 0;
        let mut credit_line_count = 0;
        let mut has_structural_error = false;

        // 1. Cardinality check for empty vouchers
        if lines.is_empty() {
            let severity = match mode {
                ValidationMode::Draft => ValidationSeverity::Warning,
                ValidationMode::Posting => ValidationSeverity::Error,
            };
            diagnostics.push(ValidationDiagnostic {
                code: "EMPTY_VOUCHER".to_string(),
                severity,
                message: "Voucher contains no line items; at least one Debit and one Credit leg are required".to_string(),
                line_number: None,
                ledger_id: None,
                field: Some("lines".to_string()),
            });
            if mode == ValidationMode::Posting {
                has_structural_error = true;
            }
        }

        // 2. Individual Line Validation
        for (idx, line) in lines.iter().enumerate() {
            let line_no = line.line_number().unwrap_or((idx + 1) as i64);
            let entry_type = line.entry_type().trim();
            let amount_str = line.amount_str().trim();
            let ledger_id = line.ledger_id().trim();

            if ledger_id.is_empty() {
                diagnostics.push(ValidationDiagnostic {
                    code: "MISSING_LEDGER".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!("Line {} is missing a valid ledger reference", line_no),
                    line_number: Some(line_no),
                    ledger_id: None,
                    field: Some("ledger_id".to_string()),
                });
                has_structural_error = true;
            }

            // Entry Type check: strict PascalCase ("Dr" or "Cr") matching SQLite CHECK constraint
            let is_dr = entry_type == "Dr";
            let is_cr = entry_type == "Cr";
            if !is_dr && !is_cr {
                diagnostics.push(ValidationDiagnostic {
                    code: "INVALID_ENTRY_TYPE".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!(
                        "Line {} has invalid entry_type '{}'; must be 'Dr' or 'Cr'",
                        line_no, entry_type
                    ),
                    line_number: Some(line_no),
                    ledger_id: Some(ledger_id.to_string()),
                    field: Some("entry_type".to_string()),
                });
                has_structural_error = true;
                continue;
            }

            // Check for negative sign
            if amount_str.starts_with('-') {
                diagnostics.push(ValidationDiagnostic {
                    code: "NEGATIVE_AMOUNT".to_string(),
                    severity: ValidationSeverity::Error,
                    message: format!(
                        "Line {} amount '{}' is negative; amounts must be unsigned positive decimals",
                        line_no, amount_str
                    ),
                    line_number: Some(line_no),
                    ledger_id: Some(ledger_id.to_string()),
                    field: Some("amount".to_string()),
                });
                has_structural_error = true;
                continue;
            }

            // Parse and validate canonical amount
            match validate_canonical_amount(amount_str, currency_decimal_places) {
                Ok(amount) => {
                    if is_dr {
                        match debit_total.checked_add(amount) {
                            Some(sum) => {
                                debit_total = sum;
                                debit_line_count += 1;
                            }
                            None => {
                                diagnostics.push(ValidationDiagnostic {
                                    code: "ARITHMETIC_OVERFLOW".to_string(),
                                    severity: ValidationSeverity::Error,
                                    message: format!(
                                        "Line {} causes total debit to exceed maximum decimal capacity",
                                        line_no
                                    ),
                                    line_number: Some(line_no),
                                    ledger_id: Some(ledger_id.to_string()),
                                    field: Some("amount".to_string()),
                                });
                                has_structural_error = true;
                            }
                        }
                    } else {
                        match credit_total.checked_add(amount) {
                            Some(sum) => {
                                credit_total = sum;
                                credit_line_count += 1;
                            }
                            None => {
                                diagnostics.push(ValidationDiagnostic {
                                    code: "ARITHMETIC_OVERFLOW".to_string(),
                                    severity: ValidationSeverity::Error,
                                    message: format!(
                                        "Line {} causes total credit to exceed maximum decimal capacity",
                                        line_no
                                    ),
                                    line_number: Some(line_no),
                                    ledger_id: Some(ledger_id.to_string()),
                                    field: Some("amount".to_string()),
                                });
                                has_structural_error = true;
                            }
                        }
                    }
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let code = if err_msg.contains("cannot be zero")
                        || err_msg.contains("strictly positive")
                    {
                        "ZERO_AMOUNT"
                    } else if err_msg.contains("exceeding company currency precision")
                        || err_msg.contains("exceeds currency decimal places")
                    {
                        "SCALE_EXCEEDS_PRECISION"
                    } else {
                        "INVALID_MONETARY_STRING"
                    };

                    diagnostics.push(ValidationDiagnostic {
                        code: code.to_string(),
                        severity: ValidationSeverity::Error,
                        message: format!("Line {} amount error: {}", line_no, err_msg),
                        line_number: Some(line_no),
                        ledger_id: Some(ledger_id.to_string()),
                        field: Some("amount".to_string()),
                    });
                    has_structural_error = true;
                }
            }
        }

        // 3. Multi-leg Cardinality & Sidedness Check
        if !lines.is_empty() {
            if debit_line_count == 0 && credit_line_count > 0 {
                let severity = match mode {
                    ValidationMode::Draft => ValidationSeverity::Warning,
                    ValidationMode::Posting => ValidationSeverity::Error,
                };
                diagnostics.push(ValidationDiagnostic {
                    code: "SINGLE_SIDED_VOUCHER".to_string(),
                    severity,
                    message:
                        "Voucher contains Credit legs only; at least one Debit leg is required"
                            .to_string(),
                    line_number: None,
                    ledger_id: None,
                    field: Some("lines".to_string()),
                });
                if mode == ValidationMode::Posting {
                    has_structural_error = true;
                }
            } else if credit_line_count == 0 && debit_line_count > 0 {
                let severity = match mode {
                    ValidationMode::Draft => ValidationSeverity::Warning,
                    ValidationMode::Posting => ValidationSeverity::Error,
                };
                diagnostics.push(ValidationDiagnostic {
                    code: "SINGLE_SIDED_VOUCHER".to_string(),
                    severity,
                    message:
                        "Voucher contains Debit legs only; at least one Credit leg is required"
                            .to_string(),
                    line_number: None,
                    ledger_id: None,
                    field: Some("lines".to_string()),
                });
                if mode == ValidationMode::Posting {
                    has_structural_error = true;
                }
            } else if lines.len() < 2 {
                let severity = match mode {
                    ValidationMode::Draft => ValidationSeverity::Warning,
                    ValidationMode::Posting => ValidationSeverity::Error,
                };
                diagnostics.push(ValidationDiagnostic {
                    code: "INSUFFICIENT_LEGS".to_string(),
                    severity,
                    message: "Voucher must contain at least 2 line items (at least one Debit and one Credit)"
                        .to_string(),
                    line_number: None,
                    ledger_id: None,
                    field: Some("lines".to_string()),
                });
                if mode == ValidationMode::Posting {
                    has_structural_error = true;
                }
            }
        }

        // 4. Equilibrium Balance Check
        let difference = if debit_total >= credit_total {
            debit_total - credit_total
        } else {
            credit_total - debit_total
        };

        let is_balanced = !has_structural_error
            && lines.len() >= 2
            && debit_line_count >= 1
            && credit_line_count >= 1
            && debit_total > Decimal::ZERO
            && debit_total == credit_total;

        if !lines.is_empty() && debit_total != credit_total {
            let severity = match mode {
                ValidationMode::Draft => ValidationSeverity::Warning,
                ValidationMode::Posting => ValidationSeverity::Error,
            };
            diagnostics.push(ValidationDiagnostic {
                code: "UNBALANCED_TOTALS".to_string(),
                severity,
                message: format!(
                    "Voucher is out of balance: Total Debit ({}) != Total Credit ({}); Difference: {}",
                    debit_total, credit_total, difference
                ),
                line_number: None,
                ledger_id: None,
                field: Some("amount".to_string()),
            });
        }

        let is_valid = match mode {
            ValidationMode::Draft => !has_structural_error,
            ValidationMode::Posting => !has_structural_error && is_balanced,
        };

        let single_side_total = if is_balanced { Some(debit_total) } else { None };

        DoubleEntryValidationResult {
            is_valid,
            is_balanced,
            line_count: lines.len(),
            debit_line_count,
            credit_line_count,
            debit_total,
            credit_total,
            difference,
            single_side_total,
            diagnostics,
        }
    }
}
