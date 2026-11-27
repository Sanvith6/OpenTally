use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobWorkLineInput {
    pub stock_item_id: String,
    pub qty: String,
    pub unit_cost: String,
    #[serde(default)]
    pub scrap_qty: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchJobWorkOutRequest {
    pub challan_number: String,
    pub challan_date: String,
    pub job_worker_ledger_id: String,
    pub source_godown_id: String,
    pub external_godown_id: String,
    pub goods_category: String, // "Input" | "CapitalGood"
    pub narration: Option<String>,
    pub lines: Vec<JobWorkLineInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveJobWorkInRequest {
    pub challan_number: String,
    pub challan_date: String,
    pub linked_out_challan_id: String,
    pub destination_godown_id: String,
    pub finished_stock_item_id: String,
    pub finished_qty: String,
    pub returned_raw_qty: String,
    pub scrap_qty: String,
    pub job_work_charges: String,
    pub scrap_recovery_value: String,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobWorkChallanDto {
    pub id: String,
    pub company_id: String,
    pub challan_number: String,
    pub challan_type: String,
    pub challan_date: String,
    pub job_worker_ledger_id: String,
    pub external_godown_id: String,
    pub linked_out_challan_id: Option<String>,
    pub goods_category: String,
    pub statutory_due_date: String,
    pub status: String,
    pub narration: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itc04Table4Row {
    pub challan_id: String,
    pub challan_number: String,
    pub challan_date: String,
    pub job_worker_name: String,
    pub job_worker_gstin: Option<String>,
    pub goods_category: String,
    pub stock_item_name: String,
    pub hsn_code: Option<String>,
    pub dispatched_qty: String,
    pub taxable_value: String,
    pub statutory_due_date: String,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Itc04Table5aRow {
    pub in_challan_number: String,
    pub in_challan_date: String,
    pub original_out_challan_number: String,
    pub original_out_challan_date: String,
    pub received_qty: String,
    pub scrap_qty: String,
    pub pending_qty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormItc04Report {
    pub company_id: String,
    pub from_date: String,
    pub to_date: String,
    pub table_4: Vec<Itc04Table4Row>,
    pub table_5a: Vec<Itc04Table5aRow>,
}

pub struct JobWorkService;

impl JobWorkService {
    fn dec(s: &str, field: &str) -> Result<Decimal, OpenTallyError> {
        Decimal::from_str(s.trim())
            .map_err(|_| OpenTallyError::Validation(format!("Invalid decimal for '{}'", field)))
    }

    pub fn compute_sec143_due_date(
        challan_date: &str,
        goods_category: &str,
    ) -> Result<String, OpenTallyError> {
        let d = NaiveDate::parse_from_str(challan_date.trim(), "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!("Invalid challan_date '{}'", challan_date))
        })?;
        let days = match goods_category.trim() {
            "Input" => 365,
            "CapitalGood" => 1095,
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid goods_category '{}': must be Input or CapitalGood",
                    other
                )))
            }
        };
        Ok((d + Duration::days(days)).format("%Y-%m-%d").to_string())
    }

    pub async fn dispatch_job_work_material_out(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: DispatchJobWorkOutRequest,
    ) -> Result<JobWorkChallanDto, OpenTallyError> {
        if req.lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "At least one line item is required for Material Out challan".into(),
            ));
        }
        let due_date = Self::compute_sec143_due_date(&req.challan_date, &req.goods_category)?;

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        // Validate external godown has is_external = 1
        let is_ext: Option<i64> =
            sqlx::query_scalar("SELECT is_external FROM godowns WHERE id = ? AND company_id = ?")
                .bind(&req.external_godown_id)
                .bind(company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

        if is_ext != Some(1) {
            return Err(OpenTallyError::Validation(
                "Target job-worker godown must have is_external = 1".into(),
            ));
        }

        let challan_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO job_work_challans (
                id, company_id, challan_number, challan_type, challan_date,
                job_worker_ledger_id, external_godown_id, linked_out_challan_id,
                goods_category, statutory_due_date, status, narration
             ) VALUES (?, ?, ?, 'MaterialOut', ?, ?, ?, NULL, ?, ?, 'Open', ?)",
        )
        .bind(&challan_id)
        .bind(company_id)
        .bind(req.challan_number.trim())
        .bind(req.challan_date.trim())
        .bind(&req.job_worker_ledger_id)
        .bind(&req.external_godown_id)
        .bind(&req.goods_category)
        .bind(&due_date)
        .bind(req.narration.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        for l in &req.lines {
            let qty = Self::dec(&l.qty, "qty")?;
            let cost = Self::dec(&l.unit_cost, "unit_cost")?;
            if qty <= Decimal::ZERO || cost < Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Line qty must be > 0 and unit_cost >= 0".into(),
                ));
            }
            let val = (qty * cost).round_dp(2);
            sqlx::query(
                "INSERT INTO job_work_challan_lines (
                    id, company_id, challan_id, stock_item_id, dispatched_qty,
                    returned_qty, scrap_qty, unit_cost, line_value
                 ) VALUES (?, ?, ?, ?, ?, '0', '0', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(company_id)
            .bind(&challan_id)
            .bind(&l.stock_item_id)
            .bind(qty.normalize().to_string())
            .bind(cost.round_dp(2).to_string())
            .bind(val.to_string())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        let row: JobWorkChallanDto = sqlx::query_as(
            "SELECT id, company_id, challan_number, challan_type, challan_date,
                    job_worker_ledger_id, external_godown_id, linked_out_challan_id,
                    goods_category, statutory_due_date, status, narration, created_at
             FROM job_work_challans WHERE id = ?",
        )
        .bind(&challan_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(row)
    }

    pub async fn receive_job_work_material_in(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: ReceiveJobWorkInRequest,
    ) -> Result<JobWorkChallanDto, OpenTallyError> {
        let returned = Self::dec(&req.returned_raw_qty, "returned_raw_qty")?;
        let scrap = Self::dec(&req.scrap_qty, "scrap_qty")?;
        let finished_qty = Self::dec(&req.finished_qty, "finished_qty")?;
        if returned < Decimal::ZERO || scrap < Decimal::ZERO || finished_qty < Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Quantities cannot be negative".into(),
            ));
        }

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let out_row: JobWorkChallanDto = sqlx::query_as(
            "SELECT id, company_id, challan_number, challan_type, challan_date,
                    job_worker_ledger_id, external_godown_id, linked_out_challan_id,
                    goods_category, statutory_due_date, status, narration, created_at
             FROM job_work_challans
             WHERE id = ? AND company_id = ? AND challan_type = 'MaterialOut'",
        )
        .bind(&req.linked_out_challan_id)
        .bind(company_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Check dispatched vs already returned + new return + scrap
        #[derive(Debug, FromRow)]
        struct OutLine {
            id: String,
            dispatched_qty: String,
            returned_qty: String,
            scrap_qty: String,
            unit_cost: String,
        }
        let out_line: OutLine = sqlx::query_as(
            "SELECT id, dispatched_qty, returned_qty, scrap_qty, unit_cost
             FROM job_work_challan_lines
             WHERE challan_id = ? AND company_id = ?
             LIMIT 1",
        )
        .bind(&out_row.id)
        .bind(company_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let disp = Self::dec(&out_line.dispatched_qty, "dispatched_qty")?;
        let prev_ret = Self::dec(&out_line.returned_qty, "returned_qty")?;
        let prev_scrap = Self::dec(&out_line.scrap_qty, "scrap_qty")?;
        let new_total_accounted = prev_ret + prev_scrap + returned + scrap;
        if new_total_accounted > disp {
            return Err(OpenTallyError::Validation(format!(
                "Over-return rejected: accounted ({}) exceeds dispatched ({})",
                new_total_accounted, disp
            )));
        }

        let new_ret = prev_ret + returned;
        let new_scrap = prev_scrap + scrap;
        sqlx::query(
            "UPDATE job_work_challan_lines SET returned_qty = ?, scrap_qty = ? WHERE id = ?",
        )
        .bind(new_ret.normalize().to_string())
        .bind(new_scrap.normalize().to_string())
        .bind(&out_line.id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let new_status = if new_total_accounted >= disp {
            "Closed"
        } else {
            "PartiallyReturned"
        };
        sqlx::query("UPDATE job_work_challans SET status = ? WHERE id = ?")
            .bind(new_status)
            .bind(&out_row.id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        let in_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO job_work_challans (
                id, company_id, challan_number, challan_type, challan_date,
                job_worker_ledger_id, external_godown_id, linked_out_challan_id,
                goods_category, statutory_due_date, status, narration
             ) VALUES (?, ?, ?, 'MaterialIn', ?, ?, ?, ?, ?, ?, 'Closed', ?)",
        )
        .bind(&in_id)
        .bind(company_id)
        .bind(req.challan_number.trim())
        .bind(req.challan_date.trim())
        .bind(&out_row.job_worker_ledger_id)
        .bind(&req.destination_godown_id)
        .bind(&out_row.id)
        .bind(&out_row.goods_category)
        .bind(&out_row.statutory_due_date)
        .bind(req.narration.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let unit_cost = Self::dec(&out_line.unit_cost, "unit_cost")?;
        let line_val = (finished_qty * unit_cost).round_dp(2);
        sqlx::query(
            "INSERT INTO job_work_challan_lines (
                id, company_id, challan_id, stock_item_id, dispatched_qty,
                returned_qty, scrap_qty, unit_cost, line_value
             ) VALUES (?, ?, ?, ?, '0', ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(&in_id)
        .bind(&req.finished_stock_item_id)
        .bind(returned.normalize().to_string())
        .bind(scrap.normalize().to_string())
        .bind(unit_cost.round_dp(2).to_string())
        .bind(line_val.to_string())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let in_row: JobWorkChallanDto = sqlx::query_as(
            "SELECT id, company_id, challan_number, challan_type, challan_date,
                    job_worker_ledger_id, external_godown_id, linked_out_challan_id,
                    goods_category, statutory_due_date, status, narration, created_at
             FROM job_work_challans WHERE id = ?",
        )
        .bind(&in_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(in_row)
    }

    pub async fn get_itc04_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<FormItc04Report, OpenTallyError> {
        let cid = company_id.to_string();
        let fd = from_date.to_string();
        let td = to_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                #[derive(Debug, FromRow)]
                struct T4Raw {
                    challan_id: String,
                    challan_number: String,
                    challan_date: String,
                    job_worker_name: String,
                    goods_category: String,
                    stock_item_name: String,
                    hsn_code: Option<String>,
                    dispatched_qty: String,
                    line_value: String,
                    statutory_due_date: String,
                    status: String,
                }
                let t4_raw: Vec<T4Raw> = sqlx::query_as(
                    "SELECT c.id AS challan_id, c.challan_number, c.challan_date,
                            l.name AS job_worker_name, c.goods_category,
                            si.name AS stock_item_name, NULL AS hsn_code,
                            cl.dispatched_qty, cl.line_value, c.statutory_due_date, c.status
                     FROM job_work_challans c
                     JOIN job_work_challan_lines cl ON cl.challan_id = c.id
                     JOIN ledgers l ON l.id = c.job_worker_ledger_id
                     JOIN stock_items si ON si.id = cl.stock_item_id
                     WHERE c.company_id = ? AND c.challan_type = 'MaterialOut'
                       AND c.challan_date BETWEEN ? AND ?
                     ORDER BY c.challan_date ASC",
                )
                .bind(&cid)
                .bind(&fd)
                .bind(&td)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let table_4 = t4_raw
                    .into_iter()
                    .map(|r| {
                        let is_overdue = r.status != "Closed" && td > r.statutory_due_date;
                        Itc04Table4Row {
                            challan_id: r.challan_id,
                            challan_number: r.challan_number,
                            challan_date: r.challan_date,
                            job_worker_name: r.job_worker_name,
                            job_worker_gstin: None,
                            goods_category: r.goods_category,
                            stock_item_name: r.stock_item_name,
                            hsn_code: r.hsn_code,
                            dispatched_qty: r.dispatched_qty,
                            taxable_value: r.line_value,
                            statutory_due_date: r.statutory_due_date,
                            is_overdue,
                        }
                    })
                    .collect();

                #[derive(Debug, FromRow)]
                struct T5Raw {
                    in_challan_number: String,
                    in_challan_date: String,
                    orig_number: String,
                    orig_date: String,
                    received_qty: String,
                    scrap_qty: String,
                    orig_dispatched: String,
                    orig_returned: String,
                    orig_scrap: String,
                }
                let t5_raw: Vec<T5Raw> = sqlx::query_as(
                    "SELECT ci.challan_number AS in_challan_number, ci.challan_date AS in_challan_date,
                            co.challan_number AS orig_number, co.challan_date AS orig_date,
                            cli.returned_qty AS received_qty, cli.scrap_qty,
                            clo.dispatched_qty AS orig_dispatched, clo.returned_qty AS orig_returned,
                            clo.scrap_qty AS orig_scrap
                     FROM job_work_challans ci
                     JOIN job_work_challans co ON co.id = ci.linked_out_challan_id
                     JOIN job_work_challan_lines cli ON cli.challan_id = ci.id
                     JOIN job_work_challan_lines clo ON clo.challan_id = co.id
                     WHERE ci.company_id = ? AND ci.challan_type = 'MaterialIn'
                       AND ci.challan_date BETWEEN ? AND ?
                     ORDER BY ci.challan_date ASC",
                )
                .bind(&cid)
                .bind(&fd)
                .bind(&td)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let table_5a = t5_raw
                    .into_iter()
                    .map(|r| {
                        let d = Decimal::from_str(&r.orig_dispatched).unwrap_or(Decimal::ZERO);
                        let ret = Decimal::from_str(&r.orig_returned).unwrap_or(Decimal::ZERO);
                        let scr = Decimal::from_str(&r.orig_scrap).unwrap_or(Decimal::ZERO);
                        let pending = (d - ret - scr).max(Decimal::ZERO);
                        Itc04Table5aRow {
                            in_challan_number: r.in_challan_number,
                            in_challan_date: r.in_challan_date,
                            original_out_challan_number: r.orig_number,
                            original_out_challan_date: r.orig_date,
                            received_qty: r.received_qty,
                            scrap_qty: r.scrap_qty,
                            pending_qty: pending.normalize().to_string(),
                        }
                    })
                    .collect();

                Ok(FormItc04Report {
                    company_id: cid,
                    from_date: fd,
                    to_date: td,
                    table_4,
                    table_5a,
                })
            })
        })
        .await
    }

    pub async fn export_itc04_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<String, OpenTallyError> {
        let report = Self::get_itc04_report(pool, company_id, from_date, to_date).await?;
        serde_json::to_string_pretty(&report).map_err(|e| OpenTallyError::Internal(e.to_string()))
    }
}
