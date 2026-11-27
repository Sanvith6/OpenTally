//! Inventory master data foundation. Stock movements and valuation deliberately remain out
//! of this module until the later inventory transaction stages.

use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub mod accounting;
pub mod attribution;
pub mod batches;
pub mod bom;
pub mod pricing;
pub mod reports;
#[cfg(test)]
mod stage15_tests;
pub mod stock_balance;
pub mod stock_journal;
pub mod stock_vouchers;
pub mod units;
pub mod valuation;
pub use accounting::*;
pub use attribution::*;
pub use batches::*;
pub use bom::*;
pub use reports::*;
pub use stock_balance::*;
pub use stock_journal::*;
pub use stock_vouchers::*;
pub use valuation::*;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockGroup {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockCategory {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Unit {
    pub id: String,
    pub company_id: String,
    pub symbol: String,
    pub formal_name: String,
    pub unit_type: String,
    pub base_unit_id: Option<String>,
    pub secondary_unit_id: Option<String>,
    pub conversion_factor: Option<String>,
    pub decimal_precision: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockItem {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub stock_group_id: String,
    pub stock_category_id: Option<String>,
    pub base_unit_id: String,
    pub alternate_unit_id: Option<String>,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub opening_quantity: String,
    pub opening_rate: String,
    pub opening_value: String,
    pub reorder_level: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Godown {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub is_external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockGroupRequest {
    pub name: String,
    pub parent_id: Option<String>,
    pub description: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockCategoryRequest {
    pub name: String,
    pub description: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitRequest {
    pub symbol: String,
    pub formal_name: String,
    pub decimal_precision: i64,
    pub unit_type: Option<String>,
    pub base_unit_id: Option<String>,
    pub secondary_unit_id: Option<String>,
    pub conversion_factor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockItemRequest {
    pub name: String,
    pub stock_group_id: String,
    pub stock_category_id: Option<String>,
    pub base_unit_id: String,
    pub alternate_unit_id: Option<String>,
    pub sku: Option<String>,
    pub description: Option<String>,
    pub opening_quantity: String,
    pub opening_rate: String,
    pub opening_value: String,
    pub reorder_level: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GodownRequest {
    pub name: String,
    pub parent_id: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub is_external: bool,
}

fn name(value: &str, field: &str) -> Result<String, OpenTallyError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        return Err(OpenTallyError::Validation(format!(
            "{} must be between 1 and 120 characters",
            field
        )));
    }
    Ok(value.to_string())
}
fn decimal(value: &str, field: &str) -> Result<Decimal, OpenTallyError> {
    let d = value.parse::<Decimal>().map_err(|_| {
        OpenTallyError::Validation(format!("{} must be a non-negative decimal", field))
    })?;
    if d.is_sign_negative() || d.scale() > 4 {
        return Err(OpenTallyError::Validation(format!(
            "{} must be non-negative with at most 4 decimal places",
            field
        )));
    }
    Ok(d)
}
fn amount(value: &str, field: &str) -> Result<String, OpenTallyError> {
    let d = decimal(value, field)?;
    Ok(d.normalize().to_string())
}
async fn audit(
    pool: &SqlitePool,
    company: &str,
    entity: &str,
    id: &str,
    action: &str,
) -> Result<(), OpenTallyError> {
    sqlx::query("INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string()).bind(company).bind(entity).bind(id).bind(action).bind("system").execute(pool).await?;
    Ok(())
}
async fn unique(
    pool: &SqlitePool,
    table: &str,
    company: &str,
    field: &str,
    value: &str,
    except: Option<&str>,
) -> Result<(), OpenTallyError> {
    let sql = format!(
        "SELECT id FROM {} WHERE company_id = ? AND {} = ? COLLATE NOCASE {} LIMIT 1",
        table,
        field,
        if except.is_some() { "AND id <> ?" } else { "" }
    );
    let mut q = sqlx::query_scalar::<_, String>(&sql)
        .bind(company)
        .bind(value);
    if let Some(id) = except {
        q = q.bind(id);
    }
    if q.fetch_optional(pool).await?.is_some() {
        return Err(OpenTallyError::Validation(format!(
            "{} '{}' already exists in this company",
            field, value
        )));
    }
    Ok(())
}
async fn exists(
    pool: &SqlitePool,
    table: &str,
    company: &str,
    id: &str,
    field: &str,
) -> Result<(), OpenTallyError> {
    let sql = format!("SELECT id FROM {} WHERE company_id = ? AND id = ?", table);
    if sqlx::query_scalar::<_, String>(&sql)
        .bind(company)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .is_none()
    {
        return Err(OpenTallyError::Validation(format!(
            "{} '{}' was not found in this company",
            field, id
        )));
    }
    Ok(())
}
async fn cycle(
    pool: &SqlitePool,
    table: &str,
    company: &str,
    id: &str,
    parent: Option<&str>,
) -> Result<(), OpenTallyError> {
    let mut current = parent.map(str::to_string);
    for _ in 0..128 {
        let Some(candidate) = current else {
            return Ok(());
        };
        if candidate == id {
            return Err(OpenTallyError::Validation(
                "Parent relationship would create a circular hierarchy".into(),
            ));
        }
        let sql = format!(
            "SELECT parent_id FROM {} WHERE company_id = ? AND id = ?",
            table
        );
        current = sqlx::query_scalar::<_, Option<String>>(&sql)
            .bind(company)
            .bind(candidate)
            .fetch_optional(pool)
            .await?
            .flatten();
    }
    Err(OpenTallyError::Validation(
        "Hierarchy exceeds the supported depth".into(),
    ))
}

pub struct InventoryService;
impl InventoryService {
    pub async fn list_stock_groups(
        pool: &SqlitePool,
        c: &str,
        search: Option<&str>,
    ) -> Result<Vec<StockGroup>, OpenTallyError> {
        let p = search.map(|s| format!("%{}%", s.trim()));
        Ok(sqlx::query_as::<_, StockGroup>("SELECT id, company_id, name, parent_id, description, is_active, created_at, updated_at FROM stock_groups WHERE company_id = ? AND (? IS NULL OR name LIKE ? COLLATE NOCASE) ORDER BY name, id").bind(c).bind(p.as_deref()).bind(p.as_deref()).fetch_all(pool).await?)
    }
    pub async fn create_stock_group(
        pool: &SqlitePool,
        c: &str,
        r: StockGroupRequest,
    ) -> Result<StockGroup, OpenTallyError> {
        let n = name(&r.name, "Stock group name")?;
        unique(pool, "stock_groups", c, "name", &n, None).await?;
        if let Some(p) = &r.parent_id {
            exists(pool, "stock_groups", c, p, "Parent stock group").await?;
            cycle(pool, "stock_groups", c, "", Some(p)).await?;
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO stock_groups (id,company_id,name,parent_id,description) VALUES (?,?,?,?,?)").bind(&id).bind(c).bind(&n).bind(&r.parent_id).bind(&r.description).execute(pool).await?;
        audit(pool, c, "stock_group", &id, "CREATE").await?;
        Self::get_stock_group(pool, c, &id).await
    }
    pub async fn get_stock_group(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<StockGroup, OpenTallyError> {
        sqlx::query_as::<_,StockGroup>("SELECT id,company_id,name,parent_id,description,is_active,created_at,updated_at FROM stock_groups WHERE company_id=? AND id=?").bind(c).bind(id).fetch_optional(pool).await?.ok_or_else(||OpenTallyError::NotFound(format!("Stock group '{}' not found",id)))
    }
    pub async fn update_stock_group(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        r: StockGroupRequest,
    ) -> Result<StockGroup, OpenTallyError> {
        let old = Self::get_stock_group(pool, c, id).await?;
        let n = name(&r.name, "Stock group name")?;
        unique(pool, "stock_groups", c, "name", &n, Some(id)).await?;
        if let Some(p) = &r.parent_id {
            exists(pool, "stock_groups", c, p, "Parent stock group").await?;
            cycle(pool, "stock_groups", c, id, Some(p)).await?;
        }
        sqlx::query("UPDATE stock_groups SET name=?,parent_id=?,description=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(n).bind(r.parent_id).bind(r.description).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_group", id, "UPDATE").await?;
        Self::get_stock_group(pool, c, &old.id).await
    }
    pub async fn set_stock_group_active(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        active: bool,
    ) -> Result<StockGroup, OpenTallyError> {
        Self::get_stock_group(pool, c, id).await?;
        sqlx::query("UPDATE stock_groups SET is_active=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(active).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_group", id, "UPDATE").await?;
        Self::get_stock_group(pool, c, id).await
    }
    pub async fn delete_stock_group(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<(), OpenTallyError> {
        let g = Self::get_stock_group(pool, c, id).await?;
        let children: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_groups WHERE company_id=? AND parent_id=?",
        )
        .bind(c)
        .bind(id)
        .fetch_one(pool)
        .await?;
        let items: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_items WHERE company_id=? AND stock_group_id=?",
        )
        .bind(c)
        .bind(id)
        .fetch_one(pool)
        .await?;
        if children > 0 || items > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete stock group '{}': it contains dependent records",
                g.name
            )));
        }
        sqlx::query("DELETE FROM stock_groups WHERE company_id=? AND id=?")
            .bind(c)
            .bind(id)
            .execute(pool)
            .await?;
        audit(pool, c, "stock_group", id, "DELETE").await
    }

    pub async fn list_stock_categories(
        pool: &SqlitePool,
        c: &str,
        search: Option<&str>,
    ) -> Result<Vec<StockCategory>, OpenTallyError> {
        let p = search.map(|s| format!("%{}%", s.trim()));
        Ok(sqlx::query_as::<_,StockCategory>("SELECT id,company_id,name,description,is_active,created_at,updated_at FROM stock_categories WHERE company_id=? AND (? IS NULL OR name LIKE ? COLLATE NOCASE) ORDER BY name,id").bind(c).bind(p.as_deref()).bind(p.as_deref()).fetch_all(pool).await?)
    }
    pub async fn create_stock_category(
        pool: &SqlitePool,
        c: &str,
        r: StockCategoryRequest,
    ) -> Result<StockCategory, OpenTallyError> {
        let n = name(&r.name, "Stock category name")?;
        unique(pool, "stock_categories", c, "name", &n, None).await?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO stock_categories(id,company_id,name,description) VALUES(?,?,?,?)")
            .bind(&id)
            .bind(c)
            .bind(n)
            .bind(r.description)
            .execute(pool)
            .await?;
        audit(pool, c, "stock_category", &id, "CREATE").await?;
        Self::get_stock_category(pool, c, &id).await
    }
    pub async fn get_stock_category(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<StockCategory, OpenTallyError> {
        sqlx::query_as::<_,StockCategory>("SELECT id,company_id,name,description,is_active,created_at,updated_at FROM stock_categories WHERE company_id=? AND id=?").bind(c).bind(id).fetch_optional(pool).await?.ok_or_else(||OpenTallyError::NotFound(format!("Stock category '{}' not found",id)))
    }
    pub async fn update_stock_category(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        r: StockCategoryRequest,
    ) -> Result<StockCategory, OpenTallyError> {
        Self::get_stock_category(pool, c, id).await?;
        let n = name(&r.name, "Stock category name")?;
        unique(pool, "stock_categories", c, "name", &n, Some(id)).await?;
        sqlx::query("UPDATE stock_categories SET name=?,description=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(n).bind(r.description).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_category", id, "UPDATE").await?;
        Self::get_stock_category(pool, c, id).await
    }
    pub async fn set_stock_category_active(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        a: bool,
    ) -> Result<StockCategory, OpenTallyError> {
        Self::get_stock_category(pool, c, id).await?;
        sqlx::query("UPDATE stock_categories SET is_active=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(a).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_category", id, "UPDATE").await?;
        Self::get_stock_category(pool, c, id).await
    }
    pub async fn delete_stock_category(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<(), OpenTallyError> {
        let x = Self::get_stock_category(pool, c, id).await?;
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM stock_items WHERE company_id=? AND stock_category_id=?",
        )
        .bind(c)
        .bind(id)
        .fetch_one(pool)
        .await?;
        if n > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete stock category '{}': it is used by stock items",
                x.name
            )));
        }
        sqlx::query("DELETE FROM stock_categories WHERE company_id=? AND id=?")
            .bind(c)
            .bind(id)
            .execute(pool)
            .await?;
        audit(pool, c, "stock_category", id, "DELETE").await
    }

    pub async fn list_units(
        pool: &SqlitePool,
        c: &str,
        search: Option<&str>,
    ) -> Result<Vec<Unit>, OpenTallyError> {
        let p = search.map(|s| format!("%{}%", s.trim()));
        Ok(sqlx::query_as::<_,Unit>("SELECT id,company_id,symbol,formal_name,unit_type,base_unit_id,secondary_unit_id,conversion_factor,decimal_precision,is_active,created_at,updated_at FROM units WHERE company_id=? AND (? IS NULL OR symbol LIKE ? COLLATE NOCASE OR formal_name LIKE ? COLLATE NOCASE) ORDER BY symbol,id").bind(c).bind(p.as_deref()).bind(p.as_deref()).bind(p.as_deref()).fetch_all(pool).await?)
    }
    pub async fn create_unit(
        pool: &SqlitePool,
        c: &str,
        r: UnitRequest,
    ) -> Result<Unit, OpenTallyError> {
        let s = name(&r.symbol, "Unit symbol")?;
        let f = name(&r.formal_name, "Unit formal name")?;
        if !(0..=4).contains(&r.decimal_precision) {
            return Err(OpenTallyError::Validation(
                "Unit precision must be between 0 and 4".into(),
            ));
        }
        unique(pool, "units", c, "symbol", &s, None).await?;
        unique(pool, "units", c, "formal_name", &f, None).await?;
        let t = r.unit_type.unwrap_or_else(|| "simple".into());
        if t == "compound" {
            let b = r.base_unit_id.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Compound units require a base unit".into())
            })?;
            let sec = r.secondary_unit_id.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Compound units require a secondary unit".into())
            })?;
            if b == sec {
                return Err(OpenTallyError::Validation(
                    "Compound unit components must differ".into(),
                ));
            }
            exists(pool, "units", c, b, "Base unit").await?;
            exists(pool, "units", c, sec, "Secondary unit").await?;
            decimal(
                r.conversion_factor.as_deref().unwrap_or(""),
                "Conversion factor",
            )?;
        } else if t != "simple" {
            return Err(OpenTallyError::Validation(
                "Unit type must be simple or compound".into(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,unit_type,base_unit_id,secondary_unit_id,conversion_factor,decimal_precision) VALUES(?,?,?,?,?,?,?,?,?)").bind(&id).bind(c).bind(s).bind(f).bind(t).bind(r.base_unit_id).bind(r.secondary_unit_id).bind(r.conversion_factor).bind(r.decimal_precision).execute(pool).await?;
        audit(pool, c, "unit", &id, "CREATE").await?;
        Self::get_unit(pool, c, &id).await
    }
    pub async fn get_unit(pool: &SqlitePool, c: &str, id: &str) -> Result<Unit, OpenTallyError> {
        sqlx::query_as::<_,Unit>("SELECT id,company_id,symbol,formal_name,unit_type,base_unit_id,secondary_unit_id,conversion_factor,decimal_precision,is_active,created_at,updated_at FROM units WHERE company_id=? AND id=?").bind(c).bind(id).fetch_optional(pool).await?.ok_or_else(||OpenTallyError::NotFound(format!("Unit '{}' not found",id)))
    }
    pub async fn update_unit(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        r: UnitRequest,
    ) -> Result<Unit, OpenTallyError> {
        Self::get_unit(pool, c, id).await?;
        let s = name(&r.symbol, "Unit symbol")?;
        let f = name(&r.formal_name, "Unit formal name")?;
        if !(0..=4).contains(&r.decimal_precision) {
            return Err(OpenTallyError::Validation(
                "Unit precision must be between 0 and 4".into(),
            ));
        }
        unique(pool, "units", c, "symbol", &s, Some(id)).await?;
        unique(pool, "units", c, "formal_name", &f, Some(id)).await?;
        let t = r.unit_type.unwrap_or_else(|| "simple".into());
        if t == "compound" {
            let b = r.base_unit_id.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Compound units require a base unit".into())
            })?;
            let sec = r.secondary_unit_id.as_deref().ok_or_else(|| {
                OpenTallyError::Validation("Compound units require a secondary unit".into())
            })?;
            exists(pool, "units", c, b, "Base unit").await?;
            exists(pool, "units", c, sec, "Secondary unit").await?;
            decimal(
                r.conversion_factor.as_deref().unwrap_or(""),
                "Conversion factor",
            )?;
        }
        sqlx::query("UPDATE units SET symbol=?,formal_name=?,unit_type=?,base_unit_id=?,secondary_unit_id=?,conversion_factor=?,decimal_precision=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(s).bind(f).bind(t).bind(r.base_unit_id).bind(r.secondary_unit_id).bind(r.conversion_factor).bind(r.decimal_precision).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "unit", id, "UPDATE").await?;
        Self::get_unit(pool, c, id).await
    }
    pub async fn set_unit_active(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        a: bool,
    ) -> Result<Unit, OpenTallyError> {
        Self::get_unit(pool, c, id).await?;
        sqlx::query(
            "UPDATE units SET is_active=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?",
        )
        .bind(a)
        .bind(c)
        .bind(id)
        .execute(pool)
        .await?;
        audit(pool, c, "unit", id, "UPDATE").await?;
        Self::get_unit(pool, c, id).await
    }
    pub async fn delete_unit(pool: &SqlitePool, c: &str, id: &str) -> Result<(), OpenTallyError> {
        let u = Self::get_unit(pool, c, id).await?;
        let n:i64=sqlx::query_scalar("SELECT COUNT(*) FROM stock_items WHERE company_id=? AND (base_unit_id=? OR alternate_unit_id=?)").bind(c).bind(id).bind(id).fetch_one(pool).await?;
        if n > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete unit '{}': it is used by stock items",
                u.symbol
            )));
        }
        sqlx::query("DELETE FROM units WHERE company_id=? AND id=?")
            .bind(c)
            .bind(id)
            .execute(pool)
            .await?;
        audit(pool, c, "unit", id, "DELETE").await
    }

    pub async fn list_stock_items(
        pool: &SqlitePool,
        c: &str,
        search: Option<&str>,
    ) -> Result<Vec<StockItem>, OpenTallyError> {
        let p = search.map(|s| format!("%{}%", s.trim()));
        Ok(sqlx::query_as::<_,StockItem>("SELECT id,company_id,name,stock_group_id,stock_category_id,base_unit_id,alternate_unit_id,sku,description,opening_quantity,opening_rate,opening_value,reorder_level,is_active,created_at,updated_at FROM stock_items WHERE company_id=? AND (? IS NULL OR name LIKE ? COLLATE NOCASE OR sku LIKE ? COLLATE NOCASE) ORDER BY name,id").bind(c).bind(p.as_deref()).bind(p.as_deref()).bind(p.as_deref()).fetch_all(pool).await?)
    }
    async fn validate_item(
        pool: &SqlitePool,
        c: &str,
        r: &StockItemRequest,
        except: Option<&str>,
    ) -> Result<(String, String, String, String), OpenTallyError> {
        let n = name(&r.name, "Stock item name")?;
        unique(pool, "stock_items", c, "name", &n, except).await?;
        if let Some(s) = r.sku.as_deref().filter(|x| !x.trim().is_empty()) {
            unique(pool, "stock_items", c, "sku", s.trim(), except).await?;
        }
        exists(pool, "stock_groups", c, &r.stock_group_id, "Stock group").await?;
        exists(pool, "units", c, &r.base_unit_id, "Base unit").await?;
        if let Some(x) = &r.stock_category_id {
            exists(pool, "stock_categories", c, x, "Stock category").await?;
        }
        if let Some(x) = &r.alternate_unit_id {
            exists(pool, "units", c, x, "Alternate unit").await?;
        }
        let q = amount(&r.opening_quantity, "Opening quantity")?;
        let rate = amount(&r.opening_rate, "Opening rate")?;
        let value = amount(&r.opening_value, "Opening value")?;
        let _reorder = amount(&r.reorder_level, "Reorder level")?;
        if q.parse::<Decimal>().unwrap_or_default() * rate.parse::<Decimal>().unwrap_or_default()
            != value.parse::<Decimal>().unwrap_or_default()
        {
            return Err(OpenTallyError::Validation("Opening value must equal opening quantity multiplied by opening rate exactly; no implicit rounding is applied".into()));
        }
        Ok((n, q, rate, value))
    }
    pub async fn create_stock_item(
        pool: &SqlitePool,
        c: &str,
        r: StockItemRequest,
    ) -> Result<StockItem, OpenTallyError> {
        let (n, q, rate, value) = Self::validate_item(pool, c, &r, None).await?;
        let reorder = amount(&r.reorder_level, "Reorder level")?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,stock_category_id,base_unit_id,alternate_unit_id,sku,description,opening_quantity,opening_rate,opening_value,reorder_level) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(c).bind(n).bind(r.stock_group_id).bind(r.stock_category_id).bind(r.base_unit_id).bind(r.alternate_unit_id).bind(r.sku.map(|x|x.trim().to_string())).bind(r.description).bind(q).bind(rate).bind(value).bind(reorder).execute(pool).await?;
        audit(pool, c, "stock_item", &id, "CREATE").await?;
        Self::get_stock_item(pool, c, &id).await
    }
    pub async fn get_stock_item(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<StockItem, OpenTallyError> {
        sqlx::query_as::<_,StockItem>("SELECT id,company_id,name,stock_group_id,stock_category_id,base_unit_id,alternate_unit_id,sku,description,opening_quantity,opening_rate,opening_value,reorder_level,is_active,created_at,updated_at FROM stock_items WHERE company_id=? AND id=?").bind(c).bind(id).fetch_optional(pool).await?.ok_or_else(||OpenTallyError::NotFound(format!("Stock item '{}' not found",id)))
    }
    pub async fn update_stock_item(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        r: StockItemRequest,
    ) -> Result<StockItem, OpenTallyError> {
        Self::get_stock_item(pool, c, id).await?;
        let (n, q, rate, value) = Self::validate_item(pool, c, &r, Some(id)).await?;
        let reorder = amount(&r.reorder_level, "Reorder level")?;
        sqlx::query("UPDATE stock_items SET name=?,stock_group_id=?,stock_category_id=?,base_unit_id=?,alternate_unit_id=?,sku=?,description=?,opening_quantity=?,opening_rate=?,opening_value=?,reorder_level=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(n).bind(r.stock_group_id).bind(r.stock_category_id).bind(r.base_unit_id).bind(r.alternate_unit_id).bind(r.sku.map(|x|x.trim().to_string())).bind(r.description).bind(q).bind(rate).bind(value).bind(reorder).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_item", id, "UPDATE").await?;
        Self::get_stock_item(pool, c, id).await
    }
    pub async fn set_stock_item_active(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        a: bool,
    ) -> Result<StockItem, OpenTallyError> {
        Self::get_stock_item(pool, c, id).await?;
        sqlx::query("UPDATE stock_items SET is_active=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(a).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "stock_item", id, "UPDATE").await?;
        Self::get_stock_item(pool, c, id).await
    }
    pub async fn delete_stock_item(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<(), OpenTallyError> {
        Self::get_stock_item(pool, c, id).await?;
        sqlx::query("DELETE FROM stock_items WHERE company_id=? AND id=?")
            .bind(c)
            .bind(id)
            .execute(pool)
            .await?;
        audit(pool, c, "stock_item", id, "DELETE").await
    }

    pub async fn list_godowns(
        pool: &SqlitePool,
        c: &str,
        search: Option<&str>,
    ) -> Result<Vec<Godown>, OpenTallyError> {
        let p = search.map(|s| format!("%{}%", s.trim()));
        Ok(sqlx::query_as::<_,Godown>("SELECT id,company_id,name,parent_id,description,is_active,created_at,updated_at,address,is_external FROM godowns WHERE company_id=? AND (? IS NULL OR name LIKE ? COLLATE NOCASE) ORDER BY name,id").bind(c).bind(p.as_deref()).bind(p.as_deref()).fetch_all(pool).await?)
    }
    pub async fn create_godown(
        pool: &SqlitePool,
        c: &str,
        r: GodownRequest,
    ) -> Result<Godown, OpenTallyError> {
        let n = name(&r.name, "Godown name")?;
        unique(pool, "godowns", c, "name", &n, None).await?;
        if let Some(p) = &r.parent_id {
            exists(pool, "godowns", c, p, "Parent godown").await?;
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO godowns(id,company_id,name,parent_id,description,address,is_external) VALUES(?,?,?,?,?,?,?)",
        )
        .bind(&id)
        .bind(c)
        .bind(n)
        .bind(&r.parent_id)
        .bind(r.description)
        .bind(r.address)
        .bind(r.is_external)
        .execute(pool)
        .await?;
        audit(pool, c, "godown", &id, "CREATE").await?;
        Self::get_godown(pool, c, &id).await
    }
    pub async fn get_godown(
        pool: &SqlitePool,
        c: &str,
        id: &str,
    ) -> Result<Godown, OpenTallyError> {
        sqlx::query_as::<_,Godown>("SELECT id,company_id,name,parent_id,description,is_active,created_at,updated_at,address,is_external FROM godowns WHERE company_id=? AND id=?").bind(c).bind(id).fetch_optional(pool).await?.ok_or_else(||OpenTallyError::NotFound(format!("Godown '{}' not found",id)))
    }
    pub async fn update_godown(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        r: GodownRequest,
    ) -> Result<Godown, OpenTallyError> {
        Self::get_godown(pool, c, id).await?;
        let n = name(&r.name, "Godown name")?;
        unique(pool, "godowns", c, "name", &n, Some(id)).await?;
        if let Some(p) = &r.parent_id {
            exists(pool, "godowns", c, p, "Parent godown").await?;
            cycle(pool, "godowns", c, id, Some(p)).await?;
        }
        sqlx::query("UPDATE godowns SET name=?,parent_id=?,description=?,address=?,is_external=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(n).bind(r.parent_id).bind(r.description).bind(r.address).bind(r.is_external).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "godown", id, "UPDATE").await?;
        Self::get_godown(pool, c, id).await
    }
    pub async fn set_godown_active(
        pool: &SqlitePool,
        c: &str,
        id: &str,
        a: bool,
    ) -> Result<Godown, OpenTallyError> {
        Self::get_godown(pool, c, id).await?;
        sqlx::query("UPDATE godowns SET is_active=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND id=?").bind(a).bind(c).bind(id).execute(pool).await?;
        audit(pool, c, "godown", id, "UPDATE").await?;
        Self::get_godown(pool, c, id).await
    }
    pub async fn delete_godown(pool: &SqlitePool, c: &str, id: &str) -> Result<(), OpenTallyError> {
        let g = Self::get_godown(pool, c, id).await?;
        let n: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM godowns WHERE company_id=? AND parent_id=?")
                .bind(c)
                .bind(id)
                .fetch_one(pool)
                .await?;
        if n > 0 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete godown '{}': it contains child godowns",
                g.name
            )));
        }
        sqlx::query("DELETE FROM godowns WHERE company_id=? AND id=?")
            .bind(c)
            .bind(id)
            .execute(pool)
            .await?;
        audit(pool, c, "godown", id, "DELETE").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use sqlx::Row;

    async fn fixture() -> DatabaseManager {
        let db = DatabaseManager::init(std::path::Path::new(":memory:"))
            .await
            .unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('c1','Inventory Co','Inventory Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        db
    }
    #[tokio::test]
    async fn migration_and_company_isolation() {
        let db = fixture().await;
        let p = db.pool();
        let g = InventoryService::create_stock_group(
            p,
            "c1",
            StockGroupRequest {
                name: "Raw Materials".into(),
                parent_id: None,
                description: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            InventoryService::list_stock_groups(p, "other", None)
                .await
                .unwrap()
                .len(),
            0
        );
        assert!(InventoryService::create_stock_group(
            p,
            "c1",
            StockGroupRequest {
                name: "raw materials".into(),
                parent_id: None,
                description: None
            }
        )
        .await
        .is_err());
        let version: String =
            sqlx::query("SELECT schema_version FROM _opentally_metadata WHERE id=1")
                .fetch_one(p)
                .await
                .unwrap()
                .get(0);
        assert_eq!(version, "0.4.0-sprint4");
        assert!(g.is_active);
    }
    #[tokio::test]
    async fn item_decimal_consistency_and_deletion_guards() {
        let db = fixture().await;
        let p = db.pool();
        let g = InventoryService::create_stock_group(
            p,
            "c1",
            StockGroupRequest {
                name: "Goods".into(),
                parent_id: None,
                description: None,
            },
        )
        .await
        .unwrap();
        let u = InventoryService::create_unit(
            p,
            "c1",
            UnitRequest {
                symbol: "Kg".into(),
                formal_name: "Kilogram".into(),
                decimal_precision: 3,
                unit_type: None,
                base_unit_id: None,
                secondary_unit_id: None,
                conversion_factor: None,
            },
        )
        .await
        .unwrap();
        let item = InventoryService::create_stock_item(
            p,
            "c1",
            StockItemRequest {
                name: "Rice".into(),
                stock_group_id: g.id.clone(),
                stock_category_id: None,
                base_unit_id: u.id.clone(),
                alternate_unit_id: None,
                sku: Some("R-1".into()),
                description: None,
                opening_quantity: "2.50".into(),
                opening_rate: "10.00".into(),
                opening_value: "25.00".into(),
                reorder_level: "1.00".into(),
            },
        )
        .await
        .unwrap();
        assert!(InventoryService::create_stock_item(
            p,
            "c1",
            StockItemRequest {
                name: "Bad".into(),
                stock_group_id: g.id.clone(),
                stock_category_id: None,
                base_unit_id: u.id.clone(),
                alternate_unit_id: None,
                sku: None,
                description: None,
                opening_quantity: "2".into(),
                opening_rate: "10".into(),
                opening_value: "21".into(),
                reorder_level: "0".into()
            }
        )
        .await
        .is_err());
        assert!(InventoryService::delete_stock_group(p, "c1", &g.id)
            .await
            .is_err());
        InventoryService::set_stock_item_active(p, "c1", &item.id, false)
            .await
            .unwrap();
        let audits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE company_id='c1' AND entity_type='stock_item'",
        )
        .fetch_one(p)
        .await
        .unwrap();
        assert!(audits >= 2);
    }
    #[tokio::test]
    async fn hierarchy_cycle_is_rejected() {
        let db = fixture().await;
        let p = db.pool();
        let a = InventoryService::create_stock_group(
            p,
            "c1",
            StockGroupRequest {
                name: "A".into(),
                parent_id: None,
                description: None,
            },
        )
        .await
        .unwrap();
        let b = InventoryService::create_stock_group(
            p,
            "c1",
            StockGroupRequest {
                name: "B".into(),
                parent_id: Some(a.id.clone()),
                description: None,
            },
        )
        .await
        .unwrap();
        assert!(InventoryService::update_stock_group(
            p,
            "c1",
            &a.id,
            StockGroupRequest {
                name: "A".into(),
                parent_id: Some(b.id),
                description: None
            }
        )
        .await
        .is_err());
    }
}
