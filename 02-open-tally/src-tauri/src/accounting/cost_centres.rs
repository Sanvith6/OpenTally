use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostCategory {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub allocate_revenue: bool,
    pub allocate_non_revenue: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCategoryRequest {
    pub name: String,
    #[serde(default = "default_true")]
    pub allocate_revenue: bool,
    #[serde(default)]
    pub allocate_non_revenue: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CostCentre {
    pub id: String,
    pub company_id: String,
    pub category_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCentreRequest {
    pub category_id: String,
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherCostAllocationInput {
    pub cost_centre_id: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherCostAllocationItem {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub voucher_line_id: String,
    pub cost_centre_id: String,
    pub cost_centre_name: String,
    pub category_name: String,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCentreBreakupItem {
    pub cost_centre_id: String,
    pub cost_centre_name: String,
    pub category_id: String,
    pub category_name: String,
    pub debit_total: String,
    pub credit_total: String,
    pub net_amount: String,
    pub allocations_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostCentreBreakupReport {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub centres: Vec<CostCentreBreakupItem>,
    pub total_debit: String,
    pub total_credit: String,
    pub net_total: String,
}

pub struct CostCentreService;

impl CostCentreService {
    /// List all cost categories for a company
    pub async fn list_cost_categories(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Vec<CostCategory>, OpenTallyError> {
        let rows = sqlx::query_as::<_, CostCategory>(
            "SELECT id, company_id, name, allocate_revenue, allocate_non_revenue, is_active, created_at, updated_at
             FROM cost_categories
             WHERE company_id = ?
             ORDER BY name",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }

    /// Create a new cost category
    pub async fn create_cost_category(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CostCategoryRequest,
    ) -> Result<CostCategory, OpenTallyError> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(OpenTallyError::Validation(
                "Cost category name cannot be empty".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO cost_categories (
                id, company_id, name, allocate_revenue, allocate_non_revenue, is_active
             ) VALUES (?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(name)
        .bind(req.allocate_revenue)
        .bind(req.allocate_non_revenue)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let row = sqlx::query_as::<_, CostCategory>(
            "SELECT id, company_id, name, allocate_revenue, allocate_non_revenue, is_active, created_at, updated_at
             FROM cost_categories WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(row)
    }

    /// List cost centres, optionally filtered by category
    pub async fn list_cost_centres(
        pool: &Pool<Sqlite>,
        company_id: &str,
        category_id: Option<&str>,
    ) -> Result<Vec<CostCentre>, OpenTallyError> {
        let rows = sqlx::query_as::<_, CostCentre>(
            "SELECT id, company_id, category_id, name, parent_id, is_active, created_at, updated_at
             FROM cost_centres
             WHERE company_id = ? AND (? IS NULL OR category_id = ?)
             ORDER BY name",
        )
        .bind(company_id)
        .bind(category_id)
        .bind(category_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }

    /// Create a new cost centre
    pub async fn create_cost_centre(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CostCentreRequest,
    ) -> Result<CostCentre, OpenTallyError> {
        let name = req.name.trim();
        if name.is_empty() {
            return Err(OpenTallyError::Validation(
                "Cost centre name cannot be empty".to_string(),
            ));
        }

        // Verify category exists
        let cat_exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM cost_categories WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(&req.category_id)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        if cat_exists.is_none() {
            return Err(OpenTallyError::Validation(
                "Referenced cost category does not exist".to_string(),
            ));
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO cost_centres (
                id, company_id, category_id, name, parent_id, is_active
             ) VALUES (?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(&req.category_id)
        .bind(name)
        .bind(&req.parent_id)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let row = sqlx::query_as::<_, CostCentre>(
            "SELECT id, company_id, category_id, name, parent_id, is_active, created_at, updated_at
             FROM cost_centres WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(row)
    }

    /// Save cost allocations for a specific voucher line, asserting SUM(amount) == line_amount
    pub async fn save_voucher_cost_allocations(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_line_id: &str,
        line_amount: Decimal,
        allocations: &[VoucherCostAllocationInput],
    ) -> Result<(), OpenTallyError> {
        if allocations.is_empty() {
            return Ok(());
        }

        let sum_allocated: Decimal = allocations.iter().map(|a| a.amount).sum();
        if sum_allocated != line_amount {
            return Err(OpenTallyError::Validation(format!(
                "Cost centre allocations sum ({}) does not balance with voucher line amount ({})",
                sum_allocated, line_amount
            )));
        }

        for a in allocations {
            if a.amount <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Cost centre allocation amount must be positive".to_string(),
                ));
            }

            // Verify cost centre belongs to this company
            let exists: Option<String> = sqlx::query_scalar(
                "SELECT id FROM cost_centres WHERE company_id = ? AND id = ? AND is_active = 1",
            )
            .bind(company_id)
            .bind(&a.cost_centre_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if exists.is_none() {
                return Err(OpenTallyError::Validation(format!(
                    "Cost centre '{}' not found or is inactive",
                    a.cost_centre_id
                )));
            }

            let alloc_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO voucher_cost_allocations (
                    id, company_id, voucher_id, voucher_line_id, cost_centre_id, amount
                 ) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&alloc_id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(voucher_line_id)
            .bind(&a.cost_centre_id)
            .bind(a.amount.normalize().to_string())
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    /// Get all cost allocations for a voucher
    pub async fn get_voucher_cost_allocations(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<Vec<VoucherCostAllocationItem>, OpenTallyError> {
        #[derive(FromRow)]
        struct RawAlloc {
            id: String,
            company_id: String,
            voucher_id: String,
            voucher_line_id: String,
            cost_centre_id: String,
            cost_centre_name: String,
            category_name: String,
            amount: String,
        }

        let rows = sqlx::query_as::<_, RawAlloc>(
            "SELECT 
                vca.id, vca.company_id, vca.voucher_id, vca.voucher_line_id,
                vca.cost_centre_id, cc.name as cost_centre_name, cat.name as category_name,
                vca.amount
             FROM voucher_cost_allocations vca
             JOIN cost_centres cc ON cc.company_id = vca.company_id AND cc.id = vca.cost_centre_id
             JOIN cost_categories cat ON cat.company_id = cc.company_id AND cat.id = cc.category_id
             WHERE vca.company_id = ? AND vca.voucher_id = ?
             ORDER BY cat.name, cc.name",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let res = rows
            .into_iter()
            .map(|r| VoucherCostAllocationItem {
                id: r.id,
                company_id: r.company_id,
                voucher_id: r.voucher_id,
                voucher_line_id: r.voucher_line_id,
                cost_centre_id: r.cost_centre_id,
                cost_centre_name: r.cost_centre_name,
                category_name: r.category_name,
                amount: r.amount,
            })
            .collect();

        Ok(res)
    }

    /// Generate Cost Centre Breakup Report with Dr/Cr totals and net impact
    pub async fn get_cost_centre_breakup_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: Option<&str>,
        to_date: Option<&str>,
        category_id_filter: Option<&str>,
        cost_centre_id_filter: Option<&str>,
    ) -> Result<CostCentreBreakupReport, OpenTallyError> {
        let cid = company_id.to_string();
        let from_owned = from_date.map(ToString::to_string);
        let to_owned = to_date.map(ToString::to_string);
        let cat_owned = category_id_filter.map(ToString::to_string);
        let cc_owned = cost_centre_id_filter.map(ToString::to_string);

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                #[derive(FromRow)]
                struct RawBreakup {
                    cost_centre_id: String,
                    cost_centre_name: String,
                    category_id: String,
                    category_name: String,
                    entry_type: String,
                    amount: String,
                }

                let rows = sqlx::query_as::<_, RawBreakup>(
                    r#"
                    SELECT 
                        cc.id as cost_centre_id,
                        cc.name as cost_centre_name,
                        cat.id as category_id,
                        cat.name as category_name,
                        vl.entry_type,
                        vca.amount
                    FROM voucher_cost_allocations vca
                    JOIN cost_centres cc ON cc.company_id = vca.company_id AND cc.id = vca.cost_centre_id
                    JOIN cost_categories cat ON cat.company_id = cc.company_id AND cat.id = cc.category_id
                    JOIN vouchers v ON v.company_id = vca.company_id AND v.id = vca.voucher_id
                    JOIN voucher_lines vl ON vl.company_id = vca.company_id AND vl.id = vca.voucher_line_id
                    WHERE vca.company_id = ?
                      AND v.status = 'posted'
                      AND (? IS NULL OR v.date >= ?)
                      AND (? IS NULL OR v.date <= ?)
                      AND (? IS NULL OR cat.id = ?)
                      AND (? IS NULL OR cc.id = ?)
                    ORDER BY cat.name, cc.name
                    "#,
                )
                .bind(&cid)
                .bind(&from_owned)
                .bind(&from_owned)
                .bind(&to_owned)
                .bind(&to_owned)
                .bind(&cat_owned)
                .bind(&cat_owned)
                .bind(&cc_owned)
                .bind(&cc_owned)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                use std::collections::BTreeMap;
                struct Agg {
                    name: String,
                    cat_id: String,
                    cat_name: String,
                    debit: Decimal,
                    credit: Decimal,
                    count: usize,
                }

                let mut map: BTreeMap<String, Agg> = BTreeMap::new();
                let mut overall_dr = Decimal::ZERO;
                let mut overall_cr = Decimal::ZERO;

                for r in rows {
                    let amt = Decimal::from_str(&r.amount).unwrap_or(Decimal::ZERO);
                    let entry = map.entry(r.cost_centre_id.clone()).or_insert_with(|| Agg {
                        name: r.cost_centre_name,
                        cat_id: r.category_id,
                        cat_name: r.category_name,
                        debit: Decimal::ZERO,
                        credit: Decimal::ZERO,
                        count: 0,
                    });

                    entry.count += 1;
                    if r.entry_type == "Dr" {
                        entry.debit += amt;
                        overall_dr += amt;
                    } else {
                        entry.credit += amt;
                        overall_cr += amt;
                    }
                }

                let centres = map
                    .into_iter()
                    .map(|(cid, agg)| {
                        let net = agg.debit - agg.credit;
                        CostCentreBreakupItem {
                            cost_centre_id: cid,
                            cost_centre_name: agg.name,
                            category_id: agg.cat_id,
                            category_name: agg.cat_name,
                            debit_total: agg.debit.round_dp(2).normalize().to_string(),
                            credit_total: agg.credit.round_dp(2).normalize().to_string(),
                            net_amount: net.round_dp(2).normalize().to_string(),
                            allocations_count: agg.count,
                        }
                    })
                    .collect();

                let net_overall = overall_dr - overall_cr;

                Ok(CostCentreBreakupReport {
                    from_date: from_owned,
                    to_date: to_owned,
                    centres,
                    total_debit: overall_dr.round_dp(2).normalize().to_string(),
                    total_credit: overall_cr.round_dp(2).normalize().to_string(),
                    net_total: net_overall.round_dp(2).normalize().to_string(),
                })
            })
        })
        .await
    }
}
