use crate::errors::OpenTallyError;
use crate::orders::models::*;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::{Pool, Row, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

pub struct OrdersService;

impl OrdersService {
    // ==========================================
    // PURCHASE ORDERS
    // ==========================================

    pub async fn create_purchase_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreatePurchaseOrderInput,
        actor: Option<&str>,
    ) -> Result<PurchaseOrderWithLines, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res = Self::create_order_internal(
            &mut conn,
            company_id,
            OrderType::PurchaseOrder,
            input.order_number,
            input.order_date,
            input.due_date,
            input.party_ledger_id,
            input.reference_no,
            input.narration,
            input.lines,
            actor,
        )
        .await;

        match res {
            Ok(po) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(po)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    pub async fn list_purchase_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<PurchaseOrder>, OpenTallyError> {
        Self::list_orders_internal(pool, company_id, OrderType::PurchaseOrder, status_filter).await
    }

    pub async fn get_purchase_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_id: &str,
    ) -> Result<PurchaseOrderWithLines, OpenTallyError> {
        Self::get_order_internal(pool, company_id, OrderType::PurchaseOrder, order_id).await
    }

    pub async fn cancel_purchase_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_id: &str,
        reason: &str,
        actor: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        Self::cancel_order_internal(
            pool,
            company_id,
            OrderType::PurchaseOrder,
            order_id,
            reason,
            actor,
        )
        .await
    }

    pub async fn convert_purchase_order_to_invoice(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: ConvertPurchaseOrderInput,
        actor: Option<&str>,
    ) -> Result<ConvertOrderResult, OpenTallyError> {
        Self::convert_order_internal(
            pool,
            company_id,
            OrderType::PurchaseOrder,
            input.order_id,
            input.invoice_date,
            input.series_id,
            input.commercial_account_ledger_id,
            input.lines,
            input.tax_lines,
            input.narration,
            input.reference_no,
            actor,
        )
        .await
    }

    // ==========================================
    // SALES ORDERS
    // ==========================================

    pub async fn create_sales_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreateSalesOrderInput,
        actor: Option<&str>,
    ) -> Result<SalesOrderWithLines, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res = Self::create_sales_order_internal(
            &mut conn,
            company_id,
            input.order_number,
            input.order_date,
            input.due_date,
            input.party_ledger_id,
            input.reference_no,
            input.narration,
            input.lines,
            actor,
        )
        .await;

        match res {
            Ok(so) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(so)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    pub async fn list_sales_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<SalesOrder>, OpenTallyError> {
        let company_id_str = company_id.to_string();
        let status_str = status_filter.map(|s| s.to_string());

        let rows = sqlx::query(
            "SELECT so.id, so.company_id, so.order_number, so.order_date, so.due_date,
                    so.party_ledger_id, l.name as party_ledger_name,
                    so.reference_no, so.narration, so.status, so.total_amount,
                    so.created_at, so.updated_at
             FROM sales_orders so
             JOIN ledgers l ON l.company_id = so.company_id AND l.id = so.party_ledger_id
             WHERE so.company_id = ? AND (? IS NULL OR so.status = ?)
             ORDER BY so.order_date DESC, so.order_number DESC",
        )
        .bind(&company_id_str)
        .bind(&status_str)
        .bind(&status_str)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut orders = Vec::new();
        for r in rows {
            orders.push(SalesOrder {
                id: r.get("id"),
                company_id: r.get("company_id"),
                order_number: r.get("order_number"),
                order_date: r.get("order_date"),
                due_date: r.get("due_date"),
                party_ledger_id: r.get("party_ledger_id"),
                party_ledger_name: r.get("party_ledger_name"),
                reference_no: r.get("reference_no"),
                narration: r.get("narration"),
                status: r.get("status"),
                total_amount: r.get("total_amount"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        }
        Ok(orders)
    }

    pub async fn get_sales_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_id: &str,
    ) -> Result<SalesOrderWithLines, OpenTallyError> {
        let order_row = sqlx::query(
            "SELECT so.id, so.company_id, so.order_number, so.order_date, so.due_date,
                    so.party_ledger_id, l.name as party_ledger_name,
                    so.reference_no, so.narration, so.status, so.total_amount,
                    so.created_at, so.updated_at
             FROM sales_orders so
             JOIN ledgers l ON l.company_id = so.company_id AND l.id = so.party_ledger_id
             WHERE so.company_id = ? AND so.id = ?",
        )
        .bind(company_id)
        .bind(order_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Sales order '{}' not found", order_id)))?;

        let lines_rows = sqlx::query(
            "SELECT sol.id, sol.company_id, sol.order_id, sol.stock_item_id, si.name as stock_item_name,
                    sol.godown_id, g.name as godown_name,
                    sol.ordered_qty, sol.fulfilled_qty, sol.unit_rate,
                    sol.discount_kind, sol.discount_value, sol.line_amount, sol.created_at
             FROM sales_order_lines sol
             JOIN stock_items si ON si.company_id = sol.company_id AND si.id = sol.stock_item_id
             LEFT JOIN godowns g ON g.company_id = sol.company_id AND g.id = sol.godown_id
             WHERE sol.company_id = ? AND sol.order_id = ?
             ORDER BY sol.id ASC",
        )
        .bind(company_id)
        .bind(order_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut lines = Vec::new();
        for r in lines_rows {
            let ordered_qty: String = r.get("ordered_qty");
            let fulfilled_qty: String = r.get("fulfilled_qty");
            let o_dec = Decimal::from_str(&ordered_qty).unwrap_or(Decimal::ZERO);
            let f_dec = Decimal::from_str(&fulfilled_qty).unwrap_or(Decimal::ZERO);
            let p_dec = (o_dec - f_dec).max(Decimal::ZERO);

            lines.push(SalesOrderLine {
                id: r.get("id"),
                company_id: r.get("company_id"),
                order_id: r.get("order_id"),
                stock_item_id: r.get("stock_item_id"),
                stock_item_name: r.get("stock_item_name"),
                godown_id: r.get("godown_id"),
                godown_name: r.get("godown_name"),
                ordered_qty,
                fulfilled_qty,
                pending_qty: p_dec.to_string(),
                unit_rate: r.get("unit_rate"),
                discount_kind: r.get("discount_kind"),
                discount_value: r.get("discount_value"),
                line_amount: r.get("line_amount"),
                created_at: r.get("created_at"),
            });
        }

        Ok(SalesOrderWithLines {
            order: SalesOrder {
                id: order_row.get("id"),
                company_id: order_row.get("company_id"),
                order_number: order_row.get("order_number"),
                order_date: order_row.get("order_date"),
                due_date: order_row.get("due_date"),
                party_ledger_id: order_row.get("party_ledger_id"),
                party_ledger_name: order_row.get("party_ledger_name"),
                reference_no: order_row.get("reference_no"),
                narration: order_row.get("narration"),
                status: order_row.get("status"),
                total_amount: order_row.get("total_amount"),
                created_at: order_row.get("created_at"),
                updated_at: order_row.get("updated_at"),
            },
            lines,
        })
    }

    pub async fn cancel_sales_order(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_id: &str,
        reason: &str,
        actor: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        Self::cancel_order_internal(
            pool,
            company_id,
            OrderType::SalesOrder,
            order_id,
            reason,
            actor,
        )
        .await
    }

    pub async fn convert_sales_order_to_invoice(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: ConvertSalesOrderInput,
        actor: Option<&str>,
    ) -> Result<ConvertOrderResult, OpenTallyError> {
        Self::convert_order_internal(
            pool,
            company_id,
            OrderType::SalesOrder,
            input.order_id,
            input.invoice_date,
            input.series_id,
            input.commercial_account_ledger_id,
            input.lines,
            input.tax_lines,
            input.narration,
            input.reference_no,
            actor,
        )
        .await
    }

    // ==========================================
    // SHARED INTERNAL LOGIC
    // ==========================================

    #[allow(clippy::too_many_arguments)]
    async fn create_order_internal(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        order_type: OrderType,
        order_number: String,
        order_date: String,
        due_date: Option<String>,
        party_ledger_id: String,
        reference_no: Option<String>,
        narration: Option<String>,
        lines: Vec<CreateOrderLineInput>,
        actor: Option<&str>,
    ) -> Result<PurchaseOrderWithLines, OpenTallyError> {
        if lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "Order must contain at least one line item".to_string(),
            ));
        }

        // Validate date boundaries
        let books_beginning: String =
            sqlx::query_scalar("SELECT books_beginning_from FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", company_id))
                })?;

        let o_date = NaiveDate::parse_from_str(&order_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid order_date '{}', expected YYYY-MM-DD",
                order_date
            ))
        })?;
        let b_date = NaiveDate::parse_from_str(&books_beginning, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid books_beginning_from '{}'",
                books_beginning
            ))
        })?;
        if o_date < b_date {
            return Err(OpenTallyError::Validation(format!(
                "Order date '{}' cannot be earlier than company books beginning date '{}'",
                order_date, books_beginning
            )));
        }

        if let Some(due) = &due_date {
            let d_date = NaiveDate::parse_from_str(due, "%Y-%m-%d").map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Invalid due_date '{}', expected YYYY-MM-DD",
                    due
                ))
            })?;
            if d_date < o_date {
                return Err(OpenTallyError::Validation(format!(
                    "Due date '{}' cannot be earlier than order date '{}'",
                    due, order_date
                )));
            }
        }

        // Period lock check
        crate::company::closing::FinancialYearClosingService::validate_period_not_locked(
            &mut *conn,
            company_id,
            &order_date,
        )
        .await?;

        // Verify party ledger exists
        let party_row = sqlx::query("SELECT id, name FROM ledgers WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(&party_ledger_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
            .ok_or_else(|| {
                OpenTallyError::NotFound(format!("Party ledger '{}' not found", party_ledger_id))
            })?;
        let party_ledger_name: String = party_row.get("name");

        let order_id = Uuid::new_v4().to_string();
        let mut total_amount = Decimal::ZERO;
        let mut prepared_lines = Vec::new();

        for line_input in lines {
            if line_input.ordered_qty <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Ordered quantity must be greater than zero".to_string(),
                ));
            }
            if line_input.unit_rate <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Unit rate must be greater than zero".to_string(),
                ));
            }

            // Verify stock item
            let stock_item_row =
                sqlx::query("SELECT id, name FROM stock_items WHERE company_id = ? AND id = ?")
                    .bind(company_id)
                    .bind(&line_input.stock_item_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?
                    .ok_or_else(|| {
                        OpenTallyError::NotFound(format!(
                            "Stock item '{}' not found",
                            line_input.stock_item_id
                        ))
                    })?;
            let stock_item_name: String = stock_item_row.get("name");

            let mut godown_name = None;
            if let Some(ref gid) = line_input.godown_id {
                let g_row = sqlx::query("SELECT name FROM godowns WHERE company_id = ? AND id = ?")
                    .bind(company_id)
                    .bind(gid)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                if let Some(r) = g_row {
                    godown_name = Some(r.get("name"));
                }
            }

            // Calculate line amount with exact decimal arithmetic
            if let Some(ref val) = line_input.discount_value {
                if *val < Decimal::ZERO {
                    return Err(OpenTallyError::Validation(
                        "Discount value cannot be negative".to_string(),
                    ));
                }
                if line_input.discount_kind.as_deref() == Some("Percentage")
                    && *val > Decimal::from(100)
                {
                    return Err(OpenTallyError::Validation(
                        "Discount percentage cannot exceed 100%".to_string(),
                    ));
                }
            }

            let base_amount = line_input.ordered_qty * line_input.unit_rate;
            let discount_amt = match line_input.discount_kind.as_deref() {
                Some("Percentage") => {
                    let pct = line_input.discount_value.unwrap_or(Decimal::ZERO);
                    base_amount * (pct / Decimal::from(100))
                }
                Some("FixedAmount") => line_input.discount_value.unwrap_or(Decimal::ZERO),
                _ => Decimal::ZERO,
            };
            let line_amount = (base_amount - discount_amt).max(Decimal::ZERO);
            total_amount += line_amount;

            let line_id = Uuid::new_v4().to_string();
            prepared_lines.push((
                line_id,
                line_input,
                line_amount,
                stock_item_name,
                godown_name,
            ));
        }

        // Insert parent order first to satisfy foreign key constraints
        let table_orders = match order_type {
            OrderType::PurchaseOrder => "purchase_orders",
            OrderType::SalesOrder => "sales_orders",
        };

        sqlx::query(&format!(
            "INSERT INTO {} (
                id, company_id, order_number, order_date, due_date, party_ledger_id,
                reference_no, narration, status, total_amount
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'Open', ?)",
            table_orders
        ))
        .bind(&order_id)
        .bind(company_id)
        .bind(&order_number)
        .bind(&order_date)
        .bind(&due_date)
        .bind(&party_ledger_id)
        .bind(&reference_no)
        .bind(&narration)
        .bind(total_amount.to_string())
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let table_lines = match order_type {
            OrderType::PurchaseOrder => "purchase_order_lines",
            OrderType::SalesOrder => "sales_order_lines",
        };

        let mut order_lines = Vec::new();
        for (line_id, line_input, line_amount, stock_item_name, godown_name) in prepared_lines {
            sqlx::query(&format!(
                "INSERT INTO {} (
                    id, company_id, order_id, stock_item_id, godown_id,
                    ordered_qty, fulfilled_qty, unit_rate, discount_kind, discount_value, line_amount
                 ) VALUES (?, ?, ?, ?, ?, ?, '0', ?, ?, ?, ?)",
                table_lines
            ))
            .bind(&line_id)
            .bind(company_id)
            .bind(&order_id)
            .bind(&line_input.stock_item_id)
            .bind(&line_input.godown_id)
            .bind(line_input.ordered_qty.to_string())
            .bind(line_input.unit_rate.to_string())
            .bind(&line_input.discount_kind)
            .bind(line_input.discount_value.map(|d| d.to_string()))
            .bind(line_amount.to_string())
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            order_lines.push(PurchaseOrderLine {
                id: line_id,
                company_id: company_id.to_string(),
                order_id: order_id.clone(),
                stock_item_id: line_input.stock_item_id,
                stock_item_name: Some(stock_item_name),
                godown_id: line_input.godown_id,
                godown_name,
                ordered_qty: line_input.ordered_qty.to_string(),
                fulfilled_qty: "0".to_string(),
                pending_qty: line_input.ordered_qty.to_string(),
                unit_rate: line_input.unit_rate.to_string(),
                discount_kind: line_input.discount_kind,
                discount_value: line_input.discount_value.map(|d| d.to_string()),
                line_amount: line_amount.to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        // Emit MCA Audit Log with SHA-256 hash chaining
        let prev_hash: String = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .unwrap_or_else(|| "GENESIS".to_string());

        let audit_id = Uuid::new_v4().to_string();
        let now_str = chrono::Utc::now().to_rfc3339();
        let entity_type_str = match order_type {
            OrderType::PurchaseOrder => "purchase_order",
            OrderType::SalesOrder => "sales_order",
        };
        let new_vals = serde_json::json!({
            "order_number": order_number,
            "order_date": order_date,
            "party_ledger_id": party_ledger_id,
            "total_amount": total_amount.to_string(),
            "lines_count": order_lines.len()
        })
        .to_string();

        let entry_hash = crate::audit::certification::AuditCertificationService::compute_entry_hash(
            &prev_hash,
            &audit_id,
            company_id,
            entity_type_str,
            &order_id,
            "CREATE",
            &now_str,
            Some(&new_vals),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, action,
                performed_by, timestamp, new_values, prev_hash, entry_hash
             ) VALUES (?, ?, ?, ?, 'CREATE', ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(entity_type_str)
        .bind(&order_id)
        .bind(actor.unwrap_or("system"))
        .bind(&now_str)
        .bind(&new_vals)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(PurchaseOrderWithLines {
            order: PurchaseOrder {
                id: order_id,
                company_id: company_id.to_string(),
                order_number,
                order_date,
                due_date,
                party_ledger_id,
                party_ledger_name: Some(party_ledger_name),
                reference_no,
                narration,
                status: "Open".to_string(),
                total_amount: total_amount.to_string(),
                created_at: now_str.clone(),
                updated_at: now_str,
            },
            lines: order_lines,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_sales_order_internal(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        order_number: String,
        order_date: String,
        due_date: Option<String>,
        party_ledger_id: String,
        reference_no: Option<String>,
        narration: Option<String>,
        lines: Vec<CreateOrderLineInput>,
        actor: Option<&str>,
    ) -> Result<SalesOrderWithLines, OpenTallyError> {
        let po = Self::create_order_internal(
            conn,
            company_id,
            OrderType::SalesOrder,
            order_number,
            order_date,
            due_date,
            party_ledger_id,
            reference_no,
            narration,
            lines,
            actor,
        )
        .await?;

        let mut s_lines = Vec::new();
        for l in po.lines {
            s_lines.push(SalesOrderLine {
                id: l.id,
                company_id: l.company_id,
                order_id: l.order_id,
                stock_item_id: l.stock_item_id,
                stock_item_name: l.stock_item_name,
                godown_id: l.godown_id,
                godown_name: l.godown_name,
                ordered_qty: l.ordered_qty,
                fulfilled_qty: l.fulfilled_qty,
                pending_qty: l.pending_qty,
                unit_rate: l.unit_rate,
                discount_kind: l.discount_kind,
                discount_value: l.discount_value,
                line_amount: l.line_amount,
                created_at: l.created_at,
            });
        }

        Ok(SalesOrderWithLines {
            order: SalesOrder {
                id: po.order.id,
                company_id: po.order.company_id,
                order_number: po.order.order_number,
                order_date: po.order.order_date,
                due_date: po.order.due_date,
                party_ledger_id: po.order.party_ledger_id,
                party_ledger_name: po.order.party_ledger_name,
                reference_no: po.order.reference_no,
                narration: po.order.narration,
                status: po.order.status,
                total_amount: po.order.total_amount,
                created_at: po.order.created_at,
                updated_at: po.order.updated_at,
            },
            lines: s_lines,
        })
    }

    async fn list_orders_internal(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        status_filter: Option<&str>,
    ) -> Result<Vec<PurchaseOrder>, OpenTallyError> {
        let table = match order_type {
            OrderType::PurchaseOrder => "purchase_orders",
            OrderType::SalesOrder => "sales_orders",
        };
        let company_id_str = company_id.to_string();
        let status_str = status_filter.map(|s| s.to_string());

        let sql = format!(
            "SELECT o.id, o.company_id, o.order_number, o.order_date, o.due_date,
                    o.party_ledger_id, l.name as party_ledger_name,
                    o.reference_no, o.narration, o.status, o.total_amount,
                    o.created_at, o.updated_at
             FROM {} o
             JOIN ledgers l ON l.company_id = o.company_id AND l.id = o.party_ledger_id
             WHERE o.company_id = ? AND (? IS NULL OR o.status = ?)
             ORDER BY o.order_date DESC, o.order_number DESC",
            table
        );

        let rows = sqlx::query(&sql)
            .bind(&company_id_str)
            .bind(&status_str)
            .bind(&status_str)
            .fetch_all(pool)
            .await
            .map_err(OpenTallyError::Database)?;

        let mut orders = Vec::new();
        for r in rows {
            orders.push(PurchaseOrder {
                id: r.get("id"),
                company_id: r.get("company_id"),
                order_number: r.get("order_number"),
                order_date: r.get("order_date"),
                due_date: r.get("due_date"),
                party_ledger_id: r.get("party_ledger_id"),
                party_ledger_name: r.get("party_ledger_name"),
                reference_no: r.get("reference_no"),
                narration: r.get("narration"),
                status: r.get("status"),
                total_amount: r.get("total_amount"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
            });
        }
        Ok(orders)
    }

    async fn get_order_internal(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        order_id: &str,
    ) -> Result<PurchaseOrderWithLines, OpenTallyError> {
        let (table_orders, table_lines) = match order_type {
            OrderType::PurchaseOrder => ("purchase_orders", "purchase_order_lines"),
            OrderType::SalesOrder => ("sales_orders", "sales_order_lines"),
        };

        let order_row = sqlx::query(&format!(
            "SELECT o.id, o.company_id, o.order_number, o.order_date, o.due_date,
                    o.party_ledger_id, l.name as party_ledger_name,
                    o.reference_no, o.narration, o.status, o.total_amount,
                    o.created_at, o.updated_at
             FROM {} o
             JOIN ledgers l ON l.company_id = o.company_id AND l.id = o.party_ledger_id
             WHERE o.company_id = ? AND o.id = ?",
            table_orders
        ))
        .bind(company_id)
        .bind(order_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Order '{}' not found", order_id)))?;

        let lines_rows = sqlx::query(&format!(
            "SELECT ol.id, ol.company_id, ol.order_id, ol.stock_item_id, si.name as stock_item_name,
                    ol.godown_id, g.name as godown_name,
                    ol.ordered_qty, ol.fulfilled_qty, ol.unit_rate,
                    ol.discount_kind, ol.discount_value, ol.line_amount, ol.created_at
             FROM {} ol
             JOIN stock_items si ON si.company_id = ol.company_id AND si.id = ol.stock_item_id
             LEFT JOIN godowns g ON g.company_id = ol.company_id AND g.id = ol.godown_id
             WHERE ol.company_id = ? AND ol.order_id = ?
             ORDER BY ol.id ASC",
            table_lines
        ))
        .bind(company_id)
        .bind(order_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut lines = Vec::new();
        for r in lines_rows {
            let ordered_qty: String = r.get("ordered_qty");
            let fulfilled_qty: String = r.get("fulfilled_qty");
            let o_dec = Decimal::from_str(&ordered_qty).unwrap_or(Decimal::ZERO);
            let f_dec = Decimal::from_str(&fulfilled_qty).unwrap_or(Decimal::ZERO);
            let p_dec = (o_dec - f_dec).max(Decimal::ZERO);

            lines.push(PurchaseOrderLine {
                id: r.get("id"),
                company_id: r.get("company_id"),
                order_id: r.get("order_id"),
                stock_item_id: r.get("stock_item_id"),
                stock_item_name: r.get("stock_item_name"),
                godown_id: r.get("godown_id"),
                godown_name: r.get("godown_name"),
                ordered_qty,
                fulfilled_qty,
                pending_qty: p_dec.to_string(),
                unit_rate: r.get("unit_rate"),
                discount_kind: r.get("discount_kind"),
                discount_value: r.get("discount_value"),
                line_amount: r.get("line_amount"),
                created_at: r.get("created_at"),
            });
        }

        Ok(PurchaseOrderWithLines {
            order: PurchaseOrder {
                id: order_row.get("id"),
                company_id: order_row.get("company_id"),
                order_number: order_row.get("order_number"),
                order_date: order_row.get("order_date"),
                due_date: order_row.get("due_date"),
                party_ledger_id: order_row.get("party_ledger_id"),
                party_ledger_name: order_row.get("party_ledger_name"),
                reference_no: order_row.get("reference_no"),
                narration: order_row.get("narration"),
                status: order_row.get("status"),
                total_amount: order_row.get("total_amount"),
                created_at: order_row.get("created_at"),
                updated_at: order_row.get("updated_at"),
            },
            lines,
        })
    }

    async fn cancel_order_internal(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        order_id: &str,
        _reason: &str,
        actor: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res = Self::cancel_order_transactional(
            &mut conn, company_id, order_type, order_id, _reason, actor,
        )
        .await;

        match res {
            Ok(()) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(())
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    async fn cancel_order_transactional(
        conn: &mut sqlx::pool::PoolConnection<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        order_id: &str,
        _reason: &str,
        actor: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        if _reason.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "Cancellation reason cannot be empty".to_string(),
            ));
        }

        let table = match order_type {
            OrderType::PurchaseOrder => "purchase_orders",
            OrderType::SalesOrder => "sales_orders",
        };

        let row = sqlx::query(&format!(
            "SELECT status, order_date FROM {} WHERE company_id = ? AND id = ?",
            table
        ))
        .bind(company_id)
        .bind(order_id)
        .fetch_optional(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Order '{}' not found", order_id)))?;

        let current_status: String = row.get("status");
        let order_date: String = row.get("order_date");

        if current_status == "Cancelled" {
            return Err(OpenTallyError::Validation(
                "Order is already cancelled".to_string(),
            ));
        }

        // Period lock check
        crate::company::closing::FinancialYearClosingService::validate_period_not_locked(
            &mut **conn,
            company_id,
            &order_date,
        )
        .await?;

        // Guardrail: reject cancelling if active non-cancelled fulfillments exist
        let active_fulfillments: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM order_fulfillments WHERE company_id = ? AND order_id = ? AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(order_id)
        .fetch_one(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if active_fulfillments > 0 {
            return Err(OpenTallyError::Validation(
                "Cannot cancel order with active fulfillments. Cancel the linked invoices first."
                    .to_string(),
            ));
        }

        sqlx::query(&format!(
            "UPDATE {} SET status = 'Cancelled', updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
            table
        ))
        .bind(company_id)
        .bind(order_id)
        .execute(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Audit log
        let prev_hash: String = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?
        .unwrap_or_else(|| "GENESIS".to_string());

        let audit_id = Uuid::new_v4().to_string();
        let now_str = chrono::Utc::now().to_rfc3339();
        let entity_type_str = match order_type {
            OrderType::PurchaseOrder => "purchase_order",
            OrderType::SalesOrder => "sales_order",
        };
        let new_vals = serde_json::json!({
            "status": "Cancelled",
            "previous_status": current_status,
            "reason": _reason
        })
        .to_string();

        let entry_hash = crate::audit::certification::AuditCertificationService::compute_entry_hash(
            &prev_hash,
            &audit_id,
            company_id,
            entity_type_str,
            order_id,
            "CANCEL",
            &now_str,
            Some(&new_vals),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, action,
                performed_by, timestamp, new_values, prev_hash, entry_hash
             ) VALUES (?, ?, ?, ?, 'CANCEL', ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(entity_type_str)
        .bind(order_id)
        .bind(actor.unwrap_or("system"))
        .bind(&now_str)
        .bind(&new_vals)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .execute(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn convert_order_internal(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        order_id: String,
        invoice_date: String,
        series_id: String,
        commercial_account_ledger_id: Option<String>,
        lines_to_fulfill: Vec<FulfillOrderLineInput>,
        tax_lines: Option<Vec<crate::commercial::TaxPostingLine>>,
        narration: Option<String>,
        reference_no: Option<String>,
        actor: Option<&str>,
    ) -> Result<ConvertOrderResult, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let res = Self::convert_order_transactional(
            &mut conn,
            company_id,
            order_type,
            order_id,
            invoice_date,
            series_id,
            commercial_account_ledger_id,
            lines_to_fulfill,
            tax_lines,
            narration,
            reference_no,
            actor,
        )
        .await;

        match res {
            Ok(val) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Ok(val)
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn convert_order_transactional(
        conn: &mut sqlx::pool::PoolConnection<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        order_id: String,
        invoice_date: String,
        series_id: String,
        commercial_account_ledger_id: Option<String>,
        lines_to_fulfill: Vec<FulfillOrderLineInput>,
        tax_lines: Option<Vec<crate::commercial::TaxPostingLine>>,
        narration: Option<String>,
        reference_no: Option<String>,
        actor: Option<&str>,
    ) -> Result<ConvertOrderResult, OpenTallyError> {
        if lines_to_fulfill.is_empty() {
            return Err(OpenTallyError::Validation(
                "Conversion requires at least one order line to fulfill".to_string(),
            ));
        }

        // Duplicate line check
        let mut seen_lines = std::collections::HashSet::new();
        for f_line in &lines_to_fulfill {
            if !seen_lines.insert(&f_line.order_line_id) {
                return Err(OpenTallyError::Validation(format!(
                    "Duplicate fulfillment line for order_line_id '{}'",
                    f_line.order_line_id
                )));
            }
        }

        let (table_orders, table_lines, doc_type) = match order_type {
            OrderType::PurchaseOrder => (
                "purchase_orders",
                "purchase_order_lines",
                crate::commercial::CommercialDocumentType::Purchase,
            ),
            OrderType::SalesOrder => (
                "sales_orders",
                "sales_order_lines",
                crate::commercial::CommercialDocumentType::Sales,
            ),
        };

        // Fetch Order header
        let order_row = sqlx::query(&format!(
            "SELECT id, order_number, order_date, party_ledger_id, status, narration
             FROM {} WHERE company_id = ? AND id = ?",
            table_orders
        ))
        .bind(company_id)
        .bind(&order_id)
        .fetch_optional(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Order '{}' not found", order_id)))?;

        let status: String = order_row.get("status");
        if status == "Cancelled" {
            return Err(OpenTallyError::Validation(
                "Cannot convert a cancelled order to an invoice".to_string(),
            ));
        }
        if status == "Fulfilled" {
            return Err(OpenTallyError::Validation(
                "Order is already completely fulfilled".to_string(),
            ));
        }

        let order_number: String = order_row.get("order_number");
        let party_ledger_id: String = order_row.get("party_ledger_id");
        let order_narration: Option<String> = order_row.get("narration");

        // Period lock check on invoice_date
        crate::company::closing::FinancialYearClosingService::validate_period_not_locked(
            &mut **conn,
            company_id,
            &invoice_date,
        )
        .await?;

        // Resolve commercial account ledger if None
        let resolved_commercial_ledger_id = match commercial_account_ledger_id {
            Some(id) => id,
            None => {
                let default_group = match order_type {
                    OrderType::PurchaseOrder => "Purchase Accounts",
                    OrderType::SalesOrder => "Sales Accounts",
                };
                let led_opt: Option<String> = sqlx::query_scalar(
                    "SELECT l.id FROM ledgers l
                     JOIN account_groups ag ON ag.company_id = l.company_id AND ag.id = l.group_id
                     WHERE l.company_id = ? AND ag.name = ? COLLATE NOCASE
                     LIMIT 1",
                )
                .bind(company_id)
                .bind(default_group)
                .fetch_optional(&mut **conn)
                .await
                .map_err(OpenTallyError::Database)?;

                led_opt.ok_or_else(|| {
                    OpenTallyError::Validation(format!(
                        "Could not resolve default commercial account ledger under '{}'",
                        default_group
                    ))
                })?
            }
        };

        // Validate and build commercial posting lines
        let mut commercial_lines = Vec::new();
        let mut fulfillment_records = Vec::new();
        let mut updated_lines_quantities = Vec::new();
        let mut total_goods_amount = Decimal::ZERO;

        for (line_idx, f_line) in lines_to_fulfill.into_iter().enumerate() {
            let line_num = (line_idx + 1) as i64;
            if f_line.fulfill_qty <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Fulfill quantity must be greater than zero".to_string(),
                ));
            }

            let line_row = sqlx::query(&format!(
                "SELECT stock_item_id, godown_id, ordered_qty, fulfilled_qty, unit_rate, discount_kind, discount_value
                 FROM {} WHERE company_id = ? AND id = ? AND order_id = ?",
                table_lines
            ))
            .bind(company_id)
            .bind(&f_line.order_line_id)
            .bind(&order_id)
            .fetch_optional(&mut **conn)
            .await
            .map_err(OpenTallyError::Database)?
            .ok_or_else(|| {
                OpenTallyError::NotFound(format!("Order line '{}' not found", f_line.order_line_id))
            })?;

            let stock_item_id: String = line_row.get("stock_item_id");
            let db_godown_id: Option<String> = line_row.get("godown_id");
            let ordered_qty: Decimal = Decimal::from_str(&line_row.get::<String, _>("ordered_qty"))
                .unwrap_or(Decimal::ZERO);
            let current_fulfilled: Decimal =
                Decimal::from_str(&line_row.get::<String, _>("fulfilled_qty"))
                    .unwrap_or(Decimal::ZERO);
            let unit_rate: Decimal = match f_line.unit_rate {
                Some(r) => r,
                None => Decimal::from_str(&line_row.get::<String, _>("unit_rate"))
                    .unwrap_or(Decimal::ZERO),
            };
            let discount_kind: Option<String> = line_row.get("discount_kind");
            let discount_val: Option<Decimal> = line_row
                .get::<Option<String>, _>("discount_value")
                .and_then(|v| Decimal::from_str(&v).ok());

            let pending_qty = (ordered_qty - current_fulfilled).max(Decimal::ZERO);
            if f_line.fulfill_qty > pending_qty {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot fulfill {} for item; only {} remaining on order",
                    f_line.fulfill_qty, pending_qty
                )));
            }

            let effective_godown_id = f_line.godown_id.or(db_godown_id);
            let base_line_amount = f_line.fulfill_qty * unit_rate;
            let discount_amount = match discount_kind.as_deref() {
                Some("Percentage") => {
                    let pct = discount_val.unwrap_or(Decimal::ZERO);
                    base_line_amount * (pct / Decimal::from(100))
                }
                Some("FixedAmount") => {
                    let fixed = discount_val.unwrap_or(Decimal::ZERO);
                    if ordered_qty > Decimal::ZERO {
                        fixed * (f_line.fulfill_qty / ordered_qty)
                    } else {
                        Decimal::ZERO
                    }
                }
                _ => Decimal::ZERO,
            };
            let line_amount = (base_line_amount - discount_amount).max(Decimal::ZERO);
            total_goods_amount += line_amount;

            // Fetch unit_id for stock_item
            let unit_id: Option<String> = sqlx::query_scalar(
                "SELECT base_unit_id FROM stock_items WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&stock_item_id)
            .fetch_optional(&mut **conn)
            .await
            .map_err(OpenTallyError::Database)?;

            commercial_lines.push(crate::commercial::CommercialLinePosting {
                line_number: line_num,
                description: format!("Fulfillment of Order {}", order_number),
                stock_item_id: Some(stock_item_id.clone()),
                unit_id,
                godown_id: effective_godown_id,
                quantity: f_line.fulfill_qty,
                rate: unit_rate,
                discount_amount,
                line_amount,
                batch_number: f_line.batch_number,
                mfg_date: f_line.mfg_date,
                expiry_date: f_line.expiry_date,
                billed_quantity: None,
                entered_unit_id: None,
            });

            let new_fulfilled = current_fulfilled + f_line.fulfill_qty;
            updated_lines_quantities.push((f_line.order_line_id.clone(), new_fulfilled));

            fulfillment_records.push((f_line.order_line_id, f_line.fulfill_qty));
        }

        let taxes = tax_lines.unwrap_or_default();

        let comm_input = crate::commercial::CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: doc_type,
            source_document_id: order_id.clone(),
            source_voucher_id: None,
            series_id,
            date: invoice_date.clone(),
            due_date: None,
            party_id: party_ledger_id.clone(),
            party_ledger_id,
            commercial_account_ledger_id: Some(resolved_commercial_ledger_id),
            total_amount: total_goods_amount, // Taxable base
            document_reference: reference_no.or_else(|| Some(order_number.clone())),
            narration: narration.or(order_narration),
            lines: commercial_lines,
            tax_lines: taxes,
            bill_allocations: vec![], // Auto-create unique NewRef matching grand total
            place_of_supply_state_code: None,
            is_reverse_charge: None,
            allow_tax_override: Some(true),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        };

        // Post commercial invoice voucher
        let comm_res = crate::commercial::CommercialPostingService::execute_posting_transaction(
            conn, company_id, comm_input,
        )
        .await?;

        let voucher_id = comm_res.voucher.voucher.id;
        let voucher_number = comm_res.voucher.voucher.voucher_number;

        // Record order fulfillments and update order lines
        let mut final_fulfillments = Vec::new();
        for (order_line_id, fulfilled_qty) in fulfillment_records {
            let ful_id = Uuid::new_v4().to_string();
            let now_ts = chrono::Utc::now().to_rfc3339();

            sqlx::query(
                "INSERT INTO order_fulfillments (
                    id, company_id, order_type, order_id, order_line_id,
                    voucher_id, commercial_doc_id, fulfilled_qty, fulfillment_date, is_cancelled
                 ) VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?, 0)",
            )
            .bind(&ful_id)
            .bind(company_id)
            .bind(order_type.as_str())
            .bind(&order_id)
            .bind(&order_line_id)
            .bind(&voucher_id)
            .bind(fulfilled_qty.to_string())
            .bind(&invoice_date)
            .execute(&mut **conn)
            .await
            .map_err(OpenTallyError::Database)?;

            final_fulfillments.push(OrderFulfillmentRecord {
                id: ful_id,
                company_id: company_id.to_string(),
                order_type: order_type.as_str().to_string(),
                order_id: order_id.clone(),
                order_line_id,
                voucher_id: voucher_id.clone(),
                commercial_doc_id: None,
                fulfilled_qty: fulfilled_qty.to_string(),
                fulfillment_date: invoice_date.clone(),
                is_cancelled: false,
                created_at: now_ts,
            });
        }

        // Update lines
        for (line_id, new_qty) in updated_lines_quantities {
            sqlx::query(&format!(
                "UPDATE {} SET fulfilled_qty = ? WHERE company_id = ? AND id = ?",
                table_lines
            ))
            .bind(new_qty.to_string())
            .bind(company_id)
            .bind(&line_id)
            .execute(&mut **conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Determine new order status
        let all_lines = sqlx::query(&format!(
            "SELECT ordered_qty, fulfilled_qty FROM {} WHERE company_id = ? AND order_id = ?",
            table_lines
        ))
        .bind(company_id)
        .bind(&order_id)
        .fetch_all(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut all_fulfilled = true;
        for l_row in all_lines {
            let o: Decimal =
                Decimal::from_str(&l_row.get::<String, _>("ordered_qty")).unwrap_or(Decimal::ZERO);
            let f: Decimal = Decimal::from_str(&l_row.get::<String, _>("fulfilled_qty"))
                .unwrap_or(Decimal::ZERO);
            if f < o {
                all_fulfilled = false;
                break;
            }
        }

        let new_status = if all_fulfilled {
            "Fulfilled"
        } else {
            "PartiallyFulfilled"
        };

        sqlx::query(&format!(
            "UPDATE {} SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
            table_orders
        ))
        .bind(new_status)
        .bind(company_id)
        .bind(&order_id)
        .execute(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Audit log
        let prev_hash: String = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?
        .unwrap_or_else(|| "GENESIS".to_string());

        let audit_id = Uuid::new_v4().to_string();
        let now_str = chrono::Utc::now().to_rfc3339();
        let entity_type_str = match order_type {
            OrderType::PurchaseOrder => "purchase_order",
            OrderType::SalesOrder => "sales_order",
        };
        let new_vals = serde_json::json!({
            "action": "CONVERT_TO_INVOICE",
            "voucher_id": voucher_id,
            "voucher_number": voucher_number,
            "new_order_status": new_status,
            "fulfilled_lines": final_fulfillments.len()
        })
        .to_string();

        let entry_hash = crate::audit::certification::AuditCertificationService::compute_entry_hash(
            &prev_hash,
            &audit_id,
            company_id,
            entity_type_str,
            &order_id,
            "UPDATE",
            &now_str,
            Some(&new_vals),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, action,
                performed_by, timestamp, new_values, prev_hash, entry_hash
             ) VALUES (?, ?, ?, ?, 'UPDATE', ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(entity_type_str)
        .bind(&order_id)
        .bind(actor.unwrap_or("system"))
        .bind(&now_str)
        .bind(&new_vals)
        .bind(&prev_hash)
        .bind(&entry_hash)
        .execute(&mut **conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(ConvertOrderResult {
            voucher_id,
            voucher_number,
            order_status: new_status.to_string(),
            fulfillments: final_fulfillments,
        })
    }

    // ==========================================
    // CANCELLATION RESTORATION HOOK
    // ==========================================

    pub async fn restore_order_fulfillments_on_cancel(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        actor: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        // Safe check if order_fulfillments table exists (for unmigrated legacy fixtures)
        let fulfillments_res = sqlx::query(
            "SELECT id, order_type, order_id, order_line_id, fulfilled_qty
             FROM order_fulfillments
             WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *conn)
        .await;

        let rows = match fulfillments_res {
            Ok(r) => r,
            Err(sqlx::Error::Database(ref e))
                if e.message().contains("no such table: order_fulfillments") =>
            {
                return Ok(());
            }
            Err(e) => return Err(OpenTallyError::Database(e)),
        };

        if rows.is_empty() {
            return Ok(());
        }

        for r in rows {
            let ful_id: String = r.get("id");
            let order_type: String = r.get("order_type");
            let order_id: String = r.get("order_id");
            let order_line_id: String = r.get("order_line_id");
            let fulfilled_qty: Decimal =
                Decimal::from_str(&r.get::<String, _>("fulfilled_qty")).unwrap_or(Decimal::ZERO);

            // Mark fulfillment cancelled
            sqlx::query(
                "UPDATE order_fulfillments SET is_cancelled = 1 WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&ful_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (table_orders, table_lines) = if order_type == "PurchaseOrder" {
                ("purchase_orders", "purchase_order_lines")
            } else {
                ("sales_orders", "sales_order_lines")
            };

            // Subtract fulfilled_qty from order line
            let current_qty: Decimal = sqlx::query_scalar(&format!(
                "SELECT fulfilled_qty FROM {} WHERE company_id = ? AND id = ?",
                table_lines
            ))
            .bind(company_id)
            .bind(&order_line_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
            .and_then(|s: String| Decimal::from_str(&s).ok())
            .unwrap_or(Decimal::ZERO);

            let restored_qty = (current_qty - fulfilled_qty).max(Decimal::ZERO);
            sqlx::query(&format!(
                "UPDATE {} SET fulfilled_qty = ? WHERE company_id = ? AND id = ?",
                table_lines
            ))
            .bind(restored_qty.to_string())
            .bind(company_id)
            .bind(&order_line_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            // Recompute order status
            let line_quantities = sqlx::query(&format!(
                "SELECT fulfilled_qty FROM {} WHERE company_id = ? AND order_id = ?",
                table_lines
            ))
            .bind(company_id)
            .bind(&order_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let total_f: Decimal = line_quantities
                .iter()
                .map(|row| {
                    let s: String = row.get("fulfilled_qty");
                    Decimal::from_str(&s).unwrap_or(Decimal::ZERO)
                })
                .sum();

            let new_status = if total_f == Decimal::ZERO {
                "Open"
            } else {
                "PartiallyFulfilled"
            };

            sqlx::query(&format!(
                "UPDATE {} SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
                table_orders
            ))
            .bind(new_status)
            .bind(company_id)
            .bind(&order_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            // Hash chained audit log
            let prev_hash: String = sqlx::query_scalar(
                "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
            )
            .bind(company_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
            .unwrap_or_else(|| "GENESIS".to_string());

            let audit_id = Uuid::new_v4().to_string();
            let now_str = chrono::Utc::now().to_rfc3339();
            let new_vals = serde_json::json!({
                "action": "FULFILLMENT_RESTORED_ON_CANCEL",
                "voucher_id": voucher_id,
                "order_id": order_id,
                "restored_qty": fulfilled_qty.to_string(),
                "new_order_status": new_status
            })
            .to_string();

            let entry_hash =
                crate::audit::certification::AuditCertificationService::compute_entry_hash(
                    &prev_hash,
                    &audit_id,
                    company_id,
                    "order_fulfillment",
                    &ful_id,
                    "CANCEL",
                    &now_str,
                    Some(&new_vals),
                );

            sqlx::query(
                "INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, action,
                    performed_by, timestamp, new_values, prev_hash, entry_hash
                 ) VALUES (?, ?, 'order_fulfillment', ?, 'CANCEL', ?, ?, ?, ?, ?)",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(&ful_id)
            .bind(actor.unwrap_or("system"))
            .bind(&now_str)
            .bind(&new_vals)
            .bind(&prev_hash)
            .bind(&entry_hash)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    // ==========================================
    // PENDING ORDERS OUTSTANDING REPORTS
    // ==========================================

    pub async fn get_pending_purchase_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<PendingOrdersReport, OpenTallyError> {
        Self::get_pending_orders_report(pool, company_id, OrderType::PurchaseOrder, as_of_date)
            .await
    }

    pub async fn get_pending_sales_orders(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<PendingOrdersReport, OpenTallyError> {
        Self::get_pending_orders_report(pool, company_id, OrderType::SalesOrder, as_of_date).await
    }

    async fn get_pending_orders_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        order_type: OrderType,
        as_of_date: &str,
    ) -> Result<PendingOrdersReport, OpenTallyError> {
        let _ = NaiveDate::parse_from_str(as_of_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid as_of_date '{}', expected YYYY-MM-DD",
                as_of_date
            ))
        })?;

        let (table_orders, table_lines, ot_str) = match order_type {
            OrderType::PurchaseOrder => {
                ("purchase_orders", "purchase_order_lines", "PurchaseOrder")
            }
            OrderType::SalesOrder => ("sales_orders", "sales_order_lines", "SalesOrder"),
        };

        let cid = company_id.to_string();
        let as_of = as_of_date.to_string();
        let t_orders = table_orders.to_string();
        let t_lines = table_lines.to_string();
        let ot_string = ot_str.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let as_of = as_of.clone();
            let t_orders = t_orders.clone();
            let t_lines = t_lines.clone();
            let ot_string = ot_string.clone();

            Box::pin(async move {
                let sql = format!(
                    "SELECT o.id as order_id, o.order_number, o.order_date, o.due_date,
                            o.party_ledger_id, l.name as party_name,
                            ol.stock_item_id, si.name as stock_item_name,
                            ol.godown_id, g.name as godown_name,
                            ol.ordered_qty, ol.fulfilled_qty, ol.unit_rate,
                            ol.discount_kind, ol.discount_value
                     FROM {} o
                     JOIN {} ol ON ol.company_id = o.company_id AND ol.order_id = o.id
                     JOIN ledgers l ON l.company_id = o.company_id AND l.id = o.party_ledger_id
                     JOIN stock_items si ON si.company_id = o.company_id AND si.id = ol.stock_item_id
                     LEFT JOIN godowns g ON g.company_id = o.company_id AND g.id = ol.godown_id
                     WHERE o.company_id = ?
                       AND o.status IN ('Open', 'PartiallyFulfilled')
                       AND o.order_date <= ?
                     ORDER BY o.order_date ASC, o.order_number ASC, ol.id ASC",
                    t_orders, t_lines
                );

                let rows = sqlx::query(&sql)
                    .bind(&cid)
                    .bind(&as_of)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                let mut items = Vec::new();
                let mut total_qty = Decimal::ZERO;
                let mut total_amount = Decimal::ZERO;
                let as_of_d = NaiveDate::parse_from_str(&as_of, "%Y-%m-%d").unwrap();

                for r in rows {
                    let ordered_qty: String = r.get("ordered_qty");
                    let fulfilled_qty: String = r.get("fulfilled_qty");
                    let unit_rate_str: String = r.get("unit_rate");
                    let due_date_opt: Option<String> = r.get("due_date");
                    let discount_kind: Option<String> = r.get("discount_kind");
                    let discount_val: Option<Decimal> = r
                        .get::<Option<String>, _>("discount_value")
                        .and_then(|v| Decimal::from_str(&v).ok());

                    let o_dec = Decimal::from_str(&ordered_qty).unwrap_or(Decimal::ZERO);
                    let f_dec = Decimal::from_str(&fulfilled_qty).unwrap_or(Decimal::ZERO);
                    let pending_dec = (o_dec - f_dec).max(Decimal::ZERO);

                    if pending_dec <= Decimal::ZERO {
                        continue;
                    }

                    let rate_dec = Decimal::from_str(&unit_rate_str).unwrap_or(Decimal::ZERO);
                    let base_pending_amt = pending_dec * rate_dec;
                    let discount_amt = match discount_kind.as_deref() {
                        Some("Percentage") => {
                            let pct = discount_val.unwrap_or(Decimal::ZERO);
                            base_pending_amt * (pct / Decimal::from(100))
                        }
                        Some("FixedAmount") => {
                            let fixed = discount_val.unwrap_or(Decimal::ZERO);
                            if o_dec > Decimal::ZERO {
                                fixed * (pending_dec / o_dec)
                            } else {
                                Decimal::ZERO
                            }
                        }
                        _ => Decimal::ZERO,
                    };
                    let pending_amount = (base_pending_amt - discount_amt).max(Decimal::ZERO);

                    let is_overdue = match &due_date_opt {
                        Some(due) => match NaiveDate::parse_from_str(due, "%Y-%m-%d") {
                            Ok(d) => d < as_of_d,
                            Err(_) => false,
                        },
                        None => false,
                    };

                    total_qty += pending_dec;
                    total_amount += pending_amount;

                    items.push(PendingOrderItem {
                        order_id: r.get("order_id"),
                        order_number: r.get("order_number"),
                        order_date: r.get("order_date"),
                        due_date: due_date_opt,
                        party_ledger_id: r.get("party_ledger_id"),
                        party_name: r.get("party_name"),
                        stock_item_id: r.get("stock_item_id"),
                        stock_item_name: r.get("stock_item_name"),
                        godown_id: r.get("godown_id"),
                        godown_name: r.get("godown_name"),
                        ordered_qty,
                        fulfilled_qty,
                        pending_qty: pending_dec.to_string(),
                        unit_rate: unit_rate_str,
                        pending_amount: pending_amount.to_string(),
                        is_overdue,
                    });
                }

                Ok(PendingOrdersReport {
                    company_id: cid,
                    order_type: ot_string,
                    as_of_date: as_of,
                    items,
                    total_pending_qty: total_qty.to_string(),
                    total_pending_amount: total_amount.to_string(),
                })
            })
        })
        .await
    }
}
