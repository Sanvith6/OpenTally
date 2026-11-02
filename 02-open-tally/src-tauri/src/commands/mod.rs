use crate::app::AppState;
use crate::database::DatabaseHealth;
use crate::errors::OpenTallyError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Subsystem lifecycle status report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubsystemStatus {
    pub name: String,
    pub status: String,
    pub details: String,
}

/// Dedicated security and encryption readiness descriptor.
/// Note: Strictly separated from operational application health.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityStatus {
    pub encryption_active: bool,
    pub encryption_engine: String,
    pub key_management_status: String,
    pub csp_enforced: bool,
}

/// Comprehensive health check payload returned across Tauri IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub app_name: String,
    pub app_version: String,
    pub status: String, // Operational application health
    pub timestamp: String,
    pub environment: String,
    pub uptime_seconds: i64,
    pub database: DatabaseHealth,
    pub security: SecurityStatus,
    pub subsystems: Vec<SubsystemStatus>,
}

/// Internal testable implementation of health generation.
pub async fn generate_health_report(
    config: &crate::config::AppConfig,
    db: &Arc<crate::database::DatabaseManager>,
    started_at: chrono::DateTime<Utc>,
) -> Result<HealthResponse, OpenTallyError> {
    let now = Utc::now();
    let uptime_seconds = (now - started_at).num_seconds();

    let db_health = db.check_health().await?;

    // Security readiness is tracked independently of operational health
    let security = SecurityStatus {
        encryption_active: false,
        encryption_engine:
            "none (standard unencrypted SQLite; SQLCipher pending key management ADR)".to_string(),
        key_management_status: "unconfigured (OS Keyring / DPAPI integration required)".to_string(),
        csp_enforced: true,
    };

    let subsystems = vec![
        SubsystemStatus {
            name: "database".to_string(),
            status: if db_health.is_connected {
                "operational".to_string()
            } else {
                "degraded".to_string()
            },
            details: format!(
                "SQLite engine with WAL mode; schema: {}; foreign keys: active",
                db_health.schema_version
            ),
        },
        SubsystemStatus {
            name: "accounting_engine".to_string(),
            status: "specifications_documented".to_string(),
            details: "Invariants documented; implementation & debit-credit posting validation pending Sprint 1".to_string(),
        },
        SubsystemStatus {
            name: "company_management".to_string(),
            status: "pending_implementation".to_string(),
            details: "Scheduled for Phase 2, Sprint 1".to_string(),
        },
        SubsystemStatus {
            name: "voucher_pipeline".to_string(),
            status: "pending_implementation".to_string(),
            details: "Scheduled for Phase 2, Sprint 2 (24 canonical voucher types documented)".to_string(),
        },
        SubsystemStatus {
            name: "interoperability_port_9000".to_string(),
            status: "inactive".to_string(),
            details: "Pending ADR-002 HTTP framework decision".to_string(),
        },
    ];

    Ok(HealthResponse {
        app_name: config.app_name.clone(),
        app_version: config.version.clone(),
        status: "healthy".to_string(),
        timestamp: now.to_rfc3339(),
        environment: config.environment.clone(),
        uptime_seconds,
        database: db_health,
        security,
        subsystems,
    })
}

/// Tauri command handler invoked from React frontend.
#[tauri::command]
pub async fn get_app_health(
    state: tauri::State<'_, AppState>,
) -> Result<HealthResponse, OpenTallyError> {
    info!(">>> [Tauri IPC] Received 'get_app_health' command from frontend WebView");
    let report = generate_health_report(&state.config, &state.db, state.started_at).await?;
    info!(
        "<<< [Tauri IPC] Successfully generated health report: status={}, db_wal={}, schema={}, encryption_active={}",
        report.status,
        report.database.journal_mode,
        report.database.schema_version,
        report.security.encryption_active
    );
    Ok(report)
}

// ---------------------------------------------------------------------------
// Company Management Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn create_company(
    state: tauri::State<'_, AppState>,
    req: crate::company::CreateCompanyRequest,
) -> Result<crate::company::Company, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'create_company' command for name='{}'",
        req.name
    );
    crate::company::CompanyService::create_company(state.db.pool(), req).await
}

#[tauri::command]
pub async fn list_companies(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::company::Company>, OpenTallyError> {
    info!(">>> [Tauri IPC] Received 'list_companies' command");
    crate::company::CompanyService::list_companies(state.db.pool()).await
}

#[tauri::command]
pub async fn get_company(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::company::Company, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'get_company' command for id='{}'",
        id
    );
    crate::company::CompanyService::get_company(state.db.pool(), &id).await
}

