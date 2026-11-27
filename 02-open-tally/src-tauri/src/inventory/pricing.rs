use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PriceLevelDto {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub is_active: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PriceListEntryDto {
    pub id: String,
    pub company_id: String,
    pub price_level_id: String,
    pub stock_item_id: String,
    pub effective_from: String,
    pub min_qty: String,
    pub max_qty: Option<String>,
    pub unit_rate: String,
    pub discount_pct: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedItemPriceDto {
    pub unit_rate: String,
    pub discount_pct: String,
    pub matched_price_level: Option<String>,
}

pub async fn list_price_levels(
    pool: &SqlitePool,
    company_id: &str,
) -> Result<Vec<PriceLevelDto>, OpenTallyError> {
    let cid = company_id.to_string();
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            sqlx::query_as::<_, PriceLevelDto>(
                "SELECT id, company_id, name, is_active, created_at
                 FROM price_levels WHERE company_id = ? ORDER BY name ASC",
            )
            .bind(&cid)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)
        })
    })
    .await
}

pub async fn create_price_level(
    pool: &SqlitePool,
    company_id: &str,
    name: &str,
) -> Result<PriceLevelDto, OpenTallyError> {
    let clean = name.trim();
    if clean.is_empty() {
        return Err(OpenTallyError::Validation(
            "Price level name cannot be empty".to_string(),
        ));
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO price_levels (id, company_id, name, is_active) VALUES (?, ?, ?, 1)")
        .bind(&id)
        .bind(company_id)
        .bind(clean)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

    sqlx::query_as::<_, PriceLevelDto>(
        "SELECT id, company_id, name, is_active, created_at FROM price_levels WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(OpenTallyError::Database)
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_price_list_entry(
    pool: &SqlitePool,
    company_id: &str,
    price_level_id: &str,
    stock_item_id: &str,
    effective_from: &str,
    min_qty: &str,
    max_qty: Option<&str>,
    unit_rate: &str,
    discount_pct: &str,
) -> Result<PriceListEntryDto, OpenTallyError> {
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO price_list_entries (id, company_id, price_level_id, stock_item_id, effective_from, min_qty, max_qty, unit_rate, discount_pct)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(company_id)
    .bind(price_level_id)
    .bind(stock_item_id)
    .bind(effective_from.trim())
    .bind(min_qty.trim())
    .bind(max_qty.map(|s| s.trim().to_string()))
    .bind(unit_rate.trim())
    .bind(discount_pct.trim())
    .execute(pool)
    .await
    .map_err(OpenTallyError::Database)?;

    sqlx::query_as::<_, PriceListEntryDto>(
        "SELECT id, company_id, price_level_id, stock_item_id, effective_from, min_qty, max_qty, unit_rate, discount_pct
         FROM price_list_entries WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(OpenTallyError::Database)
}

pub async fn list_price_list_entries(
    pool: &SqlitePool,
    company_id: &str,
    price_level_id: Option<&str>,
) -> Result<Vec<PriceListEntryDto>, OpenTallyError> {
    let cid = company_id.to_string();
    let pl_id = price_level_id.map(|s| s.to_string());
    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            if let Some(pid) = pl_id {
                sqlx::query_as::<_, PriceListEntryDto>(
                    "SELECT id, company_id, price_level_id, stock_item_id, effective_from, min_qty, max_qty, unit_rate, discount_pct
                     FROM price_list_entries WHERE company_id = ? AND price_level_id = ?
                     ORDER BY effective_from DESC, min_qty ASC",
                )
                .bind(&cid)
                .bind(&pid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
            } else {
                sqlx::query_as::<_, PriceListEntryDto>(
                    "SELECT id, company_id, price_level_id, stock_item_id, effective_from, min_qty, max_qty, unit_rate, discount_pct
                     FROM price_list_entries WHERE company_id = ?
                     ORDER BY effective_from DESC, min_qty ASC",
                )
                .bind(&cid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
            }
        })
    })
    .await
}

pub async fn resolve_item_price(
    pool: &SqlitePool,
    company_id: &str,
    stock_item_id: &str,
    party_ledger_id: Option<&str>,
    price_level_id: Option<&str>,
    quantity: &str,
    as_of_date: &str,
) -> Result<ResolvedItemPriceDto, OpenTallyError> {
    let cid = company_id.to_string();
    let item_id = stock_item_id.to_string();
    let party_id = party_ledger_id.map(|s| s.to_string());
    let explicit_pl = price_level_id.map(|s| s.to_string());
    let qty_dec = Decimal::from_str(quantity.trim()).unwrap_or(Decimal::ONE);
    let as_of = as_of_date.trim().to_string();

    execute_read_only_query(pool, move |conn| {
        Box::pin(async move {
            let resolved_pl_id: Option<String> = if let Some(pl) = explicit_pl {
                Some(pl)
            } else if let Some(pid) = party_id {
                sqlx::query_scalar(
                    "SELECT price_level_id FROM ledgers WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&pid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .flatten()
            } else {
                None
            };

            if let Some(pl_id) = resolved_pl_id {
                let pl_name: Option<String> = sqlx::query_scalar(
                    "SELECT name FROM price_levels WHERE company_id = ? AND id = ? AND is_active = 1",
                )
                .bind(&cid)
                .bind(&pl_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let entries = sqlx::query_as::<_, PriceListEntryDto>(
                    "SELECT id, company_id, price_level_id, stock_item_id, effective_from, min_qty, max_qty, unit_rate, discount_pct
                     FROM price_list_entries
                     WHERE company_id = ? AND price_level_id = ? AND stock_item_id = ? AND effective_from <= ?
                     ORDER BY effective_from DESC",
                )
                .bind(&cid)
                .bind(&pl_id)
                .bind(&item_id)
                .bind(&as_of)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                for e in entries {
                    let min_q = Decimal::from_str(&e.min_qty).unwrap_or(Decimal::ZERO);
                    let max_ok = match e.max_qty.as_deref() {
                        Some(m) if !m.trim().is_empty() => {
                            let max_q = Decimal::from_str(m).unwrap_or(Decimal::MAX);
                            qty_dec <= max_q
                        }
                        _ => true,
                    };
                    if qty_dec >= min_q && max_ok {
                        return Ok(ResolvedItemPriceDto {
                            unit_rate: e.unit_rate,
                            discount_pct: e.discount_pct,
                            matched_price_level: pl_name,
                        });
                    }
                }
            }

            Ok(ResolvedItemPriceDto {
                unit_rate: "0.00".to_string(),
                discount_pct: "0.00".to_string(),
                matched_price_level: None,
            })
        })
    })
    .await
}
