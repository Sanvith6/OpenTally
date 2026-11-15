use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockVoucher {
    pub id: String,
    pub company_id: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub voucher_date: String,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub status: String,
    pub reversal_of_voucher_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests_initial {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
        run_sprint4_stage12_valuation_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use std::path::Path;
    async fn setup() -> (DatabaseManager, String, String, String) {
        let db = DatabaseManager::init(Path::new(":memory:")).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('sv_c','Stock Co','Stock Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('sg','sv_c','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','sv_c','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('item','sv_c','Widget','sg','u','0','10','0','0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('g1','sv_c','Main'),('g2','sv_c','Branch')").execute(p).await.unwrap();
        (db, "sv_c".into(), "item".into(), "u".into())
    }
    fn line(item: &str, unit: &str, g: &str, q: &str) -> StockVoucherLineRequest {
        StockVoucherLineRequest {
            stock_item_id: item.into(),
            unit_id: unit.into(),
            godown_id: Some(g.into()),
            source_godown_id: None,
            destination_godown_id: None,
            quantity: q.into(),
            unit_cost: None,
            physical_quantity: None,
            description: None,
        }
    }
    #[tokio::test]
    async fn receipt_delivery_duplicate_and_rollback() {
        let (db, c, item, u) = setup().await;
        let r = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "10.25")],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .unwrap();
        assert!(StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .is_err());
        let d = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "11")],
            },
        )
        .await
        .unwrap();
        assert!(StockVoucherService::post(db.pool(), &c, &d.id)
            .await
            .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM stock_movements")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
    }
    #[tokio::test]
    async fn transfer_preserves_total_and_reversal_compensates() {
        let (db, c, item, u) = setup().await;
        let r = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "5")],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .unwrap();
        let mut t = line(&item, &u, "g1", "3");
        t.godown_id = None;
        t.source_godown_id = Some("g1".into());
        t.destination_godown_id = Some("g2".into());
        let x = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "transfer".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![t],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &x.id)
            .await
            .unwrap();
        let before: Vec<(String,)> =
            sqlx::query_as("SELECT signed_quantity FROM stock_movements WHERE voucher_id=?")
                .bind(&x.id)
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(before.len(), 2);
        StockVoucherService::cancel_or_reverse(db.pool(), &c, &x.id, true)
            .await
            .unwrap();
        let status: String = sqlx::query_scalar("SELECT status FROM stock_vouchers WHERE id=?")
            .bind(&x.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(status, "reversed");
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(after, 5);
    }
    #[tokio::test]
    async fn physical_adjustment_records_difference() {
        let (db, c, item, u) = setup().await;
        let mut l = line(&item, &u, "g1", "7");
        l.physical_quantity = Some("7".into());
        let a = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "adjustment".into(),
                voucher_date: "2026-04-04".into(),
                reference_no: None,
                narration: None,
                lines: vec![l],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &a.id)
            .await
            .unwrap();
        let diff: Option<String> = sqlx::query_scalar(
            "SELECT difference_quantity FROM stock_voucher_lines WHERE voucher_id=?",
        )
        .bind(&a.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(diff.as_deref(), Some("7"));
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockVoucherLine {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub line_number: i64,
    pub stock_item_id: String,
    pub unit_id: String,
    pub godown_id: Option<String>,
    pub source_godown_id: Option<String>,
    pub destination_godown_id: Option<String>,
    pub quantity: String,
    pub unit_cost: Option<String>,
    pub line_value: Option<String>,
    pub system_quantity: Option<String>,
    pub physical_quantity: Option<String>,
    pub difference_quantity: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockVoucherLineRequest {
    pub stock_item_id: String,
    pub unit_id: String,
    pub godown_id: Option<String>,
    pub source_godown_id: Option<String>,
    pub destination_godown_id: Option<String>,
    pub quantity: String,
    pub unit_cost: Option<String>,
    pub physical_quantity: Option<String>,
    pub description: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockVoucherRequest {
    pub voucher_type: String,
    pub voucher_date: String,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub lines: Vec<StockVoucherLineRequest>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockVoucherListFilter {
    pub search: Option<String>,
    pub voucher_type: Option<String>,
    pub status: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

fn decimal(value: &str, field: &str) -> Result<Decimal, OpenTallyError> {
    let d = value
        .parse::<Decimal>()
        .map_err(|_| OpenTallyError::Validation(format!("{} must be a decimal", field)))?;
    if d <= Decimal::ZERO || d.scale() > 4 {
        return Err(OpenTallyError::Validation(format!(
            "{} must be positive with at most 4 decimals",
            field
        )));
    }
    Ok(d)
}
fn valid_date(value: &str) -> Result<(), OpenTallyError> {
    if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
        return Err(OpenTallyError::Validation(
            "Voucher date must be YYYY-MM-DD".into(),
        ));
    }
    Ok(())
}
fn valid_type(value: &str) -> Result<(), OpenTallyError> {
    if ["receipt", "delivery", "transfer", "adjustment"].contains(&value) {
        Ok(())
    } else {
        Err(OpenTallyError::Validation(
            "Invalid stock voucher type".into(),
        ))
    }
}
async fn audit(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    id: &str,
    action: &str,
) -> Result<(), OpenTallyError> {
    sqlx::query("INSERT INTO audit_log(id,company_id,entity_type,entity_id,action,performed_by) VALUES(?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(company).bind("stock_voucher").bind(id).bind(action).bind("system").execute(&mut **tx).await?;
    Ok(())
}
async fn get_tx(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    id: &str,
) -> Result<StockVoucher, OpenTallyError> {
    sqlx::query_as("SELECT id,company_id,voucher_number,voucher_type,voucher_date,reference_no,narration,status,reversal_of_voucher_id,created_at,updated_at FROM stock_vouchers WHERE company_id=? AND id=?").bind(company).bind(id).fetch_optional(&mut **tx).await?.ok_or_else(||OpenTallyError::NotFound(format!("Stock voucher '{}' not found",id)))
}
async fn get_lines(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    id: &str,
) -> Result<Vec<StockVoucherLine>, OpenTallyError> {
    Ok(sqlx::query_as("SELECT id,company_id,voucher_id,line_number,stock_item_id,unit_id,godown_id,source_godown_id,destination_godown_id,quantity,unit_cost,line_value,system_quantity,physical_quantity,difference_quantity,description,created_at FROM stock_voucher_lines WHERE company_id=? AND voucher_id=? ORDER BY line_number,id").bind(company).bind(id).fetch_all(&mut **tx).await?)
}
async fn godown(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    id: &str,
) -> Result<(), OpenTallyError> {
    if sqlx::query_scalar::<_, String>(
        "SELECT id FROM godowns WHERE company_id=? AND id=? AND is_active=1",
    )
    .bind(company)
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .is_none()
    {
        return Err(OpenTallyError::Validation(format!(
            "Godown '{}' not found or inactive",
            id
        )));
    }
    Ok(())
}
async fn validate_line(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    ty: &str,
    line: &StockVoucherLineRequest,
) -> Result<Decimal, OpenTallyError> {
    let (base,precision):(String,i64)=sqlx::query_as("SELECT i.base_unit_id,u.decimal_precision FROM stock_items i JOIN units u ON u.company_id=i.company_id AND u.id=i.base_unit_id WHERE i.company_id=? AND i.id=? AND i.is_active=1").bind(company).bind(&line.stock_item_id).fetch_optional(&mut **tx).await?.ok_or_else(||OpenTallyError::Validation("Stock item not found or inactive".into()))?;
    if base != line.unit_id {
        return Err(OpenTallyError::Validation(
            "Only the configured base unit is supported".into(),
        ));
    }
    let q = decimal(&line.quantity, "Quantity")?;
    if q.scale() > precision as u32 {
        return Err(OpenTallyError::Validation(
            "Quantity exceeds unit precision".into(),
        ));
    }
    if ty == "receipt" {
        if let Some(cost) = &line.unit_cost {
            let c = cost.parse::<Decimal>().map_err(|_| {
                OpenTallyError::Validation("Receipt unit cost must be decimal".into())
            })?;
            if c < Decimal::ZERO || c.scale() > 4 {
                return Err(OpenTallyError::Validation(
                    "Receipt unit cost must be non-negative with at most 4 decimals".into(),
                ));
            }
        }
    }
    match ty {
        "receipt" | "delivery" => {
            godown(
                tx,
                company,
                line.godown_id
                    .as_deref()
                    .ok_or_else(|| OpenTallyError::Validation("Godown is required".into()))?,
            )
            .await?
        }
        "transfer" => {
            let source = line
                .source_godown_id
                .as_deref()
                .ok_or_else(|| OpenTallyError::Validation("Source godown is required".into()))?;
            let destination = line.destination_godown_id.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Destination godown is required".into())
            })?;
            if source == destination {
                return Err(OpenTallyError::Validation(
                    "Transfer godowns must differ".into(),
                ));
            }
            godown(tx, company, source).await?;
            godown(tx, company, destination).await?;
        }
        "adjustment" => {
            godown(
                tx,
                company,
                line.godown_id
                    .as_deref()
                    .ok_or_else(|| OpenTallyError::Validation("Godown is required".into()))?,
            )
            .await?;
            let physical = line.physical_quantity.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Physical quantity is required".into())
            })?;
            let p = physical.parse::<Decimal>().map_err(|_| {
                OpenTallyError::Validation("Physical quantity must be decimal".into())
            })?;
            if p < Decimal::ZERO || p.scale() > precision as u32 {
                return Err(OpenTallyError::Validation(
                    "Physical quantity is invalid".into(),
                ));
            }
        }
        _ => {}
    }
    Ok(q)
}
async fn current(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    item: &str,
    godown_id: &str,
) -> Result<Decimal, OpenTallyError> {
    let values:Vec<(String,)> = sqlx::query_as("SELECT m.signed_quantity FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id WHERE m.company_id=? AND m.stock_item_id=? AND m.godown_id=? AND v.status='posted'").bind(company).bind(item).bind(godown_id).fetch_all(&mut **tx).await?;
    let mut total = Decimal::ZERO;
    for (value,) in values {
        total += value
            .parse::<Decimal>()
            .map_err(|_| OpenTallyError::IntegrityViolation("Invalid stored quantity".into()))?;
    }
    Ok(total)
}
async fn movement(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    voucher: &StockVoucher,
    line: &StockVoucherLine,
    kind: &str,
    godown_id: &str,
    quantity: Decimal,
) -> Result<String, OpenTallyError> {
    if quantity.is_zero() {
        return Ok(String::new());
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stock_movements(id,company_id,stock_item_id,godown_id,voucher_id,voucher_line_id,movement_date,movement_type,signed_quantity) VALUES(?,?,?,?,?,?,?,?,?)").bind(&id).bind(company).bind(&line.stock_item_id).bind(godown_id).bind(&voucher.id).bind(&line.id).bind(&voucher.voucher_date).bind(kind).bind(quantity.normalize().to_string()).execute(&mut **tx).await?;
    Ok(id)
}

async fn attribute_if_available(
    tx: &mut Transaction<'_, Sqlite>,
    company: &str,
    movement_id: &str,
) -> Result<(), OpenTallyError> {
    if movement_id.is_empty() {
        return Ok(());
    }
    let available: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='inventory_valuation_attributions'")
        .fetch_one(&mut **tx).await?;
    if available == 0 {
        return Ok(());
    }
    let method: String = sqlx::query_scalar(
        "SELECT valuation_method FROM inventory_valuation_config WHERE company_id=?",
    )
    .bind(company)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or_else(|| "fifo".into());
    crate::inventory::InventoryValuationAttributionService::attribute_movement(
        tx,
        company,
        movement_id,
        &method,
    )
    .await
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
    use std::path::Path;

    async fn setup() -> (DatabaseManager, String, String, String) {
        let db = DatabaseManager::init(Path::new(":memory:")).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('sv_c','Stock Co','Stock Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('sg','sv_c','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','sv_c','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('item','sv_c','Widget','sg','u','0','10','0','0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('g1','sv_c','Main'),('g2','sv_c','Branch')").execute(p).await.unwrap();
        (db, "sv_c".into(), "item".into(), "u".into())
    }
    fn line(item: &str, unit: &str, godown: &str, q: &str) -> StockVoucherLineRequest {
        StockVoucherLineRequest {
            stock_item_id: item.into(),
            unit_id: unit.into(),
            godown_id: Some(godown.into()),
            source_godown_id: None,
            destination_godown_id: None,
            quantity: q.into(),
            unit_cost: None,
            physical_quantity: None,
            description: None,
        }
    }
    #[tokio::test]
    async fn receipt_delivery_and_duplicate_posting() {
        let (db, c, item, u) = setup().await;
        let r = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "10.25")],
            },
        )
        .await
        .unwrap();
        let posted = StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .unwrap();
        assert_eq!(posted.status, "posted");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM stock_movements WHERE company_id='sv_c'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            1
        );
        assert!(StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .is_err());
        let d = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "4.25")],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &d.id)
            .await
            .unwrap();
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements WHERE company_id='sv_c'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(n, 2);
    }
    #[tokio::test]
    async fn transfer_is_atomic_and_preserves_total() {
        let (db, c, item, u) = setup().await;
        let r = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "5")],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &r.id)
            .await
            .unwrap();
        let mut t = line(&item, &u, "g1", "3");
        t.source_godown_id = Some("g1".into());
        t.destination_godown_id = Some("g2".into());
        t.godown_id = None;
        let x = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "transfer".into(),
                voucher_date: "2026-04-03".into(),
                reference_no: None,
                narration: None,
                lines: vec![t],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), &c, &x.id)
            .await
            .unwrap();
        let total: Vec<(String,)> =
            sqlx::query_as("SELECT signed_quantity FROM stock_movements WHERE company_id='sv_c'")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let sum = total
            .into_iter()
            .map(|x| x.0.parse::<Decimal>().unwrap())
            .fold(Decimal::ZERO, |a, b| a + b);
        assert_eq!(sum, Decimal::from(5));
    }
    #[tokio::test]
    async fn insufficient_delivery_rolls_back() {
        let (db, c, item, u) = setup().await;
        let d = StockVoucherService::create_draft(
            db.pool(),
            &c,
            StockVoucherRequest {
                voucher_type: "delivery".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![line(&item, &u, "g1", "1")],
            },
        )
        .await
        .unwrap();
        assert!(StockVoucherService::post(db.pool(), &c, &d.id)
            .await
            .is_err());
        let status: String = sqlx::query_scalar(
            "SELECT status FROM stock_vouchers WHERE company_id='sv_c' AND id=?",
        )
        .bind(&d.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(status, "draft");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM stock_movements WHERE company_id='sv_c'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            0
        );
    }
}

