use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BillOfMaterials {
    pub id: String,
    pub company_id: String,
    pub finished_stock_item_id: String,
    pub bom_name: String,
    pub base_output_qty: String,
    pub default_godown_id: Option<String>,
    pub is_active: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BomComponent {
    pub id: String,
    pub company_id: String,
    pub bom_id: String,
    pub raw_stock_item_id: String,
    pub source_godown_id: Option<String>,
    pub required_qty: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomComponentDetail {
    pub id: String,
    pub raw_stock_item_id: String,
    pub raw_stock_item_name: String,
    pub source_godown_id: Option<String>,
    pub source_godown_name: Option<String>,
    pub required_qty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomDetail {
    pub id: String,
    pub company_id: String,
    pub finished_stock_item_id: String,
    pub finished_stock_item_name: String,
    pub bom_name: String,
    pub base_output_qty: String,
    pub default_godown_id: Option<String>,
    pub default_godown_name: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub components: Vec<BomComponentDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBomComponentInput {
    pub raw_stock_item_id: String,
    pub source_godown_id: Option<String>,
    pub required_qty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBomInput {
    pub company_id: String,
    pub finished_stock_item_id: String,
    pub bom_name: String,
    pub base_output_qty: String,
    pub default_godown_id: Option<String>,
    pub components: Vec<CreateBomComponentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdditionalOverheadCostInput {
    pub overhead_ledger_id: String,
    pub amount: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteManufacturingJournalInput {
    pub company_id: String,
    pub bom_id: String,
    pub production_date: String,
    pub produced_qty: String,
    pub destination_godown_id: String,
    pub batch_number: Option<String>,
    pub mfg_date: Option<String>,
    pub expiry_date: Option<String>,
    pub additional_overheads: Vec<AdditionalOverheadCostInput>,
    pub narration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumedComponentResult {
    pub raw_stock_item_id: String,
    pub raw_stock_item_name: String,
    pub godown_id: String,
    pub consumed_qty: String,
    pub consumed_cost: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturingJournalResult {
    pub stock_voucher_id: String,
    pub finished_goods_cost_layer_id: String,
    pub produced_qty: String,
    pub landed_unit_cost: String,
    pub total_raw_material_cost: String,
    pub total_overhead_cost: String,
    pub total_manufacturing_cost: String,
    pub consumed_components: Vec<ConsumedComponentResult>,
}

pub struct BomService;

impl BomService {
    /// Creates a new Bill of Materials recipe.
    pub async fn create_bom(
        pool: &SqlitePool,
        input: CreateBomInput,
        actor: &str,
    ) -> Result<BomDetail, OpenTallyError> {
        let bom_name = input.bom_name.trim();
        if bom_name.is_empty() || bom_name.len() > 100 {
            return Err(OpenTallyError::Validation(
                "BOM name must be between 1 and 100 characters".into(),
            ));
        }

        let base_output_qty = Decimal::from_str(&input.base_output_qty).map_err(|_| {
            OpenTallyError::Validation("Base output quantity must be a valid decimal number".into())
        })?;
        if base_output_qty <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Base output quantity must be strictly greater than zero".into(),
            ));
        }

        if input.components.is_empty() {
            return Err(OpenTallyError::Validation(
                "Bill of Materials must contain at least one raw material component".into(),
            ));
        }

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        // Verify company and fetch financial_year_begins
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        // Verify finished item exists
        let finished_item_name: String =
            sqlx::query_scalar("SELECT name FROM stock_items WHERE company_id = ? AND id = ?")
                .bind(&input.company_id)
                .bind(&input.finished_stock_item_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!(
                        "Finished stock item '{}' not found in company",
                        input.finished_stock_item_id
                    ))
                })?;

        // Validate components
        let mut seen_raw_items = std::collections::HashSet::new();
        let mut validated_components = Vec::new();

        for c in &input.components {
            if c.raw_stock_item_id == input.finished_stock_item_id {
                return Err(OpenTallyError::Validation(format!(
                    "Self-referential BOM component rejected: Finished item '{}' cannot be used as its own raw component",
                    finished_item_name
                )));
            }
            if !seen_raw_items.insert(&c.raw_stock_item_id) {
                return Err(OpenTallyError::Validation(format!(
                    "Duplicate raw material item '{}' in Bill of Materials",
                    c.raw_stock_item_id
                )));
            }

            let req_qty = Decimal::from_str(&c.required_qty).map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Invalid required quantity '{}' for component",
                    c.required_qty
                ))
            })?;
            if req_qty <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(format!(
                    "Required quantity for component '{}' must be strictly positive",
                    c.raw_stock_item_id
                )));
            }

            let (raw_item_name,): (String,) =
                sqlx::query_as("SELECT name FROM stock_items WHERE company_id = ? AND id = ?")
                    .bind(&input.company_id)
                    .bind(&c.raw_stock_item_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| {
                        OpenTallyError::NotFound(format!(
                            "Raw stock item '{}' not found in company",
                            c.raw_stock_item_id
                        ))
                    })?;

            let source_godown_name =
                if let Some(ref gid) = c.source_godown_id {
                    let gname: String = sqlx::query_scalar(
                        "SELECT name FROM godowns WHERE company_id = ? AND id = ?",
                    )
                    .bind(&input.company_id)
                    .bind(gid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| {
                        OpenTallyError::NotFound(format!("Godown '{}' not found in company", gid))
                    })?;
                    Some(gname)
                } else {
                    None
                };

            validated_components.push((
                c.raw_stock_item_id.clone(),
                raw_item_name,
                c.source_godown_id.clone(),
                source_godown_name,
                req_qty.normalize().to_string(),
            ));
        }

        let default_godown_name = if let Some(ref gid) = input.default_godown_id {
            let gname: String =
                sqlx::query_scalar("SELECT name FROM godowns WHERE company_id = ? AND id = ?")
                    .bind(&input.company_id)
                    .bind(gid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| {
                        OpenTallyError::NotFound(format!("Godown '{}' not found in company", gid))
                    })?;
            Some(gname)
        } else {
            None
        };

        // Check duplicate BOM name for this finished item
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM bill_of_materials WHERE company_id = ? AND finished_stock_item_id = ? AND bom_name = ? COLLATE NOCASE",
        )
        .bind(&input.company_id)
        .bind(&input.finished_stock_item_id)
        .bind(bom_name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if existing.is_some() {
            return Err(OpenTallyError::Validation(format!(
                "A Bill of Materials named '{}' already exists for '{}'",
                bom_name, finished_item_name
            )));
        }

        let bom_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO bill_of_materials (
                id, company_id, finished_stock_item_id, bom_name, base_output_qty, default_godown_id, is_active, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&bom_id)
        .bind(&input.company_id)
        .bind(&input.finished_stock_item_id)
        .bind(bom_name)
        .bind(base_output_qty.normalize().to_string())
        .bind(input.default_godown_id.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut detail_components = Vec::new();
        for (raw_id, raw_name, godown_id, godown_name, req_qty_str) in validated_components {
            let comp_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO bom_components (
                    id, company_id, bom_id, raw_stock_item_id, source_godown_id, required_qty, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
            )
            .bind(&comp_id)
            .bind(&input.company_id)
            .bind(&bom_id)
            .bind(&raw_id)
            .bind(godown_id.as_deref())
            .bind(&req_qty_str)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            detail_components.push(BomComponentDetail {
                id: comp_id,
                raw_stock_item_id: raw_id,
                raw_stock_item_name: raw_name,
                source_godown_id: godown_id,
                source_godown_name: godown_name,
                required_qty: req_qty_str,
            });
        }

        let payload = serde_json::json!({
            "action": "CREATE_BOM",
            "bom_id": &bom_id,
            "bom_name": bom_name,
            "finished_stock_item_id": &input.finished_stock_item_id,
            "base_output_qty": base_output_qty.normalize().to_string(),
            "component_count": detail_components.len(),
        })
        .to_string();

        Self::record_bom_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "bill_of_materials",
            &bom_id,
            "CREATE",
            actor,
            &payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        let now_str = chrono::Utc::now().to_rfc3339();

        Ok(BomDetail {
            id: bom_id,
            company_id: input.company_id,
            finished_stock_item_id: input.finished_stock_item_id,
            finished_stock_item_name: finished_item_name,
            bom_name: bom_name.to_string(),
            base_output_qty: base_output_qty.normalize().to_string(),
            default_godown_id: input.default_godown_id,
            default_godown_name,
            is_active: true,
            created_at: now_str.clone(),
            updated_at: now_str,
            components: detail_components,
        })
    }

    /// Fetches the details of a Bill of Materials.
    pub async fn get_bom_detail(
        pool: &SqlitePool,
        company_id: &str,
        bom_id: &str,
    ) -> Result<BomDetail, OpenTallyError> {
        let bom: Option<BillOfMaterials> = sqlx::query_as(
            "SELECT id, company_id, finished_stock_item_id, bom_name, base_output_qty, default_godown_id, is_active, created_at, updated_at
             FROM bill_of_materials
             WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(bom_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let bom = match bom {
            Some(b) => b,
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Bill of Materials '{}' not found",
                    bom_id
                )));
            }
        };

        let finished_stock_item_name: String =
            sqlx::query_scalar("SELECT name FROM stock_items WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(&bom.finished_stock_item_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        let default_godown_name: Option<String> = if let Some(ref gid) = bom.default_godown_id {
            sqlx::query_scalar("SELECT name FROM godowns WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(gid)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?
        } else {
            None
        };

        #[derive(sqlx::FromRow)]
        struct CompRow {
            id: String,
            raw_stock_item_id: String,
            raw_stock_item_name: String,
            source_godown_id: Option<String>,
            source_godown_name: Option<String>,
            required_qty: String,
        }

        let comps: Vec<CompRow> = sqlx::query_as(
            "SELECT bc.id, bc.raw_stock_item_id, si.name AS raw_stock_item_name,
                    bc.source_godown_id, g.name AS source_godown_name, bc.required_qty
             FROM bom_components bc
             JOIN stock_items si ON si.company_id = bc.company_id AND si.id = bc.raw_stock_item_id
             LEFT JOIN godowns g ON g.company_id = bc.company_id AND g.id = bc.source_godown_id
             WHERE bc.company_id = ? AND bc.bom_id = ?
             ORDER BY bc.created_at ASC",
        )
        .bind(company_id)
        .bind(bom_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let components = comps
            .into_iter()
            .map(|c| BomComponentDetail {
                id: c.id,
                raw_stock_item_id: c.raw_stock_item_id,
                raw_stock_item_name: c.raw_stock_item_name,
                source_godown_id: c.source_godown_id,
                source_godown_name: c.source_godown_name,
                required_qty: c.required_qty,
            })
            .collect();

        Ok(BomDetail {
            id: bom.id,
            company_id: bom.company_id,
            finished_stock_item_id: bom.finished_stock_item_id,
            finished_stock_item_name,
            bom_name: bom.bom_name,
            base_output_qty: bom.base_output_qty,
            default_godown_id: bom.default_godown_id,
            default_godown_name,
            is_active: bom.is_active == 1,
            created_at: bom.created_at,
            updated_at: bom.updated_at,
            components,
        })
    }

    /// Lists all Bill of Materials recipes for a company, optionally filtered by finished item.
    pub async fn list_boms(
        pool: &SqlitePool,
        company_id: &str,
        finished_stock_item_id: Option<&str>,
    ) -> Result<Vec<BomDetail>, OpenTallyError> {
        let boms: Vec<BillOfMaterials> = sqlx::query_as(
            "SELECT id, company_id, finished_stock_item_id, bom_name, base_output_qty, default_godown_id, is_active, created_at, updated_at
             FROM bill_of_materials
             WHERE company_id = ? AND (? IS NULL OR finished_stock_item_id = ?)
             ORDER BY bom_name ASC",
        )
        .bind(company_id)
        .bind(finished_stock_item_id)
        .bind(finished_stock_item_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut results = Vec::new();
        for b in boms {
            let detail = Self::get_bom_detail(pool, company_id, &b.id).await?;
            results.push(detail);
        }

        Ok(results)
    }

    /// Executes a Manufacturing Stock Journal in an atomic `BEGIN IMMEDIATE` transaction.
    ///
    /// Consumes raw-material FIFO cost layers proportionally, creates a finished-goods FIFO layer
    /// at exact landed unit cost, updates stock movements, posts overhead GL lines if applicable,
    /// and logs an immutable MCA audit_log entry with SHA-256 hash chaining.
    pub async fn execute_manufacturing_journal(
        pool: &SqlitePool,
        input: ExecuteManufacturingJournalInput,
        actor: &str,
    ) -> Result<ManufacturingJournalResult, OpenTallyError> {
        let produced_qty = Decimal::from_str(&input.produced_qty).map_err(|_| {
            OpenTallyError::Validation("Produced quantity must be a valid decimal number".into())
        })?;
        if produced_qty <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Produced quantity must be strictly greater than zero".into(),
            ));
        }

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        // Period lock check
        let locked_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM financial_year_closings
             WHERE company_id = ? AND is_locked = 1 AND period_end >= ?",
        )
        .bind(&input.company_id)
        .bind(&input.production_date)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if locked_count > 0 {
            return Err(OpenTallyError::PeriodLocked(format!(
                "Cannot produce goods on '{}': The accounting period is locked by a finalized year-end closing",
                input.production_date
            )));
        }

        // Fetch company financial_year_begins
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(&input.company_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", input.company_id))
                })?;

        // Fetch BOM
        let bom: Option<BillOfMaterials> = sqlx::query_as(
            "SELECT id, company_id, finished_stock_item_id, bom_name, base_output_qty, default_godown_id, is_active, created_at, updated_at
             FROM bill_of_materials
             WHERE company_id = ? AND id = ?",
        )
        .bind(&input.company_id)
        .bind(&input.bom_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let bom = match bom {
            Some(b) => b,
            None => {
                return Err(OpenTallyError::NotFound(format!(
                    "Bill of Materials '{}' not found",
                    input.bom_id
                )));
            }
        };

        if bom.is_active != 1 {
            return Err(OpenTallyError::Validation(format!(
                "Bill of Materials '{}' is currently deactivated",
                bom.bom_name
            )));
        }

        let base_output_qty = Decimal::from_str(&bom.base_output_qty).map_err(|_| {
            OpenTallyError::Internal("Corrupted base_output_qty in database".into())
        })?;
        if base_output_qty <= Decimal::ZERO {
            return Err(OpenTallyError::Internal(
                "Base output qty must be > 0".into(),
            ));
        }

        // Verify destination godown
        let dest_godown_name: String =
            sqlx::query_scalar("SELECT name FROM godowns WHERE company_id = ? AND id = ?")
                .bind(&input.company_id)
                .bind(&input.destination_godown_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| {
                    OpenTallyError::NotFound(format!(
                        "Destination godown '{}' not found",
                        input.destination_godown_id
                    ))
                })?;

        // Scaling factor: produced_qty / base_output_qty
        let scale_ratio = produced_qty / base_output_qty;

        // Fetch components
        let components: Vec<BomComponent> = sqlx::query_as(
            "SELECT id, company_id, bom_id, raw_stock_item_id, source_godown_id, required_qty, created_at
             FROM bom_components
             WHERE company_id = ? AND bom_id = ?
             ORDER BY created_at ASC",
        )
        .bind(&input.company_id)
        .bind(&input.bom_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if components.is_empty() {
            return Err(OpenTallyError::Validation(
                "Bill of Materials has no components configured".into(),
            ));
        }

        // 1. Create Stock Voucher for Manufacturing Journal
        let stock_voucher_id = Uuid::new_v4().to_string();
        let voucher_number = format!("MFG-{}", stock_voucher_id[..8].to_uppercase());

        sqlx::query(
            "INSERT INTO stock_vouchers (
                id, company_id, voucher_number, voucher_type, voucher_date, narration, status, created_at, updated_at
            ) VALUES (?, ?, ?, 'transfer', ?, ?, 'posted', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&stock_voucher_id)
        .bind(&input.company_id)
        .bind(&voucher_number)
        .bind(&input.production_date)
        .bind(input.narration.as_deref().unwrap_or("Manufacturing Stock Journal"))
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut total_raw_material_cost = Decimal::ZERO;
        let mut consumed_results = Vec::new();
        let mut line_number = 1i64;

        // 2. Consume Raw Material FIFO Cost Layers
        for comp in &components {
            let req_qty = Decimal::from_str(&comp.required_qty).unwrap_or(Decimal::ZERO);
            let consume_qty = (scale_ratio * req_qty).round_dp(4);

            let (raw_item_name, base_unit_id): (String, String) = sqlx::query_as(
                "SELECT name, base_unit_id FROM stock_items WHERE company_id = ? AND id = ?",
            )
            .bind(&input.company_id)
            .bind(&comp.raw_stock_item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Determine godown for component: specified source_godown or fallback to destination_godown
            let godown_id = comp
                .source_godown_id
                .as_ref()
                .unwrap_or(&input.destination_godown_id);

            // Fetch available FIFO layers in ascending receipt order
            #[derive(sqlx::FromRow)]
            struct CostLayerRow {
                id: String,
                remaining_quantity: String,
                unit_cost: String,
            }

            let available_layers: Vec<CostLayerRow> = sqlx::query_as(
                "SELECT id, remaining_quantity, unit_cost
                 FROM inventory_cost_layers
                 WHERE company_id = ? AND stock_item_id = ? AND godown_id = ? AND CAST(remaining_quantity AS REAL) > 0
                 ORDER BY receipt_date ASC, created_at ASC, id ASC",
            )
            .bind(&input.company_id)
            .bind(&comp.raw_stock_item_id)
            .bind(godown_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let total_available: Decimal = available_layers
                .iter()
                .map(|l| Decimal::from_str(&l.remaining_quantity).unwrap_or(Decimal::ZERO))
                .sum();

            if total_available < consume_qty {
                return Err(OpenTallyError::Validation(format!(
                    "Insufficient stock for component '{}' in godown. Required: {}, Available: {}",
                    raw_item_name,
                    consume_qty.normalize(),
                    total_available.normalize()
                )));
            }

            let mut needed = consume_qty;
            let mut component_cost = Decimal::ZERO;

            for layer in available_layers {
                if needed <= Decimal::ZERO {
                    break;
                }
                let rem = Decimal::from_str(&layer.remaining_quantity).unwrap_or(Decimal::ZERO);
                let u_cost = Decimal::from_str(&layer.unit_cost).unwrap_or(Decimal::ZERO);
                let take = needed.min(rem);
                let new_rem = rem - take;

                sqlx::query(
                    "UPDATE inventory_cost_layers SET remaining_quantity = ? WHERE company_id = ? AND id = ?",
                )
                .bind(new_rem.normalize().to_string())
                .bind(&input.company_id)
                .bind(&layer.id)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                component_cost += take * u_cost;
                needed -= take;
            }

            total_raw_material_cost += component_cost;

            // Insert stock_voucher_line for consumption
            let sv_line_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_voucher_lines (
                    id, company_id, voucher_id, line_number, stock_item_id, unit_id,
                    source_godown_id, quantity, description
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'BOM Raw Material Consumption')",
            )
            .bind(&sv_line_id)
            .bind(&input.company_id)
            .bind(&stock_voucher_id)
            .bind(line_number)
            .bind(&comp.raw_stock_item_id)
            .bind(&base_unit_id)
            .bind(godown_id)
            .bind(consume_qty.normalize().to_string())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Insert stock_movement (consumption via transfer_out)
            let mv_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO stock_movements (
                    id, company_id, stock_item_id, godown_id, voucher_id, voucher_line_id,
                    movement_date, movement_type, signed_quantity
                ) VALUES (?, ?, ?, ?, ?, ?, ?, 'transfer_out', ?)",
            )
            .bind(&mv_id)
            .bind(&input.company_id)
            .bind(&comp.raw_stock_item_id)
            .bind(godown_id)
            .bind(&stock_voucher_id)
            .bind(&sv_line_id)
            .bind(&input.production_date)
            .bind((-consume_qty).normalize().to_string())
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            consumed_results.push(ConsumedComponentResult {
                raw_stock_item_id: comp.raw_stock_item_id.clone(),
                raw_stock_item_name: raw_item_name,
                godown_id: godown_id.clone(),
                consumed_qty: consume_qty.normalize().to_string(),
                consumed_cost: component_cost.normalize().to_string(),
            });

            line_number += 1;
        }

        // 3. Additional Overhead Expenses
        let mut total_overhead_cost = Decimal::ZERO;
        for ovh in &input.additional_overheads {
            let ovh_amt = Decimal::from_str(&ovh.amount).map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Invalid overhead amount '{}' for ledger '{}'",
                    ovh.amount, ovh.overhead_ledger_id
                ))
            })?;
            if ovh_amt < Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Overhead expense amount cannot be negative".into(),
                ));
            }

            // Verify overhead ledger exists
            let _ledger_name: String =
                sqlx::query_scalar("SELECT name FROM ledgers WHERE company_id = ? AND id = ?")
                    .bind(&input.company_id)
                    .bind(&ovh.overhead_ledger_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|_| {
                        OpenTallyError::NotFound(format!(
                            "Overhead ledger '{}' not found",
                            ovh.overhead_ledger_id
                        ))
                    })?;

            total_overhead_cost += ovh_amt;
        }

        // 4. Calculate Finished Goods Landed Cost & Create Finished Goods FIFO Layer
        let total_manufacturing_cost = total_raw_material_cost + total_overhead_cost;
        let landed_unit_cost = (total_manufacturing_cost / produced_qty).round_dp(4);

        let (finished_item_name, finished_base_unit_id): (String, String) = sqlx::query_as(
            "SELECT name, base_unit_id FROM stock_items WHERE company_id = ? AND id = ?",
        )
        .bind(&input.company_id)
        .bind(&bom.finished_stock_item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Insert stock_voucher_line for production
        let prod_line_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_voucher_lines (
                id, company_id, voucher_id, line_number, stock_item_id, unit_id,
                destination_godown_id, quantity, description
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'BOM Finished Goods Production')",
        )
        .bind(&prod_line_id)
        .bind(&input.company_id)
        .bind(&stock_voucher_id)
        .bind(line_number)
        .bind(&bom.finished_stock_item_id)
        .bind(&finished_base_unit_id)
        .bind(&input.destination_godown_id)
        .bind(produced_qty.normalize().to_string())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Create new inventory_cost_layers row for finished goods
        let fg_layer_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO inventory_cost_layers (
                id, company_id, stock_item_id, godown_id, source_voucher_id, source_line_id,
                receipt_date, original_quantity, remaining_quantity, unit_cost, original_value
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&fg_layer_id)
        .bind(&input.company_id)
        .bind(&bom.finished_stock_item_id)
        .bind(&input.destination_godown_id)
        .bind(&stock_voucher_id)
        .bind(&prod_line_id)
        .bind(&input.production_date)
        .bind(produced_qty.normalize().to_string())
        .bind(produced_qty.normalize().to_string())
        .bind(landed_unit_cost.normalize().to_string())
        .bind(total_manufacturing_cost.normalize().to_string())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // Insert stock_movement (production via transfer_in)
        let fg_mv_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO stock_movements (
                id, company_id, stock_item_id, godown_id, voucher_id, voucher_line_id,
                movement_date, movement_type, signed_quantity
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 'transfer_in', ?)",
        )
        .bind(&fg_mv_id)
        .bind(&input.company_id)
        .bind(&bom.finished_stock_item_id)
        .bind(&input.destination_godown_id)
        .bind(&stock_voucher_id)
        .bind(&prod_line_id)
        .bind(&input.production_date)
        .bind(produced_qty.normalize().to_string())
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // 5. Post Balanced GL Lines for Overhead Allocation if overheads present
        if total_overhead_cost > Decimal::ZERO {
            // Find Stock-in-Hand ledger or default inventory ledger
            let stock_in_hand_ledger: Option<String> = sqlx::query_scalar(
                "SELECT l.id FROM ledgers l
                 JOIN account_groups ag ON ag.company_id = l.company_id AND ag.id = l.group_id
                 WHERE l.company_id = ? AND (ag.name = 'Stock-in-Hand' OR l.name = 'Stock-in-Hand')
                 LIMIT 1",
            )
            .bind(&input.company_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            if let Some(stock_ledger_id) = stock_in_hand_ledger {
                // Post GL voucher
                let gl_vch_id = Uuid::new_v4().to_string();
                let gl_number = format!("GL-MFG-{}", gl_vch_id[..8].to_uppercase());

                // Find Journal voucher type and active series, creating default series if none exists
                let j_vch_series: Option<(String, String, String)> = sqlx::query_as(
                    "SELECT vt.id, vns.id, vns.numbering_method
                     FROM voucher_types vt
                     JOIN voucher_number_series vns ON vns.company_id = vt.company_id AND vns.voucher_type_id = vt.id
                     WHERE vt.company_id = ? AND vt.name = 'Journal'
                     ORDER BY vns.is_active DESC, vns.id ASC
                     LIMIT 1",
                )
                .bind(&input.company_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                let (vt_id, series_id, numbering_method) = match j_vch_series {
                    Some(s) => s,
                    None => {
                        let vt_id: String = sqlx::query_scalar(
                            "SELECT id FROM voucher_types WHERE company_id = ? AND name = 'Journal'",
                        )
                        .bind(&input.company_id)
                        .fetch_one(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                        let sid = format!("vns_jnl_{}", &Uuid::new_v4().to_string()[..8]);
                        sqlx::query(
                            "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
                             VALUES (?, ?, ?, 'General Journal', 'JNL', 'automatic', 'yearly')",
                        )
                        .bind(&sid)
                        .bind(&input.company_id)
                        .bind(&vt_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                        (vt_id, sid, "automatic".to_string())
                    }
                };

                sqlx::query(
                    "INSERT INTO vouchers (
                        id, company_id, voucher_type_id, series_id, numbering_method,
                        voucher_number, date, effective_date, narration, status,
                        total_amount, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'BOM Manufacturing Overhead Capitalization', 'posted', ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                )
                .bind(&gl_vch_id)
                .bind(&input.company_id)
                .bind(&vt_id)
                .bind(&series_id)
                .bind(&numbering_method)
                .bind(&gl_number)
                .bind(&input.production_date)
                .bind(&input.production_date)
                .bind(total_overhead_cost.normalize().to_string())
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                // Dr Stock-in-Hand
                let dr_line_id = Uuid::new_v4().to_string();
                sqlx::query(
                        "INSERT INTO voucher_lines (
                            id, company_id, voucher_id, line_number, ledger_id, amount, entry_type, description
                        ) VALUES (?, ?, ?, 1, ?, ?, 'Dr', 'Manufacturing Overhead Allocated to Inventory')",
                    )
                    .bind(&dr_line_id)
                    .bind(&input.company_id)
                    .bind(&gl_vch_id)
                    .bind(&stock_ledger_id)
                    .bind(total_overhead_cost.normalize().to_string())
                    .execute(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                // Cr Overhead Ledgers
                let mut gl_ln = 2i64;
                for ovh in &input.additional_overheads {
                    let amt = Decimal::from_str(&ovh.amount).unwrap_or(Decimal::ZERO);
                    if amt > Decimal::ZERO {
                        let cr_line_id = Uuid::new_v4().to_string();
                        sqlx::query(
                                "INSERT INTO voucher_lines (
                                    id, company_id, voucher_id, line_number, ledger_id, amount, entry_type, description
                                ) VALUES (?, ?, ?, ?, ?, ?, 'Cr', ?)",
                            )
                            .bind(&cr_line_id)
                            .bind(&input.company_id)
                            .bind(&gl_vch_id)
                            .bind(gl_ln)
                            .bind(&ovh.overhead_ledger_id)
                            .bind(amt.normalize().to_string())
                            .bind(ovh.description.as_deref().unwrap_or("Overhead Absorption"))
                            .execute(&mut *tx)
                            .await
                            .map_err(OpenTallyError::Database)?;
                        gl_ln += 1;
                    }
                }
            }
        }

        // 6. Record MCA Audit Log
        let audit_payload = serde_json::json!({
            "action": "EXECUTE_MANUFACTURING_JOURNAL",
            "stock_voucher_id": &stock_voucher_id,
            "bom_id": &bom.id,
            "bom_name": &bom.bom_name,
            "finished_stock_item_id": &bom.finished_stock_item_id,
            "finished_stock_item_name": &finished_item_name,
            "destination_godown_id": &input.destination_godown_id,
            "destination_godown_name": &dest_godown_name,
            "produced_qty": produced_qty.normalize().to_string(),
            "total_raw_material_cost": total_raw_material_cost.normalize().to_string(),
            "total_overhead_cost": total_overhead_cost.normalize().to_string(),
            "total_manufacturing_cost": total_manufacturing_cost.normalize().to_string(),
            "landed_unit_cost": landed_unit_cost.normalize().to_string(),
        })
        .to_string();

        Self::record_bom_audit_log(
            &mut tx,
            &input.company_id,
            &fy_begins,
            "stock_vouchers",
            &stock_voucher_id,
            "CREATE",
            actor,
            &audit_payload,
        )
        .await?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(ManufacturingJournalResult {
            stock_voucher_id,
            finished_goods_cost_layer_id: fg_layer_id,
            produced_qty: produced_qty.normalize().to_string(),
            landed_unit_cost: landed_unit_cost.normalize().to_string(),
            total_raw_material_cost: total_raw_material_cost.normalize().to_string(),
            total_overhead_cost: total_overhead_cost.normalize().to_string(),
            total_manufacturing_cost: total_manufacturing_cost.normalize().to_string(),
            consumed_components: consumed_results,
        })
    }

    /// Internal helper to record an immutable MCA audit_log row with SHA-256 hash chaining.
    #[allow(clippy::too_many_arguments)]
    async fn record_bom_audit_log(
        conn: &mut SqliteConnection,
        company_id: &str,
        financial_year_begins: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        performed_by: &str,
        payload: &str,
    ) -> Result<(), OpenTallyError> {
        let prev_hash_opt: Option<String> = sqlx::query_scalar(
            "SELECT entry_hash FROM audit_log WHERE company_id = ? AND entry_hash IS NOT NULL ORDER BY timestamp DESC, id DESC LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let prev = prev_hash_opt.unwrap_or_else(|| "GENESIS".to_string());
        let audit_id = Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let entry_hash = AuditCertificationService::compute_entry_hash(
            &prev,
            &audit_id,
            company_id,
            entity_type,
            entity_id,
            action,
            &timestamp,
            Some(payload),
        );

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, financial_year_begins,
                action, new_values, performed_by, timestamp, prev_hash, entry_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(entity_type)
        .bind(entity_id)
        .bind(financial_year_begins)
        .bind(action)
        .bind(payload)
        .bind(performed_by)
        .bind(&timestamp)
        .bind(&prev)
        .bind(&entry_hash)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }

    /// Computes net finished-good unit cost after crediting scrap/by-product recovery value.
    pub fn compute_net_unit_cost_with_byproducts(
        raw_cost: Decimal,
        overhead_cost: Decimal,
        byproduct_recovery_value: Decimal,
        produced_qty: Decimal,
    ) -> Result<Decimal, OpenTallyError> {
        if produced_qty <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "produced_qty must be greater than zero".into(),
            ));
        }
        let net_total = raw_cost + overhead_cost - byproduct_recovery_value;
        if net_total < Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "By-product recovery value cannot exceed total raw + overhead cost".into(),
            ));
        }
        Ok((net_total / produced_qty).round_dp(4))
    }

    /// Recursively explodes a multi-level BOM down to leaf raw materials with cycle detection.
    pub async fn expand_multilevel_bom(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        company_id: &str,
        bom_id: &str,
        target_qty: Decimal,
    ) -> Result<Vec<(String, Decimal)>, OpenTallyError> {
        if target_qty <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "target_qty must be greater than zero".into(),
            ));
        }

        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut stack: Vec<(String, Decimal, Vec<String>)> =
            vec![(bom_id.to_string(), target_qty, vec![bom_id.to_string()])];
        let mut leaf_totals: std::collections::HashMap<String, Decimal> =
            std::collections::HashMap::new();

        while let Some((cur_bom_id, qty_needed, path)) = stack.pop() {
            visited.insert(cur_bom_id.clone());
            let detail = Self::get_bom_detail(pool, company_id, &cur_bom_id).await?;
            let std_qty = Decimal::ONE;
            let scale = qty_needed / std_qty;

            for comp in detail.components {
                let comp_qty = (Decimal::from_str(&comp.required_qty).unwrap_or(Decimal::ZERO)
                    * scale)
                    .round_dp(4);
                let sub_bom_id: Option<String> = sqlx::query_scalar(
                    "SELECT id FROM boms WHERE company_id = ? AND finished_stock_item_id = ? AND is_active = 1 LIMIT 1",
                )
                .bind(company_id)
                .bind(&comp.raw_stock_item_id)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?;

                if let Some(sub_id) = sub_bom_id {
                    if path.contains(&sub_id) {
                        return Err(OpenTallyError::Validation(format!(
                            "Circular BOM dependency detected involving BOM '{}'",
                            sub_id
                        )));
                    }
                    let mut next_path = path.clone();
                    next_path.push(sub_id.clone());
                    stack.push((sub_id, comp_qty, next_path));
                } else {
                    *leaf_totals
                        .entry(comp.raw_stock_item_id)
                        .or_insert(Decimal::ZERO) += comp_qty;
                }
            }
        }

        let mut out: Vec<(String, Decimal)> = leaf_totals.into_iter().collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }
}
