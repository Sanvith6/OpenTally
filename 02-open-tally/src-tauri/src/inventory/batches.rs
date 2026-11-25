use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StockItemBatch {
    pub id: String,
    pub company_id: String,
    pub stock_item_id: String,
    pub batch_number: String,
    pub mfg_date: Option<String>,
    pub expiry_date: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockItemBatchRequest {
    pub stock_item_id: String,
    pub batch_number: String,
    #[serde(default)]
    pub mfg_date: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExpiryReportRow {
    pub batch_id: String,
    pub stock_item_id: String,
    pub stock_item_name: String,
    pub batch_number: String,
    pub mfg_date: Option<String>,
    pub expiry_date: Option<String>,
    pub closing_quantity: String,
    pub unit_cost: String,
    pub closing_value: String,
    pub days_until_expiry: Option<i64>,
    pub status: String,
}

pub struct BatchService;

impl BatchService {
    /// List all batches for a company, optionally filtered by stock item
    pub async fn list_batches(
        pool: &Pool<Sqlite>,
        company_id: &str,
        stock_item_id: Option<&str>,
    ) -> Result<Vec<StockItemBatch>, OpenTallyError> {
        let rows = sqlx::query_as::<_, StockItemBatch>(
            "SELECT id, company_id, stock_item_id, batch_number, mfg_date, expiry_date, is_active, created_at, updated_at
             FROM stock_item_batches
             WHERE company_id = ? AND (? IS NULL OR stock_item_id = ?)
             ORDER BY stock_item_id, batch_number",
        )
        .bind(company_id)
        .bind(stock_item_id)
        .bind(stock_item_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }

    /// Explicitly create a new batch record
    pub async fn create_batch(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: StockItemBatchRequest,
    ) -> Result<StockItemBatch, OpenTallyError> {
        let batch_num = req.batch_number.trim();
        if batch_num.is_empty() {
            return Err(OpenTallyError::Validation(
                "Batch number cannot be empty".to_string(),
            ));
        }

        // Validate date formats
        if let Some(mfg) = &req.mfg_date {
            NaiveDate::from_str(mfg).map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Invalid mfg_date format '{}', expected YYYY-MM-DD",
                    mfg
                ))
            })?;
        }
        if let Some(exp) = &req.expiry_date {
            NaiveDate::from_str(exp).map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Invalid expiry_date format '{}', expected YYYY-MM-DD",
                    exp
                ))
            })?;
        }
        if let (Some(mfg), Some(exp)) = (&req.mfg_date, &req.expiry_date) {
            if mfg > exp {
                return Err(OpenTallyError::Validation(
                    "Manufacturing date cannot be after expiry date".to_string(),
                ));
            }
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_item_batches (
                id, company_id, stock_item_id, batch_number, mfg_date, expiry_date, is_active
             ) VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(&req.stock_item_id)
        .bind(batch_num)
        .bind(&req.mfg_date)
        .bind(&req.expiry_date)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let batch = sqlx::query_as::<_, StockItemBatch>(
            "SELECT id, company_id, stock_item_id, batch_number, mfg_date, expiry_date, is_active, created_at, updated_at
             FROM stock_item_batches WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(batch)
    }

    /// Get existing batch ID or create a new one inside an active transaction (e.g. during F9 Purchase)
    pub async fn get_or_create_batch(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        stock_item_id: &str,
        batch_number: &str,
        mfg_date: Option<&str>,
        expiry_date: Option<&str>,
    ) -> Result<String, OpenTallyError> {
        let batch_num = batch_number.trim();
        if batch_num.is_empty() {
            return Err(OpenTallyError::Validation(
                "Batch number cannot be empty".to_string(),
            ));
        }

        let existing: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT id, mfg_date, expiry_date FROM stock_item_batches
             WHERE company_id = ? AND stock_item_id = ? AND batch_number = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(stock_item_id)
        .bind(batch_num)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((id, cur_mfg, cur_exp)) = existing {
            // Update mfg/expiry if provided and previously null
            if (cur_mfg.is_none() && mfg_date.is_some())
                || (cur_exp.is_none() && expiry_date.is_some())
            {
                let new_mfg = mfg_date.or(cur_mfg.as_deref());
                let new_exp = expiry_date.or(cur_exp.as_deref());
                sqlx::query(
                    "UPDATE stock_item_batches SET mfg_date = ?, expiry_date = ?, updated_at = CURRENT_TIMESTAMP
                     WHERE company_id = ? AND id = ?",
                )
                .bind(new_mfg)
                .bind(new_exp)
                .bind(company_id)
                .bind(&id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }
            return Ok(id);
        }

        // Validate date formats
        if let Some(mfg) = mfg_date {
            NaiveDate::from_str(mfg).map_err(|_| {
                OpenTallyError::Validation(format!("Invalid mfg_date format '{}'", mfg))
            })?;
        }
        if let Some(exp) = expiry_date {
            NaiveDate::from_str(exp).map_err(|_| {
                OpenTallyError::Validation(format!("Invalid expiry_date format '{}'", exp))
            })?;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_item_batches (
                id, company_id, stock_item_id, batch_number, mfg_date, expiry_date, is_active
             ) VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(stock_item_id)
        .bind(batch_num)
        .bind(mfg_date)
        .bind(expiry_date)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(id)
    }

    /// Validate batch exists and is NOT expired on voucher date for F8 Sales
    pub async fn validate_batch_expiry_for_sale(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        stock_item_id: &str,
        batch_number: &str,
        voucher_date: &str,
    ) -> Result<String, OpenTallyError> {
        let batch_num = batch_number.trim();
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT id, expiry_date FROM stock_item_batches
             WHERE company_id = ? AND stock_item_id = ? AND batch_number = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(stock_item_id)
        .bind(batch_num)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let (batch_id, expiry_date) = row.ok_or_else(|| {
            OpenTallyError::Validation(format!(
                "Batch '{}' does not exist for this stock item",
                batch_num
            ))
        })?;

        if let Some(exp) = expiry_date {
            if voucher_date > exp.as_str() {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot sell expired batch '{}': expired on {}, voucher date is {}",
                    batch_num, exp, voucher_date
                )));
            }
        }

        Ok(batch_id)
    }

    /// Generate comprehensive Batch Expiry & Aging Report
    pub async fn get_batch_expiry_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        stock_item_id_filter: Option<&str>,
        as_of_date: &str,
    ) -> Result<Vec<BatchExpiryReportRow>, OpenTallyError> {
        let as_of = NaiveDate::from_str(as_of_date).map_err(|_| {
            OpenTallyError::Validation(format!("Invalid as_of_date '{}'", as_of_date))
        })?;

        let cid = company_id.to_string();
        let item_filter = stock_item_id_filter.map(|s| s.to_string());

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                #[derive(FromRow)]
                struct RawBatchQuery {
                    batch_id: String,
                    stock_item_id: String,
                    stock_item_name: String,
                    batch_number: String,
                    mfg_date: Option<String>,
                    expiry_date: Option<String>,
                }

                let rows = sqlx::query_as::<_, RawBatchQuery>(
                    r#"
                    SELECT 
                        b.id as batch_id,
                        b.stock_item_id,
                        si.name as stock_item_name,
                        b.batch_number,
                        b.mfg_date,
                        b.expiry_date
                    FROM stock_item_batches b
                    JOIN stock_items si ON si.company_id = b.company_id AND si.id = b.stock_item_id
                    WHERE b.company_id = ? AND (? IS NULL OR b.stock_item_id = ?)
                    ORDER BY b.expiry_date ASC, b.batch_number ASC
                    "#,
                )
                .bind(&cid)
                .bind(&item_filter)
                .bind(&item_filter)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut report = Vec::new();
                for r in rows {
                    let cost_layers: Vec<(String, String)> = sqlx::query_as(
                        "SELECT remaining_quantity, unit_cost FROM inventory_cost_layers WHERE company_id = ? AND batch_id = ?"
                    )
                    .bind(&cid)
                    .bind(&r.batch_id)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();

                    let mut qty = Decimal::ZERO;
                    let mut val = Decimal::ZERO;
                    for (l_qty_str, l_cost_str) in cost_layers {
                        let l_qty = Decimal::from_str(&l_qty_str).unwrap_or(Decimal::ZERO);
                        let l_cost = Decimal::from_str(&l_cost_str).unwrap_or(Decimal::ZERO);
                        qty += l_qty;
                        val += l_qty * l_cost;
                    }

                    let unit_cost = if qty > Decimal::ZERO {
                        (val / qty).round_dp(2).normalize().to_string()
                    } else {
                        "0.00".to_string()
                    };

                    let (days_until_expiry, status) = if let Some(exp_str) = &r.expiry_date {
                        if let Ok(exp_date) = NaiveDate::from_str(exp_str) {
                            let diff = (exp_date - as_of).num_days();
                            let st = if diff < 0 {
                                "Expired".to_string()
                            } else if diff <= 30 {
                                "ExpiringIn30Days".to_string()
                            } else if diff <= 60 {
                                "ExpiringIn60Days".to_string()
                            } else if diff <= 90 {
                                "ExpiringIn90Days".to_string()
                            } else {
                                "Good".to_string()
                            };
                            (Some(diff), st)
                        } else {
                            (None, "NoExpiry".to_string())
                        }
                    } else {
                        (None, "NoExpiry".to_string())
                    };

                    report.push(BatchExpiryReportRow {
                        batch_id: r.batch_id,
                        stock_item_id: r.stock_item_id,
                        stock_item_name: r.stock_item_name,
                        batch_number: r.batch_number,
                        mfg_date: r.mfg_date,
                        expiry_date: r.expiry_date,
                        closing_quantity: qty.normalize().to_string(),
                        unit_cost,
                        closing_value: val.round_dp(2).normalize().to_string(),
                        days_until_expiry,
                        status,
                    });
                }

                Ok(report)
            })
        })
        .await
    }
}
