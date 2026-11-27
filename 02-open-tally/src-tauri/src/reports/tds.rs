use crate::errors::OpenTallyError;
use crate::tds::models::*;
use crate::tds::service::to_canonical_decimal;
use crate::vouchers::service::execute_read_only_query;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::str::FromStr;

pub struct TdsReportService;

impl TdsReportService {
    /// Helper to compute due date for TDS remittance (7th of next month, or Apr 30 for March).
    pub fn compute_tds_due_date(voucher_date_str: &str) -> String {
        if let Ok(parsed) = NaiveDate::parse_from_str(voucher_date_str, "%Y-%m-%d") {
            let year = parsed.year();
            let month = parsed.month();
            if month == 3 {
                format!("{:04}-04-30", year)
            } else if month == 12 {
                format!("{:04}-01-07", year + 1)
            } else {
                format!("{:04}-{:02}-07", year, month + 1)
            }
        } else {
            voucher_date_str.to_string()
        }
    }

    /// Evaluates quarter start and end dates for a given FY (e.g. "2026-2027", "2026-27", or "2026") and quarter ("Q1", "Q2", "Q3", "Q4").
    pub fn get_quarter_date_bounds(
        fy: &str,
        quarter: &str,
    ) -> Result<(String, String), OpenTallyError> {
        let clean_fy = fy.trim();
        let start_year: i32 = if clean_fy.len() >= 4 {
            clean_fy[..4].parse::<i32>().map_err(|_| {
                OpenTallyError::Validation(format!("Invalid financial year '{}'", fy))
            })?
        } else {
            return Err(OpenTallyError::Validation(format!(
                "Invalid financial year '{}'",
                fy
            )));
        };

        let q = quarter.to_uppercase();
        let q_str = q.trim();
        match q_str {
            "Q1" => {
                let start = format!("{:04}-04-01", start_year);
                let end = format!("{:04}-06-30", start_year);
                Ok((start, end))
            }
            "Q2" => {
                let start = format!("{:04}-07-01", start_year);
                let end = format!("{:04}-09-30", start_year);
                Ok((start, end))
            }
            "Q3" => {
                let start = format!("{:04}-10-01", start_year);
                let end = format!("{:04}-12-31", start_year);
                Ok((start, end))
            }
            "Q4" => {
                let next_year = start_year + 1;
                let start = format!("{:04}-01-01", next_year);
                let end = format!("{:04}-03-31", next_year);
                Ok((start, end))
            }
            other => Err(OpenTallyError::Validation(format!(
                "Invalid quarter '{}'; expected Q1, Q2, Q3, or Q4",
                other
            ))),
        }
    }