#[tauri::command]
pub async fn select_company(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::company::Company, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'select_company' command for id='{}'",
        id
    );
    let company =
        crate::company::CompanyService::select_active_company(state.db.pool(), &id).await?;

    // Persist active company selection to SQLite storage for restart restoration
    crate::company::CompanyService::set_persisted_active_company_id(
        state.db.pool(),
        Some(&company.id),
    )
    .await?;

    {
        let mut active = state.active_company_id.write().map_err(|e| {
            OpenTallyError::Internal(format!(
                "Failed to acquire active company write lock: {}",
                e
            ))
        })?;
        *active = Some(company.id.clone());
    }
    Ok(company)
}

#[tauri::command]
pub async fn get_active_company(
    state: tauri::State<'_, AppState>,
) -> Result<Option<crate::company::Company>, OpenTallyError> {
    info!(">>> [Tauri IPC] Received 'get_active_company' command");
    let active_id = {
        let active = state.active_company_id.read().map_err(|e| {
            OpenTallyError::Internal(format!("Failed to acquire active company read lock: {}", e))
        })?;
        active.clone()
    };

    let id_to_check = match active_id {
        Some(id) => Some(id),
        None => {
            // Restore from persisted database state on application startup/reopen
            let persisted =
                crate::company::CompanyService::get_persisted_active_company_id(state.db.pool())
                    .await?;
            if let Some(ref pid) = persisted {
                let mut active = state.active_company_id.write().map_err(|e| {
                    OpenTallyError::Internal(format!(
                        "Failed to acquire active company write lock: {}",
                        e
                    ))
                })?;
                *active = Some(pid.clone());
            }
            persisted
        }
    };

    match id_to_check {
        Some(id) => match crate::company::CompanyService::get_company(state.db.pool(), &id).await {
            Ok(company) => {
                if company.status == crate::company::CompanyStatus::Active {
                    Ok(Some(company))
                } else {
                    // Archived/closed company cannot remain active
                    crate::company::CompanyService::set_persisted_active_company_id(
                        state.db.pool(),
                        None,
                    )
                    .await?;
                    {
                        let mut active = state.active_company_id.write().map_err(|e| {
                            OpenTallyError::Internal(format!(
                                "Failed to acquire active company write lock: {}",
                                e
                            ))
                        })?;
                        *active = None;
                    }
                    Ok(None)
                }
            }
            Err(_) => {
                crate::company::CompanyService::set_persisted_active_company_id(
                    state.db.pool(),
                    None,
                )
                .await?;
                {
                    let mut active = state.active_company_id.write().map_err(|e| {
                        OpenTallyError::Internal(format!(
                            "Failed to acquire active company write lock: {}",
                            e
                        ))
                    })?;
                    *active = None;
                }
                Ok(None)
            }
        },
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn update_company(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::company::UpdateCompanyRequest,
) -> Result<crate::company::Company, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'update_company' command for id='{}'",
        id
    );
    crate::company::CompanyService::update_company(state.db.pool(), &id, req).await
}

#[tauri::command]
pub async fn archive_company(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::company::Company, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'archive_company' command for id='{}'",
        id
    );
    let company = crate::company::CompanyService::archive_company(state.db.pool(), &id).await?;
    let was_active = {
        let mut active = state.active_company_id.write().map_err(|e| {
            OpenTallyError::Internal(format!(
                "Failed to acquire active company write lock: {}",
                e
            ))
        })?;
        if active.as_deref() == Some(&id) {
            *active = None;
            true
        } else {
            false
        }
    };

    if was_active {
        crate::company::CompanyService::set_persisted_active_company_id(state.db.pool(), None)
            .await?;
    }
    Ok(company)
}

// ---------------------------------------------------------------------------
// Chart of Accounts & Tree IPC Commands (Stage 3)
// ---------------------------------------------------------------------------

pub async fn require_active_company(state: &AppState) -> Result<String, OpenTallyError> {
    let company_id = {
        let active = state.active_company_id.read().map_err(|e| {
            OpenTallyError::Internal(format!("Failed to acquire active company read lock: {}", e))
        })?;
        active.clone().ok_or_else(|| {
            OpenTallyError::Validation(
                "No active company selected. Please select a company (F3) first.".to_string(),
            )
        })?
    };

    let company = crate::company::CompanyService::get_company(state.db.pool(), &company_id).await?;
    if company.status != crate::company::CompanyStatus::Active {
        return Err(OpenTallyError::Validation(format!(
            "Cannot modify chart of accounts for company '{}' with status '{}'",
            company.name, company.status
        )));
    }

    Ok(company_id)
}

pub async fn exec_get_chart_of_accounts(
    state: &AppState,
) -> Result<crate::accounting::ChartOfAccountsTree, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::TreeService::get_chart_of_accounts_tree(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn get_chart_of_accounts(
    state: tauri::State<'_, AppState>,
) -> Result<crate::accounting::ChartOfAccountsTree, OpenTallyError> {
    exec_get_chart_of_accounts(&state).await
}

pub async fn exec_list_account_groups(
    state: &AppState,
) -> Result<Vec<crate::accounting::AccountGroup>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::GroupService::list_groups(state.db.pool(), &company_id, false).await
}

