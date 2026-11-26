use crate::errors::OpenTallyError;
use crate::pdf::words::{format_indian_currency, number_to_indian_words};
use crate::pdf::writer::PdfWriter;
use crate::vouchers::service::search::execute_read_only_query;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

pub const POINTS_PER_MM: f64 = 72.0 / 25.4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChequeTemplateDto {
    pub id: String,
    pub company_id: String,
    pub bank_name: String,
    pub width_mm: f64,
    pub height_mm: f64,
    pub date_x_mm: f64,
    pub date_y_mm: f64,
    pub payee_x_mm: f64,
    pub payee_y_mm: f64,
    pub words_x_mm: f64,
    pub words_y_mm: f64,
    pub amount_x_mm: f64,
    pub amount_y_mm: f64,
    pub is_account_payee_default: bool,
}

impl Default for ChequeTemplateDto {
    fn default() -> Self {
        Self {
            id: "default_cts2010".to_string(),
            company_id: "".to_string(),
            bank_name: "CTS-2010 Standard".to_string(),
            width_mm: 202.0,
            height_mm: 92.0,
            date_x_mm: 148.0,
            date_y_mm: 9.0,
            payee_x_mm: 22.0,
            payee_y_mm: 25.0,
            words_x_mm: 26.0,
            words_y_mm: 36.0,
            amount_x_mm: 152.0,
            amount_y_mm: 42.0,
            is_account_payee_default: true,
        }
    }
}

/// Formats an ISO date `YYYY-MM-DD` into an array of 8 characters `[D, D, M, M, Y, Y, Y, Y]`.
pub fn format_cts2010_date_chars(iso_date: &str) -> Option<[char; 8]> {
    let parts: Vec<&str> = iso_date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year = parts[0];
    let month = parts[1];
    let day = parts[2];

    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }

    let mut chars = ['0'; 8];
    let mut idx = 0;
    for c in day.chars() {
        chars[idx] = c;
        idx += 1;
    }
    for c in month.chars() {
        chars[idx] = c;
        idx += 1;
    }
    for c in year.chars() {
        chars[idx] = c;
        idx += 1;
    }
    Some(chars)
}

