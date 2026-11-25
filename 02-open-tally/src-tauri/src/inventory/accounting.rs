use crate::errors::OpenTallyError;
use crate::inventory::{StockVoucher, StockVoucherLine, ValuationRequest, ValuationService};
use crate::vouchers::{
    CreateVoucherDraftRequest, ReverseVoucherRequest, VoucherDraftService, VoucherLineInput,
    VoucherPostingService, VoucherReversalService,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InventoryAccountMapping {
    pub company_id: String,
    pub inventory_asset_ledger_id: String,
    pub adjustment_expense_ledger_id: String,
    pub adjustment_gain_ledger_id: Option<String>,
    pub cogs_ledger_id: String,
    pub receipt_offset_ledger_id: String,
    pub journal_series_id: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAccountMappingRequest {
    pub inventory_asset_ledger_id: String,
    pub adjustment_expense_ledger_id: String,
    pub adjustment_gain_ledger_id: Option<String>,
    pub cogs_ledger_id: String,
    pub receipt_offset_ledger_id: String,
    pub journal_series_id: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InventoryAccountingIntegration {
    pub id: String,
    pub company_id: String,
    pub stock_voucher_id: String,
    pub stock_voucher_type: String,
    pub accounting_voucher_id: Option<String>,
    pub status: String,
    pub valuation_method: Option<String>,
    pub integrated_amount: String,
    pub attempt_count: i64,
    pub error_details: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingImpactLine {
    pub ledger_id: String,
    pub entry_type: String,
    pub amount: String,
    pub narration: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryAccountingPreview {
    pub stock_voucher: StockVoucher,
    pub valuation_method: String,
    pub amount: String,
    pub lines: Vec<AccountingImpactLine>,
    pub total_debit: String,
    pub total_credit: String,
    pub warning: Option<String>,
}

static INTEGRATION_LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();

fn integration_lock(company: &str, stock_voucher_id: &str) -> Arc<AsyncMutex<()>> {
    let locks = INTEGRATION_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks
        .entry(format!("{company}:{stock_voucher_id}"))
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn dec(v: &str) -> Result<Decimal, OpenTallyError> {
    v.parse()
        .map_err(|_| OpenTallyError::IntegrityViolation("Invalid valuation amount".into()))
}
fn fmt(v: Decimal) -> String {
    v.normalize().to_string()
}

pub struct InventoryAccountingService;

#[cfg(test)]
static TEST_INTEGRATION_FAILURE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(test)]
static TEST_INTEGRATION_FAILURE_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

#[cfg(test)]
pub fn set_test_integration_failure(mode: u8) {
    TEST_INTEGRATION_FAILURE.store(mode, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
pub fn test_integration_failure_lock() -> &'static AsyncMutex<()> {
    TEST_INTEGRATION_FAILURE_LOCK.get_or_init(|| AsyncMutex::new(()))
}

impl InventoryAccountingService {
    pub async fn get_mapping(
        pool: &SqlitePool,
        company: &str,
    ) -> Result<Option<InventoryAccountMapping>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT company_id,inventory_asset_ledger_id,adjustment_expense_ledger_id,adjustment_gain_ledger_id,cogs_ledger_id,receipt_offset_ledger_id,journal_series_id,enabled FROM inventory_account_mappings WHERE company_id=?").bind(company).fetch_optional(pool).await?)
    }
    pub async fn get_mapping_tx(
        conn: &mut sqlx::SqliteConnection,
        company: &str,
    ) -> Result<Option<InventoryAccountMapping>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT company_id,inventory_asset_ledger_id,adjustment_expense_ledger_id,adjustment_gain_ledger_id,cogs_ledger_id,receipt_offset_ledger_id,journal_series_id,enabled FROM inventory_account_mappings WHERE company_id=?").bind(company).fetch_optional(conn).await?)
    }
    async fn validate_ledger(
        pool: &SqlitePool,
        company: &str,
        id: &str,
        allowed: &[&str],
    ) -> Result<(), OpenTallyError> {
        let row:Option<(String,String,i64)>=sqlx::query_as("SELECT l.id,g.nature,l.is_reserved FROM ledgers l JOIN account_groups g ON g.company_id=l.company_id AND g.id=l.group_id WHERE l.company_id=? AND l.id=?").bind(company).bind(id).fetch_optional(pool).await?;
        match row {
            Some((_, nature, reserved)) if allowed.contains(&nature.as_str()) && reserved == 0 => {
                Ok(())
            }
            Some((_, nature, _)) => Err(OpenTallyError::Validation(format!(
                "Ledger '{}' has incompatible or reserved account classification '{}'",
                id, nature
            ))),
            None => Err(OpenTallyError::Validation(format!(
                "Ledger '{}' is not in the active company",
                id
            ))),
        }
    }
    pub async fn validate_mapping(
        pool: &SqlitePool,
        company: &str,
        request: &InventoryAccountMappingRequest,
    ) -> Result<(), OpenTallyError> {
        Self::validate_ledger(
            pool,
            company,
            &request.inventory_asset_ledger_id,
            &["Assets"],
        )
        .await?;
        Self::validate_ledger(
            pool,
            company,
            &request.adjustment_expense_ledger_id,
            &["Expenses"],
        )
        .await?;
        Self::validate_ledger(pool, company, &request.cogs_ledger_id, &["Expenses"]).await?;
        Self::validate_ledger(
            pool,
            company,
            &request.receipt_offset_ledger_id,
            &["Liabilities", "Expenses"],
        )
        .await?;
        if let Some(id) = &request.adjustment_gain_ledger_id {
            Self::validate_ledger(pool, company, id, &["Income"]).await?;
        }
        let series:i64=sqlx::query_scalar("SELECT COUNT(*) FROM voucher_number_series WHERE company_id=? AND id=? AND voucher_type_id='vt_jnl' AND is_active=1").bind(company).bind(&request.journal_series_id).fetch_one(pool).await?;
        if series != 1 {
            return Err(OpenTallyError::Validation(
                "Journal series is missing, inactive, or not a Journal series".into(),
            ));
        }
        Ok(())
    }
    pub async fn configure_mapping(
        pool: &SqlitePool,
        company: &str,
        request: InventoryAccountMappingRequest,
    ) -> Result<InventoryAccountMapping, OpenTallyError> {
        Self::validate_mapping(pool, company, &request).await?;
        let history:i64=sqlx::query_scalar("SELECT COUNT(*) FROM inventory_accounting_integrations WHERE company_id=? AND status='integrated'").bind(company).fetch_one(pool).await?;
        if history > 0 {
            let old = Self::get_mapping(pool, company).await?;
            if old.as_ref().map(|x| {
                (
                    &x.inventory_asset_ledger_id,
                    &x.cogs_ledger_id,
                    &x.receipt_offset_ledger_id,
                )
            }) != Some((
                &request.inventory_asset_ledger_id,
                &request.cogs_ledger_id,
                &request.receipt_offset_ledger_id,
            )) {
                return Err(OpenTallyError::Validation("Core account mappings cannot change after integrated accounting history exists".into()));
            }
        }
        sqlx::query("INSERT INTO inventory_account_mappings(company_id,inventory_asset_ledger_id,adjustment_expense_ledger_id,adjustment_gain_ledger_id,cogs_ledger_id,receipt_offset_ledger_id,journal_series_id,enabled) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(company_id) DO UPDATE SET inventory_asset_ledger_id=excluded.inventory_asset_ledger_id,adjustment_expense_ledger_id=excluded.adjustment_expense_ledger_id,adjustment_gain_ledger_id=excluded.adjustment_gain_ledger_id,cogs_ledger_id=excluded.cogs_ledger_id,receipt_offset_ledger_id=excluded.receipt_offset_ledger_id,journal_series_id=excluded.journal_series_id,enabled=excluded.enabled,updated_at=CURRENT_TIMESTAMP").bind(company).bind(&request.inventory_asset_ledger_id).bind(&request.adjustment_expense_ledger_id).bind(&request.adjustment_gain_ledger_id).bind(&request.cogs_ledger_id).bind(&request.receipt_offset_ledger_id).bind(&request.journal_series_id).bind(request.enabled).execute(pool).await?;
        Self::get_mapping(pool, company)
            .await?
            .ok_or_else(|| OpenTallyError::IntegrityViolation("Mapping was not persisted".into()))
    }
    async fn voucher(
        pool: &SqlitePool,
        company: &str,
        id: &str,
    ) -> Result<(StockVoucher, Vec<StockVoucherLine>), OpenTallyError> {
        crate::inventory::StockVoucherService::get(pool, company, id).await
    }
    async fn amount(
        pool: &SqlitePool,
        company: &str,
        voucher: &StockVoucher,
        lines: &[StockVoucherLine],
    ) -> Result<(Decimal, String), OpenTallyError> {
        let method = ValuationService::get_config(pool, company)
            .await?
            .valuation_method;
        let attribution_table: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='inventory_valuation_attributions'")
            .fetch_one(pool).await?;
        if attribution_table == 1 && voucher.voucher_type != "transfer" {
            let records = crate::inventory::InventoryValuationAttributionService::for_voucher(
                pool,
                company,
                &voucher.id,
            )
            .await?;
            if records.is_empty() {
                return Err(OpenTallyError::Validation(
                    "The posted inventory voucher has no immutable valuation attribution".into(),
                ));
            }
            let total = records
                .iter()
                .map(|x| dec(&x.total_value))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .sum();
            let persisted_method = records
                .first()
                .map(|x| x.valuation_method.clone())
                .unwrap_or(method);
            return Ok((total, persisted_method));
        }
        let amount = match voucher.voucher_type.as_str() {
            "receipt" => {
                let mut total = Decimal::ZERO;
                for line in lines {
                    let cost = line.unit_cost.as_deref().ok_or_else(|| {
                        OpenTallyError::Validation(
                            "Receipt has no reliable unit cost; accounting integration blocked"
                                .into(),
                        )
                    })?;
                    total += dec(&line.quantity)? * dec(cost)?;
                }
                total
            }
            "delivery" => {
                let before = if voucher.voucher_date == "0001-01-01" {
                    "0001-01-01".into()
                } else {
                    let d = chrono::NaiveDate::parse_from_str(&voucher.voucher_date, "%Y-%m-%d")
                        .map_err(|_| {
                            OpenTallyError::Validation("Invalid stock voucher date".into())
                        })?;
                    (d - chrono::Duration::days(1))
                        .format("%Y-%m-%d")
                        .to_string()
                };
                let before_rows = ValuationService::summary(
                    pool,
                    company,
                    ValuationRequest {
                        as_of_date: before,
                        stock_item_id: None,
                        stock_group_id: None,
                        stock_category_id: None,
                        godown_id: None,
                        search: None,
                        include_zero: true,
                    },
                )
                .await?;
                let after_rows = ValuationService::summary(
                    pool,
                    company,
                    ValuationRequest {
                        as_of_date: voucher.voucher_date.clone(),
                        stock_item_id: None,
                        stock_group_id: None,
                        stock_category_id: None,
                        godown_id: None,
                        search: None,
                        include_zero: true,
                    },
                )
                .await?;
                let before_v = before_rows
                    .iter()
                    .map(|x| dec(&x.total_value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum::<Decimal>();
                let after_v = after_rows
                    .iter()
                    .map(|x| dec(&x.total_value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum::<Decimal>();
                let result = before_v - after_v;
                if result < Decimal::ZERO {
                    return Err(OpenTallyError::Validation(
                        "Delivery valuation could not be reliably determined".into(),
                    ));
                }
                result
            }
            "adjustment" => {
                let before = chrono::NaiveDate::parse_from_str(&voucher.voucher_date, "%Y-%m-%d")
                    .map_err(|_| {
                    OpenTallyError::Validation("Invalid stock voucher date".into())
                })? - chrono::Duration::days(1);
                let before_rows = ValuationService::summary(
                    pool,
                    company,
                    ValuationRequest {
                        as_of_date: before.format("%Y-%m-%d").to_string(),
                        stock_item_id: None,
                        stock_group_id: None,
                        stock_category_id: None,
                        godown_id: None,
                        search: None,
                        include_zero: true,
                    },
                )
                .await?;
                let after_rows = ValuationService::summary(
                    pool,
                    company,
                    ValuationRequest {
                        as_of_date: voucher.voucher_date.clone(),
                        stock_item_id: None,
                        stock_group_id: None,
                        stock_category_id: None,
                        godown_id: None,
                        search: None,
                        include_zero: true,
                    },
                )
                .await?;
                let before_value: Decimal = before_rows
                    .iter()
                    .map(|x| dec(&x.total_value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum();
                let after_value: Decimal = after_rows
                    .iter()
                    .map(|x| dec(&x.total_value))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum();
                (after_value - before_value).abs()
            }
            "transfer" => Decimal::ZERO,
            _ => {
                return Err(OpenTallyError::Validation(
                    "Unsupported inventory voucher type".into(),
                ))
            }
        };
        Ok((amount, method))
    }
    pub async fn preview(
        pool: &SqlitePool,
        company: &str,
        id: &str,
    ) -> Result<InventoryAccountingPreview, OpenTallyError> {
        let mapping = Self::get_mapping(pool, company).await?.ok_or_else(|| {
            OpenTallyError::Validation("Inventory accounting mappings are not configured".into())
        })?;
        if !mapping.enabled {
            return Err(OpenTallyError::Validation(
                "Inventory accounting integration is disabled".into(),
            ));
        }
        let (voucher, lines) = Self::voucher(pool, company, id).await?;
        if voucher.status != "posted" {
            return Err(OpenTallyError::Validation(
                "Only posted stock vouchers can be integrated".into(),
            ));
        }
        if voucher.voucher_type == "transfer" {
            return Ok(InventoryAccountingPreview {
                stock_voucher: voucher,
                valuation_method: "none".into(),
                amount: "0".into(),
                lines: Vec::new(),
                total_debit: "0".into(),
                total_credit: "0".into(),
                warning: Some("Internal transfers have no company-wide accounting effect".into()),
            });
        }
        let (amount, method) = Self::amount(pool, company, &voucher, &lines).await?;
        let mut out = Vec::new();
        match voucher.voucher_type.as_str() {
            "receipt" => {
                out.push(AccountingImpactLine {
                    ledger_id: mapping.inventory_asset_ledger_id,
                    entry_type: "Dr".into(),
                    amount: fmt(amount),
                    narration: "Inventory receipt".into(),
                });
                out.push(AccountingImpactLine {
                    ledger_id: mapping.receipt_offset_ledger_id,
                    entry_type: "Cr".into(),
                    amount: fmt(amount),
                    narration: "Inventory receipt offset".into(),
                });
            }
            "delivery" => {
                out.push(AccountingImpactLine {
                    ledger_id: mapping.cogs_ledger_id,
                    entry_type: "Dr".into(),
                    amount: fmt(amount),
                    narration: "Inventory delivery cost".into(),
                });
                out.push(AccountingImpactLine {
                    ledger_id: mapping.inventory_asset_ledger_id,
                    entry_type: "Cr".into(),
                    amount: fmt(amount),
                    narration: "Inventory asset consumption".into(),
                });
            }
            "adjustment" => {
                let signed_difference: Decimal = lines
                    .iter()
                    .filter_map(|line| line.difference_quantity.as_deref())
                    .map(dec)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .sum();
                if signed_difference >= Decimal::ZERO {
                    let gain = mapping.adjustment_gain_ledger_id.ok_or_else(|| {
                        OpenTallyError::Validation(
                            "A gain ledger is required for positive physical adjustments".into(),
                        )
                    })?;
                    out.push(AccountingImpactLine {
                        ledger_id: mapping.inventory_asset_ledger_id,
                        entry_type: "Dr".into(),
                        amount: fmt(amount),
                        narration: "Physical stock gain".into(),
                    });
                    out.push(AccountingImpactLine {
                        ledger_id: gain,
                        entry_type: "Cr".into(),
                        amount: fmt(amount),
                        narration: "Physical stock gain".into(),
                    });
                } else {
                    out.push(AccountingImpactLine {
                        ledger_id: mapping.adjustment_expense_ledger_id,
                        entry_type: "Dr".into(),
                        amount: fmt(amount),
                        narration: "Physical stock loss".into(),
                    });
                    out.push(AccountingImpactLine {
                        ledger_id: mapping.inventory_asset_ledger_id,
                        entry_type: "Cr".into(),
                        amount: fmt(amount),
                        narration: "Physical stock loss".into(),
                    });
                }
            }
            _ => {}
        }
        Ok(InventoryAccountingPreview {
            stock_voucher: voucher,
            valuation_method: method,
            amount: fmt(amount),
            lines: out,
            total_debit: fmt(amount),
            total_credit: fmt(amount),
            warning: None,
        })
    }
    pub async fn status(
        pool: &SqlitePool,
        company: &str,
        stock_voucher_id: &str,
    ) -> Result<Option<InventoryAccountingIntegration>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT id,company_id,stock_voucher_id,stock_voucher_type,accounting_voucher_id,status,valuation_method,integrated_amount,attempt_count,error_details,created_at,updated_at FROM inventory_accounting_integrations WHERE company_id=? AND stock_voucher_id=?").bind(company).bind(stock_voucher_id).fetch_optional(pool).await?)
    }
    pub async fn history(
        pool: &SqlitePool,
        company: &str,
    ) -> Result<Vec<InventoryAccountingIntegration>, OpenTallyError> {
        Ok(sqlx::query_as("SELECT id,company_id,stock_voucher_id,stock_voucher_type,accounting_voucher_id,status,valuation_method,integrated_amount,attempt_count,error_details,created_at,updated_at FROM inventory_accounting_integrations WHERE company_id=? ORDER BY created_at,id").bind(company).fetch_all(pool).await?)
    }
    pub async fn integrate(
        pool: &SqlitePool,
        company: &str,
        stock_voucher_id: &str,
    ) -> Result<InventoryAccountingIntegration, OpenTallyError> {
        let integration_lock = integration_lock(company, stock_voucher_id);
        let _integration_guard = integration_lock.lock().await;
        let (stock_voucher, _) = Self::voucher(pool, company, stock_voucher_id).await?;
        if let Some(original_stock_id) = stock_voucher.reversal_of_voucher_id.clone() {
            let original = Self::status(pool, company, &original_stock_id)
                .await?
                .ok_or_else(|| {
                    OpenTallyError::Validation(
                        "The original inventory voucher has not been integrated".into(),
                    )
                })?;
            let accounting_id = original.accounting_voucher_id.ok_or_else(|| {
                OpenTallyError::Validation(
                    "The original inventory accounting voucher is missing".into(),
                )
            })?;
            if original.status != "integrated" {
                return Err(OpenTallyError::Validation(
                    "The original inventory voucher is not integrated".into(),
                ));
            }
            if let Some(existing) = Self::status(pool, company, stock_voucher_id).await? {
                if existing.status != "failed" {
                    return Ok(existing);
                }
            } else {
                sqlx::query("INSERT INTO inventory_accounting_integrations(id,company_id,stock_voucher_id,stock_voucher_type,status,valuation_method,integrated_amount,attempt_count) VALUES(?,?,?,?,?,?,?,1)")
                    .bind(Uuid::new_v4().to_string()).bind(company).bind(stock_voucher_id).bind(&stock_voucher.voucher_type).bind("pending").bind(&original.valuation_method).bind(&original.integrated_amount).execute(pool).await?;
            }
            let reversal = VoucherReversalService::reverse_voucher(
                pool,
                company,
                ReverseVoucherRequest {
                    original_voucher_id: accounting_id,
                    reversal_date: stock_voucher.voucher_date.clone(),
                    reason: format!(
                        "Reversal of inventory voucher {}",
                        stock_voucher.voucher_number
                    ),
                    actor_id: Some("inventory-integration".into()),
                },
            )
            .await;
            return match reversal {
                Ok(result) => {
                    sqlx::query("UPDATE inventory_accounting_integrations SET accounting_voucher_id=?,status='integrated',updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?")
                        .bind(&result.reversing_voucher.voucher.id).bind(company).bind(stock_voucher_id).execute(pool).await?;
                    Self::status(pool, company, stock_voucher_id)
                        .await?
                        .ok_or_else(|| {
                            OpenTallyError::IntegrityViolation(
                                "Reversal integration disappeared".into(),
                            )
                        })
                }
                Err(e) => {
                    sqlx::query("UPDATE inventory_accounting_integrations SET status='failed',error_details=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?")
                        .bind(e.to_string().chars().take(500).collect::<String>()).bind(company).bind(stock_voucher_id).execute(pool).await?;
                    Err(e)
                }
            };
        }
        let preview = Self::preview(pool, company, stock_voucher_id).await?;
        if preview.stock_voucher.voucher_type == "transfer" {
            return Err(OpenTallyError::Validation(
                "Internal transfers do not create company-wide accounting entries".into(),
            ));
        }
        let mut tx = pool.begin().await?;
        let id = Uuid::new_v4().to_string();
        let inserted=sqlx::query("INSERT INTO inventory_accounting_integrations(id,company_id,stock_voucher_id,stock_voucher_type,status,valuation_method,integrated_amount,attempt_count) VALUES(?,?,?,?,?,?,?,1) ON CONFLICT(company_id,stock_voucher_id) DO NOTHING").bind(&id).bind(company).bind(stock_voucher_id).bind(&preview.stock_voucher.voucher_type).bind("pending").bind(&preview.valuation_method).bind(&preview.amount).execute(&mut *tx).await?.rows_affected();
        if inserted == 0 {
            let existing = Self::status(pool, company, stock_voucher_id)
                .await?
                .ok_or_else(|| {
                    OpenTallyError::IntegrityViolation(
                        "Integration uniqueness state missing".into(),
                    )
                })?;
            if existing.status != "failed" {
                return Ok(existing);
            }
            sqlx::query("UPDATE inventory_accounting_integrations SET status='pending',attempt_count=attempt_count+1,error_details=NULL,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=? AND status='failed'").bind(company).bind(stock_voucher_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        #[cfg(test)]
        if TEST_INTEGRATION_FAILURE.load(std::sync::atomic::Ordering::SeqCst) == 1 {
            TEST_INTEGRATION_FAILURE.store(0, std::sync::atomic::Ordering::SeqCst);
            let error = OpenTallyError::Validation(
                "Injected accounting integration failure before voucher creation".into(),
            );
            sqlx::query("UPDATE inventory_accounting_integrations SET status='failed',error_details=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?")
                .bind(error.to_string()).bind(company).bind(stock_voucher_id).execute(pool).await?;
            return Err(error);
        }
        let lines = preview
            .lines
            .iter()
            .map(|x| VoucherLineInput {
                id: None,
                ledger_id: x.ledger_id.clone(),
                entry_type: x.entry_type.clone(),
                amount: x.amount.clone(),
                narration: Some(x.narration.clone()),
            })
            .collect();
        #[cfg(test)]
        if TEST_INTEGRATION_FAILURE.load(std::sync::atomic::Ordering::SeqCst) == 3 {
            TEST_INTEGRATION_FAILURE.store(0, std::sync::atomic::Ordering::SeqCst);
            let error =
                OpenTallyError::Validation("Injected accounting voucher creation failure".into());
            sqlx::query("UPDATE inventory_accounting_integrations SET status='failed',error_details=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?")
                .bind(error.to_string()).bind(company).bind(stock_voucher_id).execute(pool).await?;
            return Err(error);
        }
        let draft = VoucherDraftService::create_draft(
            pool,
            company,
            CreateVoucherDraftRequest {
                voucher_type_id: "vt_jnl".into(),
                series_id: Self::get_mapping(pool, company)
                    .await?
                    .ok_or_else(|| OpenTallyError::Validation("Mappings missing".into()))?
                    .journal_series_id,
                date: preview.stock_voucher.voucher_date.clone(),
                effective_date: Some(preview.stock_voucher.voucher_date.clone()),
                reference_no: Some(format!(
                    "Inventory {}",
                    preview.stock_voucher.voucher_number
                )),
                reference_date: None,
                narration: Some(format!(
                    "Inventory integration for {}",
                    preview.stock_voucher.voucher_number
                )),
                lines: Some(lines),
            },
        )
        .await;
        let result = match draft {
            Ok(draft) => {
                #[cfg(test)]
                if TEST_INTEGRATION_FAILURE.load(std::sync::atomic::Ordering::SeqCst) == 2 {
                    TEST_INTEGRATION_FAILURE.store(0, std::sync::atomic::Ordering::SeqCst);
                    let error = OpenTallyError::Validation(
                        "Injected accounting integration failure after draft creation".into(),
                    );
                    let _ =
                        VoucherDraftService::delete_draft(pool, company, &draft.voucher.id).await;
                    Err(error)
                } else {
                    VoucherPostingService::post_voucher(
                        pool,
                        company,
                        &draft.voucher.id,
                        Some("inventory-integration"),
                    )
                    .await
                    .map(|posted| posted.voucher.id)
                }
                #[cfg(not(test))]
                {
                    VoucherPostingService::post_voucher(
                        pool,
                        company,
                        &draft.voucher.id,
                        Some("inventory-integration"),
                    )
                    .await
                    .map(|posted| posted.voucher.id)
                }
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(accounting_id) => {
                sqlx::query("UPDATE inventory_accounting_integrations SET accounting_voucher_id=?,status='integrated',updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?").bind(&accounting_id).bind(company).bind(stock_voucher_id).execute(pool).await?;
                Self::status(pool, company, stock_voucher_id)
                    .await?
                    .ok_or_else(|| {
                        OpenTallyError::IntegrityViolation("Integrated record disappeared".into())
                    })
            }
            Err(e) => {
                let detail = e.to_string();
                sqlx::query("UPDATE inventory_accounting_integrations SET status='failed',error_details=?,updated_at=CURRENT_TIMESTAMP WHERE company_id=? AND stock_voucher_id=?").bind(&detail[..detail.len().min(500)]).bind(company).bind(stock_voucher_id).execute(pool).await?;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
        run_sprint4_stage12_valuation_migration, run_sprint5_stage14_integration_migration,
        run_sprint5_stage15_valuation_attribution_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use crate::inventory::{StockVoucherLineRequest, StockVoucherRequest, StockVoucherService};
    async fn setup() -> DatabaseManager {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("stage14.db");
        std::mem::forget(dir);
        let db = DatabaseManager::init(&db_path).await.unwrap();
        let p = db.pool();
        run_sprint1_migration(p).await.unwrap();
        run_stage3_migration(p).await.unwrap();
        run_sprint2_migration(p).await.unwrap();
        run_sprint2_stage4_hardening_migration(p).await.unwrap();
        run_sprint4_inventory_migration(p).await.unwrap();
        run_sprint4_stage11_stock_migration(p).await.unwrap();
        run_sprint4_stage12_valuation_migration(p).await.unwrap();
        run_sprint5_stage14_integration_migration(p).await.unwrap();
        run_sprint5_stage15_valuation_attribution_migration(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('ac','Accounting Co','Accounting Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO account_groups(id,company_id,name,parent_id,nature,is_primary,is_predefined,is_hidden,affects_gross_profit) VALUES('ag_a','ac','Assets',NULL,'Assets',1,0,0,0),('ag_e','ac','Expenses',NULL,'Expenses',1,0,0,1),('ag_i','ac','Income',NULL,'Income',1,0,0,1),('ag_l','ac','Liabilities',NULL,'Liabilities',1,0,0,0)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO ledgers(id,company_id,group_id,name,is_reserved,ledger_classification) VALUES('asset','ac','ag_a','Inventory Asset',0,'standard'),('expense','ac','ag_e','COGS',0,'standard'),('gain','ac','ag_i','Stock Gain',0,'standard'),('offset','ac','ag_l','Receipt Offset',0,'standard')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO voucher_number_series(id,company_id,voucher_type_id,series_name,series_code,numbering_method,restart_period) VALUES('js','ac','vt_jnl','Integration Journal','INT','automatic','yearly')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO voucher_number_configs(id,company_id,voucher_type_id,series_id,applicable_from,prefix,suffix,start_number,width) VALUES('jc','ac','vt_jnl','js','2026-04-01','J/','',1,4)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('sg','ac','Goods')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','ac','Nos','Numbers',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('item','ac','Widget','sg','u','0','0','0','0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('g','ac','Main')")
            .execute(p)
            .await
            .unwrap();
        db
    }
    #[tokio::test]
    async fn receipt_integration_is_balanced_and_idempotent() {
        let _failure_guard = test_integration_failure_lock().lock().await;
        let db = setup().await;
        InventoryAccountingService::configure_mapping(
            db.pool(),
            "ac",
            InventoryAccountMappingRequest {
                inventory_asset_ledger_id: "asset".into(),
                adjustment_expense_ledger_id: "expense".into(),
                adjustment_gain_ledger_id: Some("gain".into()),
                cogs_ledger_id: "expense".into(),
                receipt_offset_ledger_id: "offset".into(),
                journal_series_id: "js".into(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        let v = StockVoucherService::create_draft(
            db.pool(),
            "ac",
            StockVoucherRequest {
                voucher_type: "receipt".into(),
                voucher_date: "2026-04-02".into(),
                reference_no: None,
                narration: None,
                lines: vec![StockVoucherLineRequest {
                    stock_item_id: "item".into(),
                    unit_id: "u".into(),
                    godown_id: Some("g".into()),
                    source_godown_id: None,
                    destination_godown_id: None,
                    quantity: "2".into(),
                    unit_cost: Some("12.50".into()),
                    physical_quantity: None,
                    description: None,
                }],
            },
        )
        .await
        .unwrap();
        StockVoucherService::post(db.pool(), "ac", &v.id)
            .await
            .unwrap();
        let p = InventoryAccountingService::preview(db.pool(), "ac", &v.id)
            .await
            .unwrap();
        assert_eq!(p.amount, "25");
        assert_eq!(p.total_debit, p.total_credit);
        let first = InventoryAccountingService::integrate(db.pool(), "ac", &v.id)
            .await
            .unwrap();
        let second = InventoryAccountingService::integrate(db.pool(), "ac", &v.id)
            .await
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(first.status, "integrated");
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id=? AND id=?")
                .bind("ac")
                .bind(first.accounting_voucher_id.as_deref())
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(count, 1);
    }
}
