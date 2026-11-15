use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ValuationConfig {
    pub company_id: String,
    pub valuation_method: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuationRequest {
    pub as_of_date: String,
    pub stock_item_id: Option<String>,
    pub stock_group_id: Option<String>,
    pub stock_category_id: Option<String>,
    pub godown_id: Option<String>,
    pub search: Option<String>,
    pub include_zero: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuationRow {
    pub stock_item_id: String,
    pub item_name: String,
    pub godown_id: Option<String>,
    pub godown_name: Option<String>,
    pub quantity: String,
    pub valuation_method: String,
    pub unit_cost: String,
    pub total_value: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FifoLayer {
    pub id: String,
    pub stock_item_id: String,
    pub godown_id: String,
    pub source_voucher_id: Option<String>,
    pub source_line_id: Option<String>,
    pub receipt_date: String,
    pub original_quantity: String,
    pub remaining_quantity: String,
    pub unit_cost: String,
    pub original_value: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValuationHistoryRow {
    pub movement_id: String,
    pub movement_date: String,
    pub movement_type: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub quantity: String,
    pub unit_cost: String,
    pub value: String,
}

#[derive(Clone)]
struct Layer {
    qty: Decimal,
    cost: Decimal,
}
#[derive(sqlx::FromRow)]
struct Event {
    movement_id: String,
    item: String,
    voucher_id: String,
    voucher_number: String,
    date: String,
    kind: String,
    qty: String,
    cost: Option<String>,
    reversal_of: Option<String>,
}

fn d(value: &str, field: &str) -> Result<Decimal, OpenTallyError> {
    value
        .parse()
        .map_err(|_| OpenTallyError::IntegrityViolation(format!("Invalid stored {}", field)))
}
fn date(value: &str) -> Result<(), OpenTallyError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| OpenTallyError::Validation("As-of date must be YYYY-MM-DD".into()))
}
fn fmt(value: Decimal) -> String {
    value.normalize().to_string()
}

pub struct ValuationService;
impl ValuationService {
    pub async fn get_config(
        pool: &SqlitePool,
        company: &str,
    ) -> Result<ValuationConfig, OpenTallyError> {
        Ok(sqlx::query_as(
            "SELECT company_id,valuation_method FROM inventory_valuation_config WHERE company_id=?",
        )
        .bind(company)
        .fetch_optional(pool)
        .await?
        .unwrap_or(ValuationConfig {
            company_id: company.into(),
            valuation_method: "fifo".into(),
        }))
    }
    pub async fn set_config(
        pool: &SqlitePool,
        company: &str,
        method: &str,
    ) -> Result<ValuationConfig, OpenTallyError> {
        if !["fifo", "weighted_average"].contains(&method) {
            return Err(OpenTallyError::Validation(
                "Valuation method must be fifo or weighted_average".into(),
            ));
        }
        let mut tx = pool.begin().await?;
        let history: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stock_voucher_lines l JOIN stock_vouchers v ON v.company_id=l.company_id AND v.id=l.voucher_id WHERE l.company_id=? AND v.status='posted'").bind(company).fetch_one(&mut *tx).await?;
        let old: Option<String> = sqlx::query_scalar(
            "SELECT valuation_method FROM inventory_valuation_config WHERE company_id=?",
        )
        .bind(company)
        .fetch_optional(&mut *tx)
        .await?;
        if history > 0 && old.as_deref() != Some(method) {
            return Err(OpenTallyError::Validation(
                "Valuation method cannot change after posted valuation history exists".into(),
            ));
        }
        sqlx::query("INSERT INTO inventory_valuation_config(company_id,valuation_method) VALUES(?,?) ON CONFLICT(company_id) DO UPDATE SET valuation_method=excluded.valuation_method,updated_at=CURRENT_TIMESTAMP").bind(company).bind(method).execute(&mut *tx).await?;
        tx.commit().await?;
        Self::get_config(pool, company).await
    }
    async fn events(
        pool: &SqlitePool,
        company: &str,
        request: &ValuationRequest,
    ) -> Result<Vec<Event>, OpenTallyError> {
        date(&request.as_of_date)?;
        let search = request.search.as_ref().map(|x| format!("%{}%", x));
        Ok(sqlx::query_as("SELECT m.id movement_id,m.stock_item_id item,m.voucher_id,v.voucher_number,m.movement_date date,m.movement_type kind,m.signed_quantity qty,l.unit_cost cost,v.reversal_of_voucher_id reversal_of FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id JOIN stock_items i ON i.company_id=m.company_id AND i.id=m.stock_item_id WHERE m.company_id=? AND (v.status='posted' OR v.status='reversed') AND m.movement_date<=? AND (? IS NULL OR m.stock_item_id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?) AND (? IS NULL OR i.name LIKE ? COLLATE NOCASE OR i.sku LIKE ? COLLATE NOCASE) AND (? IS NULL OR m.godown_id=?) ORDER BY m.movement_date,CASE WHEN v.reversal_of_voucher_id IS NULL THEN 0 ELSE 1 END,v.voucher_number,m.id")
            .bind(company).bind(&request.as_of_date).bind(&request.stock_item_id).bind(&request.stock_item_id).bind(&request.stock_group_id).bind(&request.stock_group_id).bind(&request.stock_category_id).bind(&request.stock_category_id).bind(search.as_deref()).bind(search.as_deref()).bind(&request.godown_id).bind(&request.godown_id).fetch_all(pool).await?)
    }
    pub async fn summary(
        pool: &SqlitePool,
        company: &str,
        request: ValuationRequest,
    ) -> Result<Vec<ValuationRow>, OpenTallyError> {
        let config = Self::get_config(pool, company).await?;
        let events = Self::events(pool, company, &request).await?;
        let mut layers: BTreeMap<String, Vec<Layer>> = BTreeMap::new();
        let mut opening: BTreeMap<String, (Decimal, Decimal, Decimal)> = BTreeMap::new();
        let items: Vec<(String,String,String,String,String,String)> = sqlx::query_as("SELECT i.id,i.name,i.opening_quantity,i.opening_value,i.opening_rate,COALESCE(i.base_unit_id,'') FROM stock_items i WHERE i.company_id=? AND (? IS NULL OR i.id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?)")
            .bind(company).bind(&request.stock_item_id).bind(&request.stock_item_id).bind(&request.stock_group_id).bind(&request.stock_group_id).bind(&request.stock_category_id).bind(&request.stock_category_id).fetch_all(pool).await?;
        for (id, _name, q, value, rate, _) in items {
            let qty = d(&q, "opening quantity")?;
            let val = d(&value, "opening value")?;
            let cost = if qty.is_zero() {
                Decimal::ZERO
            } else {
                val / qty
            };
            if !qty.is_zero() && val.is_zero() {
                let _ = d(&rate, "opening rate")?;
            }
            opening.insert(id, (qty, val, cost));
        }
        for (id, (qty, _value, cost)) in &opening {
            if !qty.is_zero() {
                layers.insert(
                    id.clone(),
                    vec![Layer {
                        qty: *qty,
                        cost: *cost,
                    }],
                );
            }
        }
        let mut weighted: BTreeMap<String, (Decimal, Decimal)> = opening
            .iter()
            .map(|(id, (qty, value, _))| (id.clone(), (*qty, *value)))
            .collect();
        let mut consumed: BTreeMap<String, Vec<(Decimal, Decimal)>> = BTreeMap::new();
        for event in events {
            let quantity = d(&event.qty, "movement quantity")?;
            let entry = layers.entry(event.item.clone()).or_default();
            let weighted_entry = weighted.entry(event.item.clone()).or_default();
            match event.kind.as_str() {
                "receipt" => {
                    let cost = event
                        .cost
                        .as_deref()
                        .map(|x| d(x, "unit cost"))
                        .transpose()?
                        .unwrap_or(Decimal::ZERO);
                    let q = quantity.abs();
                    if let Some(original) = &event.reversal_of {
                        if let Some(restored) = consumed.get(original) {
                            for (restored_qty, restored_cost) in restored {
                                entry.push(Layer {
                                    qty: *restored_qty,
                                    cost: *restored_cost,
                                });
                                weighted_entry.0 += *restored_qty;
                                weighted_entry.1 += *restored_qty * *restored_cost;
                            }
                        }
                    } else if !q.is_zero() {
                        entry.push(Layer { qty: q, cost });
                        weighted_entry.0 += q;
                        weighted_entry.1 += q * cost;
                    }
                }
                "delivery" => {
                    let mut need = quantity.abs();
                    let mut consumed_for_voucher = Vec::new();
                    for layer in entry.iter_mut().filter(|x| x.qty > Decimal::ZERO) {
                        if need.is_zero() {
                            break;
                        }
                        let take = layer.qty.min(need);
                        layer.qty -= take;
                        need -= take;
                        consumed_for_voucher.push((take, layer.cost));
                    }
                    if !need.is_zero() {
                        return Err(OpenTallyError::Validation(
                            "Insufficient valued stock".into(),
                        ));
                    }
                    consumed.insert(event.voucher_id.clone(), consumed_for_voucher);
                    let average = if weighted_entry.0.is_zero() {
                        Decimal::ZERO
                    } else {
                        weighted_entry.1 / weighted_entry.0
                    };
                    weighted_entry.1 -= quantity.abs() * average;
                    weighted_entry.0 -= quantity.abs();
                }
                "adjustment" => {
                    if quantity > Decimal::ZERO {
                        let cost = entry
                            .iter()
                            .find(|x| x.qty > Decimal::ZERO)
                            .map(|x| x.cost)
                            .unwrap_or(Decimal::ZERO);
                        entry.push(Layer {
                            qty: quantity,
                            cost,
                        });
                        weighted_entry.1 += quantity * cost;
                        weighted_entry.0 += quantity;
                    } else {
                        let mut need = -quantity;
                        for layer in entry.iter_mut().filter(|x| x.qty > Decimal::ZERO) {
                            let take = layer.qty.min(need);
                            layer.qty -= take;
                            need -= take;
                            if need.is_zero() {
                                break;
                            }
                        }
                        if !need.is_zero() {
                            return Err(OpenTallyError::Validation(
                                "Insufficient valued stock".into(),
                            ));
                        }
                        let average = if weighted_entry.0.is_zero() {
                            Decimal::ZERO
                        } else {
                            weighted_entry.1 / weighted_entry.0
                        };
                        weighted_entry.1 -= (-quantity) * average;
                        weighted_entry.0 += quantity;
                    }
                }
                "transfer_in" | "transfer_out" => {}
                _ => {}
            }
        }
        let mut out = Vec::new();
        let names: Vec<(String, String)> =
            sqlx::query_as("SELECT id,name FROM stock_items WHERE company_id=?")
                .bind(company)
                .fetch_all(pool)
                .await?;
        for (id, name) in names {
            let ls = layers.remove(&id).unwrap_or_default();
            let qty: Decimal = ls.iter().map(|x| x.qty).sum();
            let value: Decimal = if config.valuation_method == "weighted_average" {
                weighted
                    .get(&id)
                    .map(|(_, value)| *value)
                    .unwrap_or(Decimal::ZERO)
            } else {
                ls.iter().map(|x| x.qty * x.cost).sum()
            };
            if request.include_zero || !qty.is_zero() {
                out.push(ValuationRow {
                    stock_item_id: id,
                    item_name: name,
                    godown_id: None,
                    godown_name: None,
                    quantity: fmt(qty),
                    valuation_method: config.valuation_method.clone(),
                    unit_cost: fmt(if qty.is_zero() {
                        Decimal::ZERO
                    } else {
                        value / qty
                    }),
                    total_value: fmt(value),
                });
            }
        }
        Ok(out)
    }
    pub async fn fifo_layers(
        pool: &SqlitePool,
        company: &str,
        request: ValuationRequest,
    ) -> Result<Vec<FifoLayer>, OpenTallyError> {
        let events = Self::events(pool, company, &request).await?;
        let mut layers: Vec<FifoLayer> = Vec::new();
        let mut consumed: BTreeMap<String, Vec<(Decimal, Decimal)>> = BTreeMap::new();
        let openings: Vec<(String, String, String)> = sqlx::query_as("SELECT id,opening_quantity,opening_value FROM stock_items WHERE company_id=? AND (? IS NULL OR id=?)")
            .bind(company).bind(&request.stock_item_id).bind(&request.stock_item_id).fetch_all(pool).await?;
        for (item, quantity, value) in openings {
            let quantity = d(&quantity, "opening quantity")?;
            if !quantity.is_zero() {
                let value = d(&value, "opening value")?;
                layers.push(FifoLayer {
                    id: format!("OPENING-{}", item),
                    stock_item_id: item,
                    godown_id: String::new(),
                    source_voucher_id: None,
                    source_line_id: None,
                    receipt_date: "0000-00-00".into(),
                    original_quantity: fmt(quantity),
                    remaining_quantity: fmt(quantity),
                    unit_cost: fmt(value / quantity),
                    original_value: fmt(value),
                });
            }
        }
        for event in events {
            let quantity = d(&event.qty, "movement quantity")?.abs();
            match event.kind.as_str() {
                "receipt" => {
                    let cost = event
                        .cost
                        .as_deref()
                        .map(|x| d(x, "unit cost"))
                        .transpose()?
                        .unwrap_or(Decimal::ZERO);
                    if let Some(original) = &event.reversal_of {
                        if let Some(restored) = consumed.get(original) {
                            for (restored_qty, restored_cost) in restored {
                                layers.push(FifoLayer {
                                    id: format!("{}-restore", event.movement_id),
                                    stock_item_id: event.item.clone(),
                                    godown_id: String::new(),
                                    source_voucher_id: Some(event.voucher_id.clone()),
                                    source_line_id: None,
                                    receipt_date: event.date.clone(),
                                    original_quantity: fmt(*restored_qty),
                                    remaining_quantity: fmt(*restored_qty),
                                    unit_cost: fmt(*restored_cost),
                                    original_value: fmt(*restored_qty * *restored_cost),
                                });
                            }
                        }
                    } else {
                        layers.push(FifoLayer {
                            id: event.movement_id,
                            stock_item_id: event.item,
                            godown_id: String::new(),
                            source_voucher_id: Some(event.voucher_id),
                            source_line_id: None,
                            receipt_date: event.date,
                            original_quantity: fmt(quantity),
                            remaining_quantity: fmt(quantity),
                            unit_cost: fmt(cost),
                            original_value: fmt(quantity * cost),
                        });
                    }
                }
                "delivery" => {
                    let mut need = quantity;
                    let mut consumed_for_voucher = Vec::new();
                    for layer in layers.iter_mut().filter(|x| x.stock_item_id == event.item) {
                        if need.is_zero() {
                            break;
                        }
                        let available = d(&layer.remaining_quantity, "layer quantity")?;
                        let take = available.min(need);
                        layer.remaining_quantity = fmt(available - take);
                        need -= take;
                        consumed_for_voucher.push((take, d(&layer.unit_cost, "layer unit cost")?));
                    }
                    if !need.is_zero() {
                        return Err(OpenTallyError::Validation(
                            "Insufficient valued stock".into(),
                        ));
                    }
                    consumed.insert(event.voucher_id, consumed_for_voucher);
                }
                _ => {}
            }
        }
        Ok(layers)
    }
    pub async fn history(
        pool: &SqlitePool,
        company: &str,
        request: ValuationRequest,
    ) -> Result<Vec<ValuationHistoryRow>, OpenTallyError> {
        let events = Self::events(pool, company, &request).await?;
        let mut out = Vec::new();
        for e in events {
            let q = d(&e.qty, "movement quantity")?.abs();
            let cost = e
                .cost
                .as_deref()
                .map(|x| d(x, "unit cost"))
                .transpose()?
                .unwrap_or(Decimal::ZERO);
            out.push(ValuationHistoryRow {
                movement_id: e.movement_id,
                movement_date: e.date,
                movement_type: e.kind,
                voucher_id: e.voucher_id,
                voucher_number: e.voucher_number,
                quantity: fmt(q),
                unit_cost: fmt(cost),
                value: fmt(q * cost),
            });
        }
        Ok(out)
    }
    pub async fn layers(
        pool: &SqlitePool,
        company: &str,
    ) -> Result<Vec<FifoLayer>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT id,stock_item_id,godown_id,source_voucher_id,source_line_id,receipt_date,original_quantity,remaining_quantity,unit_cost,original_value FROM inventory_cost_layers WHERE company_id=? ORDER BY receipt_date,id").bind(company).fetch_all(pool).await?)
    }
    pub fn new_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
        run_sprint4_stage12_valuation_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use crate::inventory::{StockVoucherLineRequest, StockVoucherRequest, StockVoucherService};
    use std::path::Path;

    async fn setup(opening: (&str, &str)) -> DatabaseManager {
        let db = DatabaseManager::init(Path::new(":memory:")).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('vc','Valuation Co','Valuation Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('sg','vc','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','vc','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('i','vc','Item','sg','u',?,'10',?,'0')").bind(opening.0).bind(opening.1).execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('g','vc','Main')")
            .execute(p)
            .await
            .unwrap();
        db
    }
    fn line(q: &str, c: Option<&str>) -> StockVoucherLineRequest {
        StockVoucherLineRequest {
            stock_item_id: "i".into(),
            unit_id: "u".into(),
            godown_id: Some("g".into()),
            source_godown_id: None,
            destination_godown_id: None,
            quantity: q.into(),
            unit_cost: c.map(str::to_string),
            physical_quantity: None,
            description: None,
        }
    }
    async fn receipt(db: &DatabaseManager, q: &str, c: &str) {
        let v = StockVoucherService::create_draft(
            db.pool(),
            "vc",
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(q, Some(c))],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "vc", &v.id)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn fifo_consumes_oldest_layers() {
        let db = setup(("0", "0")).await;
        receipt(&db, "5", "10").await;
        receipt(&db, "3", "20").await;
        let v = StockVoucherService::create_draft(
            db.pool(),
            "vc",
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![line("6", None)],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "vc", &v.id)
            .await
            .unwrap();
        let rows = ValuationService::summary(
            db.pool(),
            "vc",
            ValuationRequest {
                as_of_date: "2026-04-03".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(rows[0].quantity, "2");
        assert_eq!(rows[0].total_value, "40");
    }
    #[tokio::test]
    async fn weighted_average_is_exact_and_method_is_locked() {
        let db = setup(("0", "0")).await;
        ValuationService::set_config(db.pool(), "vc", "weighted_average")
            .await
            .unwrap();
        receipt(&db, "5", "10").await;
        receipt(&db, "3", "20").await;
        let v = StockVoucherService::create_draft(
            db.pool(),
            "vc",
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![line("6", None)],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "vc", &v.id)
            .await
            .unwrap();
        let rows = ValuationService::summary(
            db.pool(),
            "vc",
            ValuationRequest {
                as_of_date: "2026-04-03".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(rows[0].total_value, "27.5");
        assert!(ValuationService::set_config(db.pool(), "vc", "fifo")
            .await
            .is_err());
    }
    #[tokio::test]
    async fn opening_value_is_counted_once() {
        let db = setup(("2", "20")).await;
        let rows = ValuationService::summary(
            db.pool(),
            "vc",
            ValuationRequest {
                as_of_date: "2026-04-01".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(rows[0].quantity, "2");
        assert_eq!(rows[0].total_value, "20");
    }
    #[tokio::test]
    async fn delivery_reversal_restores_fifo_cost() {
        let db = setup(("0", "0")).await;
        receipt(&db, "5", "10").await;
        let delivery = StockVoucherService::create_draft(
            db.pool(),
            "vc",
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![line("2", None)],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "vc", &delivery.id)
            .await
            .unwrap();
        StockVoucherService::cancel_or_reverse(db.pool(), "vc", &delivery.id, true)
            .await
            .unwrap();
        let rows = ValuationService::summary(
            db.pool(),
            "vc",
            ValuationRequest {
                as_of_date: "2026-04-03".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(rows[0].quantity, "5");
        assert_eq!(rows[0].total_value, "50");
    }
}
