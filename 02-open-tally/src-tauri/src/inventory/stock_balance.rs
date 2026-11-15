use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBalanceRequest {
    pub as_of_date: String,
    pub stock_item_id: Option<String>,
    pub stock_group_id: Option<String>,
    pub stock_category_id: Option<String>,
    pub godown_id: Option<String>,
    pub search: Option<String>,
    pub include_zero: bool,
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
    #[tokio::test]
    async fn balance_is_as_of_and_company_isolated() {
        let db = DatabaseManager::init(Path::new(":memory:")).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('b1','Balance Co','Balance Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('g','b1','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','b1','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('i','b1','Item','g','u','2','1','2','0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('d','b1','Depot')")
            .execute(p)
            .await
            .unwrap();
        let v = StockVoucherService::create_draft(
            p,
            "b1",
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-10".into(),
                reference_no: None,
                narration: None,
                lines: vec![StockVoucherLineRequest {
                    stock_item_id: "i".into(),
                    unit_id: "u".into(),
                    godown_id: Some("d".into()),
                    source_godown_id: None,
                    destination_godown_id: None,
                    quantity: "3.25".into(),
                    unit_cost: None,
                    physical_quantity: None,
                    description: None,
                }],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(p, "b1", &v.id).await.unwrap();
        let before = StockBalanceService::query(
            p,
            "b1",
            StockBalanceRequest {
                as_of_date: "2026-04-09".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
            true,
        )
        .await
        .unwrap();
        let after = StockBalanceService::query(
            p,
            "b1",
            StockBalanceRequest {
                as_of_date: "2026-04-10".into(),
                stock_item_id: None,
                stock_group_id: None,
                stock_category_id: None,
                godown_id: None,
                search: None,
                include_zero: true,
            },
            true,
        )
        .await
        .unwrap();
        assert_eq!(before[0].closing_quantity, "2");
        assert_eq!(after[0].closing_quantity, "5.25");
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockBalanceRow {
    pub stock_item_id: String,
    pub item_name: String,
    pub stock_group_name: String,
    pub base_unit_symbol: String,
    pub godown_id: Option<String>,
    pub godown_name: Option<String>,
    pub opening_quantity: String,
    pub received_quantity: String,
    pub delivered_quantity: String,
    pub transfer_in_quantity: String,
    pub transfer_out_quantity: String,
    pub adjustment_quantity: String,
    pub closing_quantity: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockMovementHistory {
    pub movement_id: String,
    pub stock_item_id: String,
    pub item_name: String,
    pub godown_id: String,
    pub godown_name: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub movement_date: String,
    pub movement_type: String,
    pub signed_quantity: String,
}
#[derive(sqlx::FromRow)]
struct Raw {
    item_id: String,
    item_name: String,
    group_name: String,
    unit_symbol: String,
    opening: String,
    godown_id: Option<String>,
    godown_name: Option<String>,
    movement_type: Option<String>,
    signed_quantity: Option<String>,
}
#[derive(Default)]
struct Totals {
    received: Decimal,
    delivered: Decimal,
    transfer_in: Decimal,
    transfer_out: Decimal,
    adjustment: Decimal,
}
fn dec(s: &str) -> Result<Decimal, OpenTallyError> {
    s.parse()
        .map_err(|_| OpenTallyError::IntegrityViolation("Invalid stored stock quantity".into()))
}
pub struct StockBalanceService;
impl StockBalanceService {
    pub async fn query(
        pool: &SqlitePool,
        c: &str,
        r: StockBalanceRequest,
        overall: bool,
    ) -> Result<Vec<StockBalanceRow>, OpenTallyError> {
        if chrono::NaiveDate::parse_from_str(&r.as_of_date, "%Y-%m-%d").is_err() {
            return Err(OpenTallyError::Validation(
                "As-of date must be YYYY-MM-DD".into(),
            ));
        }
        let search = r.search.map(|s| format!("%{}%", s));
        let raw:Vec<Raw>=sqlx::query_as("SELECT i.id item_id,i.name item_name,g.name group_name,u.symbol unit_symbol,i.opening_quantity opening,m.godown_id,gd.name godown_name,m.movement_type,m.signed_quantity FROM stock_items i JOIN stock_groups g ON g.company_id=i.company_id AND g.id=i.stock_group_id JOIN units u ON u.company_id=i.company_id AND u.id=i.base_unit_id LEFT JOIN stock_movements m ON m.company_id=i.company_id AND m.stock_item_id=i.id AND m.movement_date<=? AND EXISTS (SELECT 1 FROM stock_vouchers v2 WHERE v2.company_id=m.company_id AND v2.id=m.voucher_id AND v2.status='posted') LEFT JOIN godowns gd ON gd.company_id=m.company_id AND gd.id=m.godown_id WHERE i.company_id=? AND (? IS NULL OR i.id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?) AND (? IS NULL OR i.name LIKE ? COLLATE NOCASE OR i.sku LIKE ? COLLATE NOCASE) AND (? IS NULL OR m.godown_id=?) ORDER BY i.name,gd.name,i.id,m.id").bind(&r.as_of_date).bind(c).bind(&r.stock_item_id).bind(&r.stock_item_id).bind(&r.stock_group_id).bind(&r.stock_group_id).bind(&r.stock_category_id).bind(&r.stock_category_id).bind(search.as_deref()).bind(search.as_deref()).bind(&r.godown_id).bind(&r.godown_id).fetch_all(pool).await?;
        let mut map: BTreeMap<(String, Option<String>), (Raw, Totals)> = BTreeMap::new();
        for row in raw {
            let key = (
                row.item_id.clone(),
                if overall { None } else { row.godown_id.clone() },
            );
            let entry = map.entry(key).or_insert_with(|| {
                (
                    Raw {
                        item_id: row.item_id.clone(),
                        item_name: row.item_name.clone(),
                        group_name: row.group_name.clone(),
                        unit_symbol: row.unit_symbol.clone(),
                        opening: row.opening.clone(),
                        godown_id: row.godown_id.clone(),
                        godown_name: row.godown_name.clone(),
                        movement_type: None,
                        signed_quantity: None,
                    },
                    Totals::default(),
                )
            });
            if let (Some(kind), Some(value)) = (row.movement_type, row.signed_quantity) {
                let q = dec(&value)?;
                match kind.as_str() {
                    "receipt" => entry.1.received += q,
                    "delivery" => entry.1.delivered += -q,
                    "transfer_in" => entry.1.transfer_in += q,
                    "transfer_out" => entry.1.transfer_out += -q,
                    "adjustment" => entry.1.adjustment += q,
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for ((_, gid), (row, t)) in map {
            let opening = if overall {
                dec(&row.opening)?
            } else {
                Decimal::ZERO
            };
            let closing =
                opening + t.received - t.delivered + t.transfer_in - t.transfer_out + t.adjustment;
            if r.include_zero || !closing.is_zero() {
                out.push(StockBalanceRow {
                    stock_item_id: row.item_id,
                    item_name: row.item_name,
                    stock_group_name: row.group_name,
                    base_unit_symbol: row.unit_symbol,
                    godown_id: if overall { None } else { gid },
                    godown_name: if overall { None } else { row.godown_name },
                    opening_quantity: opening.normalize().to_string(),
                    received_quantity: t.received.normalize().to_string(),
                    delivered_quantity: t.delivered.normalize().to_string(),
                    transfer_in_quantity: t.transfer_in.normalize().to_string(),
                    transfer_out_quantity: t.transfer_out.normalize().to_string(),
                    adjustment_quantity: t.adjustment.normalize().to_string(),
                    closing_quantity: closing.normalize().to_string(),
                });
            }
        }
        Ok(out)
    }
    pub async fn history(
        pool: &SqlitePool,
        c: &str,
        item: Option<String>,
        godown: Option<String>,
    ) -> Result<Vec<StockMovementHistory>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT m.id movement_id,m.stock_item_id,i.name item_name,m.godown_id,g.name godown_name,m.voucher_id,v.voucher_number,m.movement_date,m.movement_type,m.signed_quantity FROM stock_movements m JOIN stock_items i ON i.company_id=m.company_id AND i.id=m.stock_item_id JOIN godowns g ON g.company_id=m.company_id AND g.id=m.godown_id JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id WHERE m.company_id=? AND v.status='posted' AND (? IS NULL OR m.stock_item_id=?) AND (? IS NULL OR m.godown_id=?) ORDER BY m.movement_date,m.id").bind(c).bind(&item).bind(&item).bind(&godown).bind(&godown).fetch_all(pool).await?)
    }
}
