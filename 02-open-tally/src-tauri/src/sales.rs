use crate::errors::OpenTallyError;
use crate::vouchers::{CreateVoucherDraftRequest, UpdateVoucherDraftRequest, VoucherDraftService};
use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscountKind {
    None,
    Amount,
    Percentage,
}

impl DiscountKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Amount => "amount",
            Self::Percentage => "percentage",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesLineRequest {
    pub description: String,
    pub stock_item_id: Option<String>,
    pub unit_id: Option<String>,
    pub godown_id: Option<String>,
    pub quantity: String,
    pub rate: String,
    pub discount_kind: DiscountKind,
    pub discount_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesDocumentRequest {
    pub series_id: String,
    pub party_id: String,
    pub invoice_date: String,
    pub document_reference: Option<String>,
    pub due_date_override: Option<String>,
    pub due_date_override_reason: Option<String>,
    pub narration: Option<String>,
    pub document_discount_amount: String,
    pub additional_charges: String,
    pub lines: Vec<SalesLineRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct SalesDocument {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub party_id: String,
    pub invoice_date: String,
    pub document_reference: Option<String>,
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
pub struct SalesDocumentLine {
    pub id: String,
    pub company_id: String,
    pub sales_document_id: String,
    pub line_number: i64,
    pub description: String,
    pub stock_item_id: Option<String>,
    pub unit_id: Option<String>,
    pub godown_id: Option<String>,
    pub quantity: String,
    pub rate: String,
    pub discount_kind: String,
    pub discount_value: String,
    pub discount_amount: String,
    pub line_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SalesDocumentWithLines {
    pub document: SalesDocument,
    pub voucher_id: String,
    pub voucher_number: String,
    pub lines: Vec<SalesDocumentLine>,
}

pub struct SalesDocumentService;

impl SalesDocumentService {
    pub async fn create_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: SalesDocumentRequest,
    ) -> Result<SalesDocumentWithLines, OpenTallyError> {
        let prepared = prepare(pool, company_id, request).await?;
        let voucher = VoucherDraftService::create_draft(
            pool,
            company_id,
            CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_owned(),
                series_id: prepared.series_id.clone(),
                date: prepared.invoice_date.clone(),
                effective_date: None,
                reference_no: prepared.document_reference.clone(),
                reference_date: None,
                narration: prepared.narration.clone(),
                lines: None,
            },
        )
        .await?;
        let document_id = Uuid::new_v4().to_string();
        let result = insert_sales(
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
        result.map(|document| SalesDocumentWithLines {
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
    ) -> Result<SalesDocumentWithLines, OpenTallyError> {
        let document = select_document(pool, company_id, id).await?;
        let voucher_number: String = sqlx::query_scalar(
            "SELECT voucher_number FROM vouchers WHERE company_id = ? AND id = ? AND status = 'draft'",
        )
        .bind(company_id)
        .bind(&document.voucher_id)
        .fetch_one(pool)
        .await?;
        let lines = select_lines(pool, company_id, id).await?;
        Ok(SalesDocumentWithLines {
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
    ) -> Result<Vec<SalesDocument>, OpenTallyError> {
        let rows = if let Some(search) = search {
            sqlx::query_as::<_, SalesDocument>(
                "SELECT id, company_id, voucher_id, party_id, invoice_date, document_reference,
                        due_date, due_date_source, due_date_override_reason, narration, status,
                        subtotal, discount_total, additional_charges, net_total, tax_status,
                        created_at, updated_at FROM sales_documents
                 WHERE company_id = ? AND status = 'draft'
                   AND (document_reference LIKE '%' || ? || '%' COLLATE NOCASE OR narration LIKE '%' || ? || '%' COLLATE NOCASE)
                 ORDER BY invoice_date DESC, updated_at DESC, id",
            )
            .bind(company_id).bind(search).bind(search).fetch_all(pool).await?
        } else {
            sqlx::query_as::<_, SalesDocument>(
                "SELECT id, company_id, voucher_id, party_id, invoice_date, document_reference,
                        due_date, due_date_source, due_date_override_reason, narration, status,
                        subtotal, discount_total, additional_charges, net_total, tax_status,
                        created_at, updated_at FROM sales_documents
                 WHERE company_id = ? AND status = 'draft'
                 ORDER BY invoice_date DESC, updated_at DESC, id",
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
        request: SalesDocumentRequest,
    ) -> Result<SalesDocumentWithLines, OpenTallyError> {
        let current = select_document(pool, company_id, id).await?;
        let prepared = prepare(pool, company_id, request).await?;
        VoucherDraftService::update_draft_header(
            pool,
            company_id,
            &current.voucher_id,
            UpdateVoucherDraftRequest {
                voucher_type_id: Some("vt_sales".to_owned()),
                series_id: Some(prepared.series_id.clone()),
                date: Some(prepared.invoice_date.clone()),
                effective_date: None,
                reference_no: prepared.document_reference.clone(),
                reference_date: None,
                narration: prepared.narration.clone(),
            },
        )
        .await?;
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE sales_documents SET party_id=?, invoice_date=?, document_reference=?, due_date=?,
             due_date_source=?, due_date_override_reason=?, narration=?, subtotal=?, discount_total=?,
             additional_charges=?, net_total=?, updated_at=CURRENT_TIMESTAMP
             WHERE company_id=? AND id=?",
        )
        .bind(&prepared.party_id)
        .bind(&prepared.invoice_date)
        .bind(&prepared.document_reference)
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
        sqlx::query("DELETE FROM sales_document_lines WHERE company_id=? AND sales_document_id=?")
            .bind(company_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        insert_lines(&mut tx, company_id, id, prepared.lines).await?;
        audit(&mut tx, company_id, id, "UPDATE", Some(&current), None).await?;
        tx.commit().await?;
        Self::get_draft(pool, company_id, id).await
    }

    pub async fn reorder_lines(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
        line_ids: Vec<String>,
    ) -> Result<SalesDocumentWithLines, OpenTallyError> {
        let current = Self::get_draft(pool, company_id, id).await?;
        if line_ids.len() != current.lines.len() {
            return Err(OpenTallyError::Validation(
                "Line reorder must include every line exactly once".to_owned(),
            ));
        }
        let existing: std::collections::HashSet<&str> =
            current.lines.iter().map(|line| line.id.as_str()).collect();
        if line_ids
            .iter()
            .any(|line_id| !existing.contains(line_id.as_str()))
            || line_ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != line_ids.len()
        {
            return Err(OpenTallyError::Validation(
                "Line reorder contains an invalid or duplicate line".to_owned(),
            ));
        }
        let mut tx = pool.begin().await?;
        for line in &line_ids {
            sqlx::query("UPDATE sales_document_lines SET line_number=line_number+1000000 WHERE company_id=? AND sales_document_id=? AND id=?")
                .bind(company_id).bind(id).bind(line).execute(&mut *tx).await?;
        }
        for (index, line) in line_ids.iter().enumerate() {
            sqlx::query("UPDATE sales_document_lines SET line_number=? WHERE company_id=? AND sales_document_id=? AND id=?")
                .bind((index + 1) as i64).bind(company_id).bind(id).bind(line).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Self::get_draft(pool, company_id, id).await
    }

    pub async fn delete_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
    ) -> Result<(), OpenTallyError> {
        let document = select_document(pool, company_id, id).await?;
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM sales_documents WHERE company_id = ? AND id = ?")
            .bind(company_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        audit(&mut tx, company_id, id, "DELETE", Some(&document), None).await?;
        tx.commit().await?;
        VoucherDraftService::delete_draft(pool, company_id, &document.voucher_id).await
    }

    pub async fn post_sales_document(
        pool: &Pool<Sqlite>,
        company_id: &str,
        id: &str,
        sales_ledger_id: Option<String>,
    ) -> Result<crate::commercial::CommercialPostingResult, OpenTallyError> {
        let doc_with_lines = Self::get_draft(pool, company_id, id).await?;
        let doc = doc_with_lines.document;

        if doc.status == "posted" {
            return Err(OpenTallyError::Validation(
                "Sales document is already posted".to_string(),
            ));
        }
        if doc.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot post sales document in status '{}'",
                doc.status
            )));
        }

        // Resolve customer party ledger ID
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

        let posting_lines: Vec<crate::commercial::CommercialLinePosting> = doc_with_lines
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
                Ok(crate::commercial::CommercialLinePosting {
                    line_number: l.line_number,
                    description: l.description,
                    stock_item_id: l.stock_item_id,
                    unit_id: l.unit_id,
                    godown_id: l.godown_id,
                    quantity: qty,
                    rate,
                    discount_amount,
                    line_amount,
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                })
            })
            .collect::<Result<Vec<_>, OpenTallyError>>()?;

        let net_total = Decimal::from_str_exact(&doc.net_total).map_err(|_| {
            OpenTallyError::IntegrityViolation(format!("Invalid net_total: {}", doc.net_total))
        })?;

        let input = crate::commercial::CommercialPostingInput {
            company_id: company_id.to_string(),
            document_type: crate::commercial::CommercialDocumentType::Sales,
            source_document_id: doc.id.clone(),
            source_voucher_id: Some(doc.voucher_id.clone()),
            series_id,
            date: doc.invoice_date.clone(),
            due_date: Some(doc.due_date.clone()),
            party_id: doc.party_id.clone(),
            party_ledger_id,
            commercial_account_ledger_id: sales_ledger_id,
            total_amount: net_total,
            document_reference: doc.document_reference.clone(),
            narration: doc.narration.clone(),
            lines: posting_lines,
            tax_lines: Vec::new(),
            bill_allocations: Vec::new(),
            place_of_supply_state_code: None,
            is_reverse_charge: None,
            allow_tax_override: None,
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
        };

        crate::commercial::CommercialPostingService::post_commercial_voucher(
            pool, company_id, input,
        )
        .await
    }
}

struct Prepared {
    series_id: String,
    party_id: String,
    invoice_date: String,
    document_reference: Option<String>,
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
    request: SalesLineRequest,
    quantity: Decimal,
    rate: Decimal,
    discount_value: Decimal,
    discount_amount: Decimal,
    line_amount: Decimal,
}

async fn prepare(
    pool: &SqlitePool,
    company_id: &str,
    request: SalesDocumentRequest,
) -> Result<Prepared, OpenTallyError> {
    let party: Option<(bool, bool, i64)> = sqlx::query_as(
        "SELECT is_active, customer_enabled, credit_period_days FROM party_profiles WHERE company_id = ? AND id = ?",
    ).bind(company_id).bind(&request.party_id).fetch_optional(pool).await?;
    let (active, customer, credit_days) =
        party.ok_or_else(|| OpenTallyError::NotFound("Customer party not found".to_owned()))?;
    if !active || !customer {
        return Err(OpenTallyError::Validation(
            "Sales customer must be an active customer party".to_owned(),
        ));
    }
    let invoice_date = date(&request.invoice_date)?;
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
                (invoice_date + Duration::days(credit_days))
                    .format("%Y-%m-%d")
                    .to_string(),
                "party_default".to_owned(),
            )
        };
    if request.lines.is_empty() {
        return Err(OpenTallyError::Validation(
            "A sales draft requires at least one line".to_owned(),
        ));
    }
    let mut subtotal = Decimal::ZERO;
    let mut discount_total = Decimal::ZERO;
    let mut lines = Vec::with_capacity(request.lines.len());
    for line in request.lines {
        let description = line.description.trim();
        if description.is_empty() {
            return Err(OpenTallyError::Validation(
                "Sales line description is required".to_owned(),
            ));
        }
        let quantity = nonnegative(&line.quantity, "quantity")?;
        if quantity <= Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "Sales quantity must be greater than zero".to_owned(),
            ));
        }
        let rate = nonnegative(&line.rate, "rate")?;
        let discount_value = nonnegative(&line.discount_value, "discount")?;
        if line.stock_item_id.is_some() && (line.unit_id.is_none() || line.godown_id.is_none()) {
            return Err(OpenTallyError::Validation(
                "Inventory-linked sales lines require unit and godown".to_owned(),
            ));
        }
        if line.stock_item_id.is_none() && (line.unit_id.is_some() || line.godown_id.is_some()) {
            return Err(OpenTallyError::Validation(
                "Accounting-only lines cannot reference inventory locations".to_owned(),
            ));
        }
        validate_reference(pool, company_id, &line).await?;
        let gross = quantity * rate;
        let discount_amount = match line.discount_kind {
            DiscountKind::None => {
                if discount_value != Decimal::ZERO {
                    return Err(OpenTallyError::Validation(
                        "Discount value must be zero for no discount".to_owned(),
                    ));
                }
                Decimal::ZERO
            }
            DiscountKind::Amount => {
                if discount_value > gross {
                    return Err(OpenTallyError::Validation(
                        "Discount cannot exceed line amount".to_owned(),
                    ));
                }
                discount_value
            }
            DiscountKind::Percentage => {
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
        invoice_date: invoice_date.format("%Y-%m-%d").to_string(),
        document_reference: request.document_reference,
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
    line: &SalesLineRequest,
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

async fn insert_sales(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
    voucher_id: &str,
    p: Prepared,
) -> Result<(SalesDocument, Vec<SalesDocumentLine>), OpenTallyError> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO sales_documents (id,company_id,voucher_id,party_id,invoice_date,document_reference,due_date,due_date_source,due_date_override_reason,narration,subtotal,discount_total,additional_charges,net_total) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(id).bind(company_id).bind(voucher_id).bind(&p.party_id).bind(&p.invoice_date).bind(&p.document_reference).bind(&p.due_date).bind(&p.due_date_source).bind(&p.due_date_override_reason).bind(&p.narration).bind(p.subtotal.normalize().to_string()).bind(p.discount_total.normalize().to_string()).bind(p.additional_charges.normalize().to_string()).bind(p.net_total.normalize().to_string()).execute(&mut *tx).await.map_err(map_constraint)?;
    insert_lines(&mut tx, company_id, id, p.lines).await?;
    let mut lines = Vec::new();
    audit(&mut tx, company_id, id, "CREATE", None, None).await?;
    let document = sqlx::query_as::<_, SalesDocument>("SELECT id,company_id,voucher_id,party_id,invoice_date,document_reference,due_date,due_date_source,due_date_override_reason,narration,status,subtotal,discount_total,additional_charges,net_total,tax_status,created_at,updated_at FROM sales_documents WHERE company_id=? AND id=?").bind(company_id).bind(id).fetch_one(&mut *tx).await?;
    let rows = sqlx::query_as::<_, SalesDocumentLine>("SELECT id,company_id,sales_document_id,line_number,description,stock_item_id,unit_id,godown_id,quantity,rate,discount_kind,discount_value,discount_amount,line_amount,created_at FROM sales_document_lines WHERE company_id=? AND sales_document_id=? ORDER BY line_number").bind(company_id).bind(id).fetch_all(&mut *tx).await?;
    lines.extend(rows);
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
        sqlx::query("INSERT INTO sales_document_lines (id,company_id,sales_document_id,line_number,description,stock_item_id,unit_id,godown_id,quantity,rate,discount_kind,discount_value,discount_amount,line_amount) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&line_id).bind(company_id).bind(document_id).bind((index + 1) as i64)
            .bind(line.request.description.trim()).bind(&line.request.stock_item_id)
            .bind(&line.request.unit_id).bind(&line.request.godown_id)
            .bind(line.quantity.normalize().to_string()).bind(line.rate.normalize().to_string())
            .bind(line.request.discount_kind.as_str()).bind(line.discount_value.normalize().to_string())
            .bind(line.discount_amount.normalize().to_string()).bind(line.line_amount.normalize().to_string())
            .execute(&mut **tx).await.map_err(map_constraint)?;
    }
    Ok(())
}

async fn select_document(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
) -> Result<SalesDocument, OpenTallyError> {
    sqlx::query_as::<_, SalesDocument>("SELECT id,company_id,voucher_id,party_id,invoice_date,document_reference,due_date,due_date_source,due_date_override_reason,narration,status,subtotal,discount_total,additional_charges,net_total,tax_status,created_at,updated_at FROM sales_documents WHERE company_id=? AND id=? AND status='draft'").bind(company_id).bind(id).fetch_optional(pool).await?.ok_or_else(|| OpenTallyError::NotFound(format!("Sales draft '{}' not found", id)))
}
async fn select_lines(
    pool: &SqlitePool,
    company_id: &str,
    id: &str,
) -> Result<Vec<SalesDocumentLine>, OpenTallyError> {
    Ok(sqlx::query_as::<_, SalesDocumentLine>("SELECT id,company_id,sales_document_id,line_number,description,stock_item_id,unit_id,godown_id,quantity,rate,discount_kind,discount_value,discount_amount,line_amount,created_at FROM sales_document_lines WHERE company_id=? AND sales_document_id=? ORDER BY line_number").bind(company_id).bind(id).fetch_all(pool).await?)
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
                "Sales document reference or linked record is already used".to_owned(),
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
    old: Option<&SalesDocument>,
    _new: Option<&SalesDocumentRequest>,
) -> Result<(), OpenTallyError> {
    let old_values = old.map(serde_json::to_string).transpose()?;
    sqlx::query("INSERT INTO audit_log (id,company_id,entity_type,entity_id,action,old_values,performed_by) VALUES (? ,?, 'sales_document', ?, ?, ?, 'system')").bind(Uuid::new_v4().to_string()).bind(company_id).bind(id).bind(action).bind(old_values).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod stage18_tests {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
        run_sprint4_stage12_valuation_migration, run_sprint5_stage14_integration_migration,
        run_sprint5_stage15_valuation_attribution_migration, run_sprint6_stage17_party_migration,
        run_sprint6_stage18_sales_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use rust_decimal::Decimal;

    async fn fixture() -> (tempfile::TempDir, DatabaseManager) {
        let dir = tempfile::tempdir().unwrap();
        let db = DatabaseManager::init(&dir.path().join("stage18.db"))
            .await
            .unwrap();
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
        run_sprint6_stage17_party_migration(p).await.unwrap();
        seed(p).await;
        run_sprint6_stage18_sales_migration(p).await.unwrap();
        (dir, db)
    }

    async fn seed(p: &sqlx::Pool<sqlx::Sqlite>) {
        sqlx::query("INSERT INTO companies (id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES ('cmp_a','Alpha','Alpha','India','Delhi','2026-04-01','2026-04-01'),('cmp_b','Beta','Beta','India','Karnataka','2026-04-01','2026-04-01')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO account_groups (id,company_id,name,parent_id,nature,is_primary) VALUES ('grp_a','cmp_a','Current Assets',NULL,'Assets',1),('grp_b','cmp_b','Current Assets',NULL,'Assets',1)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO ledgers (id,company_id,group_id,name,is_reserved,ledger_classification) VALUES ('led_a','cmp_a','grp_a','Receivable',0,'standard'),('led_b','cmp_b','grp_b','Receivable',0,'standard')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO party_profiles (id,company_id,ledger_id,party_code,display_name,customer_enabled,supplier_enabled,credit_period_days,credit_limit) VALUES ('party_a','cmp_a','led_a','CUST-A','Customer A',1,0,30,'1000'),('party_b','cmp_b','led_b','CUST-B','Customer B',1,0,0,'0')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO voucher_number_series (id,company_id,voucher_type_id,series_name,series_code,numbering_method,restart_period) VALUES ('series_a','cmp_a','vt_sales','Sales Series','SAL','automatic','yearly')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO voucher_number_configs (id,company_id,voucher_type_id,series_id,applicable_from,prefix,suffix,start_number,width) VALUES ('cfg_a','cmp_a','vt_sales','series_a','2026-04-01','SAL/','','1',4)").execute(p).await.unwrap();
        sqlx::query(
            "INSERT INTO stock_groups (id,company_id,name) VALUES ('sg_a','cmp_a','Goods')",
        )
        .execute(p)
        .await
        .unwrap();
        sqlx::query("INSERT INTO units (id,company_id,symbol,formal_name,unit_type,decimal_precision) VALUES ('unit_a','cmp_a','PCS','Pieces','simple',2)").execute(p).await.unwrap();
        sqlx::query("INSERT INTO godowns (id,company_id,name) VALUES ('godown_a','cmp_a','Main')")
            .execute(p)
            .await
            .unwrap();
        sqlx::query("INSERT INTO stock_items (id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES ('item_a','cmp_a','Widget','sg_a','unit_a','0','0','0','0')").execute(p).await.unwrap();
    }

    fn accounting_line(description: &str) -> SalesLineRequest {
        SalesLineRequest {
            description: description.to_owned(),
            stock_item_id: None,
            unit_id: None,
            godown_id: None,
            quantity: "1".to_owned(),
            rate: "100.25".to_owned(),
            discount_kind: DiscountKind::None,
            discount_value: "0".to_owned(),
        }
    }

    fn request() -> SalesDocumentRequest {
        SalesDocumentRequest {
            series_id: "series_a".to_owned(),
            party_id: "party_a".to_owned(),
            invoice_date: "2026-04-10".to_owned(),
            document_reference: Some("PO-1".to_owned()),
            due_date_override: None,
            due_date_override_reason: None,
            narration: Some("Foundation draft".to_owned()),
            document_discount_amount: "0".to_owned(),
            additional_charges: "0".to_owned(),
            lines: vec![accounting_line("Consulting")],
        }
    }

    #[tokio::test]
    async fn sales_draft_creation_calculates_exact_totals_and_default_due_date() {
        let (_dir, db) = fixture().await;
        let doc = SalesDocumentService::create_draft(db.pool(), "cmp_a", request())
            .await
            .unwrap();
        assert_eq!(doc.document.status, "draft");
        assert_eq!(doc.document.due_date, "2026-05-10");
        assert_eq!(
            Decimal::from_str_exact(&doc.document.net_total).unwrap(),
            Decimal::new(10025, 2)
        );
        let sequence_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM voucher_number_sequences WHERE company_id='cmp_a'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(sequence_count, 0);
    }

    #[tokio::test]
    async fn customer_role_active_and_company_validation_are_enforced() {
        let (_dir, db) = fixture().await;
        let mut req = request();
        req.party_id = "party_b".to_owned();
        assert!(SalesDocumentService::create_draft(db.pool(), "cmp_a", req)
            .await
            .is_err());
        sqlx::query(
            "UPDATE party_profiles SET is_active=0 WHERE company_id='cmp_a' AND id='party_a'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        assert!(
            SalesDocumentService::create_draft(db.pool(), "cmp_a", request())
                .await
                .is_err()
        );
        assert!(SalesDocumentService::list_drafts(db.pool(), "cmp_b", None)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn inventory_linked_line_and_discount_validation_are_supported() {
        let (_dir, db) = fixture().await;
        let mut req = request();
        req.lines = vec![SalesLineRequest {
            description: "Widget sale".to_owned(),
            stock_item_id: Some("item_a".to_owned()),
            unit_id: Some("unit_a".to_owned()),
            godown_id: Some("godown_a".to_owned()),
            quantity: "2.50".to_owned(),
            rate: "10.00".to_owned(),
            discount_kind: DiscountKind::Percentage,
            discount_value: "10".to_owned(),
        }];
        let doc = SalesDocumentService::create_draft(db.pool(), "cmp_a", req)
            .await
            .unwrap();
        assert_eq!(doc.lines[0].line_number, 1);
        assert_eq!(doc.lines[0].line_amount, "22.5");
        let mut invalid = request();
        invalid.lines[0].quantity = "0".to_owned();
        assert!(
            SalesDocumentService::create_draft(db.pool(), "cmp_a", invalid)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn draft_update_reorder_delete_and_override_provenance_are_persisted() {
        let (_dir, db) = fixture().await;
        let mut req = request();
        req.due_date_override = Some("2026-06-01".to_owned());
        req.due_date_override_reason = Some("Customer agreement".to_owned());
        req.lines.push(accounting_line("Second"));
        let doc = SalesDocumentService::create_draft(db.pool(), "cmp_a", req)
            .await
            .unwrap();
        assert_eq!(doc.document.due_date_source, "explicit_override");
        let loaded = SalesDocumentService::get_draft(db.pool(), "cmp_a", &doc.document.id)
            .await
            .unwrap();
        assert_eq!(loaded.lines.len(), 2);
        let mut replacement = request();
        replacement.lines = vec![accounting_line("Replacement")];
        let updated =
            SalesDocumentService::update_draft(db.pool(), "cmp_a", &doc.document.id, replacement)
                .await
                .unwrap();
        assert_eq!(updated.lines.len(), 1);
        SalesDocumentService::delete_draft(db.pool(), "cmp_a", &doc.document.id)
            .await
            .unwrap();
        assert!(
            SalesDocumentService::get_draft(db.pool(), "cmp_a", &doc.document.id)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn stage18_migration_chain_is_restart_idempotent() {
        let (_dir, db) = fixture().await;
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
        run_sprint6_stage17_party_migration(p).await.unwrap();
        run_sprint6_stage18_sales_migration(p).await.unwrap();
    }
}
