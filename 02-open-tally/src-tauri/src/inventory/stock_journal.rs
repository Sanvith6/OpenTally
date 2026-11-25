use crate::errors::OpenTallyError;
use crate::inventory::batches::BatchService;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockJournalLine {
    pub stock_item_id: String,
    pub godown_id: String,
    #[serde(default)]
    pub batch_number: Option<String>,
    pub quantity: Decimal,
    pub rate: Decimal,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockJournalRequest {
    pub voucher_number: Option<String>,
    pub series_id: Option<String>,
    pub voucher_date: String,
    #[serde(default)]
    pub narration: Option<String>,
    pub source_lines: Vec<StockJournalLine>,
    pub destination_lines: Vec<StockJournalLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockJournalResponse {
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub total_source_value: String,
    pub total_destination_value: String,
    pub lines_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodownSummaryItemRow {
    pub stock_item_id: String,
    pub stock_item_name: String,
    pub base_unit: String,
    pub inflow_quantity: String,
    pub outflow_quantity: String,
    pub closing_quantity: String,
    pub average_rate: String,
    pub closing_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodownSummaryReport {
    pub godown_id: String,
    pub godown_name: String,
    pub address: Option<String>,
    pub is_external: bool,
    pub items: Vec<GodownSummaryItemRow>,
    pub total_godown_value: String,
}

pub struct StockJournalService;

impl StockJournalService {
    /// Post a complete Stock Journal voucher (Alt+F7) with atomic FIFO layer transfer
    pub async fn post_stock_journal(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: StockJournalRequest,
    ) -> Result<StockJournalResponse, OpenTallyError> {
        if req.source_lines.is_empty() && req.destination_lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "Stock journal must have at least one source or destination line".to_string(),
            ));
        }

        NaiveDate::from_str(&req.voucher_date).map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid voucher_date format '{}', expected YYYY-MM-DD",
                req.voucher_date
            ))
        })?;

        let mut tx = pool.begin().await?;

        crate::company::FinancialYearClosingService::validate_period_not_locked(
            &mut *tx,
            company_id,
            &req.voucher_date,
        )
        .await?;

        // 1. Generate or validate voucher number
        let vch_num = match req.voucher_number {
            Some(ref num) if !num.trim().is_empty() => num.trim().to_string(),
            _ => format!(
                "SJ-{}",
                Uuid::new_v4().simple().to_string()[..8].to_uppercase()
            ),
        };

        let voucher_id = Uuid::new_v4().to_string();

        // 2. Insert into stock_vouchers
        sqlx::query(
            "INSERT INTO stock_vouchers (
                id, company_id, voucher_number, voucher_type, voucher_date, narration, status
             ) VALUES (?, ?, ?, 'transfer', ?, ?, 'posted')",
        )
        .bind(&voucher_id)
        .bind(company_id)
        .bind(&vch_num)
        .bind(&req.voucher_date)
        .bind(&req.narration)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut total_consumed_cost = Decimal::ZERO;
        let mut line_num = 1;

        // 3. Process Source Lines (Consumption / Transfer Out)
        for s_line in &req.source_lines {
            if s_line.quantity <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Source quantity must be greater than zero".to_string(),
                ));
            }

            // Lookup unit
            let unit_id: String = sqlx::query_scalar(
                "SELECT base_unit_id FROM stock_items WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&s_line.stock_item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Stock item '{}' not found",
                    s_line.stock_item_id
                ))
            })?;

            // Resolve batch if present
            let batch_id = if let Some(ref b_num) = s_line.batch_number {
                let bid = BatchService::get_or_create_batch(
                    &mut tx,
                    company_id,
                    &s_line.stock_item_id,
                    b_num,
                    None,
                    None,
                )
                .await?;
                Some(bid)
            } else {
                None
            };

            // Check current stock in source godown
            let layer_qtys: Vec<String> = sqlx::query_scalar(
                "SELECT remaining_quantity FROM inventory_cost_layers
                 WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                 AND (? IS NULL OR batch_id = ?)",
            )
            .bind(company_id)
            .bind(&s_line.stock_item_id)
            .bind(&s_line.godown_id)
            .bind(batch_id.as_deref())
            .bind(batch_id.as_deref())
            .fetch_all(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let mut current_stock = Decimal::ZERO;
            for q_str in layer_qtys {
                if let Ok(q) = Decimal::from_str(&q_str) {
                    current_stock += q;
                }
            }

            if current_stock < s_line.quantity {
                return Err(OpenTallyError::Validation(format!(
                    "Insufficient stock in source godown for item '{}'. Available: {}, Requested: {}",
                    s_line.stock_item_id, current_stock, s_line.quantity
                )));
            }

            // Consume layers FIFO
            #[derive(FromRow)]
            struct LayerRow {
                id: String,
                remaining_quantity: String,
                unit_cost: String,
            }

            let available_layers: Vec<LayerRow> = sqlx::query_as(
                "SELECT id, remaining_quantity, unit_cost FROM inventory_cost_layers
                 WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                 AND (? IS NULL OR batch_id = ?) AND remaining_quantity > '0'
                 ORDER BY receipt_date ASC, created_at ASC, id ASC",
            )
            .bind(company_id)
            .bind(&s_line.stock_item_id)
            .bind(&s_line.godown_id)
            .bind(batch_id.as_deref())
            .bind(batch_id.as_deref())
            .fetch_all(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let mut needed = s_line.quantity;
            let mut line_consumed_cost = Decimal::ZERO;

            for l in available_layers {
                if needed <= Decimal::ZERO {
                    break;
                }
                let rem = Decimal::from_str(&l.remaining_quantity).unwrap_or(Decimal::ZERO);
                let u_cost = Decimal::from_str(&l.unit_cost).unwrap_or(Decimal::ZERO);
                let take = needed.min(rem);
                let new_rem = rem - take;

                sqlx::query(
                    "UPDATE inventory_cost_layers SET remaining_quantity = ?
                     WHERE company_id = ? AND id = ?",
                )
                .bind(new_rem.normalize().to_string())
                .bind(company_id)
                .bind(&l.id)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                line_consumed_cost += take * u_cost;
                needed -= take;
            }

            total_consumed_cost += line_consumed_cost;

            // Insert stock_voucher_line
            let sv_line_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_voucher_lines (
                    id, company_id, voucher_id, line_number, stock_item_id, unit_id,
                    source_godown_id, quantity, description, batch_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'Stock Journal Consumption', ?)",
            )
            .bind(&sv_line_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind(line_num)
            .bind(&s_line.stock_item_id)
            .bind(&unit_id)
            .bind(&s_line.godown_id)
            .bind(s_line.quantity.normalize().to_string())
            .bind(batch_id.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Insert stock movement transfer_out
            let mv_out_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_movements (
                    id, company_id, stock_item_id, godown_id, voucher_id, voucher_line_id,
                    movement_date, movement_type, signed_quantity, batch_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, 'transfer_out', ?, ?)",
            )
            .bind(&mv_out_id)
            .bind(company_id)
            .bind(&s_line.stock_item_id)
            .bind(&s_line.godown_id)
            .bind(&voucher_id)
            .bind(&sv_line_id)
            .bind(&req.voucher_date)
            .bind((-s_line.quantity).normalize().to_string())
            .bind(batch_id.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            line_num += 1;
        }

        // 4. Process Destination Lines (Production / Transfer In)
        let total_dest_qty: Decimal = req.destination_lines.iter().map(|l| l.quantity).sum();
        let mut total_destination_value = Decimal::ZERO;

        for d_line in &req.destination_lines {
            if d_line.quantity <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Destination quantity must be greater than zero".to_string(),
                ));
            }

            let unit_id: String = sqlx::query_scalar(
                "SELECT base_unit_id FROM stock_items WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&d_line.stock_item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Stock item '{}' not found",
                    d_line.stock_item_id
                ))
            })?;

            let batch_id = if let Some(ref b_num) = d_line.batch_number {
                let bid = BatchService::get_or_create_batch(
                    &mut tx,
                    company_id,
                    &d_line.stock_item_id,
                    b_num,
                    None,
                    None,
                )
                .await?;
                Some(bid)
            } else {
                None
            };

            // Derive unit cost from consumed source cost to ensure 100% value preservation
            let (assigned_unit_cost, assigned_val) =
                if total_consumed_cost > Decimal::ZERO && total_dest_qty > Decimal::ZERO {
                    let share_val = if req.destination_lines.len() == 1 {
                        total_consumed_cost
                    } else {
                        total_consumed_cost * (d_line.quantity / total_dest_qty)
                    };
                    let u_cost = share_val / d_line.quantity;
                    (u_cost, share_val)
                } else if d_line.rate > Decimal::ZERO {
                    let u_cost = d_line.rate;
                    (u_cost, u_cost * d_line.quantity)
                } else {
                    (Decimal::ZERO, Decimal::ZERO)
                };

            total_destination_value += assigned_val;

            // Insert stock_voucher_line first so inventory_cost_layers can reference it
            let sv_line_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_voucher_lines (
                    id, company_id, voucher_id, line_number, stock_item_id, unit_id,
                    destination_godown_id, quantity, description, batch_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'Stock Journal Production', ?)",
            )
            .bind(&sv_line_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind(line_num)
            .bind(&d_line.stock_item_id)
            .bind(&unit_id)
            .bind(&d_line.godown_id)
            .bind(d_line.quantity.normalize().to_string())
            .bind(batch_id.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Create new FIFO layer in destination godown
            let layer_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO inventory_cost_layers (
                    id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id,
                    receipt_date, original_quantity, remaining_quantity, unit_cost, original_value, batch_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&layer_id)
            .bind(company_id)
            .bind(&d_line.stock_item_id)
            .bind(&d_line.godown_id)
            .bind(&voucher_id)
            .bind(&sv_line_id)
            .bind(&req.voucher_date)
            .bind(d_line.quantity.normalize().to_string())
            .bind(d_line.quantity.normalize().to_string())
            .bind(assigned_unit_cost.normalize().to_string())
            .bind(assigned_val.normalize().to_string())
            .bind(batch_id.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Insert stock movement transfer_in
            let mv_in_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_movements (
                    id, company_id, stock_item_id, godown_id, voucher_id, voucher_line_id,
                    movement_date, movement_type, signed_quantity, batch_id
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, 'transfer_in', ?, ?)",
            )
            .bind(&mv_in_id)
            .bind(company_id)
            .bind(&d_line.stock_item_id)
            .bind(&d_line.godown_id)
            .bind(&voucher_id)
            .bind(&sv_line_id)
            .bind(&req.voucher_date)
            .bind(d_line.quantity.normalize().to_string())
            .bind(batch_id.as_deref())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            line_num += 1;
        }

        // 5. MCA immutable audit log
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or_else(|_| "2026-04-01".to_string());

        let audit_payload = serde_json::json!({
            "action": "CREATE",
            "voucher_type": "transfer",
            "voucher_number": vch_num,
            "source_lines": req.source_lines.len(),
            "destination_lines": req.destination_lines.len(),
            "total_value": total_destination_value.to_string(),
        })
        .to_string();

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins, action,
                new_values, performed_by
             ) VALUES (?, ?, 'stock_vouchers', ?, ?, 'POST', ?, 'system')",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&voucher_id)
        .bind(&fy_begins)
        .bind(audit_payload)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await?;

        Ok(StockJournalResponse {
            voucher_id,
            voucher_number: vch_num,
            voucher_date: req.voucher_date,
            total_source_value: total_consumed_cost.round_dp(2).normalize().to_string(),
            total_destination_value: total_destination_value.round_dp(2).normalize().to_string(),
            lines_count: req.source_lines.len() + req.destination_lines.len(),
        })
    }

    /// List recent stock journal vouchers
    pub async fn list_stock_journals(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<StockJournalResponse>, OpenTallyError> {
        #[derive(FromRow)]
        struct RawSJ {
            id: String,
            voucher_number: String,
            voucher_date: String,
            lines_count: i64,
        }

        let rows = sqlx::query_as::<_, RawSJ>(
            "SELECT sv.id, sv.voucher_number, sv.voucher_date, COUNT(svl.id) as lines_count
             FROM stock_vouchers sv
             LEFT JOIN stock_voucher_lines svl ON svl.company_id = sv.company_id AND svl.voucher_id = sv.id
             WHERE sv.company_id = ? AND sv.voucher_type = 'transfer'
             GROUP BY sv.id, sv.voucher_number, sv.voucher_date
             ORDER BY sv.voucher_date DESC, sv.created_at DESC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let list = rows
            .into_iter()
            .map(|r| StockJournalResponse {
                voucher_id: r.id,
                voucher_number: r.voucher_number,
                voucher_date: r.voucher_date,
                total_source_value: "0.00".to_string(),
                total_destination_value: "0.00".to_string(),
                lines_count: r.lines_count as usize,
            })
            .collect();

        Ok(list)
    }

    /// Generate Godown Summary report with item stock balances and valuations
    pub async fn get_godown_summary_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        godown_id: &str,
        as_of_date: &str,
    ) -> Result<GodownSummaryReport, OpenTallyError> {
        let cid = company_id.to_string();
        let gid = godown_id.to_string();
        let as_of = as_of_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                // 1. Get godown details
                #[derive(FromRow)]
                struct GodownMeta {
                    id: String,
                    name: String,
                    address: Option<String>,
                    is_external: i64,
                }

                let meta = sqlx::query_as::<_, GodownMeta>(
                    "SELECT id, name, address, is_external FROM godowns WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&gid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| OpenTallyError::NotFound(format!("Godown '{}' not found", gid)))?;

                // 2. Query movements for all items in this godown up to as_of_date
                #[derive(FromRow)]
                struct RawItemStock {
                    stock_item_id: String,
                    stock_item_name: String,
                    unit_symbol: String,
                }

                let items = sqlx::query_as::<_, RawItemStock>(
                    "SELECT DISTINCT si.id as stock_item_id, si.name as stock_item_name, u.symbol as unit_symbol
                     FROM stock_items si
                     JOIN units u ON u.company_id = si.company_id AND u.id = si.base_unit_id
                     WHERE si.company_id = ?
                     ORDER BY si.name",
                )
                .bind(&cid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut item_rows = Vec::new();
                let mut total_godown_val = Decimal::ZERO;

                for itm in items {
                    // Movements calculation with exact rust_decimal
                    let movements: Vec<String> = sqlx::query_scalar(
                        "SELECT signed_quantity FROM stock_movements
                         WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                         AND movement_date <= ?",
                    )
                    .bind(&cid)
                    .bind(&itm.stock_item_id)
                    .bind(&gid)
                    .bind(&as_of)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();

                    let mut inflow = Decimal::ZERO;
                    let mut outflow = Decimal::ZERO;
                    for m_str in movements {
                        if let Ok(m_dec) = Decimal::from_str(&m_str) {
                            if m_dec > Decimal::ZERO {
                                inflow += m_dec;
                            } else if m_dec < Decimal::ZERO {
                                outflow += -m_dec;
                            }
                        }
                    }

                    // Valuation from cost layers with exact rust_decimal
                    let cost_layers: Vec<(String, String)> = sqlx::query_as(
                        "SELECT remaining_quantity, unit_cost FROM inventory_cost_layers
                         WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                         AND receipt_date <= ?",
                    )
                    .bind(&cid)
                    .bind(&itm.stock_item_id)
                    .bind(&gid)
                    .bind(&as_of)
                    .fetch_all(&mut *conn)
                    .await
                    .unwrap_or_default();

                    let mut val = Decimal::ZERO;
                    for (l_qty_str, l_cost_str) in cost_layers {
                        let l_qty = Decimal::from_str(&l_qty_str).unwrap_or(Decimal::ZERO);
                        let l_cost = Decimal::from_str(&l_cost_str).unwrap_or(Decimal::ZERO);
                        val += l_qty * l_cost;
                    }

                    let closing = inflow - outflow;

                    if closing.is_zero() && inflow.is_zero() && outflow.is_zero() {
                        continue; // Skip items with no activity in this godown
                    }

                    let avg_rate = if closing > Decimal::ZERO {
                        (val / closing).round_dp(2).normalize().to_string()
                    } else {
                        "0.00".to_string()
                    };

                    total_godown_val += val;

                    item_rows.push(GodownSummaryItemRow {
                        stock_item_id: itm.stock_item_id,
                        stock_item_name: itm.stock_item_name,
                        base_unit: itm.unit_symbol,
                        inflow_quantity: inflow.normalize().to_string(),
                        outflow_quantity: outflow.normalize().to_string(),
                        closing_quantity: closing.normalize().to_string(),
                        average_rate: avg_rate,
                        closing_value: val.round_dp(2).normalize().to_string(),
                    });
                }

                Ok(GodownSummaryReport {
                    godown_id: meta.id,
                    godown_name: meta.name,
                    address: meta.address,
                    is_external: meta.is_external != 0,
                    items: item_rows,
                    total_godown_value: total_godown_val.round_dp(2).normalize().to_string(),
                })
            })
        })
        .await
    }
}
