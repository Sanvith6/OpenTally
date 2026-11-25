use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

fn iso_to_slash_dmy(iso_date: &str) -> String {
    let clean = iso_date.trim();
    if clean.len() == 10 && clean.chars().nth(4) == Some('-') && clean.chars().nth(7) == Some('-') {
        let y = &clean[0..4];
        let m = &clean[5..7];
        let d = &clean[8..10];
        return format!("{}/{}/{}", d, m, y);
    }
    clean.to_string()
}

// -----------------------------------------------------------------------------
// DTOs for NIC e-Invoice v1.1 Schema
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceTranDtls {
    #[serde(rename = "TaxSch")]
    pub tax_sch: String,
    #[serde(rename = "SupTyp")]
    pub sup_typ: String,
    #[serde(rename = "RegRev")]
    pub reg_rev: String,
    #[serde(rename = "EcmGstin")]
    pub ecm_gstin: Option<String>,
    #[serde(rename = "IgstOnIntra")]
    pub igst_on_intra: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceDocDtls {
    #[serde(rename = "Typ")]
    pub typ: String, // "INV", "CRN", "DBN"
    #[serde(rename = "No")]
    pub no: String,
    #[serde(rename = "Dt")]
    pub dt: String, // "DD/MM/YYYY"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceSellerDtls {
    #[serde(rename = "Gstin")]
    pub gstin: String,
    #[serde(rename = "LglNm")]
    pub lgl_nm: String,
    #[serde(rename = "TrdNm")]
    pub trd_nm: String,
    #[serde(rename = "Addr1")]
    pub addr1: String,
    #[serde(rename = "Loc")]
    pub loc: String,
    #[serde(rename = "Pin")]
    pub pin: i64,
    #[serde(rename = "Stcd")]
    pub stcd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceBuyerDtls {
    #[serde(rename = "Gstin")]
    pub gstin: String,
    #[serde(rename = "LglNm")]
    pub lgl_nm: String,
    #[serde(rename = "TrdNm")]
    pub trd_nm: String,
    #[serde(rename = "Pos")]
    pub pos: String,
    #[serde(rename = "Addr1")]
    pub addr1: String,
    #[serde(rename = "Loc")]
    pub loc: String,
    #[serde(rename = "Pin")]
    pub pin: i64,
    #[serde(rename = "Stcd")]
    pub stcd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceItem {
    #[serde(rename = "SlNo")]
    pub sl_no: String,
    #[serde(rename = "PrdDesc")]
    pub prd_desc: String,
    #[serde(rename = "IsServc")]
    pub is_servc: String,
    #[serde(rename = "HsnCd")]
    pub hsn_cd: String,
    #[serde(rename = "Qty")]
    pub qty: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitPrice")]
    pub unit_price: f64,
    #[serde(rename = "TotAmt")]
    pub tot_amt: f64,
    #[serde(rename = "Discount")]
    pub discount: f64,
    #[serde(rename = "AssAmt")]
    pub ass_amt: f64,
    #[serde(rename = "GstRt")]
    pub gst_rt: f64,
    #[serde(rename = "IgstAmt")]
    pub igst_amt: f64,
    #[serde(rename = "CgstAmt")]
    pub cgst_amt: f64,
    #[serde(rename = "SgstAmt")]
    pub sgst_amt: f64,
    #[serde(rename = "CesAmt")]
    pub ces_amt: f64,
    #[serde(rename = "TotItemVal")]
    pub tot_item_val: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceValDtls {
    #[serde(rename = "AssVal")]
    pub ass_val: f64,
    #[serde(rename = "CgstVal")]
    pub cgst_val: f64,
    #[serde(rename = "SgstVal")]
    pub sgst_val: f64,
    #[serde(rename = "IgstVal")]
    pub igst_val: f64,
    #[serde(rename = "CesVal")]
    pub ces_val: f64,
    #[serde(rename = "RndOffAmt")]
    pub rnd_off_amt: f64,
    #[serde(rename = "TotInvVal")]
    pub tot_inv_val: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinvoiceV11Payload {
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "TranDtls")]
    pub tran_dtls: EinvoiceTranDtls,
    #[serde(rename = "DocDtls")]
    pub doc_dtls: EinvoiceDocDtls,
    #[serde(rename = "SellerDtls")]
    pub seller_dtls: EinvoiceSellerDtls,
    #[serde(rename = "BuyerDtls")]
    pub buyer_dtls: EinvoiceBuyerDtls,
    #[serde(rename = "ItemList")]
    pub item_list: Vec<EinvoiceItem>,
    #[serde(rename = "ValDtls")]
    pub val_dtls: EinvoiceValDtls,
}

// -----------------------------------------------------------------------------
// DTOs for NIC e-Way Bill Schema
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportDetailsInput {
    pub transport_mode: String, // "Road", "Rail", "Air", "Ship", or "1", "2", "3", "4"
    pub distance_km: u32,
    pub transporter_id: Option<String>,
    pub transporter_name: Option<String>,
    pub trans_doc_no: Option<String>,
    pub trans_doc_date: Option<String>,
    pub vehicle_no: Option<String>,
    pub vehicle_type: Option<String>, // "R" (Regular), "O" (ODC)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct EwayBillItem {
    pub itemNo: usize,
    pub productId: usize,
    pub productName: String,
    pub productDesc: String,
    pub hsnCode: i64,
    pub quantity: f64,
    pub qtyUnit: String,
    pub taxableAmount: f64,
    pub sgstRate: f64,
    pub cgstRate: f64,
    pub igstRate: f64,
    pub cessRate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct EwayBillEntry {
    pub userGstin: String,
    pub supplyType: String,
    pub subSupplyType: String,
    pub docType: String,
    pub docNo: String,
    pub docDate: String,
    pub fromGstin: String,
    pub fromTrdName: String,
    pub fromAddr1: String,
    pub fromPlace: String,
    pub fromPincode: i64,
    pub actFromStateCode: i64,
    pub fromStateCode: i64,
    pub toGstin: String,
    pub toTrdName: String,
    pub toAddr1: String,
    pub toPlace: String,
    pub toPincode: i64,
    pub actToStateCode: i64,
    pub toStateCode: i64,
    pub totalValue: f64,
    pub cgstValue: f64,
    pub sgstValue: f64,
    pub igstValue: f64,
    pub cessValue: f64,
    pub totInvValue: f64,
    pub transMode: String,
    pub transDistance: String,
    pub transporterId: Option<String>,
    pub transporterName: Option<String>,
    pub transDocNo: Option<String>,
    pub transDocDate: Option<String>,
    pub vehicleNo: Option<String>,
    pub vehicleType: String,
    pub itemList: Vec<EwayBillItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct EwayBillJsonPayload {
    pub version: String,
    pub billLists: Vec<EwayBillEntry>,
}

// -----------------------------------------------------------------------------
// DTOs for Voucher e-Invoice Details Metadata Persistence
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherEinvoiceMetadataInput {
    pub irn: Option<String>,
    pub ack_no: Option<String>,
    pub ack_date: Option<String>,
    pub signed_qr_code: Option<String>,
    pub einvoice_status: Option<String>,
    pub eway_bill_no: Option<String>,
    pub eway_bill_date: Option<String>,
    pub eway_valid_upto: Option<String>,
    pub transporter_id: Option<String>,
    pub transporter_name: Option<String>,
    pub transport_mode: Option<String>,
    pub vehicle_no: Option<String>,
    pub distance_km: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VoucherEinvoiceMetadataDto {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub irn: Option<String>,
    pub ack_no: Option<String>,
    pub ack_date: Option<String>,
    pub signed_qr_code: Option<String>,
    pub einvoice_status: String,
    pub eway_bill_no: Option<String>,
    pub eway_bill_date: Option<String>,
    pub eway_valid_upto: Option<String>,
    pub transporter_id: Option<String>,
    pub transporter_name: Option<String>,
    pub transport_mode: Option<String>,
    pub vehicle_no: Option<String>,
    pub distance_km: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

// -----------------------------------------------------------------------------
// Service Implementation
// -----------------------------------------------------------------------------

pub struct EinvoiceService;

impl EinvoiceService {
    /// Generate standard NIC e-Invoice v1.1 JSON payload for a posted B2B commercial voucher.
    pub async fn export_einvoice_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<String, OpenTallyError> {
        let cid = company_id.to_string();
        let vid = voucher_id.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let vid = vid.clone();

            Box::pin(async move {
                // 1. Fetch Company & Seller GSTIN
                let company_row: Option<(String, String, String)> = sqlx::query_as(
                    "SELECT name, mailing_name, state FROM companies WHERE id = ?",
                )
                .bind(&cid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (comp_name, comp_mailing, comp_state) = company_row.ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", cid))
                })?;

                let seller_gst_row: Option<(String, String)> = sqlx::query_as(
                    "SELECT COALESCE(gstin, ''), state_code FROM company_gst_profiles WHERE company_id = ?",
                )
                .bind(&cid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (seller_gstin, seller_state_code) = match seller_gst_row {
                    Some((g, s)) if g.trim().len() == 15 => (g.trim().to_uppercase(), s),
                    _ => {
                        return Err(OpenTallyError::Validation(
                            "Company has no valid 15-character GSTIN configured in company_gst_profiles".to_string(),
                        ));
                    }
                };

                // 2. Fetch Voucher Header
                let voucher_row: Option<(String, String, String, String)> = sqlx::query_as(
                    "SELECT voucher_type_id, voucher_number, date, status
                     FROM vouchers WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (v_type, v_num, v_date, v_status) = voucher_row.ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Voucher '{}' not found", vid))
                })?;

                if v_status != "posted" {
                    return Err(OpenTallyError::Validation(format!(
                        "Only posted vouchers can be exported for e-Invoice (voucher status is '{}')",
                        v_status
                    )));
                }

                let doc_typ = match v_type.as_str() {
                    "vt_sales" => "INV",
                    "vt_credit_note" => "CRN",
                    "vt_debit_note" => "DBN",
                    _ => {
                        return Err(OpenTallyError::Validation(format!(
                            "e-Invoice is only applicable for Sales, Credit Note, or Debit Note vouchers (got '{}')",
                            v_type
                        )));
                    }
                };

                // 3. Fetch GST Lines & Party Info
                #[derive(sqlx::FromRow)]
                struct GstLineDb {
                    #[allow(dead_code)]
                    party_ledger_id: String,
                    party_gstin: Option<String>,
                    hsn_sac_code: String,
                    place_of_supply_state_code: String,
                    #[allow(dead_code)]
                    supply_type: String,
                    taxable_value: String,
                    gst_rate: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                    is_reverse_charge: i64,
                    party_name: String,
                }

                let gst_lines = sqlx::query_as::<_, GstLineDb>(
                    "SELECT vgd.party_ledger_id, COALESCE(vgd.party_gstin, lgp.gstin) as party_gstin,
                            vgd.hsn_sac_code, vgd.place_of_supply_state_code, vgd.supply_type,
                            vgd.taxable_value, vgd.gst_rate, vgd.cgst_amount, vgd.sgst_amount,
                            vgd.igst_amount, vgd.cess_amount, vgd.is_reverse_charge,
                            l.name as party_name
                     FROM voucher_gst_details vgd
                     JOIN ledgers l ON l.company_id = vgd.company_id AND l.id = vgd.party_ledger_id
                     LEFT JOIN ledger_gst_profiles lgp ON lgp.company_id = vgd.company_id AND lgp.ledger_id = vgd.party_ledger_id
                     WHERE vgd.company_id = ? AND vgd.voucher_id = ? AND vgd.is_cancelled = 0
                     ORDER BY vgd.created_at ASC",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if gst_lines.is_empty() {
                    return Err(OpenTallyError::Validation(
                        "No active GST details found for this voucher".to_string(),
                    ));
                }

                let first_line = &gst_lines[0];
                let buyer_gstin = match &first_line.party_gstin {
                    Some(g) if g.trim().len() == 15 => g.trim().to_uppercase(),
                    _ => {
                        return Err(OpenTallyError::Validation(
                            "e-Invoice is only applicable for B2B transactions with a registered 15-digit Buyer GSTIN".to_string(),
                        ));
                    }
                };

                let buyer_state_code = buyer_gstin[0..2].to_string();
                let buyer_name = first_line.party_name.clone();
                let pos = first_line.place_of_supply_state_code.clone();
                let reg_rev = if first_line.is_reverse_charge == 1 { "Y" } else { "N" };

                // Build ItemList and accumulate values
                let mut item_list = Vec::new();
                let mut total_ass_val = Decimal::ZERO;
                let mut total_cgst = Decimal::ZERO;
                let mut total_sgst = Decimal::ZERO;
                let mut total_igst = Decimal::ZERO;
                let mut total_cess = Decimal::ZERO;

                for (idx, line) in gst_lines.iter().enumerate() {
                    let taxable = Decimal::from_str_exact(&line.taxable_value).unwrap_or(Decimal::ZERO);
                    let rate = Decimal::from_str_exact(&line.gst_rate).unwrap_or(Decimal::ZERO);
                    let cgst = Decimal::from_str_exact(&line.cgst_amount).unwrap_or(Decimal::ZERO);
                    let sgst = Decimal::from_str_exact(&line.sgst_amount).unwrap_or(Decimal::ZERO);
                    let igst = Decimal::from_str_exact(&line.igst_amount).unwrap_or(Decimal::ZERO);
                    let cess = Decimal::from_str_exact(&line.cess_amount).unwrap_or(Decimal::ZERO);

                    total_ass_val += taxable;
                    total_cgst += cgst;
                    total_sgst += sgst;
                    total_igst += igst;
                    total_cess += cess;

                    let item_tax = cgst + sgst + igst + cess;
                    let item_tot = taxable + item_tax;
                    let is_service = if line.hsn_sac_code.starts_with("99") { "Y" } else { "N" };

                    item_list.push(EinvoiceItem {
                        sl_no: (idx + 1).to_string(),
                        prd_desc: format!("Line Item #{} (HSN {})", idx + 1, line.hsn_sac_code),
                        is_servc: is_service.to_string(),
                        hsn_cd: line.hsn_sac_code.clone(),
                        qty: 1.0,
                        unit: "NOS".to_string(),
                        unit_price: taxable.to_string().parse().unwrap_or(0.0),
                        tot_amt: taxable.to_string().parse().unwrap_or(0.0),
                        discount: 0.0,
                        ass_amt: taxable.to_string().parse().unwrap_or(0.0),
                        gst_rt: rate.to_string().parse().unwrap_or(0.0),
                        igst_amt: igst.to_string().parse().unwrap_or(0.0),
                        cgst_amt: cgst.to_string().parse().unwrap_or(0.0),
                        sgst_amt: sgst.to_string().parse().unwrap_or(0.0),
                        ces_amt: cess.to_string().parse().unwrap_or(0.0),
                        tot_item_val: item_tot.to_string().parse().unwrap_or(0.0),
                    });
                }

                let total_tax = total_cgst + total_sgst + total_igst + total_cess;
                let tot_inv_val = total_ass_val + total_tax;

                let payload = EinvoiceV11Payload {
                    version: "1.1".to_string(),
                    tran_dtls: EinvoiceTranDtls {
                        tax_sch: "GST".to_string(),
                        sup_typ: "B2B".to_string(),
                        reg_rev: reg_rev.to_string(),
                        ecm_gstin: None,
                        igst_on_intra: "N".to_string(),
                    },
                    doc_dtls: EinvoiceDocDtls {
                        typ: doc_typ.to_string(),
                        no: v_num,
                        dt: iso_to_slash_dmy(&v_date),
                    },
                    seller_dtls: EinvoiceSellerDtls {
                        gstin: seller_gstin,
                        lgl_nm: comp_name.clone(),
                        trd_nm: comp_mailing,
                        addr1: comp_state.clone(),
                        loc: comp_state,
                        pin: 560001,
                        stcd: seller_state_code,
                    },
                    buyer_dtls: EinvoiceBuyerDtls {
                        gstin: buyer_gstin,
                        lgl_nm: buyer_name.clone(),
                        trd_nm: buyer_name,
                        pos,
                        addr1: "N/A".to_string(),
                        loc: "City".to_string(),
                        pin: 560001,
                        stcd: buyer_state_code,
                    },
                    item_list,
                    val_dtls: EinvoiceValDtls {
                        ass_val: total_ass_val.to_string().parse().unwrap_or(0.0),
                        cgst_val: total_cgst.to_string().parse().unwrap_or(0.0),
                        sgst_val: total_sgst.to_string().parse().unwrap_or(0.0),
                        igst_val: total_igst.to_string().parse().unwrap_or(0.0),
                        ces_val: total_cess.to_string().parse().unwrap_or(0.0),
                        rnd_off_amt: 0.0,
                        tot_inv_val: tot_inv_val.to_string().parse().unwrap_or(0.0),
                    },
                };

                let json_string = serde_json::to_string_pretty(&payload).map_err(|e| {
                    OpenTallyError::Internal(format!("Failed to serialize e-Invoice JSON: {}", e))
                })?;

                Ok(json_string)
            })
        })
        .await
    }

    /// Generate standard NIC e-Way Bill JSON payload for a posted commercial voucher and transport details.
    pub async fn export_eway_bill_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        transport: TransportDetailsInput,
    ) -> Result<String, OpenTallyError> {
        let mode_norm = match transport.transport_mode.trim().to_lowercase().as_str() {
            "road" | "1" => "1".to_string(),
            "rail" | "2" => "2".to_string(),
            "air" | "3" => "3".to_string(),
            "ship" | "4" => "4".to_string(),
            other => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid transport mode '{}'; expected Road (1), Rail (2), Air (3), or Ship (4)",
                    other
                )));
            }
        };

        if transport.distance_km == 0 {
            return Err(OpenTallyError::Validation(
                "Distance in kilometers must be greater than 0 for e-Way Bill generation"
                    .to_string(),
            ));
        }

        if mode_norm == "1" {
            let veh = transport.vehicle_no.as_deref().unwrap_or("").trim();
            if veh.is_empty() {
                return Err(OpenTallyError::Validation(
                    "Vehicle number is required when transport mode is Road".to_string(),
                ));
            }
        }

        if let Some(ref tid) = transport.transporter_id {
            if tid.trim().len() != 15 {
                return Err(OpenTallyError::Validation(format!(
                    "Transporter ID must be 15 characters, got '{}'",
                    tid
                )));
            }
        }

        let cid = company_id.to_string();
        let vid = voucher_id.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let vid = vid.clone();
            let transport = transport.clone();
            let mode_norm = mode_norm.clone();

            Box::pin(async move {
                // 1. Fetch Company & Seller GSTIN
                let company_row: Option<(String, String)> = sqlx::query_as(
                    "SELECT name, state FROM companies WHERE id = ?",
                )
                .bind(&cid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (comp_name, comp_state) = company_row.ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Company '{}' not found", cid))
                })?;

                let seller_gst_row: Option<(String, String)> = sqlx::query_as(
                    "SELECT COALESCE(gstin, ''), state_code FROM company_gst_profiles WHERE company_id = ?",
                )
                .bind(&cid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (seller_gstin, seller_state_code) = match seller_gst_row {
                    Some((g, s)) if g.trim().len() == 15 => (g.trim().to_uppercase(), s),
                    _ => {
                        return Err(OpenTallyError::Validation(
                            "Company has no valid 15-character GSTIN configured".to_string(),
                        ));
                    }
                };

                let seller_state_int: i64 = seller_state_code.parse().unwrap_or(29);

                // 2. Fetch Voucher Header
                let voucher_row: Option<(String, String, String, String)> = sqlx::query_as(
                    "SELECT voucher_type_id, voucher_number, date, status
                     FROM vouchers WHERE company_id = ? AND id = ?",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let (v_type, v_num, v_date, v_status) = voucher_row.ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Voucher '{}' not found", vid))
                })?;

                if v_status != "posted" {
                    return Err(OpenTallyError::Validation(format!(
                        "Only posted vouchers can be exported for e-Way Bill (voucher status is '{}')",
                        v_status
                    )));
                }

                let doc_type = match v_type.as_str() {
                    "vt_sales" => "INV",
                    "vt_credit_note" => "CRN",
                    "vt_debit_note" => "DBN",
                    _ => "INV",
                };

                // 3. Fetch GST Lines & Party Info
                #[derive(sqlx::FromRow)]
                struct GstLineDb {
                    party_gstin: Option<String>,
                    hsn_sac_code: String,
                    taxable_value: String,
                    gst_rate: String,
                    cgst_amount: String,
                    sgst_amount: String,
                    igst_amount: String,
                    cess_amount: String,
                    party_name: String,
                }

                let gst_lines = sqlx::query_as::<_, GstLineDb>(
                    "SELECT COALESCE(vgd.party_gstin, lgp.gstin) as party_gstin,
                            vgd.hsn_sac_code, vgd.taxable_value, vgd.gst_rate,
                            vgd.cgst_amount, vgd.sgst_amount, vgd.igst_amount, vgd.cess_amount,
                            l.name as party_name
                     FROM voucher_gst_details vgd
                     JOIN ledgers l ON l.company_id = vgd.company_id AND l.id = vgd.party_ledger_id
                     LEFT JOIN ledger_gst_profiles lgp ON lgp.company_id = vgd.company_id AND lgp.ledger_id = vgd.party_ledger_id
                     WHERE vgd.company_id = ? AND vgd.voucher_id = ? AND vgd.is_cancelled = 0
                     ORDER BY vgd.created_at ASC",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                if gst_lines.is_empty() {
                    return Err(OpenTallyError::Validation(
                        "No active GST details found for this voucher".to_string(),
                    ));
                }

                let first_line = &gst_lines[0];
                let buyer_gstin = first_line
                    .party_gstin
                    .as_deref()
                    .unwrap_or("URP")
                    .trim()
                    .to_uppercase();
                let buyer_name = first_line.party_name.clone();
                let buyer_state_int: i64 = if buyer_gstin.len() == 15 {
                    buyer_gstin[0..2].parse().unwrap_or(seller_state_int)
                } else {
                    seller_state_int
                };

                let mut total_ass_val = Decimal::ZERO;
                let mut total_cgst = Decimal::ZERO;
                let mut total_sgst = Decimal::ZERO;
                let mut total_igst = Decimal::ZERO;
                let mut total_cess = Decimal::ZERO;
                let mut items = Vec::new();

                for (idx, line) in gst_lines.iter().enumerate() {
                    let taxable = Decimal::from_str_exact(&line.taxable_value).unwrap_or(Decimal::ZERO);
                    let rate = Decimal::from_str_exact(&line.gst_rate).unwrap_or(Decimal::ZERO);
                    let cgst = Decimal::from_str_exact(&line.cgst_amount).unwrap_or(Decimal::ZERO);
                    let sgst = Decimal::from_str_exact(&line.sgst_amount).unwrap_or(Decimal::ZERO);
                    let igst = Decimal::from_str_exact(&line.igst_amount).unwrap_or(Decimal::ZERO);
                    let cess = Decimal::from_str_exact(&line.cess_amount).unwrap_or(Decimal::ZERO);

                    total_ass_val += taxable;
                    total_cgst += cgst;
                    total_sgst += sgst;
                    total_igst += igst;
                    total_cess += cess;

                    let hsn_int: i64 = line.hsn_sac_code.parse().unwrap_or(0);
                    let (cr, sr, ir) = if igst.is_zero() {
                        ((rate / Decimal::from(2)).to_string().parse().unwrap_or(0.0),
                         (rate / Decimal::from(2)).to_string().parse().unwrap_or(0.0),
                         0.0)
                    } else {
                        (0.0, 0.0, rate.to_string().parse().unwrap_or(0.0))
                    };

                    items.push(EwayBillItem {
                        itemNo: idx + 1,
                        productId: idx + 1,
                        productName: format!("Item #{}", idx + 1),
                        productDesc: format!("HSN {}", line.hsn_sac_code),
                        hsnCode: hsn_int,
                        quantity: 1.0,
                        qtyUnit: "NOS".to_string(),
                        taxableAmount: taxable.to_string().parse().unwrap_or(0.0),
                        sgstRate: sr,
                        cgstRate: cr,
                        igstRate: ir,
                        cessRate: 0.0,
                    });
                }

                let total_tax = total_cgst + total_sgst + total_igst + total_cess;
                let tot_inv_val = total_ass_val + total_tax;

                let entry = EwayBillEntry {
                    userGstin: seller_gstin.clone(),
                    supplyType: "O".to_string(),
                    subSupplyType: "1".to_string(),
                    docType: doc_type.to_string(),
                    docNo: v_num,
                    docDate: iso_to_slash_dmy(&v_date),
                    fromGstin: seller_gstin,
                    fromTrdName: comp_name,
                    fromAddr1: comp_state.clone(),
                    fromPlace: comp_state,
                    fromPincode: 560001,
                    actFromStateCode: seller_state_int,
                    fromStateCode: seller_state_int,
                    toGstin: buyer_gstin,
                    toTrdName: buyer_name,
                    toAddr1: "N/A".to_string(),
                    toPlace: "City".to_string(),
                    toPincode: 560001,
                    actToStateCode: buyer_state_int,
                    toStateCode: buyer_state_int,
                    totalValue: total_ass_val.to_string().parse().unwrap_or(0.0),
                    cgstValue: total_cgst.to_string().parse().unwrap_or(0.0),
                    sgstValue: total_sgst.to_string().parse().unwrap_or(0.0),
                    igstValue: total_igst.to_string().parse().unwrap_or(0.0),
                    cessValue: total_cess.to_string().parse().unwrap_or(0.0),
                    totInvValue: tot_inv_val.to_string().parse().unwrap_or(0.0),
                    transMode: mode_norm,
                    transDistance: transport.distance_km.to_string(),
                    transporterId: transport.transporter_id,
                    transporterName: transport.transporter_name,
                    transDocNo: transport.trans_doc_no,
                    transDocDate: transport.trans_doc_date.as_deref().map(iso_to_slash_dmy),
                    vehicleNo: transport.vehicle_no,
                    vehicleType: transport.vehicle_type.unwrap_or_else(|| "R".to_string()),
                    itemList: items,
                };

                let root = EwayBillJsonPayload {
                    version: "1.0.0621".to_string(),
                    billLists: vec![entry],
                };

                let json_string = serde_json::to_string_pretty(&root).map_err(|e| {
                    OpenTallyError::Internal(format!("Failed to serialize e-Way Bill JSON: {}", e))
                })?;

                Ok(json_string)
            })
        })
        .await
    }

    /// Upsert e-Invoice metadata (IRN, Ack No/Date, e-Way bill number) inside an explicit transaction with MCA audit log.
    pub async fn save_voucher_einvoice_metadata(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        input: VoucherEinvoiceMetadataInput,
    ) -> Result<VoucherEinvoiceMetadataDto, OpenTallyError> {
        // Validation: IRN length exactly 64 hex characters if present
        if let Some(ref irn) = input.irn {
            let clean = irn.trim();
            if clean.len() != 64 || !clean.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(OpenTallyError::Validation(format!(
                    "IRN must be exactly 64 hexadecimal characters, got '{}' (len {})",
                    clean,
                    clean.len()
                )));
            }
        }

        // Validation: e-Way bill number length exactly 12 digits if present
        if let Some(ref ewb) = input.eway_bill_no {
            let clean = ewb.trim();
            if clean.len() != 12 || !clean.chars().all(|c| c.is_ascii_digit()) {
                return Err(OpenTallyError::Validation(format!(
                    "e-Way Bill number must be exactly 12 digits, got '{}'",
                    clean
                )));
            }
        }

        let status = match input.einvoice_status.as_deref() {
            Some("Generated") => "Generated",
            Some("Cancelled") => "Cancelled",
            _ => "Pending",
        };

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // Ensure voucher exists
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM vouchers WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(voucher_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

        if exists.is_none() {
            return Err(OpenTallyError::NotFound(format!(
                "Voucher '{}' not found in company '{}'",
                voucher_id, company_id
            )));
        }

        // Fetch dynamic FY begins for audit log
        let fy_begins: String =
            match sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?
            {
                Some(fy) => fy,
                None => "2026-04-01".to_string(),
            };

        let row_id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO voucher_einvoice_details (
                id, company_id, voucher_id, irn, ack_no, ack_date, signed_qr_code,
                einvoice_status, eway_bill_no, eway_bill_date, eway_valid_upto,
                transporter_id, transporter_name, transport_mode, vehicle_no, distance_km,
                created_at, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id, voucher_id) DO UPDATE SET
                irn = COALESCE(excluded.irn, voucher_einvoice_details.irn),
                ack_no = COALESCE(excluded.ack_no, voucher_einvoice_details.ack_no),
                ack_date = COALESCE(excluded.ack_date, voucher_einvoice_details.ack_date),
                signed_qr_code = COALESCE(excluded.signed_qr_code, voucher_einvoice_details.signed_qr_code),
                einvoice_status = excluded.einvoice_status,
                eway_bill_no = COALESCE(excluded.eway_bill_no, voucher_einvoice_details.eway_bill_no),
                eway_bill_date = COALESCE(excluded.eway_bill_date, voucher_einvoice_details.eway_bill_date),
                eway_valid_upto = COALESCE(excluded.eway_valid_upto, voucher_einvoice_details.eway_valid_upto),
                transporter_id = COALESCE(excluded.transporter_id, voucher_einvoice_details.transporter_id),
                transporter_name = COALESCE(excluded.transporter_name, voucher_einvoice_details.transporter_name),
                transport_mode = COALESCE(excluded.transport_mode, voucher_einvoice_details.transport_mode),
                vehicle_no = COALESCE(excluded.vehicle_no, voucher_einvoice_details.vehicle_no),
                distance_km = COALESCE(excluded.distance_km, voucher_einvoice_details.distance_km),
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(&row_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(input.irn.as_deref().map(|s| s.trim()))
        .bind(input.ack_no.as_deref().map(|s| s.trim()))
        .bind(input.ack_date.as_deref().map(|s| s.trim()))
        .bind(input.signed_qr_code.as_deref().map(|s| s.trim()))
        .bind(status)
        .bind(input.eway_bill_no.as_deref().map(|s| s.trim()))
        .bind(input.eway_bill_date.as_deref().map(|s| s.trim()))
        .bind(input.eway_valid_upto.as_deref().map(|s| s.trim()))
        .bind(input.transporter_id.as_deref().map(|s| s.trim()))
        .bind(input.transporter_name.as_deref().map(|s| s.trim()))
        .bind(input.transport_mode.as_deref().map(|s| s.trim()))
        .bind(input.vehicle_no.as_deref().map(|s| s.trim()))
        .bind(input.distance_km)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Audit log
        let audit_id = Uuid::new_v4().to_string();
        let new_values = serde_json::to_string(&input).unwrap_or_default();
        sqlx::query(
            "INSERT INTO audit_log (
                id, company_id, entity_type, entity_id, ledger_id,
                financial_year_begins, action, old_values, new_values, performed_by
             ) VALUES (?, ?, 'voucher_einvoice_details', ?, NULL, ?, 'UPDATE', NULL, ?, 'system')",
        )
        .bind(&audit_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(&fy_begins)
        .bind(&new_values)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        Self::get_voucher_einvoice_metadata(pool, company_id, voucher_id)
            .await?
            .ok_or_else(|| {
                OpenTallyError::Internal("Failed to read back saved metadata".to_string())
            })
    }

    /// Read-only fetch of voucher e-Invoice details metadata.
    pub async fn get_voucher_einvoice_metadata(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<Option<VoucherEinvoiceMetadataDto>, OpenTallyError> {
        let cid = company_id.to_string();
        let vid = voucher_id.to_string();

        execute_read_only_query(pool, move |conn| {
            let cid = cid.clone();
            let vid = vid.clone();

            Box::pin(async move {
                let row = sqlx::query_as::<_, VoucherEinvoiceMetadataDto>(
                    "SELECT id, company_id, voucher_id, irn, ack_no, ack_date, signed_qr_code,
                            einvoice_status, eway_bill_no, eway_bill_date, eway_valid_upto,
                            transporter_id, transporter_name, transport_mode, vehicle_no, distance_km,
                            created_at, updated_at
                     FROM voucher_einvoice_details
                     WHERE company_id = ? AND voucher_id = ?",
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                Ok(row)
            })
        })
        .await
    }
}
