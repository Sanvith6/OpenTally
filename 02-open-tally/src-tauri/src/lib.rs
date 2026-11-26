pub mod accounting;
pub mod app;
pub mod audit;
pub mod backup;
pub mod banking;
pub mod billwise;
pub mod commands;
pub mod commercial;
pub mod company;
pub mod config;
pub mod currency;
pub mod database;
pub mod errors;
pub mod extensions;
pub mod gst;
pub mod interoperability;
pub mod inventory;
pub mod lan;
pub mod logging;
pub mod navigation;
pub mod orders;
pub mod party;
pub mod payroll;
pub mod pdf;
pub mod purchase;
pub mod reports;
pub mod sales;
pub mod security;
pub mod tds;
pub mod vouchers;

#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod banking_tests;
#[cfg(test)]
mod billwise_tests;
#[cfg(test)]
mod closing_budgets_audit_tests;
#[cfg(test)]
mod commercial_tests;
#[cfg(test)]
mod godown_batch_costing_tests;
#[cfg(test)]
mod gsp_extensions_tests;
#[cfg(test)]
mod gst_compliance_tests;
#[cfg(test)]
mod gst_tests;
#[cfg(test)]
mod interoperability_tests;
#[cfg(test)]
mod lan_multiuser_tests;
#[cfg(test)]
mod multicurrency_cheque_tests;
#[cfg(test)]
mod navigation_tests;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod onboarding_tests;
#[cfg(test)]
mod orders_tests;
#[cfg(test)]
mod payroll_tests;
#[cfg(test)]
mod pdf_tests;
#[cfg(test)]
mod security_bom_tests;
#[cfg(test)]
mod tds_tests;
#[cfg(test)]
mod v1_release_e2e_tests;

use crate::app::AppState;
use crate::config::AppConfig;
use crate::database::DatabaseManager;
use std::sync::Arc;
use tauri::Manager;
use tracing::info;

