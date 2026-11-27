pub mod cheque;
pub mod payslip;
pub mod statements;
pub mod words;
pub mod writer;

pub use cheque::{export_cheque_pdf, generate_cheque_pdf_bytes, ChequeTemplateDto};

use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::path::Path;
use words::{format_indian_currency, number_to_indian_words};
use writer::PdfWriter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePdfItemLine {
    pub sl_no: usize,
    pub description: String,
    pub hsn_sac: String,
    pub quantity: Decimal,
    pub unit: String,
    pub rate: Decimal,
    pub discount: Decimal,
    pub taxable_value: Decimal,
    pub gst_rate: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceHsnTaxRow {
    pub hsn_sac: String,
    pub taxable_value: Decimal,
    pub cgst_rate: Decimal,
    pub cgst_amount: Decimal,
    pub sgst_rate: Decimal,
    pub sgst_amount: Decimal,
    pub igst_rate: Decimal,
    pub igst_amount: Decimal,
    pub cess_amount: Decimal,
    pub total_tax: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePdfModel {
    pub title: String,
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub original_invoice_no: Option<String>,
    pub original_invoice_date: Option<String>,
    pub place_of_supply: String,
    pub is_reverse_charge: bool,
    pub supplier_name: String,
    pub supplier_address: String,
    pub supplier_gstin: String,
    pub supplier_state: String,
    pub supplier_state_code: String,
    pub buyer_name: String,
    pub buyer_gstin: String,
    pub buyer_state: String,
    pub buyer_state_code: String,
    pub items: Vec<InvoicePdfItemLine>,
    pub hsn_summary: Vec<InvoiceHsnTaxRow>,
    pub total_taxable: Decimal,
    pub total_tax: Decimal,
    pub grand_total: Decimal,
    pub amount_in_words: String,
    #[serde(default)]
    pub irn: Option<String>,
    #[serde(default)]
    pub ack_no: Option<String>,
    #[serde(default)]
    pub ack_date: Option<String>,
    #[serde(default)]
    pub eway_bill_no: Option<String>,
    #[serde(default)]
    pub eway_valid_upto: Option<String>,
}

/// Generates a binary PDF byte stream for an Indian GST Tax Invoice / Note model.
pub fn generate_invoice_pdf(model: &InvoicePdfModel) -> Vec<u8> {
    let mut pdf = PdfWriter::new();

    let left = 35.0;
    let right = PdfWriter::PAGE_WIDTH - 35.0;
    let width = right - left;

    // Document Title Banner
    pdf.filled_rect(left, 790.0, width, 24.0, 0.93, 1.0);
    pdf.text_center(&model.title, left + (width / 2.0), 797.0, "/F2", 13.0);

    // Box 1: Supplier (Company) Block (Left) & Metadata (Right)
    let meta_box_y = 690.0;
    let meta_box_h = 95.0;
    pdf.rect(left, meta_box_y, width, meta_box_h, 0.75);

    // Vertical divider dividing Supplier and Invoice Details
    let mid_x = left + 270.0;
    pdf.line(mid_x, meta_box_y, mid_x, meta_box_y + meta_box_h, 0.75);

    // Left: Supplier Details
    pdf.text(&model.supplier_name, left + 8.0, 770.0, "/F2", 10.0);
    let mut cur_y = 757.0;
    if !model.supplier_address.is_empty() {
        pdf.text(&model.supplier_address, left + 8.0, cur_y, "/F1", 8.5);
        cur_y -= 12.0;
    }
    pdf.text(
        &format!(
            "GSTIN: {}",
            if model.supplier_gstin.is_empty() {
                "Unregistered"
            } else {
                &model.supplier_gstin
            }
        ),
        left + 8.0,
        cur_y,
        "/F2",
        8.5,
    );
    cur_y -= 12.0;
    pdf.text(
        &format!(
            "State: {} (Code: {})",
            model.supplier_state, model.supplier_state_code
        ),
        left + 8.0,
        cur_y,
        "/F1",
        8.5,
    );

    // Right: Invoice Metadata
    let r_x = mid_x + 8.0;
    pdf.text(
        &format!("Voucher / Inv No: {}", model.voucher_number),
        r_x,
        770.0,
        "/F2",
        9.0,
    );
    pdf.text(
        &format!("Date: {}", model.voucher_date),
        r_x,
        757.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!("Place of Supply: {}", model.place_of_supply),
        r_x,
        744.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!(
            "Reverse Charge: {}",
            if model.is_reverse_charge { "Yes" } else { "No" }
        ),
        r_x,
        731.0,
        "/F1",
        8.5,
    );

    if let (Some(orig_no), Some(orig_dt)) =
        (&model.original_invoice_no, &model.original_invoice_date)
    {
        pdf.text(
            &format!("Original Inv: {} ({})", orig_no, orig_dt),
            r_x,
            718.0,
            "/F2",
            8.0,
        );
    }

    if let Some(ref ewb) = model.eway_bill_no {
        let valid_str = match &model.eway_valid_upto {
            Some(v) => format!(" (Valid: {})", v),
            None => String::new(),
        };
        pdf.text(
            &format!("e-Way Bill: {}{}", ewb, valid_str),
            r_x,
            706.0,
            "/F2",
            7.5,
        );
    }

    if let Some(ref irn) = model.irn {
        let ack_str = match (&model.ack_no, &model.ack_date) {
            (Some(no), Some(dt)) => format!(" | Ack: {} ({})", no, dt),
            (Some(no), None) => format!(" | Ack: {}", no),
            _ => String::new(),
        };
        pdf.text(
            &format!("IRN: {}{}", irn, ack_str),
            left + 8.0,
            694.0,
            "/F2",
            6.5,
        );
    }

    // Box 2: Billed To (Buyer / Customer / Supplier)
    let buyer_y = 625.0;
    let buyer_h = 60.0;
    pdf.rect(left, buyer_y, width, buyer_h, 0.75);
    pdf.text("Billed To / Party Details:", left + 8.0, 672.0, "/F2", 9.0);
    pdf.text(&model.buyer_name, left + 8.0, 658.0, "/F2", 9.5);
    pdf.text(
        &format!(
            "GSTIN: {}",
            if model.buyer_gstin.is_empty() {
                "Unregistered"
            } else {
                &model.buyer_gstin
            }
        ),
        left + 8.0,
        646.0,
        "/F1",
        8.5,
    );
    pdf.text(
        &format!(
            "State: {} (Code: {})",
            model.buyer_state, model.buyer_state_code
        ),
        left + 8.0,
        634.0,
        "/F1",
        8.5,
    );

    // Box 3: Itemized Line Table
    let table_top = 615.0;
    let table_h = 210.0;
    let table_bottom = table_top - table_h;
    pdf.rect(left, table_bottom, width, table_h, 0.75);

    // Table Header
    let header_h = 18.0;
    pdf.filled_rect(left, table_top - header_h, width, header_h, 0.95, 0.75);

    let col_sl = left + 25.0;
    let col_desc = left + 180.0;
    let col_hsn = left + 235.0;
    let col_qty = left + 285.0;
    let col_rate = left + 345.0;
    let col_taxable = left + 415.0;
    let col_rate_gst = left + 465.0;
    let col_total = right;

    // Header labels
    pdf.text("Sl", left + 6.0, table_top - 13.0, "/F2", 8.0);
    pdf.text(
        "Description of Goods / Services",
        col_sl + 6.0,
        table_top - 13.0,
        "/F2",
        8.0,
    );
    pdf.text("HSN/SAC", col_desc + 4.0, table_top - 13.0, "/F2", 8.0);
    pdf.text_right("Qty", col_qty - 4.0, table_top - 13.0, "/F2", 8.0);
    pdf.text_right("Rate", col_rate - 4.0, table_top - 13.0, "/F2", 8.0);
    pdf.text_right(
        "Taxable Value",
        col_taxable - 4.0,
        table_top - 13.0,
        "/F2",
        8.0,
    );
    pdf.text_right("GST%", col_rate_gst - 4.0, table_top - 13.0, "/F2", 8.0);
    pdf.text_right("Total (INR)", col_total - 4.0, table_top - 13.0, "/F2", 8.0);

    // Vertical column grid lines
    for &col_x in &[
        col_sl,
        col_desc,
        col_hsn,
        col_qty,
        col_rate,
        col_taxable,
        col_rate_gst,
    ] {
        pdf.line(col_x, table_bottom, col_x, table_top, 0.5);
    }

    // Rows
    let mut row_y = table_top - header_h - 14.0;
    for item in &model.items {
        if row_y < table_bottom + 15.0 {
            break;
        }
        pdf.text(&item.sl_no.to_string(), left + 8.0, row_y, "/F1", 8.0);

        let desc_trimmed = if item.description.len() > 30 {
            format!("{}...", &item.description[..27])
        } else {
            item.description.clone()
        };
        pdf.text(&desc_trimmed, col_sl + 6.0, row_y, "/F1", 8.0);
        pdf.text(&item.hsn_sac, col_desc + 4.0, row_y, "/F1", 8.0);
        pdf.text_right(
            &format!("{:.2} {}", item.quantity, item.unit),
            col_qty - 4.0,
            row_y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(item.rate),
            col_rate - 4.0,
            row_y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(item.taxable_value),
            col_taxable - 4.0,
            row_y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format!("{:.0}%", item.gst_rate),
            col_rate_gst - 4.0,
            row_y,
            "/F1",
            8.0,
        );
        pdf.text_right(
            &format_indian_currency(item.total),
            col_total - 4.0,
            row_y,
            "/F1",
            8.0,
        );

        row_y -= 14.0;
    }

    // Box 4: HSN/SAC Tax Summary Table
    let hsn_table_top = table_bottom - 10.0;
    let hsn_table_h = 90.0;
    let hsn_table_bottom = hsn_table_top - hsn_table_h;
    pdf.rect(left, hsn_table_bottom, width, hsn_table_h, 0.75);

    // HSN Header
    let hsn_hdr_h = 16.0;
    pdf.filled_rect(
        left,
        hsn_table_top - hsn_hdr_h,
        width,
        hsn_hdr_h,
        0.95,
        0.75,
    );
    pdf.text(
        "HSN/SAC Tax Breakup",
        left + 8.0,
        hsn_table_top - 11.5,
        "/F2",
        8.0,
    );

    let hcol1 = left + 80.0;
    let hcol2 = left + 175.0;
    let hcol3 = left + 265.0;
    let hcol4 = left + 355.0;
    let hcol5 = left + 445.0;
    let hcol_end = right;

    pdf.text("HSN/SAC", left + 8.0, hsn_table_top - 11.5, "/F2", 7.5);
    pdf.text_right(
        "Taxable Value",
        hcol2 - 4.0,
        hsn_table_top - 11.5,
        "/F2",
        7.5,
    );
    pdf.text_right(
        "Central Tax (CGST)",
        hcol3 - 4.0,
        hsn_table_top - 11.5,
        "/F2",
        7.5,
    );
    pdf.text_right(
        "State Tax (SGST)",
        hcol4 - 4.0,
        hsn_table_top - 11.5,
        "/F2",
        7.5,
    );
    pdf.text_right(
        "Integrated Tax (IGST)",
        hcol5 - 4.0,
        hsn_table_top - 11.5,
        "/F2",
        7.5,
    );
    pdf.text_right(
        "Total Tax",
        hcol_end - 4.0,
        hsn_table_top - 11.5,
        "/F2",
        7.5,
    );

    for &hx in &[hcol1, hcol2, hcol3, hcol4, hcol5] {
        pdf.line(hx, hsn_table_bottom, hx, hsn_table_top, 0.5);
    }

    let mut hrow_y = hsn_table_top - hsn_hdr_h - 13.0;
    for h in &model.hsn_summary {
        if hrow_y < hsn_table_bottom + 12.0 {
            break;
        }
        pdf.text(&h.hsn_sac, left + 8.0, hrow_y, "/F1", 7.5);
        pdf.text_right(
            &format_indian_currency(h.taxable_value),
            hcol2 - 4.0,
            hrow_y,
            "/F1",
            7.5,
        );
        pdf.text_right(
            &format!(
                "{} ({:.1}%)",
                format_indian_currency(h.cgst_amount),
                h.cgst_rate
            ),
            hcol3 - 4.0,
            hrow_y,
            "/F1",
            7.5,
        );
        pdf.text_right(
            &format!(
                "{} ({:.1}%)",
                format_indian_currency(h.sgst_amount),
                h.sgst_rate
            ),
            hcol4 - 4.0,
            hrow_y,
            "/F1",
            7.5,
        );
        pdf.text_right(
            &format!(
                "{} ({:.1}%)",
                format_indian_currency(h.igst_amount),
                h.igst_rate
            ),
            hcol5 - 4.0,
            hrow_y,
            "/F1",
            7.5,
        );
        pdf.text_right(
            &format_indian_currency(h.total_tax),
            hcol_end - 4.0,
            hrow_y,
            "/F1",
            7.5,
        );
        hrow_y -= 13.0;
    }

    // Box 5: Totals, Amount in Words, Declaration & Signatory
    let bottom_box_top = hsn_table_bottom - 10.0;
    let bottom_box_h = 100.0;
    let bottom_box_y = bottom_box_top - bottom_box_h;
    pdf.rect(left, bottom_box_y, width, bottom_box_h, 0.75);

    // Horizontal split in bottom box
    let words_divider_y = bottom_box_top - 42.0;
    pdf.line(left, words_divider_y, right, words_divider_y, 0.75);

    // Left of top split: Amount in Words
    let totals_mid_x = right - 190.0;
    pdf.line(
        totals_mid_x,
        words_divider_y,
        totals_mid_x,
        bottom_box_top,
        0.75,
    );

    pdf.text(
        "Invoice Amount in Words:",
        left + 8.0,
        bottom_box_top - 14.0,
        "/F2",
        8.0,
    );
    pdf.text(
        &model.amount_in_words,
        left + 8.0,
        bottom_box_top - 28.0,
        "/F2",
        8.5,
    );

    // Right of top split: Grand Total
    pdf.text(
        "Grand Total:",
        totals_mid_x + 8.0,
        bottom_box_top - 18.0,
        "/F1",
        9.0,
    );
    pdf.text_right(
        &format!("INR {}", format_indian_currency(model.grand_total)),
        right - 8.0,
        bottom_box_top - 18.0,
        "/F2",
        11.5,
    );
    pdf.text(
        &format!("Tax: INR {}", format_indian_currency(model.total_tax)),
        totals_mid_x + 8.0,
        bottom_box_top - 34.0,
        "/F1",
        8.0,
    );

    // Lower half: Terms & Declaration (Left) and Signatory (Right)
    let sig_split_x = right - 180.0;
    pdf.line(
        sig_split_x,
        bottom_box_y,
        sig_split_x,
        words_divider_y,
        0.75,
    );

    pdf.text(
        "Declaration:",
        left + 8.0,
        words_divider_y - 12.0,
        "/F2",
        8.0,
    );
    pdf.text(
        "We declare that this invoice shows the actual price of the goods or services described",
        left + 8.0,
        words_divider_y - 24.0,
        "/F1",
        7.5,
    );
    pdf.text(
        "and that all particulars are true and correct under Indian statutory laws.",
        left + 8.0,
        words_divider_y - 35.0,
        "/F1",
        7.5,
    );

    // Signatory block
    pdf.text(
        &format!("For {}", model.supplier_name),
        sig_split_x + 8.0,
        words_divider_y - 14.0,
        "/F2",
        8.0,
    );
    pdf.text(
        "Authorised Signatory",
        sig_split_x + 8.0,
        bottom_box_y + 12.0,
        "/F1",
        8.0,
    );

    pdf.build()
}

/// Gathers full statutory data for a voucher and exports the invoice PDF file to output_path.
pub async fn export_invoice_pdf_file(
    pool: &Pool<Sqlite>,
    company_id: &str,
    voucher_id: &str,
    output_path: &str,
) -> Result<String, OpenTallyError> {
    let pdf_bytes = generate_invoice_pdf_bytes(pool, company_id, voucher_id).await?;

    let p = Path::new(output_path);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(p, &pdf_bytes).map_err(|e| {
        OpenTallyError::Validation(format!(
            "Failed to write invoice PDF to '{}': {}",
            output_path, e
        ))
    })?;

    Ok(output_path.to_string())
}

/// Generates PDF binary bytes for a voucher inside ReadOnlyConnectionGuard.
pub async fn generate_invoice_pdf_bytes(
    pool: &Pool<Sqlite>,
    company_id: &str,
    voucher_id: &str,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = company_id.to_string();
    let vid = voucher_id.to_string();

    execute_read_only_query(pool, move |conn| {
        let cid = cid.clone();
        let vid = vid.clone();
        Box::pin(async move {
            // 1. Fetch Company
            let company_row: Option<(String, String, String, String)> = sqlx::query_as(
                "SELECT name, mailing_name, state, currency_symbol FROM companies WHERE id = ?",
            )
            .bind(&cid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (comp_name, comp_mailing, comp_state, _curr_sym) = company_row.ok_or_else(|| {
                OpenTallyError::NotFound(format!("Company '{}' not found", cid))
            })?;

            // 2. Fetch Company GST Profile
            let comp_gst: Option<(String, String)> = sqlx::query_as(
                "SELECT COALESCE(gstin, ''), state_code FROM company_gst_profiles WHERE company_id = ?",
            )
            .bind(&cid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (comp_gstin, comp_state_code) = comp_gst.unwrap_or_else(|| ("".to_string(), "01".to_string()));

            // 3. Fetch Voucher Header
            #[allow(clippy::type_complexity)]
            let voucher_row: Option<(String, String, String, Option<String>, Option<String>, String, String)> = sqlx::query_as(
                "SELECT id, voucher_type_id, voucher_number, reference_no, reference_date, date, status
                 FROM vouchers WHERE company_id = ? AND id = ?",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (_v_id, v_type, v_num, _ref_no, _ref_dt, v_date, _v_status) = voucher_row.ok_or_else(|| {
                OpenTallyError::NotFound(format!("Voucher '{}' not found in company '{}'", vid, cid))
            })?;

            let title = match v_type.as_str() {
                "vt_sales" => "TAX INVOICE".to_string(),
                "vt_credit_note" => "CREDIT NOTE".to_string(),
                "vt_debit_note" => "DEBIT NOTE".to_string(),
                _ => "TAX INVOICE".to_string(),
            };

            // 4. Note linkage (if Credit/Debit note)
            let note_link: Option<(String, String)> = sqlx::query_as(
                "SELECT original_invoice_number, original_invoice_date
                 FROM commercial_note_linkages WHERE company_id = ? AND note_voucher_id = ?",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (orig_no, orig_dt) = match note_link {
                Some((n, d)) => (Some(n), Some(d)),
                None => (None, None),
            };

            // 5. Fetch GST Details & Party
            #[derive(sqlx::FromRow)]
            struct GstDetailRow {
                #[allow(dead_code)]
                party_ledger_id: String,
                party_name: String,
                party_gstin: Option<String>,
                place_of_supply_state_code: String,
                #[allow(dead_code)]
                supply_type: String,
                is_reverse_charge: i64,
                hsn_sac_code: String,
                taxable_value: String,
                gst_rate: String,
                cgst_amount: String,
                sgst_amount: String,
                igst_amount: String,
                cess_amount: String,
            }

            let gst_rows = sqlx::query_as::<_, GstDetailRow>(
                "SELECT vgd.party_ledger_id, l.name as party_name, vgd.party_gstin,
                        vgd.place_of_supply_state_code, vgd.supply_type, vgd.is_reverse_charge,
                        vgd.hsn_sac_code, vgd.taxable_value, vgd.gst_rate,
                        vgd.cgst_amount, vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount
                 FROM voucher_gst_details vgd
                 JOIN ledgers l ON l.company_id = vgd.company_id AND l.id = vgd.party_ledger_id
                 WHERE vgd.company_id = ? AND vgd.voucher_id = ?
                 ORDER BY vgd.created_at ASC",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let mut buyer_name = "Cash / Counter Customer".to_string();
            let mut buyer_gstin = "".to_string();
            let mut buyer_state_code = comp_state_code.clone();
            let mut place_of_supply = comp_state_code.clone();
            let mut is_reverse_charge = false;

            if let Some(first) = gst_rows.first() {
                buyer_name = first.party_name.clone();
                buyer_gstin = first.party_gstin.clone().unwrap_or_default();
                buyer_state_code = first.place_of_supply_state_code.clone();
                place_of_supply = format!("State Code {}", first.place_of_supply_state_code);
                is_reverse_charge = first.is_reverse_charge == 1;
            } else {
                // Fallback: check party from voucher lines
                let p_name: Option<String> = sqlx::query_scalar(
                    "SELECT l.name FROM voucher_lines vl
                     JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                     WHERE vl.company_id = ? AND vl.voucher_id = ?
                     ORDER BY vl.line_number ASC LIMIT 1",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if let Some(pn) = p_name {
                    buyer_name = pn;
                }
            }

            // 6. Fetch Items (from stock voucher lines if available, else from GST details or voucher lines)
            #[derive(sqlx::FromRow)]
            struct StockLineRow {
                description: String,
                quantity: String,
                unit_name: Option<String>,
                unit_cost: String,
                line_value: String,
            }

            let stock_rows = sqlx::query_as::<_, StockLineRow>(
                "SELECT svl.description, svl.quantity, u.symbol as unit_name, svl.unit_cost, svl.line_value
                 FROM stock_vouchers sv
                 JOIN stock_voucher_lines svl ON svl.voucher_id = sv.id AND svl.company_id = sv.company_id
                 LEFT JOIN units u ON u.company_id = svl.company_id AND u.id = svl.unit_id
                 WHERE sv.company_id = ? AND sv.reference_no = ?
                 ORDER BY svl.line_number ASC",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let mut items = Vec::new();
            let mut total_taxable = Decimal::ZERO;
            let mut total_tax = Decimal::ZERO;

            if !stock_rows.is_empty() {
                for (idx, sl) in stock_rows.iter().enumerate() {
                    let qty = Decimal::from_str_exact(&sl.quantity).unwrap_or(Decimal::ONE);
                    let rate = Decimal::from_str_exact(&sl.unit_cost).unwrap_or(Decimal::ZERO);
                    let val = Decimal::from_str_exact(&sl.line_value).unwrap_or(qty * rate);

                    // Match GST rate from gst_rows
                    let gst_info = gst_rows.get(idx);
                    let gst_rate = gst_info
                        .and_then(|g| Decimal::from_str_exact(&g.gst_rate).ok())
                        .unwrap_or(Decimal::ZERO);
                    let hsn = gst_info
                        .map(|g| g.hsn_sac_code.clone())
                        .unwrap_or_else(|| "GEN-001".to_string());

                    let tax = if let Some(g) = gst_info {
                        let c = Decimal::from_str_exact(&g.cgst_amount).unwrap_or(Decimal::ZERO);
                        let s = Decimal::from_str_exact(&g.sgst_amount).unwrap_or(Decimal::ZERO);
                        let i = Decimal::from_str_exact(&g.igst_amount).unwrap_or(Decimal::ZERO);
                        let cs = Decimal::from_str_exact(&g.cess_amount).unwrap_or(Decimal::ZERO);
                        let line_sum = c + s + i + cs;
                        if line_sum > Decimal::ZERO {
                            line_sum
                        } else {
                            (val * gst_rate / Decimal::new(100, 0)).round_dp(2)
                        }
                    } else {
                        (val * gst_rate / Decimal::new(100, 0)).round_dp(2)
                    };
                    total_taxable += val;
                    total_tax += tax;

                    items.push(InvoicePdfItemLine {
                        sl_no: idx + 1,
                        description: sl.description.clone(),
                        hsn_sac: hsn,
                        quantity: qty,
                        unit: sl.unit_name.clone().unwrap_or_else(|| "Nos".to_string()),
                        rate,
                        discount: Decimal::ZERO,
                        taxable_value: val,
                        gst_rate,
                        tax_amount: tax,
                        total: val + tax,
                    });
                }
            } else if !gst_rows.is_empty() {
                for (idx, gr) in gst_rows.iter().enumerate() {
                    let val = Decimal::from_str_exact(&gr.taxable_value).unwrap_or(Decimal::ZERO);
                    let gr_rate = Decimal::from_str_exact(&gr.gst_rate).unwrap_or(Decimal::ZERO);
                    let c = Decimal::from_str_exact(&gr.cgst_amount).unwrap_or(Decimal::ZERO);
                    let s = Decimal::from_str_exact(&gr.sgst_amount).unwrap_or(Decimal::ZERO);
                    let i = Decimal::from_str_exact(&gr.igst_amount).unwrap_or(Decimal::ZERO);
                    let cs = Decimal::from_str_exact(&gr.cess_amount).unwrap_or(Decimal::ZERO);
                    let tax = c + s + i + cs;

                    total_taxable += val;
                    total_tax += tax;

                    items.push(InvoicePdfItemLine {
                        sl_no: idx + 1,
                        description: format!("Item / Service ({})", gr.hsn_sac_code),
                        hsn_sac: gr.hsn_sac_code.clone(),
                        quantity: Decimal::ONE,
                        unit: "Nos".to_string(),
                        rate: val,
                        discount: Decimal::ZERO,
                        taxable_value: val,
                        gst_rate: gr_rate,
                        tax_amount: tax,
                        total: val + tax,
                    });
                }
            } else {
                // Pure ledger lines
                #[derive(sqlx::FromRow)]
                struct VLine {
                    ledger_name: String,
                    amount: String,
                    entry_type: String,
                }
                let v_lines = sqlx::query_as::<_, VLine>(
                    "SELECT l.name as ledger_name, vl.amount, vl.entry_type
                     FROM voucher_lines vl
                     JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                     WHERE vl.company_id = ? AND vl.voucher_id = ?
                     ORDER BY vl.line_number ASC",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut sl = 1;
                for l in v_lines {
                    let amt = Decimal::from_str_exact(&l.amount).unwrap_or(Decimal::ZERO);
                    if l.entry_type == "Cr" && v_type == "vt_sales" {
                        total_taxable += amt;
                        items.push(InvoicePdfItemLine {
                            sl_no: sl,
                            description: l.ledger_name,
                            hsn_sac: "999999".to_string(),
                            quantity: Decimal::ONE,
                            unit: "Nos".to_string(),
                            rate: amt,
                            discount: Decimal::ZERO,
                            taxable_value: amt,
                            gst_rate: Decimal::ZERO,
                            tax_amount: Decimal::ZERO,
                            total: amt,
                        });
                        sl += 1;
                    }
                }
            }

            // 7. Aggregate HSN Tax Summary
            struct HsnBucket {
                taxable: Decimal,
                cgst_rate: Decimal,
                cgst_amt: Decimal,
                sgst_rate: Decimal,
                sgst_amt: Decimal,
                igst_rate: Decimal,
                igst_amt: Decimal,
                cess_amt: Decimal,
            }

            let mut hsn_map: HashMap<String, HsnBucket> = HashMap::new();
            for gr in &gst_rows {
                let val = Decimal::from_str_exact(&gr.taxable_value).unwrap_or(Decimal::ZERO);
                let rate = Decimal::from_str_exact(&gr.gst_rate).unwrap_or(Decimal::ZERO);
                let c = Decimal::from_str_exact(&gr.cgst_amount).unwrap_or(Decimal::ZERO);
                let s = Decimal::from_str_exact(&gr.sgst_amount).unwrap_or(Decimal::ZERO);
                let i = Decimal::from_str_exact(&gr.igst_amount).unwrap_or(Decimal::ZERO);
                let cs = Decimal::from_str_exact(&gr.cess_amount).unwrap_or(Decimal::ZERO);

                let half_rate = (rate / Decimal::new(2, 0)).round_dp(2);
                let (c_rate, s_rate, i_rate) = if i > Decimal::ZERO {
                    (Decimal::ZERO, Decimal::ZERO, rate)
                } else {
                    (half_rate, half_rate, Decimal::ZERO)
                };

                if let Some(entry) = hsn_map.get_mut(&gr.hsn_sac_code) {
                    entry.taxable += val;
                    entry.cgst_amt += c;
                    entry.sgst_amt += s;
                    entry.igst_amt += i;
                    entry.cess_amt += cs;
                } else {
                    hsn_map.insert(
                        gr.hsn_sac_code.clone(),
                        HsnBucket {
                            taxable: val,
                            cgst_rate: c_rate,
                            cgst_amt: c,
                            sgst_rate: s_rate,
                            sgst_amt: s,
                            igst_rate: i_rate,
                            igst_amt: i,
                            cess_amt: cs,
                        },
                    );
                }
            }

            let mut hsn_summary = Vec::new();
            for (code, b) in hsn_map {
                let tot = b.cgst_amt + b.sgst_amt + b.igst_amt + b.cess_amt;
                hsn_summary.push(InvoiceHsnTaxRow {
                    hsn_sac: code,
                    taxable_value: b.taxable,
                    cgst_rate: b.cgst_rate,
                    cgst_amount: b.cgst_amt,
                    sgst_rate: b.sgst_rate,
                    sgst_amount: b.sgst_amt,
                    igst_rate: b.igst_rate,
                    igst_amount: b.igst_amt,
                    cess_amount: b.cess_amt,
                    total_tax: tot,
                });
            }

            let grand_total = total_taxable + total_tax;
            let words = number_to_indian_words(grand_total);

            // Fetch e-Invoice and e-Way Bill metadata if present
            #[derive(sqlx::FromRow)]
            struct EinvoiceMetaRow {
                irn: Option<String>,
                ack_no: Option<String>,
                ack_date: Option<String>,
                eway_bill_no: Option<String>,
                eway_valid_upto: Option<String>,
            }

            let einvoice_meta: Option<EinvoiceMetaRow> = sqlx::query_as(
                "SELECT irn, ack_no, ack_date, eway_bill_no, eway_valid_upto
                 FROM voucher_einvoice_details
                 WHERE company_id = ? AND voucher_id = ?",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_optional(&mut *conn)
            .await
            .unwrap_or(None);

            let (irn, ack_no, ack_date, eway_bill_no, eway_valid_upto) = match einvoice_meta {
                Some(m) => (m.irn, m.ack_no, m.ack_date, m.eway_bill_no, m.eway_valid_upto),
                None => (None, None, None, None, None),
            };

            let model = InvoicePdfModel {
                title,
                voucher_id: vid,
                voucher_number: v_num,
                voucher_date: v_date,
                original_invoice_no: orig_no,
                original_invoice_date: orig_dt,
                place_of_supply,
                is_reverse_charge,
                supplier_name: comp_name,
                supplier_address: comp_mailing,
                supplier_gstin: comp_gstin,
                supplier_state: comp_state,
                supplier_state_code: comp_state_code,
                buyer_name,
                buyer_gstin,
                buyer_state: "India".to_string(),
                buyer_state_code,
                items,
                hsn_summary,
                total_taxable,
                total_tax,
                grand_total,
                amount_in_words: words,
                irn,
                ack_no,
                ack_date,
                eway_bill_no,
                eway_valid_upto,
            };

            let bytes = generate_invoice_pdf(&model);
            Ok(bytes)
        })
    })
    .await
}