pub struct StockVoucherService;
impl StockVoucherService {
    pub async fn update_draft(
        pool: &SqlitePool,
        company: &str,
        id: &str,
        request: StockVoucherRequest,
    ) -> Result<StockVoucher, OpenTallyError> {
        valid_type(&request.voucher_type)?;
        valid_date(&request.voucher_date)?;
        if request.lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "At least one line is required".into(),
            ));
        }
        let mut tx = pool.begin().await?;
        let existing = get_tx(&mut tx, company, id).await?;
        if existing.status != "draft" {
            return Err(OpenTallyError::Validation(
                "Only drafts can be updated".into(),
            ));
        }
        sqlx::query("UPDATE stock_vouchers SET voucher_type=?,voucher_date=?,reference_no=?,narration=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(&request.voucher_type).bind(&request.voucher_date).bind(&request.reference_no).bind(&request.narration).bind(company).bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM stock_voucher_lines WHERE company_id=? AND voucher_id=?")
            .bind(company)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for (index, line) in request.lines.iter().enumerate() {
            validate_line(&mut tx, company, &request.voucher_type, line).await?;
            sqlx::query("INSERT INTO stock_voucher_lines(id,company_id,voucher_id,line_number,stock_item_id,unit_id,godown_id,source_godown_id,destination_godown_id,quantity,unit_cost,line_value,physical_quantity,description) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(company).bind(id).bind((index+1) as i64).bind(&line.stock_item_id).bind(&line.unit_id).bind(&line.godown_id).bind(&line.source_godown_id).bind(&line.destination_godown_id).bind(&line.quantity).bind(&line.unit_cost).bind(line.unit_cost.as_ref().map(|c| line.quantity.parse::<Decimal>().ok().and_then(|q| c.parse::<Decimal>().ok().map(|cost| (q*cost).normalize().to_string())))).bind(&line.physical_quantity).bind(&line.description).execute(&mut *tx).await?;
        }
        audit(&mut tx, company, id, "UPDATE").await?;
        let output = get_tx(&mut tx, company, id).await?;
        tx.commit().await?;
        Ok(output)
    }
    pub async fn create_draft(
        pool: &SqlitePool,
        company: &str,
        request: StockVoucherRequest,
    ) -> Result<StockVoucher, OpenTallyError> {
        valid_type(&request.voucher_type)?;
        valid_date(&request.voucher_date)?;
        if request.lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "At least one line is required".into(),
            ));
        }
        let mut tx = pool.begin().await?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO stock_vouchers(id,company_id,voucher_number,voucher_type,voucher_date,reference_no,narration) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(company).bind(format!("DRAFT-{}",&id[..8])).bind(&request.voucher_type).bind(&request.voucher_date).bind(&request.reference_no).bind(&request.narration).execute(&mut *tx).await?;
        for (index, line) in request.lines.iter().enumerate() {
            validate_line(&mut tx, company, &request.voucher_type, line).await?;
            sqlx::query("INSERT INTO stock_voucher_lines(id,company_id,voucher_id,line_number,stock_item_id,unit_id,godown_id,source_godown_id,destination_godown_id,quantity,unit_cost,line_value,physical_quantity,description) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(company).bind(&id).bind((index+1) as i64).bind(&line.stock_item_id).bind(&line.unit_id).bind(&line.godown_id).bind(&line.source_godown_id).bind(&line.destination_godown_id).bind(&line.quantity).bind(&line.unit_cost).bind(line.unit_cost.as_ref().map(|c| line.quantity.parse::<Decimal>().ok().and_then(|q| c.parse::<Decimal>().ok().map(|cost| (q*cost).normalize().to_string())))).bind(&line.physical_quantity).bind(&line.description).execute(&mut *tx).await?;
        }
        audit(&mut tx, company, &id, "CREATE").await?;
        let output = get_tx(&mut tx, company, &id).await?;
        tx.commit().await?;
        Ok(output)
    }
    pub async fn get(
        pool: &SqlitePool,
        company: &str,
        id: &str,
    ) -> Result<(StockVoucher, Vec<StockVoucherLine>), OpenTallyError> {
        let mut tx = pool.begin().await?;
        let v = get_tx(&mut tx, company, id).await?;
        let l = get_lines(&mut tx, company, id).await?;
        tx.commit().await?;
        Ok((v, l))
    }
    pub async fn list(
        pool: &SqlitePool,
        company: &str,
        filter: StockVoucherListFilter,
    ) -> Result<Vec<StockVoucher>, OpenTallyError> {
        let search = filter.search.map(|x| format!("%{}%", x));
        Ok(sqlx::query_as("SELECT id,company_id,voucher_number,voucher_type,voucher_date,reference_no,narration,status,reversal_of_voucher_id,created_at,updated_at FROM stock_vouchers WHERE company_id=? AND (? IS NULL OR voucher_number LIKE ? COLLATE NOCASE OR narration LIKE ? COLLATE NOCASE) AND (? IS NULL OR voucher_type=?) AND (? IS NULL OR status=?) AND (? IS NULL OR voucher_date>=?) AND (? IS NULL OR voucher_date<=?) ORDER BY voucher_date DESC,id DESC").bind(company).bind(search.as_deref()).bind(search.as_deref()).bind(search.as_deref()).bind(&filter.voucher_type).bind(&filter.voucher_type).bind(&filter.status).bind(&filter.status).bind(&filter.date_from).bind(&filter.date_from).bind(&filter.date_to).bind(&filter.date_to).fetch_all(pool).await?)
    }
    pub async fn post(
        pool: &SqlitePool,
        company: &str,
        id: &str,
    ) -> Result<StockVoucher, OpenTallyError> {
        let mut tx = pool.begin().await?;
        let voucher = get_tx(&mut tx, company, id).await?;
        if voucher.status != "draft" {
            return Err(OpenTallyError::Validation(
                "Only drafts can be posted".into(),
            ));
        }
        let stored = get_lines(&mut tx, company, id).await?;
        let sequence: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_vouchers WHERE company_id=? AND status<>'draft'",
        )
        .bind(company)
        .fetch_one(&mut *tx)
        .await?;
        let number = format!("ST-{}-{:06}", &voucher.voucher_date[..4], sequence + 1);
        sqlx::query("UPDATE stock_vouchers SET voucher_number=?,status='posted',updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(&number).bind(company).bind(id).execute(&mut *tx).await?;
        for line in &stored {
            let request = StockVoucherLineRequest {
                stock_item_id: line.stock_item_id.clone(),
                unit_id: line.unit_id.clone(),
                godown_id: line.godown_id.clone(),
                source_godown_id: line.source_godown_id.clone(),
                destination_godown_id: line.destination_godown_id.clone(),
                quantity: line.quantity.clone(),
                unit_cost: line.unit_cost.clone(),
                physical_quantity: line.physical_quantity.clone(),
                description: line.description.clone(),
            };
            let q = validate_line(&mut tx, company, &voucher.voucher_type, &request).await?;
            match voucher.voucher_type.as_str() {
                "receipt" => {
                    let movement_id = movement(
                        &mut tx,
                        company,
                        &voucher,
                        line,
                        "receipt",
                        line.godown_id.as_deref().unwrap(),
                        q,
                    )
                    .await?;
                    attribute_if_available(&mut tx, company, &movement_id).await?
                }
                "delivery" => {
                    let g = line.godown_id.as_deref().unwrap();
                    if current(&mut tx, company, &line.stock_item_id, g).await? < q {
                        return Err(OpenTallyError::Validation("Insufficient stock".into()));
                    }
                    let movement_id =
                        movement(&mut tx, company, &voucher, line, "delivery", g, -q).await?;
                    attribute_if_available(&mut tx, company, &movement_id).await?
                }
                "transfer" => {
                    let source = line.source_godown_id.as_deref().unwrap();
                    if current(&mut tx, company, &line.stock_item_id, source).await? < q {
                        return Err(OpenTallyError::Validation(
                            "Insufficient source stock".into(),
                        ));
                    }
                    let movement_id =
                        movement(&mut tx, company, &voucher, line, "transfer_out", source, -q)
                            .await?;
                    attribute_if_available(&mut tx, company, &movement_id).await?;
                    let movement_id = movement(
                        &mut tx,
                        company,
                        &voucher,
                        line,
                        "transfer_in",
                        line.destination_godown_id.as_deref().unwrap(),
                        q,
                    )
                    .await?;
                    attribute_if_available(&mut tx, company, &movement_id).await?
                }
                "adjustment" => {
                    let g = line.godown_id.as_deref().unwrap();
                    let system = current(&mut tx, company, &line.stock_item_id, g).await?;
                    let physical = line
                        .physical_quantity
                        .as_deref()
                        .unwrap()
                        .parse::<Decimal>()
                        .map_err(|_| {
                            OpenTallyError::Validation("Invalid physical quantity".into())
                        })?;
                    let difference = physical - system;
                    sqlx::query("UPDATE stock_voucher_lines SET system_quantity=?,difference_quantity=? WHERE company_id=? AND id=?").bind(system.normalize().to_string()).bind(difference.normalize().to_string()).bind(company).bind(&line.id).execute(&mut *tx).await?;
                    let movement_id = movement(
                        &mut tx,
                        company,
                        &voucher,
                        line,
                        "adjustment",
                        g,
                        difference,
                    )
                    .await?;
                    attribute_if_available(&mut tx, company, &movement_id).await?
                }
                _ => unreachable!(),
            }
        }
        audit(&mut tx, company, id, "UPDATE").await?;
        let output = get_tx(&mut tx, company, id).await?;
        tx.commit().await?;
        Ok(output)
    }
    pub async fn cancel_or_reverse(
        pool: &SqlitePool,
        company: &str,
        id: &str,
        reverse: bool,
    ) -> Result<StockVoucher, OpenTallyError> {
        let mut tx = pool.begin().await?;
        let voucher = get_tx(&mut tx, company, id).await?;
        if voucher.status != "posted" {
            return Err(OpenTallyError::Validation(
                "Only posted vouchers can be cancelled or reversed".into(),
            ));
        }
        if !reverse {
            sqlx::query("UPDATE stock_vouchers SET status='cancelled',updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(company).bind(id).execute(&mut *tx).await?;
            audit(&mut tx, company, id, "UPDATE").await?;
            tx.commit().await?;
            return Ok(voucher);
        }
        let original_lines = get_lines(&mut tx, company, id).await?;
        let new_id = Uuid::new_v4().to_string();
        let number = format!("ST-REV-{}", &new_id[..8]);
        sqlx::query("INSERT INTO stock_vouchers(id,company_id,voucher_number,voucher_type,voucher_date,narration,status,reversal_of_voucher_id) VALUES(?,?,?,?,?,?,?,?)")
            .bind(&new_id).bind(company).bind(&number).bind(&voucher.voucher_type).bind(&voucher.voucher_date)
            .bind("Compensating reversal").bind("posted").bind(id).execute(&mut *tx).await?;
        let reversing = get_tx(&mut tx, company, &new_id).await?;
        for (index, old_line) in original_lines.iter().enumerate() {
            let new_line_id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO stock_voucher_lines(id,company_id,voucher_id,line_number,stock_item_id,unit_id,godown_id,source_godown_id,destination_godown_id,quantity,unit_cost,line_value,physical_quantity,description) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
                .bind(&new_line_id).bind(company).bind(&new_id).bind((index + 1) as i64).bind(&old_line.stock_item_id)
                .bind(&old_line.unit_id).bind(&old_line.godown_id).bind(&old_line.source_godown_id).bind(&old_line.destination_godown_id)
                .bind(&old_line.quantity).bind(&old_line.unit_cost).bind(&old_line.line_value).bind(&old_line.physical_quantity).bind(&old_line.description).execute(&mut *tx).await?;
            let new_line = StockVoucherLine {
                id: new_line_id,
                company_id: company.into(),
                voucher_id: new_id.clone(),
                line_number: (index + 1) as i64,
                stock_item_id: old_line.stock_item_id.clone(),
                unit_id: old_line.unit_id.clone(),
                godown_id: old_line.godown_id.clone(),
                source_godown_id: old_line.source_godown_id.clone(),
                destination_godown_id: old_line.destination_godown_id.clone(),
                quantity: old_line.quantity.clone(),
                unit_cost: old_line.unit_cost.clone(),
                line_value: old_line.line_value.clone(),
                system_quantity: None,
                physical_quantity: None,
                difference_quantity: None,
                description: old_line.description.clone(),
                created_at: String::new(),
            };
            let movements: Vec<(String,String,String)> = sqlx::query_as("SELECT godown_id,movement_type,signed_quantity FROM stock_movements WHERE company_id=? AND voucher_line_id=? ORDER BY id")
                .bind(company).bind(&old_line.id).fetch_all(&mut *tx).await?;
            for (godown_id, movement_type, stored_quantity) in movements {
                let quantity = -stored_quantity.parse::<Decimal>().map_err(|_| {
                    OpenTallyError::IntegrityViolation("Invalid stored quantity".into())
                })?;
                let inverse_type = match movement_type.as_str() {
                    "receipt" => "delivery",
                    "delivery" => "receipt",
                    "transfer_in" => "transfer_out",
                    "transfer_out" => "transfer_in",
                    "adjustment" => "adjustment",
                    _ => {
                        return Err(OpenTallyError::IntegrityViolation(
                            "Unknown stored movement type".into(),
                        ))
                    }
                };
                let movement_id = movement(
                    &mut tx,
                    company,
                    &reversing,
                    &new_line,
                    inverse_type,
                    &godown_id,
                    quantity,
                )
                .await?;
                attribute_if_available(&mut tx, company, &movement_id).await?;
            }
        }
        sqlx::query("UPDATE stock_vouchers SET status='reversed',updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(company).bind(id).execute(&mut *tx).await?;
        audit(&mut tx, company, id, "UPDATE").await?;
        audit(&mut tx, company, &new_id, "CREATE").await?;
        tx.commit().await?;
        Ok(reversing)
    }
}
