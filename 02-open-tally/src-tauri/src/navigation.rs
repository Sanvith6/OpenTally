use crate::errors::OpenTallyError;
use crate::vouchers::model::{VoucherLineageResponse, VoucherRegisterItem};
use crate::vouchers::service::search::escape_like_wildcards;
use crate::vouchers::service::{execute_read_only_query, SqliteVoucherSearchRepository};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use tracing::info;

/// Result item returned by the global Go To command palette search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GotoSearchResult {
    pub category: String, // "Screen" | "Ledger" | "Account Group" | "Stock Item" | "Voucher"
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub target_view: String,
    pub target_id: Option<String>,
}

/// Detailed line item in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherFullLineDetail {
    pub id: String,
    pub line_number: i64,
    pub ledger_id: String,
    pub ledger_name: String,
    pub group_name: String,
    pub classification: String,
    pub entry_type: String, // "Dr" | "Cr"
    pub amount: String,
    pub narration: Option<String>,
}

/// Detailed GST line item in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherGstDetailItem {
    pub id: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub party_gstin: Option<String>,
    pub hsn_sac_code: String,
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub taxability: String,
    pub taxable_value: String,
    pub gst_rate: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub is_reverse_charge: bool,
    pub is_cancelled: bool,
}

/// Detailed bill-wise allocation in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherBillAllocationItem {
    pub id: String,
    pub voucher_line_id: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub bill_name: String,
    pub allocation_type: String,
    pub amount: String,
    pub dr_cr: String,
    pub bill_date: String,
    pub due_date: Option<String>,
    pub is_cancelled: bool,
}

/// Detailed stock movement in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherStockMovementItem {
    pub id: String,
    pub stock_item_id: String,
    pub stock_item_name: String,
    pub godown_name: Option<String>,
    pub movement_type: String,
    pub quantity: String,
    pub rate: Option<String>,
    pub amount: Option<String>,
}

/// Detailed bank allocation in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherBankAllocationItem {
    pub id: String,
    pub transaction_type: String,
    pub instrument_number: Option<String>,
    pub instrument_date: Option<String>,
    pub favouring_name: Option<String>,
    pub amount: String,
    pub bank_date: Option<String>,
}

/// Detailed multi-currency breakdown in the voucher inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherCurrencyDetailItem {
    pub id: String,
    pub currency_code: String,
    pub foreign_amount: String,
    pub exchange_rate: String,
    pub base_amount: String,
}

/// Full comprehensive voucher details returned to the frontend inspector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoucherFullDetail {
    pub voucher: VoucherRegisterItem,
    pub lines: Vec<VoucherFullLineDetail>,
    pub gst_details: Vec<VoucherGstDetailItem>,
    pub bill_allocations: Vec<VoucherBillAllocationItem>,
    pub stock_movements: Vec<VoucherStockMovementItem>,
    pub lineage: Option<VoucherLineageResponse>,
    #[serde(default)]
    pub bank_allocations: Vec<VoucherBankAllocationItem>,
    #[serde(default)]
    pub currency_details: Vec<VoucherCurrencyDetailItem>,
}

pub struct NavigationSearchService;

