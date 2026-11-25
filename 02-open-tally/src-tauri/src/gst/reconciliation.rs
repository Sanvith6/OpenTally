use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[inline]
fn fmt_2dp(d: Decimal) -> String {
    format!("{:.2}", d)
}

/// Parse a decimal from a serde_json::Value (handles number, string, or missing).
fn parse_decimal_from_val(val: Option<&serde_json::Value>) -> Decimal {
    match val {
        Some(serde_json::Value::Number(num)) => {
            Decimal::from_str(&num.to_string()).unwrap_or(Decimal::ZERO)
        }
        Some(serde_json::Value::String(s)) => Decimal::from_str_exact(s.trim())
            .unwrap_or_else(|_| Decimal::from_str(s.trim()).unwrap_or(Decimal::ZERO)),
        _ => Decimal::ZERO,
    }
}

/// Normalize dates in DD-MM-YYYY or DD/MM/YYYY into canonical ISO YYYY-MM-DD.
fn normalize_date_to_iso(dt_str: &str) -> String {
    let clean = dt_str.trim();
    if clean.len() == 10 {
        let b = clean.as_bytes();
        // Check if DD-MM-YYYY or DD/MM/YYYY
        if (b[2] == b'-' || b[2] == b'/') && (b[5] == b'-' || b[5] == b'/') {
            let day = &clean[0..2];
            let month = &clean[3..5];
            let year = &clean[6..10];
            return format!("{}-{}-{}", year, month, day);
        }
    }
    clean.to_string()
}

/// Convert return_period "MMYYYY" into (start_date, end_date) in "YYYY-MM-DD".
pub fn get_period_date_bounds(return_period: &str) -> Result<(String, String), OpenTallyError> {
    let rp = return_period.trim();
    if rp.len() != 6 || !rp.chars().all(|c| c.is_ascii_digit()) {
        return Err(OpenTallyError::Validation(format!(
            "Invalid return_period '{}', expected 6-digit MMYYYY format (e.g. 042026)",
            rp
        )));
    }
    let month: u32 = rp[0..2].parse().unwrap_or(0);
    let year: i32 = rp[2..6].parse().unwrap_or(0);

    if !(1..=12).contains(&month) || !(2000..=2100).contains(&year) {
        return Err(OpenTallyError::Validation(format!(
            "Invalid month or year in return_period '{}'",
            rp
        )));
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => 30,
    };

    let start = format!("{:04}-{:02}-01", year, month);
    let end = format!("{:04}-{:02}-{:02}", year, month, days_in_month);
    Ok((start, end))
}

