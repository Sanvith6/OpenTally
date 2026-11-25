use super::models::*;
use super::parser::parse_tally_xml;
use super::security::sanitize_and_decode_xml;
use crate::errors::OpenTallyError;
use crate::vouchers::model::{ValidationMode, VoucherLineInput};
use crate::vouchers::service::DoubleEntryValidator;
use rust_decimal::Decimal;
use sqlx::{Pool, Sqlite};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Generates a read-only dry-run preview of importing a Tally XML file into a company.
pub async fn preview_tally_xml_import(
    pool: &Pool<Sqlite>,
    company_id: &str,
    xml_content: &str,
) -> Result<TallyXmlImportPreview, OpenTallyError> {
    let sanitized_xml = sanitize_and_decode_xml(xml_content.as_bytes())?;
    let doc = parse_tally_xml(&sanitized_xml)?;

    // Step 1: Read company metadata
    #[derive(sqlx::FromRow)]
    struct CompMeta {
        books_beginning_from: String,
    }

    let comp_meta: Option<CompMeta> =
        sqlx::query_as("SELECT books_beginning_from FROM companies WHERE id = ?")
            .bind(company_id)
            .fetch_optional(pool)
            .await
            .map_err(OpenTallyError::Database)?;

    let books_beginning = match comp_meta {
        Some(m) => m.books_beginning_from,
        None => {
            return Err(OpenTallyError::NotFound(format!(
                "Company '{}' not found",
                company_id
            )))
        }
    };

    // Step 2: Fetch existing masters for deduplication and referential integrity
    let existing_groups: HashSet<String> =
        sqlx::query_scalar("SELECT LOWER(name) FROM account_groups WHERE company_id = ?")
            .bind(company_id)
            .fetch_all(pool)
            .await
            .map_err(OpenTallyError::Database)?
            .into_iter()
            .collect();

    let existing_ledgers: HashSet<String> =
        sqlx::query_scalar("SELECT LOWER(name) FROM ledgers WHERE company_id = ?")
            .bind(company_id)
            .fetch_all(pool)
            .await
            .map_err(OpenTallyError::Database)?
            .into_iter()
            .collect();

    let existing_stock_items: HashSet<String> =
        sqlx::query_scalar("SELECT LOWER(name) FROM stock_items WHERE company_id = ?")
            .bind(company_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let existing_vouchers: HashSet<String> =
        sqlx::query_scalar("SELECT LOWER(voucher_number) FROM vouchers WHERE company_id = ?")
            .bind(company_id)
            .fetch_all(pool)
            .await
            .map_err(OpenTallyError::Database)?
            .into_iter()
            .collect();

    let mut errors = Vec::new();
    let mut known_groups = existing_groups.clone();
    for g in &doc.groups {
        known_groups.insert(g.name.to_lowercase());
    }
    let mut known_ledgers = existing_ledgers.clone();
    for l in &doc.ledgers {
        known_ledgers.insert(l.name.to_lowercase());
    }

    // 1. Group Preview
    let mut groups_to_create = 0;
    let mut groups_skipped = 0;
    for g in &doc.groups {
        let name_lower = g.name.to_lowercase();
        if existing_groups.contains(&name_lower) {
            groups_skipped += 1;
        } else {
            groups_to_create += 1;

            if let Some(ref p) = g.parent {
                let p_lower = p.to_lowercase();
                if p_lower != "primary" && !known_groups.contains(&p_lower) {
                    errors.push(TallyXmlValidationError {
                        item_type: "GROUP".to_string(),
                        identifier: g.name.clone(),
                        message: format!(
                            "Parent group '{}' not found in database or import payload",
                            p
                        ),
                        severity: "FATAL".to_string(),
                    });
                }
            }
        }
    }

    // 2. Ledger Preview
    let mut ledgers_to_create = 0;
    let mut ledgers_skipped = 0;
    for l in &doc.ledgers {
        let name_lower = l.name.to_lowercase();
        if existing_ledgers.contains(&name_lower) {
            ledgers_skipped += 1;
        } else {
            ledgers_to_create += 1;
            known_ledgers.insert(name_lower.clone());

            let p_lower = l.parent.to_lowercase();
            if !known_groups.contains(&p_lower) {
                errors.push(TallyXmlValidationError {
                    item_type: "LEDGER".to_string(),
                    identifier: l.name.clone(),
                    message: format!(
                        "Parent group '{}' not found for ledger '{}'",
                        l.parent, l.name
                    ),
                    severity: "FATAL".to_string(),
                });
            }
        }
    }

    // 3. Stock Items Preview
    let mut stock_items_to_create = 0;
    let mut stock_items_skipped = 0;
    for itm in &doc.stock_items {
        let name_lower = itm.name.to_lowercase();
        if existing_stock_items.contains(&name_lower) {
            stock_items_skipped += 1;
        } else {
            stock_items_to_create += 1;
        }
    }

    // 4. Vouchers Preview
    let mut vouchers_to_create = 0;
    let mut vouchers_skipped = 0;
    for v in &doc.vouchers {
        let v_num_lower = v.voucher_number.to_lowercase();
        if existing_vouchers.contains(&v_num_lower) {
            vouchers_skipped += 1;
            continue;
        }

        // Validate date against books beginning
        if v.date < books_beginning {
            errors.push(TallyXmlValidationError {
                item_type: "VOUCHER".to_string(),
                identifier: v.voucher_number.clone(),
                message: format!(
                    "Voucher date '{}' is before company books beginning date '{}'",
                    v.date, books_beginning
                ),
                severity: "FATAL".to_string(),
            });
        }

        // Validate lines exist
        if v.lines.is_empty() {
            errors.push(TallyXmlValidationError {
                item_type: "VOUCHER".to_string(),
                identifier: v.voucher_number.clone(),
                message: "Voucher has no ledger entry lines".to_string(),
                severity: "FATAL".to_string(),
            });
            continue;
        }

        // Validate double-entry equilibrium
        let mut total_dr = Decimal::ZERO;
        let mut total_cr = Decimal::ZERO;

        for line in &v.lines {
            let l_lower = line.ledger_name.to_lowercase();
            if !known_ledgers.contains(&l_lower) {
                errors.push(TallyXmlValidationError {
                    item_type: "VOUCHER".to_string(),
                    identifier: v.voucher_number.clone(),
                    message: format!(
                        "Referenced ledger '{}' does not exist in company or import payload",
                        line.ledger_name
                    ),
                    severity: "FATAL".to_string(),
                });
            }

            if line.entry_type == "Dr" {
                total_dr += line.amount;
            } else {
                total_cr += line.amount;
            }
        }

        if total_dr != total_cr {
            errors.push(TallyXmlValidationError {
                item_type: "VOUCHER".to_string(),
                identifier: v.voucher_number.clone(),
                message: format!(
                    "Double-entry equilibrium violation: Total Dr ({}) != Total Cr ({})",
                    total_dr, total_cr
                ),
                severity: "FATAL".to_string(),
            });
        }

        vouchers_to_create += 1;
    }

    let can_import = errors.iter().all(|e| e.severity != "FATAL");

    Ok(TallyXmlImportPreview {
        groups_to_create,
        groups_skipped,
        ledgers_to_create,
        ledgers_skipped,
        stock_items_to_create,
        stock_items_skipped,
        vouchers_to_create,
        vouchers_skipped,
        can_import,
        errors,
    })
}

/// Executes an atomic import of Tally XML data within a single SQLite transaction.
/// Rolls back completely if any fatal validation or persistence error occurs.
pub async fn execute_tally_xml_import(
    pool: &Pool<Sqlite>,
    company_id: &str,
    xml_content: &str,
) -> Result<TallyXmlImportResult, OpenTallyError> {
    let preview = preview_tally_xml_import(pool, company_id, xml_content).await?;
    if !preview.can_import {
        let fatal_count = preview
            .errors
            .iter()
            .filter(|e| e.severity == "FATAL")
            .count();
        return Err(OpenTallyError::Validation(format!(
            "Atomic XML import aborted: {} blocking validation error(s) detected",
            fatal_count
        )));
    }

    let sanitized_xml = sanitize_and_decode_xml(xml_content.as_bytes())?;
    let doc = parse_tally_xml(&sanitized_xml)?;

    let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

    // Fetch dynamic company financial year beginning for MCA audit log
    #[derive(sqlx::FromRow)]
    struct CompInfo {
        financial_year_begins: String,
        currency_decimal_places: i64,
    }

    let comp_info: CompInfo = sqlx::query_as(
        "SELECT financial_year_begins, currency_decimal_places FROM companies WHERE id = ?",
    )
    .bind(company_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    let mut groups_created = 0;
    let mut ledgers_created = 0;
    let mut stock_items_created = 0;
    let mut vouchers_created = 0;

    // Cache existing account group name -> id
    #[derive(sqlx::FromRow)]
    struct GroupMapRow {
        id: String,
        name: String,
        nature: String,
        affects_gross_profit: i64,
    }

    let mut group_map: HashMap<String, (String, String, i64)> = sqlx::query_as::<_, GroupMapRow>(
        "SELECT id, name, nature, affects_gross_profit FROM account_groups WHERE company_id = ?",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?
    .into_iter()
    .map(|r| {
        (
            r.name.to_lowercase(),
            (r.id, r.nature, r.affects_gross_profit),
        )
    })
    .collect();

    // 1. Create Units
    for u in &doc.units {
        let unit_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM units WHERE company_id = ? AND symbol = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&u.symbol)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if unit_exists.is_none() {
            let u_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO units (id, company_id, symbol, formal_name, unit_type, decimal_precision)
                 VALUES (?, ?, ?, ?, 'simple', ?)",
            )
            .bind(&u_id)
            .bind(company_id)
            .bind(&u.symbol)
            .bind(&u.formal_name)
            .bind(u.decimal_places as i64)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }
    }

    // 2. Create Godowns
    for gd in &doc.godowns {
        let gd_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM godowns WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&gd.name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if gd_exists.is_none() {
            let gd_id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO godowns (id, company_id, name) VALUES (?, ?, ?)")
                .bind(&gd_id)
                .bind(company_id)
                .bind(&gd.name)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;
        }
    }

    // 3. Create Stock Groups
    for sg in &doc.stock_groups {
        let sg_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM stock_groups WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&sg.name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if sg_exists.is_none() {
            let sg_id = Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO stock_groups (id, company_id, name) VALUES (?, ?, ?)")
                .bind(&sg_id)
                .bind(company_id)
                .bind(&sg.name)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;
        }
    }

    // 4. Create Account Groups
    for g in &doc.groups {
        let name_lower = g.name.to_lowercase();
        if group_map.contains_key(&name_lower) {
            continue;
        }

        let parent_info = if let Some(ref p) = g.parent {
            if p.eq_ignore_ascii_case("primary") {
                None
            } else {
                group_map.get(&p.to_lowercase()).cloned()
            }
        } else {
            None
        };

        let parent_id = parent_info.as_ref().map(|(id, _, _)| id.clone());
        let is_primary = if parent_id.is_none() { 1 } else { 0 };
        let nature = parent_info
            .as_ref()
            .map(|(_, n, _)| n.clone())
            .unwrap_or_else(|| "Assets".to_string());
        let affects_gp = parent_info.as_ref().map(|(_, _, agp)| *agp).unwrap_or(0);

        let g_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, is_primary, is_predefined, nature, affects_gross_profit)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .bind(&g_id)
        .bind(company_id)
        .bind(&g.name)
        .bind(&parent_id)
        .bind(is_primary)
        .bind(&nature)
        .bind(affects_gp)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        group_map.insert(name_lower, (g_id, nature, affects_gp));
        groups_created += 1;
    }

    // 5. Create Ledgers
    let mut ledger_map: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT LOWER(name), id FROM ledgers WHERE company_id = ?",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?
    .into_iter()
    .collect();

    for l in &doc.ledgers {
        let name_lower = l.name.to_lowercase();
        if ledger_map.contains_key(&name_lower) {
            continue;
        }

        let group_id = group_map
            .get(&l.parent.to_lowercase())
            .map(|(id, _, _)| id.clone())
            .ok_or_else(|| {
                OpenTallyError::Validation(format!(
                    "Parent group '{}' not found for ledger '{}'",
                    l.parent, l.name
                ))
            })?;

        let l_id = Uuid::new_v4().to_string();
        let op_type = if l.is_deemed_positive { "Dr" } else { "Cr" };
        let op_amt_str = l.opening_balance.normalize().to_string();

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, name, group_id, is_reserved, ledger_classification)
             VALUES (?, ?, ?, ?, 0, 'standard')",
        )
        .bind(&l_id)
        .bind(company_id)
        .bind(&l.name)
        .bind(&group_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if l.opening_balance > Decimal::ZERO {
            let ob_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&ob_id)
            .bind(company_id)
            .bind(&l_id)
            .bind(&comp_info.financial_year_begins)
            .bind(op_type)
            .bind(&op_amt_str)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        ledger_map.insert(name_lower, l_id);
        ledgers_created += 1;
    }

    // 6. Create Stock Items
    let default_group_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM stock_groups WHERE company_id = ? LIMIT 1")
            .bind(company_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or_default();

    let default_unit_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM units WHERE company_id = ? LIMIT 1")
            .bind(company_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or_default();

    for itm in &doc.stock_items {
        let itm_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM stock_items WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&itm.name)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or_default();

        if itm_exists.is_none() {
            let sg_id = if let Some(ref p) = itm.parent {
                sqlx::query_scalar(
                    "SELECT id FROM stock_groups WHERE company_id = ? AND name = ? COLLATE NOCASE",
                )
                .bind(company_id)
                .bind(p)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or_default()
            } else {
                default_group_id.clone()
            };

            let u_id = if !itm.base_units.is_empty() {
                sqlx::query_scalar(
                    "SELECT id FROM units WHERE company_id = ? AND symbol = ? COLLATE NOCASE",
                )
                .bind(company_id)
                .bind(&itm.base_units)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or_default()
            } else {
                default_unit_id.clone()
            };

            if let (Some(sg), Some(u)) = (
                sg_id.or_else(|| default_group_id.clone()),
                u_id.or_else(|| default_unit_id.clone()),
            ) {
                let itm_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id,
                                             opening_quantity, opening_rate, opening_value)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&itm_id)
                .bind(company_id)
                .bind(&itm.name)
                .bind(&sg)
                .bind(&u)
                .bind(itm.opening_balance_qty.normalize().to_string())
                .bind(itm.opening_balance_rate.normalize().to_string())
                .bind(itm.opening_balance_val.normalize().to_string())
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                stock_items_created += 1;
            }
        }
    }

    // 7. Create Vouchers
    let voucher_type_map: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT LOWER(name), id FROM voucher_types WHERE company_id = ?",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?
    .into_iter()
    .collect();

    for v in &doc.vouchers {
        let v_exists: Option<String> = sqlx::query_scalar(
            "SELECT id FROM vouchers WHERE company_id = ? AND voucher_number = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(&v.voucher_number)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if v_exists.is_some() {
            continue;
        }

        // Map voucher type
        let vt_id = voucher_type_map
            .get(&v.voucher_type.to_lowercase())
            .cloned()
            .unwrap_or_else(|| {
                // Fallback mappings
                match v.voucher_type.to_lowercase().as_str() {
                    "sales" => "vt_sales".to_string(),
                    "purchase" => "vt_pur".to_string(),
                    "payment" => "vt_payment".to_string(),
                    "receipt" => "vt_receipt".to_string(),
                    "contra" => "vt_contra".to_string(),
                    "credit note" | "creditnote" => "vt_credit_note".to_string(),
                    "debit note" | "debitnote" => "vt_debit_note".to_string(),
                    _ => "vt_jnl".to_string(),
                }
            });

        // Construct lines and validate double entry
        let mut line_inputs = Vec::new();
        for (idx, line) in v.lines.iter().enumerate() {
            let l_id = ledger_map
                .get(&line.ledger_name.to_lowercase())
                .ok_or_else(|| {
                    OpenTallyError::Validation(format!(
                        "Ledger '{}' not found for voucher '{}'",
                        line.ledger_name, v.voucher_number
                    ))
                })?;

            line_inputs.push(VoucherLineInput {
                id: None,
                ledger_id: l_id.clone(),
                entry_type: line.entry_type.clone(),
                amount: line.amount.normalize().to_string(),
                narration: line.narration.clone(),
            });
            let _ = idx;
        }

        let val_res = DoubleEntryValidator::validate(
            &line_inputs,
            comp_info.currency_decimal_places as u32,
            ValidationMode::Posting,
        );

        if !val_res.is_valid || !val_res.is_balanced {
            let diag_msg = val_res
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(OpenTallyError::Validation(format!(
                "Double-entry validation failed for voucher '{}': {}",
                v.voucher_number, diag_msg
            )));
        }

        let total_amt: Decimal = v
            .lines
            .iter()
            .filter(|l| l.entry_type == "Dr")
            .map(|l| l.amount)
            .sum();

        let series_row: Option<(String, String)> = sqlx::query_as(
            "SELECT id, numbering_method FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = ? LIMIT 1",
        )
        .bind(company_id)
        .bind(&vt_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let (series_id, numbering_method) = match series_row {
            Some(s) => s,
            None => {
                let s_id = format!("series_imp_{}_{}", vt_id, Uuid::new_v4().simple());
                sqlx::query(
                    "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
                     VALUES (?, ?, ?, 'Import Series', 'IMP', 'manual', 'never')",
                )
                .bind(&s_id)
                .bind(company_id)
                .bind(&vt_id)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                (s_id, "manual".to_string())
            }
        };

        let (seq_num, period_start) = if numbering_method == "automatic" {
            let max_seq: Option<i64> = sqlx::query_scalar(
                "SELECT MAX(sequence_number) FROM vouchers WHERE company_id = ? AND series_id = ? AND period_start = ?",
            )
            .bind(company_id)
            .bind(&series_id)
            .bind(&comp_info.financial_year_begins)
            .fetch_optional(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?
            .flatten();

            (
                Some(max_seq.unwrap_or(0) + 1),
                Some(comp_info.financial_year_begins.clone()),
            )
        } else {
            (None, None)
        };

        let vch_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method,
                                  voucher_number, date, effective_date, narration, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'draft')",
        )
        .bind(&vch_id)
        .bind(company_id)
        .bind(&vt_id)
        .bind(&series_id)
        .bind(&numbering_method)
        .bind(&v.voucher_number)
        .bind(&v.date)
        .bind(&v.date)
        .bind(&v.narration)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        for (idx, line) in line_inputs.iter().enumerate() {
            let line_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&line_id)
            .bind(company_id)
            .bind(&vch_id)
            .bind((idx + 1) as i64)
            .bind(&line.ledger_id)
            .bind(&line.entry_type)
            .bind(&line.amount)
            .bind(&line.narration)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        sqlx::query(
            "UPDATE vouchers
             SET status = 'posted',
                 period_start = ?,
                 sequence_number = ?,
                 total_amount = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE company_id = ? AND id = ? AND status = 'draft'",
        )
        .bind(period_start)
        .bind(seq_num)
        .bind(total_amt.normalize().to_string())
        .bind(company_id)
        .bind(&vch_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        vouchers_created += 1;
    }

    // Step 8: Emit immutable MCA audit log
    let audit_id = Uuid::new_v4().to_string();
    let audit_payload = serde_json::json!({
        "action": "TALLY_XML_IMPORT",
        "groups_created": groups_created,
        "ledgers_created": ledgers_created,
        "stock_items_created": stock_items_created,
        "vouchers_created": vouchers_created,
    })
    .to_string();

    sqlx::query(
        "INSERT INTO audit_log (id, company_id, entity_type, entity_id, financial_year_begins, action, new_values, performed_by)
         VALUES (?, ?, 'tally_xml_import', ?, ?, 'CREATE', ?, 'system')",
    )
    .bind(&audit_id)
    .bind(company_id)
    .bind(&audit_id)
    .bind(&comp_info.financial_year_begins)
    .bind(&audit_payload)
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    tx.commit().await.map_err(OpenTallyError::Database)?;

    Ok(TallyXmlImportResult {
        success: true,
        groups_created,
        ledgers_created,
        stock_items_created,
        vouchers_created,
        message: format!(
            "Successfully imported {} groups, {} ledgers, {} stock items, and {} vouchers",
            groups_created, ledgers_created, stock_items_created, vouchers_created
        ),
    })
}
