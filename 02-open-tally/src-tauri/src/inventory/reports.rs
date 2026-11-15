use crate::errors::OpenTallyError;
use crate::inventory::{
    StockBalanceRequest, StockBalanceService, ValuationRequest, ValuationService,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryReportFilter {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub as_of_date: Option<String>,
    pub stock_item_id: Option<String>,
    pub stock_group_id: Option<String>,
    pub stock_category_id: Option<String>,
    pub godown_id: Option<String>,
    pub voucher_type: Option<String>,
    pub search: Option<String>,
    pub include_zero: bool,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}
impl InventoryReportFilter {
    fn page(&self) -> Result<(i64, i64), OpenTallyError> {
        let page = self.page.unwrap_or(1);
        let size = self.page_size.unwrap_or(100);
        if page < 1 || !(1..=500).contains(&size) {
            return Err(OpenTallyError::Validation(
                "Page must be positive and page size must be between 1 and 500".into(),
            ));
        }
        Ok(((page - 1) * size, size))
    }
    fn date(&self) -> String {
        self.as_of_date
            .clone()
            .or_else(|| self.date_to.clone())
            .or_else(|| self.date_from.clone())
            .unwrap_or_else(|| "9999-12-31".into())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSummaryReportRow {
    pub stock_item_id: String,
    pub item_name: String,
    pub stock_group_name: String,
    pub base_unit_symbol: String,
    pub opening_quantity: String,
    pub received_quantity: String,
    pub delivered_quantity: String,
    pub transfer_in_quantity: String,
    pub transfer_out_quantity: String,
    pub adjustment_quantity: String,
    pub closing_quantity: String,
    pub valuation_method: String,
    pub unit_cost: String,
    pub total_stock_value: String,
    pub warning: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockSummaryReport {
    pub as_of_date: String,
    pub rows: Vec<StockSummaryReportRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct StockRegisterRow {
    pub movement_id: String,
    pub movement_date: String,
    pub voucher_number: String,
    pub voucher_type: String,
    pub stock_item_id: String,
    pub item_name: String,
    pub godown_id: String,
    pub godown_name: String,
    pub direction: String,
    pub quantity: String,
    pub base_unit_symbol: String,
    pub source_reference: Option<String>,
    pub voucher_status: String,
    pub reversal_of_voucher_id: Option<String>,
    pub signed_quantity: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockRegisterReport {
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub rows: Vec<StockRegisterRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockLedgerRow {
    pub movement_id: String,
    pub movement_date: String,
    pub voucher_number: String,
    pub movement_type: String,
    pub signed_quantity: String,
    pub running_balance: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockLedgerReport {
    pub stock_item_id: String,
    pub godown_id: Option<String>,
    pub opening_quantity: String,
    pub closing_quantity: String,
    pub rows: Vec<StockLedgerRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationFinding {
    pub check_id: String,
    pub company_id: String,
    pub stock_item_id: Option<String>,
    pub godown_id: Option<String>,
    pub expected_value: String,
    pub actual_value: String,
    pub difference: String,
    pub severity: String,
    pub explanation: String,
}

fn decimal(value: &str) -> Result<Decimal, OpenTallyError> {
    value
        .parse()
        .map_err(|_| OpenTallyError::IntegrityViolation("Invalid stored inventory quantity".into()))
}
fn valid_date(value: &str) -> Result<(), OpenTallyError> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|_| ())
        .map_err(|_| {
            OpenTallyError::Validation(format!("Invalid date '{}'; expected YYYY-MM-DD", value))
        })
}
fn previous_date(value: &str) -> Result<String, OpenTallyError> {
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| OpenTallyError::Validation("Invalid date".into()))?;
    Ok((date - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string())
}

pub struct InventoryReportsService;
impl InventoryReportsService {
    pub async fn stock_summary(
        pool: &SqlitePool,
        company: &str,
        filter: InventoryReportFilter,
    ) -> Result<StockSummaryReport, OpenTallyError> {
        let as_of = filter.date();
        valid_date(&as_of)?;
        let balances = StockBalanceService::query(
            pool,
            company,
            StockBalanceRequest {
                as_of_date: as_of.clone(),
                stock_item_id: filter.stock_item_id.clone(),
                stock_group_id: filter.stock_group_id.clone(),
                stock_category_id: filter.stock_category_id.clone(),
                godown_id: filter.godown_id.clone(),
                search: filter.search.clone(),
                include_zero: filter.include_zero,
            },
            true,
        )
        .await?;
        let values = ValuationService::summary(
            pool,
            company,
            ValuationRequest {
                as_of_date: as_of.clone(),
                stock_item_id: filter.stock_item_id,
                stock_group_id: filter.stock_group_id,
                stock_category_id: filter.stock_category_id,
                godown_id: filter.godown_id,
                search: filter.search,
                include_zero: filter.include_zero,
            },
        )
        .await?;
        let mut rows = Vec::new();
        for balance in balances {
            let value = values
                .iter()
                .find(|x| x.stock_item_id == balance.stock_item_id);
            let (method, cost, total) = value
                .map(|x| {
                    (
                        x.valuation_method.clone(),
                        x.unit_cost.clone(),
                        x.total_value.clone(),
                    )
                })
                .unwrap_or(("unavailable".into(), "0".into(), "0".into()));
            let warning = if decimal(&balance.closing_quantity)? != Decimal::ZERO
                && decimal(&cost)? == Decimal::ZERO
            {
                Some("Stock includes zero-cost or legacy unvalued receipts".into())
            } else {
                None
            };
            rows.push(StockSummaryReportRow {
                stock_item_id: balance.stock_item_id,
                item_name: balance.item_name,
                stock_group_name: balance.stock_group_name,
                base_unit_symbol: balance.base_unit_symbol,
                opening_quantity: balance.opening_quantity,
                received_quantity: balance.received_quantity,
                delivered_quantity: balance.delivered_quantity,
                transfer_in_quantity: balance.transfer_in_quantity,
                transfer_out_quantity: balance.transfer_out_quantity,
                adjustment_quantity: balance.adjustment_quantity,
                closing_quantity: balance.closing_quantity,
                valuation_method: method,
                unit_cost: cost,
                total_stock_value: total,
                warning,
            });
        }
        Ok(StockSummaryReport {
            as_of_date: as_of,
            rows,
        })
    }
    async fn register_query(
        pool: &SqlitePool,
        company: &str,
        filter: &InventoryReportFilter,
        count: bool,
    ) -> Result<Vec<StockRegisterRow>, OpenTallyError> {
        let search = filter.search.as_ref().map(|x| format!("%{}%", x));
        let sql = if count {
            "SELECT COUNT(*) as movement_id FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id JOIN stock_items i ON i.company_id=m.company_id AND i.id=m.stock_item_id JOIN units u ON u.company_id=m.company_id AND u.id=l.unit_id JOIN godowns g ON g.company_id=m.company_id AND g.id=m.godown_id WHERE m.company_id=? AND (? IS NULL OR m.movement_date>=?) AND (? IS NULL OR m.movement_date<=?) AND (? IS NULL OR m.stock_item_id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?) AND (? IS NULL OR m.godown_id=?) AND (? IS NULL OR v.voucher_type=?) AND (? IS NULL OR i.name LIKE ? COLLATE NOCASE OR i.sku LIKE ? COLLATE NOCASE OR v.voucher_number LIKE ? COLLATE NOCASE OR v.reference_no LIKE ? COLLATE NOCASE)"
        } else {
            "SELECT m.id movement_id,m.movement_date,v.voucher_number,v.voucher_type,m.stock_item_id,i.name item_name,m.godown_id,g.name godown_name,CASE WHEN substr(m.signed_quantity,1,1)='-' THEN 'out' ELSE 'in' END direction,CASE WHEN substr(m.signed_quantity,1,1)='-' THEN substr(m.signed_quantity,2) ELSE m.signed_quantity END quantity,u.symbol base_unit_symbol,COALESCE(v.reference_no,v.narration) source_reference,v.status voucher_status,v.reversal_of_voucher_id,m.signed_quantity FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id JOIN stock_items i ON i.company_id=m.company_id AND i.id=m.stock_item_id JOIN units u ON u.company_id=m.company_id AND u.id=l.unit_id JOIN godowns g ON g.company_id=m.company_id AND g.id=m.godown_id WHERE m.company_id=? AND (? IS NULL OR m.movement_date>=?) AND (? IS NULL OR m.movement_date<=?) AND (? IS NULL OR m.stock_item_id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?) AND (? IS NULL OR m.godown_id=?) AND (? IS NULL OR v.voucher_type=?) AND (? IS NULL OR i.name LIKE ? COLLATE NOCASE OR i.sku LIKE ? COLLATE NOCASE OR v.voucher_number LIKE ? COLLATE NOCASE OR v.reference_no LIKE ? COLLATE NOCASE) ORDER BY m.movement_date,m.id LIMIT ? OFFSET ?"
        };
        let mut q = sqlx::query_as::<_, StockRegisterRow>(sql)
            .bind(company)
            .bind(&filter.date_from)
            .bind(&filter.date_from)
            .bind(&filter.date_to)
            .bind(&filter.date_to)
            .bind(&filter.stock_item_id)
            .bind(&filter.stock_item_id)
            .bind(&filter.stock_group_id)
            .bind(&filter.stock_group_id)
            .bind(&filter.stock_category_id)
            .bind(&filter.stock_category_id)
            .bind(&filter.godown_id)
            .bind(&filter.godown_id)
            .bind(&filter.voucher_type)
            .bind(&filter.voucher_type)
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref())
            .bind(search.as_deref());
        if !count {
            let (offset, size) = filter.page()?;
            q = q.bind(size).bind(offset);
        }
        Ok(q.fetch_all(pool).await?)
    }
    pub async fn stock_register(
        pool: &SqlitePool,
        company: &str,
        filter: InventoryReportFilter,
    ) -> Result<StockRegisterReport, OpenTallyError> {
        let (offset, size) = filter.page()?;
        let total:i64=sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id JOIN stock_items i ON i.company_id=m.company_id AND i.id=m.stock_item_id WHERE m.company_id=? AND (? IS NULL OR m.movement_date>=?) AND (? IS NULL OR m.movement_date<=?) AND (? IS NULL OR m.stock_item_id=?) AND (? IS NULL OR i.stock_group_id=?) AND (? IS NULL OR i.stock_category_id=?) AND (? IS NULL OR m.godown_id=?) AND (? IS NULL OR v.voucher_type=?)").bind(company).bind(&filter.date_from).bind(&filter.date_from).bind(&filter.date_to).bind(&filter.date_to).bind(&filter.stock_item_id).bind(&filter.stock_item_id).bind(&filter.stock_group_id).bind(&filter.stock_group_id).bind(&filter.stock_category_id).bind(&filter.stock_category_id).bind(&filter.godown_id).bind(&filter.godown_id).bind(&filter.voucher_type).bind(&filter.voucher_type).fetch_one(pool).await?;
        let rows = Self::register_query(pool, company, &filter, false).await?;
        Ok(StockRegisterReport {
            total,
            page: offset / size + 1,
            page_size: size,
            rows,
        })
    }
    pub async fn stock_ledger(
        pool: &SqlitePool,
        company: &str,
        filter: InventoryReportFilter,
    ) -> Result<StockLedgerReport, OpenTallyError> {
        let item = filter.stock_item_id.clone().ok_or_else(|| {
            OpenTallyError::Validation("Stock item is required for the stock ledger".into())
        })?;
        let start = filter.date_from.clone().unwrap_or("0001-01-01".into());
        let opening_date = if start == "0001-01-01" {
            "0001-01-01".into()
        } else {
            previous_date(&start)?
        };
        let opening = StockBalanceService::query(
            pool,
            company,
            StockBalanceRequest {
                as_of_date: opening_date,
                stock_item_id: Some(item.clone()),
                stock_group_id: None,
                stock_category_id: None,
                godown_id: filter.godown_id.clone(),
                search: None,
                include_zero: true,
            },
            filter.godown_id.is_none(),
        )
        .await?
        .into_iter()
        .map(|x| decimal(&x.closing_quantity))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<Decimal>();
        let register = Self::stock_register(
            pool,
            company,
            InventoryReportFilter {
                date_from: filter.date_from.clone(),
                date_to: filter.date_to.clone(),
                stock_item_id: Some(item.clone()),
                godown_id: filter.godown_id.clone(),
                ..filter
            },
        )
        .await?;
        let mut running = opening;
        let mut rows = Vec::new();
        for movement in register
            .rows
            .into_iter()
            .filter(|movement| movement.voucher_status != "cancelled")
        {
            let q = decimal(&movement.signed_quantity)?;
            running += q;
            rows.push(StockLedgerRow {
                movement_id: movement.movement_id,
                movement_date: movement.movement_date,
                voucher_number: movement.voucher_number,
                movement_type: movement.voucher_type,
                signed_quantity: movement.signed_quantity,
                running_balance: running.normalize().to_string(),
            });
        }
        Ok(StockLedgerReport {
            stock_item_id: item,
            godown_id: None,
            opening_quantity: opening.normalize().to_string(),
            closing_quantity: running.normalize().to_string(),
            rows,
        })
    }
    pub async fn reconciliation(
        pool: &SqlitePool,
        company: &str,
        filter: InventoryReportFilter,
    ) -> Result<Vec<ReconciliationFinding>, OpenTallyError> {
        let mut out = Vec::new();
        let attribution_tables: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='inventory_valuation_attributions'")
            .fetch_one(pool).await?;
        if attribution_tables == 1 {
            let missing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements m JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id LEFT JOIN inventory_valuation_attributions a ON a.company_id=m.company_id AND a.stock_movement_id=m.id WHERE m.company_id=? AND v.status='posted' AND v.voucher_type<>'transfer' AND a.id IS NULL")
                .bind(company).fetch_one(pool).await?;
            if missing > 0 {
                out.push(ReconciliationFinding {
                    check_id: "missing-valuation-attribution".into(),
                    company_id: company.into(),
                    stock_item_id: None,
                    godown_id: None,
                    expected_value: "0".into(),
                    actual_value: missing.to_string(),
                    difference: missing.to_string(),
                    severity: "critical".into(),
                    explanation:
                        "Posted inventory movements are missing immutable valuation attribution"
                            .into(),
                });
            }
            let mismatches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory_valuation_attributions a JOIN inventory_accounting_integrations i ON i.company_id=a.company_id AND i.stock_voucher_id=a.stock_voucher_id WHERE a.company_id=? AND i.status='integrated' GROUP BY a.stock_voucher_id HAVING SUM(CAST(a.total_value AS DECIMAL)) <> CAST(i.integrated_amount AS DECIMAL)").bind(company).fetch_optional(pool).await?.unwrap_or(0);
            if mismatches > 0 {
                out.push(ReconciliationFinding { check_id: "valuation-accounting-mismatch".into(), company_id: company.into(), stock_item_id: None, godown_id: None, expected_value: "0".into(), actual_value: mismatches.to_string(), difference: mismatches.to_string(), severity: "critical".into(), explanation: "Persisted valuation attribution does not equal integrated accounting amount".into() });
            }
            let duplicate_accounting: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM (SELECT accounting_voucher_id FROM inventory_accounting_integrations WHERE company_id=? AND accounting_voucher_id IS NOT NULL GROUP BY accounting_voucher_id HAVING COUNT(*) > 1)")
                .bind(company).fetch_one(pool).await?;
            if duplicate_accounting > 0 {
                out.push(ReconciliationFinding { check_id: "duplicate-accounting-voucher".into(), company_id: company.into(), stock_item_id: None, godown_id: None, expected_value: "1 per attribution".into(), actual_value: duplicate_accounting.to_string(), difference: duplicate_accounting.to_string(), severity: "critical".into(), explanation: "One accounting voucher is linked to multiple inventory integrations".into() });
            }
            let integrations: Vec<(String, Option<String>, String, String)> = sqlx::query_as("SELECT stock_voucher_id,accounting_voucher_id,status,integrated_amount FROM inventory_accounting_integrations WHERE company_id=? ORDER BY stock_voucher_id")
                .bind(company).fetch_all(pool).await?;
            for (stock_voucher_id, accounting_voucher_id, status, integrated_amount) in integrations
            {
                if status == "pending" || status == "failed" {
                    out.push(ReconciliationFinding {
                        check_id: format!("integration-{}", status),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "integrated".into(),
                        actual_value: status.clone(),
                        difference: "pending".into(),
                        severity: if status == "failed" {
                            "critical"
                        } else {
                            "warning"
                        }
                        .into(),
                        explanation: format!(
                            "Inventory voucher {} has an {} accounting integration",
                            stock_voucher_id, status
                        ),
                    });
                }
                if status != "integrated" {
                    continue;
                }
                let Some(accounting_id) = accounting_voucher_id else {
                    out.push(ReconciliationFinding {
                        check_id: "missing-accounting-voucher".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "accounting voucher".into(),
                        actual_value: "missing".into(),
                        difference: integrated_amount,
                        severity: "critical".into(),
                        explanation: format!(
                            "Integrated inventory voucher {} has no accounting voucher",
                            stock_voucher_id
                        ),
                    });
                    continue;
                };
                let reference_no: Option<String> = sqlx::query_scalar(
                    "SELECT reference_no FROM vouchers WHERE company_id=? AND id=?",
                )
                .bind(company)
                .bind(&accounting_id)
                .fetch_optional(pool)
                .await?;
                if let Some(reference_no) = reference_no.as_deref() {
                    let duplicate_ids: Vec<String> = sqlx::query_scalar(
                        "SELECT id FROM vouchers WHERE company_id=? AND reference_no=? AND id<>? AND status IN ('posted','cancelled','reversed') ORDER BY id",
                    )
                    .bind(company)
                    .bind(reference_no)
                    .bind(&accounting_id)
                    .fetch_all(pool)
                    .await?;
                    if !duplicate_ids.is_empty() {
                        let attribution_id: Option<String> = sqlx::query_scalar(
                            "SELECT id FROM inventory_valuation_attributions WHERE company_id=? AND stock_voucher_id=? ORDER BY id LIMIT 1",
                        )
                        .bind(company)
                        .bind(&stock_voucher_id)
                        .fetch_optional(pool)
                        .await?;
                        let stock_item_id: Option<String> = sqlx::query_scalar(
                            "SELECT stock_item_id FROM inventory_valuation_attributions WHERE company_id=? AND stock_voucher_id=? ORDER BY id LIMIT 1",
                        )
                        .bind(company)
                        .bind(&stock_voucher_id)
                        .fetch_optional(pool)
                        .await?;
                        let mut voucher_ids = vec![accounting_id.clone()];
                        voucher_ids.extend(duplicate_ids);
                        out.push(ReconciliationFinding {
                            check_id: "duplicate-accounting-voucher".into(),
                            company_id: company.into(),
                            stock_item_id,
                            godown_id: None,
                            expected_value: "1 accounting voucher per attribution".into(),
                            actual_value: voucher_ids.join(","),
                            difference: (voucher_ids.len() as i64 - 1).to_string(),
                            severity: "critical".into(),
                            explanation: format!(
                                "Accounting vouchers {} independently reference stock voucher {} and attribution {}",
                                voucher_ids.join(", "),
                                stock_voucher_id,
                                attribution_id.as_deref().unwrap_or("missing")
                            ),
                        });
                    }
                }
                let lines: Vec<(String, String)> = sqlx::query_as("SELECT entry_type,amount FROM voucher_lines WHERE company_id=? AND voucher_id=? ORDER BY line_number")
                    .bind(company).bind(&accounting_id).fetch_all(pool).await?;
                if lines.is_empty() {
                    out.push(ReconciliationFinding {
                        check_id: "missing-accounting-lines".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "balanced lines".into(),
                        actual_value: "0 lines".into(),
                        difference: integrated_amount.clone(),
                        severity: "critical".into(),
                        explanation: format!("Accounting voucher {} has no lines", accounting_id),
                    });
                    continue;
                }
                let mut debit = Decimal::ZERO;
                let mut credit = Decimal::ZERO;
                let mut invalid = Vec::new();
                let mut zero_lines = 0_i64;
                for (entry_type, amount) in lines {
                    let value = match decimal(&amount) {
                        Ok(value) => value,
                        Err(_) => {
                            invalid.push(format!("{entry_type}:{amount}"));
                            continue;
                        }
                    };
                    if value.is_zero() {
                        zero_lines += 1;
                    }
                    match entry_type.as_str() {
                        "Dr" => debit += value,
                        "Cr" => credit += value,
                        _ => invalid.push(format!("{entry_type}:{amount}")),
                    }
                }
                if !invalid.is_empty() {
                    out.push(ReconciliationFinding {
                        check_id: "accounting-invalid-entry".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "valid Dr/Cr decimal lines".into(),
                        actual_value: invalid.join(","),
                        difference: invalid.len().to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} contains invalid accounting lines",
                            accounting_id
                        ),
                    });
                }
                if zero_lines > 0 {
                    out.push(ReconciliationFinding {
                        check_id: "accounting-zero-value".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "non-zero accounting lines".into(),
                        actual_value: zero_lines.to_string(),
                        difference: zero_lines.to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} contains zero-value lines",
                            accounting_id
                        ),
                    });
                }
                if debit.is_zero() {
                    out.push(ReconciliationFinding {
                        check_id: "missing-debit-line".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "debit amount".into(),
                        actual_value: debit.normalize().to_string(),
                        difference: (debit - credit).normalize().to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} has no debit amount",
                            accounting_id
                        ),
                    });
                }
                if credit.is_zero() {
                    out.push(ReconciliationFinding {
                        check_id: "missing-credit-line".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: "credit amount".into(),
                        actual_value: credit.normalize().to_string(),
                        difference: (debit - credit).normalize().to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} has no credit amount",
                            accounting_id
                        ),
                    });
                }
                if debit != credit {
                    out.push(ReconciliationFinding {
                        check_id: "accounting-imbalance".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: debit.normalize().to_string(),
                        actual_value: credit.normalize().to_string(),
                        difference: (debit - credit).normalize().to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} ({}) is not balanced",
                            accounting_id,
                            reference_no.as_deref().unwrap_or("no reference")
                        ),
                    });
                }
                let expected = decimal(&integrated_amount)?;
                if debit != expected {
                    out.push(ReconciliationFinding {
                        check_id: "accounting-valuation-mismatch".into(),
                        company_id: company.into(),
                        stock_item_id: None,
                        godown_id: None,
                        expected_value: expected.normalize().to_string(),
                        actual_value: debit.normalize().to_string(),
                        difference: (debit - expected).normalize().to_string(),
                        severity: "critical".into(),
                        explanation: format!(
                            "Accounting voucher {} does not equal persisted inventory valuation",
                            accounting_id
                        ),
                    });
                }
            }
        }
        let orphan:i64=sqlx::query_scalar("SELECT COUNT(*) FROM stock_movements m LEFT JOIN stock_vouchers v ON v.company_id=m.company_id AND v.id=m.voucher_id LEFT JOIN stock_voucher_lines l ON l.company_id=m.company_id AND l.id=m.voucher_line_id WHERE m.company_id=? AND (v.id IS NULL OR l.id IS NULL)").bind(company).fetch_one(pool).await?;
        if orphan > 0 {
            out.push(ReconciliationFinding {
                check_id: "orphan-references".into(),
                company_id: company.into(),
                stock_item_id: None,
                godown_id: None,
                expected_value: "0".into(),
                actual_value: orphan.to_string(),
                difference: orphan.to_string(),
                severity: "critical".into(),
                explanation: "Stock movements reference missing voucher or voucher lines".into(),
            });
        }
        let duplicate:i64=sqlx::query_scalar("SELECT COALESCE(SUM(n-1),0) FROM (SELECT COUNT(*) n FROM stock_movements WHERE company_id=? GROUP BY voucher_line_id,godown_id,movement_type HAVING n>1)").bind(company).fetch_one(pool).await?;
        if duplicate > 0 {
            out.push(ReconciliationFinding {
                check_id: "duplicate-movements".into(),
                company_id: company.into(),
                stock_item_id: None,
                godown_id: None,
                expected_value: "0".into(),
                actual_value: duplicate.to_string(),
                difference: duplicate.to_string(),
                severity: "critical".into(),
                explanation:
                    "More than one movement exists for a voucher line, godown, and movement type"
                        .into(),
            });
        }
        let summary = Self::stock_summary(pool, company, filter.clone()).await?;
        for row in summary.rows {
            if decimal(&row.closing_quantity)? < Decimal::ZERO {
                out.push(ReconciliationFinding {
                    check_id: "negative-quantity".into(),
                    company_id: company.into(),
                    stock_item_id: Some(row.stock_item_id.clone()),
                    godown_id: None,
                    expected_value: ">=0".into(),
                    actual_value: row.closing_quantity.clone(),
                    difference: row.closing_quantity,
                    severity: "critical".into(),
                    explanation: "Movement-derived closing stock is negative".into(),
                });
            }
            if let Some(warning) = row.warning {
                out.push(ReconciliationFinding {
                    check_id: "missing-valuation-data".into(),
                    company_id: company.into(),
                    stock_item_id: Some(row.stock_item_id.clone()),
                    godown_id: None,
                    expected_value: "valued stock".into(),
                    actual_value: row.total_stock_value,
                    difference: "unavailable".into(),
                    severity: "warning".into(),
                    explanation: warning,
                });
            }
        }
        Ok(out)
    }
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
    async fn setup() -> DatabaseManager {
        let db = DatabaseManager::init(Path::new(":memory:")).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('rc','Reports Co','Reports Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('g','rc','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','rc','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('i','rc','Item','g','u','2','10','20','0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('d','rc','Depot')")
            .execute(p)
            .await
            .unwrap();
        db
    }
    #[tokio::test]
    async fn reports_are_filtered_paginated_and_reconciled() {
        let db = setup().await;
        let v = StockVoucherService::create_draft(
            db.pool(),
            "rc",
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: Some("R1".into()),
                narration: None,
                lines: vec![StockVoucherLineRequest {
                    stock_item_id: "i".into(),
                    unit_id: "u".into(),
                    godown_id: Some("d".into()),
                    source_godown_id: None,
                    destination_godown_id: None,
                    quantity: "3.25".into(),
                    unit_cost: Some("12.5".into()),
                    physical_quantity: None,
                    description: None,
                }],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "rc", &v.id)
            .await
            .unwrap();
        let f = InventoryReportFilter {
            date_from: None,
            date_to: Some("2026-04-02".into()),
            as_of_date: Some("2026-04-02".into()),
            stock_item_id: None,
            stock_group_id: None,
            stock_category_id: None,
            godown_id: None,
            voucher_type: None,
            search: None,
            include_zero: true,
            page: Some(1),
            page_size: Some(1),
        };
        let summary = InventoryReportsService::stock_summary(db.pool(), "rc", f.clone())
            .await
            .unwrap();
        assert_eq!(summary.rows[0].closing_quantity, "5.25");
        let register = InventoryReportsService::stock_register(db.pool(), "rc", f.clone())
            .await
            .unwrap();
        assert_eq!(register.total, 1);
        assert_eq!(register.rows[0].quantity, "3.25");
        let findings = InventoryReportsService::reconciliation(db.pool(), "rc", f)
            .await
            .unwrap();
        assert!(findings.is_empty());
    }
}
