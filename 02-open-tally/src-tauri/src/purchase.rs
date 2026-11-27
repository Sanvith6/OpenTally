use crate::commercial::{
    CommercialDocumentType, CommercialLinePosting, CommercialPostingInput, CommercialPostingResult,
    CommercialPostingService,
};
use crate::errors::OpenTallyError;
use crate::vouchers::{CreateVoucherDraftRequest, UpdateVoucherDraftRequest, VoucherDraftService};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseDiscountKind {
    None,
    Amount,
    Percentage,
}

impl PurchaseDiscountKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Amount => "amount",
            Self::Percentage => "percentage",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseLineRequest {
    pub description: String,
    pub stock_item_id: Option<String>,
    pub unit_id: Option<String>,
    pub godown_id: Option<String>,
    pub quantity: String,
    #[serde(default)]
    pub billed_quantity: Option<String>,
    pub rate: String,
    pub discount_kind: PurchaseDiscountKind,
    pub discount_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseDocumentRequest {
    pub series_id: String,
    pub party_id: String,
    pub bill_date: String,
    pub supplier_invoice_no: Option<String>,
    pub due_date_override: Option<String>,
    pub due_date_override_reason: Option<String>,
    pub narration: Option<String>,
    pub document_discount_amount: String,
    pub additional_charges: String,
    pub lines: Vec<PurchaseLineRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct PurchaseDocument {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub party_id: String,
    pub bill_date: String,
    pub supplier_invoice_no: Option<String>,
    pub due_date: String,
    pub due_date_source: String,
    pub due_date_override_reason: Option<String>,
    pub narration: Option<String>,
    pub status: String,
    pub subtotal: String,
    pub discount_total: String,
    pub additional_charges: String,
    pub net_total: String,
    pub tax_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct PurchaseDocumentLine {
    pub id: String,
    pub company_id: String,
    pub purchase_document_id: String,
    pub line_number: i64,
    pub description: String,
    pub stock_item_id: Option<String>,
    pub unit_id: Option<String>,
    pub godown_id: Option<String>,
    pub quantity: String,
    pub billed_quantity: Option<String>,
    pub rate: String,
    pub discount_kind: String,
    pub discount_value: String,
    pub discount_amount: String,
    pub line_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PurchaseDocumentWithLines {
    pub document: PurchaseDocument,
    pub voucher_id: String,
    pub voucher_number: String,
    pub lines: Vec<PurchaseDocumentLine>,
}

pub struct PurchaseDocumentService;

impl PurchaseDocumentService {
    pub async fn create_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: PurchaseDocumentRequest,
    ) -> Result<PurchaseDocumentWithLines, OpenTallyError> {
        let prepared = prepare(pool, company_id, request).await?;
        let voucher = VoucherDraftService::create_draft(
            pool,
            company_id,
            CreateVoucherDraftRequest {
                voucher_type_id: "vt_pur".to_owned(),
                series_id: prepared.series_id.clone(),
                date: prepared.bill_date.clone(),
                effective_date: None,
                reference_no: prepared.supplier_invoice_no.clone(),
                reference_date: None,
                narration: prepared.narration.clone(),
                lines: None,
            },
        )
        .await?;

        let document_id = Uuid::new_v4().to_string();
        let result = insert_purchase(
            pool,
            company_id,
            &document_id,
            &voucher.voucher.id,
            prepared,
        )
        .await;

        if result.is_err() {
            let _ = VoucherDraftService::delete_draft(pool, company_id, &voucher.voucher.id).await;
        }

        result.map(|document| PurchaseDocumentWithLines {
            voucher_id: voucher.voucher.id,
            voucher_number: voucher.voucher.voucher_number,
            lines: document.1,
            document: document.0,
        })
    }

    pub async fn get_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
    ) -> Result<PurchaseDocumentWithLines, OpenTallyError> {
        let document = select_document(pool, company_id, id).await?;
        let voucher_number: String = sqlx::query_scalar(
            "SELECT voucher_number FROM vouchers WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&document.voucher_id)
        .fetch_one(pool)
        .await?;

        let lines = select_lines(pool, company_id, id).await?;
        Ok(PurchaseDocumentWithLines {
            voucher_id: document.voucher_id.clone(),
            voucher_number,
            document,
            lines,
        })
    }

    pub async fn list_drafts(
        pool: &Pool<Sqlite>,
        company_id: &str,
        search: Option<&str>,
    ) -> Result<Vec<PurchaseDocument>, OpenTallyError> {
        let rows = if let Some(search) = search {
            sqlx::query_as::<_, PurchaseDocument>(
                "SELECT id, company_id, voucher_id, party_id, bill_date, supplier_invoice_no,
                        due_date, due_date_source, due_date_override_reason, narration, status,
                        subtotal, discount_total, additional_charges, net_total, tax_status,
                        created_at, updated_at FROM purchase_documents
                 WHERE company_id = ?
                   AND (supplier_invoice_no LIKE '%' || ? || '%' COLLATE NOCASE OR narration LIKE '%' || ? || '%' COLLATE NOCASE)
                 ORDER BY bill_date DESC, updated_at DESC, id",
            )
            .bind(company_id)
            .bind(search)
            .bind(search)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, PurchaseDocument>(
                "SELECT id, company_id, voucher_id, party_id, bill_date, supplier_invoice_no,
                        due_date, due_date_source, due_date_override_reason, narration, status,
                        subtotal, discount_total, additional_charges, net_total, tax_status,
                        created_at, updated_at FROM purchase_documents
                 WHERE company_id = ?
                 ORDER BY bill_date DESC, updated_at DESC, id",
            )
            .bind(company_id)
            .fetch_all(pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn update_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
        request: PurchaseDocumentRequest,
    ) -> Result<PurchaseDocumentWithLines, OpenTallyError> {
        let current = select_document(pool, company_id, id).await?;
        let prepared = prepare(pool, company_id, request).await?;

        if current.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot update purchase document in status '{}'",
                current.status
            )));
        }

        VoucherDraftService::update_draft_header(
            pool,
            company_id,
            &current.voucher_id,
            UpdateVoucherDraftRequest {
                voucher_type_id: Some("vt_pur".to_owned()),
                series_id: Some(prepared.series_id.clone()),
                date: Some(prepared.bill_date.clone()),
                effective_date: None,
                reference_no: prepared.supplier_invoice_no.clone(),
                reference_date: None,
                narration: prepared.narration.clone(),
            },
        )
        .await?;

        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE purchase_documents SET party_id=?, bill_date=?, supplier_invoice_no=?, due_date=?,
             due_date_source=?, due_date_override_reason=?, narration=?, subtotal=?, discount_total=?,
             additional_charges=?, net_total=?, updated_at=CURRENT_TIMESTAMP
             WHERE company_id=? AND id=?",
        )
        .bind(&prepared.party_id)
        .bind(&prepared.bill_date)
        .bind(&prepared.supplier_invoice_no)
        .bind(&prepared.due_date)
        .bind(&prepared.due_date_source)
        .bind(&prepared.due_date_override_reason)
        .bind(&prepared.narration)
        .bind(prepared.subtotal.normalize().to_string())
        .bind(prepared.discount_total.normalize().to_string())
        .bind(prepared.additional_charges.normalize().to_string())
        .bind(prepared.net_total.normalize().to_string())
        .bind(company_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM purchase_document_lines WHERE company_id=? AND purchase_document_id=?",
        )
        .bind(company_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        insert_lines(&mut tx, company_id, id, prepared.lines).await?;
        audit(&mut tx, company_id, id, "UPDATE", Some(&current), None).await?;
        tx.commit().await?;

        Self::get_draft(pool, company_id, id).await
    }

    pub async fn delete_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
    ) -> Result<(), OpenTallyError> {
        let document = select_document(pool, company_id, id).await?;
        if document.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete purchase document in status '{}'",
                document.status
            )));
        }

        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM purchase_documents WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit(&mut tx, company_id, id, "DELETE", Some(&document), None).await?;
        tx.commit().await?;

        VoucherDraftService::delete_draft(pool, company_id, &document.voucher_id).await
    }

    /// Atomically posts a Purchase Document to both GL and FIFO inventory cost layers.
    pub async fn post_purchase_document(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
        purchase_ledger_id: Option<String>,
    ) -> Result<CommercialPostingResult, OpenTallyError> {
        Self::post_purchase_document_with_tds(pool, company_id, id, purchase_ledger_id, None).await
    }

    /// Atomically posts a Purchase Document with optional TDS section deduction.
    pub async fn post_purchase_document_with_tds(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
        purchase_ledger_id: Option<String>,
        tds_section_code: Option<String>,
    ) -> Result<CommercialPostingResult, OpenTallyError> {
        let doc_with_lines = Self::get_draft(pool, company_id, id).await?;
        let doc = doc_with_lines.document;

        if doc.status == "posted" {
            return Err(OpenTallyError::Validation(
                "Purchase document is already posted".to_string(),
            ));
        }
        if doc.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot post purchase document in status '{}'",
                doc.status
            )));
        }

        // Resolve supplier party ledger ID
        let party_ledger_id: String = sqlx::query_scalar(
            "SELECT ledger_id FROM party_profiles WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(&doc.party_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let series_id: String =
            sqlx::query_scalar("SELECT series_id FROM vouchers WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(&doc.voucher_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        let posting_lines: Vec<CommercialLinePosting> = doc_with_lines
            .lines
            .into_iter()
            .map(|l| {
                let qty = Decimal::from_str_exact(&l.quantity).map_err(|_| {
                    OpenTallyError::IntegrityViolation(format!("Invalid quantity: {}", l.quantity))
                })?;
                let rate = Decimal::from_str_exact(&l.rate).map_err(|_| {
                    OpenTallyError::IntegrityViolation(format!("Invalid rate: {}", l.rate))
                })?;
                let discount_amount =
                    Decimal::from_str_exact(&l.discount_amount).map_err(|_| {
                        OpenTallyError::IntegrityViolation(format!(
                            "Invalid discount: {}",
                            l.discount_amount
                        ))
                    })?;
                let line_amount = Decimal::from_str_exact(&l.line_amount).map_err(|_| {
                    OpenTallyError::IntegrityViolation(format!(
                        "Invalid line amount: {}",
                        l.line_amount
                    ))
                })?;
                Ok(CommercialLinePosting {
                    line_number: l.line_number,
                    description: l.description,
                    stock_item_id: l.stock_item_id,
                    unit_id: l.unit_id,
                    godown_id: l.godown_id,
                    quantity: qty,
                    billed_quantity: l
                        .billed_quantity
                        .as_deref()
                        .map(Decimal::from_str_exact)
                        .transpose()
                        .map_err(|_| {
                            OpenTallyError::IntegrityViolation("Invalid billed quantity".to_owned())
                        })?,
                    rate,
                    discount_amount,
                    line_amount,
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                    entered_unit_id: None,
                })
            })
            .collect::<Result<Vec<_>, OpenTallyError>>()?;

        let net_total = Decimal::from_str_exact(&doc.net_total).map_err(|_| {
            OpenTallyError::IntegrityViolation(format!("Invalid net_total: {}", doc.net_total))
        })?;

        let input = CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: CommercialDocumentType::Purchase,
            source_document_id: doc.id.clone(),
            source_voucher_id: Some(doc.voucher_id.clone()),
            series_id,
            date: doc.bill_date.clone(),
            due_date: Some(doc.due_date.clone()),
            party_id: doc.party_id.clone(),
            party_ledger_id,
            commercial_account_ledger_id: purchase_ledger_id,
            total_amount: net_total,
            document_reference: doc.supplier_invoice_no.clone(),
            narration: doc.narration.clone(),
            lines: posting_lines,
            tax_lines: Vec::new(),
            bill_allocations: Vec::new(),
            place_of_supply_state_code: None,
            is_reverse_charge: None,
            allow_tax_override: None,
            original_voucher_id: None,
            return_reason: None,
            tds_section_code,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        };

        CommercialPostingService::post_commercial_voucher(pool, company_id, input).await
    }
}