#[tauri::command]
pub async fn list_account_groups(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::accounting::AccountGroup>, OpenTallyError> {
    exec_list_account_groups(&state).await
}

pub async fn exec_create_account_group(
    state: &AppState,
    req: crate::accounting::CreateAccountGroupRequest,
) -> Result<crate::accounting::AccountGroup, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::GroupService::create_group(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn create_account_group(
    state: tauri::State<'_, AppState>,
    req: crate::accounting::CreateAccountGroupRequest,
) -> Result<crate::accounting::AccountGroup, OpenTallyError> {
    exec_create_account_group(&state, req).await
}

pub async fn exec_update_account_group(
    state: &AppState,
    id: String,
    req: crate::accounting::UpdateAccountGroupRequest,
) -> Result<crate::accounting::AccountGroup, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::GroupService::update_group(state.db.pool(), &company_id, &id, req).await
}

#[tauri::command]
pub async fn update_account_group(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::accounting::UpdateAccountGroupRequest,
) -> Result<crate::accounting::AccountGroup, OpenTallyError> {
    exec_update_account_group(&state, id, req).await
}

pub async fn exec_delete_account_group(state: &AppState, id: String) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::GroupService::delete_group(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn delete_account_group(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_account_group(&state, id).await
}

pub async fn exec_list_ledgers(
    state: &AppState,
    group_id: Option<String>,
    search_query: Option<String>,
) -> Result<Vec<crate::accounting::Ledger>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::LedgerService::list_ledgers(
        state.db.pool(),
        &company_id,
        group_id.as_deref(),
        search_query.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_ledgers(
    state: tauri::State<'_, AppState>,
    group_id: Option<String>,
    search_query: Option<String>,
) -> Result<Vec<crate::accounting::Ledger>, OpenTallyError> {
    exec_list_ledgers(&state, group_id, search_query).await
}

pub async fn exec_get_ledger(
    state: &AppState,
    id: String,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::LedgerService::get_ledger(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn get_ledger(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    exec_get_ledger(&state, id).await
}

pub async fn exec_create_ledger(
    state: &AppState,
    req: crate::accounting::CreateLedgerRequest,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let initial_opb = req.opening_balance.clone();
    let ledger =
        crate::accounting::LedgerService::create_ledger(state.db.pool(), &company_id, req).await?;
    if let Some(mut opb_req) = initial_opb {
        opb_req.ledger_id = ledger.id.clone();
        crate::accounting::OpeningBalanceService::set_opening_balance(
            state.db.pool(),
            &company_id,
            opb_req,
        )
        .await?;
    }
    Ok(ledger)
}

#[tauri::command]
pub async fn create_ledger(
    state: tauri::State<'_, AppState>,
    req: crate::accounting::CreateLedgerRequest,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    exec_create_ledger(&state, req).await
}

pub async fn exec_update_ledger(
    state: &AppState,
    id: String,
    req: crate::accounting::UpdateLedgerRequest,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::LedgerService::update_ledger(state.db.pool(), &company_id, &id, req).await
}

#[tauri::command]
pub async fn update_ledger(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::accounting::UpdateLedgerRequest,
) -> Result<crate::accounting::Ledger, OpenTallyError> {
    exec_update_ledger(&state, id, req).await
}

pub async fn exec_delete_ledger(state: &AppState, id: String) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::LedgerService::delete_ledger(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn delete_ledger(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_ledger(&state, id).await
}

pub async fn exec_set_opening_balance(
    state: &AppState,
    req: crate::accounting::SetOpeningBalanceRequest,
) -> Result<Option<crate::accounting::OpeningBalance>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::accounting::OpeningBalanceService::set_opening_balance(state.db.pool(), &company_id, req)
        .await
}

#[tauri::command]
pub async fn set_opening_balance(
    state: tauri::State<'_, AppState>,
    req: crate::accounting::SetOpeningBalanceRequest,
) -> Result<Option<crate::accounting::OpeningBalance>, OpenTallyError> {
    exec_set_opening_balance(&state, req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::database::DatabaseManager;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_generate_health_report() {
        let config = AppConfig::default();
        let db = Arc::new(
            DatabaseManager::init(&PathBuf::from(":memory:"))
                .await
                .expect("Failed to initialize test DB"),
        );
        let started_at = Utc::now();

        let report = generate_health_report(&config, &db, started_at)
            .await
            .expect("Failed to generate health report");

        assert_eq!(report.app_name, "OpenTally");
        assert_eq!(report.status, "healthy");
        assert!(report.database.is_connected);
        assert_eq!(report.database.foreign_keys_enabled, true);
        assert_eq!(report.security.encryption_active, false);
        assert_eq!(report.subsystems[1].status, "specifications_documented");
        assert_eq!(report.subsystems.len(), 5);
    }

    #[tokio::test]
    async fn test_all_11_registered_accounting_ipc_commands() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("cmd_test.db");
        let db = DatabaseManager::init(&db_path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();

        let config = AppConfig::default();
        let state = AppState::new(config, Arc::new(db));

        // Phase 1: When no active company is selected, commands reject execution
        assert!(exec_get_chart_of_accounts(&state).await.is_err());
        assert!(exec_list_account_groups(&state).await.is_err());
        assert!(exec_list_ledgers(&state, None, None).await.is_err());

        // Create and select a company
        let comp = crate::company::service::CompanyService::create_company(
            state.db.pool(),
            crate::company::model::CreateCompanyRequest {
                name: "IPC Test Co".to_string(),
                mailing_name: None,
                country: None,
                state: None,
                financial_year_begins: "2026-04-01".to_string(),
                books_beginning_from: None,
                currency_symbol: None,
                currency_formal_name: None,
                currency_decimal_places: Some(2),
            },
        )
        .await
        .unwrap();

        *state.active_company_id.write().unwrap() = Some(comp.id.clone());

        // 1. Test exec_get_chart_of_accounts
        let coa = exec_get_chart_of_accounts(&state).await.unwrap();
        assert!(!coa.nodes.is_empty());
        assert_eq!(coa.equilibrium.total_debit, "0.00");

        // 2. Test exec_list_account_groups
        let groups = exec_list_account_groups(&state).await.unwrap();
        assert_eq!(groups.len(), 28); // 28 visible canonical predefined groups
        let debtors = groups.iter().find(|g| g.name == "Sundry Debtors").unwrap();

        // 3. Test exec_create_account_group
        let created_group = exec_create_account_group(
            &state,
            crate::accounting::CreateAccountGroupRequest {
                name: "East Region Debtors".to_string(),
                parent_id: debtors.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(created_group.name, "East Region Debtors");

        // 4. Test exec_update_account_group
        let updated_group = exec_update_account_group(
            &state,
            created_group.id.clone(),
            crate::accounting::UpdateAccountGroupRequest {
                name: "North-East Region Debtors".to_string(),
                parent_id: Some(debtors.id.clone()),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated_group.name, "North-East Region Debtors");

        // 5. Test exec_create_ledger
        let created_ledger = exec_create_ledger(
            &state,
            crate::accounting::CreateLedgerRequest {
                name: "Acme Enterprises".to_string(),
                group_id: updated_group.id.clone(),
                description: Some("Key customer".to_string()),
                opening_balance: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(created_ledger.name, "Acme Enterprises");

        // 6. Test exec_get_ledger
        let fetched_ledger = exec_get_ledger(&state, created_ledger.id.clone())
            .await
            .unwrap();
        assert_eq!(fetched_ledger.name, "Acme Enterprises");

        // 7. Test exec_list_ledgers
        let ledgers = exec_list_ledgers(&state, None, Some("Acme".to_string()))
            .await
            .unwrap();
        assert_eq!(ledgers.len(), 1);

        // 8. Test exec_update_ledger
        let updated_ledger = exec_update_ledger(
            &state,
            created_ledger.id.clone(),
            crate::accounting::UpdateLedgerRequest {
                name: "Acme Corp Ltd".to_string(),
                group_id: Some(updated_group.id.clone()),
                description: Some("Updated customer".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(updated_ledger.name, "Acme Corp Ltd");

        // 9. Test exec_set_opening_balance (with normalization)
        let opb = exec_set_opening_balance(
            &state,
            crate::accounting::SetOpeningBalanceRequest {
                ledger_id: created_ledger.id.clone(),
                financial_year_begins: None,
                balance_type: crate::accounting::BalanceType::Dr,
                amount: "100.2300".to_string(), // normalizes to 100.23
            },
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(opb.amount, "100.23");

        // Clear opening balance
        let cleared_opb = exec_set_opening_balance(
            &state,
            crate::accounting::SetOpeningBalanceRequest {
                ledger_id: created_ledger.id.clone(),
                financial_year_begins: None,
                balance_type: crate::accounting::BalanceType::Dr,
                amount: "0.00".to_string(),
            },
        )
        .await
        .unwrap();
        assert!(cleared_opb.is_none());

        // 10. Test exec_delete_ledger (succeeds now that opening balance is cleared)
        exec_delete_ledger(&state, created_ledger.id.clone())
            .await
            .unwrap();
        assert!(exec_get_ledger(&state, created_ledger.id.clone())
            .await
            .is_err());

        // 11. Test exec_delete_account_group (succeeds now that child ledger is deleted)
        exec_delete_account_group(&state, updated_group.id.clone())
            .await
            .unwrap();
    }
}