/// Generates binary `%PDF-1.4` bytes for an Indian CTS-2010 bank cheque.
///
/// Executed strictly inside `ReadOnlyConnectionGuard` to guarantee zero state mutation.
pub async fn generate_cheque_pdf_bytes(
    pool: &Pool<Sqlite>,
    company_id: &str,
    voucher_id: &str,
    template_id: Option<&str>,
    is_account_payee: Option<bool>,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = company_id.to_string();
    let vid = voucher_id.to_string();
    let tid = template_id.map(|s| s.to_string());

    execute_read_only_query(pool, move |conn| {
        let cid = cid.clone();
        let vid = vid.clone();
        let tid = tid.clone();
        Box::pin(async move {
            // 1. Fetch Voucher Header
            let voucher_row: Option<(String, String, String, String)> = sqlx::query_as(
                "SELECT id, voucher_number, date, status FROM vouchers WHERE company_id = ? AND id = ?",
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (_v_id, voucher_number, voucher_date, voucher_status) = voucher_row.ok_or_else(|| {
                OpenTallyError::NotFound(format!("Voucher '{}' not found in company '{}'", vid, cid))
            })?;

            if voucher_status == "cancelled" {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot print cheque for cancelled voucher '{}'",
                    voucher_number
                )));
            }

            // 2. Fetch Bank Allocation if present (prioritize Cheque or explicit favouring_name)
            #[allow(clippy::type_complexity)]
            let alloc_row: Option<(
                Option<String>,
                Option<String>,
                Option<String>,
                String,
                String,
                String,
            )> = sqlx::query_as(
                r#"
                SELECT instrument_number, instrument_date, favouring_name, amount, bank_ledger_id, transaction_type
                FROM bank_allocations
                WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0
                ORDER BY CASE WHEN transaction_type = 'Cheque' THEN 0 WHEN favouring_name IS NOT NULL AND length(trim(favouring_name)) > 0 THEN 1 ELSE 2 END, rowid DESC LIMIT 1
                "#,
            )
            .bind(&cid)
            .bind(&vid)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let (cheque_date, payee_name, amount_dec, bank_ledger_id) = if let Some((
                _inst_num,
                inst_date,
                favouring,
                amt_str,
                b_ledger_id,
                _tx_type,
            )) = alloc_row
            {
                let d = inst_date.unwrap_or_else(|| voucher_date.clone());
                let amt = Decimal::from_str(&amt_str).map_err(|e| {
                    OpenTallyError::Validation(format!("Invalid bank allocation amount: {}", e))
                })?;
                let mut f = favouring.unwrap_or_default();
                if f.trim().is_empty() {
                    // Fallback to party ledger name on voucher if favouring is not set on bank allocation
                    let party_name_res: Option<(String,)> = sqlx::query_as(
                        r#"
                        SELECT l.name
                        FROM voucher_lines vl
                        JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                        WHERE vl.company_id = ? AND vl.voucher_id = ? AND vl.ledger_id != ?
                        ORDER BY vl.line_number ASC LIMIT 1
                        "#,
                    )
                    .bind(&cid)
                    .bind(&vid)
                    .bind(&b_ledger_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    if let Some((pname,)) = party_name_res {
                        f = pname;
                    }
                }
                (d, f, amt, Some(b_ledger_id))
            } else {
                // Fallback: derive payee and amount from voucher lines
                let lines: Vec<(String, String, String, String)> = sqlx::query_as(
                    r#"
                    SELECT vl.ledger_id, l.name, vl.entry_type, vl.amount
                    FROM voucher_lines vl
                    JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
                    WHERE vl.company_id = ? AND vl.voucher_id = ?
                    ORDER BY vl.line_number ASC
                    "#,
                )
                .bind(&cid)
                .bind(&vid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

                let mut payee = String::new();
                let mut amt = Decimal::ZERO;
                let mut bank_id = None;

                for (l_id, l_name, entry_type, a_str) in &lines {
                    let a = Decimal::from_str(a_str).unwrap_or(Decimal::ZERO);
                    if entry_type == "Dr" && payee.is_empty() {
                        payee = l_name.clone();
                        amt = a;
                    } else if entry_type == "Cr" && bank_id.is_none() {
                        bank_id = Some(l_id.clone());
                    }
                }
                (voucher_date, payee, amt, bank_id)
            };

            if amount_dec <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(
                    "Cheque amount must be strictly greater than zero".to_string(),
                ));
            }

            let effective_payee = if payee_name.trim().is_empty() {
                "Bearer".to_string()
            } else {
                payee_name.trim().to_string()
            };

            // 3. Resolve Cheque Template
            let template = resolve_cheque_template(&mut *conn, &cid, tid.as_deref(), bank_ledger_id.as_deref()).await?;

            let account_payee_final = is_account_payee.unwrap_or(template.is_account_payee_default);

            // 4. Render PDF
            let width_pt = template.width_mm * POINTS_PER_MM;
            let height_pt = template.height_mm * POINTS_PER_MM;

            let mut writer = PdfWriter::with_dimensions(width_pt, height_pt);

            // A/C Payee Only Crossing
            if account_payee_final {
                let cross_x = 10.0 * POINTS_PER_MM;
                let cross_y = height_pt - (10.0 * POINTS_PER_MM);
                // Two parallel lines diagonally across the top-left corner
                writer.line(cross_x - 15.0, cross_y - 20.0, cross_x + 35.0, cross_y + 15.0, 0.75);
                writer.line(cross_x - 10.0, cross_y - 27.0, cross_x + 40.0, cross_y + 8.0, 0.75);
                writer.text_rotated("A/C PAYEE ONLY", cross_x - 2.0, cross_y - 14.0, 35.0, "/F2", 7.0);
            }

            // Date: D D M M Y Y Y Y
            let date_chars = format_cts2010_date_chars(&cheque_date).unwrap_or(['0', '1', '0', '4', '2', '0', '2', '6']);
            let date_pt_x = template.date_x_mm * POINTS_PER_MM;
            let date_pt_y = (template.height_mm - template.date_y_mm) * POINTS_PER_MM;
            // Standard CTS-2010 box step is ~4.5 mm
            let step_pt = 4.5 * POINTS_PER_MM;
            writer.text_spaced_chars(&date_chars, date_pt_x, date_pt_y, step_pt, "/F2", 10.0);

            // Payee
            let payee_pt_x = template.payee_x_mm * POINTS_PER_MM;
            let payee_pt_y = (template.height_mm - template.payee_y_mm) * POINTS_PER_MM;
            let formatted_payee = format!("{} ***", effective_payee);
            writer.text(&formatted_payee, payee_pt_x, payee_pt_y, "/F2", 10.0);

            // Amount in Indian Words
            let words = number_to_indian_words(amount_dec);
            let words_pt_x = template.words_x_mm * POINTS_PER_MM;
            let words_pt_y = (template.height_mm - template.words_y_mm) * POINTS_PER_MM;

            if words.len() > 52 {
                // Split cleanly at last space before character 52
                let split_idx = words[..52].rfind(' ').unwrap_or(52);
                let line1 = &words[..split_idx];
                let line2 = words[split_idx..].trim();
                writer.text(line1, words_pt_x, words_pt_y, "/F1", 9.0);
                writer.text(line2, words_pt_x, words_pt_y - 12.0, "/F1", 9.0);
            } else {
                writer.text(&words, words_pt_x, words_pt_y, "/F1", 9.0);
            }

            // Numeric Amount: ** 50,000.00/-
            let amt_pt_x = template.amount_x_mm * POINTS_PER_MM;
            let amt_pt_y = (template.height_mm - template.amount_y_mm) * POINTS_PER_MM;
            let formatted_num = format!("** {}/-", format_indian_currency(amount_dec));
            writer.text(&formatted_num, amt_pt_x, amt_pt_y, "/F2", 11.0);

            Ok(writer.build())
        })
    })
    .await
}