/// Run the OpenTally desktop application.
pub fn run() {
    logging::init_logging();
    info!("Starting OpenTally desktop application runtime...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle();
            let app_dir = match std::env::var("OPENTALLY_DATA_DIR") {
                Ok(dir) if !dir.trim().is_empty() => std::path::PathBuf::from(dir),
                _ => app_handle
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from(".")),
            };

            std::fs::create_dir_all(&app_dir)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let db_path = app_dir.join("opentally.db");

            info!("Configuring database path at: {:?}", db_path);

            let db = tauri::async_runtime::block_on(async {
                let m = DatabaseManager::init(&db_path).await?;
                crate::database::migrations::run_all_migrations(m.pool()).await?;
                Ok::<_, crate::errors::OpenTallyError>(m)
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            let config = AppConfig::new().with_db_path(db_path.clone());
            let state = AppState::new(config, Arc::new(db));

            if std::env::var("OPENTALLY_LAN_HOST").as_deref() == Ok("1") {
                let pool = state.db.pool().clone();
                let handle = tauri::async_runtime::block_on(async {
                    crate::lan::start_lan_listener(pool, None, None).await
                })
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                if let Ok(mut lock) = state.lan_server.write() {
                    *lock = Some(handle);
                }
            }

            app.manage(state);

            info!("OpenTally core backend initialized successfully.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_health,
            commands::create_company,
            commands::list_companies,
            commands::get_company,
            commands::select_company,
            commands::get_active_company,
            commands::update_company,
            commands::archive_company,
            commands::get_chart_of_accounts,
            commands::list_account_groups,
            commands::create_account_group,
            commands::update_account_group,
            commands::delete_account_group,
            commands::list_ledgers,
            commands::get_ledger,
            commands::list_parties,
            commands::get_party,
            commands::create_party,
            commands::update_party,
            commands::set_party_active,
            commands::list_sales_drafts,
            commands::get_sales_draft,
            commands::create_sales_draft,
            commands::update_sales_draft,
            commands::delete_sales_draft,
            commands::reorder_sales_lines,
            commands::post_sales_document,
            commands::list_purchase_drafts,
            commands::get_purchase_draft,
            commands::create_purchase_draft,
            commands::update_purchase_draft,
            commands::delete_purchase_draft,
            commands::post_purchase_document,
            commands::day_book,
            commands::ledger_report,
            commands::trial_balance,
            commands::profit_and_loss,
            commands::balance_sheet,
            commands::cash_flow,
            commands::export_report_preview,
            commands::get_bills_receivable,
            commands::get_bills_payable,
            commands::get_party_pending_bills,
            commands::get_aging_analysis,
            commands::record_bill_allocations,
            commands::get_company_gst_profile,
            commands::save_company_gst_profile,
            commands::get_ledger_gst_profile,
            commands::save_ledger_gst_profile,
            commands::list_hsn_sac_codes,
            commands::create_hsn_sac_code,
            commands::get_item_gst_mapping,
            commands::save_item_gst_mapping,
            commands::get_gst_ledger_mapping,
            commands::save_gst_ledger_mapping,
            commands::preview_gst_calculation,
            commands::get_gstr1_summary,
            commands::get_gstr3b_summary,
            commands::global_goto_search,
            commands::get_voucher_full_detail,
            commands::create_ledger,
            commands::update_ledger,
            commands::delete_ledger,
            commands::set_opening_balance,
            commands::list_voucher_types,
            commands::get_voucher_type,
            commands::create_voucher_series,
            commands::list_voucher_series,
            commands::get_voucher_series,
            commands::update_voucher_series,
            commands::deactivate_voucher_series,
            commands::delete_voucher_series,
            commands::create_voucher_number_config,
            commands::list_voucher_number_configs,
            commands::resolve_effective_voucher_config,
            commands::inspect_voucher_sequence,
            commands::resync_voucher_sequence,
            commands::create_voucher_draft,
            commands::get_voucher_draft,
            commands::list_voucher_drafts,
            commands::update_voucher_draft,
            commands::delete_voucher_draft,
            commands::add_voucher_line,
            commands::update_voucher_line,
            commands::delete_voucher_line,
            commands::reorder_voucher_lines,
            commands::save_voucher_draft,
            commands::post_voucher,
            commands::cancel_voucher,
            commands::reverse_voucher,
            commands::create_correction_draft,
            commands::list_stock_groups,
            commands::create_stock_group,
            commands::update_stock_group,
            commands::set_stock_group_active,
            commands::delete_stock_group,
            commands::list_stock_categories,
            commands::create_stock_category,
            commands::update_stock_category,
            commands::set_stock_category_active,
            commands::delete_stock_category,
            commands::list_units,
            commands::create_unit,
            commands::update_unit,
            commands::set_unit_active,
            commands::delete_unit,
            commands::list_stock_items,
            commands::create_stock_item,
            commands::update_stock_item,
            commands::set_stock_item_active,
            commands::delete_stock_item,
            commands::list_godowns,
            commands::create_godown,
            commands::update_godown,
            commands::set_godown_active,
            commands::delete_godown,
            commands::create_stock_voucher,
            commands::get_stock_voucher,
            commands::update_stock_voucher,
            commands::list_stock_vouchers,
            commands::post_stock_voucher,
            commands::cancel_stock_voucher,
            commands::reverse_stock_voucher,
            commands::get_stock_balance,
            commands::get_stock_movement_history,
            commands::get_inventory_valuation_config,
            commands::set_inventory_valuation_method,
            commands::get_stock_valuation,
            commands::get_fifo_layers,
            commands::get_valuation_history,
            commands::get_stock_summary_report,
            commands::get_stock_movement_register,
            commands::get_stock_ledger_report,
            commands::get_inventory_reconciliation,
            commands::get_inventory_account_mapping,
            commands::configure_inventory_account_mapping,
            commands::preview_inventory_accounting,
            commands::integrate_inventory_accounting,
            commands::get_inventory_accounting_status,
            commands::list_inventory_accounting_history,
            commands::get_inventory_valuation_attribution,
            commands::get_inventory_valuation_attribution_details,
            commands::export_invoice_pdf,
            commands::create_database_backup,
            commands::restore_database_backup,
            commands::get_backup_status,
            commands::post_commercial_voucher,
            commands::export_company_tally_xml,
            commands::preview_tally_xml_import,
            commands::execute_tally_xml_import,
            commands::export_gstr1_json,
            commands::export_gstr3b_json,
            commands::get_company_features,
            commands::save_company_features,
            commands::create_sample_company,
            commands::list_bank_ledgers,
            commands::get_bank_reconciliation,
            commands::reconcile_bank_entries,
            commands::preview_bank_statement_auto_match,
            commands::list_tds_sections,
            commands::create_tds_section,
            commands::get_company_tds_profile,
            commands::save_company_tds_profile,
            commands::get_ledger_tds_profile,
            commands::save_ledger_tds_profile,
            commands::preview_tds_calculation,
            commands::record_tds_challan_payment,
            commands::get_tds_outstanding,
            commands::get_form_26q_summary,
            commands::export_form_26q_json,
            commands::export_form_26q_csv,
            commands::import_and_reconcile_gstr2b,
            commands::get_gstr2b_reconciliation_report,
            commands::export_gstr2b_reconciliation_csv,
            commands::export_einvoice_json,
            commands::export_eway_bill_json,
            commands::save_voucher_einvoice_metadata,
            commands::get_voucher_einvoice_metadata,
            commands::get_godown_summary_report,
            commands::list_stock_item_batches,
            commands::create_stock_item_batch,
            commands::get_batch_expiry_report,
            commands::post_stock_journal_voucher,
            commands::list_stock_journal_vouchers,
            commands::list_cost_categories,
            commands::create_cost_category,
            commands::list_cost_centres,
            commands::create_cost_centre,
            commands::get_voucher_cost_allocations,
            commands::get_cost_centre_breakup_report,
            commands::get_ratio_analysis,
            commands::list_budgets,
            commands::create_budget,
            commands::get_budget,
            commands::delete_budget,
            commands::get_budget_variance_report,
            commands::preview_year_end_closing,
            commands::execute_year_end_closing,
            commands::list_closed_years,
            commands::verify_audit_trail_integrity,
            commands::export_audit_certification_csv,
            commands::list_purchase_orders,
            commands::create_purchase_order,
            commands::get_purchase_order,
            commands::cancel_purchase_order,
            commands::convert_purchase_order_to_invoice,
            commands::list_sales_orders,
            commands::create_sales_order,
            commands::get_sales_order,
            commands::cancel_sales_order,
            commands::convert_sales_order_to_invoice,
            commands::get_pending_purchase_orders,
            commands::get_pending_sales_orders,
            commands::get_company_security_status,
            commands::enable_company_security,
            commands::authenticate_company_user,
            commands::logout_company_session,
            commands::list_company_users,
            commands::create_company_user,
            commands::update_company_user_role,
            commands::update_company_user_password,
            commands::deactivate_company_user,
            commands::list_boms,
            commands::get_bom_detail,
            commands::create_bom,
            commands::execute_manufacturing_journal,
            commands::list_employees,
            commands::create_employee,
            commands::deactivate_employee,
            commands::preview_payroll_run,
            commands::execute_payroll_run,
            commands::list_payroll_runs,
            commands::get_payroll_statutory_summary,
            commands::export_payslip_pdf,
            commands::export_payroll_csv,
            commands::seed_company_currency_defaults,
            commands::list_currencies,
            commands::create_currency,
            commands::set_exchange_rate,
            commands::list_exchange_rates,
            commands::lookup_exchange_rate,
            commands::convert_currency_amount,
            commands::list_open_foreign_bills,
            commands::settle_foreign_bill_with_forex,
            commands::get_unrealized_forex_report,
            commands::list_cheque_templates,
            commands::upsert_cheque_template,
            commands::generate_cheque_pdf_bytes,
            commands::export_cheque_pdf,
            commands::start_lan_server,
            commands::stop_lan_server,
            commands::get_lan_server_status,
            commands::revoke_lan_session,
            commands::get_company_gsp_config,
            commands::save_company_gsp_config,
            commands::push_einvoice_to_gsp,
            commands::push_eway_bill_to_gsp,
            commands::list_custom_reports,
            commands::save_custom_report,
            commands::delete_custom_report,
            commands::run_custom_report,
            commands::export_custom_report_csv,
            commands::export_custom_report_template,
            commands::import_custom_report_template
        ])
        .run(tauri::generate_context!())
        .expect("Error while running OpenTally desktop application");
}