    /// Generates TDS Outstanding report (unpaid deductions grouped by section code).
    #[allow(clippy::type_complexity)]
    pub async fn get_tds_outstanding(
        pool: &Pool<Sqlite>,
        company_id: &str,
        as_of_date: &str,
    ) -> Result<TdsOutstandingReport, OpenTallyError> {
        let company_id_owned = company_id.to_string();
        let as_of_date_owned = as_of_date.to_string();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let rows: Vec<(
                    String, String, String, String, String, String,
                    Option<String>, String, Option<String>,
                    String, String, String
                )> = sqlx::query_as(
                    r#"
                    SELECT
                        vtd.id, vtd.voucher_id, v.voucher_number, v.date as voucher_date,
                        vtd.party_ledger_id, l.name as party_name, vtd.party_pan,
                        vtd.section_code, tnp.name as section_name,
                        vtd.gross_amount, vtd.tds_rate, vtd.tds_amount
                    FROM voucher_tds_details vtd
                    JOIN vouchers v ON vtd.company_id = v.company_id AND vtd.voucher_id = v.id
                    JOIN ledgers l ON vtd.company_id = l.company_id AND vtd.party_ledger_id = l.id
                    LEFT JOIN tds_nature_of_payments tnp ON vtd.company_id = tnp.company_id AND vtd.section_code = tnp.section_code
                    WHERE vtd.company_id = ?
                      AND vtd.is_paid = 0
                      AND vtd.is_cancelled = 0
                      AND v.status = 'posted'
                      AND v.date <= ?
                    ORDER BY vtd.section_code ASC, v.date ASC, v.voucher_number ASC
                    "#,
                )
                .bind(&company_id_owned)
                .bind(&as_of_date_owned)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut section_map: HashMap<String, (String, Decimal, Vec<TdsOutstandingVoucherItem>)> = HashMap::new();
                let mut total_unpaid = Decimal::ZERO;

                for (
                    id, v_id, v_num, v_date, p_id, p_name, p_pan,
                    sec_code, sec_name, gross, rate, amt
                ) in rows {
                    let parsed_amt = Decimal::from_str(&amt).unwrap_or(Decimal::ZERO);
                    total_unpaid += parsed_amt;

                    let due_date = Self::compute_tds_due_date(&v_date);
                    let status = if as_of_date_owned.as_str() > due_date.as_str() {
                        "Overdue".to_string()
                    } else {
                        "Due".to_string()
                    };

                    let item = TdsOutstandingVoucherItem {
                        voucher_tds_id: id,
                        voucher_id: v_id,
                        voucher_number: v_num,
                        voucher_date: v_date,
                        party_ledger_id: p_id,
                        party_name: p_name,
                        party_pan: p_pan,
                        section_code: sec_code.clone(),
                        gross_amount: gross,
                        tds_rate: rate,
                        tds_amount: amt,
                        due_date,
                        status,
                    };

                    let entry = section_map.entry(sec_code.clone()).or_insert_with(|| {
                        (sec_name.unwrap_or_else(|| sec_code.clone()), Decimal::ZERO, Vec::new())
                    });
                    entry.1 += parsed_amt;
                    entry.2.push(item);
                }

                let mut sections = Vec::with_capacity(section_map.len());
                let mut sorted_keys: Vec<String> = section_map.keys().cloned().collect();
                sorted_keys.sort();

                for k in sorted_keys {
                    if let Some((name, sum_amt, vchs)) = section_map.remove(&k) {
                        sections.push(TdsOutstandingSectionSummary {
                            section_code: k,
                            section_name: name,
                            unpaid_tds_amount: to_canonical_decimal(sum_amt),
                            voucher_count: vchs.len(),
                            vouchers: vchs,
                        });
                    }
                }

                Ok(TdsOutstandingReport {
                    as_of_date: as_of_date_owned,
                    total_unpaid_tds: to_canonical_decimal(total_unpaid),
                    sections,
                })
            })
        })
        .await
    }

    /// Generates statutory Form 26Q summary report for a given FY and quarter.
    #[allow(clippy::type_complexity)]
    pub async fn get_form_26q_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<Form26qSummary, OpenTallyError> {
        let (start_date, end_date) = Self::get_quarter_date_bounds(financial_year, quarter)?;

        let company_id_owned = company_id.to_string();
        let fy_owned = financial_year.to_string();
        let q_owned = quarter.to_uppercase();

        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                // 1. Deductor details
                let comp_name: String =
                    sqlx::query_scalar("SELECT name FROM companies WHERE id = ?")
                        .bind(&company_id_owned)
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                let deductor_tan: Option<String> =
                    sqlx::query_scalar("SELECT tan FROM company_tds_profiles WHERE company_id = ?")
                        .bind(&company_id_owned)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?
                        .flatten();

                // 2. Fetch all active deductions in this quarter
                let rows: Vec<(
                    String,
                    String,
                    Option<String>,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    i64,
                )> = sqlx::query_as(
                    r#"
                    SELECT
                        vtd.id, v.voucher_number, vtd.party_pan, l.name as deductee_name,
                        vtd.deductee_type, vtd.section_code, v.date as payment_date,
                        vtd.gross_amount, vtd.tds_rate, vtd.tds_amount,
                        vtd.is_pan_missing_higher_rate, vtd.challan_serial_no,
                        vtd.challan_date, vtd.challan_bsr_code, vtd.payment_voucher_id,
                        vtd.is_paid
                    FROM voucher_tds_details vtd
                    JOIN vouchers v ON vtd.company_id = v.company_id AND vtd.voucher_id = v.id
                    JOIN ledgers l ON vtd.company_id = l.company_id AND vtd.party_ledger_id = l.id
                    WHERE vtd.company_id = ?
                      AND vtd.is_cancelled = 0
                      AND v.status = 'posted'
                      AND v.date >= ? AND v.date <= ?
                    ORDER BY v.date ASC, v.voucher_number ASC
                    "#,
                )
                .bind(&company_id_owned)
                .bind(&start_date)
                .bind(&end_date)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut total_paid = Decimal::ZERO;
                let mut total_deducted = Decimal::ZERO;
                let mut total_deposited = Decimal::ZERO;
                let mut total_unpaid = Decimal::ZERO;

                let mut challan_map: HashMap<
                    (String, String, String),
                    (Option<String>, Decimal, usize),
                > = HashMap::new();
                let mut deductions = Vec::with_capacity(rows.len());

                for (
                    id,
                    v_num,
                    pan,
                    d_name,
                    d_type,
                    sec,
                    p_date,
                    gross,
                    rate,
                    amt,
                    higher_rate,
                    c_serial,
                    c_date,
                    c_bsr,
                    pv_id,
                    is_paid,
                ) in rows
                {
                    let g_dec = Decimal::from_str(&gross).unwrap_or(Decimal::ZERO);
                    let a_dec = Decimal::from_str(&amt).unwrap_or(Decimal::ZERO);

                    total_paid += g_dec;
                    total_deducted += a_dec;

                    if is_paid == 1 {
                        total_deposited += a_dec;
                        if let (Some(ref bsr), Some(ref dt), Some(ref ser)) =
                            (&c_bsr, &c_date, &c_serial)
                        {
                            let key = (bsr.clone(), dt.clone(), ser.clone());
                            let entry =
                                challan_map
                                    .entry(key)
                                    .or_insert((pv_id.clone(), Decimal::ZERO, 0));
                            entry.1 += a_dec;
                            entry.2 += 1;
                        }
                    } else {
                        total_unpaid += a_dec;
                    }

                    deductions.push(Form26qDeductionItem {
                        voucher_tds_id: id,
                        voucher_number: v_num,
                        deductee_pan: pan,
                        deductee_name: d_name,
                        deductee_type: d_type,
                        section_code: sec,
                        payment_date: p_date,
                        amount_paid: gross,
                        tds_rate: rate,
                        tds_deducted: amt,
                        is_pan_missing_higher_rate: higher_rate == 1,
                        challan_serial_no: c_serial,
                        challan_date: c_date,
                        challan_bsr_code: c_bsr,
                        status: if is_paid == 1 {
                            "Deposited".to_string()
                        } else {
                            "Unpaid".to_string()
                        },
                    });
                }

                let mut challans = Vec::with_capacity(challan_map.len());
                for ((bsr, dt, ser), (pv_id, dep_amt, count)) in challan_map {
                    challans.push(Form26qChallanItem {
                        challan_bsr_code: bsr,
                        challan_date: dt,
                        challan_serial_no: ser,
                        payment_voucher_id: pv_id,
                        total_tds_deposited: to_canonical_decimal(dep_amt),
                        voucher_count: count,
                    });
                }
                challans.sort_by(|a, b| a.challan_date.cmp(&b.challan_date));

                Ok(Form26qSummary {
                    company_id: company_id_owned,
                    financial_year: fy_owned,
                    quarter: q_owned,
                    deductor_tan,
                    deductor_name: comp_name,
                    total_amount_paid: to_canonical_decimal(total_paid),
                    total_tds_deducted: to_canonical_decimal(total_deducted),
                    total_tds_deposited: to_canonical_decimal(total_deposited),
                    total_tds_unpaid: to_canonical_decimal(total_unpaid),
                    challans,
                    deductions,
                })
            })
        })
        .await
    }

    /// Serializes Form 26Q summary to JSON format.
    pub async fn export_form_26q_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_form_26q_summary(pool, company_id, financial_year, quarter).await?;
        serde_json::to_string_pretty(&summary)
            .map_err(|e| OpenTallyError::Validation(e.to_string()))
    }

    /// Exports Form 26Q deductions to CWE-1236 safe CSV format.
    pub async fn export_form_26q_csv(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_form_26q_summary(pool, company_id, financial_year, quarter).await?;

        let sanitize = |s: &str| -> String {
            let trimmed = s.trim();
            if trimmed.starts_with('=')
                || trimmed.starts_with('+')
                || trimmed.starts_with('-')
                || trimmed.starts_with('@')
            {
                format!("'{}", trimmed)
            } else if trimmed.contains(',') || trimmed.contains('"') || trimmed.contains('\n') {
                format!("\"{}\"", trimmed.replace('"', "\"\""))
            } else {
                trimmed.to_string()
            }
        };

        let mut lines = Vec::new();
        if let Some(ref tan) = summary.deductor_tan {
            lines.push(format!(
                "# Deductor TAN: {}, FY: {}, Quarter: {}",
                tan, summary.financial_year, summary.quarter
            ));
        }
        lines.push("Voucher No,Deductee PAN,Deductee Name,Deductee Type,Section,Payment Date,Gross Amount,TDS Rate,TDS Deducted,Challan BSR,Challan Date,Challan Serial,Status".to_string());

        for d in summary.deductions {
            let row = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                sanitize(&d.voucher_number),
                sanitize(d.deductee_pan.as_deref().unwrap_or("PANNOTAVBL")),
                sanitize(&d.deductee_name),
                sanitize(&d.deductee_type),
                sanitize(&d.section_code),
                sanitize(&d.payment_date),
                sanitize(&d.amount_paid),
                sanitize(&d.tds_rate),
                sanitize(&d.tds_deducted),
                sanitize(d.challan_bsr_code.as_deref().unwrap_or("")),
                sanitize(d.challan_date.as_deref().unwrap_or("")),
                sanitize(d.challan_serial_no.as_deref().unwrap_or("")),
                sanitize(&d.status),
            );
            lines.push(row);
        }

        Ok(lines.join("\r\n"))
    }

    /// Generates Form 27Q (NRI / Foreign Remittance TDS — Sec 195) quarterly summary.
    pub async fn get_form_27q_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<Form26qSummary, OpenTallyError> {
        let mut summary =
            Self::get_form_26q_summary(pool, company_id, financial_year, quarter).await?;
        summary
            .deductions
            .retain(|d| d.section_code.starts_with("195") || d.deductee_type == "Company_Foreign");
        let mut total_paid = Decimal::ZERO;
        let mut total_ded = Decimal::ZERO;
        let mut total_dep = Decimal::ZERO;
        for d in &summary.deductions {
            let paid = Decimal::from_str(&d.amount_paid).unwrap_or(Decimal::ZERO);
            let ded = Decimal::from_str(&d.tds_deducted).unwrap_or(Decimal::ZERO);
            total_paid += paid;
            total_ded += ded;
            if d.status == "Deposited" {
                total_dep += ded;
            }
        }
        summary.total_amount_paid = to_canonical_decimal(total_paid);
        summary.total_tds_deducted = to_canonical_decimal(total_ded);
        summary.total_tds_deposited = to_canonical_decimal(total_dep);
        Ok(summary)
    }

    /// Exports Form 27Q (NRI TDS) quarterly return as JSON.
    pub async fn export_form_27q_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<String, OpenTallyError> {
        let summary = Self::get_form_27q_summary(pool, company_id, financial_year, quarter).await?;
        serde_json::to_string_pretty(&summary).map_err(|e| OpenTallyError::Internal(e.to_string()))
    }

    /// Generates Form 27EQ (TCS — Sec 206C) quarterly summary.
    pub async fn get_form_27eq_summary(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<Form26qSummary, OpenTallyError> {
        let mut summary =
            Self::get_form_26q_summary(pool, company_id, financial_year, quarter).await?;
        summary
            .deductions
            .retain(|d| d.section_code.starts_with("206C"));
        let mut total_paid = Decimal::ZERO;
        let mut total_ded = Decimal::ZERO;
        let mut total_dep = Decimal::ZERO;
        for d in &summary.deductions {
            let paid = Decimal::from_str(&d.amount_paid).unwrap_or(Decimal::ZERO);
            let ded = Decimal::from_str(&d.tds_deducted).unwrap_or(Decimal::ZERO);
            total_paid += paid;
            total_ded += ded;
            if d.status == "Deposited" {
                total_dep += ded;
            }
        }
        summary.total_amount_paid = to_canonical_decimal(total_paid);
        summary.total_tds_deducted = to_canonical_decimal(total_ded);
        summary.total_tds_deposited = to_canonical_decimal(total_dep);
        Ok(summary)
    }

    /// Exports Form 27EQ (TCS) quarterly return as JSON.
    pub async fn export_form_27eq_json(
        pool: &Pool<Sqlite>,
        company_id: &str,
        financial_year: &str,
        quarter: &str,
    ) -> Result<String, OpenTallyError> {
        let summary =
            Self::get_form_27eq_summary(pool, company_id, financial_year, quarter).await?;
        serde_json::to_string_pretty(&summary).map_err(|e| OpenTallyError::Internal(e.to_string()))
    }
}
