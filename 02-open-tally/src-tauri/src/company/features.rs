//! Unified Company Features (F11) Service
//!
//! Provides consolidated access and atomic updates for company features:
//! - Accounting: Maintain Accounts, Bill-wise Tracking
//! - Inventory: Maintain Inventory, Perpetual COGS integration
//! - Taxation (Statutory GST): GST enablement, GSTIN validation, State Code, Registration Type
//! - Smarter Auto-Seeding: Idempotently creates and wires standard 8-ledger GST and Inventory mappings.
//! - MCA Immutable Audit Logging.

use crate::errors::OpenTallyError;
use crate::gst::validate_gstin;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyFeaturesDto {
    pub company_id: String,
    pub maintain_inventory: bool,
    pub billwise_tracking: bool,
    pub enable_perpetual_cogs: bool,
    pub gst_enabled: bool,
    pub gstin: Option<String>,
    pub state_code: Option<String>,
    pub registration_type: String,
    pub has_gst_ledgers_mapped: bool,
    pub has_inventory_ledgers_mapped: bool,
    pub tds_enabled: bool,
    pub tan: Option<String>,
    pub deductor_type: Option<String>,
    pub has_tds_sections_seeded: bool,
    #[serde(default)]
    pub enable_payroll: bool,
    #[serde(default)]
    pub has_payroll_ledgers_mapped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveCompanyFeaturesInput {
    pub maintain_inventory: bool,
    pub billwise_tracking: bool,
    pub enable_perpetual_cogs: bool,
    pub gst_enabled: bool,
    pub gstin: Option<String>,
    pub state_code: Option<String>,
    pub registration_type: Option<String>,
    #[serde(default = "default_true")]
    pub auto_seed_gst_ledgers: bool,
    #[serde(default = "default_true")]
    pub auto_seed_inventory_ledgers: bool,
    #[serde(default)]
    pub tds_enabled: Option<bool>,
    #[serde(default)]
    pub tan: Option<String>,
    #[serde(default)]
    pub deductor_type: Option<String>,
    #[serde(default = "default_true")]
    pub auto_seed_tds_defaults: bool,
    #[serde(default)]
    pub enable_payroll: Option<bool>,
    #[serde(default = "default_true")]
    pub auto_seed_payroll_ledgers: bool,
}

pub struct CompanyFeaturesService;

impl CompanyFeaturesService {
    /// Retrieve consolidated company features across settings, inventory mappings, and GST profile.
    pub async fn get_company_features(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<CompanyFeaturesDto, OpenTallyError> {
        // 1. Verify company exists
        let comp_exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM companies WHERE id = ?")
                .bind(company_id)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        if comp_exists.is_none() {
            return Err(OpenTallyError::NotFound(format!(
                "Company '{}' not found",
                company_id
            )));
        }

        // 2. Read key-value settings
        let settings: Vec<(String, String)> =
            sqlx::query_as("SELECT key, value FROM company_settings WHERE company_id = ?")
                .bind(company_id)
                .fetch_all(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        let mut maintain_inventory = true;
        let mut billwise_tracking = true;
        let mut enable_perpetual_cogs = false;
        let mut gst_enabled = false;
        let mut enable_payroll = false;

        for (k, v) in settings {
            match k.as_str() {
                "maintain_inventory" => maintain_inventory = v == "true" || v == "1",
                "enable_billwise" => billwise_tracking = v == "true" || v == "1",
                "integrate_accounts_inventory" => enable_perpetual_cogs = v == "true" || v == "1",
                "enable_gst" => gst_enabled = v == "true" || v == "1",
                "enable_payroll" => enable_payroll = v == "true" || v == "1",
                _ => {}
            }
        }

        // 3. Read GST Profile
        let gst_profile: Option<(Option<String>, String, String, i64)> = sqlx::query_as(
            "SELECT gstin, state_code, registration_type, gst_enabled FROM company_gst_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let (gstin, state_code, registration_type) = match gst_profile {
            Some((g, s, r, enabled)) => {
                gst_enabled = enabled == 1;
                (g, Some(s), r)
            }
            None => (None, None, "Regular".to_string()),
        };

        // 4. Check GST Ledgers Mapping
        let has_gst_ledgers_mapped: bool =
            sqlx::query_scalar("SELECT COUNT(*) > 0 FROM gst_ledger_mappings WHERE company_id = ?")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        // 5. Check Inventory Account Mapping
        let inv_map_row: Option<i64> = sqlx::query_scalar(
            "SELECT enabled FROM inventory_account_mappings WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let has_inventory_ledgers_mapped = inv_map_row.is_some();
        if let Some(en) = inv_map_row {
            enable_perpetual_cogs = en == 1;
        }

        // 6. Read TDS Profile and Sections
        let tds_profile: Option<(Option<String>, String, i64)> = sqlx::query_as(
            "SELECT tan, deductor_type, tds_enabled FROM company_tds_profiles WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let (tan, deductor_type, tds_enabled) = match tds_profile {
            Some((t, d, en)) => (t, Some(d), en == 1),
            None => (None, Some("Company".to_string()), false),
        };

        let has_tds_sections_seeded: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM tds_nature_of_payments WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let payroll_table_exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'payroll_ledger_mappings'",
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        let has_payroll_ledgers_mapped: bool = if payroll_table_exists {
            sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM payroll_ledger_mappings WHERE company_id = ?",
            )
            .bind(company_id)
            .fetch_one(pool)
            .await
            .map_err(OpenTallyError::Database)?
        } else {
            false
        };

        Ok(CompanyFeaturesDto {
            company_id: company_id.to_string(),
            maintain_inventory,
            billwise_tracking,
            enable_perpetual_cogs,
            gst_enabled,
            gstin,
            state_code,
            registration_type,
            has_gst_ledgers_mapped,
            has_inventory_ledgers_mapped,
            tds_enabled,
            tan,
            deductor_type,
            has_tds_sections_seeded,
            enable_payroll,
            has_payroll_ledgers_mapped,
        })
    }

    /// Atomically updates company features, validates GSTIN, and auto-seeds missing GST/Inventory ledgers.
    pub async fn save_company_features(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: SaveCompanyFeaturesInput,
    ) -> Result<CompanyFeaturesDto, OpenTallyError> {
        // Validate GSTIN if GST is enabled and GSTIN is provided
        let clean_gstin = input
            .gstin
            .as_ref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());

        let clean_state_code = if let Some(ref g) = clean_gstin {
            let sc = &g[..2];
            Some(sc.to_string())
        } else {
            input
                .state_code
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        };

        if input.gst_enabled {
            if let Some(ref g) = clean_gstin {
                validate_gstin(g, clean_state_code.as_deref())?;
            }
        }

        let is_tds_enabled = input.tds_enabled.unwrap_or(false);
        let is_payroll_enabled = input.enable_payroll.unwrap_or(false);
        let clean_tan = input
            .tan
            .as_ref()
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty());

        if is_tds_enabled {
            if let Some(ref t) = clean_tan {
                crate::tds::validate_tan(t)?;
            }
        }

        let deductor_type_val = input.deductor_type.unwrap_or_else(|| "Company".to_string());

        let registration_type = input
            .registration_type
            .unwrap_or_else(|| "Regular".to_string());

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let update_res: Result<(), OpenTallyError> = async {
            // 1. Update company_settings
            let settings_to_upsert = [
                ("maintain_inventory", if input.maintain_inventory { "true" } else { "false" }),
                ("enable_billwise", if input.billwise_tracking { "true" } else { "false" }),
                ("integrate_accounts_inventory", if input.enable_perpetual_cogs { "true" } else { "false" }),
                ("enable_gst", if input.gst_enabled { "true" } else { "false" }),
                ("enable_payroll", if is_payroll_enabled { "true" } else { "false" }),
            ];

            for (k, v) in settings_to_upsert {
                sqlx::query(
                    "INSERT INTO company_settings (company_id, key, value, updated_at)
                     VALUES (?, ?, ?, CURRENT_TIMESTAMP)
                     ON CONFLICT(company_id, key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
                )
                .bind(company_id)
                .bind(k)
                .bind(v)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }

            // 2. Upsert company_gst_profiles
            let state_code_to_use = clean_state_code.clone().unwrap_or_else(|| "29".to_string());
            sqlx::query(
                "INSERT INTO company_gst_profiles (company_id, gstin, state_code, registration_type, gst_enabled, updated_at)
                 VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT(company_id) DO UPDATE SET
                     gstin = excluded.gstin,
                     state_code = excluded.state_code,
                     registration_type = excluded.registration_type,
                     gst_enabled = excluded.gst_enabled,
                     updated_at = CURRENT_TIMESTAMP",
            )
            .bind(company_id)
            .bind(&clean_gstin)
            .bind(&state_code_to_use)
            .bind(&registration_type)
            .bind(if input.gst_enabled { 1 } else { 0 })
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            let fy_begins: String = sqlx::query_scalar(
                "SELECT financial_year_begins FROM companies WHERE id = ?",
            )
            .bind(company_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            // 3. Auto-seed standard GST Ledgers if requested and enabled
            if input.gst_enabled && input.auto_seed_gst_ledgers {
                Self::ensure_gst_ledgers_and_mappings(&mut conn, company_id, &fy_begins).await?;
            }

            // 4. Auto-seed standard Inventory Ledgers and mappings if requested and enabled
            if input.maintain_inventory && input.auto_seed_inventory_ledgers {
                Self::ensure_inventory_ledgers_and_mappings(
                    &mut conn,
                    company_id,
                    input.enable_perpetual_cogs,
                    &fy_begins,
                )
                .await?;
            } else if !input.maintain_inventory {
                // If inventory is disabled, ensure perpetual COGS is marked disabled
                sqlx::query(
                    "UPDATE inventory_account_mappings SET enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE company_id = ?",
                )
                .bind(company_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            }

            // 5. Save TDS Profile and Auto-seed Sections if requested
            let tds_en_int = if is_tds_enabled { 1i64 } else { 0i64 };
            sqlx::query(
                r#"
                INSERT INTO company_tds_profiles (company_id, tan, deductor_type, tds_enabled, updated_at)
                VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(company_id) DO UPDATE SET
                    tan = excluded.tan,
                    deductor_type = excluded.deductor_type,
                    tds_enabled = excluded.tds_enabled,
                    updated_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(company_id)
            .bind(&clean_tan)
            .bind(&deductor_type_val)
            .bind(tds_en_int)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if is_tds_enabled && input.auto_seed_tds_defaults {
                crate::tds::TdsService::seed_default_tds_sections_tx(&mut conn, company_id).await?;
            }

            if is_payroll_enabled && input.auto_seed_payroll_ledgers {
                Self::ensure_payroll_ledgers_and_mappings(&mut conn, company_id, &fy_begins).await?;
            }

            // 6. Emit MCA Immutable Audit Log
            let audit_id = Uuid::new_v4().to_string();
            let payload = serde_json::json!({
                "maintain_inventory": input.maintain_inventory,
                "billwise_tracking": input.billwise_tracking,
                "enable_perpetual_cogs": input.enable_perpetual_cogs,
                "gst_enabled": input.gst_enabled,
                "gstin": clean_gstin,
                "state_code": state_code_to_use,
                "registration_type": registration_type,
                "tds_enabled": is_tds_enabled,
                "tan": clean_tan,
                "deductor_type": deductor_type_val,
                "enable_payroll": is_payroll_enabled,
            });

            sqlx::query(
                "INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, new_values, performed_by)
                 VALUES (?, ?, 'company_features', ?, 'UPDATE', ?, 'system')",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(company_id)
            .bind(payload.to_string())
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            Ok(())
        }
        .await;

        match update_res {
            Ok(()) => {
                sqlx::query("COMMIT")
                    .execute(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;
                Self::get_company_features(pool, company_id).await
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    /// Idempotently creates Duties & Taxes ledgers and maps them in `gst_ledger_mappings`.
    async fn ensure_gst_ledgers_and_mappings(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        fy_begins: &str,
    ) -> Result<(), OpenTallyError> {
        // Find Duties & Taxes group
        let duties_taxes_group_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Duties & Taxes' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Find or create standard GST ledgers
        let out_cgst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Output CGST",
            &duties_taxes_group_id,
            "Cr",
            fy_begins,
        )
        .await?;
        let out_sgst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Output SGST",
            &duties_taxes_group_id,
            "Cr",
            fy_begins,
        )
        .await?;
        let out_igst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Output IGST",
            &duties_taxes_group_id,
            "Cr",
            fy_begins,
        )
        .await?;
        let out_cess = Self::find_or_create_ledger(
            conn,
            company_id,
            "Output Cess",
            &duties_taxes_group_id,
            "Cr",
            fy_begins,
        )
        .await?;

        let in_cgst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Input CGST",
            &duties_taxes_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let in_sgst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Input SGST",
            &duties_taxes_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let in_igst = Self::find_or_create_ledger(
            conn,
            company_id,
            "Input IGST",
            &duties_taxes_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let in_cess = Self::find_or_create_ledger(
            conn,
            company_id,
            "Input Cess",
            &duties_taxes_group_id,
            "Dr",
            fy_begins,
        )
        .await?;

        // Find Sales Accounts and Purchase Accounts groups
        let sales_group_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sales Accounts' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(ref gid) = sales_group_id {
            Self::find_or_create_ledger(conn, company_id, "Sales Account", gid, "Cr", fy_begins)
                .await?;
        }

        let pur_group_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Purchase Accounts' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(ref gid) = pur_group_id {
            Self::find_or_create_ledger(conn, company_id, "Purchase Account", gid, "Dr", fy_begins)
                .await?;
        }

        // Upsert gst_ledger_mappings
        sqlx::query(
            "INSERT INTO gst_ledger_mappings (
                company_id, output_cgst_ledger_id, output_sgst_ledger_id, output_igst_ledger_id, output_cess_ledger_id,
                input_cgst_ledger_id, input_sgst_ledger_id, input_igst_ledger_id, input_cess_ledger_id, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET
                output_cgst_ledger_id = excluded.output_cgst_ledger_id,
                output_sgst_ledger_id = excluded.output_sgst_ledger_id,
                output_igst_ledger_id = excluded.output_igst_ledger_id,
                output_cess_ledger_id = excluded.output_cess_ledger_id,
                input_cgst_ledger_id = excluded.input_cgst_ledger_id,
                input_sgst_ledger_id = excluded.input_sgst_ledger_id,
                input_igst_ledger_id = excluded.input_igst_ledger_id,
                input_cess_ledger_id = excluded.input_cess_ledger_id,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(company_id)
        .bind(&out_cgst)
        .bind(&out_sgst)
        .bind(&out_igst)
        .bind(Some(&out_cess))
        .bind(&in_cgst)
        .bind(&in_sgst)
        .bind(&in_igst)
        .bind(Some(&in_cess))
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }

    /// Idempotently creates Inventory ledgers and sets up `inventory_account_mappings`.
    async fn ensure_inventory_ledgers_and_mappings(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        enabled: bool,
        fy_begins: &str,
    ) -> Result<(), OpenTallyError> {
        // Stock-in-Hand group
        let stock_group_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Stock-in-Hand' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Direct Expenses group
        let direct_exp_group_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Direct Expenses' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Direct Incomes group
        let direct_inc_group_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Direct Incomes' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Current Liabilities group (for offset)
        let curr_liab_group_id: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Current Liabilities' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let stock_ledger_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Stock-in-Hand",
            &stock_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let cogs_ledger_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Cost of Goods Sold",
            &direct_exp_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let adj_exp_ledger_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Inventory Adjustment Expense",
            &direct_exp_group_id,
            "Dr",
            fy_begins,
        )
        .await?;
        let adj_gain_ledger_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Inventory Adjustment Gain",
            &direct_inc_group_id,
            "Cr",
            fy_begins,
        )
        .await?;
        let offset_ledger_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Purchase Offset / Goods Inward Offset",
            &curr_liab_group_id,
            "Cr",
            fy_begins,
        )
        .await?;

        // Find or create Journal Series
        let journal_series_id: String = match sqlx::query_scalar::<_, String>(
            "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = 'vt_jnl' LIMIT 1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        {
            Some(sid) => sid,
            None => {
                let sid = format!("series_int_jnl_{}", Uuid::new_v4().simple());
                sqlx::query(
                    "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
                     VALUES (?, ?, 'vt_jnl', 'Integration Journal', 'INT', 'automatic', 'never')",
                )
                .bind(&sid)
                .bind(company_id)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
                sid
            }
        };

        // Upsert inventory_account_mappings
        sqlx::query(
            "INSERT INTO inventory_account_mappings (
                company_id, inventory_asset_ledger_id, adjustment_expense_ledger_id,
                adjustment_gain_ledger_id, cogs_ledger_id, receipt_offset_ledger_id,
                journal_series_id, enabled, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET
                inventory_asset_ledger_id = excluded.inventory_asset_ledger_id,
                adjustment_expense_ledger_id = excluded.adjustment_expense_ledger_id,
                adjustment_gain_ledger_id = excluded.adjustment_gain_ledger_id,
                cogs_ledger_id = excluded.cogs_ledger_id,
                receipt_offset_ledger_id = excluded.receipt_offset_ledger_id,
                journal_series_id = excluded.journal_series_id,
                enabled = excluded.enabled,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(company_id)
        .bind(&stock_ledger_id)
        .bind(&adj_exp_ledger_id)
        .bind(Some(&adj_gain_ledger_id))
        .bind(&cogs_ledger_id)
        .bind(&offset_ledger_id)
        .bind(&journal_series_id)
        .bind(if enabled { 1 } else { 0 })
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }

    /// Idempotently creates standard 7 Payroll ledgers and maps them in `payroll_ledger_mappings`.
    pub(crate) async fn ensure_payroll_ledgers_and_mappings(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        fy_begins: &str,
    ) -> Result<(), OpenTallyError> {
        let indirect_expenses_gid: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Indirect Expenses' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let current_liabilities_gid: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Current Liabilities' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let duties_taxes_gid: String = sqlx::query_scalar(
            "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Duties & Taxes' COLLATE NOCASE",
        )
        .bind(company_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let salary_expense_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Salary Expense",
            &indirect_expenses_gid,
            "Dr",
            fy_begins,
        )
        .await?;
        let employer_pf_expense_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Employer PF Contribution",
            &indirect_expenses_gid,
            "Dr",
            fy_begins,
        )
        .await?;
        let employer_esi_expense_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Employer ESI Contribution",
            &indirect_expenses_gid,
            "Dr",
            fy_begins,
        )
        .await?;
        let salary_payable_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Salary Payable",
            &current_liabilities_gid,
            "Cr",
            fy_begins,
        )
        .await?;
        let pf_payable_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "PF Payable",
            &duties_taxes_gid,
            "Cr",
            fy_begins,
        )
        .await?;
        let esi_payable_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "ESI Payable",
            &duties_taxes_gid,
            "Cr",
            fy_begins,
        )
        .await?;
        let pt_payable_id = Self::find_or_create_ledger(
            conn,
            company_id,
            "Professional Tax Payable",
            &duties_taxes_gid,
            "Cr",
            fy_begins,
        )
        .await?;

        sqlx::query(
            "INSERT INTO payroll_ledger_mappings (
                company_id, salary_expense_ledger_id, employer_pf_expense_ledger_id,
                employer_esi_expense_ledger_id, salary_payable_ledger_id,
                pf_payable_ledger_id, esi_payable_ledger_id, pt_payable_ledger_id, updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(company_id) DO UPDATE SET
                salary_expense_ledger_id = excluded.salary_expense_ledger_id,
                employer_pf_expense_ledger_id = excluded.employer_pf_expense_ledger_id,
                employer_esi_expense_ledger_id = excluded.employer_esi_expense_ledger_id,
                salary_payable_ledger_id = excluded.salary_payable_ledger_id,
                pf_payable_ledger_id = excluded.pf_payable_ledger_id,
                esi_payable_ledger_id = excluded.esi_payable_ledger_id,
                pt_payable_ledger_id = excluded.pt_payable_ledger_id,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(company_id)
        .bind(&salary_expense_id)
        .bind(&employer_pf_expense_id)
        .bind(&employer_esi_expense_id)
        .bind(&salary_payable_id)
        .bind(&pf_payable_id)
        .bind(&esi_payable_id)
        .bind(&pt_payable_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(())
    }

    /// Helper to find existing ledger by name or create a new one.
    async fn find_or_create_ledger(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        name: &str,
        group_id: &str,
        balance_type: &str,
        fy_begins: &str,
    ) -> Result<String, OpenTallyError> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM ledgers WHERE company_id = ? AND name = ? COLLATE NOCASE",
        )
        .bind(company_id)
        .bind(name)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(id) = existing {
            return Ok(id);
        }

        let ledger_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES (?, ?, ?, ?, 0, 'standard')",
        )
        .bind(&ledger_id)
        .bind(company_id)
        .bind(group_id)
        .bind(name)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Opening balance 0
        let opb_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES (?, ?, ?, ?, ?, '0.00')",
        )
        .bind(&opb_id)
        .bind(company_id)
        .bind(&ledger_id)
        .bind(fy_begins)
        .bind(balance_type)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(ledger_id)
    }
}
