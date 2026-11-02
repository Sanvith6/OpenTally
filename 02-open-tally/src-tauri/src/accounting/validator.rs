use crate::accounting::model::AccountGroup;
use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::str::FromStr;

/// Validates that an account group name is valid.
pub fn validate_group_name(name: &str) -> Result<String, OpenTallyError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(
            "Account group name cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 100 {
        return Err(OpenTallyError::Validation(
            "Account group name cannot exceed 100 characters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Detects if setting `target_parent_id` as the parent of `group_id` would form a circular hierarchy.
pub fn detect_group_cycle(
    groups: &[AccountGroup],
    group_id: &str,
    target_parent_id: &str,
) -> Result<(), OpenTallyError> {
    if group_id == target_parent_id {
        return Err(OpenTallyError::Validation(format!(
            "Circular hierarchy detected: group '{}' cannot be its own parent",
            group_id
        )));
    }

    let mut current_id = Some(target_parent_id.to_string());
    let mut visited = HashSet::new();
    visited.insert(target_parent_id.to_string());

    while let Some(curr) = current_id {
        if curr == group_id {
            let group_name = groups
                .iter()
                .find(|g| g.id == group_id)
                .map(|g| g.name.as_str())
                .unwrap_or(group_id);
            let target_name = groups
                .iter()
                .find(|g| g.id == target_parent_id)
                .map(|g| g.name.as_str())
                .unwrap_or(target_parent_id);
            return Err(OpenTallyError::Validation(format!(
                "Circular hierarchy detected: group '{}' is an ancestor of '{}'",
                group_name, target_name
            )));
        }

        if let Some(parent_group) = groups.iter().find(|g| g.id == curr) {
            if let Some(ref next_parent_id) = parent_group.parent_id {
                if !visited.insert(next_parent_id.clone()) {
                    break;
                }
                current_id = Some(next_parent_id.clone());
            } else {
                current_id = None;
            }
        } else {
            current_id = None;
        }
    }

    Ok(())
}

/// Validates that an account group retains the same nature when reparented.
pub fn validate_nature_match(
    parent_nature: &str,
    target_parent_nature: &str,
) -> Result<(), OpenTallyError> {
    if parent_nature != target_parent_nature {
        return Err(OpenTallyError::Validation(format!(
            "Cannot reparent group across differing natures: cannot move '{}' group under '{}' parent",
            parent_nature, target_parent_nature
        )));
    }
    Ok(())
}

/// Validates ledger name.
pub fn validate_ledger_name(name: &str) -> Result<String, OpenTallyError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(
            "Ledger name cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 100 {
        return Err(OpenTallyError::Validation(
            "Ledger name cannot exceed 100 characters".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Validates opening balance monetary amount against company currency precision without silent rounding.
pub fn validate_opening_balance_amount(
    amount_str: &str,
    currency_decimal_places: u8,
) -> Result<Decimal, OpenTallyError> {
    let trimmed = amount_str.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(
            "Opening balance amount cannot be empty".to_string(),
        ));
    }

    let dec = Decimal::from_str(trimmed).map_err(|e| {
        OpenTallyError::Validation(format!(
            "Invalid opening balance amount '{}': {}",
            trimmed, e
        ))
    })?;

    if dec < Decimal::ZERO {
        return Err(OpenTallyError::Validation(
            "Opening balance amount must be non-negative".to_string(),
        ));
    }

    // Normalize trailing fractional zeros (e.g. 100.2300 normalizes to 100.23)
    let normalized = dec.normalize();

    // Check scale without silent rounding
    if normalized.scale() > currency_decimal_places as u32 {
        return Err(OpenTallyError::Validation(format!(
            "Amount '{}' exceeds company currency precision of {} decimal places",
            trimmed, currency_decimal_places
        )));
    }

    Ok(normalized)
}
