pub mod accounting;
pub mod app;
pub mod commands;
pub mod company;
pub mod config;
pub mod database;
pub mod errors;
pub mod interoperability;
pub mod inventory;
pub mod logging;
pub mod party;
pub mod reports;
pub mod sales;
pub mod vouchers;

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

    let config = AppConfig::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle();
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));

            std::fs::create_dir_all(&app_dir)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let db_path = app_dir.join("opentally.db");

            info!("Configuring database path at: {:?}", db_path);

            let db = tauri::async_runtime::block_on(async {
                let m = DatabaseManager::init(&db_path).await?;
                crate::database::migrations::run_sprint1_migration(m.pool()).await?;
                crate::database::migrations::run_stage3_migration(m.pool()).await?;
                crate::database::migrations::run_sprint2_migration(m.pool()).await?;
                crate::database::migrations::run_sprint2_stage4_hardening_migration(m.pool())
                    .await?;
                crate::database::migrations::run_sprint4_inventory_migration(m.pool()).await?;
                crate::database::migrations::run_sprint4_stage11_stock_migration(m.pool()).await?;
                crate::database::migrations::run_sprint4_stage12_valuation_migration(m.pool())
                    .await?;
                crate::database::migrations::run_sprint5_stage14_integration_migration(m.pool())
                    .await?;
                crate::database::migrations::run_sprint5_stage15_valuation_attribution_migration(
                    m.pool(),
                )
                .await?;
                crate::database::migrations::run_sprint6_stage17_party_migration(m.pool()).await?;
                crate::database::migrations::run_sprint6_stage18_sales_migration(m.pool()).await?;
                Ok::<_, crate::errors::OpenTallyError>(m)
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            let state = AppState::new(config, Arc::new(db));
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
            commands::day_book,
            commands::ledger_report,
            commands::trial_balance,
            commands::profit_and_loss,
            commands::balance_sheet,
            commands::cash_flow,
            commands::export_report_preview,
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
            commands::get_inventory_valuation_attribution_details
        ])
        .run(tauri::generate_context!())
        .expect("Error while running OpenTally desktop application");
}
