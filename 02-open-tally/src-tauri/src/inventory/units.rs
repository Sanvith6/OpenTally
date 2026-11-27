use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompoundUnitDto {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub symbol: String,
    pub unit_type: String,
    pub first_unit_id: Option<String>,
    pub second_unit_id: Option<String>,
    pub conversion_factor: Option<String>,
}

pub async fn create_compound_unit(
    pool: &SqlitePool,
    company_id: &str,
    name: &str,
    symbol: &str,
    first_unit_id: &str,
    second_unit_id: &str,
    conversion_factor: &str,
) -> Result<CompoundUnitDto, OpenTallyError> {
    let factor = Decimal::from_str(conversion_factor.trim()).map_err(|_| {
        OpenTallyError::Validation(format!("Invalid conversion factor '{}'", conversion_factor))
    })?;
    if factor <= Decimal::ZERO {
        return Err(OpenTallyError::Validation(
            "Conversion factor must be strictly positive".to_string(),
        ));
    }
    if first_unit_id == second_unit_id {
        return Err(OpenTallyError::Validation(
            "First and second units of a compound unit must differ".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let factor_str = format!("{}", factor.normalize());

    sqlx::query(
        "INSERT INTO units (id, company_id, formal_name, symbol, decimal_precision, unit_type, base_unit_id, secondary_unit_id, first_unit_id, second_unit_id, conversion_factor)
         VALUES (?, ?, ?, ?, 2, 'compound', ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(company_id)
    .bind(name.trim())
    .bind(symbol.trim())
    .bind(first_unit_id)
    .bind(second_unit_id)
    .bind(first_unit_id)
    .bind(second_unit_id)
    .bind(&factor_str)
    .execute(pool)
    .await
    .map_err(OpenTallyError::Database)?;

    Ok(CompoundUnitDto {
        id,
        company_id: company_id.to_string(),
        name: name.trim().to_string(),
        symbol: symbol.trim().to_string(),
        unit_type: "Compound".to_string(),
        first_unit_id: Some(first_unit_id.to_string()),
        second_unit_id: Some(second_unit_id.to_string()),
        conversion_factor: Some(factor_str),
    })
}

/// Converts an entered quantity in `entered_unit_id` into the stock item's base unit
/// using exact `rust_decimal` arithmetic. Rejects incompatible units.
pub async fn convert_to_base_quantity_tx(
    conn: &mut SqliteConnection,
    company_id: &str,
    stock_item_id: &str,
    entered_qty: Decimal,
    entered_unit_id: Option<&str>,
) -> Result<Decimal, OpenTallyError> {
    let entered_uid = match entered_unit_id {
        Some(u) if !u.trim().is_empty() => u.trim(),
        _ => return Ok(entered_qty),
    };

    #[derive(sqlx::FromRow)]
    struct ItemUnitRow {
        base_unit_id: String,
        alternate_unit_id: Option<String>,
        alternate_conversion_num: Option<String>,
        alternate_conversion_den: Option<String>,
    }

    let item = sqlx::query_as::<_, ItemUnitRow>(
        "SELECT base_unit_id, alternate_unit_id, alternate_conversion_num, alternate_conversion_den
         FROM stock_items WHERE company_id = ? AND id = ?",
    )
    .bind(company_id)
    .bind(stock_item_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?
    .ok_or_else(|| OpenTallyError::NotFound(format!("Stock item '{}' not found", stock_item_id)))?;

    if entered_uid == item.base_unit_id {
        return Ok(entered_qty);
    }

    // 1. Check item-level alternate unit conversion: num / den
    if item.alternate_unit_id.as_deref() == Some(entered_uid) {
        let num = item
            .alternate_conversion_num
            .as_deref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ONE);
        let den = item
            .alternate_conversion_den
            .as_deref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or(Decimal::ONE);
        if den <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Invalid alternate unit denominator".to_string(),
            ));
        }
        return Ok((entered_qty * num / den).round_dp(4));
    }

    // 2. Check compound unit: entered_uid is a Compound unit whose second_unit_id == base_unit_id
    //    or base_unit_id itself is Compound (first_unit_id == entered_uid or second_unit_id == entered_uid)
    #[derive(sqlx::FromRow)]
    struct UnitRow {
        unit_type: String,
        first_unit_id: Option<String>,
        second_unit_id: Option<String>,
        conversion_factor: Option<String>,
    }

    let entered_unit = sqlx::query_as::<_, UnitRow>(
        "SELECT COALESCE(unit_type, 'Simple') AS unit_type, first_unit_id, second_unit_id, conversion_factor
         FROM units WHERE company_id = ? AND id = ?",
    )
    .bind(company_id)
    .bind(entered_uid)
    .fetch_optional(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?;

    if let Some(u) = entered_unit {
        if u.unit_type.eq_ignore_ascii_case("compound")
            && u.second_unit_id.as_deref() == Some(&item.base_unit_id)
        {
            let factor = u
                .conversion_factor
                .as_deref()
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ONE);
            return Ok((entered_qty * factor).round_dp(4));
        }
    }

    // Also check if item.base_unit_id is Compound and entered_uid == first_unit_id or second_unit_id
    let base_unit = sqlx::query_as::<_, UnitRow>(
        "SELECT COALESCE(unit_type, 'Simple') AS unit_type, first_unit_id, second_unit_id, conversion_factor
         FROM units WHERE company_id = ? AND id = ?",
    )
    .bind(company_id)
    .bind(&item.base_unit_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?;

    if let Some(bu) = base_unit {
        if bu.unit_type.eq_ignore_ascii_case("compound") {
            let factor = bu
                .conversion_factor
                .as_deref()
                .and_then(|s| Decimal::from_str(s).ok())
                .unwrap_or(Decimal::ONE);
            if bu.first_unit_id.as_deref() == Some(entered_uid) {
                return Ok(entered_qty);
            }
            if bu.second_unit_id.as_deref() == Some(entered_uid) && factor > Decimal::ZERO {
                return Ok((entered_qty / factor).round_dp(4));
            }
        }
    }

    Err(OpenTallyError::Validation(format!(
        "Unit '{}' is incompatible with stock item base unit '{}'",
        entered_uid, item.base_unit_id
    )))
}
