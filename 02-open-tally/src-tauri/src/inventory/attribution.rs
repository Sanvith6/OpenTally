use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, Transaction};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InventoryValuationAttribution {
    pub id: String,
    pub company_id: String,
    pub stock_voucher_id: String,
    pub voucher_type: String,
    pub voucher_line_id: String,
    pub stock_item_id: String,
    pub stock_movement_id: String,
    pub valuation_method: String,
    pub quantity: String,
    pub unit_cost: String,
    pub total_value: String,
    pub context: String,
    pub posting_timestamp: String,
    pub reversal_of_attribution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InventoryValuationLayerUse {
    pub id: String,
    pub attribution_id: String,
    pub source_movement_id: Option<String>,
    pub source_voucher_id: Option<String>,
    pub quantity: String,
    pub unit_cost: String,
    pub total_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryValuationAttributionDetails {
    pub attribution: InventoryValuationAttribution,
    pub layer_uses: Vec<InventoryValuationLayerUse>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct Movement {
    id: String,
    voucher_id: String,
    voucher_type: String,
    voucher_line_id: String,
    signed_quantity: String,
    unit_cost: Option<String>,
    reversal_of_voucher_id: Option<String>,
}

fn decimal(value: &str) -> Result<Decimal, OpenTallyError> {
    value
        .parse()
        .map_err(|_| OpenTallyError::IntegrityViolation("Invalid valuation decimal".into()))
}
fn fmt(value: Decimal) -> String {
    value.normalize().to_string()
}

pub struct InventoryValuationAttributionService;
impl InventoryValuationAttributionService {
    pub async fn attribute_movement(
        tx: &mut Transaction<'_, Sqlite>,
        company: &str,
        movement_id: &str,
        valuation_method: &str,
    ) -> Result<(), OpenTallyError> {
        let target: Movement = sqlx::query_as("SELECT m.id,m.voucher_id,v.voucher_type,m.voucher_line_id,m.movement_date,m.signed_quantity,l.unit_cost,v.reversal_of_voucher_id FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id WHERE m.company_id=? AND m.id=?")
            .bind(company).bind(movement_id).fetch_one(&mut **tx).await?;
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM inventory_valuation_attributions WHERE company_id=? AND stock_movement_id=?")
            .bind(company).bind(movement_id).fetch_optional(&mut **tx).await?;
        if exists.is_some() {
            return Ok(());
        }

        if let Some(original_voucher) = &target.reversal_of_voucher_id {
            let original: Option<InventoryValuationAttribution> = sqlx::query_as("SELECT id,company_id,stock_voucher_id,voucher_type,voucher_line_id,stock_item_id,stock_movement_id,valuation_method,quantity,unit_cost,total_value,context,posting_timestamp,reversal_of_attribution_id FROM inventory_valuation_attributions WHERE company_id=? AND stock_voucher_id=? AND stock_item_id=(SELECT stock_item_id FROM stock_movements WHERE company_id=? AND id=?) ORDER BY id LIMIT 1")
                .bind(company).bind(original_voucher).bind(company).bind(movement_id).fetch_optional(&mut **tx).await?;
            if let Some(original) = original {
                let id = Uuid::new_v4().to_string();
                sqlx::query("INSERT INTO inventory_valuation_attributions(id,company_id,stock_voucher_id,voucher_type,voucher_line_id,stock_item_id,stock_movement_id,valuation_method,quantity,unit_cost,total_value,context,reversal_of_attribution_id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)")
                    .bind(&id).bind(company).bind(&target.voucher_id).bind(&target.voucher_type).bind(&target.voucher_line_id).bind(&original.stock_item_id).bind(movement_id).bind(&original.valuation_method).bind(&original.quantity).bind(&original.unit_cost).bind(&original.total_value).bind(&original.context).bind(&original.id).execute(&mut **tx).await?;
                let uses: Vec<InventoryValuationLayerUse> = sqlx::query_as("SELECT id,attribution_id,source_movement_id,source_voucher_id,quantity,unit_cost,total_value FROM inventory_valuation_layer_uses WHERE company_id=? AND attribution_id=?")
                    .bind(company).bind(original.id).fetch_all(&mut **tx).await?;
                for use_row in uses {
                    sqlx::query("INSERT INTO inventory_valuation_layer_uses(id,company_id,attribution_id,source_movement_id,source_voucher_id,quantity,unit_cost,total_value) VALUES(?,?,?,?,?,?,?,?)")
                        .bind(Uuid::new_v4().to_string()).bind(company).bind(&id).bind(use_row.source_movement_id).bind(use_row.source_voucher_id).bind(use_row.quantity).bind(use_row.unit_cost).bind(use_row.total_value).execute(&mut **tx).await?;
                }
                return Ok(());
            }
        }

        let item: String = sqlx::query_scalar(
            "SELECT stock_item_id FROM stock_movements WHERE company_id=? AND id=?",
        )
        .bind(company)
        .bind(movement_id)
        .fetch_one(&mut **tx)
        .await?;
        let opening: (String, String) = sqlx::query_as(
            "SELECT opening_quantity,opening_value FROM stock_items WHERE company_id=? AND id=?",
        )
        .bind(company)
        .bind(&item)
        .fetch_one(&mut **tx)
        .await?;
        let mut fifo: VecDeque<(Option<String>, Option<String>, Decimal, Decimal)> =
            VecDeque::new();
        let oq = decimal(&opening.0)?;
        if oq > Decimal::ZERO {
            fifo.push_back((None, None, oq, decimal(&opening.1)? / oq));
        }
        let mut weighted_qty = oq;
        let mut weighted_value = decimal(&opening.1)?;
        let movements: Vec<Movement> = sqlx::query_as("SELECT m.id,m.voucher_id,v.voucher_type,m.voucher_line_id,m.movement_date,m.signed_quantity,l.unit_cost,v.reversal_of_voucher_id FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id WHERE m.company_id=? AND m.stock_item_id=? AND v.status='posted' ORDER BY m.movement_date,m.rowid")
            .bind(company).bind(&item).fetch_all(&mut **tx).await?;
        let mut target_cost = Decimal::ZERO;
        let target_qty = decimal(&target.signed_quantity)?.abs();
        let mut layer_uses = Vec::new();
        for movement in movements {
            let q = decimal(&movement.signed_quantity)?;
            let absolute = q.abs();
            if movement.voucher_type == "receipt"
                || (movement.voucher_type == "adjustment" && q > Decimal::ZERO)
            {
                let cost = movement
                    .unit_cost
                    .as_deref()
                    .map(decimal)
                    .transpose()?
                    .unwrap_or_else(|| {
                        if weighted_qty.is_zero() {
                            Decimal::ZERO
                        } else {
                            weighted_value / weighted_qty
                        }
                    });
                fifo.push_back((
                    Some(movement.id.clone()),
                    Some(movement.voucher_id.clone()),
                    absolute,
                    cost,
                ));
                weighted_qty += absolute;
                weighted_value += absolute * cost;
                if movement.id == target.id {
                    target_cost = absolute * cost;
                }
            } else if movement.voucher_type == "delivery"
                || (movement.voucher_type == "adjustment" && q < Decimal::ZERO)
            {
                let average = if weighted_qty.is_zero() {
                    Decimal::ZERO
                } else {
                    weighted_value / weighted_qty
                };
                let mut need = absolute;
                while need > Decimal::ZERO {
                    let Some(front) = fifo.front_mut() else {
                        return Err(OpenTallyError::Validation(
                            "Insufficient valued stock".into(),
                        ));
                    };
                    let take = front.2.min(need);
                    if movement.id == target.id {
                        layer_uses.push((front.0.clone(), front.1.clone(), take, front.3));
                    }
                    front.2 -= take;
                    need -= take;
                    if front.2.is_zero() {
                        fifo.pop_front();
                    }
                }
                if movement.id == target.id {
                    target_cost = if valuation_method == "fifo" {
                        layer_uses.iter().map(|x| x.2 * x.3).sum()
                    } else {
                        absolute * average
                    };
                }
                weighted_qty -= absolute;
                weighted_value -= absolute * average;
            }
        }
        let unit_cost = if target_qty.is_zero() {
            Decimal::ZERO
        } else {
            target_cost / target_qty
        };
        let attribution_id = Uuid::new_v4().to_string();
        let context = serde_json::json!({"method": valuation_method, "source": "posting-replay", "quantity_context": fmt(target_qty), "layer_count": layer_uses.len()}).to_string();
        sqlx::query("INSERT INTO inventory_valuation_attributions(id,company_id,stock_voucher_id,voucher_type,voucher_line_id,stock_item_id,stock_movement_id,valuation_method,quantity,unit_cost,total_value,context) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&attribution_id).bind(company).bind(&target.voucher_id).bind(&target.voucher_type).bind(&target.voucher_line_id).bind(&item).bind(&target.id).bind(valuation_method).bind(fmt(target_qty)).bind(fmt(unit_cost)).bind(fmt(target_cost)).bind(context).execute(&mut **tx).await?;
        for (source_movement_id, source_voucher_id, quantity, cost) in layer_uses {
            sqlx::query("INSERT INTO inventory_valuation_layer_uses(id,company_id,attribution_id,source_movement_id,source_voucher_id,quantity,unit_cost,total_value) VALUES(?,?,?,?,?,?,?,?)")
                .bind(Uuid::new_v4().to_string()).bind(company).bind(&attribution_id).bind(source_movement_id).bind(source_voucher_id).bind(fmt(quantity)).bind(fmt(cost)).bind(fmt(quantity * cost)).execute(&mut **tx).await?;
        }
        Ok(())
    }

    pub async fn for_voucher(
        pool: &sqlx::SqlitePool,
        company: &str,
        voucher_id: &str,
    ) -> Result<Vec<InventoryValuationAttribution>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT id,company_id,stock_voucher_id,voucher_type,voucher_line_id,stock_item_id,stock_movement_id,valuation_method,quantity,unit_cost,total_value,context,posting_timestamp,reversal_of_attribution_id FROM inventory_valuation_attributions WHERE company_id=? AND stock_voucher_id=? ORDER BY voucher_line_id,id").bind(company).bind(voucher_id).fetch_all(pool).await?)
    }

    pub async fn details_for_voucher(
        pool: &sqlx::SqlitePool,
        company: &str,
        voucher_id: &str,
    ) -> Result<Vec<InventoryValuationAttributionDetails>, OpenTallyError> {
        let attributions = Self::for_voucher(pool, company, voucher_id).await?;
        let mut details = Vec::with_capacity(attributions.len());
        for attribution in attributions {
            let layer_uses = sqlx::query_as("SELECT id,attribution_id,source_movement_id,source_voucher_id,quantity,unit_cost,total_value FROM inventory_valuation_layer_uses WHERE company_id=? AND attribution_id=? ORDER BY id")
                .bind(company).bind(&attribution.id).fetch_all(pool).await?;
            details.push(InventoryValuationAttributionDetails {
                attribution,
                layer_uses,
            });
        }
        Ok(details)
    }
}
