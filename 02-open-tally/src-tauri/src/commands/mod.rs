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

// ---------------------------------------------------------------------------
// Voucher Type & Numbering Series IPC Commands (Sprint 2 Stage 2)
// ---------------------------------------------------------------------------

pub async fn exec_list_voucher_types(
    state: &AppState,
) -> Result<Vec<crate::vouchers::VoucherType>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherTypeService::list_voucher_types(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn list_voucher_types(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::vouchers::VoucherType>, OpenTallyError> {
    exec_list_voucher_types(&state).await
}

pub async fn exec_get_voucher_type(
    state: &AppState,
    id: String,
) -> Result<crate::vouchers::VoucherType, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherTypeService::get_voucher_type(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn get_voucher_type(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::vouchers::VoucherType, OpenTallyError> {
    exec_get_voucher_type(&state, id).await
}

pub async fn exec_create_voucher_series(
    state: &AppState,
    req: crate::vouchers::CreateVoucherSeriesRequest,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::create_series(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn create_voucher_series(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::CreateVoucherSeriesRequest,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    exec_create_voucher_series(&state, req).await
}

pub async fn exec_list_voucher_series(
    state: &AppState,
    voucher_type_id: String,
) -> Result<Vec<crate::vouchers::VoucherNumberSeries>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::list_series_by_voucher_type(
        state.db.pool(),
        &company_id,
        &voucher_type_id,
    )
    .await
}

#[tauri::command]
pub async fn list_voucher_series(
    state: tauri::State<'_, AppState>,
    voucher_type_id: String,
) -> Result<Vec<crate::vouchers::VoucherNumberSeries>, OpenTallyError> {
    exec_list_voucher_series(&state, voucher_type_id).await
}

pub async fn exec_get_voucher_series(
    state: &AppState,
    id: String,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::get_series(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn get_voucher_series(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    exec_get_voucher_series(&state, id).await
}

pub async fn exec_update_voucher_series(
    state: &AppState,
    id: String,
    req: crate::vouchers::UpdateVoucherSeriesRequest,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::update_series(state.db.pool(), &company_id, &id, req)
        .await
}

#[tauri::command]
pub async fn update_voucher_series(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::vouchers::UpdateVoucherSeriesRequest,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    exec_update_voucher_series(&state, id, req).await
}

pub async fn exec_deactivate_voucher_series(
    state: &AppState,
    id: String,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::deactivate_series(state.db.pool(), &company_id, &id)
        .await
}

#[tauri::command]
pub async fn deactivate_voucher_series(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::vouchers::VoucherNumberSeries, OpenTallyError> {
    exec_deactivate_voucher_series(&state, id).await
}

pub async fn exec_delete_voucher_series(
    state: &AppState,
    id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::delete_series(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn delete_voucher_series(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_voucher_series(&state, id).await
}

pub async fn exec_create_voucher_number_config(
    state: &AppState,
    series_id: String,
    req: crate::vouchers::CreateVoucherNumberConfigRequest,
) -> Result<crate::vouchers::VoucherNumberConfig, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::create_configuration(
        state.db.pool(),
        &company_id,
        &series_id,
        req,
    )
    .await
}

#[tauri::command]
pub async fn create_voucher_number_config(
    state: tauri::State<'_, AppState>,
    series_id: String,
    req: crate::vouchers::CreateVoucherNumberConfigRequest,
) -> Result<crate::vouchers::VoucherNumberConfig, OpenTallyError> {
    exec_create_voucher_number_config(&state, series_id, req).await
}

pub async fn exec_list_voucher_number_configs(
    state: &AppState,
    series_id: String,
) -> Result<Vec<crate::vouchers::VoucherNumberConfig>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherSeriesService::list_configurations(
        state.db.pool(),
        &company_id,
        &series_id,
    )
    .await
}

#[tauri::command]
pub async fn list_voucher_number_configs(
    state: tauri::State<'_, AppState>,
    series_id: String,
) -> Result<Vec<crate::vouchers::VoucherNumberConfig>, OpenTallyError> {
    exec_list_voucher_number_configs(&state, series_id).await
}

// ---------------------------------------------------------------------------
// Stage 3: Numbering Configuration Resolution & Safe Sequence Diagnostics IPC Commands
// ---------------------------------------------------------------------------

pub async fn exec_resolve_effective_voucher_config(
    state: &AppState,
    series_id: String,
    voucher_date: String,
) -> Result<crate::vouchers::EffectiveNumberConfig, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let mut conn = state
        .db
        .pool()
        .acquire()
        .await
        .map_err(OpenTallyError::Database)?;
    crate::vouchers::VoucherSequenceService::resolve_effective_config(
        &mut conn,
        &company_id,
        &series_id,
        &voucher_date,
    )
    .await
}

#[tauri::command]
pub async fn resolve_effective_voucher_config(
    state: tauri::State<'_, AppState>,
    series_id: String,
    voucher_date: String,
) -> Result<crate::vouchers::EffectiveNumberConfig, OpenTallyError> {
    exec_resolve_effective_voucher_config(&state, series_id, voucher_date).await
}

pub async fn exec_inspect_voucher_sequence(
    state: &AppState,
    req: crate::vouchers::InspectSequenceRequest,
) -> Result<crate::vouchers::SequenceRecoveryReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let mut conn = state
        .db
        .pool()
        .acquire()
        .await
        .map_err(OpenTallyError::Database)?;
    crate::vouchers::VoucherSequenceService::inspect_sequence(
        &mut conn,
        &company_id,
        &req.series_id,
        &req.period_start,
    )
    .await
}

#[tauri::command]
pub async fn inspect_voucher_sequence(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::InspectSequenceRequest,
) -> Result<crate::vouchers::SequenceRecoveryReport, OpenTallyError> {
    exec_inspect_voucher_sequence(&state, req).await
}

pub async fn exec_resync_voucher_sequence(
    state: &AppState,
    req: crate::vouchers::ResyncSequenceRequest,
) -> Result<crate::vouchers::SequenceRecoveryReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let mut tx = state
        .db
        .pool()
        .begin()
        .await
        .map_err(OpenTallyError::Database)?;
    let report = crate::vouchers::VoucherSequenceService::resync_voucher_sequence(
        &mut tx,
        &company_id,
        &req.series_id,
        &req.period_start,
        "admin",
        &req.resolution_mode,
        req.resolution_reason.as_deref(),
    )
    .await?;
    tx.commit().await.map_err(OpenTallyError::Database)?;
    Ok(report)
}

#[tauri::command]
pub async fn resync_voucher_sequence(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::ResyncSequenceRequest,
) -> Result<crate::vouchers::SequenceRecoveryReport, OpenTallyError> {
    exec_resync_voucher_sequence(&state, req).await
}

// ---------------------------------------------------------------------------
// Sprint 2 Stage 4: Voucher Creation & Draft Management IPC Commands
// ---------------------------------------------------------------------------

pub async fn exec_create_voucher_draft(
    state: &AppState,
    req: crate::vouchers::CreateVoucherDraftRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::create_draft(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn create_voucher_draft(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::CreateVoucherDraftRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    exec_create_voucher_draft(&state, req).await
}

pub async fn exec_get_voucher_draft(
    state: &AppState,
    voucher_id: String,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::get_draft(state.db.pool(), &company_id, &voucher_id).await
}

#[tauri::command]
pub async fn get_voucher_draft(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    exec_get_voucher_draft(&state, voucher_id).await
}

pub async fn exec_list_voucher_drafts(
    state: &AppState,
    filter: crate::vouchers::VoucherFilter,
) -> Result<crate::vouchers::PaginatedResponse<crate::vouchers::VoucherSummary>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::list_drafts(state.db.pool(), &company_id, filter).await
}

#[tauri::command]
pub async fn list_voucher_drafts(
    state: tauri::State<'_, AppState>,
    filter: crate::vouchers::VoucherFilter,
) -> Result<crate::vouchers::PaginatedResponse<crate::vouchers::VoucherSummary>, OpenTallyError> {
    exec_list_voucher_drafts(&state, filter).await
}

pub async fn exec_update_voucher_draft(
    state: &AppState,
    voucher_id: String,
    req: crate::vouchers::UpdateVoucherDraftRequest,
) -> Result<crate::vouchers::Voucher, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::update_draft_header(
        state.db.pool(),
        &company_id,
        &voucher_id,
        req,
    )
    .await
}

#[tauri::command]
pub async fn update_voucher_draft(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    req: crate::vouchers::UpdateVoucherDraftRequest,
) -> Result<crate::vouchers::Voucher, OpenTallyError> {
    exec_update_voucher_draft(&state, voucher_id, req).await
}

pub async fn exec_delete_voucher_draft(
    state: &AppState,
    voucher_id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::delete_draft(state.db.pool(), &company_id, &voucher_id)
        .await
}

#[tauri::command]
pub async fn delete_voucher_draft(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_voucher_draft(&state, voucher_id).await
}

pub async fn exec_add_voucher_line(
    state: &AppState,
    voucher_id: String,
    req: crate::vouchers::AddVoucherLineRequest,
) -> Result<crate::vouchers::VoucherLine, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::add_line(state.db.pool(), &company_id, &voucher_id, req)
        .await
}

#[tauri::command]
pub async fn add_voucher_line(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    req: crate::vouchers::AddVoucherLineRequest,
) -> Result<crate::vouchers::VoucherLine, OpenTallyError> {
    exec_add_voucher_line(&state, voucher_id, req).await
}

pub async fn exec_update_voucher_line(
    state: &AppState,
    voucher_id: String,
    line_id: String,
    req: crate::vouchers::UpdateVoucherLineRequest,
) -> Result<crate::vouchers::VoucherLine, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::update_line(
        state.db.pool(),
        &company_id,
        &voucher_id,
        &line_id,
        req,
    )
    .await
}

#[tauri::command]
pub async fn update_voucher_line(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    line_id: String,
    req: crate::vouchers::UpdateVoucherLineRequest,
) -> Result<crate::vouchers::VoucherLine, OpenTallyError> {
    exec_update_voucher_line(&state, voucher_id, line_id, req).await
}

pub async fn exec_delete_voucher_line(
    state: &AppState,
    voucher_id: String,
    line_id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::delete_line(
        state.db.pool(),
        &company_id,
        &voucher_id,
        &line_id,
    )
    .await
}

#[tauri::command]
pub async fn delete_voucher_line(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    line_id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_voucher_line(&state, voucher_id, line_id).await
}

pub async fn exec_reorder_voucher_lines(
    state: &AppState,
    voucher_id: String,
    req: crate::vouchers::ReorderVoucherLinesRequest,
) -> Result<Vec<crate::vouchers::VoucherLine>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::reorder_lines(
        state.db.pool(),
        &company_id,
        &voucher_id,
        req,
    )
    .await
}

#[tauri::command]
pub async fn reorder_voucher_lines(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    req: crate::vouchers::ReorderVoucherLinesRequest,
) -> Result<Vec<crate::vouchers::VoucherLine>, OpenTallyError> {
    exec_reorder_voucher_lines(&state, voucher_id, req).await
}

pub async fn exec_save_voucher_draft(
    state: &AppState,
    req: crate::vouchers::SaveVoucherDraftRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherDraftService::save_draft(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn save_voucher_draft(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::SaveVoucherDraftRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    exec_save_voucher_draft(&state, req).await
}

// ---------------------------------------------------------------------------
// Sprint 2 Stage 5B: Voucher Posting Engine IPC Command
// ---------------------------------------------------------------------------

pub async fn exec_post_voucher(
    state: &AppState,
    req: crate::vouchers::PostVoucherRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherPostingService::post_voucher(
        state.db.pool(),
        &company_id,
        &req.voucher_id,
        req.actor_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn post_voucher(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::PostVoucherRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    exec_post_voucher(&state, req).await
}

// ---------------------------------------------------------------------------
// Voucher Cancellation, Reversal & Correction IPC Commands (Sprint 2 Stage 6)
// ---------------------------------------------------------------------------

pub async fn exec_cancel_voucher(
    state: &AppState,
    req: crate::vouchers::CancelVoucherRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherCancellationService::cancel_voucher(state.db.pool(), &company_id, req)
        .await
}

#[tauri::command]
pub async fn cancel_voucher(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::CancelVoucherRequest,
) -> Result<crate::vouchers::VoucherWithLines, OpenTallyError> {
    exec_cancel_voucher(&state, req).await
}

pub async fn exec_reverse_voucher(
    state: &AppState,
    req: crate::vouchers::ReverseVoucherRequest,
) -> Result<crate::vouchers::ReversalResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherReversalService::reverse_voucher(state.db.pool(), &company_id, req)
        .await
}

#[tauri::command]
pub async fn reverse_voucher(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::ReverseVoucherRequest,
) -> Result<crate::vouchers::ReversalResult, OpenTallyError> {
    exec_reverse_voucher(&state, req).await
}

pub async fn exec_create_correction_draft(
    state: &AppState,
    req: crate::vouchers::CreateCorrectionDraftRequest,
) -> Result<crate::vouchers::CorrectionResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherCorrectionService::create_correction_draft(
        state.db.pool(),
        &company_id,
        req,
    )
    .await
}

#[tauri::command]
pub async fn create_correction_draft(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::CreateCorrectionDraftRequest,
) -> Result<crate::vouchers::CorrectionResult, OpenTallyError> {
    exec_create_correction_draft(&state, req).await
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
        assert!(report.database.foreign_keys_enabled);
        assert!(!report.security.encryption_active);
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

    #[tokio::test]
    async fn test_all_10_registered_voucher_masters_ipc_commands() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("cmd_voucher_test.db");
        let db = DatabaseManager::init(&db_path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_migration(db.pool())
            .await
            .unwrap();

        let config = AppConfig::default();
        let state = AppState::new(config, Arc::new(db));

        // Rejection when no active company
        assert!(exec_list_voucher_types(&state).await.is_err());
        assert!(exec_get_voucher_type(&state, "vt_sales".to_string())
            .await
            .is_err());
        assert!(exec_list_voucher_series(&state, "vt_sales".to_string())
            .await
            .is_err());

        // Create and select active company
        let comp = crate::company::service::CompanyService::create_company(
            state.db.pool(),
            crate::company::model::CreateCompanyRequest {
                name: "Voucher Master IPC Test Co".to_string(),
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

        // 1. exec_list_voucher_types
        let types = exec_list_voucher_types(&state).await.unwrap();
        assert_eq!(types.len(), 24);

        // 2. exec_get_voucher_type
        let vt = exec_get_voucher_type(&state, "vt_sales".to_string())
            .await
            .unwrap();
        assert_eq!(vt.name, "Sales");

        // 3. exec_create_voucher_series
        let series = exec_create_voucher_series(
            &state,
            crate::vouchers::CreateVoucherSeriesRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_name: "Export Series".to_string(),
                series_code: "EXP".to_string(),
                numbering_method: "automatic".to_string(),
                restart_period: "yearly".to_string(),
                applicable_from: Some("2026-04-01".to_string()),
                prefix: Some("EXP/{YY-YY}/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();
        assert_eq!(series.series_code, "EXP");

        // 4. exec_list_voucher_series
        let series_list = exec_list_voucher_series(&state, "vt_sales".to_string())
            .await
            .unwrap();
        assert_eq!(series_list.len(), 1);
        assert_eq!(series_list[0].id, series.id);

        // 5. exec_get_voucher_series
        let fetched_series = exec_get_voucher_series(&state, series.id.clone())
            .await
            .unwrap();
        assert_eq!(fetched_series.series_name, "Export Series");

        // 6. exec_update_voucher_series
        let updated_series = exec_update_voucher_series(
            &state,
            series.id.clone(),
            crate::vouchers::UpdateVoucherSeriesRequest {
                series_name: Some("Overseas Export Series".to_string()),
                is_active: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(updated_series.series_name, "Overseas Export Series");

        // 7. exec_create_voucher_number_config
        let config2 = exec_create_voucher_number_config(
            &state,
            series.id.clone(),
            crate::vouchers::CreateVoucherNumberConfigRequest {
                applicable_from: "2027-04-01".to_string(),
                prefix: Some("EXP/{YY-YY}-V2/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();
        assert_eq!(config2.applicable_from, "2027-04-01");

        // 8. exec_list_voucher_number_configs
        let configs = exec_list_voucher_number_configs(&state, series.id.clone())
            .await
            .unwrap();
        assert_eq!(configs.len(), 2);

        // 9. exec_deactivate_voucher_series
        let deactivated = exec_deactivate_voucher_series(&state, series.id.clone())
            .await
            .unwrap();
        assert!(!deactivated.is_active);

        // 10. exec_delete_voucher_series
        exec_delete_voucher_series(&state, series.id.clone())
            .await
            .unwrap();
        assert!(exec_get_voucher_series(&state, series.id.clone())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_stage3_registered_voucher_ipc_commands() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("cmd_stage3_test.db");
        let db = DatabaseManager::init(&db_path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_migration(db.pool())
            .await
            .unwrap();

        let config = AppConfig::default();
        let state = AppState::new(config, Arc::new(db));

        let comp = crate::company::service::CompanyService::create_company(
            state.db.pool(),
            crate::company::model::CreateCompanyRequest {
                name: "Stage 3 IPC Test Co".to_string(),
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

        let series = exec_create_voucher_series(
            &state,
            crate::vouchers::CreateVoucherSeriesRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_name: "Domestic Sales".to_string(),
                series_code: "DOM".to_string(),
                numbering_method: "automatic".to_string(),
                restart_period: "yearly".to_string(),
                applicable_from: Some("2026-04-01".to_string()),
                prefix: Some("DOM/{YY-YY}/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();

        // 1. exec_resolve_effective_voucher_config
        let eff = exec_resolve_effective_voucher_config(
            &state,
            series.id.clone(),
            "2026-08-15".to_string(),
        )
        .await
        .unwrap();
        assert_eq!(eff.prefix, "DOM/{YY-YY}/");
        assert_eq!(eff.period_start, "2026-04-01");

        // 2. exec_inspect_voucher_sequence
        let insp = exec_inspect_voucher_sequence(
            &state,
            crate::vouchers::InspectSequenceRequest {
                series_id: series.id.clone(),
                period_start: "2026-04-01".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(insp.start_number_baseline, 1);
        assert_eq!(insp.previous_next_number, 1);

        // 3. exec_resync_voucher_sequence
        let resync = exec_resync_voucher_sequence(
            &state,
            crate::vouchers::ResyncSequenceRequest {
                series_id: series.id.clone(),
                period_start: "2026-04-01".to_string(),
                resolution_mode: "strict".to_string(),
                resolution_reason: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(resync.new_next_number, 1);
    }

    #[tokio::test]
    async fn test_stage4_registered_voucher_draft_ipc_commands() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("cmd_stage4_test.db");
        let db = DatabaseManager::init(&db_path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_migration(db.pool())
            .await
            .unwrap();

        let config = AppConfig::default();
        let state = AppState::new(config, Arc::new(db));

        // Create company & select as active
        let company = crate::company::CompanyService::create_company(
            state.db.pool(),
            crate::company::CreateCompanyRequest {
                name: "Stage 4 IPC Test Co".to_string(),
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

        *state.active_company_id.write().unwrap() = Some(company.id.clone());

        // Create a test series
        let series = exec_create_voucher_series(
            &state,
            crate::vouchers::CreateVoucherSeriesRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_name: "Draft Series".to_string(),
                series_code: "DFT".to_string(),
                numbering_method: "automatic".to_string(),
                restart_period: "yearly".to_string(),
                applicable_from: Some("2026-04-01".to_string()),
                prefix: Some("DFT/{YY-YY}/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();

        // Create test ledger
        let group_id = "grp_cmd_stage4".to_string();
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ($1, $2, 'Current Assets IPC', NULL, 'Assets', 1)",
        )
        .bind(&group_id)
        .bind(&company.id)
        .execute(state.db.pool())
        .await
        .unwrap();

        let ledger1_id = "led1_cmd_stage4".to_string();
        let ledger2_id = "led2_cmd_stage4".to_string();
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ($1, $2, $3, 'Cash Test', 0, 'standard'), ($4, $2, $3, 'Sales Test', 0, 'standard')",
        )
        .bind(&ledger1_id)
        .bind(&company.id)
        .bind(&group_id)
        .bind(&ledger2_id)
        .execute(state.db.pool())
        .await
        .unwrap();

        // 1. exec_create_voucher_draft
        let draft = exec_create_voucher_draft(
            &state,
            crate::vouchers::CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_id: series.id.clone(),
                date: "2026-05-15".to_string(),
                effective_date: None,
                reference_no: Some("REF-001".to_string()),
                reference_date: None,
                narration: Some("Draft IPC test".to_string()),
                lines: Some(vec![crate::vouchers::VoucherLineInput {
                    id: None,
                    ledger_id: ledger1_id.clone(),
                    entry_type: "Dr".to_string(),
                    amount: "2500.00".to_string(),
                    narration: None,
                }]),
            },
        )
        .await
        .unwrap();
        assert_eq!(draft.voucher.status, "draft");
        assert_eq!(draft.lines.len(), 1);

        // 2. exec_get_voucher_draft
        let fetched = exec_get_voucher_draft(&state, draft.voucher.id.clone())
            .await
            .unwrap();
        assert_eq!(fetched.voucher.id, draft.voucher.id);

        // 3. exec_list_voucher_drafts
        let drafts_list =
            exec_list_voucher_drafts(&state, crate::vouchers::VoucherFilter::default())
                .await
                .unwrap();
        assert_eq!(drafts_list.items.len(), 1);

        // 4. exec_update_voucher_draft
        let updated_hdr = exec_update_voucher_draft(
            &state,
            draft.voucher.id.clone(),
            crate::vouchers::UpdateVoucherDraftRequest {
                narration: Some("Updated via IPC".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(updated_hdr.narration, Some("Updated via IPC".to_string()));

        // 5. exec_add_voucher_line
        let line2 = exec_add_voucher_line(
            &state,
            draft.voucher.id.clone(),
            crate::vouchers::AddVoucherLineRequest {
                ledger_id: ledger2_id.clone(),
                entry_type: "Cr".to_string(),
                amount: "2500.00".to_string(),
                narration: None,
                line_number: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(line2.line_number, 2);

        // 6. exec_update_voucher_line
        let line2_updated = exec_update_voucher_line(
            &state,
            draft.voucher.id.clone(),
            line2.id.clone(),
            crate::vouchers::UpdateVoucherLineRequest {
                amount: Some("3000.00".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(line2_updated.amount, "3000.00");

        // 7. exec_reorder_voucher_lines
        let line1_id = draft.lines[0].id.clone();
        let reordered = exec_reorder_voucher_lines(
            &state,
            draft.voucher.id.clone(),
            crate::vouchers::ReorderVoucherLinesRequest {
                line_ids: vec![line2.id.clone(), line1_id.clone()],
            },
        )
        .await
        .unwrap();
        assert_eq!(reordered[0].id, line2.id);
        assert_eq!(reordered[0].line_number, 1);

        // 8. exec_delete_voucher_line
        exec_delete_voucher_line(&state, draft.voucher.id.clone(), line2.id.clone())
            .await
            .unwrap();
        let after_line_del = exec_get_voucher_draft(&state, draft.voucher.id.clone())
            .await
            .unwrap();
        assert_eq!(after_line_del.lines.len(), 1);

        // 9. exec_save_voucher_draft (atomic replace)
        let saved = exec_save_voucher_draft(
            &state,
            crate::vouchers::SaveVoucherDraftRequest {
                voucher_id: Some(draft.voucher.id.clone()),
                voucher_type_id: "vt_sales".to_string(),
                series_id: series.id.clone(),
                date: "2026-05-20".to_string(),
                effective_date: None,
                reference_no: Some("REF-SAVED".to_string()),
                reference_date: None,
                narration: Some("Saved atomic".to_string()),
                lines: vec![
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: ledger1_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: "5000.00".to_string(),
                        narration: None,
                    },
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: ledger2_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: "5000.00".to_string(),
                        narration: None,
                    },
                ],
            },
        )
        .await
        .unwrap();
        assert_eq!(saved.lines.len(), 2);
        assert!(saved.is_balanced);

        // 10. exec_post_voucher
        let posted = exec_post_voucher(
            &state,
            crate::vouchers::PostVoucherRequest {
                voucher_id: saved.voucher.id.clone(),
                actor_id: Some("operator_test".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(posted.voucher.status, "posted");
        assert!(posted.voucher.sequence_number.is_some());

        // 11. exec_delete_voucher_draft (deleting posted voucher is rejected)
        assert!(exec_delete_voucher_draft(&state, draft.voucher.id.clone())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_stage6_registered_voucher_cancellation_reversal_correction_ipc_commands() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("cmd_stage6_test.db");
        let db = DatabaseManager::init(&db_path).await.unwrap();
        crate::database::migrations::run_sprint1_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_stage3_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_migration(db.pool())
            .await
            .unwrap();
        crate::database::migrations::run_sprint2_stage4_hardening_migration(db.pool())
            .await
            .unwrap();

        let config = AppConfig::default();
        let state = AppState::new(config, Arc::new(db));

        // Create company & select as active
        let company = crate::company::CompanyService::create_company(
            state.db.pool(),
            crate::company::CreateCompanyRequest {
                name: "Stage 6 IPC Test Co".to_string(),
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

        *state.active_company_id.write().unwrap() = Some(company.id.clone());

        // Create Sales series
        let sales_series = exec_create_voucher_series(
            &state,
            crate::vouchers::CreateVoucherSeriesRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_name: "Sales Primary".to_string(),
                series_code: "SALES".to_string(),
                numbering_method: "automatic".to_string(),
                restart_period: "yearly".to_string(),
                applicable_from: Some("2026-04-01".to_string()),
                prefix: Some("SAL/{YY-YY}/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();

        // Create Reversal series
        exec_create_voucher_series(
            &state,
            crate::vouchers::CreateVoucherSeriesRequest {
                voucher_type_id: "vt_reversing_journal".to_string(),
                series_name: "Reversal Primary".to_string(),
                series_code: "REV".to_string(),
                numbering_method: "automatic".to_string(),
                restart_period: "yearly".to_string(),
                applicable_from: Some("2026-04-01".to_string()),
                prefix: Some("REV/{YY-YY}/".to_string()),
                suffix: None,
                start_number: Some(1),
                width: Some(4),
            },
        )
        .await
        .unwrap();

        // Create Ledgers
        let group_id = "grp_cmd_stage6".to_string();
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ($1, $2, 'Current Assets IPC 6', NULL, 'Assets', 1)",
        )
        .bind(&group_id)
        .bind(&company.id)
        .execute(state.db.pool())
        .await
        .unwrap();

        let cash_id = "cash_cmd_stage6".to_string();
        let sales_id = "sales_cmd_stage6".to_string();
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ($1, $2, $3, 'Cash Test 6', 0, 'standard'), ($4, $2, $3, 'Sales Test 6', 0, 'standard')",
        )
        .bind(&cash_id)
        .bind(&company.id)
        .bind(&group_id)
        .bind(&sales_id)
        .execute(state.db.pool())
        .await
        .unwrap();

        // Post Voucher 1 for cancellation test
        let draft1 = exec_create_voucher_draft(
            &state,
            crate::vouchers::CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_id: sales_series.id.clone(),
                date: "2026-05-10".to_string(),
                effective_date: None,
                reference_no: Some("INV-001".to_string()),
                reference_date: None,
                narration: Some("To be cancelled".to_string()),
                lines: Some(vec![
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: cash_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: "1000.00".to_string(),
                        narration: None,
                    },
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: sales_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: "1000.00".to_string(),
                        narration: None,
                    },
                ]),
            },
        )
        .await
        .unwrap();

        let posted1 = exec_post_voucher(
            &state,
            crate::vouchers::PostVoucherRequest {
                voucher_id: draft1.voucher.id.clone(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();

        // 1. Test exec_cancel_voucher
        let cancelled = exec_cancel_voucher(
            &state,
            crate::vouchers::CancelVoucherRequest {
                voucher_id: posted1.voucher.id.clone(),
                reason: "Customer cancelled transaction".to_string(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(cancelled.voucher.status, "cancelled");

        // Post Voucher 2 for reversal test
        let draft2 = exec_create_voucher_draft(
            &state,
            crate::vouchers::CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_id: sales_series.id.clone(),
                date: "2026-05-12".to_string(),
                effective_date: None,
                reference_no: Some("INV-002".to_string()),
                reference_date: None,
                narration: Some("To be reversed".to_string()),
                lines: Some(vec![
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: cash_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: "2000.00".to_string(),
                        narration: None,
                    },
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: sales_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: "2000.00".to_string(),
                        narration: None,
                    },
                ]),
            },
        )
        .await
        .unwrap();

        let posted2 = exec_post_voucher(
            &state,
            crate::vouchers::PostVoucherRequest {
                voucher_id: draft2.voucher.id.clone(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();

        // 2. Test exec_reverse_voucher
        let rev_result = exec_reverse_voucher(
            &state,
            crate::vouchers::ReverseVoucherRequest {
                original_voucher_id: posted2.voucher.id.clone(),
                reversal_date: "2026-05-15".to_string(),
                reason: "Incorrect client charged".to_string(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(rev_result.original_voucher.voucher.status, "reversed");
        assert_eq!(rev_result.reversing_voucher.voucher.status, "posted");

        // Post Voucher 3 for correction draft test
        let draft3 = exec_create_voucher_draft(
            &state,
            crate::vouchers::CreateVoucherDraftRequest {
                voucher_type_id: "vt_sales".to_string(),
                series_id: sales_series.id.clone(),
                date: "2026-05-18".to_string(),
                effective_date: None,
                reference_no: Some("INV-003".to_string()),
                reference_date: None,
                narration: Some("To be corrected".to_string()),
                lines: Some(vec![
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: cash_id.clone(),
                        entry_type: "Dr".to_string(),
                        amount: "3000.00".to_string(),
                        narration: None,
                    },
                    crate::vouchers::VoucherLineInput {
                        id: None,
                        ledger_id: sales_id.clone(),
                        entry_type: "Cr".to_string(),
                        amount: "3000.00".to_string(),
                        narration: None,
                    },
                ]),
            },
        )
        .await
        .unwrap();

        let posted3 = exec_post_voucher(
            &state,
            crate::vouchers::PostVoucherRequest {
                voucher_id: draft3.voucher.id.clone(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();

        // 3. Test exec_create_correction_draft
        let cor_result = exec_create_correction_draft(
            &state,
            crate::vouchers::CreateCorrectionDraftRequest {
                original_voucher_id: posted3.voucher.id.clone(),
                reversal_date: Some("2026-05-19".to_string()),
                reversal_reason: "Price revised downward".to_string(),
                actor_id: Some("operator_ipc".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(cor_result.original_voucher.voucher.status, "reversed");
        assert_eq!(cor_result.reversing_voucher.voucher.status, "posted");
        assert_eq!(cor_result.corrected_draft.voucher.status, "draft");
        assert_eq!(cor_result.corrected_draft.lines.len(), 2);
    }
}