/// Helper to resolve a cheque template from DB or fallback default.
async fn resolve_cheque_template(
    conn: &mut sqlx::SqliteConnection,
    company_id: &str,
    template_id: Option<&str>,
    bank_ledger_id: Option<&str>,
) -> Result<ChequeTemplateDto, OpenTallyError> {
    #[allow(clippy::type_complexity)]
    let query_result: Option<(
        String,
        String,
        String,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        i64,
    )> = if let Some(tid) = template_id {
        sqlx::query_as(
            r#"
            SELECT id, company_id, bank_name, width_mm, height_mm,
                   date_x_mm, date_y_mm, payee_x_mm, payee_y_mm,
                   words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default
            FROM cheque_templates
            WHERE company_id = ? AND id = ?
            "#,
        )
        .bind(company_id)
        .bind(tid)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
    } else if let Some(b_id) = bank_ledger_id {
        // Match by bank ledger name
        let b_name: Option<String> =
            sqlx::query_scalar("SELECT name FROM ledgers WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(b_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

        if let Some(name) = b_name {
            sqlx::query_as(
                r#"
                SELECT id, company_id, bank_name, width_mm, height_mm,
                       date_x_mm, date_y_mm, payee_x_mm, payee_y_mm,
                       words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default
                FROM cheque_templates
                WHERE company_id = ? AND (? LIKE '%' || bank_name || '%' OR bank_name LIKE '%' || ? || '%')
                LIMIT 1
                "#,
            )
            .bind(company_id)
            .bind(&name)
            .bind(&name)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
        } else {
            None
        }
    } else {
        None
    };

    let final_row = match query_result {
        Some(r) => Some(r),
        None => {
            // Pick first template for company
            sqlx::query_as(
                r#"
                SELECT id, company_id, bank_name, width_mm, height_mm,
                       date_x_mm, date_y_mm, payee_x_mm, payee_y_mm,
                       words_x_mm, words_y_mm, amount_x_mm, amount_y_mm, is_account_payee_default
                FROM cheque_templates
                WHERE company_id = ?
                ORDER BY rowid ASC LIMIT 1
                "#,
            )
            .bind(company_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?
        }
    };

    if let Some((
        id,
        comp_id,
        b_name,
        w_mm,
        h_mm,
        d_x,
        d_y,
        p_x,
        p_y,
        w_x,
        w_y,
        a_x,
        a_y,
        ac_payee,
    )) = final_row
    {
        Ok(ChequeTemplateDto {
            id,
            company_id: comp_id,
            bank_name: b_name,
            width_mm: w_mm,
            height_mm: h_mm,
            date_x_mm: d_x,
            date_y_mm: d_y,
            payee_x_mm: p_x,
            payee_y_mm: p_y,
            words_x_mm: w_x,
            words_y_mm: w_y,
            amount_x_mm: a_x,
            amount_y_mm: a_y,
            is_account_payee_default: ac_payee != 0,
        })
    } else {
        // Fallback default CTS-2010
        Ok(ChequeTemplateDto {
            company_id: company_id.to_string(),
            ..Default::default()
        })
    }
}

/// Generates and exports a cheque PDF to disk, returning the saved absolute path.
pub async fn export_cheque_pdf(
    pool: &Pool<Sqlite>,
    company_id: &str,
    voucher_id: &str,
    template_id: Option<&str>,
    is_account_payee: Option<bool>,
    output_dir: Option<&str>,
) -> Result<String, OpenTallyError> {
    let pdf_bytes =
        generate_cheque_pdf_bytes(pool, company_id, voucher_id, template_id, is_account_payee)
            .await?;

    let dir = output_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    std::fs::create_dir_all(&dir).map_err(|e| {
        OpenTallyError::Internal(format!(
            "Failed to create cheque PDF output directory: {}",
            e
        ))
    })?;

    let file_name = format!(
        "Cheque_{}_{}.pdf",
        voucher_id,
        chrono::Utc::now().timestamp()
    );
    let target_path = dir.join(file_name);

    std::fs::write(&target_path, pdf_bytes).map_err(|e| {
        OpenTallyError::Internal(format!("Failed to write cheque PDF to disk: {}", e))
    })?;

    Ok(target_path.to_string_lossy().to_string())
}