impl NavigationSearchService {
    /// Global multi-category search for Go To command palette.
    /// Strictly enforces read-only execution and multi-company isolation.
    pub async fn global_goto_search(
        pool: &Pool<Sqlite>,
        company_id: &str,
        query: &str,
        limit: u32,
    ) -> Result<Vec<GotoSearchResult>, OpenTallyError> {
        let clean_query = query.trim();
        if clean_query.is_empty() {
            return Ok(Vec::new());
        }

        let escaped = escape_like_wildcards(clean_query);
        let pattern = format!("%{}%", escaped);
        let cap = (limit.clamp(1, 50)) as i64;
        let comp_id = company_id.to_string();

        execute_read_only_query(pool, move |tx| {
            Box::pin(async move {
                let mut results: Vec<GotoSearchResult> = Vec::new();

                // 1. Search Ledgers
                let ledger_sql = "
                    SELECT l.id, l.name, g.name as group_name
                    FROM ledgers l
                    JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
                    WHERE l.company_id = ? AND (l.name LIKE ? ESCAPE '\\' OR (l.description IS NOT NULL AND l.description LIKE ? ESCAPE '\\'))
                    ORDER BY l.name ASC LIMIT ?
                ";
                let ledger_rows = sqlx::query(ledger_sql)
                    .bind(&comp_id)
                    .bind(&pattern)
                    .bind(&pattern)
                    .bind(cap)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                for row in ledger_rows {
                    let id: String = row.get("id");
                    let name: String = row.get("name");
                    let group_name: String = row.get("group_name");
                    results.push(GotoSearchResult {
                        category: "Ledger".to_string(),
                        id: id.clone(),
                        title: name,
                        subtitle: format!("Group: {}", group_name),
                        target_view: "ledger_report".to_string(),
                        target_id: Some(id),
                    });
                }

                // 2. Search Account Groups
                let group_sql = "
                    SELECT g.id, g.name, g.nature
                    FROM account_groups g
                    WHERE g.company_id = ? AND g.name LIKE ? ESCAPE '\\'
                    ORDER BY g.name ASC LIMIT ?
                ";
                let group_rows = sqlx::query(group_sql)
                    .bind(&comp_id)
                    .bind(&pattern)
                    .bind(cap)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                for row in group_rows {
                    let id: String = row.get("id");
                    let name: String = row.get("name");
                    let nature: String = row.get("nature");
                    results.push(GotoSearchResult {
                        category: "Account Group".to_string(),
                        id: id.clone(),
                        title: name,
                        subtitle: format!("Nature: {}", nature),
                        target_view: "chart_of_accounts".to_string(),
                        target_id: Some(id),
                    });
                }

                // 3. Search Stock Items (defensive table existence check)
                let has_stock_items: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='stock_items')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_stock_items {
                    let stock_sql = "
                        SELECT s.id, s.name, COALESCE(s.sku, '') as sku, COALESCE(u.symbol, '') as unit_symbol
                        FROM stock_items s
                        LEFT JOIN units u ON u.company_id = s.company_id AND u.id = s.base_unit_id
                        WHERE s.company_id = ? AND (s.name LIKE ? ESCAPE '\\' OR (s.sku IS NOT NULL AND s.sku LIKE ? ESCAPE '\\'))
                        ORDER BY s.name ASC LIMIT ?
                    ";
                    let stock_rows = sqlx::query(stock_sql)
                        .bind(&comp_id)
                        .bind(&pattern)
                        .bind(&pattern)
                        .bind(cap)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                    for row in stock_rows {
                        let id: String = row.get("id");
                        let name: String = row.get("name");
                        let sku: String = row.get("sku");
                        let unit_symbol: String = row.get("unit_symbol");
                        let subtitle = if sku.is_empty() {
                            if unit_symbol.is_empty() { "Stock Item".to_string() } else { format!("Unit: {}", unit_symbol) }
                        } else {
                            format!("SKU: {} | Unit: {}", sku, unit_symbol)
                        };
                        results.push(GotoSearchResult {
                            category: "Stock Item".to_string(),
                            id: id.clone(),
                            title: name,
                            subtitle,
                            target_view: "inventory".to_string(),
                            target_id: Some(id),
                        });
                    }
                }

                // 4. Search Vouchers (by voucher number or narration)
                let voucher_sql = "
                    SELECT v.id, v.voucher_number, vt.name as voucher_type_name, v.date,
                           COALESCE(v.total_amount, '0.00') as total_amount, COALESCE(v.narration, '') as narration
                    FROM vouchers v
                    JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
                    WHERE v.company_id = ? AND (v.voucher_number LIKE ? ESCAPE '\\' OR (v.narration IS NOT NULL AND v.narration LIKE ? ESCAPE '\\'))
                    ORDER BY v.date DESC, v.voucher_number DESC LIMIT ?
                ";
                let voucher_rows = sqlx::query(voucher_sql)
                    .bind(&comp_id)
                    .bind(&pattern)
                    .bind(&pattern)
                    .bind(cap)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                for row in voucher_rows {
                    let id: String = row.get("id");
                    let v_num: String = row.get("voucher_number");
                    let v_type: String = row.get("voucher_type_name");
                    let date: String = row.get("date");
                    let amount: String = row.get("total_amount");
                    let narr: String = row.get("narration");
                    let subtitle = if narr.is_empty() {
                        format!("{} | Date: {} | Amount: ₹{}", v_type, date, amount)
                    } else {
                        format!("{} | Date: {} | ₹{} | {}", v_type, date, amount, narr)
                    };
                    results.push(GotoSearchResult {
                        category: "Voucher".to_string(),
                        id: id.clone(),
                        title: v_num,
                        subtitle,
                        target_view: "voucher_detail".to_string(),
                        target_id: Some(id),
                    });
                }

                Ok(results)
            })
        })
        .await
    }

    /// Retrieve complete details of a voucher for the inspector view.
    /// Strictly enforces read-only execution and multi-company isolation.
    pub async fn get_voucher_full_detail(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<VoucherFullDetail, OpenTallyError> {
        let comp_id = company_id.to_string();
        let vch_id = voucher_id.to_string();

        execute_read_only_query(pool, move |tx| {
            Box::pin(async move {
                // 1. Fetch Voucher Header
                let header_sql = "
                    SELECT v.id, v.company_id, v.voucher_type_id, vt.name as voucher_type_name,
                           v.series_id, s.series_name, v.voucher_number, v.date, v.effective_date,
                           v.status, v.reference_no, v.reference_date, v.narration, v.total_amount,
                           (SELECT COUNT(*) FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id) as line_count,
                           v.reversal_of_voucher_id,
                           (SELECT rev.id FROM vouchers rev WHERE rev.company_id = v.company_id AND rev.reversal_of_voucher_id = v.id LIMIT 1) as reversal_voucher_id,
                           CASE 
                               WHEN v.status = 'cancelled' THEN (
                                   SELECT json_extract(a.new_values, '$.reason') 
                                   FROM audit_log a 
                                   WHERE a.company_id = v.company_id 
                                     AND a.entity_type = 'voucher' 
                                     AND a.entity_id = v.id 
                                     AND a.action = 'CANCEL' 
                                   ORDER BY a.timestamp DESC, a.id DESC LIMIT 1
                               )
                               ELSE NULL 
                           END as cancellation_reason,
                           v.created_at, v.updated_at
                    FROM vouchers v
                    JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
                    JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id
                    WHERE v.company_id = ? AND v.id = ?
                ";

                let voucher = sqlx::query_as::<_, VoucherRegisterItem>(header_sql)
                    .bind(&comp_id)
                    .bind(&vch_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?
                    .ok_or_else(|| OpenTallyError::NotFound(format!("Voucher '{}' not found", vch_id)))?;

                // 2. Fetch GL Lines
                let lines_sql = "
                    SELECT vl.id, vl.line_number, vl.ledger_id, l.name as ledger_name,
                           g.name as group_name, g.nature as classification, vl.entry_type, vl.amount, vl.narration
                    FROM voucher_lines vl
                    JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                    JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
                    WHERE vl.company_id = ? AND vl.voucher_id = ?
                    ORDER BY vl.line_number ASC
                ";
                let line_rows = sqlx::query(lines_sql)
                    .bind(&comp_id)
                    .bind(&vch_id)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                let mut lines = Vec::new();
                for r in line_rows {
                    lines.push(VoucherFullLineDetail {
                        id: r.get("id"),
                        line_number: r.get("line_number"),
                        ledger_id: r.get("ledger_id"),
                        ledger_name: r.get("ledger_name"),
                        group_name: r.get("group_name"),
                        classification: r.get("classification"),
                        entry_type: r.get("entry_type"),
                        amount: r.get("amount"),
                        narration: r.get("narration"),
                    });
                }

                // 3. Fetch GST Breakdown (defensive table existence check)
                let mut gst_details = Vec::new();
                let has_gst: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='voucher_gst_details')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_gst {
                    let gst_sql = "
                        SELECT vg.id, vg.party_ledger_id, l.name as party_name, vg.party_gstin,
                               vg.hsn_sac_code, vg.place_of_supply_state_code, vg.supply_type,
                               vg.taxability, vg.taxable_value, vg.gst_rate, vg.cgst_amount,
                               vg.sgst_amount, vg.igst_amount, vg.cess_amount, vg.is_reverse_charge,
                               vg.is_cancelled
                        FROM voucher_gst_details vg
                        JOIN ledgers l ON l.company_id = vg.company_id AND l.id = vg.party_ledger_id
                        WHERE vg.company_id = ? AND vg.voucher_id = ?
                        ORDER BY vg.created_at ASC
                    ";
                    let gst_rows = sqlx::query(gst_sql)
                        .bind(&comp_id)
                        .bind(&vch_id)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                    for r in gst_rows {
                        let is_rc: i64 = r.get("is_reverse_charge");
                        let is_canc: i64 = r.get("is_cancelled");
                        gst_details.push(VoucherGstDetailItem {
                            id: r.get("id"),
                            party_ledger_id: r.get("party_ledger_id"),
                            party_name: r.get("party_name"),
                            party_gstin: r.get("party_gstin"),
                            hsn_sac_code: r.get("hsn_sac_code"),
                            place_of_supply_state_code: r.get("place_of_supply_state_code"),
                            supply_type: r.get("supply_type"),
                            taxability: r.get("taxability"),
                            taxable_value: r.get("taxable_value"),
                            gst_rate: r.get("gst_rate"),
                            cgst_amount: r.get("cgst_amount"),
                            sgst_amount: r.get("sgst_amount"),
                            igst_amount: r.get("igst_amount"),
                            cess_amount: r.get("cess_amount"),
                            is_reverse_charge: is_rc != 0,
                            is_cancelled: is_canc != 0,
                        });
                    }
                }

                // 4. Fetch Bill Allocations (defensive table existence check)
                let mut bill_allocations = Vec::new();
                let has_billwise: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='bill_allocations')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_billwise {
                    let bill_sql = "
                        SELECT ba.id, ba.voucher_line_id, ba.party_ledger_id, l.name as party_name,
                               ba.bill_name, ba.allocation_type, ba.amount, ba.dr_cr, ba.bill_date,
                               ba.due_date, ba.is_cancelled
                        FROM bill_allocations ba
                        JOIN ledgers l ON l.company_id = ba.company_id AND l.id = ba.party_ledger_id
                        WHERE ba.company_id = ? AND ba.voucher_id = ?
                        ORDER BY ba.created_at ASC
                    ";
                    let bill_rows = sqlx::query(bill_sql)
                        .bind(&comp_id)
                        .bind(&vch_id)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                    for r in bill_rows {
                        let is_canc: i64 = r.get("is_cancelled");
                        bill_allocations.push(VoucherBillAllocationItem {
                            id: r.get("id"),
                            voucher_line_id: r.get("voucher_line_id"),
                            party_ledger_id: r.get("party_ledger_id"),
                            party_name: r.get("party_name"),
                            bill_name: r.get("bill_name"),
                            allocation_type: r.get("allocation_type"),
                            amount: r.get("amount"),
                            dr_cr: r.get("dr_cr"),
                            bill_date: r.get("bill_date"),
                            due_date: r.get("due_date"),
                            is_cancelled: is_canc != 0,
                        });
                    }
                }

                // 5. Fetch Stock Movements (defensive table existence check)
                let mut stock_movements = Vec::new();
                let has_stock: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='stock_movements')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_stock {
                    let stock_sql = "
                        SELECT sm.id, sm.stock_item_id, si.name as stock_item_name,
                               g.name as godown_name, sm.movement_type, sm.signed_quantity
                        FROM stock_movements sm
                        JOIN stock_items si ON si.company_id = sm.company_id AND si.id = sm.stock_item_id
                        LEFT JOIN godowns g ON g.company_id = sm.company_id AND g.id = sm.godown_id
                        WHERE sm.company_id = ? AND sm.voucher_id IN (
                            SELECT id FROM stock_vouchers WHERE company_id = ? AND (reference_no = ? OR id = ?)
                        )
                        ORDER BY sm.created_at ASC
                    ";
                    let stock_rows = sqlx::query(stock_sql)
                        .bind(&comp_id)
                        .bind(&comp_id)
                        .bind(&vch_id)
                        .bind(&vch_id)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(OpenTallyError::Database)?;

                    for r in stock_rows {
                        stock_movements.push(VoucherStockMovementItem {
                            id: r.get("id"),
                            stock_item_id: r.get("stock_item_id"),
                            stock_item_name: r.get("stock_item_name"),
                            godown_name: r.get("godown_name"),
                            movement_type: r.get("movement_type"),
                            quantity: r.get("signed_quantity"),
                            rate: None,
                            amount: None,
                        });
                    }
                }

                // 6. Fetch Lineage Graph
                let lineage = SqliteVoucherSearchRepository::get_voucher_lineage_impl(&mut *tx, &comp_id, &vch_id).await.ok();

                // 7. Fetch Bank Allocations
                let mut bank_allocations = Vec::new();
                let has_bank_alloc: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='bank_allocations')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_bank_alloc {
                    #[allow(clippy::type_complexity)]
                    let b_rows: Vec<(String, String, Option<String>, Option<String>, Option<String>, String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT id, transaction_type, instrument_number, instrument_date, favouring_name, amount, bank_date
                        FROM bank_allocations
                        WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0
                        ORDER BY rowid ASC
                        "#,
                    )
                    .bind(&comp_id)
                    .bind(&vch_id)
                    .fetch_all(&mut *tx)
                    .await
                    .unwrap_or_default();

                    for (b_id, t_type, inst_no, inst_d, fav, amt, b_date) in b_rows {
                        bank_allocations.push(VoucherBankAllocationItem {
                            id: b_id,
                            transaction_type: t_type,
                            instrument_number: inst_no,
                            instrument_date: inst_d,
                            favouring_name: fav,
                            amount: amt,
                            bank_date: b_date,
                        });
                    }
                }

                // 8. Fetch Multi-Currency Details
                let mut currency_details = Vec::new();
                let has_curr_details: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='voucher_currency_details')"
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if has_curr_details {
                    let c_rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
                        r#"
                        SELECT id, currency_code, foreign_amount, exchange_rate, base_amount
                        FROM voucher_currency_details
                        WHERE company_id = ? AND voucher_id = ?
                        ORDER BY rowid ASC
                        "#,
                    )
                    .bind(&comp_id)
                    .bind(&vch_id)
                    .fetch_all(&mut *tx)
                    .await
                    .unwrap_or_default();

                    for (c_id, code, foreign, rate, base) in c_rows {
                        currency_details.push(VoucherCurrencyDetailItem {
                            id: c_id,
                            currency_code: code,
                            foreign_amount: foreign,
                            exchange_rate: rate,
                            base_amount: base,
                        });
                    }
                }

                info!(
                    "Retrieved voucher full detail: id={}, number={}, lines={}, gst={}, bills={}, stock={}, bank={}, currency={}",
                    voucher.id, voucher.voucher_number, lines.len(), gst_details.len(), bill_allocations.len(), stock_movements.len(), bank_allocations.len(), currency_details.len()
                );

                Ok(VoucherFullDetail {
                    voucher,
                    lines,
                    gst_details,
                    bill_allocations,
                    stock_movements,
                    lineage,
                    bank_allocations,
                    currency_details,
                })
            })
        })
        .await
    }
}
