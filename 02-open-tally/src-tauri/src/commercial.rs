use crate::errors::OpenTallyError;
use crate::inventory::accounting::InventoryAccountingService;
use crate::inventory::attribution::InventoryValuationAttributionService;
use crate::vouchers::model::{ValidationMode, VoucherLineInput, VoucherWithLines};
use crate::vouchers::service::{DoubleEntryValidator, VoucherSequenceService};
use crate::vouchers::validator::validate_voucher_date;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommercialDocumentType {
    Sales,
    Purchase,
    CreditNote,
    DebitNote,
}

impl CommercialDocumentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sales => "sales",
            Self::Purchase => "purchase",
            Self::CreditNote => "credit_note",
            Self::DebitNote => "debit_note",
        }
    }
}

/// Extensible placeholder hook for Milestone 3 (Indian Statutory GST).
/// Allows subsequent tax calculation engines to attach verified tax legs
/// without breaking or refactoring the commercial posting transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaxPostingLine {
    pub tax_type: String,      // e.g. "CGST", "SGST", "IGST"
    pub tax_ledger_id: String, // Ledger ID under Duties & Taxes
    pub rate: Decimal,         // Tax rate percentage (e.g. 9.00%)
    pub amount: Decimal,       // Exact computed tax amount
}

/// Extensible placeholder hook for Milestone 2 (Bill-Wise AR/AP Allocation).
/// Allows subsequent bill-tracking engines to attach invoice settlement allocations
/// without altering the core double-entry posting pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BillAllocationPostingLine {
    pub bill_type: String, // "New Ref", "Agst Ref", "Advance", "On Account"
    pub bill_name: String, // Invoice or bill reference identifier
    pub amount: Decimal,   // Allocated bill amount
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommercialLinePosting {
    pub line_number: i64,
    pub description: String,
    pub stock_item_id: Option<String>,
    pub unit_id: Option<String>,
    pub godown_id: Option<String>,
    pub quantity: Decimal,
    pub rate: Decimal,
    pub discount_amount: Decimal,
    pub line_amount: Decimal,
    #[serde(default)]
    pub batch_number: Option<String>,
    #[serde(default)]
    pub mfg_date: Option<String>,
    #[serde(default)]
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommercialPostingInput {
    pub company_id: String,
    pub document_type: CommercialDocumentType,
    pub source_document_id: String,
    pub source_voucher_id: Option<String>,
    pub series_id: String,
    pub date: String,
    pub due_date: Option<String>,
    pub party_id: String,
    pub party_ledger_id: String,
    pub commercial_account_ledger_id: Option<String>,
    pub total_amount: Decimal,
    pub document_reference: Option<String>,
    pub narration: Option<String>,
    pub lines: Vec<CommercialLinePosting>,
    pub tax_lines: Vec<TaxPostingLine>,
    pub bill_allocations: Vec<BillAllocationPostingLine>,
    #[serde(default)]
    pub place_of_supply_state_code: Option<String>,
    #[serde(default)]
    pub is_reverse_charge: Option<bool>,
    #[serde(default)]
    pub allow_tax_override: Option<bool>,
    #[serde(default)]
    pub original_voucher_id: Option<String>,
    #[serde(default)]
    pub return_reason: Option<String>,
    #[serde(default)]
    pub tds_section_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommercialPostingResult {
    pub voucher: VoucherWithLines,
    pub stock_voucher_id: Option<String>,
    pub cogs_amount: Option<Decimal>,
    pub cost_layer_ids: Vec<String>,
    pub consumed_layer_ids: Vec<String>,
}

pub struct CommercialPostingService;

impl CommercialPostingService {
    /// Atomically executes full double-entry GL posting and FIFO inventory movement/layer updates
    /// inside an explicit BEGIN IMMEDIATE serialized SQLite transaction.
    pub async fn post_commercial_voucher(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CommercialPostingInput,
    ) -> Result<CommercialPostingResult, OpenTallyError> {
        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let result = Self::execute_posting_transaction(&mut conn, company_id, input).await;

        match result {
            Ok(res) => match sqlx::query("COMMIT").execute(&mut *conn).await {
                Ok(_) => Ok(res),
                Err(commit_err) => {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    let _ = conn.close().await;
                    Err(OpenTallyError::Database(commit_err))
                }
            },
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    pub(crate) async fn execute_posting_transaction(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        input: CommercialPostingInput,
    ) -> Result<CommercialPostingResult, OpenTallyError> {
        // Step 1: Validate tenancy & fiscal boundaries
        let company_row = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT financial_year_begins, books_beginning_from, currency_decimal_places
             FROM companies WHERE id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Company '{}' not found", company_id)))?;

        let (fy_begins, books_beginning, precision) = company_row;
        validate_voucher_date(&input.date, &books_beginning)?;
        crate::company::FinancialYearClosingService::validate_period_not_locked(
            &mut *conn,
            company_id,
            &input.date,
        )
        .await?;

        // Step 2: Validate party ledger
        let party_ledger: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT id, name, is_reserved FROM ledgers WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&input.party_ledger_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let (party_ledger_id, _party_name, party_reserved) = party_ledger.ok_or_else(|| {
            OpenTallyError::Validation(format!(
                "Party ledger '{}' is not found in this company",
                input.party_ledger_id
            ))
        })?;

        if party_reserved == 1 {
            return Err(OpenTallyError::Validation(
                "Party ledger cannot be a reserved system ledger".to_string(),
            ));
        }

        // Step 3: Resolve & validate commercial account ledger (Sales / Purchase / Notes)
        let voucher_type_id = match input.document_type {
            CommercialDocumentType::Sales => "vt_sales",
            CommercialDocumentType::Purchase => "vt_pur",
            CommercialDocumentType::CreditNote => "vt_credit_note",
            CommercialDocumentType::DebitNote => "vt_debit_note",
        };

        let commercial_ledger_id = match input.commercial_account_ledger_id {
            Some(ref lid) => {
                let exists: Option<i64> =
                    sqlx::query_scalar("SELECT 1 FROM ledgers WHERE company_id = ? AND id = ?")
                        .bind(company_id)
                        .bind(lid)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                if exists.is_none() {
                    return Err(OpenTallyError::Validation(format!(
                        "Commercial ledger '{}' not found in this company",
                        lid
                    )));
                }
                lid.clone()
            }
            None => match input.document_type {
                CommercialDocumentType::Purchase => {
                    if let Some(mapping) =
                        InventoryAccountingService::get_mapping_tx(conn, company_id).await?
                    {
                        if mapping.enabled {
                            mapping.inventory_asset_ledger_id
                        } else {
                            Self::resolve_default_commercial_ledger(
                                conn,
                                company_id,
                                "Purchase Accounts",
                            )
                            .await?
                        }
                    } else {
                        Self::resolve_default_commercial_ledger(
                            conn,
                            company_id,
                            "Purchase Accounts",
                        )
                        .await?
                    }
                }
                CommercialDocumentType::Sales | CommercialDocumentType::CreditNote => {
                    Self::resolve_default_commercial_ledger(conn, company_id, "Sales Accounts")
                        .await?
                }
                CommercialDocumentType::DebitNote => {
                    if let Some(mapping) =
                        InventoryAccountingService::get_mapping_tx(conn, company_id).await?
                    {
                        if mapping.enabled {
                            mapping.inventory_asset_ledger_id
                        } else {
                            Self::resolve_default_commercial_ledger(
                                conn,
                                company_id,
                                "Purchase Accounts",
                            )
                            .await?
                        }
                    } else {
                        Self::resolve_default_commercial_ledger(
                            conn,
                            company_id,
                            "Purchase Accounts",
                        )
                        .await?
                    }
                }
            },
        };

        // Step 3b: Original Invoice Linkage & Over-Return Guard for Credit/Debit Notes
        let mut orig_voucher_info: Option<(String, String, String)> = None;
        if input.document_type == CommercialDocumentType::CreditNote
            || input.document_type == CommercialDocumentType::DebitNote
        {
            if let Some(ref orig_vch_id) = input.original_voucher_id {
                let orig_row: Option<(String, String, String, String, String)> = sqlx::query_as(
                    "SELECT id, voucher_type_id, voucher_number, date, status
                     FROM vouchers WHERE company_id = ? AND id = ?",
                )
                .bind(company_id)
                .bind(orig_vch_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (orig_id, orig_vt, orig_number, orig_date, orig_status) =
                    orig_row.ok_or_else(|| {
                        OpenTallyError::Validation(format!(
                            "Original voucher '{}' not found in this company",
                            orig_vch_id
                        ))
                    })?;

                if orig_status != "posted" {
                    return Err(OpenTallyError::Validation(format!(
                        "Original voucher '{}' is in status '{}', must be 'posted'",
                        orig_number, orig_status
                    )));
                }

                if input.document_type == CommercialDocumentType::CreditNote
                    && orig_vt != "vt_sales"
                {
                    return Err(OpenTallyError::Validation(format!(
                        "Credit Note can only be linked to a Sales invoice ('vt_sales'), found '{}'",
                        orig_vt
                    )));
                }
                if input.document_type == CommercialDocumentType::DebitNote && orig_vt != "vt_pur" {
                    return Err(OpenTallyError::Validation(format!(
                        "Debit Note can only be linked to a Purchase bill ('vt_pur'), found '{}'",
                        orig_vt
                    )));
                }

                // Over-return validation:
                // Identify original commercial line amount (the sales/purchase taxable amount)
                let orig_taxable: Option<String> = match input.document_type {
                    CommercialDocumentType::CreditNote => {
                        // In Sales invoices (vt_sales): Line 1 is Party (Dr), Line 2 is Sales revenue (Cr).
                        // The first 'Cr' line ordered by line_number is the commercial sales base.
                        sqlx::query_scalar(
                            "SELECT amount FROM voucher_lines
                             WHERE company_id = ? AND voucher_id = ?
                               AND entry_type = 'Cr'
                             ORDER BY line_number ASC
                             LIMIT 1",
                        )
                        .bind(company_id)
                        .bind(&orig_id)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?
                    }
                    CommercialDocumentType::DebitNote => {
                        // In Purchase bills (vt_pur): Line 1 is Purchase/Inventory asset (Dr).
                        // The first 'Dr' line ordered by line_number is the commercial purchase base.
                        sqlx::query_scalar(
                            "SELECT amount FROM voucher_lines
                             WHERE company_id = ? AND voucher_id = ?
                               AND entry_type = 'Dr'
                             ORDER BY line_number ASC
                             LIMIT 1",
                        )
                        .bind(company_id)
                        .bind(&orig_id)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?
                    }
                    _ => unreachable!(),
                };

                // 1. Line-level item quantity over-return validation
                for line in &input.lines {
                    if let Some(ref item_id) = line.stock_item_id {
                        let orig_qty_str: Option<String> = sqlx::query_scalar(
                            "SELECT svl.quantity
                             FROM stock_vouchers sv
                             JOIN stock_voucher_lines svl ON svl.voucher_id = sv.id AND svl.company_id = sv.company_id
                             WHERE sv.company_id = ? AND sv.reference_no = ? AND svl.stock_item_id = ?
                             LIMIT 1",
                        )
                        .bind(company_id)
                        .bind(&orig_id)
                        .bind(item_id)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                        if let Some(ref o_qty) = orig_qty_str {
                            if let Ok(orig_q) = Decimal::from_str_exact(o_qty) {
                                let prev_qty_rows: Vec<String> = sqlx::query_scalar(
                                    "SELECT svl.quantity
                                     FROM commercial_note_linkages cnl
                                     JOIN vouchers v ON v.id = cnl.note_voucher_id AND v.company_id = cnl.company_id
                                     JOIN stock_vouchers sv ON sv.reference_no = v.id AND sv.company_id = v.company_id
                                     JOIN stock_voucher_lines svl ON svl.voucher_id = sv.id AND svl.company_id = sv.company_id
                                     WHERE cnl.company_id = ? AND cnl.original_voucher_id = ?
                                       AND v.status = 'posted' AND svl.stock_item_id = ?",
                                )
                                .bind(company_id)
                                .bind(&orig_id)
                                .bind(item_id)
                                .fetch_all(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?;

                                let prev_qty_sum: Decimal = prev_qty_rows
                                    .iter()
                                    .filter_map(|s| Decimal::from_str_exact(s).ok())
                                    .sum();

                                if (prev_qty_sum + line.quantity) > orig_q {
                                    return Err(OpenTallyError::Validation(format!(
                                        "Over-return rejected for item '{}': Total returned quantity ({}) exceeds original quantity ({})",
                                        item_id,
                                        prev_qty_sum + line.quantity,
                                        orig_q
                                    )));
                                }
                            }
                        }
                    }
                }

                // 2. Document-level taxable value over-return validation
                if let Some(ref o_amt_str) = orig_taxable {
                    if let Ok(orig_taxable_val) = Decimal::from_str_exact(o_amt_str) {
                        // Query previous return commercial lines only (excluding tax lines and inventory movements)
                        // In Credit Notes: Line 1 is Sales Return (Dr).
                        // In Debit Notes: Line 2 is Purchase Return (Cr).
                        let (note_entry_type, note_line_number) = match input.document_type {
                            CommercialDocumentType::CreditNote => ("Dr", 1i64),
                            CommercialDocumentType::DebitNote => ("Cr", 2i64),
                            _ => unreachable!(),
                        };

                        let prev_returned_rows: Vec<String> = sqlx::query_scalar(
                            "SELECT vl.amount
                             FROM commercial_note_linkages cnl
                             JOIN vouchers v ON v.id = cnl.note_voucher_id AND v.company_id = cnl.company_id
                             JOIN voucher_lines vl ON vl.voucher_id = v.id AND vl.company_id = v.company_id
                             WHERE cnl.company_id = ? AND cnl.original_voucher_id = ?
                               AND v.status = 'posted'
                               AND vl.entry_type = ?
                               AND vl.line_number = ?",
                        )
                        .bind(company_id)
                        .bind(&orig_id)
                        .bind(note_entry_type)
                        .bind(note_line_number)
                        .fetch_all(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                        let prev_returned_sum: Decimal = prev_returned_rows
                            .iter()
                            .filter_map(|s| Decimal::from_str_exact(s).ok())
                            .sum();

                        if (prev_returned_sum + input.total_amount) > orig_taxable_val
                            && orig_taxable_val > Decimal::ZERO
                        {
                            return Err(OpenTallyError::Validation(format!(
                                "Over-return rejected: Total returned taxable value ({}) would exceed original invoice taxable value ({})",
                                prev_returned_sum + input.total_amount,
                                orig_taxable_val
                            )));
                        }
                    }
                }

                orig_voucher_info = Some((orig_id, orig_number, orig_date));
            }
        }

        // Step 4: Validate line items
        if input.lines.is_empty() {
            return Err(OpenTallyError::Validation(
                "Commercial voucher requires at least one line item".to_string(),
            ));
        }

        for line in &input.lines {
            if line.quantity <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(format!(
                    "Line {}: quantity must be strictly positive",
                    line.line_number
                )));
            }
            if line.rate < Decimal::ZERO {
                return Err(OpenTallyError::Validation(format!(
                    "Line {}: rate cannot be negative",
                    line.line_number
                )));
            }
            if let Some(ref item_id) = line.stock_item_id {
                let item_active: Option<(String, bool)> = sqlx::query_as(
                    "SELECT base_unit_id, is_active FROM stock_items WHERE company_id = ? AND id = ?",
                )
                .bind(company_id)
                .bind(item_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (base_unit, active) = item_active.ok_or_else(|| {
                    OpenTallyError::Validation(format!("Stock item '{}' not found", item_id))
                })?;

                if !active {
                    return Err(OpenTallyError::Validation(format!(
                        "Stock item '{}' is inactive",
                        item_id
                    )));
                }

                let unit_id = line.unit_id.as_deref().ok_or_else(|| {
                    OpenTallyError::Validation(format!(
                        "Line {}: missing unit for stock item",
                        line.line_number
                    ))
                })?;

                if unit_id != base_unit {
                    return Err(OpenTallyError::Validation(format!(
                        "Line {}: unit '{}' does not match item base unit '{}'",
                        line.line_number, unit_id, base_unit
                    )));
                }

                let godown_id = line.godown_id.as_deref().ok_or_else(|| {
                    OpenTallyError::Validation(format!(
                        "Line {}: missing godown for stock item",
                        line.line_number
                    ))
                })?;

                let godown_active: Option<bool> = sqlx::query_scalar(
                    "SELECT is_active FROM godowns WHERE company_id = ? AND id = ?",
                )
                .bind(company_id)
                .bind(godown_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if godown_active != Some(true) {
                    return Err(OpenTallyError::Validation(format!(
                        "Godown '{}' is not active in this company",
                        godown_id
                    )));
                }
            }
        }

        // Step 4b: Allocate or resolve voucher_id
        let voucher_id = input
            .source_voucher_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Step 5: Process Inventory Movements & FIFO Cost Layers
        let has_inventory_lines = input.lines.iter().any(|l| l.stock_item_id.is_some());
        let mut stock_voucher_id_opt: Option<String> = None;
        let mut created_layer_ids = Vec::new();
        let mut consumed_layer_ids = Vec::new();
        let mut total_cogs = Decimal::ZERO;

        if has_inventory_lines {
            let sv_id = Uuid::new_v4().to_string();
            let sv_number = format!(
                "STK-{}-{}",
                match input.document_type {
                    CommercialDocumentType::Sales => "DLV",
                    CommercialDocumentType::Purchase => "RCP",
                    CommercialDocumentType::CreditNote => "RTN-IN",
                    CommercialDocumentType::DebitNote => "RTN-OUT",
                },
                &sv_id[..8]
            );
            let sv_type = match input.document_type {
                CommercialDocumentType::Sales => "delivery",
                CommercialDocumentType::Purchase => "receipt",
                CommercialDocumentType::CreditNote => "receipt",
                CommercialDocumentType::DebitNote => "delivery",
            };

            sqlx::query(
                "INSERT INTO stock_vouchers (
                    id, company_id, voucher_number, voucher_type, voucher_date,
                    reference_no, narration, status
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, 'posted')",
            )
            .bind(&sv_id)
            .bind(company_id)
            .bind(&sv_number)
            .bind(sv_type)
            .bind(&input.date)
            .bind(&voucher_id)
            .bind(&input.narration)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            stock_voucher_id_opt = Some(sv_id.clone());

            for line in &input.lines {
                if let Some(ref item_id) = line.stock_item_id {
                    let sv_line_id = Uuid::new_v4().to_string();
                    let godown_id = line.godown_id.as_deref().unwrap();
                    let unit_id = line.unit_id.as_deref().unwrap();

                    // 4-decimal unit cost for inventory valuation
                    let unit_cost = if line.quantity.is_zero() {
                        Decimal::ZERO
                    } else {
                        (line.line_amount / line.quantity).round_dp(4)
                    };

                    sqlx::query(
                        "INSERT INTO stock_voucher_lines (
                            id, company_id, voucher_id, line_number, stock_item_id,
                            unit_id, godown_id, quantity, unit_cost, line_value, description
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&sv_line_id)
                    .bind(company_id)
                    .bind(&sv_id)
                    .bind(line.line_number)
                    .bind(item_id)
                    .bind(unit_id)
                    .bind(godown_id)
                    .bind(line.quantity.normalize().to_string())
                    .bind(unit_cost.normalize().to_string())
                    .bind(line.line_amount.normalize().to_string())
                    .bind(&line.description)
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    let movement_id = Uuid::new_v4().to_string();
                    let signed_qty = match input.document_type {
                        CommercialDocumentType::Purchase | CommercialDocumentType::CreditNote => {
                            line.quantity
                        }
                        CommercialDocumentType::Sales | CommercialDocumentType::DebitNote => {
                            -line.quantity
                        }
                    };

                    let has_batch_support: bool = sqlx::query_scalar::<_, i32>(
                        "SELECT COUNT(*) FROM pragma_table_info('stock_movements') WHERE name = 'batch_id'",
                    )
                    .fetch_one(&mut *conn)
                    .await
                    .unwrap_or(0)
                        > 0;

                    let batch_id = if has_batch_support {
                        match input.document_type {
                            CommercialDocumentType::Purchase => {
                                if let Some(ref b_num) = line.batch_number {
                                    let bid = crate::inventory::batches::BatchService::get_or_create_batch(
                                        conn,
                                        company_id,
                                        item_id,
                                        b_num,
                                        line.mfg_date.as_deref(),
                                        line.expiry_date.as_deref(),
                                    )
                                    .await?;
                                    Some(bid)
                                } else {
                                    None
                                }
                            }
                            CommercialDocumentType::Sales => {
                                if let Some(ref b_num) = line.batch_number {
                                    let bid = crate::inventory::batches::BatchService::validate_batch_expiry_for_sale(
                                        conn,
                                        company_id,
                                        item_id,
                                        b_num,
                                        &input.date,
                                    )
                                    .await?;
                                    Some(bid)
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if has_batch_support {
                        sqlx::query(
                            "INSERT INTO stock_movements (
                                id, company_id, stock_item_id, godown_id, voucher_id,
                                voucher_line_id, movement_date, movement_type, signed_quantity, batch_id
                             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(&movement_id)
                        .bind(company_id)
                        .bind(item_id)
                        .bind(godown_id)
                        .bind(&sv_id)
                        .bind(&sv_line_id)
                        .bind(&input.date)
                        .bind(sv_type)
                        .bind(signed_qty.normalize().to_string())
                        .bind(batch_id.as_deref())
                        .execute(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;
                    } else {
                        sqlx::query(
                            "INSERT INTO stock_movements (
                                id, company_id, stock_item_id, godown_id, voucher_id,
                                voucher_line_id, movement_date, movement_type, signed_quantity
                             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(&movement_id)
                        .bind(company_id)
                        .bind(item_id)
                        .bind(godown_id)
                        .bind(&sv_id)
                        .bind(&sv_line_id)
                        .bind(&input.date)
                        .bind(sv_type)
                        .bind(signed_qty.normalize().to_string())
                        .execute(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;
                    }

                    match input.document_type {
                        CommercialDocumentType::Purchase => {
                            // Create new FIFO cost layer
                            let layer_id = Uuid::new_v4().to_string();
                            if has_batch_support {
                                sqlx::query(
                                    "INSERT INTO inventory_cost_layers (
                                        id, company_id, stock_item_id, godown_id,
                                        source_voucher_id, source_line_id, receipt_date,
                                        original_quantity, remaining_quantity, unit_cost, original_value, batch_id
                                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                                )
                                .bind(&layer_id)
                                .bind(company_id)
                                .bind(item_id)
                                .bind(godown_id)
                                .bind(&sv_id)
                                .bind(&sv_line_id)
                                .bind(&input.date)
                                .bind(line.quantity.normalize().to_string())
                                .bind(line.quantity.normalize().to_string())
                                .bind(unit_cost.normalize().to_string())
                                .bind(line.line_amount.normalize().to_string())
                                .bind(batch_id.as_deref())
                                .execute(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?;
                            } else {
                                sqlx::query(
                                    "INSERT INTO inventory_cost_layers (
                                        id, company_id, stock_item_id, godown_id,
                                        source_voucher_id, source_line_id, receipt_date,
                                        original_quantity, remaining_quantity, unit_cost, original_value
                                     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                                )
                                .bind(&layer_id)
                                .bind(company_id)
                                .bind(item_id)
                                .bind(godown_id)
                                .bind(&sv_id)
                                .bind(&sv_line_id)
                                .bind(&input.date)
                                .bind(line.quantity.normalize().to_string())
                                .bind(line.quantity.normalize().to_string())
                                .bind(unit_cost.normalize().to_string())
                                .bind(line.line_amount.normalize().to_string())
                                .execute(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?;
                            }

                            created_layer_ids.push(layer_id);
                        }
                        CommercialDocumentType::CreditNote => {
                            // Restore returned stock as a new FIFO layer at the original COGS unit cost
                            let orig_cost_str: Option<String> = if let Some(ref o_id) =
                                input.original_voucher_id
                            {
                                sqlx::query_scalar(
                                    "SELECT svl.unit_cost
                                     FROM stock_vouchers sv
                                     JOIN stock_voucher_lines svl ON svl.voucher_id = sv.id AND svl.company_id = sv.company_id
                                     WHERE sv.company_id = ? AND sv.reference_no = ? AND svl.stock_item_id = ?
                                     LIMIT 1",
                                )
                                .bind(company_id)
                                .bind(o_id)
                                .bind(item_id)
                                .fetch_optional(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?
                            } else {
                                None
                            };

                            let cogs_unit_cost = orig_cost_str
                                .and_then(|s| Decimal::from_str_exact(&s).ok())
                                .unwrap_or(unit_cost);

                            let layer_id = Uuid::new_v4().to_string();
                            let layer_val = (line.quantity * cogs_unit_cost).round_dp(4);
                            total_cogs += layer_val;

                            sqlx::query(
                                "INSERT INTO inventory_cost_layers (
                                    id, company_id, stock_item_id, godown_id,
                                    source_voucher_id, source_line_id, receipt_date,
                                    original_quantity, remaining_quantity, unit_cost, original_value
                                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                            )
                            .bind(&layer_id)
                            .bind(company_id)
                            .bind(item_id)
                            .bind(godown_id)
                            .bind(&sv_id)
                            .bind(&sv_line_id)
                            .bind(&input.date)
                            .bind(line.quantity.normalize().to_string())
                            .bind(line.quantity.normalize().to_string())
                            .bind(cogs_unit_cost.normalize().to_string())
                            .bind(layer_val.normalize().to_string())
                            .execute(&mut *conn)
                            .await
                            .map_err(OpenTallyError::Database)?;

                            created_layer_ids.push(layer_id);
                        }
                        CommercialDocumentType::Sales | CommercialDocumentType::DebitNote => {
                            // FIFO layer consumption
                            let layers: Vec<(String, String, String)> = if has_batch_support {
                                sqlx::query_as(
                                    "SELECT id, remaining_quantity, unit_cost
                                     FROM inventory_cost_layers
                                     WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                                       AND (? IS NULL OR batch_id = ?)
                                     ORDER BY receipt_date ASC, created_at ASC, id ASC",
                                )
                                .bind(company_id)
                                .bind(item_id)
                                .bind(godown_id)
                                .bind(batch_id.as_deref())
                                .bind(batch_id.as_deref())
                                .fetch_all(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?
                            } else {
                                sqlx::query_as(
                                    "SELECT id, remaining_quantity, unit_cost
                                     FROM inventory_cost_layers
                                     WHERE company_id = ? AND stock_item_id = ? AND godown_id = ?
                                     ORDER BY receipt_date ASC, created_at ASC, id ASC",
                                )
                                .bind(company_id)
                                .bind(item_id)
                                .bind(godown_id)
                                .fetch_all(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?
                            };

                            let mut available_qty = Decimal::ZERO;
                            let mut parsed_layers = Vec::new();
                            for (l_id, rem_str, cost_str) in layers {
                                let rem = Decimal::from_str_exact(&rem_str).map_err(|_| {
                                    OpenTallyError::IntegrityViolation(format!(
                                        "Corrupted layer quantity '{}' in layer '{}'",
                                        rem_str, l_id
                                    ))
                                })?;
                                let cost = Decimal::from_str_exact(&cost_str).map_err(|_| {
                                    OpenTallyError::IntegrityViolation(format!(
                                        "Corrupted layer cost '{}' in layer '{}'",
                                        cost_str, l_id
                                    ))
                                })?;
                                if rem > Decimal::ZERO {
                                    available_qty += rem;
                                    parsed_layers.push((l_id, rem, cost));
                                }
                            }

                            if available_qty < line.quantity {
                                return Err(OpenTallyError::Validation(format!(
                                    "Insufficient stock for item '{}' in godown '{}'. Needed: {}, available in cost layers: {}",
                                    item_id, godown_id, line.quantity, available_qty
                                )));
                            }

                            let mut need_qty = line.quantity;
                            for (l_id, mut rem_qty, layer_cost) in parsed_layers {
                                if need_qty <= Decimal::ZERO {
                                    break;
                                }
                                let take = rem_qty.min(need_qty);
                                rem_qty -= take;
                                need_qty -= take;

                                sqlx::query(
                                    "UPDATE inventory_cost_layers
                                     SET remaining_quantity = ?
                                     WHERE company_id = ? AND id = ?",
                                )
                                .bind(rem_qty.normalize().to_string())
                                .bind(company_id)
                                .bind(&l_id)
                                .execute(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?;

                                let consumption_id = Uuid::new_v4().to_string();
                                let consumed_val = (take * layer_cost).round_dp(4);
                                if input.document_type == CommercialDocumentType::Sales {
                                    total_cogs += consumed_val;
                                }

                                sqlx::query(
                                    "INSERT INTO inventory_valuation_consumptions (
                                        id, company_id, movement_id, layer_id,
                                        quantity, unit_cost, value
                                     ) VALUES (?, ?, ?, ?, ?, ?, ?)",
                                )
                                .bind(&consumption_id)
                                .bind(company_id)
                                .bind(&movement_id)
                                .bind(&l_id)
                                .bind(take.normalize().to_string())
                                .bind(layer_cost.normalize().to_string())
                                .bind(consumed_val.normalize().to_string())
                                .execute(&mut *conn)
                                .await
                                .map_err(OpenTallyError::Database)?;

                                consumed_layer_ids.push(l_id);
                            }
                        }
                    }

                    // Attribute valuation if attributions subsystem is installed
                    let _ = InventoryValuationAttributionService::attribute_movement(
                        conn,
                        company_id,
                        &movement_id,
                        "fifo",
                    )
                    .await;
                }
            }
        }

        // Step 6: Allocate Next Sequential Voucher Number
        let seq_alloc = VoucherSequenceService::allocate_next_sequence(
            conn,
            company_id,
            voucher_type_id,
            &input.series_id,
            &input.date,
        )
        .await?;

        // Step 6b: Resolve GST Configuration and Calculate/Validate Taxes (Milestone 3)
        let company_gst =
            crate::gst::GstService::get_company_gst_profile_tx(conn, company_id).await?;
        let gst_enabled = company_gst
            .as_ref()
            .map(|p| p.gst_enabled == 1)
            .unwrap_or(false);

        let mut computed_tax_lines = Vec::new();
        let mut gst_detail_records = Vec::new();

        if gst_enabled {
            let comp_state = company_gst
                .as_ref()
                .map(|p| p.state_code.as_str())
                .unwrap_or("01");
            let party_gst = crate::gst::GstService::get_ledger_gst_profile_tx(
                conn,
                company_id,
                &party_ledger_id,
            )
            .await?;
            let party_state = party_gst
                .as_ref()
                .map(|p| p.state_code.as_str())
                .unwrap_or(comp_state);
            let party_gstin = party_gst.as_ref().and_then(|p| p.gstin.clone());

            let pos_state = input
                .place_of_supply_state_code
                .as_deref()
                .unwrap_or(party_state);
            let ledger_mapping =
                crate::gst::GstService::get_gst_ledger_mapping_tx(conn, company_id).await?;
            let is_reverse_charge = input.is_reverse_charge.unwrap_or(false);

            let mut total_cgst = Decimal::ZERO;
            let mut total_sgst = Decimal::ZERO;
            let mut total_igst = Decimal::ZERO;
            let mut total_cess = Decimal::ZERO;

            for line in &input.lines {
                let (hsn_code, gst_rate, cess_rate, taxability) =
                    if let Some(ref item_id) = line.stock_item_id {
                        let item_map = sqlx::query_as::<_, (String, String, String, String)>(
                            "SELECT hsc.code, igm.gst_rate, igm.cess_rate, igm.taxability
                         FROM item_gst_mappings igm
                         JOIN hsn_sac_codes hsc ON hsc.id = igm.hsn_sac_code_id
                         WHERE igm.company_id = ? AND igm.stock_item_id = ?",
                        )
                        .bind(company_id)
                        .bind(item_id)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                        if let Some((code, gr, cr, tax)) = item_map {
                            (
                                code,
                                Decimal::from_str_exact(&gr).unwrap_or(Decimal::ZERO),
                                Decimal::from_str_exact(&cr).unwrap_or(Decimal::ZERO),
                                crate::gst::Taxability::from_str_loose(&tax),
                            )
                        } else {
                            (
                                "GEN-001".to_string(),
                                Decimal::ZERO,
                                Decimal::ZERO,
                                crate::gst::Taxability::Taxable,
                            )
                        }
                    } else {
                        (
                            "GEN-001".to_string(),
                            Decimal::ZERO,
                            Decimal::ZERO,
                            crate::gst::Taxability::Taxable,
                        )
                    };

                let calc_res = crate::gst::calculate_gst(&crate::gst::GstCalculationInput {
                    company_state_code: comp_state.to_string(),
                    place_of_supply_state_code: pos_state.to_string(),
                    taxability,
                    taxable_value: line.line_amount,
                    gst_rate,
                    cess_rate,
                    is_reverse_charge,
                });

                total_cgst += calc_res.cgst_amount;
                total_sgst += calc_res.sgst_amount;
                total_igst += calc_res.igst_amount;
                total_cess += calc_res.cess_amount;

                gst_detail_records.push((
                    line.line_number,
                    party_gstin.clone(),
                    hsn_code,
                    pos_state.to_string(),
                    calc_res.supply_type.as_str().to_string(),
                    taxability.as_str().to_string(),
                    line.line_amount,
                    gst_rate,
                    calc_res.cgst_amount,
                    calc_res.sgst_amount,
                    calc_res.igst_amount,
                    calc_res.cess_amount,
                    if is_reverse_charge { 1i64 } else { 0i64 },
                ));
            }

            // Anti-drift Tax Guard
            if !input.tax_lines.is_empty() {
                let allow_override = input.allow_tax_override.unwrap_or(false);
                for t in &input.tax_lines {
                    let expected_amt = match t.tax_type.to_uppercase().as_str() {
                        "CGST" => total_cgst,
                        "SGST" => total_sgst,
                        "IGST" => total_igst,
                        "CESS" => total_cess,
                        _ => Decimal::ZERO,
                    };
                    if t.amount != expected_amt {
                        if !allow_override {
                            return Err(OpenTallyError::Validation(format!(
                                "Tax drift detected for {}: manual tax {} differs from canonical computed tax {}. Override rejected without allow_tax_override: true.",
                                t.tax_type, t.amount, expected_amt
                            )));
                        } else {
                            let audit_id = Uuid::new_v4().to_string();
                            let payload = serde_json::json!({
                                "tax_type": t.tax_type,
                                "manual_amount": t.amount.to_string(),
                                "canonical_amount": expected_amt.to_string(),
                                "reason": "Explicit tax override permitted"
                            })
                            .to_string();
                            sqlx::query(
                                "INSERT INTO audit_log (id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by)
                                 VALUES (?, ?, 'voucher_tax', ?, ?, ?, 'UPDATE', NULL, ?, 'system')",
                            )
                            .bind(&audit_id)
                            .bind(company_id)
                            .bind(&voucher_id)
                            .bind(&t.tax_ledger_id)
                            .bind(&fy_begins)
                            .bind(&payload)
                            .execute(&mut *conn)
                            .await
                            .map_err(OpenTallyError::Database)?;
                        }
                    }
                }
                computed_tax_lines = input.tax_lines.clone();
                if let Some(first_rec) = gst_detail_records.first_mut() {
                    let override_cgst = input
                        .tax_lines
                        .iter()
                        .find(|t| t.tax_type.eq_ignore_ascii_case("CGST"))
                        .map(|t| t.amount)
                        .unwrap_or(Decimal::ZERO);
                    let override_sgst = input
                        .tax_lines
                        .iter()
                        .find(|t| t.tax_type.eq_ignore_ascii_case("SGST"))
                        .map(|t| t.amount)
                        .unwrap_or(Decimal::ZERO);
                    let override_igst = input
                        .tax_lines
                        .iter()
                        .find(|t| t.tax_type.eq_ignore_ascii_case("IGST"))
                        .map(|t| t.amount)
                        .unwrap_or(Decimal::ZERO);
                    let override_cess = input
                        .tax_lines
                        .iter()
                        .find(|t| t.tax_type.eq_ignore_ascii_case("Cess"))
                        .map(|t| t.amount)
                        .unwrap_or(Decimal::ZERO);
                    if override_cgst > Decimal::ZERO
                        || override_sgst > Decimal::ZERO
                        || override_igst > Decimal::ZERO
                        || override_cess > Decimal::ZERO
                    {
                        first_rec.8 = override_cgst;
                        first_rec.9 = override_sgst;
                        first_rec.10 = override_igst;
                        first_rec.11 = override_cess;
                    }
                }
            } else if let Some(ref map) = ledger_mapping {
                match input.document_type {
                    CommercialDocumentType::Sales | CommercialDocumentType::CreditNote => {
                        if total_cgst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "CGST".to_string(),
                                tax_ledger_id: map.output_cgst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_cgst,
                            });
                        }
                        if total_sgst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "SGST".to_string(),
                                tax_ledger_id: map.output_sgst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_sgst,
                            });
                        }
                        if total_igst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "IGST".to_string(),
                                tax_ledger_id: map.output_igst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_igst,
                            });
                        }
                        if total_cess > Decimal::ZERO {
                            if let Some(ref cess_led) = map.output_cess_ledger_id {
                                computed_tax_lines.push(TaxPostingLine {
                                    tax_type: "Cess".to_string(),
                                    tax_ledger_id: cess_led.clone(),
                                    rate: Decimal::ZERO,
                                    amount: total_cess,
                                });
                            }
                        }
                    }
                    CommercialDocumentType::Purchase | CommercialDocumentType::DebitNote => {
                        if total_cgst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "CGST".to_string(),
                                tax_ledger_id: map.input_cgst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_cgst,
                            });
                        }
                        if total_sgst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "SGST".to_string(),
                                tax_ledger_id: map.input_sgst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_sgst,
                            });
                        }
                        if total_igst > Decimal::ZERO {
                            computed_tax_lines.push(TaxPostingLine {
                                tax_type: "IGST".to_string(),
                                tax_ledger_id: map.input_igst_ledger_id.clone(),
                                rate: Decimal::ZERO,
                                amount: total_igst,
                            });
                        }
                        if total_cess > Decimal::ZERO {
                            if let Some(ref cess_led) = map.input_cess_ledger_id {
                                computed_tax_lines.push(TaxPostingLine {
                                    tax_type: "Cess".to_string(),
                                    tax_ledger_id: cess_led.clone(),
                                    rate: Decimal::ZERO,
                                    amount: total_cess,
                                });
                            }
                        }
                    }
                }
            }
        } else if !input.tax_lines.is_empty() {
            computed_tax_lines = input.tax_lines.clone();
        }

        let total_tax_amt: Decimal = computed_tax_lines.iter().map(|t| t.amount).sum();
        let invoice_grand_total = input.total_amount + total_tax_amt;

        // Step 6c: Calculate TDS if section specified (Milestone 9)
        let mut tds_calc_opt = None;
        let mut tds_amount_decimal = Decimal::ZERO;
        let mut tds_payable_ledger_id = String::new();

        if input.document_type == CommercialDocumentType::Purchase {
            if let Some(ref sec_code) = input.tds_section_code {
                if !sec_code.trim().is_empty() {
                    let calc = crate::tds::TdsService::calculate_tds_tx(
                        conn,
                        company_id,
                        &party_ledger_id,
                        sec_code.trim(),
                        input.total_amount, // Taxable Base (GST-exclusive per CBDT)
                        &input.date,
                    )
                    .await?;

                    if calc.threshold_exceeded && !calc.tds_amount.is_empty() {
                        let parsed_tds =
                            Decimal::from_str(&calc.tds_amount).unwrap_or(Decimal::ZERO);
                        if parsed_tds > Decimal::ZERO {
                            tds_amount_decimal = parsed_tds;
                            tds_payable_ledger_id = match calc.tds_payable_ledger_id.clone() {
                                Some(id) => id,
                                None => {
                                    crate::tds::TdsService::seed_default_tds_sections_tx(
                                        conn, company_id,
                                    )
                                    .await?
                                }
                            };
                            tds_calc_opt = Some(calc);
                        }
                    }
                }
            }
        }

        // Step 7: Construct Balanced Double-Entry General Ledger Lines
        let mut voucher_lines = Vec::new();
        let mut line_num = 1i64;

        match input.document_type {
            CommercialDocumentType::Sales => {
                // Line 1: Debit Party (Debtor/Customer) with Grand Total (Taxable + GST)
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: party_ledger_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: invoice_grand_total.normalize().to_string(),
                    narration: Some(format!("Sales invoice to {}", party_ledger_id)),
                });
                line_num += 1;

                // Line 2: Credit Sales Ledger with Net Taxable Base
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: commercial_ledger_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: input.total_amount.normalize().to_string(),
                    narration: Some("Sales revenue".to_string()),
                });
                line_num += 1;

                // Output Tax Lines: Credit Output Duties & Taxes Ledgers
                for tax_line in &computed_tax_lines {
                    voucher_lines.push(VoucherLineInput {
                        id: None,
                        ledger_id: tax_line.tax_ledger_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: tax_line.amount.normalize().to_string(),
                        narration: Some(format!("Output {}", tax_line.tax_type)),
                    });
                    line_num += 1;
                }

                // Lines: COGS & Inventory Asset Integration (if configured)
                if total_cogs > Decimal::ZERO {
                    if let Some(mapping) =
                        InventoryAccountingService::get_mapping_tx(conn, company_id).await?
                    {
                        if mapping.enabled {
                            voucher_lines.push(VoucherLineInput {
                                id: None,
                                ledger_id: mapping.cogs_ledger_id.clone(),
                                entry_type: "Dr".to_string(),
                                amount: total_cogs.normalize().to_string(),
                                narration: Some("Cost of goods sold".to_string()),
                            });
                            line_num += 1;

                            voucher_lines.push(VoucherLineInput {
                                id: None,
                                ledger_id: mapping.inventory_asset_ledger_id.clone(),
                                entry_type: "Cr".to_string(),
                                amount: total_cogs.normalize().to_string(),
                                narration: Some("Inventory reduction".to_string()),
                            });
                            let _ = line_num;
                        }
                    }
                }
            }
            CommercialDocumentType::Purchase => {
                // Line 1: Debit Purchase / Inventory Asset Ledger with Net Taxable Base
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: commercial_ledger_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: input.total_amount.normalize().to_string(),
                    narration: Some("Purchase cost".to_string()),
                });
                line_num += 1;

                // Input Tax Lines (ITC): Debit Input Duties & Taxes Ledgers
                for tax_line in &computed_tax_lines {
                    voucher_lines.push(VoucherLineInput {
                        id: None,
                        ledger_id: tax_line.tax_ledger_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: tax_line.amount.normalize().to_string(),
                        narration: Some(format!("Input ITC {}", tax_line.tax_type)),
                    });
                    line_num += 1;
                }

                // If TDS deducted: Credit TDS Payable Ledger
                if tds_amount_decimal > Decimal::ZERO {
                    let sec_code = input.tds_section_code.as_deref().unwrap_or("");
                    voucher_lines.push(VoucherLineInput {
                        id: None,
                        ledger_id: tds_payable_ledger_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: tds_amount_decimal.normalize().to_string(),
                        narration: Some(format!("TDS deducted under Section {}", sec_code)),
                    });
                    line_num += 1;
                }

                // Line 2: Credit Party (Creditor/Supplier) with Net Payable after TDS: (Taxable + GST) - TDS
                let party_payable = invoice_grand_total - tds_amount_decimal;
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: party_ledger_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: party_payable.normalize().to_string(),
                    narration: Some(format!("Purchase bill from {}", party_ledger_id)),
                });
                let _ = line_num;
            }
            CommercialDocumentType::CreditNote => {
                // Line 1: Debit Sales Return / Sales Revenue with Net Taxable Base
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: commercial_ledger_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: input.total_amount.normalize().to_string(),
                    narration: Some("Sales return".to_string()),
                });
                line_num += 1;

                // Line 2: Debit Output Duties & Taxes Ledgers (reversing output GST liability)
                for tax_line in &computed_tax_lines {
                    voucher_lines.push(VoucherLineInput {
                        id: None,
                        ledger_id: tax_line.tax_ledger_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: tax_line.amount.normalize().to_string(),
                        narration: Some(format!("Output {} reversal", tax_line.tax_type)),
                    });
                    line_num += 1;
                }

                // Line 3: Credit Party (Debtor/Customer) with Grand Total (Taxable + GST)
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: party_ledger_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: invoice_grand_total.normalize().to_string(),
                    narration: Some(format!("Credit note to {}", party_ledger_id)),
                });
                line_num += 1;

                // Line 4: Inventory restoration & COGS reversal (if configured)
                if total_cogs > Decimal::ZERO {
                    if let Some(mapping) =
                        InventoryAccountingService::get_mapping_tx(conn, company_id).await?
                    {
                        if mapping.enabled {
                            voucher_lines.push(VoucherLineInput {
                                id: None,
                                ledger_id: mapping.inventory_asset_ledger_id.clone(),
                                entry_type: "Dr".to_string(),
                                amount: total_cogs.normalize().to_string(),
                                narration: Some(
                                    "Restoration of inventory on sales return".to_string(),
                                ),
                            });
                            line_num += 1;

                            voucher_lines.push(VoucherLineInput {
                                id: None,
                                ledger_id: mapping.cogs_ledger_id.clone(),
                                entry_type: "Cr".to_string(),
                                amount: total_cogs.normalize().to_string(),
                                narration: Some("Reversal of COGS on sales return".to_string()),
                            });
                            let _ = line_num;
                        }
                    }
                }
            }
            CommercialDocumentType::DebitNote => {
                // Line 1: Debit Party (Creditor/Supplier) with Grand Total (Taxable + GST)
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: party_ledger_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: invoice_grand_total.normalize().to_string(),
                    narration: Some(format!("Debit note to {}", party_ledger_id)),
                });
                line_num += 1;

                // Line 2: Credit Stock-in-Hand / Purchase Return with Net Taxable Base
                voucher_lines.push(VoucherLineInput {
                    id: None,
                    ledger_id: commercial_ledger_id.clone(),
                    entry_type: "Cr".to_string(),
                    amount: input.total_amount.normalize().to_string(),
                    narration: Some("Purchase return".to_string()),
                });
                line_num += 1;

                // Line 3: Credit Input Duties & Taxes Ledgers (reversing input tax credit)
                for tax_line in &computed_tax_lines {
                    voucher_lines.push(VoucherLineInput {
                        id: None,
                        ledger_id: tax_line.tax_ledger_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: tax_line.amount.normalize().to_string(),
                        narration: Some(format!("Input {} reversal", tax_line.tax_type)),
                    });
                    line_num += 1;
                }
                let _ = line_num;
            }
        }

        // Validate Pure Double-Entry Equilibrium
        let val_res = DoubleEntryValidator::validate(
            &voucher_lines
                .iter()
                .enumerate()
                .map(|(idx, l)| crate::vouchers::model::VoucherLine {
                    id: Uuid::new_v4().to_string(),
                    company_id: company_id.to_string(),
                    voucher_id: voucher_id.clone(),
                    line_number: (idx + 1) as i64,
                    ledger_id: l.ledger_id.clone(),
                    entry_type: l.entry_type.clone(),
                    amount: l.amount.clone(),
                    narration: l.narration.clone(),
                    created_at: String::new(),
                })
                .collect::<Vec<_>>(),
            precision as u32,
            ValidationMode::Posting,
        );

        if !val_res.is_valid || !val_res.is_balanced {
            return Err(OpenTallyError::Validation(format!(
                "Commercial voucher out of double-entry balance: {:?}",
                val_res.diagnostics
            )));
        }

        let total_voucher_amount = val_res
            .single_side_total
            .ok_or_else(|| OpenTallyError::Validation("Missing single-side total".to_string()))?
            .to_string();

        // Step 8: Insert Accounting Header (in Draft) and Lines, then transition to Posted
        if input.source_voucher_id.is_none() {
            let provisional_voucher_number = format!("DRAFT-{}", &voucher_id[..8]);
            sqlx::query(
                "INSERT INTO vouchers (
                    id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount
                 ) VALUES (?, ?, ?, ?, 'automatic', NULL, NULL, ?, ?, ?, 'draft', ?, ?, ?, NULL)",
            )
            .bind(&voucher_id)
            .bind(company_id)
            .bind(voucher_type_id)
            .bind(&input.series_id)
            .bind(&provisional_voucher_number)
            .bind(&input.date)
            .bind(&input.date)
            .bind(&input.document_reference)
            .bind(&input.date)
            .bind(&input.narration)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        } else {
            sqlx::query("DELETE FROM voucher_lines WHERE company_id = ? AND voucher_id = ?")
                .bind(company_id)
                .bind(&voucher_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
        }

        let mut party_line_id = String::new();
        for (idx, line) in voucher_lines.iter().enumerate() {
            let line_id = Uuid::new_v4().to_string();
            if line.ledger_id == party_ledger_id {
                party_line_id = line_id.clone();
            }
            sqlx::query(
                "INSERT INTO voucher_lines (
                    id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind((idx + 1) as i64)
            .bind(&line.ledger_id)
            .bind(&line.entry_type)
            .bind(&line.amount)
            .bind(&line.narration)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Now transition voucher to 'posted'
        sqlx::query(
            "UPDATE vouchers
             SET series_id = ?,
                 numbering_method = 'automatic',
                 voucher_number = ?,
                 period_start = ?,
                 sequence_number = ?,
                 date = ?,
                 effective_date = ?,
                 reference_no = ?,
                 reference_date = ?,
                 narration = ?,
                 total_amount = ?,
                 status = 'posted',
                 updated_at = CURRENT_TIMESTAMP
             WHERE company_id = ? AND id = ?",
        )
        .bind(&input.series_id)
        .bind(&seq_alloc.voucher_number)
        .bind(&seq_alloc.period_start)
        .bind(seq_alloc.sequence_number)
        .bind(&input.date)
        .bind(&input.date)
        .bind(&input.document_reference)
        .bind(&input.date)
        .bind(&input.narration)
        .bind(&total_voucher_amount)
        .bind(company_id)
        .bind(&voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Step 8b: Process Bill-Wise Allocations (Milestone 2)
        let party_entry_type = match input.document_type {
            CommercialDocumentType::Sales | CommercialDocumentType::DebitNote => "Dr",
            CommercialDocumentType::Purchase | CommercialDocumentType::CreditNote => "Cr",
        };

        let party_bill_total = if input.document_type == CommercialDocumentType::Purchase {
            invoice_grand_total - tds_amount_decimal
        } else {
            invoice_grand_total
        };

        if !input.bill_allocations.is_empty() {
            let bill_inputs: Vec<crate::billwise::BillAllocationInput> = input
                .bill_allocations
                .iter()
                .map(|ba| crate::billwise::BillAllocationInput {
                    bill_name: ba.bill_name.clone(),
                    allocation_type: match ba.bill_type.as_str() {
                        "New Ref" | "NewRef" => crate::billwise::AllocationType::NewRef,
                        "Agst Ref" | "AgstRef" => crate::billwise::AllocationType::AgstRef,
                        "Advance" => crate::billwise::AllocationType::Advance,
                        "On Account" | "OnAccount" => crate::billwise::AllocationType::OnAccount,
                        _ => crate::billwise::AllocationType::NewRef,
                    },
                    amount: ba.amount.normalize().to_string(),
                    dr_cr: Some(party_entry_type.to_string()),
                    due_date: ba.due_date.clone().or_else(|| input.due_date.clone()),
                })
                .collect();

            crate::billwise::BillWiseService::validate_and_post_allocations(
                conn,
                company_id,
                &voucher_id,
                &party_line_id,
                &party_ledger_id,
                party_entry_type,
                party_bill_total,
                &input.date,
                input.due_date.as_deref(),
                &bill_inputs,
            )
            .await?;
        } else if crate::billwise::BillWiseService::is_billwise_enabled(conn, company_id).await? {
            let (alloc_type, bill_name) = if let Some((ref o_id, ref o_num, _)) = orig_voucher_info
            {
                let orig_bill_name: Option<String> = sqlx::query_scalar(
                    "SELECT bill_name FROM bill_allocations WHERE company_id = ? AND voucher_id = ? AND allocation_type IN ('NewRef', 'New Ref') LIMIT 1",
                )
                .bind(company_id)
                .bind(o_id)
                .fetch_optional(&mut *conn)
                .await
                .unwrap_or(None);

                let target_bill = orig_bill_name.unwrap_or_else(|| o_num.clone());
                (crate::billwise::AllocationType::AgstRef, target_bill)
            } else {
                let default_name = input
                    .document_reference
                    .as_ref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(&seq_alloc.voucher_number);
                let default_type = match input.document_type {
                    CommercialDocumentType::CreditNote | CommercialDocumentType::DebitNote => {
                        crate::billwise::AllocationType::OnAccount
                    }
                    _ => crate::billwise::AllocationType::NewRef,
                };
                (default_type, default_name.clone())
            };

            let auto_alloc = vec![crate::billwise::BillAllocationInput {
                bill_name,
                allocation_type: alloc_type,
                amount: party_bill_total.normalize().to_string(),
                dr_cr: Some(party_entry_type.to_string()),
                due_date: input.due_date.clone(),
            }];

            crate::billwise::BillWiseService::validate_and_post_allocations(
                conn,
                company_id,
                &voucher_id,
                &party_line_id,
                &party_ledger_id,
                party_entry_type,
                party_bill_total,
                &input.date,
                input.due_date.as_deref(),
                &auto_alloc,
            )
            .await?;
        }

        // Step 8c: Record Voucher GST Details (Milestone 3)
        let to_canonical_gst = |d: Decimal| -> String {
            let mut s = d.round_dp(4).to_string();
            if !s.contains('.') {
                s.push_str(".00");
            } else if let Some(idx) = s.find('.') {
                let dec_len = s.len() - idx - 1;
                if dec_len < 2 {
                    s.push('0');
                }
            }
            s
        };

        for rec in gst_detail_records {
            let detail_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO voucher_gst_details (
                    id, company_id, voucher_id, party_ledger_id, party_gstin, hsn_sac_code,
                    place_of_supply_state_code, supply_type, taxability, taxable_value, gst_rate,
                    cgst_amount, sgst_amount, igst_amount, cess_amount, is_reverse_charge, is_cancelled
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
            )
            .bind(&detail_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind(&party_ledger_id)
            .bind(&rec.1)
            .bind(&rec.2)
            .bind(&rec.3)
            .bind(&rec.4)
            .bind(&rec.5)
            .bind(to_canonical_gst(rec.6))
            .bind(to_canonical_gst(rec.7))
            .bind(to_canonical_gst(rec.8))
            .bind(to_canonical_gst(rec.9))
            .bind(to_canonical_gst(rec.10))
            .bind(to_canonical_gst(rec.11))
            .bind(rec.12)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Step 8d: Record Voucher TDS Details (Milestone 9)
        if let Some(ref calc) = tds_calc_opt {
            crate::tds::TdsService::record_voucher_tds_details_tx(
                conn,
                company_id,
                &voucher_id,
                &party_ledger_id,
                calc,
            )
            .await?;
        }

        // Step 9: Link and Lock Source Commercial Document (if backed by a commercial draft document)
        if input.source_voucher_id.is_some() {
            match input.document_type {
                CommercialDocumentType::Sales => {
                    let updated = sqlx::query(
                        "UPDATE sales_documents
                         SET status = 'posted', voucher_id = ?, updated_at = CURRENT_TIMESTAMP
                         WHERE company_id = ? AND id = ? AND status = 'draft'",
                    )
                    .bind(&voucher_id)
                    .bind(company_id)
                    .bind(&input.source_document_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?
                    .rows_affected();

                    if updated == 0 {
                        return Err(OpenTallyError::Validation(format!(
                            "Sales document '{}' is not in draft status or already posted",
                            input.source_document_id
                        )));
                    }
                }
                CommercialDocumentType::Purchase => {
                    let updated = sqlx::query(
                        "UPDATE purchase_documents
                         SET status = 'posted', voucher_id = ?, updated_at = CURRENT_TIMESTAMP
                         WHERE company_id = ? AND id = ? AND status = 'draft'",
                    )
                    .bind(&voucher_id)
                    .bind(company_id)
                    .bind(&input.source_document_id)
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?
                    .rows_affected();

                    if updated == 0 {
                        return Err(OpenTallyError::Validation(format!(
                            "Purchase document '{}' is not in draft status or already posted",
                            input.source_document_id
                        )));
                    }
                }
                CommercialDocumentType::CreditNote | CommercialDocumentType::DebitNote => {}
            }
        }

        // Record commercial note linkage if linked to an original invoice
        if let Some((ref o_id, ref o_num, ref o_date)) = orig_voucher_info {
            let link_id = Uuid::new_v4().to_string();
            let note_type_str = match input.document_type {
                CommercialDocumentType::CreditNote => "credit_note",
                CommercialDocumentType::DebitNote => "debit_note",
                _ => unreachable!(),
            };
            sqlx::query(
                "INSERT INTO commercial_note_linkages (
                    id, company_id, note_voucher_id, original_voucher_id,
                    note_type, original_invoice_number, original_invoice_date, reason
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&link_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind(o_id)
            .bind(note_type_str)
            .bind(o_num)
            .bind(o_date)
            .bind(&input.return_reason)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Step 9b: Auto-attach bank allocations if commercial voucher touches bank ledger (Milestone 8)
        let posted_vlines = sqlx::query_as::<_, crate::vouchers::model::VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at FROM voucher_lines WHERE company_id = ? AND voucher_id = ?"
        )
        .bind(company_id)
        .bind(&voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        crate::banking::BankingService::auto_create_or_attach_bank_allocations(
            conn,
            company_id,
            &voucher_id,
            &input.date,
            &posted_vlines,
            None,
        )
        .await?;

        // Step 10: Emit MCA Audit Log
        let audit_id = Uuid::new_v4().to_string();
        let audit_payload = serde_json::json!({
            "action": "COMMERCIAL_POST",
            "document_type": input.document_type.as_str(),
            "source_document_id": input.source_document_id,
            "voucher_id": voucher_id,
            "voucher_number": seq_alloc.voucher_number,
            "stock_voucher_id": stock_voucher_id_opt,
            "total_amount": input.total_amount.to_string(),
            "total_cogs": total_cogs.to_string(),
        })
        .to_string();

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, action, new_values, performed_by
             ) VALUES (?, ?, 'commercial_document', ?, 'POST', ?, 'system')",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(&input.source_document_id)
        .bind(&audit_payload)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let posted_voucher = sqlx::query_as::<_, crate::vouchers::model::Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&voucher_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let posted_lines = sqlx::query_as::<_, crate::vouchers::model::VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines WHERE company_id = ? AND voucher_id = ?
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(&voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let val_res = crate::vouchers::service::DoubleEntryValidator::validate(
            &posted_lines,
            4,
            crate::vouchers::model::ValidationMode::Draft,
        );

        Ok(CommercialPostingResult {
            voucher: VoucherWithLines {
                voucher: posted_voucher,
                lines: posted_lines,
                debit_total: val_res.debit_total.to_string(),
                credit_total: val_res.credit_total.to_string(),
                difference: val_res.difference.to_string(),
                is_balanced: val_res.is_balanced,
            },
            stock_voucher_id: stock_voucher_id_opt,
            cogs_amount: if total_cogs > Decimal::ZERO {
                Some(total_cogs)
            } else {
                None
            },
            cost_layer_ids: created_layer_ids,
            consumed_layer_ids,
        })
    }

    /// Verifies whether an F9 Purchase voucher can be safely cancelled.
    /// Rejects with Validation error if any FIFO cost layer created by this purchase
    /// has already been partially or fully consumed by a subsequent sale.
    pub async fn verify_and_cancel_purchase_fifo(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_number: &str,
    ) -> Result<(), OpenTallyError> {
        // If inventory subsystem is not installed in this schema (e.g. pure Sprint 2 accounting fixture), skip
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'inventory_cost_layers'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        // Find all stock vouchers and cost layers tied to this purchase
        let layers: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, original_quantity, remaining_quantity
             FROM inventory_cost_layers
             WHERE company_id = ? AND source_voucher_id IN (
                 SELECT id FROM stock_vouchers WHERE company_id = ? AND (reference_no = ? OR reference_no = ? OR id = ?)
             )",
        )
        .bind(company_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_number)
        .bind(voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        for (layer_id, orig_str, rem_str) in layers {
            let orig = Decimal::from_str_exact(&orig_str).map_err(|_| {
                OpenTallyError::IntegrityViolation(format!("Corrupt layer quantity: {}", orig_str))
            })?;
            let rem = Decimal::from_str_exact(&rem_str).map_err(|_| {
                OpenTallyError::IntegrityViolation(format!("Corrupt layer quantity: {}", rem_str))
            })?;

            if rem < orig {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot cancel purchase voucher '{}': FIFO cost layer '{}' has been partially or fully consumed by subsequent sales",
                    voucher_number, layer_id
                )));
            }

            // Also check consumption ledger directly
            let has_consumption: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM inventory_valuation_consumptions WHERE company_id = ? AND layer_id = ? LIMIT 1",
            )
            .bind(company_id)
            .bind(&layer_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if has_consumption.is_some() {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot cancel purchase voucher '{}': FIFO cost layer '{}' has recorded consumptions",
                    voucher_number, layer_id
                )));
            }
        }

        // If completely unconsumed: zero remaining quantity on layers and mark stock voucher cancelled
        sqlx::query(
            "UPDATE inventory_cost_layers
             SET remaining_quantity = '0'
             WHERE company_id = ? AND source_voucher_id IN (
                 SELECT id FROM stock_vouchers WHERE company_id = ? AND (reference_no = ? OR reference_no = ? OR id = ?)
             )",
        )
        .bind(company_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_number)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "UPDATE stock_vouchers
             SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
             WHERE company_id = ? AND (reference_no = ? OR reference_no = ? OR id = ?)",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_number)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let pur_docs_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'purchase_documents'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if pur_docs_exists.is_some() {
            sqlx::query(
                "UPDATE purchase_documents
                 SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
                 WHERE company_id = ? AND voucher_id = ?",
            )
            .bind(company_id)
            .bind(voucher_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    /// Safely cancels an F8 Sales voucher by restoring consumed quantities back to their FIFO layers.
    pub async fn restore_sales_fifo_on_cancel(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_number: &str,
    ) -> Result<(), OpenTallyError> {
        // If inventory valuation subsystem is not installed in this schema, skip
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'inventory_valuation_consumptions'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        let consumptions: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT c.id, c.layer_id, c.quantity
             FROM inventory_valuation_consumptions c
             JOIN stock_movements m ON m.company_id = c.company_id AND m.id = c.movement_id
             WHERE m.company_id = ? AND m.voucher_id IN (
                 SELECT id FROM stock_vouchers WHERE company_id = ? AND (reference_no = ? OR reference_no = ? OR id = ?)
             )",
        )
        .bind(company_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_number)
        .bind(voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        for (c_id, layer_id, qty_str) in consumptions {
            let restore_qty = Decimal::from_str_exact(&qty_str).map_err(|_| {
                OpenTallyError::IntegrityViolation(format!(
                    "Corrupt consumption quantity: {}",
                    qty_str
                ))
            })?;

            let current_rem_str: Option<String> = sqlx::query_scalar(
                "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&layer_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if let Some(curr_str) = current_rem_str {
                let current_rem = Decimal::from_str_exact(&curr_str).map_err(|_| {
                    OpenTallyError::IntegrityViolation(format!(
                        "Corrupt layer remaining: {}",
                        curr_str
                    ))
                })?;
                let updated_rem = current_rem + restore_qty;

                sqlx::query(
                    "UPDATE inventory_cost_layers
                     SET remaining_quantity = ?
                     WHERE company_id = ? AND id = ?",
                )
                .bind(updated_rem.normalize().to_string())
                .bind(company_id)
                .bind(&layer_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }

            sqlx::query(
                "DELETE FROM inventory_valuation_consumptions WHERE company_id = ? AND id = ?",
            )
            .bind(company_id)
            .bind(&c_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        sqlx::query(
            "UPDATE stock_vouchers
             SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
             WHERE company_id = ? AND (reference_no = ? OR reference_no = ? OR id = ?)",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_number)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let sales_docs_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sales_documents'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if sales_docs_exists.is_some() {
            sqlx::query(
                "UPDATE sales_documents
                 SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
                 WHERE company_id = ? AND voucher_id = ?",
            )
            .bind(company_id)
            .bind(voucher_id)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }

    async fn resolve_default_commercial_ledger(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        default_group_name: &str,
    ) -> Result<String, OpenTallyError> {
        let resolved: Option<String> = sqlx::query_scalar(
            "SELECT l.id FROM ledgers l
             JOIN account_groups g ON g.company_id = l.company_id AND g.id = l.group_id
             WHERE l.company_id = ? AND g.name = ?
             ORDER BY l.created_at ASC LIMIT 1",
        )
        .bind(company_id)
        .bind(default_group_name)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        resolved.ok_or_else(|| {
            OpenTallyError::Validation(format!(
                "No default ledger found under '{}'; explicit commercial ledger required",
                default_group_name
            ))
        })
    }
}
