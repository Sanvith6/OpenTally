//! Milestone 18: Safe Declarative Custom Report Extensions (Clean-Room TDL Alternative)
//!
//! Compile-time static allowlist per `DataSource` (`Vouchers`, `Ledgers`, `StockItems`,
//! `OutstandingBills`, `GstInvoices`), executing strictly inside `ReadOnlyConnectionGuard`
//! (`execute_read_only_query`), computing exact `rust_decimal` totals, and exporting
//! CWE-1236-safe CSV or shareable `.opentally-report.json` templates.

use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use crate::vouchers::service::{escape_rfc4180, execute_read_only_query, sanitize_text_cell};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomReportDataSource {
    Vouchers,
    Ledgers,
    StockItems,
    OutstandingBills,
    GstInvoices,
}

impl CustomReportDataSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vouchers => "Vouchers",
            Self::Ledgers => "Ledgers",
            Self::StockItems => "StockItems",
            Self::OutstandingBills => "OutstandingBills",
            Self::GstInvoices => "GstInvoices",
        }
    }

    pub fn parse(s: &str) -> Result<Self, OpenTallyError> {
        match s {
            "Vouchers" => Ok(Self::Vouchers),
            "Ledgers" => Ok(Self::Ledgers),
            "StockItems" => Ok(Self::StockItems),
            "OutstandingBills" => Ok(Self::OutstandingBills),
            "GstInvoices" => Ok(Self::GstInvoices),
            other => Err(OpenTallyError::Validation(format!(
                "Unsupported custom report data_source '{}'",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomReportFilter {
    pub field: String,
    pub op: String, // eq | contains | gte | lte | between
    pub value: String,
    pub value_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomReportDefinitionDto {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub description: String,
    pub data_source: String,
    pub columns: Vec<String>,
    pub filters: Vec<CustomReportFilter>,
    pub sort_by: Option<String>,
    pub sort_dir: String,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveCustomReportInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub data_source: String,
    pub columns: Vec<String>,
    pub filters: Option<Vec<CustomReportFilter>>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomReportExecutionResult {
    pub report_id: String,
    pub report_name: String,
    pub data_source: String,
    pub columns: Vec<String>,
    pub rows: Vec<HashMap<String, String>>,
    pub numeric_totals: HashMap<String, String>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomReportTemplateFile {
    pub format: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub data_source: String,
    pub columns: Vec<String>,
    pub filters: Vec<CustomReportFilter>,
    pub sort_by: Option<String>,
    pub sort_dir: String,
}

struct ColumnMeta {
    key: &'static str,
    sql_expr: &'static str,
    is_numeric: bool,
}

fn allowed_columns(ds: CustomReportDataSource) -> &'static [ColumnMeta] {
    match ds {
        CustomReportDataSource::Vouchers => &[
            ColumnMeta {
                key: "id",
                sql_expr: "v.id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "voucher_number",
                sql_expr: "v.voucher_number",
                is_numeric: false,
            },
            ColumnMeta {
                key: "date",
                sql_expr: "v.date",
                is_numeric: false,
            },
            ColumnMeta {
                key: "status",
                sql_expr: "v.status",
                is_numeric: false,
            },
            ColumnMeta {
                key: "voucher_type_id",
                sql_expr: "v.voucher_type_id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "reference_no",
                sql_expr: "COALESCE(v.reference_no, '')",
                is_numeric: false,
            },
            ColumnMeta {
                key: "narration",
                sql_expr: "COALESCE(v.narration, '')",
                is_numeric: false,
            },
            ColumnMeta {
                key: "total_amount",
                sql_expr: "v.total_amount",
                is_numeric: true,
            },
        ],
        CustomReportDataSource::Ledgers => &[
            ColumnMeta {
                key: "id",
                sql_expr: "l.id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "name",
                sql_expr: "l.name",
                is_numeric: false,
            },
            ColumnMeta {
                key: "group_id",
                sql_expr: "l.group_id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "ledger_classification",
                sql_expr: "l.ledger_classification",
                is_numeric: false,
            },
            ColumnMeta {
                key: "is_reserved",
                sql_expr: "CAST(l.is_reserved AS TEXT)",
                is_numeric: false,
            },
        ],
        CustomReportDataSource::StockItems => &[
            ColumnMeta {
                key: "id",
                sql_expr: "si.id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "name",
                sql_expr: "si.name",
                is_numeric: false,
            },
            ColumnMeta {
                key: "sku",
                sql_expr: "COALESCE(si.sku, '')",
                is_numeric: false,
            },
            ColumnMeta {
                key: "hsn_sac_code",
                sql_expr: "COALESCE(si.hsn_sac_code, '')",
                is_numeric: false,
            },
            ColumnMeta {
                key: "opening_qty",
                sql_expr: "COALESCE(si.opening_qty, '0')",
                is_numeric: true,
            },
            ColumnMeta {
                key: "opening_value",
                sql_expr: "COALESCE(si.opening_value, '0')",
                is_numeric: true,
            },
        ],
        CustomReportDataSource::OutstandingBills => &[
            ColumnMeta {
                key: "id",
                sql_expr: "ba.id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "bill_name",
                sql_expr: "ba.bill_name",
                is_numeric: false,
            },
            ColumnMeta {
                key: "party_ledger_id",
                sql_expr: "ba.party_ledger_id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "allocation_type",
                sql_expr: "ba.allocation_type",
                is_numeric: false,
            },
            ColumnMeta {
                key: "dr_cr",
                sql_expr: "ba.dr_cr",
                is_numeric: false,
            },
            ColumnMeta {
                key: "amount",
                sql_expr: "ba.amount",
                is_numeric: true,
            },
            ColumnMeta {
                key: "due_date",
                sql_expr: "COALESCE(ba.due_date, '')",
                is_numeric: false,
            },
        ],
        CustomReportDataSource::GstInvoices => &[
            ColumnMeta {
                key: "id",
                sql_expr: "sd.id",
                is_numeric: false,
            },
            ColumnMeta {
                key: "document_number",
                sql_expr: "sd.document_number",
                is_numeric: false,
            },
            ColumnMeta {
                key: "document_date",
                sql_expr: "sd.document_date",
                is_numeric: false,
            },
            ColumnMeta {
                key: "status",
                sql_expr: "sd.status",
                is_numeric: false,
            },
            ColumnMeta {
                key: "grand_total",
                sql_expr: "sd.grand_total",
                is_numeric: true,
            },
            ColumnMeta {
                key: "total_tax",
                sql_expr: "sd.total_tax",
                is_numeric: true,
            },
        ],
    }
}

fn base_from_clause(ds: CustomReportDataSource) -> &'static str {
    match ds {
        CustomReportDataSource::Vouchers => "FROM vouchers v WHERE v.company_id = ?",
        CustomReportDataSource::Ledgers => "FROM ledgers l WHERE l.company_id = ?",
        CustomReportDataSource::StockItems => "FROM stock_items si WHERE si.company_id = ?",
        CustomReportDataSource::OutstandingBills => {
            "FROM bill_allocations ba WHERE ba.company_id = ? AND ba.is_cancelled = 0"
        }
        CustomReportDataSource::GstInvoices => "FROM sales_documents sd WHERE sd.company_id = ?",
    }
}

fn validate_definition(
    ds: CustomReportDataSource,
    columns: &[String],
    filters: &[CustomReportFilter],
    sort_by: Option<&str>,
    sort_dir: &str,
) -> Result<(), OpenTallyError> {
    if columns.is_empty() {
        return Err(OpenTallyError::Validation(
            "Custom report must select at least one column.".into(),
        ));
    }
    let allowed = allowed_columns(ds);
    for col in columns {
        if !allowed.iter().any(|c| c.key == col) {
            return Err(OpenTallyError::Validation(format!(
                "Column '{}' is not in the allowlist for data_source '{}'",
                col,
                ds.as_str()
            )));
        }
    }
    for f in filters {
        if !allowed.iter().any(|c| c.key == f.field) {
            return Err(OpenTallyError::Validation(format!(
                "Filter field '{}' is not in the allowlist for data_source '{}'",
                f.field,
                ds.as_str()
            )));
        }
        if !matches!(f.op.as_str(), "eq" | "contains" | "gte" | "lte" | "between") {
            return Err(OpenTallyError::Validation(format!(
                "Unsupported filter operator '{}'",
                f.op
            )));
        }
        if f.op == "between" && f.value_end.is_none() {
            return Err(OpenTallyError::Validation(
                "Filter operator 'between' requires 'value_end'".into(),
            ));
        }
    }
    if let Some(sb) = sort_by {
        if !sb.trim().is_empty() && !allowed.iter().any(|c| c.key == sb) {
            return Err(OpenTallyError::Validation(format!(
                "sort_by '{}' is not in the allowlist for data_source '{}'",
                sb,
                ds.as_str()
            )));
        }
    }
    if !matches!(sort_dir, "ASC" | "DESC") {
        return Err(OpenTallyError::Validation(format!(
            "Invalid sort_dir '{}'. Expected 'ASC' or 'DESC'.",
            sort_dir
        )));
    }
    Ok(())
}

pub struct CustomReportService;

impl CustomReportService {
    pub async fn list_reports(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<CustomReportDefinitionDto>, OpenTallyError> {
        let cid = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            Box::pin(async move {
                #[allow(clippy::type_complexity)]
                let rows: Vec<(
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    Option<String>,
                    String,
                    String,
                    String,
                    String,
                )> = sqlx::query_as(
                    "SELECT id, company_id, name, description, data_source, columns_json, filters_json, sort_by, sort_dir, created_by, created_at, updated_at
                     FROM custom_report_definitions WHERE company_id = ? ORDER BY name ASC",
                )
                .bind(&cid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut out = Vec::with_capacity(rows.len());
                for (id, comp_id, name, desc, ds, cols_j, flt_j, sb, sd, cby, cat, uat) in rows {
                    let columns: Vec<String> = serde_json::from_str(&cols_j).unwrap_or_default();
                    let filters: Vec<CustomReportFilter> =
                        serde_json::from_str(&flt_j).unwrap_or_default();
                    out.push(CustomReportDefinitionDto {
                        id,
                        company_id: comp_id,
                        name,
                        description: desc,
                        data_source: ds,
                        columns,
                        filters,
                        sort_by: sb,
                        sort_dir: sd,
                        created_by: cby,
                        created_at: cat,
                        updated_at: uat,
                    });
                }
                Ok(out)
            })
        })
        .await
    }

    pub async fn save_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: SaveCustomReportInput,
        actor: &str,
    ) -> Result<CustomReportDefinitionDto, OpenTallyError> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(OpenTallyError::Validation(
                "Custom report name cannot be empty.".into(),
            ));
        }
        let ds = CustomReportDataSource::parse(&input.data_source)?;
        let filters = input.filters.unwrap_or_default();
        let sort_dir = input.sort_dir.unwrap_or_else(|| "ASC".to_string());
        validate_definition(
            ds,
            &input.columns,
            &filters,
            input.sort_by.as_deref(),
            &sort_dir,
        )?;

        let id = input
            .id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("crpt_{}", uuid::Uuid::new_v4().simple()));
        let cols_json = serde_json::to_string(&input.columns)
            .map_err(|e| OpenTallyError::Internal(e.to_string()))?;
        let flt_json =
            serde_json::to_string(&filters).map_err(|e| OpenTallyError::Internal(e.to_string()))?;
        let desc = input.description.unwrap_or_default();

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::query(
            r#"
            INSERT INTO custom_report_definitions (id, company_id, name, description, data_source, columns_json, filters_json, sort_by, sort_dir, created_by, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                data_source = excluded.data_source,
                columns_json = excluded.columns_json,
                filters_json = excluded.filters_json,
                sort_by = excluded.sort_by,
                sort_dir = excluded.sort_dir,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(&name)
        .bind(&desc)
        .bind(ds.as_str())
        .bind(&cols_json)
        .bind(&flt_json)
        .bind(&input.sort_by)
        .bind(&sort_dir)
        .bind(actor)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let details = format!("Saved custom report '{}' ({})", name, ds.as_str());
        let _ = AuditCertificationService::record_chained_event(
            pool,
            company_id,
            "CUSTOM_REPORT",
            &id,
            "SAVE",
            actor,
            Some(&details),
        )
        .await;

        let all = Self::list_reports(pool, company_id).await?;
        all.into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| OpenTallyError::Internal("Saved custom report missing".into()))
    }

    pub async fn delete_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        report_id: &str,
        actor: &str,
    ) -> Result<(), OpenTallyError> {
        let res =
            sqlx::query("DELETE FROM custom_report_definitions WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(report_id)
                .execute(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        if res.rows_affected() == 0 {
            return Err(OpenTallyError::NotFound(format!(
                "Custom report '{}' not found",
                report_id
            )));
        }

        let _ = AuditCertificationService::record_chained_event(
            pool,
            company_id,
            "CUSTOM_REPORT",
            report_id,
            "DELETE",
            actor,
            Some("Deleted custom report definition"),
        )
        .await;

        Ok(())
    }

    pub async fn run_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        report_id: &str,
    ) -> Result<CustomReportExecutionResult, OpenTallyError> {
        let all = Self::list_reports(pool, company_id).await?;
        let def = all.into_iter().find(|r| r.id == report_id).ok_or_else(|| {
            OpenTallyError::NotFound(format!("Custom report '{}' not found", report_id))
        })?;

        let ds = CustomReportDataSource::parse(&def.data_source)?;
        validate_definition(
            ds,
            &def.columns,
            &def.filters,
            def.sort_by.as_deref(),
            &def.sort_dir,
        )?;

        let allowed = allowed_columns(ds);
        let select_exprs: Vec<String> = def
            .columns
            .iter()
            .map(|col| {
                let meta = allowed.iter().find(|c| c.key == col).unwrap();
                format!("{} AS \"{}\"", meta.sql_expr, meta.key)
            })
            .collect();

        let mut sql = format!(
            "SELECT {} {}",
            select_exprs.join(", "),
            base_from_clause(ds)
        );
        let mut bind_values: Vec<String> = Vec::new();

        for f in &def.filters {
            let meta = allowed.iter().find(|c| c.key == f.field).unwrap();
            match f.op.as_str() {
                "eq" => {
                    sql.push_str(&format!(" AND {} = ?", meta.sql_expr));
                    bind_values.push(f.value.clone());
                }
                "contains" => {
                    sql.push_str(&format!(" AND {} LIKE ?", meta.sql_expr));
                    bind_values.push(format!("%{}%", f.value));
                }
                "gte" => {
                    sql.push_str(&format!(" AND {} >= ?", meta.sql_expr));
                    bind_values.push(f.value.clone());
                }
                "lte" => {
                    sql.push_str(&format!(" AND {} <= ?", meta.sql_expr));
                    bind_values.push(f.value.clone());
                }
                "between" => {
                    sql.push_str(&format!(" AND {} BETWEEN ? AND ?", meta.sql_expr));
                    bind_values.push(f.value.clone());
                    bind_values.push(f.value_end.clone().unwrap_or_default());
                }
                _ => {}
            }
        }

        if let Some(ref sb) = def.sort_by {
            if !sb.trim().is_empty() {
                let meta = allowed.iter().find(|c| c.key == sb).unwrap();
                sql.push_str(&format!(" ORDER BY {} {}", meta.sql_expr, def.sort_dir));
            }
        }
        sql.push_str(" LIMIT 5000");

        let cid = company_id.to_string();
        let cols_clone = def.columns.clone();
        let rows_out = execute_read_only_query(pool, move |conn| {
            let sql = sql.clone();
            let cid = cid.clone();
            let bind_values = bind_values.clone();
            let cols = cols_clone.clone();
            Box::pin(async move {
                let mut q = sqlx::query(&sql).bind(&cid);
                for bv in &bind_values {
                    q = q.bind(bv);
                }
                let db_rows = q
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                let mut result_rows = Vec::with_capacity(db_rows.len());
                for r in db_rows {
                    let mut map = HashMap::new();
                    for col in &cols {
                        let val: String = r.try_get::<String, _>(col.as_str()).unwrap_or_default();
                        map.insert(col.clone(), val);
                    }
                    result_rows.push(map);
                }
                Ok(result_rows)
            })
        })
        .await?;

        // Compute exact rust_decimal totals for numeric columns
        let mut numeric_totals = HashMap::new();
        for col in &def.columns {
            if let Some(meta) = allowed.iter().find(|c| c.key == col && c.is_numeric) {
                let mut sum = Decimal::ZERO;
                for row in &rows_out {
                    if let Some(v) = row.get(meta.key) {
                        if let Ok(d) = Decimal::from_str(v.trim()) {
                            sum += d;
                        }
                    }
                }
                numeric_totals.insert(col.clone(), sum.round_dp(2).to_string());
            }
        }

        let row_count = rows_out.len();
        Ok(CustomReportExecutionResult {
            report_id: def.id,
            report_name: def.name,
            data_source: def.data_source,
            columns: def.columns,
            rows: rows_out,
            numeric_totals,
            row_count,
        })
    }

    pub async fn export_report_csv(
        pool: &Pool<Sqlite>,
        company_id: &str,
        report_id: &str,
    ) -> Result<String, OpenTallyError> {
        let res = Self::run_report(pool, company_id, report_id).await?;
        let mut out = String::new();
        let headers: Vec<String> = res
            .columns
            .iter()
            .map(|c| escape_rfc4180(&sanitize_text_cell(c)))
            .collect();
        out.push_str(&headers.join(","));
        out.push('\n');

        for row in &res.rows {
            let cells: Vec<String> = res
                .columns
                .iter()
                .map(|c| {
                    let raw = row.get(c).map(|s| s.as_str()).unwrap_or("");
                    escape_rfc4180(&sanitize_text_cell(raw))
                })
                .collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        Ok(out)
    }

    pub async fn export_template(
        pool: &Pool<Sqlite>,
        company_id: &str,
        report_id: &str,
    ) -> Result<String, OpenTallyError> {
        let all = Self::list_reports(pool, company_id).await?;
        let def = all.into_iter().find(|r| r.id == report_id).ok_or_else(|| {
            OpenTallyError::NotFound(format!("Custom report '{}' not found", report_id))
        })?;

        let tmpl = CustomReportTemplateFile {
            format: "opentally-custom-report".to_string(),
            version: "3.0.0".to_string(),
            name: def.name,
            description: def.description,
            data_source: def.data_source,
            columns: def.columns,
            filters: def.filters,
            sort_by: def.sort_by,
            sort_dir: def.sort_dir,
        };
        serde_json::to_string_pretty(&tmpl).map_err(|e| OpenTallyError::Internal(e.to_string()))
    }

    pub async fn import_template(
        pool: &Pool<Sqlite>,
        company_id: &str,
        template_json: &str,
        actor: &str,
    ) -> Result<CustomReportDefinitionDto, OpenTallyError> {
        let tmpl: CustomReportTemplateFile = serde_json::from_str(template_json).map_err(|e| {
            OpenTallyError::Validation(format!("Invalid .opentally-report.json template: {}", e))
        })?;
        if tmpl.format != "opentally-custom-report" {
            return Err(OpenTallyError::Validation(
                "Unsupported template format; expected 'opentally-custom-report'.".into(),
            ));
        }
        Self::save_report(
            pool,
            company_id,
            SaveCustomReportInput {
                id: None,
                name: tmpl.name,
                description: Some(tmpl.description),
                data_source: tmpl.data_source,
                columns: tmpl.columns,
                filters: Some(tmpl.filters),
                sort_by: tmpl.sort_by,
                sort_dir: Some(tmpl.sort_dir),
            },
            actor,
        )
        .await
    }
}
