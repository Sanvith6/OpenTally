use crate::errors::OpenTallyError;
use crate::reports::model::{ReportMovement, ReportOpeningBalance};
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceAccumulator {
    pub opening_debit: Decimal,
    pub opening_credit: Decimal,
    pub period_debit: Decimal,
    pub period_credit: Decimal,
}

impl Default for BalanceAccumulator {
    fn default() -> Self {
        Self {
            opening_debit: Decimal::ZERO,
            opening_credit: Decimal::ZERO,
            period_debit: Decimal::ZERO,
            period_credit: Decimal::ZERO,
        }
    }
}

impl BalanceAccumulator {
    pub fn apply_opening(&mut self, opening: &ReportOpeningBalance) -> Result<(), OpenTallyError> {
        let amount = parse_report_amount(&opening.amount)?;
        match opening.balance_type.as_str() {
            "Dr" => self.opening_debit += amount,
            "Cr" => self.opening_credit += amount,
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid opening balance type '{}'; expected Dr or Cr",
                    other
                )))
            }
        }
        Ok(())
    }

    pub fn apply_movement(&mut self, movement: &ReportMovement) -> Result<(), OpenTallyError> {
        let amount = parse_report_amount(&movement.amount)?;
        match movement.entry_type.as_str() {
            "Dr" => self.period_debit += amount,
            "Cr" => self.period_credit += amount,
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid report entry type '{}'; expected Dr or Cr",
                    other
                )))
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculatedBalance {
    pub opening_debit: Decimal,
    pub opening_credit: Decimal,
    pub period_debit: Decimal,
    pub period_credit: Decimal,
    pub closing_debit: Decimal,
    pub closing_credit: Decimal,
}

pub fn calculate_balance(
    opening: &[ReportOpeningBalance],
    movements: &[ReportMovement],
) -> Result<CalculatedBalance, OpenTallyError> {
    let mut accumulator = BalanceAccumulator::default();
    for item in opening {
        accumulator.apply_opening(item)?;
    }
    for movement in movements {
        accumulator.apply_movement(movement)?;
    }

    let net = accumulator.opening_debit + accumulator.period_debit
        - accumulator.opening_credit
        - accumulator.period_credit;
    let (closing_debit, closing_credit) = if net >= Decimal::ZERO {
        (net, Decimal::ZERO)
    } else {
        (Decimal::ZERO, -net)
    };

    Ok(CalculatedBalance {
        opening_debit: accumulator.opening_debit,
        opening_credit: accumulator.opening_credit,
        period_debit: accumulator.period_debit,
        period_credit: accumulator.period_credit,
        closing_debit,
        closing_credit,
    })
}

pub fn parse_report_amount(value: &str) -> Result<Decimal, OpenTallyError> {
    Decimal::from_str(value).map_err(|_| {
        OpenTallyError::Validation(format!("Invalid canonical report amount '{}'", value))
    })
}
