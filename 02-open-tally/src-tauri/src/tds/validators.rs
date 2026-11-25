use crate::errors::OpenTallyError;

/// Validates an Indian Tax Deduction and Collection Account Number (TAN).
/// Format: 4 uppercase alphabets, 5 numeric digits, 1 uppercase alphabet (e.g. BLRP12345A).
pub fn validate_tan(tan: &str) -> Result<(), OpenTallyError> {
    let clean = tan.trim().to_uppercase();
    if clean.len() != 10 {
        return Err(OpenTallyError::Validation(format!(
            "Invalid TAN '{}': TAN must be exactly 10 characters",
            tan
        )));
    }

    let bytes = clean.as_bytes();
    for (i, &b) in bytes.iter().enumerate().take(4) {
        if !b.is_ascii_uppercase() {
            return Err(OpenTallyError::Validation(format!(
                "Invalid TAN '{}': character at position {} must be an uppercase letter (A-Z)",
                tan,
                i + 1
            )));
        }
    }

    for (i, &b) in bytes.iter().enumerate().take(9).skip(4) {
        if !b.is_ascii_digit() {
            return Err(OpenTallyError::Validation(format!(
                "Invalid TAN '{}': character at position {} must be a numeric digit (0-9)",
                tan,
                i + 1
            )));
        }
    }

    if !bytes[9].is_ascii_uppercase() {
        return Err(OpenTallyError::Validation(format!(
            "Invalid TAN '{}': 10th character must be an uppercase letter (A-Z)",
            tan
        )));
    }

    Ok(())
}

/// Validates an Indian Permanent Account Number (PAN).
/// Format: 5 uppercase alphabets, 4 numeric digits, 1 uppercase alphabet (e.g. ABCDE1234F).
/// The 4th character must designate a valid taxpayer entity:
/// - P: Individual
/// - C: Company
/// - H: Hindu Undivided Family (HUF)
/// - F: Partnership Firm / LLP
/// - A: Association of Persons (AOP)
/// - T: Trust
/// - B: Body of Individuals (BOI)
/// - L: Local Authority
/// - J: Artificial Juridical Person
/// - G: Government
pub fn validate_pan(pan: &str) -> Result<(), OpenTallyError> {
    let clean = pan.trim().to_uppercase();
    if clean.len() != 10 {
        return Err(OpenTallyError::Validation(format!(
            "Invalid PAN '{}': PAN must be exactly 10 characters",
            pan
        )));
    }

    let bytes = clean.as_bytes();
    for (i, &b) in bytes.iter().enumerate().take(5) {
        if !b.is_ascii_uppercase() {
            return Err(OpenTallyError::Validation(format!(
                "Invalid PAN '{}': character at position {} must be an uppercase letter (A-Z)",
                pan,
                i + 1
            )));
        }
    }

    let allowed_entity_types = ['P', 'C', 'H', 'F', 'A', 'T', 'B', 'L', 'J', 'G'];
    let fourth_char = bytes[3] as char;
    if !allowed_entity_types.contains(&fourth_char) {
        return Err(OpenTallyError::Validation(format!(
            "Invalid PAN '{}': 4th character '{}' is not a valid taxpayer entity type (must be one of P, C, H, F, A, T, B, L, J, G)",
            pan, fourth_char
        )));
    }

    for (i, &b) in bytes.iter().enumerate().take(9).skip(5) {
        if !b.is_ascii_digit() {
            return Err(OpenTallyError::Validation(format!(
                "Invalid PAN '{}': character at position {} must be a numeric digit (0-9)",
                pan,
                i + 1
            )));
        }
    }

    if !bytes[9].is_ascii_uppercase() {
        return Err(OpenTallyError::Validation(format!(
            "Invalid PAN '{}': 10th character must be an uppercase letter (A-Z)",
            pan
        )));
    }

    Ok(())
}