struct Prepared {
    series_id: String,
    party_id: String,
    bill_date: String,
    supplier_invoice_no: Option<String>,
    due_date: String,
    due_date_source: String,
    due_date_override_reason: Option<String>,
    narration: Option<String>,
    subtotal: Decimal,
    discount_total: Decimal,
    additional_charges: Decimal,
    net_total: Decimal,
    lines: Vec<PreparedLine>,
}

struct PreparedLine {
    request: PurchaseLineRequest,
    quantity: Decimal,
    billed_quantity: Option<Decimal>,
    rate: Decimal,
    discount_value: Decimal,
    discount_amount: Decimal,
    line_amount: Decimal,
}

async fn prepare(
    pool: &SqlitePool,
    company_id: &str,
    request: PurchaseDocumentRequest,
) -> Result<Prepared, OpenTallyError> {
    let party: Option<(bool, bool, i64)> = sqlx::query_as(
        "SELECT is_active, supplier_enabled, credit_period_days FROM party_profiles WHERE company_id = ? AND id = ?",
    )
    .bind(company_id)
    .bind(&request.party_id)
    .fetch_optional(pool)
    .await?;

    let (active, supplier, credit_days) =
        party.ok_or_else(|| OpenTallyError::NotFound("Supplier party not found".to_owned()))?;
    if !active || !supplier {
        return Err(OpenTallyError::Validation(
            "Purchase supplier must be an active supplier party".to_owned(),
        ));
    }

    let bill_date = date(&request.bill_date)?;
    let (due_date, due_date_source) =
        if let Some(override_date) = request.due_date_override.as_deref() {
            if request
                .due_date_override_reason
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(OpenTallyError::Validation(
                    "Due-date override requires a reason".to_owned(),
                ));
            }
            (
                date(override_date)?.format("%Y-%m-%d").to_string(),
                "explicit_override".to_owned(),
            )
        } else {
            (
                (bill_date + Duration::days(credit_days))
                    .format("%Y-%m-%d")
                    .to_string(),
                "party_default".to_owned(),
            )
        };

    if request.lines.is_empty() {
        return Err(OpenTallyError::Validation(
            "A purchase document requires at least one line".to_owned(),
        ));
    }

    let mut subtotal = Decimal::ZERO;
    let mut discount_total = Decimal::ZERO;
    let mut lines = Vec::with_capacity(request.lines.len());

    for line in request.lines {
        let description = line.description.trim();
        if description.is_empty() {
            return Err(OpenTallyError::Validation(
                "Purchase line description is required".to_owned(),
            ));
        }
        let quantity = nonnegative(&line.quantity, "quantity")?;
        if quantity <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Purchase quantity must be greater than zero".to_owned(),
            ));
        }
        let rate = nonnegative(&line.rate, "rate")?;
        let billed_quantity = line
            .billed_quantity
            .as_deref()
            .map(|value| nonnegative(value, "billed quantity"))
            .transpose()?;
        if billed_quantity.is_some_and(|billed| billed > quantity) {
            return Err(OpenTallyError::Validation(
                "Billed quantity cannot exceed actual quantity".to_owned(),
            ));
        }
        let discount_value = nonnegative(&line.discount_value, "discount")?;

        if line.stock_item_id.is_some() && (line.unit_id.is_none() || line.godown_id.is_none()) {
            return Err(OpenTallyError::Validation(
                "Inventory-linked purchase lines require unit and godown".to_owned(),
            ));
        }
        if line.stock_item_id.is_none() && (line.unit_id.is_some() || line.godown_id.is_some()) {
            return Err(OpenTallyError::Validation(
                "Accounting-only lines cannot reference inventory locations".to_owned(),
            ));
        }

        validate_reference(pool, company_id, &line).await?;

        let gross = billed_quantity.unwrap_or(quantity) * rate;
        let discount_amount = match line.discount_kind {
            PurchaseDiscountKind::None => {
                if discount_value != Decimal::ZERO {
                    return Err(OpenTallyError::Validation(
                        "Discount value must be zero for no discount".to_owned(),
                    ));
                }
                Decimal::ZERO
            }
            PurchaseDiscountKind::Amount => {
                if discount_value > gross {
                    return Err(OpenTallyError::Validation(
                        "Discount cannot exceed line amount".to_owned(),
                    ));
                }
                discount_value
            }
            PurchaseDiscountKind::Percentage => {
                if discount_value > Decimal::new(100, 0) {
                    return Err(OpenTallyError::Validation(
                        "Discount percentage must be between 0 and 100".to_owned(),
                    ));
                }
                gross * discount_value / Decimal::new(100, 0)
            }
        };

        let line_amount = gross - discount_amount;
        subtotal += gross;
        discount_total += discount_amount;

        lines.push(PreparedLine {
            request: line,
            quantity,
            billed_quantity,
            rate,
            discount_value,
            discount_amount,
            line_amount,
        });
    }

    let document_discount = nonnegative(&request.document_discount_amount, "document discount")?;
    let additional_charges = nonnegative(&request.additional_charges, "additional charges")?;

    if document_discount > subtotal - discount_total {
        return Err(OpenTallyError::Validation(
            "Document discount cannot exceed discounted subtotal".to_owned(),
        ));
    }

    discount_total += document_discount;
    let net_total = subtotal - discount_total + additional_charges;

    Ok(Prepared {
        series_id: request.series_id,
        party_id: request.party_id,
        bill_date: bill_date.format("%Y-%m-%d").to_string(),
        supplier_invoice_no: request.supplier_invoice_no,
        due_date,
        due_date_source,
        due_date_override_reason: request.due_date_override_reason,
        narration: request.narration,
        subtotal,
        discount_total,
        additional_charges,
        net_total,
        lines,
    })
}