// -----------------------------------------------------------------------------
// DTOs & Models
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr2bReconciledInvoiceRow {
    pub id: String,
    pub batch_id: String,
    pub supplier_gstin: String,
    pub supplier_name: Option<String>,
    pub invoice_number: String,
    pub invoice_date: String,
    pub invoice_value: String,
    pub taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub total_tax: String,
    pub itc_availability: String,
    pub reconciliation_status: String, // "Matched", "Mismatch", "MissingInBooks", "MissingIn2B"
    pub matched_voucher_id: Option<String>,
    pub matched_voucher_number: Option<String>,
    pub books_taxable_value: Option<String>,
    pub books_total_tax: Option<String>,
    pub tax_difference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr2bReconciliationSummary {
    pub total_portal_invoices: usize,
    pub matched_count: usize,
    pub mismatch_count: usize,
    pub missing_in_books_count: usize,
    pub missing_in_2b_count: usize,
    pub matched_itc: String,
    pub mismatch_itc: String,
    pub missing_in_books_itc: String,
    pub at_risk_itc: String,
    pub total_eligible_itc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr2bReconciliationReport {
    pub company_id: String,
    pub return_period: String,
    pub batch_id: Option<String>,
    pub imported_at: Option<String>,
    pub summary: Gstr2bReconciliationSummary,
    pub invoices: Vec<Gstr2bReconciledInvoiceRow>,
}

#[derive(Debug, Clone)]
pub struct ParsedPortalInvoice {
    pub supplier_gstin: String,
    pub supplier_name: Option<String>,
    pub invoice_number: String,
    pub invoice_date: String,
    pub invoice_value: Decimal,
    pub taxable_value: Decimal,
    pub cgst_amount: Decimal,
    pub sgst_amount: Decimal,
    pub igst_amount: Decimal,
    pub cess_amount: Decimal,
    pub total_tax: Decimal,
    pub itc_availability: String,
}

#[derive(Debug, Clone)]
struct BooksPurchaseVoucher {
    voucher_id: String,
    voucher_number: String,
    reference_no: Option<String>,
    date: String,
    party_gstin: Option<String>,
    party_name: String,
    taxable_value: Decimal,
    cgst_amount: Decimal,
    sgst_amount: Decimal,
    igst_amount: Decimal,
    cess_amount: Decimal,
    total_tax: Decimal,
}

// -----------------------------------------------------------------------------
// GSTR-2B Service
// -----------------------------------------------------------------------------

pub struct Gstr2bService;

impl Gstr2bService {
    /// Parse raw GST Portal GSTR-2B JSON payload into structured portal invoices.
    /// Supports standard nested formats: `data.docdata.b2b`, `docdata.b2b`, or `b2b`.
    pub fn parse_gstr2b_json(json_str: &str) -> Result<Vec<ParsedPortalInvoice>, OpenTallyError> {
        // Enforce 25 MB payload limit
        if json_str.len() > 25 * 1024 * 1024 {
            return Err(OpenTallyError::Validation(format!(
                "GSTR-2B JSON payload exceeds maximum allowed size of 25MB (got {} bytes)",
                json_str.len()
            )));
        }

        let root: serde_json::Value = serde_json::from_str(json_str).map_err(|e| {
            OpenTallyError::Validation(format!("Invalid GSTR-2B JSON syntax: {}", e))
        })?;

        // Locate b2b array
        let b2b_val = root
            .get("data")
            .and_then(|d| d.get("docdata"))
            .and_then(|dd| dd.get("b2b"))
            .or_else(|| root.get("docdata").and_then(|dd| dd.get("b2b")))
            .or_else(|| root.get("b2b"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| {
                OpenTallyError::Validation(
                    "GSTR-2B JSON missing required 'b2b' supplier invoices array".to_string(),
                )
            })?;

        let mut invoices = Vec::new();

        for supplier in b2b_val {
            let ctin = supplier
                .get("ctin")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .trim()
                .to_uppercase();

            if ctin.len() != 15 {
                continue;
            }

            let cname = supplier
                .get("cname")
                .and_then(|n| n.as_str())
                .map(|s| s.trim().to_string());

            if let Some(inv_arr) = supplier.get("inv").and_then(|i| i.as_array()) {
                for inv in inv_arr {
                    let inum = inv
                        .get("inum")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if inum.is_empty() {
                        continue;
                    }

                    let raw_dt = inv.get("dt").and_then(|d| d.as_str()).unwrap_or("");
                    let iso_dt = normalize_date_to_iso(raw_dt);

                    let itcavl = inv
                        .get("itcavl")
                        .and_then(|a| a.as_str())
                        .map(|s| s.trim().to_uppercase())
                        .unwrap_or_else(|| "Y".to_string());
                    let itc_avail = if itcavl == "N" { "N" } else { "Y" }.to_string();

                    let val_from_inv = parse_decimal_from_val(inv.get("val"));

                    let mut inv_taxable = Decimal::ZERO;
                    let mut inv_cgst = Decimal::ZERO;
                    let mut inv_sgst = Decimal::ZERO;
                    let mut inv_igst = Decimal::ZERO;
                    let mut inv_cess = Decimal::ZERO;

                    if let Some(items) = inv.get("items").and_then(|it| it.as_array()) {
                        for it in items {
                            inv_taxable += parse_decimal_from_val(it.get("txval"));
                            inv_cgst += parse_decimal_from_val(it.get("camt"));
                            inv_sgst += parse_decimal_from_val(it.get("samt"));
                            inv_igst += parse_decimal_from_val(it.get("iamt"));
                            inv_cess += parse_decimal_from_val(it.get("csamt"));
                        }
                    }

                    let inv_total_tax = inv_cgst + inv_sgst + inv_igst + inv_cess;
                    let final_val = if val_from_inv.is_zero() {
                        inv_taxable + inv_total_tax
                    } else {
                        val_from_inv
                    };

                    invoices.push(ParsedPortalInvoice {
                        supplier_gstin: ctin.clone(),
                        supplier_name: cname.clone(),
                        invoice_number: inum,
                        invoice_date: iso_dt,
                        invoice_value: final_val,
                        taxable_value: inv_taxable,
                        cgst_amount: inv_cgst,
                        sgst_amount: inv_sgst,
                        igst_amount: inv_igst,
                        cess_amount: inv_cess,
                        total_tax: inv_total_tax,
                        itc_availability: itc_avail,
                    });
                }
            }
        }

        Ok(invoices)
    }

    /// Import and reconcile GSTR-2B JSON for a company and return period inside an explicit BEGIN IMMEDIATE transaction.
    pub async fn import_and_reconcile_gstr2b(
        pool: &Pool<Sqlite>,
        company_id: &str,
        return_period: &str,
        json_payload: &str,
    ) -> Result<Gstr2bReconciliationReport, OpenTallyError> {
        let (period_start, period_end) = get_period_date_bounds(return_period)?;
        let parsed_portal_invoices = Self::parse_gstr2b_json(json_payload)?;

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // Fetch company GSTIN
        let company_gstin: Option<String> = sqlx::query_scalar(
            "SELECT COALESCE(gstin, '') FROM company_gst_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .filter(|g: &String| g.len() == 15);

        let company_gstin = company_gstin.unwrap_or_else(|| "000000000000000".to_string());

        // Dynamic financial year begins from companies
        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => format!("{}-04-01", &period_start[0..4]),
            };

        // Query all posted purchase vouchers and their GST lines in the period
        #[derive(sqlx::FromRow)]
        struct RawPurchaseGstRow {
            voucher_id: String,
            voucher_number: String,
            reference_no: Option<String>,
            date: String,
            party_gstin: Option<String>,
            party_name: String,
            taxable_value: String,
            cgst_amount: String,
            sgst_amount: String,
            igst_amount: String,
            cess_amount: String,
        }

        let raw_rows = sqlx::query_as::<_, RawPurchaseGstRow>(
            "SELECT v.id as voucher_id, v.voucher_number, v.reference_no, v.date,
                    COALESCE(vgd.party_gstin, lgp.gstin) as party_gstin,
                    l.name as party_name,
                    vgd.taxable_value, vgd.cgst_amount, vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount
             FROM vouchers v
             JOIN voucher_gst_details vgd ON vgd.company_id = v.company_id AND vgd.voucher_id = v.id
             JOIN ledgers l ON l.company_id = v.company_id AND l.id = vgd.party_ledger_id
             LEFT JOIN ledger_gst_profiles lgp ON lgp.company_id = v.company_id AND lgp.ledger_id = vgd.party_ledger_id
             WHERE v.company_id = ?
               AND v.voucher_type_id = 'vt_pur'
               AND v.status = 'posted'
               AND vgd.is_cancelled = 0
               AND v.date >= ? AND v.date <= ?
             ORDER BY v.date ASC, v.voucher_number ASC",
        )
        .bind(company_id)
        .bind(&period_start)
        .bind(&period_end)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Aggregate by voucher_id
        let mut books_map: HashMap<String, BooksPurchaseVoucher> = HashMap::new();
        let mut books_order: Vec<String> = Vec::new();

        for r in raw_rows {
            let tax_val = Decimal::from_str_exact(&r.taxable_value).unwrap_or(Decimal::ZERO);
            let c_val = Decimal::from_str_exact(&r.cgst_amount).unwrap_or(Decimal::ZERO);
            let s_val = Decimal::from_str_exact(&r.sgst_amount).unwrap_or(Decimal::ZERO);
            let i_val = Decimal::from_str_exact(&r.igst_amount).unwrap_or(Decimal::ZERO);
            let cs_val = Decimal::from_str_exact(&r.cess_amount).unwrap_or(Decimal::ZERO);
            let tot_t = c_val + s_val + i_val + cs_val;

            if let Some(entry) = books_map.get_mut(&r.voucher_id) {
                entry.taxable_value += tax_val;
                entry.cgst_amount += c_val;
                entry.sgst_amount += s_val;
                entry.igst_amount += i_val;
                entry.cess_amount += cs_val;
                entry.total_tax += tot_t;
            } else {
                books_order.push(r.voucher_id.clone());
                books_map.insert(
                    r.voucher_id.clone(),
                    BooksPurchaseVoucher {
                        voucher_id: r.voucher_id,
                        voucher_number: r.voucher_number,
                        reference_no: r.reference_no,
                        date: r.date,
                        party_gstin: r.party_gstin.map(|g| g.trim().to_uppercase()),
                        party_name: r.party_name,
                        taxable_value: tax_val,
                        cgst_amount: c_val,
                        sgst_amount: s_val,
                        igst_amount: i_val,
                        cess_amount: cs_val,
                        total_tax: tot_t,
                    },
                );
            }
        }

        // Delete any existing batches and portal invoices for this (company_id, return_period)
        sqlx::query("DELETE FROM gstr2b_import_batches WHERE company_id = ? AND return_period = ?")
            .bind(company_id)
            .bind(return_period)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let batch_id = Uuid::new_v4().to_string();
        let mut matched_books_vids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        let mut reconciled_rows: Vec<Gstr2bReconciledInvoiceRow> = Vec::new();
        let mut total_portal_itc = Decimal::ZERO;
        let mut matched_itc = Decimal::ZERO;
        let mut mismatch_itc = Decimal::ZERO;
        let mut missing_in_books_itc = Decimal::ZERO;
        let mut total_eligible_itc = Decimal::ZERO;

        let mut matched_count = 0usize;
        let mut mismatch_count = 0usize;
        let mut missing_in_books_count = 0usize;

        // Tolerance: ₹1.00 for rounding differences
        let tolerance = Decimal::ONE;

        for portal_inv in &parsed_portal_invoices {
            total_portal_itc += portal_inv.total_tax;

            // Search for matching books voucher
            let p_gstin = &portal_inv.supplier_gstin;
            let p_inv_norm = portal_inv.invoice_number.trim().to_uppercase();

            let matched_book = books_order.iter().find_map(|vid| {
                if matched_books_vids.contains(vid) {
                    return None;
                }
                if let Some(bv) = books_map.get(vid) {
                    if let Some(ref bgstin) = bv.party_gstin {
                        if bgstin == p_gstin {
                            // Compare with reference_no first, then voucher_number
                            let ref_norm = bv
                                .reference_no
                                .as_ref()
                                .map(|r| r.trim().to_uppercase())
                                .unwrap_or_default();
                            let vnum_norm = bv.voucher_number.trim().to_uppercase();

                            if ref_norm == p_inv_norm || vnum_norm == p_inv_norm {
                                return Some(bv.clone());
                            }
                        }
                    }
                }
                None
            });

            let inv_id = Uuid::new_v4().to_string();

            if let Some(bv) = matched_book {
                matched_books_vids.insert(bv.voucher_id.clone());

                let tax_diff = portal_inv.total_tax - bv.total_tax;
                let taxable_diff = portal_inv.taxable_value - bv.taxable_value;

                let is_exact = tax_diff.abs() <= tolerance && taxable_diff.abs() <= tolerance;
                let status = if is_exact { "Matched" } else { "Mismatch" };

                if is_exact {
                    matched_count += 1;
                    matched_itc += portal_inv.total_tax;
                    if portal_inv.itc_availability == "Y" {
                        total_eligible_itc += portal_inv.total_tax;
                    }
                } else {
                    mismatch_count += 1;
                    mismatch_itc += portal_inv.total_tax;
                }

                reconciled_rows.push(Gstr2bReconciledInvoiceRow {
                    id: inv_id,
                    batch_id: batch_id.clone(),
                    supplier_gstin: portal_inv.supplier_gstin.clone(),
                    supplier_name: portal_inv.supplier_name.clone().or(Some(bv.party_name)),
                    invoice_number: portal_inv.invoice_number.clone(),
                    invoice_date: portal_inv.invoice_date.clone(),
                    invoice_value: fmt_2dp(portal_inv.invoice_value),
                    taxable_value: fmt_2dp(portal_inv.taxable_value),
                    cgst_amount: fmt_2dp(portal_inv.cgst_amount),
                    sgst_amount: fmt_2dp(portal_inv.sgst_amount),
                    igst_amount: fmt_2dp(portal_inv.igst_amount),
                    cess_amount: fmt_2dp(portal_inv.cess_amount),
                    total_tax: fmt_2dp(portal_inv.total_tax),
                    itc_availability: portal_inv.itc_availability.clone(),
                    reconciliation_status: status.to_string(),
                    matched_voucher_id: Some(bv.voucher_id.clone()),
                    matched_voucher_number: Some(bv.voucher_number.clone()),
                    books_taxable_value: Some(fmt_2dp(bv.taxable_value)),
                    books_total_tax: Some(fmt_2dp(bv.total_tax)),
                    tax_difference: fmt_2dp(tax_diff),
                });
            } else {
                missing_in_books_count += 1;
                missing_in_books_itc += portal_inv.total_tax;

                reconciled_rows.push(Gstr2bReconciledInvoiceRow {
                    id: inv_id,
                    batch_id: batch_id.clone(),
                    supplier_gstin: portal_inv.supplier_gstin.clone(),
                    supplier_name: portal_inv.supplier_name.clone(),
                    invoice_number: portal_inv.invoice_number.clone(),
                    invoice_date: portal_inv.invoice_date.clone(),
                    invoice_value: fmt_2dp(portal_inv.invoice_value),
                    taxable_value: fmt_2dp(portal_inv.taxable_value),
                    cgst_amount: fmt_2dp(portal_inv.cgst_amount),
                    sgst_amount: fmt_2dp(portal_inv.sgst_amount),
                    igst_amount: fmt_2dp(portal_inv.igst_amount),
                    cess_amount: fmt_2dp(portal_inv.cess_amount),
                    total_tax: fmt_2dp(portal_inv.total_tax),
                    itc_availability: portal_inv.itc_availability.clone(),
                    reconciliation_status: "MissingInBooks".to_string(),
                    matched_voucher_id: None,
                    matched_voucher_number: None,
                    books_taxable_value: None,
                    books_total_tax: None,
                    tax_difference: fmt_2dp(portal_inv.total_tax),
                });
            }
        }

        // Identify MissingIn2B (At-Risk ITC): Active purchase vouchers with registered supplier GSTIN absent in 2B
        let mut missing_in_2b_count = 0usize;
        let mut at_risk_itc = Decimal::ZERO;

        for vid in &books_order {
            if !matched_books_vids.contains(vid) {
                if let Some(bv) = books_map.get(vid) {
                    if let Some(ref bgstin) = bv.party_gstin {
                        if bgstin.len() == 15 {
                            missing_in_2b_count += 1;
                            at_risk_itc += bv.total_tax;

                            let inv_no = bv
                                .reference_no
                                .as_ref()
                                .cloned()
                                .unwrap_or_else(|| bv.voucher_number.clone());
                            let inv_id = Uuid::new_v4().to_string();

                            reconciled_rows.push(Gstr2bReconciledInvoiceRow {
                                id: inv_id,
                                batch_id: batch_id.clone(),
                                supplier_gstin: bgstin.clone(),
                                supplier_name: Some(bv.party_name.clone()),
                                invoice_number: inv_no,
                                invoice_date: bv.date.clone(),
                                invoice_value: fmt_2dp(bv.taxable_value + bv.total_tax),
                                taxable_value: fmt_2dp(bv.taxable_value),
                                cgst_amount: fmt_2dp(bv.cgst_amount),
                                sgst_amount: fmt_2dp(bv.sgst_amount),
                                igst_amount: fmt_2dp(bv.igst_amount),
                                cess_amount: fmt_2dp(bv.cess_amount),
                                total_tax: fmt_2dp(bv.total_tax),
                                itc_availability: "Y".to_string(),
                                reconciliation_status: "MissingIn2B".to_string(),
                                matched_voucher_id: Some(bv.voucher_id.clone()),
                                matched_voucher_number: Some(bv.voucher_number.clone()),
                                books_taxable_value: Some(fmt_2dp(bv.taxable_value)),
                                books_total_tax: Some(fmt_2dp(bv.total_tax)),
                                tax_difference: fmt_2dp(-bv.total_tax),
                            });
                        }
                    }
                }
            }
        }

        // Insert batch header
        sqlx::query(
            "INSERT INTO gstr2b_import_batches (
                id, company_id, return_period, gstin, total_portal_invoices, total_portal_itc
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&batch_id)
        .bind(company_id)
        .bind(return_period)
        .bind(&company_gstin)
        .bind(parsed_portal_invoices.len() as i64)
        .bind(fmt_2dp(total_portal_itc))
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Insert reconciled portal invoices
        for row in &reconciled_rows {
            sqlx::query(
                "INSERT INTO gstr2b_portal_invoices (
                    id, company_id, batch_id, supplier_gstin, supplier_name,
                    invoice_number, invoice_date, invoice_value, taxable_value,
                    cgst_amount, sgst_amount, igst_amount, cess_amount,
                    itc_availability, reconciliation_status, matched_voucher_id, tax_difference
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&row.id)
            .bind(company_id)
            .bind(&batch_id)
            .bind(&row.supplier_gstin)
            .bind(&row.supplier_name)
            .bind(&row.invoice_number)
            .bind(&row.invoice_date)
            .bind(&row.invoice_value)
            .bind(&row.taxable_value)
            .bind(&row.cgst_amount)
            .bind(&row.sgst_amount)
            .bind(&row.igst_amount)
            .bind(&row.cess_amount)
            .bind(&row.itc_availability)
            .bind(&row.reconciliation_status)
            .bind(&row.matched_voucher_id)
            .bind(&row.tax_difference)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        // Log MCA immutable audit entry
        let audit_id = Uuid::new_v4().to_string();
        let new_values = serde_json::json!({
            "batch_id": batch_id,
            "return_period": return_period,
            "total_portal_invoices": parsed_portal_invoices.len(),
            "total_portal_itc": fmt_2dp(total_portal_itc),
            "matched_count": matched_count,
            "mismatch_count": mismatch_count,
            "missing_in_books_count": missing_in_books_count,
            "missing_in_2b_count": missing_in_2b_count,
            "at_risk_itc": fmt_2dp(at_risk_itc)
        })
        .to_string();

        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
             ) VALUES (?, ?, 'gstr2b_reconciliation', ?, NULL, ?, 'CREATE', NULL, ?, 'system')",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(&batch_id)
        .bind(&fy_begins)
        .bind(&new_values)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let summary = Gstr2bReconciliationSummary {
            total_portal_invoices: parsed_portal_invoices.len(),
            matched_count,
            mismatch_count,
            missing_in_books_count,
            missing_in_2b_count,
            matched_itc: fmt_2dp(matched_itc),
            mismatch_itc: fmt_2dp(mismatch_itc),
            missing_in_books_itc: fmt_2dp(missing_in_books_itc),
            at_risk_itc: fmt_2dp(at_risk_itc),
            total_eligible_itc: fmt_2dp(total_eligible_itc),
        };

        Ok(Gstr2bReconciliationReport {
            company_id: company_id.to_string(),
            return_period: return_period.to_string(),
            batch_id: Some(batch_id),
            imported_at: Some(chrono::Utc::now().to_rfc3339()),
            summary,
            invoices: reconciled_rows,
        })
    }

    /// Read-only fetch of GSTR-2B reconciliation report using ReadOnlyConnectionGuard.
    pub async fn get_gstr2b_reconciliation_report(
        pool: &Pool<Sqlite>,
        company_id: &str,
        return_period: &str,
    ) -> Result<Gstr2bReconciliationReport, OpenTallyError> {
        let cid = company_id.to_string();
        let rp = return_period.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let rp = rp.clone();

            Box::pin(async move {
                #[derive(sqlx::FromRow)]
                struct BatchRow {
                    id: String,
                    imported_at: String,
                    total_portal_invoices: i64,
                }

                let batch: Option<BatchRow> = sqlx::query_as(
                    "SELECT id, imported_at, total_portal_invoices
                     FROM gstr2b_import_batches
                     WHERE company_id = ? AND return_period = ?
                     ORDER BY imported_at DESC LIMIT 1",
                )
                .bind(&cid)
                .bind(&rp)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (batch_id, imported_at, total_portal) = match batch {
                    Some(b) => (Some(b.id), Some(b.imported_at), b.total_portal_invoices as usize),
                    None => (None, None, 0usize),
                };

                #[derive(sqlx::FromRow)]
                struct InvDbRow {
                    id: String,
                    batch_id: String,
                    supplier_gstin: String,
                    supplier_name: Option<String>,
                    invoice_number: String,
                    invoice_date: String,
                    invoice_value: String,
                    taxable_value: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                    itc_availability: String,
                    reconciliation_status: String,
                    matched_voucher_id: Option<String>,
                    matched_voucher_number: Option<String>,
                    tax_difference: String,
                }

                let rows: Vec<InvDbRow> = if let Some(ref bid) = batch_id {
                    sqlx::query_as(
                        "SELECT pi.id, pi.batch_id, pi.supplier_gstin, pi.supplier_name,
                                pi.invoice_number, pi.invoice_date, pi.invoice_value, pi.taxable_value,
                                pi.cgst_amount, pi.sgst_amount, pi.igst_amount, pi.cess_amount,
                                pi.itc_availability, pi.reconciliation_status, pi.matched_voucher_id,
                                v.voucher_number as matched_voucher_number, pi.tax_difference
                         FROM gstr2b_portal_invoices pi
                         LEFT JOIN vouchers v ON v.company_id = pi.company_id AND v.id = pi.matched_voucher_id
                         WHERE pi.company_id = ? AND pi.batch_id = ?
                         ORDER BY pi.reconciliation_status ASC, pi.invoice_date ASC, pi.invoice_number ASC",
                    )
                    .bind(&cid)
                    .bind(bid)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?
                } else {
                    Vec::new()
                };

                // Aggregate GST details for matched vouchers to prevent Cartesian expansion
                let mut matched_vids: Vec<String> = rows
                    .iter()
                    .filter_map(|r| r.matched_voucher_id.clone())
                    .collect();
                matched_vids.sort();
                matched_vids.dedup();

                #[derive(sqlx::FromRow)]
                struct BooksGstLine {
                    voucher_id: String,
                    taxable_value: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                }

                let mut books_agg_map: HashMap<String, (Decimal, Decimal)> = HashMap::new();
                if !matched_vids.is_empty() {
                    let lines = sqlx::query_as::<_, BooksGstLine>(
                        "SELECT vgd.voucher_id, vgd.taxable_value, vgd.cgst_amount,
                                vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount
                         FROM voucher_gst_details vgd
                         WHERE vgd.company_id = ? AND vgd.is_cancelled = 0",
                    )
                    .bind(&cid)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    for line in lines {
                        if matched_vids.contains(&line.voucher_id) {
                            let tax_val = Decimal::from_str_exact(&line.taxable_value).unwrap_or(Decimal::ZERO);
                            let c = Decimal::from_str_exact(&line.cgst_amount).unwrap_or(Decimal::ZERO);
                            let s = Decimal::from_str_exact(&line.sgst_amount).unwrap_or(Decimal::ZERO);
                            let i = Decimal::from_str_exact(&line.igst_amount).unwrap_or(Decimal::ZERO);
                            let cs = Decimal::from_str_exact(&line.cess_amount).unwrap_or(Decimal::ZERO);
                            let tax = c + s + i + cs;

                            let entry = books_agg_map.entry(line.voucher_id).or_insert((Decimal::ZERO, Decimal::ZERO));
                            entry.0 += tax_val;
                            entry.1 += tax;
                        }
                    }
                }

                let mut matched_count = 0usize;
                let mut mismatch_count = 0usize;
                let mut missing_in_books_count = 0usize;
                let mut missing_in_2b_count = 0usize;

                let mut matched_itc = Decimal::ZERO;
                let mut mismatch_itc = Decimal::ZERO;
                let mut missing_in_books_itc = Decimal::ZERO;
                let mut at_risk_itc = Decimal::ZERO;
                let mut total_eligible_itc = Decimal::ZERO;

                let mut invoices = Vec::new();

                for r in rows {
                    let cgst = Decimal::from_str_exact(&r.cgst_amount).unwrap_or(Decimal::ZERO);
                    let sgst = Decimal::from_str_exact(&r.sgst_amount).unwrap_or(Decimal::ZERO);
                    let igst = Decimal::from_str_exact(&r.igst_amount).unwrap_or(Decimal::ZERO);
                    let cess = Decimal::from_str_exact(&r.cess_amount).unwrap_or(Decimal::ZERO);
                    let tot_tax = cgst + sgst + igst + cess;

                    let (books_taxable, books_tot_tax) = match r.matched_voucher_id.as_deref().and_then(|vid| books_agg_map.get(vid)) {
                        Some(&(taxable, tax)) => (Some(fmt_2dp(taxable)), Some(fmt_2dp(tax))),
                        None => (None, None),
                    };

                    match r.reconciliation_status.as_str() {
                        "Matched" => {
                            matched_count += 1;
                            matched_itc += tot_tax;
                            if r.itc_availability == "Y" {
                                total_eligible_itc += tot_tax;
                            }
                        }
                        "Mismatch" => {
                            mismatch_count += 1;
                            mismatch_itc += tot_tax;
                        }
                        "MissingInBooks" => {
                            missing_in_books_count += 1;
                            missing_in_books_itc += tot_tax;
                        }
                        "MissingIn2B" => {
                            missing_in_2b_count += 1;
                            at_risk_itc += tot_tax;
                        }
                        _ => {}
                    }

                    invoices.push(Gstr2bReconciledInvoiceRow {
                        id: r.id,
                        batch_id: r.batch_id,
                        supplier_gstin: r.supplier_gstin,
                        supplier_name: r.supplier_name,
                        invoice_number: r.invoice_number,
                        invoice_date: r.invoice_date,
                        invoice_value: r.invoice_value,
                        taxable_value: r.taxable_value,
                        cgst_amount: r.cgst_amount,
                        sgst_amount: r.sgst_amount,
                        igst_amount: r.igst_amount,
                        cess_amount: r.cess_amount,
                        total_tax: fmt_2dp(tot_tax),
                        itc_availability: r.itc_availability,
                        reconciliation_status: r.reconciliation_status,
                        matched_voucher_id: r.matched_voucher_id,
                        matched_voucher_number: r.matched_voucher_number,
                        books_taxable_value: books_taxable,
                        books_total_tax: books_tot_tax,
                        tax_difference: r.tax_difference,
                    });
                }

                let summary = Gstr2bReconciliationSummary {
                    total_portal_invoices: total_portal,
                    matched_count,
                    mismatch_count,
                    missing_in_books_count,
                    missing_in_2b_count,
                    matched_itc: fmt_2dp(matched_itc),
                    mismatch_itc: fmt_2dp(mismatch_itc),
                    missing_in_books_itc: fmt_2dp(missing_in_books_itc),
                    at_risk_itc: fmt_2dp(at_risk_itc),
                    total_eligible_itc: fmt_2dp(total_eligible_itc),
                };

                Ok(Gstr2bReconciliationReport {
                    company_id: cid,
                    return_period: rp,
                    batch_id,
                    imported_at,
                    summary,
                    invoices,
                })
            })
        })
        .await
    }

    /// Export GSTR-2B reconciliation report as a CWE-1236 injection-safe CSV string.
    pub async fn export_gstr2b_reconciliation_csv(
        pool: &Pool<Sqlite>,
        company_id: &str,
        return_period: &str,
    ) -> Result<String, OpenTallyError> {
        let report =
            Self::get_gstr2b_reconciliation_report(pool, company_id, return_period).await?;

        let mut csv = String::with_capacity(4096);
        csv.push_str("Status,Supplier GSTIN,Supplier Name,Invoice No,Invoice Date,Portal Taxable,Portal Tax,Books Tax,Variance,ITC Available,Matched Voucher No\r\n");

        for inv in report.invoices {
            csv.push_str(&sanitize_csv_cell(&inv.reconciliation_status));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.supplier_gstin));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(
                inv.supplier_name.as_deref().unwrap_or(""),
            ));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.invoice_number));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.invoice_date));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.taxable_value));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.total_tax));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(
                inv.books_total_tax.as_deref().unwrap_or("0.00"),
            ));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.tax_difference));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(&inv.itc_availability));
            csv.push(',');
            csv.push_str(&sanitize_csv_cell(
                inv.matched_voucher_number.as_deref().unwrap_or(""),
            ));
            csv.push_str("\r\n");
        }

        Ok(csv)
    }
}

/// CWE-1236 safe cell formatting: sanitize leading formula triggers ('=', '+', '-', '@', '\t', '\r')
fn sanitize_csv_cell(val: &str) -> String {
    let trimmed = val.trim();
    let safe_str = if trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@')
        || trimmed.starts_with('\t')
        || trimmed.starts_with('\r')
    {
        format!("'{}", trimmed)
    } else {
        trimmed.to_string()
    };

    if safe_str.contains(',')
        || safe_str.contains('"')
        || safe_str.contains('\n')
        || safe_str.contains('\r')
    {
        format!("\"{}\"", safe_str.replace('"', "\"\""))
    } else {
        safe_str
    }
}
