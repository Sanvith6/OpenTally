use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[inline]
fn fmt_2dp(d: Decimal) -> String {
    format!("{:.2}", d)
}

// -----------------------------------------------------------------------------
// GSTR-1 Models
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2bInvoiceRow {
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub party_gstin: String,
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub invoice_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2cLargeRow {
    pub voucher_id: String,
    pub voucher_number: String,
    pub voucher_date: String,
    pub party_ledger_id: String,
    pub party_name: String,
    pub place_of_supply_state_code: String,
    pub taxable_value: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub invoice_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1B2cSmallRow {
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub gst_rate: String,
    pub taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub total_tax: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1HsnSummaryRow {
    pub hsn_sac_code: String,
    pub description: Option<String>,
    pub total_taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub total_tax: String,
    pub total_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1CdnrRow {
    pub note_voucher_id: String,
    pub note_voucher_number: String,
    pub note_date: String,
    pub note_type: String, // "C" for Credit Note, "D" for Debit Note
    pub original_invoice_number: String,
    pub original_invoice_date: String,
    pub party_name: String,
    pub party_gstin: String,
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub note_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1CdnurRow {
    pub note_voucher_id: String,
    pub note_voucher_number: String,
    pub note_date: String,
    pub note_type: String, // "C" for Credit Note, "D" for Debit Note
    pub original_invoice_number: String,
    pub original_invoice_date: String,
    pub party_name: String,
    pub place_of_supply_state_code: String,
    pub supply_type: String,
    pub taxable_value: String,
    pub cgst_amount: String,
    pub sgst_amount: String,
    pub igst_amount: String,
    pub cess_amount: String,
    pub note_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1SummaryReport {
    pub company_id: String,
    pub from_date: String,
    pub to_date: String,
    pub total_taxable_value: String,
    pub total_cgst: String,
    pub total_sgst: String,
    pub total_igst: String,
    pub total_cess: String,
    pub total_tax: String,
    pub total_invoice_value: String,
    pub b2b_invoices: Vec<Gstr1B2bInvoiceRow>,
    pub b2c_large_invoices: Vec<Gstr1B2cLargeRow>,
    pub b2c_small_summary: Vec<Gstr1B2cSmallRow>,
    pub hsn_summary: Vec<Gstr1HsnSummaryRow>,
    pub cdnr_table_9b: Vec<Gstr1CdnrRow>,
    pub cdnur_table_9b: Vec<Gstr1CdnurRow>,
}

// -----------------------------------------------------------------------------
// GSTR-3B Models
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bSection31Row {
    pub nature_of_supplies: String,
    pub total_taxable_value: String,
    pub integrated_tax: String,
    pub central_tax: String,
    pub state_ut_tax: String,
    pub cess: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bSection4ItcRow {
    pub itc_details: String,
    pub integrated_tax: String,
    pub central_tax: String,
    pub state_ut_tax: String,
    pub cess: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bNetPayableSummary {
    pub integrated_tax: String,
    pub central_tax: String,
    pub state_ut_tax: String,
    pub cess: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bSummaryReport {
    pub company_id: String,
    pub from_date: String,
    pub to_date: String,
    pub table_3_1: Vec<Gstr3bSection31Row>,
    pub table_4_itc: Vec<Gstr3bSection4ItcRow>,
    pub total_output_tax: Gstr3bNetPayableSummary,
    pub total_eligible_itc: Gstr3bNetPayableSummary,
    pub net_tax_payable: Gstr3bNetPayableSummary,
    pub closing_itc_balance: Gstr3bNetPayableSummary,
}

pub struct GstReportService;

impl GstReportService {
    /// Generates statutory GSTR-1 Outward Supplies Summary report using ReadOnlyConnectionGuard.
    pub async fn get_gstr1_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Gstr1SummaryReport, OpenTallyError> {
        let _ = NaiveDate::parse_from_str(from_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid from_date '{}', expected YYYY-MM-DD",
                from_date
            ))
        })?;
        let _ = NaiveDate::parse_from_str(to_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid to_date '{}', expected YYYY-MM-DD",
                to_date
            ))
        })?;

        let company_id_str = company_id.to_string();
        let from_date_str = from_date.to_string();
        let to_date_str = to_date.to_string();

        execute_read_only_query(pool, move |conn| {
            let company_id_str = company_id_str.clone();
            let from_date_str = from_date_str.clone();
            let to_date_str = to_date_str.clone();

            Box::pin(async move {
                #[derive(Debug, Clone, sqlx::FromRow)]
                struct RawGstLine {
                    voucher_id: String,
                    voucher_number: String,
                    voucher_date: String,
                    party_ledger_id: String,
                    party_name: String,
                    party_gstin: Option<String>,
                    hsn_sac_code: String,
                    place_of_supply_state_code: String,
                    supply_type: String,
                    #[allow(dead_code)]
                    taxability: String,
                    taxable_value: String,
                    gst_rate: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                }

                let lines = sqlx::query_as::<_, RawGstLine>(
                    "SELECT v.id as voucher_id, v.voucher_number, v.date as voucher_date,
                            vgd.party_ledger_id, l.name as party_name, vgd.party_gstin,
                            vgd.hsn_sac_code, vgd.place_of_supply_state_code, vgd.supply_type,
                            vgd.taxability, vgd.taxable_value, vgd.gst_rate,
                            vgd.cgst_amount, vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount
                     FROM voucher_gst_details vgd
                     JOIN vouchers v ON v.company_id = vgd.company_id AND v.id = vgd.voucher_id
                     JOIN ledgers l ON l.company_id = vgd.company_id AND l.id = vgd.party_ledger_id
                     WHERE vgd.company_id = ?
                       AND v.voucher_type_id = 'vt_sales'
                       AND vgd.is_cancelled = 0
                       AND v.status = 'posted'
                       AND v.date >= ? AND v.date <= ?
                     ORDER BY v.date ASC, v.voucher_number ASC",
                )
                .bind(&company_id_str)
                .bind(&from_date_str)
                .bind(&to_date_str)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                // Aggregate by voucher to classify B2B vs B2C Large vs B2C Small
                struct VoucherAgg {
                    voucher_number: String,
                    voucher_date: String,
                    party_ledger_id: String,
                    party_name: String,
                    party_gstin: Option<String>,
                    pos_state: String,
                    supply_type: String,
                    taxable: Decimal,
                    cgst: Decimal,
                    sgst: Decimal,
                    igst: Decimal,
                    cess: Decimal,
                }

                #[derive(Default)]
                struct TaxBucket {
                    taxable: Decimal,
                    cgst: Decimal,
                    sgst: Decimal,
                    igst: Decimal,
                    cess: Decimal,
                }

                #[derive(Hash, PartialEq, Eq)]
                struct B2cSmallKey {
                    pos: String,
                    supply_type: String,
                    rate: String,
                }

                let mut v_map: HashMap<String, VoucherAgg> = HashMap::new();
                let mut v_lines: HashMap<String, Vec<RawGstLine>> = HashMap::new();
                let mut v_order: Vec<String> = Vec::new();

                let mut hsn_map: HashMap<String, TaxBucket> = HashMap::new();
                let mut b2c_small_map: HashMap<B2cSmallKey, TaxBucket> = HashMap::new();

                let mut grand_taxable = Decimal::ZERO;
                let mut grand_cgst = Decimal::ZERO;
                let mut grand_sgst = Decimal::ZERO;
                let mut grand_igst = Decimal::ZERO;
                let mut grand_cess = Decimal::ZERO;

                for l in lines {
                    let taxable =
                        Decimal::from_str_exact(&l.taxable_value).unwrap_or(Decimal::ZERO);
                    let cgst = Decimal::from_str_exact(&l.cgst_amount).unwrap_or(Decimal::ZERO);
                    let sgst = Decimal::from_str_exact(&l.sgst_amount).unwrap_or(Decimal::ZERO);
                    let igst = Decimal::from_str_exact(&l.igst_amount).unwrap_or(Decimal::ZERO);
                    let cess = Decimal::from_str_exact(&l.cess_amount).unwrap_or(Decimal::ZERO);

                    grand_taxable += taxable;
                    grand_cgst += cgst;
                    grand_sgst += sgst;
                    grand_igst += igst;
                    grand_cess += cess;

                    // HSN Aggregation
                    let h_entry = hsn_map.entry(l.hsn_sac_code.clone()).or_default();
                    h_entry.taxable += taxable;
                    h_entry.cgst += cgst;
                    h_entry.sgst += sgst;
                    h_entry.igst += igst;
                    h_entry.cess += cess;

                    if !v_map.contains_key(&l.voucher_id) {
                        v_order.push(l.voucher_id.clone());
                    }
                    let v_entry = v_map.entry(l.voucher_id.clone()).or_insert(VoucherAgg {
                        voucher_number: l.voucher_number.clone(),
                        voucher_date: l.voucher_date.clone(),
                        party_ledger_id: l.party_ledger_id.clone(),
                        party_name: l.party_name.clone(),
                        party_gstin: l.party_gstin.clone().filter(|g| !g.trim().is_empty()),
                        pos_state: l.place_of_supply_state_code.clone(),
                        supply_type: l.supply_type.clone(),
                        taxable: Decimal::ZERO,
                        cgst: Decimal::ZERO,
                        sgst: Decimal::ZERO,
                        igst: Decimal::ZERO,
                        cess: Decimal::ZERO,
                    });
                    v_entry.taxable += taxable;
                    v_entry.cgst += cgst;
                    v_entry.sgst += sgst;
                    v_entry.igst += igst;
                    v_entry.cess += cess;

                    v_lines.entry(l.voucher_id.clone()).or_default().push(l);
                }

                let mut b2b_invoices = Vec::new();
                let mut b2c_large_invoices = Vec::new();

                let b2c_large_threshold = Decimal::from(250_000);

                for vid in v_order {
                    if let Some(v) = v_map.remove(&vid) {
                        let total_tax = v.cgst + v.sgst + v.igst + v.cess;
                        let invoice_val = v.taxable + total_tax;

                        if let Some(ref gstin) = v.party_gstin {
                            b2b_invoices.push(Gstr1B2bInvoiceRow {
                                voucher_id: vid,
                                voucher_number: v.voucher_number,
                                voucher_date: v.voucher_date,
                                party_ledger_id: v.party_ledger_id,
                                party_name: v.party_name,
                                party_gstin: gstin.clone(),
                                place_of_supply_state_code: v.pos_state,
                                supply_type: v.supply_type,
                                taxable_value: fmt_2dp(v.taxable),
                                cgst_amount: fmt_2dp(v.cgst),
                                sgst_amount: fmt_2dp(v.sgst),
                                igst_amount: fmt_2dp(v.igst),
                                cess_amount: fmt_2dp(v.cess),
                                invoice_value: fmt_2dp(invoice_val),
                            });
                        } else if v.supply_type == "InterState" && invoice_val > b2c_large_threshold
                        {
                            b2c_large_invoices.push(Gstr1B2cLargeRow {
                                voucher_id: vid,
                                voucher_number: v.voucher_number,
                                voucher_date: v.voucher_date,
                                party_ledger_id: v.party_ledger_id,
                                party_name: v.party_name,
                                place_of_supply_state_code: v.pos_state,
                                taxable_value: fmt_2dp(v.taxable),
                                igst_amount: fmt_2dp(v.igst),
                                cess_amount: fmt_2dp(v.cess),
                                invoice_value: fmt_2dp(invoice_val),
                            });
                        } else if let Some(lines_for_v) = v_lines.remove(&vid) {
                            // B2C Small: Unregistered intra-state, or unregistered inter-state <= 2.5L
                            for line in lines_for_v {
                                let taxable = Decimal::from_str_exact(&line.taxable_value)
                                    .unwrap_or(Decimal::ZERO);
                                let cgst = Decimal::from_str_exact(&line.cgst_amount)
                                    .unwrap_or(Decimal::ZERO);
                                let sgst = Decimal::from_str_exact(&line.sgst_amount)
                                    .unwrap_or(Decimal::ZERO);
                                let igst = Decimal::from_str_exact(&line.igst_amount)
                                    .unwrap_or(Decimal::ZERO);
                                let cess = Decimal::from_str_exact(&line.cess_amount)
                                    .unwrap_or(Decimal::ZERO);

                                let key = B2cSmallKey {
                                    pos: line.place_of_supply_state_code.clone(),
                                    supply_type: line.supply_type.clone(),
                                    rate: line.gst_rate.clone(),
                                };
                                let b_entry = b2c_small_map.entry(key).or_default();
                                b_entry.taxable += taxable;
                                b_entry.cgst += cgst;
                                b_entry.sgst += sgst;
                                b_entry.igst += igst;
                                b_entry.cess += cess;
                            }
                        }
                    }
                }

                let mut b2c_small_summary = Vec::new();
                for (key, b) in b2c_small_map {
                    let total_tax = b.cgst + b.sgst + b.igst + b.cess;
                    b2c_small_summary.push(Gstr1B2cSmallRow {
                        place_of_supply_state_code: key.pos,
                        supply_type: key.supply_type,
                        gst_rate: key.rate,
                        taxable_value: fmt_2dp(b.taxable),
                        cgst_amount: fmt_2dp(b.cgst),
                        sgst_amount: fmt_2dp(b.sgst),
                        igst_amount: fmt_2dp(b.igst),
                        cess_amount: fmt_2dp(b.cess),
                        total_tax: fmt_2dp(total_tax),
                    });
                }

                let mut hsn_summary = Vec::new();
                for (code, h) in hsn_map {
                    let total_tax = h.cgst + h.sgst + h.igst + h.cess;
                    let total_val = h.taxable + total_tax;
                    hsn_summary.push(Gstr1HsnSummaryRow {
                        hsn_sac_code: code,
                        description: None,
                        total_taxable_value: fmt_2dp(h.taxable),
                        cgst_amount: fmt_2dp(h.cgst),
                        sgst_amount: fmt_2dp(h.sgst),
                        igst_amount: fmt_2dp(h.igst),
                        cess_amount: fmt_2dp(h.cess),
                        total_tax: fmt_2dp(total_tax),
                        total_value: fmt_2dp(total_val),
                    });
                }

                let grand_tax = grand_cgst + grand_sgst + grand_igst + grand_cess;
                let grand_invoice_val = grand_taxable + grand_tax;

                #[derive(Debug, Clone, sqlx::FromRow)]
                struct RawNoteLine {
                    voucher_id: String,
                    voucher_number: String,
                    voucher_date: String,
                    voucher_type_id: String,
                    original_invoice_number: Option<String>,
                    original_invoice_date: Option<String>,
                    party_name: String,
                    party_gstin: Option<String>,
                    place_of_supply_state_code: String,
                    supply_type: String,
                    taxable_value: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                }

                let note_rows = sqlx::query_as::<_, RawNoteLine>(
                    "SELECT v.id as voucher_id, v.voucher_number, v.date as voucher_date,
                            v.voucher_type_id,
                            cnl.original_invoice_number, cnl.original_invoice_date,
                            l.name as party_name, vgd.party_gstin,
                            vgd.place_of_supply_state_code, vgd.supply_type,
                            vgd.taxable_value, vgd.cgst_amount, vgd.sgst_amount,
                            vgd.igst_amount, vgd.cess_amount
                     FROM voucher_gst_details vgd
                     JOIN vouchers v ON v.company_id = vgd.company_id AND v.id = vgd.voucher_id
                     JOIN ledgers l ON l.company_id = vgd.company_id AND l.id = vgd.party_ledger_id
                     LEFT JOIN commercial_note_linkages cnl ON cnl.company_id = vgd.company_id AND cnl.note_voucher_id = v.id
                     WHERE vgd.company_id = ?
                       AND v.voucher_type_id IN ('vt_credit_note', 'vt_debit_note')
                       AND vgd.is_cancelled = 0
                       AND v.status = 'posted'
                       AND v.date >= ? AND v.date <= ?
                     ORDER BY v.date ASC, v.voucher_number ASC",
                )
                .bind(&company_id_str)
                .bind(&from_date_str)
                .bind(&to_date_str)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                struct NoteVoucherAgg {
                    voucher_number: String,
                    voucher_date: String,
                    voucher_type_id: String,
                    original_invoice_number: String,
                    original_invoice_date: String,
                    party_name: String,
                    party_gstin: Option<String>,
                    pos_state: String,
                    supply_type: String,
                    taxable: Decimal,
                    cgst: Decimal,
                    sgst: Decimal,
                    igst: Decimal,
                    cess: Decimal,
                }

                let mut note_map: HashMap<String, NoteVoucherAgg> = HashMap::new();
                let mut note_order: Vec<String> = Vec::new();

                for nl in note_rows {
                    let tax_val = Decimal::from_str_exact(&nl.taxable_value).unwrap_or(Decimal::ZERO);
                    let c_val = Decimal::from_str_exact(&nl.cgst_amount).unwrap_or(Decimal::ZERO);
                    let s_val = Decimal::from_str_exact(&nl.sgst_amount).unwrap_or(Decimal::ZERO);
                    let i_val = Decimal::from_str_exact(&nl.igst_amount).unwrap_or(Decimal::ZERO);
                    let cs_val = Decimal::from_str_exact(&nl.cess_amount).unwrap_or(Decimal::ZERO);

                    if let Some(entry) = note_map.get_mut(&nl.voucher_id) {
                        entry.taxable += tax_val;
                        entry.cgst += c_val;
                        entry.sgst += s_val;
                        entry.igst += i_val;
                        entry.cess += cs_val;
                    } else {
                        note_order.push(nl.voucher_id.clone());
                        note_map.insert(
                            nl.voucher_id.clone(),
                            NoteVoucherAgg {
                                voucher_number: nl.voucher_number,
                                voucher_date: nl.voucher_date,
                                voucher_type_id: nl.voucher_type_id,
                                original_invoice_number: nl.original_invoice_number.unwrap_or_else(|| "N/A".to_string()),
                                original_invoice_date: nl.original_invoice_date.unwrap_or_else(|| "N/A".to_string()),
                                party_name: nl.party_name,
                                party_gstin: nl.party_gstin,
                                pos_state: nl.place_of_supply_state_code,
                                supply_type: nl.supply_type,
                                taxable: tax_val,
                                cgst: c_val,
                                sgst: s_val,
                                igst: i_val,
                                cess: cs_val,
                            },
                        );
                    }
                }

                let mut cdnr_table_9b = Vec::new();
                let mut cdnur_table_9b = Vec::new();

                for v_id in note_order {
                    if let Some(agg) = note_map.remove(&v_id) {
                        let total_tax = agg.cgst + agg.sgst + agg.igst + agg.cess;
                        let note_val = agg.taxable + total_tax;
                        let note_type = if agg.voucher_type_id == "vt_credit_note" {
                            "C".to_string()
                        } else {
                            "D".to_string()
                        };

                        let is_registered = agg
                            .party_gstin
                            .as_ref()
                            .map(|g| g.trim().len() == 15)
                            .unwrap_or(false);

                        if is_registered {
                            cdnr_table_9b.push(Gstr1CdnrRow {
                                note_voucher_id: v_id,
                                note_voucher_number: agg.voucher_number,
                                note_date: agg.voucher_date,
                                note_type,
                                original_invoice_number: agg.original_invoice_number,
                                original_invoice_date: agg.original_invoice_date,
                                party_name: agg.party_name,
                                party_gstin: agg.party_gstin.unwrap_or_default(),
                                place_of_supply_state_code: agg.pos_state,
                                supply_type: agg.supply_type,
                                taxable_value: fmt_2dp(agg.taxable),
                                cgst_amount: fmt_2dp(agg.cgst),
                                sgst_amount: fmt_2dp(agg.sgst),
                                igst_amount: fmt_2dp(agg.igst),
                                cess_amount: fmt_2dp(agg.cess),
                                note_value: fmt_2dp(note_val),
                            });
                        } else {
                            cdnur_table_9b.push(Gstr1CdnurRow {
                                note_voucher_id: v_id,
                                note_voucher_number: agg.voucher_number,
                                note_date: agg.voucher_date,
                                note_type,
                                original_invoice_number: agg.original_invoice_number,
                                original_invoice_date: agg.original_invoice_date,
                                party_name: agg.party_name,
                                place_of_supply_state_code: agg.pos_state,
                                supply_type: agg.supply_type,
                                taxable_value: fmt_2dp(agg.taxable),
                                cgst_amount: fmt_2dp(agg.cgst),
                                sgst_amount: fmt_2dp(agg.sgst),
                                igst_amount: fmt_2dp(agg.igst),
                                cess_amount: fmt_2dp(agg.cess),
                                note_value: fmt_2dp(note_val),
                            });
                        }
                    }
                }

                Ok(Gstr1SummaryReport {
                    company_id: company_id_str,
                    from_date: from_date_str,
                    to_date: to_date_str,
                    total_taxable_value: fmt_2dp(grand_taxable),
                    total_cgst: fmt_2dp(grand_cgst),
                    total_sgst: fmt_2dp(grand_sgst),
                    total_igst: fmt_2dp(grand_igst),
                    total_cess: fmt_2dp(grand_cess),
                    total_tax: fmt_2dp(grand_tax),
                    total_invoice_value: fmt_2dp(grand_invoice_val),
                    b2b_invoices,
                    b2c_large_invoices,
                    b2c_small_summary,
                    hsn_summary,
                    cdnr_table_9b,
                    cdnur_table_9b,
                })
            })
        })
        .await
    }

    /// Generates statutory GSTR-3B Monthly Summary report using ReadOnlyConnectionGuard.
    pub async fn get_gstr3b_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<Gstr3bSummaryReport, OpenTallyError> {
        let _ = NaiveDate::parse_from_str(from_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid from_date '{}', expected YYYY-MM-DD",
                from_date
            ))
        })?;
        let _ = NaiveDate::parse_from_str(to_date, "%Y-%m-%d").map_err(|_| {
            OpenTallyError::Validation(format!(
                "Invalid to_date '{}', expected YYYY-MM-DD",
                to_date
            ))
        })?;

        let company_id_str = company_id.to_string();
        let from_date_str = from_date.to_string();
        let to_date_str = to_date.to_string();

        execute_read_only_query(pool, move |conn| {
            let company_id_str = company_id_str.clone();
            let from_date_str = from_date_str.clone();
            let to_date_str = to_date_str.clone();

            Box::pin(async move {
                #[derive(Debug, sqlx::FromRow)]
                struct GstRow {
                    voucher_type_id: String,
                    taxability: String,
                    taxable_value: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                    is_reverse_charge: i64,
                }

                let rows = sqlx::query_as::<_, GstRow>(
                    "SELECT v.voucher_type_id, vgd.taxability, vgd.taxable_value,
                            vgd.cgst_amount, vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount,
                            vgd.is_reverse_charge
                     FROM voucher_gst_details vgd
                     JOIN vouchers v ON v.company_id = vgd.company_id AND v.id = vgd.voucher_id
                     WHERE vgd.company_id = ?
                       AND vgd.is_cancelled = 0
                       AND v.status = 'posted'
                       AND v.date >= ? AND v.date <= ?",
                )
                .bind(&company_id_str)
                .bind(&from_date_str)
                .bind(&to_date_str)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                // 1. Table 3.1 Outward supplies
                let mut out_taxable_base = Decimal::ZERO;
                let mut out_cgst = Decimal::ZERO;
                let mut out_sgst = Decimal::ZERO;
                let mut out_igst = Decimal::ZERO;
                let mut out_cess = Decimal::ZERO;

                let mut out_exempt_base = Decimal::ZERO;

                // 2. Table 4 Inward supplies (ITC)
                let mut itc_cgst = Decimal::ZERO;
                let mut itc_sgst = Decimal::ZERO;
                let mut itc_igst = Decimal::ZERO;
                let mut itc_cess = Decimal::ZERO;

                let mut rcm_cgst = Decimal::ZERO;
                let mut rcm_sgst = Decimal::ZERO;
                let mut rcm_igst = Decimal::ZERO;
                let mut rcm_cess = Decimal::ZERO;

                for r in rows {
                    let taxable = Decimal::from_str_exact(&r.taxable_value).unwrap_or(Decimal::ZERO);
                    let cgst = Decimal::from_str_exact(&r.cgst_amount).unwrap_or(Decimal::ZERO);
                    let sgst = Decimal::from_str_exact(&r.sgst_amount).unwrap_or(Decimal::ZERO);
                    let igst = Decimal::from_str_exact(&r.igst_amount).unwrap_or(Decimal::ZERO);
                    let cess = Decimal::from_str_exact(&r.cess_amount).unwrap_or(Decimal::ZERO);

                    if r.voucher_type_id == "vt_sales" {
                        if r.taxability == "Taxable" {
                            out_taxable_base += taxable;
                            out_cgst += cgst;
                            out_sgst += sgst;
                            out_igst += igst;
                            out_cess += cess;
                        } else {
                            out_exempt_base += taxable;
                        }
                    } else if r.voucher_type_id == "vt_credit_note" {
                        if r.taxability == "Taxable" {
                            out_taxable_base -= taxable;
                            out_cgst -= cgst;
                            out_sgst -= sgst;
                            out_igst -= igst;
                            out_cess -= cess;
                        } else {
                            out_exempt_base -= taxable;
                        }
                    } else if r.voucher_type_id == "vt_pur" {
                        if r.is_reverse_charge == 1 {
                            rcm_cgst += cgst;
                            rcm_sgst += sgst;
                            rcm_igst += igst;
                            rcm_cess += cess;
                        } else {
                            itc_cgst += cgst;
                            itc_sgst += sgst;
                            itc_igst += igst;
                            itc_cess += cess;
                        }
                    } else if r.voucher_type_id == "vt_debit_note" {
                        itc_cgst -= cgst;
                        itc_sgst -= sgst;
                        itc_igst -= igst;
                        itc_cess -= cess;
                    }
                }

                let table_3_1 = vec![
                    Gstr3bSection31Row {
                        nature_of_supplies: "(a) Outward taxable supplies (other than zero rated, nil rated and exempted)".to_string(),
                        total_taxable_value: fmt_2dp(out_taxable_base),
                        integrated_tax: fmt_2dp(out_igst),
                        central_tax: fmt_2dp(out_cgst),
                        state_ut_tax: fmt_2dp(out_sgst),
                        cess: fmt_2dp(out_cess),
                    },
                    Gstr3bSection31Row {
                        nature_of_supplies: "(b) Outward taxable supplies (zero rated)".to_string(),
                        total_taxable_value: "0.00".to_string(),
                        integrated_tax: "0.00".to_string(),
                        central_tax: "0.00".to_string(),
                        state_ut_tax: "0.00".to_string(),
                        cess: "0.00".to_string(),
                    },
                    Gstr3bSection31Row {
                        nature_of_supplies: "(c) Other outward supplies (Nil rated, exempted, Non-GST)".to_string(),
                        total_taxable_value: fmt_2dp(out_exempt_base),
                        integrated_tax: "0.00".to_string(),
                        central_tax: "0.00".to_string(),
                        state_ut_tax: "0.00".to_string(),
                        cess: "0.00".to_string(),
                    },
                    Gstr3bSection31Row {
                        nature_of_supplies: "(d) Inward supplies (liable to reverse charge)".to_string(),
                        total_taxable_value: "0.00".to_string(),
                        integrated_tax: fmt_2dp(rcm_igst),
                        central_tax: fmt_2dp(rcm_cgst),
                        state_ut_tax: fmt_2dp(rcm_sgst),
                        cess: fmt_2dp(rcm_cess),
                    },
                ];

                let table_4_itc = vec![
                    Gstr3bSection4ItcRow {
                        itc_details: "(A)(3) Inward supplies liable to reverse charge".to_string(),
                        integrated_tax: fmt_2dp(rcm_igst),
                        central_tax: fmt_2dp(rcm_cgst),
                        state_ut_tax: fmt_2dp(rcm_sgst),
                        cess: fmt_2dp(rcm_cess),
                    },
                    Gstr3bSection4ItcRow {
                        itc_details: "(A)(5) All other ITC (standard inward purchases)".to_string(),
                        integrated_tax: fmt_2dp(itc_igst),
                        central_tax: fmt_2dp(itc_cgst),
                        state_ut_tax: fmt_2dp(itc_sgst),
                        cess: fmt_2dp(itc_cess),
                    },
                ];

                let total_out_tax = out_cgst + out_sgst + out_igst + out_cess;
                let total_out_summary = Gstr3bNetPayableSummary {
                    integrated_tax: fmt_2dp(out_igst),
                    central_tax: fmt_2dp(out_cgst),
                    state_ut_tax: fmt_2dp(out_sgst),
                    cess: fmt_2dp(out_cess),
                    total: fmt_2dp(total_out_tax),
                };

                let total_itc = itc_cgst + itc_sgst + itc_igst + itc_cess + rcm_cgst + rcm_sgst + rcm_igst + rcm_cess;
                let total_itc_cgst = itc_cgst + rcm_cgst;
                let total_itc_sgst = itc_sgst + rcm_sgst;
                let total_itc_igst = itc_igst + rcm_igst;
                let total_itc_cess = itc_cess + rcm_cess;

                let total_itc_summary = Gstr3bNetPayableSummary {
                    integrated_tax: fmt_2dp(total_itc_igst),
                    central_tax: fmt_2dp(total_itc_cgst),
                    state_ut_tax: fmt_2dp(total_itc_sgst),
                    cess: fmt_2dp(total_itc_cess),
                    total: fmt_2dp(total_itc),
                };

                // Compute Net Tax Payable (Output minus ITC, clamped to 0)
                let net_cgst = (out_cgst - total_itc_cgst).max(Decimal::ZERO);
                let net_sgst = (out_sgst - total_itc_sgst).max(Decimal::ZERO);
                let net_igst = (out_igst - total_itc_igst).max(Decimal::ZERO);
                let net_cess = (out_cess - total_itc_cess).max(Decimal::ZERO);
                let net_total = net_cgst + net_sgst + net_igst + net_cess;

                let net_payable_summary = Gstr3bNetPayableSummary {
                    integrated_tax: fmt_2dp(net_igst),
                    central_tax: fmt_2dp(net_cgst),
                    state_ut_tax: fmt_2dp(net_sgst),
                    cess: fmt_2dp(net_cess),
                    total: fmt_2dp(net_total),
                };

                // Compute Closing ITC Balance (ITC minus Output, clamped to 0)
                let rem_cgst = (total_itc_cgst - out_cgst).max(Decimal::ZERO);
                let rem_sgst = (total_itc_sgst - out_sgst).max(Decimal::ZERO);
                let rem_igst = (total_itc_igst - out_igst).max(Decimal::ZERO);
                let rem_cess = (total_itc_cess - out_cess).max(Decimal::ZERO);
                let rem_total = rem_cgst + rem_sgst + rem_igst + rem_cess;

                let closing_itc_summary = Gstr3bNetPayableSummary {
                    integrated_tax: fmt_2dp(rem_igst),
                    central_tax: fmt_2dp(rem_cgst),
                    state_ut_tax: fmt_2dp(rem_sgst),
                    cess: fmt_2dp(rem_cess),
                    total: fmt_2dp(rem_total),
                };

                Ok(Gstr3bSummaryReport {
                    company_id: company_id_str,
                    from_date: from_date_str,
                    to_date: to_date_str,
                    table_3_1,
                    table_4_itc,
                    total_output_tax: total_out_summary,
                    total_eligible_itc: total_itc_summary,
                    net_tax_payable: net_payable_summary,
                    closing_itc_balance: closing_itc_summary,
                })
            })
        })
        .await
    }

    /// Exports structured GSTR-1 JSON suitable for statutory filing review.
    /// Excludes cancelled vouchers and formats all monetary amounts with canonical 2-decimal precision.
    pub async fn export_gstr1_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_gstr1_summary(pool, company_id, from_date, to_date).await?;

        let gstin: Option<String> =
            sqlx::query_scalar("SELECT gstin FROM company_gst_profiles WHERE company_id = ?")
                .bind(company_id)
                .fetch_optional(pool)
                .await
                .unwrap_or_default()
                .flatten();

        let fp = date_to_fp(to_date);

        let payload = Gstr1JsonPayload {
            gstin: gstin.unwrap_or_else(|| "27AABCU9603R1ZM".to_string()),
            fp,
            b2b: summary.b2b_invoices,
            b2cl: summary.b2c_large_invoices,
            b2cs: summary.b2c_small_summary,
            cdnr: summary.cdnr_table_9b,
            cdnur: summary.cdnur_table_9b,
            hsn: summary.hsn_summary,
            total_taxable_value: summary.total_taxable_value,
            total_tax: summary.total_tax,
            total_invoice_value: summary.total_invoice_value,
        };

        serde_json::to_string_pretty(&payload).map_err(OpenTallyError::Serialization)
    }

    /// Exports structured GSTR-3B JSON suitable for statutory filing review.
    /// Excludes cancelled vouchers and formats all monetary amounts with canonical 2-decimal precision.
    pub async fn export_gstr3b_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        from_date: &str,
        to_date: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_gstr3b_summary(pool, company_id, from_date, to_date).await?;

        let gstin: Option<String> =
            sqlx::query_scalar("SELECT gstin FROM company_gst_profiles WHERE company_id = ?")
                .bind(company_id)
                .fetch_optional(pool)
                .await
                .unwrap_or_default()
                .flatten();

        let ret_period = date_to_fp(to_date);

        let payload = Gstr3bJsonPayload {
            gstin: gstin.unwrap_or_else(|| "27AABCU9603R1ZM".to_string()),
            ret_period,
            sup_details: summary.table_3_1,
            itc_elg: summary.table_4_itc,
            tax_payable: summary.net_tax_payable,
            closing_itc: summary.closing_itc_balance,
        };

        serde_json::to_string_pretty(&payload).map_err(OpenTallyError::Serialization)
    }
}

fn date_to_fp(date_str: &str) -> String {
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() >= 2 {
        format!("{}{}", parts[1], parts[0])
    } else {
        "042026".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr1JsonPayload {
    pub gstin: String,
    pub fp: String,
    pub b2b: Vec<Gstr1B2bInvoiceRow>,
    pub b2cl: Vec<Gstr1B2cLargeRow>,
    pub b2cs: Vec<Gstr1B2cSmallRow>,
    pub cdnr: Vec<Gstr1CdnrRow>,
    pub cdnur: Vec<Gstr1CdnurRow>,
    pub hsn: Vec<Gstr1HsnSummaryRow>,
    pub total_taxable_value: String,
    pub total_tax: String,
    pub total_invoice_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gstr3bJsonPayload {
    pub gstin: String,
    pub ret_period: String,
    pub sup_details: Vec<Gstr3bSection31Row>,
    pub itc_elg: Vec<Gstr3bSection4ItcRow>,
    pub tax_payable: Gstr3bNetPayableSummary,
    pub closing_itc: Gstr3bNetPayableSummary,
}