async fn validate_reference(
    pool: &SqlitePool,
    company_id: &str,
    line: &PurchaseLineRequest,
) -> Result<(), OpenTallyError> {
    if let Some(stock_item_id) = &line.stock_item_id {
        let active: Option<bool> =
            sqlx::query_scalar("SELECT is_active FROM stock_items WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(stock_item_id)
                .fetch_optional(pool)
                .await?;
        if active != Some(true) {
            return Err(OpenTallyError::Validation(
                "Stock item must be active in the company".to_owned(),
            ));
        }
    }
    if let Some(unit_id) = &line.unit_id {
        let active: Option<bool> =
            sqlx::query_scalar("SELECT is_active FROM units WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(unit_id)
                .fetch_optional(pool)
                .await?;
        if active != Some(true) {
            return Err(OpenTallyError::Validation(
                "Unit must be active in the company".to_owned(),
            ));
        }
    }
    if let Some(godown_id) = &line.godown_id {
        let active: Option<bool> =
            sqlx::query_scalar("SELECT is_active FROM godowns WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(godown_id)
                .fetch_optional(pool)
                .await?;
        if active != Some(true) {
            return Err(OpenTallyError::Validation(
                "Godown must be active in the company".to_owned(),
            ));
        }
    }
    Ok(())
}

async fn insert_purchase(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
    voucher_id: &str,
    p: Prepared,
) -> Result<(PurchaseDocument, Vec<PurchaseDocumentLine>), OpenTallyError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO purchase_documents (
            id, company_id, voucher_id, party_id, bill_date, supplier_invoice_no,
            due_date, due_date_source, due_date_override_reason, narration,
            subtotal, discount_total, additional_charges, net_total
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(company_id)
    .bind(voucher_id)
    .bind(&p.party_id)
    .bind(&p.bill_date)
    .bind(&p.supplier_invoice_no)
    .bind(&p.due_date)
    .bind(&p.due_date_source)
    .bind(&p.due_date_override_reason)
    .bind(&p.narration)
    .bind(p.subtotal.normalize().to_string())
    .bind(p.discount_total.normalize().to_string())
    .bind(p.additional_charges.normalize().to_string())
    .bind(p.net_total.normalize().to_string())
    .execute(&mut *tx)
    .await
    .map_err(map_constraint)?;

    insert_lines(&mut tx, company_id, id, p.lines).await?;
    audit(&mut tx, company_id, id, "CREATE", None, None).await?;

    let document = sqlx::query_as::<_, PurchaseDocument>(
        "SELECT id, company_id, voucher_id, party_id, bill_date, supplier_invoice_no,
                due_date, due_date_source, due_date_override_reason, narration, status,
                subtotal, discount_total, additional_charges, net_total, tax_status,
                created_at, updated_at FROM purchase_documents WHERE company_id=? AND id=?",
    )
    .bind(company_id)
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    let lines = sqlx::query_as::<_, PurchaseDocumentLine>(
        "SELECT id, company_id, purchase_document_id, line_number, description,
                stock_item_id, unit_id, godown_id, quantity, billed_quantity, rate, discount_kind,
                discount_value, discount_amount, line_amount, created_at
         FROM purchase_document_lines WHERE company_id=? AND purchase_document_id=?
         ORDER BY line_number",
    )
    .bind(company_id)
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok((document, lines))
}

async fn insert_lines(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    company_id: &str,
    document_id: &str,
    lines: Vec<PreparedLine>,
) -> Result<(), OpenTallyError> {
    for (index, line) in lines.into_iter().enumerate() {
        let line_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO purchase_document_lines (
                id, company_id, purchase_document_id, line_number, description,
                stock_item_id, unit_id, godown_id, quantity, billed_quantity, rate, discount_kind,
                discount_value, discount_amount, line_amount
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&line_id)
        .bind(company_id)
        .bind(document_id)
        .bind((index + 1) as i64)
        .bind(line.request.description.trim())
        .bind(&line.request.stock_item_id)
        .bind(&line.request.unit_id)
        .bind(&line.request.godown_id)
        .bind(line.quantity.normalize().to_string())
        .bind(line.billed_quantity.map(|v| v.normalize().to_string()))
        .bind(line.rate.normalize().to_string())
        .bind(line.request.discount_kind.as_str())
        .bind(line.discount_value.normalize().to_string())
        .bind(line.discount_amount.normalize().to_string())
        .bind(line.line_amount.normalize().to_string())
        .execute(&mut **tx)
        .await
        .map_err(map_constraint)?;
    }
    Ok(())
}

async fn select_document(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
) -> Result<PurchaseDocument, OpenTallyError> {
    sqlx::query_as::<_, PurchaseDocument>(
        "SELECT id, company_id, voucher_id, party_id, bill_date, supplier_invoice_no,
                due_date, due_date_source, due_date_override_reason, narration, status,
                subtotal, discount_total, additional_charges, net_total, tax_status,
                created_at, updated_at FROM purchase_documents WHERE company_id=? AND id=?",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| OpenTallyError::NotFound(format!("Purchase document '{}' not found", id)))
}

async fn select_lines(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
) -> Result<Vec<PurchaseDocumentLine>, OpenTallyError> {
    Ok(sqlx::query_as::<_, PurchaseDocumentLine>(
        "SELECT id, company_id, purchase_document_id, line_number, description,
                stock_item_id, unit_id, godown_id, quantity, billed_quantity, rate, discount_kind,
                discount_value, discount_amount, line_amount, created_at
         FROM purchase_document_lines WHERE company_id=? AND purchase_document_id=?
         ORDER BY line_number",
    )
    .bind(company_id)
    .bind(id)
    .fetch_all(pool)
    .await?)
}

fn date(value: &str) -> Result<NaiveDate, OpenTallyError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        OpenTallyError::Validation(format!("Invalid date '{}'; expected YYYY-MM-DD", value))
    })
}

fn nonnegative(value: &str, field: &str) -> Result<Decimal, OpenTallyError> {
    let value = Decimal::from_str_exact(value.trim())
        .map_err(|_| OpenTallyError::Validation(format!("{} must be an exact Decimal", field)))?;
    if value.is_sign_negative() {
        return Err(OpenTallyError::Validation(format!(
            "{} cannot be negative",
            field
        )));
    }
    Ok(value)
}

fn map_constraint(error: sqlx::Error) -> OpenTallyError {
    if let sqlx::Error::Database(db) = &error {
        if db.message().contains("UNIQUE") {
            return OpenTallyError::Validation(
                "Purchase document reference or linked record is already used".to_owned(),
            );
        }
    }
    OpenTallyError::Database(error)
}

async fn audit(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    company_id: &str,
    id: &str,
    action: &str,
    old: Option<&PurchaseDocument>,
    _new: Option<&PurchaseDocumentRequest>,
) -> Result<(), OpenTallyError> {
    let old_values = old.map(serde_json::to_string).transpose()?;
    sqlx::query(
        "INSERT INTO audit_log (
            id, company_id, entity_type, entity_id, action, old_values, performed_by
         ) VALUES (?, ?, 'purchase_document', ?, ?, ?, 'system')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(company_id)
    .bind(id)
    .bind(action)
    .bind(old_values)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
