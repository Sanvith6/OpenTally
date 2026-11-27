use crate::app::AppState;
pub mod factory_ui;
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

pub async fn exec_list_parties(
    state: &AppState,
    search: Option<String>,
) -> Result<Vec<crate::party::PartyProfile>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::party::PartyService::list(state.db.pool(), &company_id, search.as_deref()).await
}

#[tauri::command]
pub async fn list_parties(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::party::PartyProfile>, OpenTallyError> {
    exec_list_parties(&state, search).await
}

pub async fn exec_get_party(
    state: &AppState,
    id: String,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::party::PartyService::get(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn get_party(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    exec_get_party(&state, id).await
}

pub async fn exec_create_party(
    state: &AppState,
    request: crate::party::PartyRequest,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::party::PartyService::create(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn create_party(
    state: tauri::State<'_, AppState>,
    request: crate::party::PartyRequest,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    exec_create_party(&state, request).await
}

pub async fn exec_update_party(
    state: &AppState,
    id: String,
    request: crate::party::PartyRequest,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::party::PartyService::update(state.db.pool(), &company_id, &id, request).await
}

#[tauri::command]
pub async fn update_party(
    state: tauri::State<'_, AppState>,
    id: String,
    request: crate::party::PartyRequest,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    exec_update_party(&state, id, request).await
}

pub async fn exec_set_party_active(
    state: &AppState,
    id: String,
    active: bool,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::party::PartyService::set_active(state.db.pool(), &company_id, &id, active).await
}

#[tauri::command]
pub async fn set_party_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::party::PartyProfile, OpenTallyError> {
    exec_set_party_active(&state, id, active).await
}

pub async fn exec_list_sales_drafts(
    state: &AppState,
    search: Option<String>,
) -> Result<Vec<crate::sales::SalesDocument>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::sales::SalesDocumentService::list_drafts(state.db.pool(), &company_id, search.as_deref())
        .await
}

#[tauri::command]
pub async fn list_sales_drafts(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::sales::SalesDocument>, OpenTallyError> {
    exec_list_sales_drafts(&state, search).await
}

#[tauri::command]
pub async fn get_sales_draft(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::sales::SalesDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::sales::SalesDocumentService::get_draft(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn create_sales_draft(
    state: tauri::State<'_, AppState>,
    request: crate::sales::SalesDocumentRequest,
) -> Result<crate::sales::SalesDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::sales::SalesDocumentService::create_draft(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn update_sales_draft(
    state: tauri::State<'_, AppState>,
    id: String,
    request: crate::sales::SalesDocumentRequest,
) -> Result<crate::sales::SalesDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::sales::SalesDocumentService::update_draft(state.db.pool(), &company_id, &id, request)
        .await
}

#[tauri::command]
pub async fn delete_sales_draft(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::sales::SalesDocumentService::delete_draft(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn reorder_sales_lines(
    state: tauri::State<'_, AppState>,
    id: String,
    line_ids: Vec<String>,
) -> Result<crate::sales::SalesDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::sales::SalesDocumentService::reorder_lines(state.db.pool(), &company_id, &id, line_ids)
        .await
}

#[tauri::command]
pub async fn post_sales_document(
    state: tauri::State<'_, AppState>,
    id: String,
    sales_ledger_id: Option<String>,
) -> Result<crate::commercial::CommercialPostingResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    crate::sales::SalesDocumentService::post_sales_document(
        state.db.pool(),
        &company_id,
        &id,
        sales_ledger_id,
    )
    .await
}

#[tauri::command]
pub async fn list_purchase_drafts(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::purchase::PurchaseDocument>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::purchase::PurchaseDocumentService::list_drafts(
        state.db.pool(),
        &company_id,
        search.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_purchase_draft(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::purchase::PurchaseDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::purchase::PurchaseDocumentService::get_draft(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn create_purchase_draft(
    state: tauri::State<'_, AppState>,
    request: crate::purchase::PurchaseDocumentRequest,
) -> Result<crate::purchase::PurchaseDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::purchase::PurchaseDocumentService::create_draft(state.db.pool(), &company_id, request)
        .await
}

#[tauri::command]
pub async fn update_purchase_draft(
    state: tauri::State<'_, AppState>,
    id: String,
    request: crate::purchase::PurchaseDocumentRequest,
) -> Result<crate::purchase::PurchaseDocumentWithLines, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::purchase::PurchaseDocumentService::update_draft(
        state.db.pool(),
        &company_id,
        &id,
        request,
    )
    .await
}

#[tauri::command]
pub async fn delete_purchase_draft(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::purchase::PurchaseDocumentService::delete_draft(state.db.pool(), &company_id, &id).await
}

#[tauri::command]
pub async fn post_purchase_document(
    state: tauri::State<'_, AppState>,
    id: String,
    purchase_ledger_id: Option<String>,
    tds_section_code: Option<String>,
) -> Result<crate::commercial::CommercialPostingResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    crate::purchase::PurchaseDocumentService::post_purchase_document_with_tds(
        state.db.pool(),
        &company_id,
        &id,
        purchase_ledger_id,
        tds_section_code,
    )
    .await
}

pub async fn exec_day_book(
    state: &AppState,
    request: crate::reports::DayBookRequest,
) -> Result<crate::reports::ReportPage<crate::reports::DayBookEntry>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::DayBookService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn day_book(
    state: tauri::State<'_, AppState>,
    request: crate::reports::DayBookRequest,
) -> Result<crate::reports::ReportPage<crate::reports::DayBookEntry>, OpenTallyError> {
    exec_day_book(&state, request).await
}

pub async fn exec_ledger_report(
    state: &AppState,
    request: crate::reports::LedgerReportRequest,
) -> Result<crate::reports::LedgerReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::LedgerReportService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn ledger_report(
    state: tauri::State<'_, AppState>,
    request: crate::reports::LedgerReportRequest,
) -> Result<crate::reports::LedgerReport, OpenTallyError> {
    exec_ledger_report(&state, request).await
}

pub async fn exec_trial_balance(
    state: &AppState,
    request: crate::reports::TrialBalanceRequest,
) -> Result<crate::reports::TrialBalanceReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewFinancialStatements,
        )
        .await?;
    crate::reports::TrialBalanceService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn trial_balance(
    state: tauri::State<'_, AppState>,
    request: crate::reports::TrialBalanceRequest,
) -> Result<crate::reports::TrialBalanceReport, OpenTallyError> {
    exec_trial_balance(&state, request).await
}

pub async fn exec_profit_and_loss(
    state: &AppState,
    request: crate::reports::ProfitAndLossRequest,
) -> Result<crate::reports::ProfitAndLossReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewFinancialStatements,
        )
        .await?;
    crate::reports::ProfitAndLossService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn profit_and_loss(
    state: tauri::State<'_, AppState>,
    request: crate::reports::ProfitAndLossRequest,
) -> Result<crate::reports::ProfitAndLossReport, OpenTallyError> {
    exec_profit_and_loss(&state, request).await
}

pub async fn exec_balance_sheet(
    state: &AppState,
    request: crate::reports::BalanceSheetRequest,
) -> Result<crate::reports::BalanceSheetReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewFinancialStatements,
        )
        .await?;
    crate::reports::BalanceSheetService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn balance_sheet(
    state: tauri::State<'_, AppState>,
    request: crate::reports::BalanceSheetRequest,
) -> Result<crate::reports::BalanceSheetReport, OpenTallyError> {
    exec_balance_sheet(&state, request).await
}

pub async fn exec_cash_flow(
    state: &AppState,
    request: crate::reports::CashFlowRequest,
) -> Result<crate::reports::CashFlowReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewFinancialStatements,
        )
        .await?;
    crate::reports::CashFlowService::generate(state.db.pool(), &company_id, request).await
}

#[tauri::command]
pub async fn cash_flow(
    state: tauri::State<'_, AppState>,
    request: crate::reports::CashFlowRequest,
) -> Result<crate::reports::CashFlowReport, OpenTallyError> {
    exec_cash_flow(&state, request).await
}

pub async fn exec_export_report_preview(
    state: &AppState,
    request: crate::vouchers::VoucherExportPreviewRequest,
) -> Result<crate::vouchers::VoucherExportPreviewResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::vouchers::VoucherExportService::export_preview(state.db.pool(), &company_id, request)
        .await
}

#[tauri::command]
pub async fn export_report_preview(
    state: tauri::State<'_, AppState>,
    request: crate::vouchers::VoucherExportPreviewRequest,
) -> Result<crate::vouchers::VoucherExportPreviewResult, OpenTallyError> {
    exec_export_report_preview(&state, request).await
}

// ---------------------------------------------------------------------------
// Milestone 2: Bill-Wise Outstanding Tracking (AR/AP) & Aging Commands
// ---------------------------------------------------------------------------

pub async fn exec_get_bills_receivable(
    state: &AppState,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::billwise::BillWiseReportService::get_bills_receivable(
        state.db.pool(),
        &company_id,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_bills_receivable(
    state: tauri::State<'_, AppState>,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    exec_get_bills_receivable(&state, as_of_date).await
}

pub async fn exec_get_bills_payable(
    state: &AppState,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::billwise::BillWiseReportService::get_bills_payable(
        state.db.pool(),
        &company_id,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_bills_payable(
    state: tauri::State<'_, AppState>,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    exec_get_bills_payable(&state, as_of_date).await
}

pub async fn exec_get_party_pending_bills(
    state: &AppState,
    party_ledger_id: String,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::billwise::BillWiseReportService::get_party_pending_bills(
        state.db.pool(),
        &company_id,
        &party_ledger_id,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_party_pending_bills(
    state: tauri::State<'_, AppState>,
    party_ledger_id: String,
    as_of_date: String,
) -> Result<Vec<crate::billwise::PendingBill>, OpenTallyError> {
    exec_get_party_pending_bills(&state, party_ledger_id, as_of_date).await
}

pub async fn exec_get_aging_analysis(
    state: &AppState,
    report_type: String,
    as_of_date: String,
) -> Result<crate::billwise::AgingReport, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::billwise::BillWiseReportService::get_aging_analysis(
        state.db.pool(),
        &company_id,
        &report_type,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_aging_analysis(
    state: tauri::State<'_, AppState>,
    report_type: String,
    as_of_date: String,
) -> Result<crate::billwise::AgingReport, OpenTallyError> {
    exec_get_aging_analysis(&state, report_type, as_of_date).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn record_bill_allocations(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    voucher_line_id: String,
    party_ledger_id: String,
    line_entry_type: String,
    line_amount: String,
    bill_date: String,
    due_date: Option<String>,
    allocations: Vec<crate::billwise::BillAllocationInput>,
) -> Result<Vec<crate::billwise::BillAllocation>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let parsed_amt = rust_decimal::Decimal::from_str_exact(&line_amount)
        .map_err(|_| OpenTallyError::Validation(format!("Invalid line amount: {}", line_amount)))?;
    let mut conn = state
        .db
        .pool()
        .acquire()
        .await
        .map_err(OpenTallyError::Database)?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;
    let res = crate::billwise::BillWiseService::validate_and_post_allocations(
        &mut conn,
        &company_id,
        &voucher_id,
        &voucher_line_id,
        &party_ledger_id,
        &line_entry_type,
        parsed_amt,
        &bill_date,
        due_date.as_deref(),
        &allocations,
    )
    .await;
    match res {
        Ok(v) => {
            sqlx::query("COMMIT")
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
            Ok(v)
        }
        Err(e) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(e)
        }
    }
}

// -----------------------------------------------------------------------------
// Milestone 3: Indian GST IPC Commands
// -----------------------------------------------------------------------------

#[tauri::command]
pub async fn get_company_gst_profile(
    state: tauri::State<'_, AppState>,
) -> Result<Option<crate::gst::CompanyGstProfile>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::get_company_gst_profile(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn save_company_gst_profile(
    state: tauri::State<'_, AppState>,
    gstin: Option<String>,
    state_code: String,
    registration_type: String,
    gst_enabled: bool,
) -> Result<crate::gst::CompanyGstProfile, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::save_company_gst_profile(
        state.db.pool(),
        &company_id,
        gstin.as_deref(),
        &state_code,
        &registration_type,
        gst_enabled,
    )
    .await
}

#[tauri::command]
pub async fn get_ledger_gst_profile(
    state: tauri::State<'_, AppState>,
    ledger_id: String,
) -> Result<Option<crate::gst::LedgerGstProfile>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::get_ledger_gst_profile(state.db.pool(), &company_id, &ledger_id).await
}

#[tauri::command]
pub async fn save_ledger_gst_profile(
    state: tauri::State<'_, AppState>,
    ledger_id: String,
    gstin: Option<String>,
    state_code: String,
    registration_type: String,
) -> Result<crate::gst::LedgerGstProfile, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::save_ledger_gst_profile(
        state.db.pool(),
        &company_id,
        &ledger_id,
        gstin.as_deref(),
        &state_code,
        &registration_type,
    )
    .await
}

#[tauri::command]
pub async fn list_hsn_sac_codes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::gst::HsnSacCode>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::list_hsn_sac_codes(state.db.pool(), &company_id).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_hsn_sac_code(
    state: tauri::State<'_, AppState>,
    code: String,
    description: Option<String>,
    code_type: String,
    default_gst_rate: String,
    cess_rate: String,
    effective_from: String,
) -> Result<crate::gst::HsnSacCode, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::create_hsn_sac_code(
        state.db.pool(),
        &company_id,
        &code,
        description.as_deref(),
        &code_type,
        &default_gst_rate,
        &cess_rate,
        &effective_from,
    )
    .await
}

#[tauri::command]
pub async fn get_item_gst_mapping(
    state: tauri::State<'_, AppState>,
    stock_item_id: String,
) -> Result<Option<crate::gst::ItemGstMapping>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::get_item_gst_mapping(state.db.pool(), &company_id, &stock_item_id).await
}

#[tauri::command]
pub async fn save_item_gst_mapping(
    state: tauri::State<'_, AppState>,
    stock_item_id: String,
    hsn_sac_code_id: String,
    gst_rate: String,
    cess_rate: String,
    taxability: String,
) -> Result<crate::gst::ItemGstMapping, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::save_item_gst_mapping(
        state.db.pool(),
        &company_id,
        &stock_item_id,
        &hsn_sac_code_id,
        &gst_rate,
        &cess_rate,
        &taxability,
    )
    .await
}

#[tauri::command]
pub async fn get_gst_ledger_mapping(
    state: tauri::State<'_, AppState>,
) -> Result<Option<crate::gst::GstLedgerMapping>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::gst::GstService::get_gst_ledger_mapping(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn save_gst_ledger_mapping(
    state: tauri::State<'_, AppState>,
    mapping: crate::gst::GstLedgerMapping,
) -> Result<crate::gst::GstLedgerMapping, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let mut clean_map = mapping;
    clean_map.company_id = company_id;
    crate::gst::GstService::save_gst_ledger_mapping(state.db.pool(), clean_map).await
}

#[tauri::command]
pub async fn preview_gst_calculation(
    state: tauri::State<'_, AppState>,
    place_of_supply_state_code: Option<String>,
    taxability: String,
    taxable_value: String,
    gst_rate: String,
    cess_rate: String,
) -> Result<crate::gst::GstCalculationResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let comp_profile =
        crate::gst::GstService::get_company_gst_profile(state.db.pool(), &company_id).await?;
    let comp_state = comp_profile
        .as_ref()
        .map(|p| p.state_code.clone())
        .unwrap_or_else(|| "01".to_string());
    let pos = place_of_supply_state_code.unwrap_or_else(|| comp_state.clone());

    let val = rust_decimal::Decimal::from_str_exact(&taxable_value).map_err(|_| {
        OpenTallyError::Validation(format!("Invalid taxable value: {}", taxable_value))
    })?;
    let rate = rust_decimal::Decimal::from_str_exact(&gst_rate)
        .map_err(|_| OpenTallyError::Validation(format!("Invalid GST rate: {}", gst_rate)))?;
    let cess = rust_decimal::Decimal::from_str_exact(&cess_rate)
        .map_err(|_| OpenTallyError::Validation(format!("Invalid Cess rate: {}", cess_rate)))?;

    let input = crate::gst::GstCalculationInput {
        company_state_code: comp_state,
        place_of_supply_state_code: pos,
        taxability: crate::gst::Taxability::from_str_loose(&taxability),
        taxable_value: val,
        gst_rate: rate,
        cess_rate: cess,
        is_reverse_charge: false,
    };

    Ok(crate::gst::calculate_gst(&input))
}

#[tauri::command]
pub async fn get_gstr1_summary(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<crate::reports::gst::Gstr1SummaryReport, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::reports::gst::GstReportService::get_gstr1_summary(
        state.db.pool(),
        &company_id,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn get_gstr3b_summary(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<crate::reports::gst::Gstr3bSummaryReport, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::reports::gst::GstReportService::get_gstr3b_summary(
        state.db.pool(),
        &company_id,
        &from_date,
        &to_date,
    )
    .await
}

pub async fn exec_global_goto_search(
    state: &AppState,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<crate::navigation::GotoSearchResult>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::navigation::NavigationSearchService::global_goto_search(
        state.db.pool(),
        &company_id,
        &query,
        limit.unwrap_or(20),
    )
    .await
}

#[tauri::command]
pub async fn global_goto_search(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<crate::navigation::GotoSearchResult>, OpenTallyError> {
    exec_global_goto_search(&state, query, limit).await
}

pub async fn exec_get_voucher_full_detail(
    state: &AppState,
    voucher_id: String,
) -> Result<crate::navigation::VoucherFullDetail, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::navigation::NavigationSearchService::get_voucher_full_detail(
        state.db.pool(),
        &company_id,
        &voucher_id,
    )
    .await
}

#[tauri::command]
pub async fn get_voucher_full_detail(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<crate::navigation::VoucherFullDetail, OpenTallyError> {
    exec_get_voucher_full_detail(&state, voucher_id).await
}

pub async fn exec_export_invoice_pdf(
    state: &AppState,
    voucher_id: String,
    output_path: String,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::pdf::export_invoice_pdf_file(state.db.pool(), &company_id, &voucher_id, &output_path)
        .await
}

#[tauri::command]
pub async fn export_invoice_pdf(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
    output_path: String,
) -> Result<String, OpenTallyError> {
    exec_export_invoice_pdf(&state, voucher_id, output_path).await
}

pub async fn exec_create_database_backup(
    state: &AppState,
    destination_path: String,
) -> Result<crate::backup::BackupManifest, OpenTallyError> {
    let active_cid = state.active_company_id.read().ok().and_then(|g| g.clone());
    if let Some(ref cid) = active_cid {
        let _ = state
            .resolve_actor(
                cid,
                crate::security::models::ActionPermission::BackupDatabase,
            )
            .await?;
    }
    let active_db = &state.config.database_path;
    crate::backup::create_database_backup(
        state.db.pool(),
        active_db,
        std::path::Path::new(&destination_path),
    )
    .await
}

#[tauri::command]
pub async fn create_database_backup(
    state: tauri::State<'_, AppState>,
    destination_path: String,
) -> Result<crate::backup::BackupManifest, OpenTallyError> {
    exec_create_database_backup(&state, destination_path).await
}

pub async fn exec_restore_database_backup(
    state: &AppState,
    backup_path: String,
) -> Result<crate::backup::RestoreResult, OpenTallyError> {
    let active_cid = state.active_company_id.read().ok().and_then(|g| g.clone());
    if let Some(ref cid) = active_cid {
        let _ = state
            .resolve_actor(
                cid,
                crate::security::models::ActionPermission::RestoreDatabase,
            )
            .await?;
    }
    let pool = state.db.pool().clone();
    let active_db = state.config.database_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async move {
            crate::backup::restore_database_backup(
                &pool,
                &active_db,
                std::path::Path::new(&backup_path),
            )
            .await
        })
    })
    .await
    .map_err(|e| OpenTallyError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn restore_database_backup(
    state: tauri::State<'_, AppState>,
    backup_path: String,
) -> Result<crate::backup::RestoreResult, OpenTallyError> {
    exec_restore_database_backup(&state, backup_path).await
}

pub async fn exec_get_backup_status(
    state: &AppState,
) -> Result<crate::backup::BackupStatus, OpenTallyError> {
    let active_db = &state.config.database_path;
    crate::backup::get_backup_status(state.db.pool(), active_db).await
}

#[tauri::command]
pub async fn get_backup_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::backup::BackupStatus, OpenTallyError> {
    exec_get_backup_status(&state).await
}

pub async fn exec_post_commercial_voucher(
    state: &AppState,
    mut input: crate::commercial::CommercialPostingInput,
) -> Result<crate::commercial::CommercialPostingResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    input.company_id = company_id.clone();
    crate::commercial::CommercialPostingService::post_commercial_voucher(
        state.db.pool(),
        &company_id,
        input,
    )
    .await
}

#[tauri::command]
pub async fn post_commercial_voucher(
    state: tauri::State<'_, AppState>,
    input: crate::commercial::CommercialPostingInput,
) -> Result<crate::commercial::CommercialPostingResult, OpenTallyError> {
    exec_post_commercial_voucher(&state, input).await
}

pub async fn exec_export_company_tally_xml(
    state: &AppState,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::interoperability::export_company_tally_xml(
        state.db.pool(),
        &company_id,
        from_date.as_deref(),
        to_date.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn export_company_tally_xml(
    state: tauri::State<'_, AppState>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<String, OpenTallyError> {
    exec_export_company_tally_xml(&state, from_date, to_date).await
}

pub async fn exec_preview_tally_xml_import(
    state: &AppState,
    xml_content: String,
) -> Result<crate::interoperability::TallyXmlImportPreview, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::interoperability::preview_tally_xml_import(state.db.pool(), &company_id, &xml_content)
        .await
}

#[tauri::command]
pub async fn preview_tally_xml_import(
    state: tauri::State<'_, AppState>,
    xml_content: String,
) -> Result<crate::interoperability::TallyXmlImportPreview, OpenTallyError> {
    exec_preview_tally_xml_import(&state, xml_content).await
}

pub async fn exec_execute_tally_xml_import(
    state: &AppState,
    xml_content: String,
) -> Result<crate::interoperability::TallyXmlImportResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::interoperability::execute_tally_xml_import(state.db.pool(), &company_id, &xml_content)
        .await
}

#[tauri::command]
pub async fn execute_tally_xml_import(
    state: tauri::State<'_, AppState>,
    xml_content: String,
) -> Result<crate::interoperability::TallyXmlImportResult, OpenTallyError> {
    exec_execute_tally_xml_import(&state, xml_content).await
}

pub async fn exec_export_gstr1_json(
    state: &AppState,
    from_date: String,
    to_date: String,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::gst::GstReportService::export_gstr1_json(
        state.db.pool(),
        &company_id,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn export_gstr1_json(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<String, OpenTallyError> {
    exec_export_gstr1_json(&state, from_date, to_date).await
}

#[tauri::command]
pub async fn validate_gstr1_export(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<crate::gst::gstn_validator::GstnValidationReport, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::reports::gst::GstReportService::validate_gstr1_export(
        state.db.pool(),
        &company_id,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn get_cmp08_quarterly_summary(
    state: tauri::State<'_, AppState>,
    financial_year: i32,
    quarter: u8,
) -> Result<crate::reports::gst::Cmp08QuarterlySummary, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::reports::gst::GstReportService::get_cmp08_quarterly_summary(
        state.db.pool(),
        &company_id,
        financial_year,
        quarter,
    )
    .await
}

pub async fn exec_export_gstr3b_json(
    state: &AppState,
    from_date: String,
    to_date: String,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::reports::gst::GstReportService::export_gstr3b_json(
        state.db.pool(),
        &company_id,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn export_gstr3b_json(
    state: tauri::State<'_, AppState>,
    from_date: String,
    to_date: String,
) -> Result<String, OpenTallyError> {
    exec_export_gstr3b_json(&state, from_date, to_date).await
}

pub async fn exec_get_company_features(
    state: &AppState,
    company_id: Option<String>,
) -> Result<crate::company::features::CompanyFeaturesDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::company::features::CompanyFeaturesService::get_company_features(state.db.pool(), &cid)
        .await
}

#[tauri::command]
pub async fn get_company_features(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<crate::company::features::CompanyFeaturesDto, OpenTallyError> {
    exec_get_company_features(&state, company_id).await
}

pub async fn exec_save_company_features(
    state: &AppState,
    company_id: Option<String>,
    input: crate::company::features::SaveCompanyFeaturesInput,
) -> Result<crate::company::features::CompanyFeaturesDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::company::features::CompanyFeaturesService::save_company_features(
        state.db.pool(),
        &cid,
        input,
    )
    .await
}

#[tauri::command]
pub async fn save_company_features(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::company::features::SaveCompanyFeaturesInput,
) -> Result<crate::company::features::CompanyFeaturesDto, OpenTallyError> {
    exec_save_company_features(&state, company_id, input).await
}

pub async fn exec_create_sample_company(
    state: &AppState,
) -> Result<crate::company::model::Company, OpenTallyError> {
    let company =
        crate::company::sample::SampleCompanyService::create_sample_company(state.db.pool())
            .await?;
    let mut active = state.active_company_id.write().map_err(|e| {
        OpenTallyError::Internal(format!(
            "Failed to acquire active company write lock: {}",
            e
        ))
    })?;
    *active = Some(company.id.clone());
    Ok(company)
}

#[tauri::command]
pub async fn create_sample_company(
    state: tauri::State<'_, AppState>,
) -> Result<crate::company::model::Company, OpenTallyError> {
    exec_create_sample_company(&state).await
}

// ---------------------------------------------------------------------------
// Milestone 8: Banking & BRS IPC Commands
// ---------------------------------------------------------------------------

pub async fn exec_list_bank_ledgers(
    state: &AppState,
) -> Result<Vec<crate::banking::BankLedgerItem>, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::banking::BankingService::list_bank_ledgers(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn list_bank_ledgers(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::banking::BankLedgerItem>, OpenTallyError> {
    exec_list_bank_ledgers(&state).await
}

pub async fn exec_get_bank_reconciliation(
    state: &AppState,
    bank_ledger_id: String,
    from_date: String,
    to_date: String,
    include_reconciled: Option<bool>,
) -> Result<crate::banking::BankReconciliationStatement, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::banking::BankingService::get_bank_reconciliation(
        state.db.pool(),
        &company_id,
        &bank_ledger_id,
        &from_date,
        &to_date,
        include_reconciled.unwrap_or(true),
    )
    .await
}

#[tauri::command]
pub async fn get_bank_reconciliation(
    state: tauri::State<'_, AppState>,
    bank_ledger_id: String,
    from_date: String,
    to_date: String,
    include_reconciled: Option<bool>,
) -> Result<crate::banking::BankReconciliationStatement, OpenTallyError> {
    exec_get_bank_reconciliation(
        &state,
        bank_ledger_id,
        from_date,
        to_date,
        include_reconciled,
    )
    .await
}

pub async fn exec_reconcile_bank_entries(
    state: &AppState,
    updates: Vec<crate::banking::ReconcileEntryInput>,
) -> Result<crate::banking::ReconcileResult, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::banking::BankingService::reconcile_bank_entries(
        state.db.pool(),
        &company_id,
        updates,
        None,
    )
    .await
}

#[tauri::command]
pub async fn reconcile_bank_entries(
    state: tauri::State<'_, AppState>,
    updates: Vec<crate::banking::ReconcileEntryInput>,
) -> Result<crate::banking::ReconcileResult, OpenTallyError> {
    exec_reconcile_bank_entries(&state, updates).await
}

pub async fn exec_preview_bank_statement_auto_match(
    state: &AppState,
    bank_ledger_id: String,
    csv_content: String,
) -> Result<crate::banking::BankStatementMatchPreview, OpenTallyError> {
    let company_id = require_active_company(state).await?;
    crate::banking::BankingService::preview_bank_statement_auto_match(
        state.db.pool(),
        &company_id,
        &bank_ledger_id,
        &csv_content,
    )
    .await
}

#[tauri::command]
pub async fn preview_bank_statement_auto_match(
    state: tauri::State<'_, AppState>,
    bank_ledger_id: String,
    csv_content: String,
) -> Result<crate::banking::BankStatementMatchPreview, OpenTallyError> {
    exec_preview_bank_statement_auto_match(&state, bank_ledger_id, csv_content).await
}

// ---------------------------------------------------------------------------
// Milestone 9: Indian TDS & TCS, Challan Tracking & Form 26Q IPC Commands
// ---------------------------------------------------------------------------

pub async fn exec_list_tds_sections(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Vec<crate::tds::TdsNatureOfPayment>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::tds::TdsService::list_tds_sections(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn list_tds_sections(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::tds::TdsNatureOfPayment>, OpenTallyError> {
    exec_list_tds_sections(&state, company_id).await
}

pub async fn exec_create_tds_section(
    state: &AppState,
    mut input: crate::tds::CreateTdsNatureOfPaymentInput,
) -> Result<crate::tds::TdsNatureOfPayment, OpenTallyError> {
    if input.company_id.is_empty() {
        input.company_id = require_active_company(state).await?;
    }
    crate::tds::TdsService::create_tds_section(state.db.pool(), input, None).await
}

#[tauri::command]
pub async fn create_tds_section(
    state: tauri::State<'_, AppState>,
    input: crate::tds::CreateTdsNatureOfPaymentInput,
) -> Result<crate::tds::TdsNatureOfPayment, OpenTallyError> {
    exec_create_tds_section(&state, input).await
}

pub async fn exec_get_company_tds_profile(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Option<crate::tds::CompanyTdsProfile>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::tds::TdsService::get_company_tds_profile(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn get_company_tds_profile(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Option<crate::tds::CompanyTdsProfile>, OpenTallyError> {
    exec_get_company_tds_profile(&state, company_id).await
}

pub async fn exec_save_company_tds_profile(
    state: &AppState,
    mut input: crate::tds::SaveCompanyTdsProfileInput,
) -> Result<crate::tds::CompanyTdsProfile, OpenTallyError> {
    if input.company_id.is_empty() {
        input.company_id = require_active_company(state).await?;
    }
    crate::tds::TdsService::save_company_tds_profile(state.db.pool(), input, None).await
}

#[tauri::command]
pub async fn save_company_tds_profile(
    state: tauri::State<'_, AppState>,
    input: crate::tds::SaveCompanyTdsProfileInput,
) -> Result<crate::tds::CompanyTdsProfile, OpenTallyError> {
    exec_save_company_tds_profile(&state, input).await
}

pub async fn exec_get_ledger_tds_profile(
    state: &AppState,
    company_id: Option<String>,
    ledger_id: String,
) -> Result<Option<crate::tds::LedgerTdsProfile>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::tds::TdsService::get_ledger_tds_profile(state.db.pool(), &cid, &ledger_id).await
}

#[tauri::command]
pub async fn get_ledger_tds_profile(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    ledger_id: String,
) -> Result<Option<crate::tds::LedgerTdsProfile>, OpenTallyError> {
    exec_get_ledger_tds_profile(&state, company_id, ledger_id).await
}

pub async fn exec_save_ledger_tds_profile(
    state: &AppState,
    mut input: crate::tds::SaveLedgerTdsProfileInput,
) -> Result<crate::tds::LedgerTdsProfile, OpenTallyError> {
    if input.company_id.is_empty() {
        input.company_id = require_active_company(state).await?;
    }
    crate::tds::TdsService::save_ledger_tds_profile(state.db.pool(), input, None).await
}

#[tauri::command]
pub async fn save_ledger_tds_profile(
    state: tauri::State<'_, AppState>,
    input: crate::tds::SaveLedgerTdsProfileInput,
) -> Result<crate::tds::LedgerTdsProfile, OpenTallyError> {
    exec_save_ledger_tds_profile(&state, input).await
}

pub async fn exec_preview_tds_calculation(
    state: &AppState,
    mut input: crate::tds::TdsCalculationPreviewInput,
) -> Result<crate::tds::TdsCalculationPreviewResult, OpenTallyError> {
    if input.company_id.is_empty() {
        input.company_id = require_active_company(state).await?;
    }
    crate::tds::TdsService::preview_tds_calculation(state.db.pool(), input).await
}

#[tauri::command]
pub async fn preview_tds_calculation(
    state: tauri::State<'_, AppState>,
    input: crate::tds::TdsCalculationPreviewInput,
) -> Result<crate::tds::TdsCalculationPreviewResult, OpenTallyError> {
    exec_preview_tds_calculation(&state, input).await
}

pub async fn exec_record_tds_challan_payment(
    state: &AppState,
    mut input: crate::tds::RecordTdsChallanPaymentInput,
) -> Result<usize, OpenTallyError> {
    if input.company_id.is_empty() {
        input.company_id = require_active_company(state).await?;
    }
    crate::tds::TdsService::record_tds_challan_payment(state.db.pool(), input).await
}

#[tauri::command]
pub async fn record_tds_challan_payment(
    state: tauri::State<'_, AppState>,
    input: crate::tds::RecordTdsChallanPaymentInput,
) -> Result<usize, OpenTallyError> {
    exec_record_tds_challan_payment(&state, input).await
}

pub async fn exec_get_tds_outstanding(
    state: &AppState,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::tds::TdsOutstandingReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::reports::TdsReportService::get_tds_outstanding(state.db.pool(), &cid, &as_of_date).await
}

#[tauri::command]
pub async fn get_tds_outstanding(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::tds::TdsOutstandingReport, OpenTallyError> {
    exec_get_tds_outstanding(&state, company_id, as_of_date).await
}

pub async fn exec_get_form_26q_summary(
    state: &AppState,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<crate::tds::Form26qSummary, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::reports::TdsReportService::get_form_26q_summary(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn get_form_26q_summary(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<crate::tds::Form26qSummary, OpenTallyError> {
    exec_get_form_26q_summary(&state, company_id, financial_year, quarter).await
}

pub async fn exec_export_form_26q_json(
    state: &AppState,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::reports::TdsReportService::export_form_26q_json(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn export_form_26q_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    exec_export_form_26q_json(&state, company_id, financial_year, quarter).await
}

#[tauri::command]
pub async fn get_form_27q_summary(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<crate::tds::models::Form26qSummary, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::reports::TdsReportService::get_form_27q_summary(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn export_form_27q_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::reports::TdsReportService::export_form_27q_json(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn get_form_27eq_summary(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<crate::tds::models::Form26qSummary, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::reports::TdsReportService::get_form_27eq_summary(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn export_form_27eq_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::reports::TdsReportService::export_form_27eq_json(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn list_pt_slabs(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    state_code: Option<String>,
) -> Result<Vec<crate::payroll::models::ProfessionalTaxSlab>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::payroll::PayrollService::list_pt_slabs(state.db.pool(), &cid, state_code).await
}

#[tauri::command]
pub async fn export_epfo_ecr_file(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    pay_period: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::payroll::PayrollService::export_epfo_ecr_file(state.db.pool(), &cid, &pay_period).await
}

#[tauri::command]
pub async fn export_trial_balance_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::pdf::statements::export_trial_balance_pdf(state.db.pool(), &cid, &as_of_date).await
}

#[tauri::command]
pub async fn export_profit_and_loss_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    from_date: String,
    to_date: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::pdf::statements::export_profit_and_loss_pdf(state.db.pool(), &cid, &from_date, &to_date)
        .await
}

#[tauri::command]
pub async fn export_balance_sheet_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::pdf::statements::export_balance_sheet_pdf(state.db.pool(), &cid, &as_of_date).await
}

#[tauri::command]
pub async fn export_ledger_statement_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    ledger_id: String,
    from_date: String,
    to_date: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::pdf::statements::export_ledger_statement_pdf(
        state.db.pool(),
        &cid,
        &ledger_id,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn export_outstanding_aging_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    report_type: String,
    as_of_date: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::pdf::statements::export_outstanding_aging_pdf(
        state.db.pool(),
        &cid,
        &report_type,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn dispatch_job_work_material_out(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    request: crate::inventory::job_work::DispatchJobWorkOutRequest,
) -> Result<crate::inventory::job_work::JobWorkChallanDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::inventory::job_work::JobWorkService::dispatch_job_work_material_out(
        state.db.pool(),
        &cid,
        request,
    )
    .await
}

#[tauri::command]
pub async fn receive_job_work_material_in(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    request: crate::inventory::job_work::ReceiveJobWorkInRequest,
) -> Result<crate::inventory::job_work::JobWorkChallanDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::inventory::job_work::JobWorkService::receive_job_work_material_in(
        state.db.pool(),
        &cid,
        request,
    )
    .await
}

#[tauri::command]
pub async fn get_itc04_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    from_date: String,
    to_date: String,
) -> Result<crate::inventory::job_work::FormItc04Report, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::inventory::job_work::JobWorkService::get_itc04_report(
        state.db.pool(),
        &cid,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn export_itc04_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    from_date: String,
    to_date: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::inventory::job_work::JobWorkService::export_itc04_json(
        state.db.pool(),
        &cid,
        &from_date,
        &to_date,
    )
    .await
}

#[tauri::command]
pub async fn parse_weighbridge_reading(
    raw_frame: String,
    tare_weight: String,
) -> Result<crate::extensions::WeighbridgeReading, OpenTallyError> {
    crate::extensions::HardwareBridgeService::parse_weighbridge_reading(&raw_frame, &tare_weight)
}

#[tauri::command]
pub async fn lookup_stock_item_by_barcode(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    barcode: String,
) -> Result<Option<(String, String)>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    crate::extensions::HardwareBridgeService::lookup_stock_item_by_barcode(
        state.db.pool(),
        &cid,
        &barcode,
    )
    .await
}

pub async fn exec_export_form_26q_csv(
    state: &AppState,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::reports::TdsReportService::export_form_26q_csv(
        state.db.pool(),
        &cid,
        &financial_year,
        &quarter,
    )
    .await
}

#[tauri::command]
pub async fn export_form_26q_csv(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    financial_year: String,
    quarter: String,
) -> Result<String, OpenTallyError> {
    exec_export_form_26q_csv(&state, company_id, financial_year, quarter).await
}

// -----------------------------------------------------------------------------
// Milestone 10: GST Compliance (GSTR-2B Reconciliation, e-Invoice, e-Way Bill)
// -----------------------------------------------------------------------------

pub async fn exec_import_and_reconcile_gstr2b(
    state: &AppState,
    company_id: Option<String>,
    return_period: String,
    json_payload: String,
) -> Result<crate::gst::Gstr2bReconciliationReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::Gstr2bService::import_and_reconcile_gstr2b(
        state.db.pool(),
        &cid,
        &return_period,
        &json_payload,
    )
    .await
}

#[tauri::command]
pub async fn import_and_reconcile_gstr2b(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    return_period: String,
    json_payload: String,
) -> Result<crate::gst::Gstr2bReconciliationReport, OpenTallyError> {
    exec_import_and_reconcile_gstr2b(&state, company_id, return_period, json_payload).await
}

pub async fn exec_get_gstr2b_reconciliation_report(
    state: &AppState,
    company_id: Option<String>,
    return_period: String,
) -> Result<crate::gst::Gstr2bReconciliationReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::Gstr2bService::get_gstr2b_reconciliation_report(
        state.db.pool(),
        &cid,
        &return_period,
    )
    .await
}

#[tauri::command]
pub async fn get_gstr2b_reconciliation_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    return_period: String,
) -> Result<crate::gst::Gstr2bReconciliationReport, OpenTallyError> {
    exec_get_gstr2b_reconciliation_report(&state, company_id, return_period).await
}

pub async fn exec_export_gstr2b_reconciliation_csv(
    state: &AppState,
    company_id: Option<String>,
    return_period: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::Gstr2bService::export_gstr2b_reconciliation_csv(
        state.db.pool(),
        &cid,
        &return_period,
    )
    .await
}

#[tauri::command]
pub async fn export_gstr2b_reconciliation_csv(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    return_period: String,
) -> Result<String, OpenTallyError> {
    exec_export_gstr2b_reconciliation_csv(&state, company_id, return_period).await
}

pub async fn exec_export_einvoice_json(
    state: &AppState,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::EinvoiceService::export_einvoice_json(state.db.pool(), &cid, &voucher_id).await
}

#[tauri::command]
pub async fn export_einvoice_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<String, OpenTallyError> {
    exec_export_einvoice_json(&state, company_id, voucher_id).await
}

pub async fn exec_export_eway_bill_json(
    state: &AppState,
    company_id: Option<String>,
    voucher_id: String,
    transport: crate::gst::TransportDetailsInput,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::EinvoiceService::export_eway_bill_json(
        state.db.pool(),
        &cid,
        &voucher_id,
        transport,
    )
    .await
}

#[tauri::command]
pub async fn export_eway_bill_json(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
    transport: crate::gst::TransportDetailsInput,
) -> Result<String, OpenTallyError> {
    exec_export_eway_bill_json(&state, company_id, voucher_id, transport).await
}

pub async fn exec_save_voucher_einvoice_metadata(
    state: &AppState,
    company_id: Option<String>,
    voucher_id: String,
    input: crate::gst::VoucherEinvoiceMetadataInput,
) -> Result<crate::gst::VoucherEinvoiceMetadataDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::EinvoiceService::save_voucher_einvoice_metadata(
        state.db.pool(),
        &cid,
        &voucher_id,
        input,
    )
    .await
}

#[tauri::command]
pub async fn save_voucher_einvoice_metadata(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
    input: crate::gst::VoucherEinvoiceMetadataInput,
) -> Result<crate::gst::VoucherEinvoiceMetadataDto, OpenTallyError> {
    exec_save_voucher_einvoice_metadata(&state, company_id, voucher_id, input).await
}

pub async fn exec_get_voucher_einvoice_metadata(
    state: &AppState,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<Option<crate::gst::VoucherEinvoiceMetadataDto>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::gst::EinvoiceService::get_voucher_einvoice_metadata(state.db.pool(), &cid, &voucher_id)
        .await
}

#[tauri::command]
pub async fn get_voucher_einvoice_metadata(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<Option<crate::gst::VoucherEinvoiceMetadataDto>, OpenTallyError> {
    exec_get_voucher_einvoice_metadata(&state, company_id, voucher_id).await
}

// =========================================================================
// Milestone 11: Godown Summary, Batches, Stock Journal, Cost Centres
// =========================================================================

pub async fn exec_get_godown_summary_report(
    state: &AppState,
    company_id: Option<String>,
    godown_id: String,
    as_of_date: String,
) -> Result<crate::inventory::stock_journal::GodownSummaryReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::inventory::stock_journal::StockJournalService::get_godown_summary_report(
        state.db.pool(),
        &cid,
        &godown_id,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_godown_summary_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    godown_id: String,
    as_of_date: String,
) -> Result<crate::inventory::stock_journal::GodownSummaryReport, OpenTallyError> {
    exec_get_godown_summary_report(&state, company_id, godown_id, as_of_date).await
}

pub async fn exec_list_stock_item_batches(
    state: &AppState,
    company_id: Option<String>,
    stock_item_id: Option<String>,
) -> Result<Vec<crate::inventory::batches::StockItemBatch>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::inventory::batches::BatchService::list_batches(
        state.db.pool(),
        &cid,
        stock_item_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_stock_item_batches(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    stock_item_id: Option<String>,
) -> Result<Vec<crate::inventory::batches::StockItemBatch>, OpenTallyError> {
    exec_list_stock_item_batches(&state, company_id, stock_item_id).await
}

pub async fn exec_create_stock_item_batch(
    state: &AppState,
    company_id: Option<String>,
    req: crate::inventory::batches::StockItemBatchRequest,
) -> Result<crate::inventory::batches::StockItemBatch, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::inventory::batches::BatchService::create_batch(state.db.pool(), &cid, req).await
}

#[tauri::command]
pub async fn create_stock_item_batch(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    req: crate::inventory::batches::StockItemBatchRequest,
) -> Result<crate::inventory::batches::StockItemBatch, OpenTallyError> {
    exec_create_stock_item_batch(&state, company_id, req).await
}

pub async fn exec_get_batch_expiry_report(
    state: &AppState,
    company_id: Option<String>,
    stock_item_id: Option<String>,
    as_of_date: String,
) -> Result<Vec<crate::inventory::batches::BatchExpiryReportRow>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::inventory::batches::BatchService::get_batch_expiry_report(
        state.db.pool(),
        &cid,
        stock_item_id.as_deref(),
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_batch_expiry_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    stock_item_id: Option<String>,
    as_of_date: String,
) -> Result<Vec<crate::inventory::batches::BatchExpiryReportRow>, OpenTallyError> {
    exec_get_batch_expiry_report(&state, company_id, stock_item_id, as_of_date).await
}

pub async fn exec_post_stock_journal_voucher(
    state: &AppState,
    company_id: Option<String>,
    req: crate::inventory::stock_journal::StockJournalRequest,
) -> Result<crate::inventory::stock_journal::StockJournalResponse, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    let _actor = state
        .resolve_actor(&cid, crate::security::models::ActionPermission::PostVoucher)
        .await?;
    crate::inventory::stock_journal::StockJournalService::post_stock_journal(
        state.db.pool(),
        &cid,
        req,
    )
    .await
}

#[tauri::command]
pub async fn post_stock_journal_voucher(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    req: crate::inventory::stock_journal::StockJournalRequest,
) -> Result<crate::inventory::stock_journal::StockJournalResponse, OpenTallyError> {
    exec_post_stock_journal_voucher(&state, company_id, req).await
}

pub async fn exec_list_stock_journal_vouchers(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Vec<crate::inventory::stock_journal::StockJournalResponse>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::inventory::stock_journal::StockJournalService::list_stock_journals(state.db.pool(), &cid)
        .await
}

#[tauri::command]
pub async fn list_stock_journal_vouchers(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::inventory::stock_journal::StockJournalResponse>, OpenTallyError> {
    exec_list_stock_journal_vouchers(&state, company_id).await
}

pub async fn exec_list_cost_categories(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Vec<crate::accounting::cost_centres::CostCategory>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::list_cost_categories(state.db.pool(), &cid)
        .await
}

#[tauri::command]
pub async fn list_cost_categories(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::accounting::cost_centres::CostCategory>, OpenTallyError> {
    exec_list_cost_categories(&state, company_id).await
}

pub async fn exec_create_cost_category(
    state: &AppState,
    company_id: Option<String>,
    req: crate::accounting::cost_centres::CostCategoryRequest,
) -> Result<crate::accounting::cost_centres::CostCategory, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::create_cost_category(
        state.db.pool(),
        &cid,
        req,
    )
    .await
}

#[tauri::command]
pub async fn create_cost_category(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    req: crate::accounting::cost_centres::CostCategoryRequest,
) -> Result<crate::accounting::cost_centres::CostCategory, OpenTallyError> {
    exec_create_cost_category(&state, company_id, req).await
}

pub async fn exec_list_cost_centres(
    state: &AppState,
    company_id: Option<String>,
    category_id: Option<String>,
) -> Result<Vec<crate::accounting::cost_centres::CostCentre>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::list_cost_centres(
        state.db.pool(),
        &cid,
        category_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_cost_centres(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    category_id: Option<String>,
) -> Result<Vec<crate::accounting::cost_centres::CostCentre>, OpenTallyError> {
    exec_list_cost_centres(&state, company_id, category_id).await
}

pub async fn exec_create_cost_centre(
    state: &AppState,
    company_id: Option<String>,
    req: crate::accounting::cost_centres::CostCentreRequest,
) -> Result<crate::accounting::cost_centres::CostCentre, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::create_cost_centre(
        state.db.pool(),
        &cid,
        req,
    )
    .await
}

#[tauri::command]
pub async fn create_cost_centre(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    req: crate::accounting::cost_centres::CostCentreRequest,
) -> Result<crate::accounting::cost_centres::CostCentre, OpenTallyError> {
    exec_create_cost_centre(&state, company_id, req).await
}

pub async fn exec_get_voucher_cost_allocations(
    state: &AppState,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<Vec<crate::accounting::cost_centres::VoucherCostAllocationItem>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::get_voucher_cost_allocations(
        state.db.pool(),
        &cid,
        &voucher_id,
    )
    .await
}

#[tauri::command]
pub async fn get_voucher_cost_allocations(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
) -> Result<Vec<crate::accounting::cost_centres::VoucherCostAllocationItem>, OpenTallyError> {
    exec_get_voucher_cost_allocations(&state, company_id, voucher_id).await
}

pub async fn exec_get_cost_centre_breakup_report(
    state: &AppState,
    company_id: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
    category_id: Option<String>,
    cost_centre_id: Option<String>,
) -> Result<crate::accounting::cost_centres::CostCentreBreakupReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::cost_centres::CostCentreService::get_cost_centre_breakup_report(
        state.db.pool(),
        &cid,
        from_date.as_deref(),
        to_date.as_deref(),
        category_id.as_deref(),
        cost_centre_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_cost_centre_breakup_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
    category_id: Option<String>,
    cost_centre_id: Option<String>,
) -> Result<crate::accounting::cost_centres::CostCentreBreakupReport, OpenTallyError> {
    exec_get_cost_centre_breakup_report(
        &state,
        company_id,
        from_date,
        to_date,
        category_id,
        cost_centre_id,
    )
    .await
}

// =========================================================================
// Milestone 12: Ratio Analysis, Budgets, Year-End Closing & MCA Audit Trail
// =========================================================================

pub async fn exec_get_ratio_analysis(
    state: &AppState,
    company_id: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<crate::reports::ratios::RatioAnalysisReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    let (fdate, tdate) = match (from_date, to_date) {
        (Some(f), Some(t)) => (f, t),
        (Some(f), None) => {
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            (f, now)
        }
        (None, Some(t)) => {
            let fy_begins: String =
                sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                    .bind(&cid)
                    .fetch_one(state.db.pool())
                    .await?;
            (fy_begins, t)
        }
        (None, None) => {
            let fy_begins: String =
                sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = ?")
                    .bind(&cid)
                    .fetch_one(state.db.pool())
                    .await?;
            let now = chrono::Local::now().format("%Y-%m-%d").to_string();
            (fy_begins, now)
        }
    };
    crate::reports::ratios::RatioAnalysisService::generate(state.db.pool(), &cid, &fdate, &tdate)
        .await
}

#[tauri::command]
pub async fn get_ratio_analysis(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    from_date: Option<String>,
    to_date: Option<String>,
) -> Result<crate::reports::ratios::RatioAnalysisReport, OpenTallyError> {
    exec_get_ratio_analysis(&state, company_id, from_date, to_date).await
}

pub async fn exec_list_budgets(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Vec<crate::accounting::budgets::Budget>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::budgets::BudgetService::list_budgets(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn list_budgets(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::accounting::budgets::Budget>, OpenTallyError> {
    exec_list_budgets(&state, company_id).await
}

pub async fn exec_create_budget(
    state: &AppState,
    company_id: Option<String>,
    req: crate::accounting::budgets::BudgetRequest,
) -> Result<crate::accounting::budgets::BudgetDetails, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::budgets::BudgetService::create_budget(state.db.pool(), &cid, req).await
}

#[tauri::command]
pub async fn create_budget(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    req: crate::accounting::budgets::BudgetRequest,
) -> Result<crate::accounting::budgets::BudgetDetails, OpenTallyError> {
    exec_create_budget(&state, company_id, req).await
}

pub async fn exec_get_budget(
    state: &AppState,
    company_id: Option<String>,
    budget_id: String,
) -> Result<crate::accounting::budgets::BudgetDetails, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::budgets::BudgetService::get_budget(state.db.pool(), &cid, &budget_id).await
}

#[tauri::command]
pub async fn get_budget(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    budget_id: String,
) -> Result<crate::accounting::budgets::BudgetDetails, OpenTallyError> {
    exec_get_budget(&state, company_id, budget_id).await
}

pub async fn exec_delete_budget(
    state: &AppState,
    company_id: Option<String>,
    budget_id: String,
) -> Result<(), OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::budgets::BudgetService::delete_budget(state.db.pool(), &cid, &budget_id)
        .await
}

#[tauri::command]
pub async fn delete_budget(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    budget_id: String,
) -> Result<(), OpenTallyError> {
    exec_delete_budget(&state, company_id, budget_id).await
}

pub async fn exec_get_budget_variance_report(
    state: &AppState,
    company_id: Option<String>,
    budget_id: String,
    as_of_date: String,
) -> Result<crate::accounting::budgets::BudgetVarianceReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::accounting::budgets::BudgetService::get_budget_variance_report(
        state.db.pool(),
        &cid,
        &budget_id,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn get_budget_variance_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    budget_id: String,
    as_of_date: String,
) -> Result<crate::accounting::budgets::BudgetVarianceReport, OpenTallyError> {
    exec_get_budget_variance_report(&state, company_id, budget_id, as_of_date).await
}

pub async fn exec_preview_year_end_closing(
    state: &AppState,
    company_id: Option<String>,
    period_end: String,
) -> Result<crate::company::closing::YearEndClosingPreview, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::company::closing::FinancialYearClosingService::preview_year_end_closing(
        state.db.pool(),
        &cid,
        &period_end,
    )
    .await
}

#[tauri::command]
pub async fn preview_year_end_closing(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    period_end: String,
) -> Result<crate::company::closing::YearEndClosingPreview, OpenTallyError> {
    exec_preview_year_end_closing(&state, company_id, period_end).await
}

pub async fn exec_execute_year_end_closing(
    state: &AppState,
    company_id: Option<String>,
    period_end: String,
    closed_by: Option<String>,
) -> Result<crate::company::closing::FinancialYearClosingRecord, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::YearEndClosing,
        )
        .await?;
    let effective_closed_by = closed_by.unwrap_or(actor);
    crate::company::closing::FinancialYearClosingService::execute_financial_year_closing(
        state.db.pool(),
        &cid,
        &period_end,
        Some(&effective_closed_by),
    )
    .await
}

#[tauri::command]
pub async fn execute_year_end_closing(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    period_end: String,
    closed_by: Option<String>,
) -> Result<crate::company::closing::FinancialYearClosingRecord, OpenTallyError> {
    exec_execute_year_end_closing(&state, company_id, period_end, closed_by).await
}

pub async fn exec_list_closed_years(
    state: &AppState,
    company_id: Option<String>,
) -> Result<Vec<crate::company::closing::FinancialYearClosingRecord>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::company::closing::FinancialYearClosingService::list_closed_years(state.db.pool(), &cid)
        .await
}

#[tauri::command]
pub async fn list_closed_years(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::company::closing::FinancialYearClosingRecord>, OpenTallyError> {
    exec_list_closed_years(&state, company_id).await
}

pub async fn exec_verify_audit_trail_integrity(
    state: &AppState,
    company_id: Option<String>,
) -> Result<crate::audit::certification::AuditCertificationReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
        state.db.pool(),
        &cid,
    )
    .await
}

#[tauri::command]
pub async fn verify_audit_trail_integrity(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<crate::audit::certification::AuditCertificationReport, OpenTallyError> {
    exec_verify_audit_trail_integrity(&state, company_id).await
}

pub async fn exec_export_audit_certification_csv(
    state: &AppState,
    company_id: Option<String>,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(state).await?,
    };
    let report =
        crate::audit::certification::AuditCertificationService::verify_audit_trail_integrity(
            state.db.pool(),
            &cid,
        )
        .await?;
    Ok(crate::audit::certification::AuditCertificationService::export_certification_csv(&report))
}

#[tauri::command]
pub async fn export_audit_certification_csv(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<String, OpenTallyError> {
    exec_export_audit_certification_csv(&state, company_id).await
}

// =========================================================================
// MILESTONE 13: ORDER PROCESSING IPC COMMANDS
// =========================================================================

async fn resolve_orders_company_id(
    state: &AppState,
    company_id: Option<String>,
) -> Result<String, OpenTallyError> {
    match company_id {
        Some(c) => {
            let comp = crate::company::CompanyService::get_company(state.db.pool(), &c).await?;
            if comp.status != crate::company::CompanyStatus::Active {
                return Err(OpenTallyError::Validation(format!(
                    "Company '{}' is not active",
                    comp.name
                )));
            }
            Ok(c)
        }
        None => require_active_company(state).await,
    }
}

pub async fn exec_list_purchase_orders(
    state: &AppState,
    company_id: Option<String>,
    status_filter: Option<String>,
) -> Result<Vec<crate::orders::PurchaseOrder>, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::list_purchase_orders(
        state.db.pool(),
        &cid,
        status_filter.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn list_purchase_orders(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    status_filter: Option<String>,
) -> Result<Vec<crate::orders::PurchaseOrder>, OpenTallyError> {
    exec_list_purchase_orders(&state, company_id, status_filter).await
}

pub async fn exec_create_purchase_order(
    state: &AppState,
    company_id: Option<String>,
    input: crate::orders::CreatePurchaseOrderInput,
) -> Result<crate::orders::PurchaseOrderWithLines, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::create_purchase_order(
        state.db.pool(),
        &cid,
        input,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn create_purchase_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::orders::CreatePurchaseOrderInput,
) -> Result<crate::orders::PurchaseOrderWithLines, OpenTallyError> {
    exec_create_purchase_order(&state, company_id, input).await
}

pub async fn exec_get_purchase_order(
    state: &AppState,
    company_id: Option<String>,
    order_id: String,
) -> Result<crate::orders::PurchaseOrderWithLines, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::get_purchase_order(state.db.pool(), &cid, &order_id).await
}

#[tauri::command]
pub async fn get_purchase_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    order_id: String,
) -> Result<crate::orders::PurchaseOrderWithLines, OpenTallyError> {
    exec_get_purchase_order(&state, company_id, order_id).await
}

pub async fn exec_cancel_purchase_order(
    state: &AppState,
    company_id: Option<String>,
    order_id: String,
    reason: String,
) -> Result<(), OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::cancel_purchase_order(
        state.db.pool(),
        &cid,
        &order_id,
        &reason,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn cancel_purchase_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    order_id: String,
    reason: String,
) -> Result<(), OpenTallyError> {
    exec_cancel_purchase_order(&state, company_id, order_id, reason).await
}

pub async fn exec_convert_purchase_order_to_invoice(
    state: &AppState,
    company_id: Option<String>,
    input: crate::orders::ConvertPurchaseOrderInput,
) -> Result<crate::orders::ConvertOrderResult, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::convert_purchase_order_to_invoice(
        state.db.pool(),
        &cid,
        input,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn convert_purchase_order_to_invoice(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::orders::ConvertPurchaseOrderInput,
) -> Result<crate::orders::ConvertOrderResult, OpenTallyError> {
    exec_convert_purchase_order_to_invoice(&state, company_id, input).await
}

pub async fn exec_list_sales_orders(
    state: &AppState,
    company_id: Option<String>,
    status_filter: Option<String>,
) -> Result<Vec<crate::orders::SalesOrder>, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::list_sales_orders(state.db.pool(), &cid, status_filter.as_deref())
        .await
}

#[tauri::command]
pub async fn list_sales_orders(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    status_filter: Option<String>,
) -> Result<Vec<crate::orders::SalesOrder>, OpenTallyError> {
    exec_list_sales_orders(&state, company_id, status_filter).await
}

pub async fn exec_create_sales_order(
    state: &AppState,
    company_id: Option<String>,
    input: crate::orders::CreateSalesOrderInput,
) -> Result<crate::orders::SalesOrderWithLines, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::create_sales_order(
        state.db.pool(),
        &cid,
        input,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn create_sales_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::orders::CreateSalesOrderInput,
) -> Result<crate::orders::SalesOrderWithLines, OpenTallyError> {
    exec_create_sales_order(&state, company_id, input).await
}

pub async fn exec_get_sales_order(
    state: &AppState,
    company_id: Option<String>,
    order_id: String,
) -> Result<crate::orders::SalesOrderWithLines, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::get_sales_order(state.db.pool(), &cid, &order_id).await
}

#[tauri::command]
pub async fn get_sales_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    order_id: String,
) -> Result<crate::orders::SalesOrderWithLines, OpenTallyError> {
    exec_get_sales_order(&state, company_id, order_id).await
}

pub async fn exec_cancel_sales_order(
    state: &AppState,
    company_id: Option<String>,
    order_id: String,
    reason: String,
) -> Result<(), OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::cancel_sales_order(
        state.db.pool(),
        &cid,
        &order_id,
        &reason,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn cancel_sales_order(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    order_id: String,
    reason: String,
) -> Result<(), OpenTallyError> {
    exec_cancel_sales_order(&state, company_id, order_id, reason).await
}

pub async fn exec_convert_sales_order_to_invoice(
    state: &AppState,
    company_id: Option<String>,
    input: crate::orders::ConvertSalesOrderInput,
) -> Result<crate::orders::ConvertOrderResult, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::convert_sales_order_to_invoice(
        state.db.pool(),
        &cid,
        input,
        Some("operator_ipc"),
    )
    .await
}

#[tauri::command]
pub async fn convert_sales_order_to_invoice(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::orders::ConvertSalesOrderInput,
) -> Result<crate::orders::ConvertOrderResult, OpenTallyError> {
    exec_convert_sales_order_to_invoice(&state, company_id, input).await
}

pub async fn exec_get_pending_purchase_orders(
    state: &AppState,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::orders::PendingOrdersReport, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::get_pending_purchase_orders(state.db.pool(), &cid, &as_of_date)
        .await
}

#[tauri::command]
pub async fn get_pending_purchase_orders(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::orders::PendingOrdersReport, OpenTallyError> {
    exec_get_pending_purchase_orders(&state, company_id, as_of_date).await
}

pub async fn exec_get_pending_sales_orders(
    state: &AppState,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::orders::PendingOrdersReport, OpenTallyError> {
    let cid = resolve_orders_company_id(state, company_id).await?;
    crate::orders::OrdersService::get_pending_sales_orders(state.db.pool(), &cid, &as_of_date).await
}

#[tauri::command]
pub async fn get_pending_sales_orders(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::orders::PendingOrdersReport, OpenTallyError> {
    exec_get_pending_sales_orders(&state, company_id, as_of_date).await
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
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    let effective_actor = req.actor_id.as_deref().unwrap_or(&actor);
    crate::vouchers::VoucherPostingService::post_voucher_with_allocations(
        state.db.pool(),
        &company_id,
        &req.voucher_id,
        Some(effective_actor),
        req.bank_allocations.as_deref(),
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
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::CancelOrReverseVoucher,
        )
        .await?;
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
    let _actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::CancelOrReverseVoucher,
        )
        .await?;
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

// ---------------------------------------------------------------------------
// Sprint 4 Inventory master-data IPC commands
// ---------------------------------------------------------------------------
pub async fn exec_list_stock_groups(
    state: &AppState,
    search: Option<String>,
) -> Result<Vec<crate::inventory::StockGroup>, OpenTallyError> {
    let c = require_active_company(state).await?;
    crate::inventory::InventoryService::list_stock_groups(state.db.pool(), &c, search.as_deref())
        .await
}
#[tauri::command]
pub async fn list_stock_groups(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::inventory::StockGroup>, OpenTallyError> {
    exec_list_stock_groups(&state, search).await
}
#[tauri::command]
pub async fn create_stock_group(
    state: tauri::State<'_, AppState>,
    req: crate::inventory::StockGroupRequest,
) -> Result<crate::inventory::StockGroup, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::create_stock_group(state.db.pool(), &c, req).await
}
#[tauri::command]
pub async fn update_stock_group(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::inventory::StockGroupRequest,
) -> Result<crate::inventory::StockGroup, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::update_stock_group(state.db.pool(), &c, &id, req).await
}
#[tauri::command]
pub async fn set_stock_group_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::inventory::StockGroup, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::set_stock_group_active(state.db.pool(), &c, &id, active)
        .await
}
#[tauri::command]
pub async fn delete_stock_group(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::delete_stock_group(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn list_stock_categories(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::inventory::StockCategory>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::list_stock_categories(
        state.db.pool(),
        &c,
        search.as_deref(),
    )
    .await
}
#[tauri::command]
pub async fn create_stock_category(
    state: tauri::State<'_, AppState>,
    req: crate::inventory::StockCategoryRequest,
) -> Result<crate::inventory::StockCategory, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::create_stock_category(state.db.pool(), &c, req).await
}
#[tauri::command]
pub async fn update_stock_category(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::inventory::StockCategoryRequest,
) -> Result<crate::inventory::StockCategory, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::update_stock_category(state.db.pool(), &c, &id, req).await
}
#[tauri::command]
pub async fn set_stock_category_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::inventory::StockCategory, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::set_stock_category_active(state.db.pool(), &c, &id, active)
        .await
}
#[tauri::command]
pub async fn delete_stock_category(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::delete_stock_category(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn list_units(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::inventory::Unit>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::list_units(state.db.pool(), &c, search.as_deref()).await
}
#[tauri::command]
pub async fn create_unit(
    state: tauri::State<'_, AppState>,
    req: crate::inventory::UnitRequest,
) -> Result<crate::inventory::Unit, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::create_unit(state.db.pool(), &c, req).await
}
#[tauri::command]
pub async fn update_unit(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::inventory::UnitRequest,
) -> Result<crate::inventory::Unit, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::update_unit(state.db.pool(), &c, &id, req).await
}
#[tauri::command]
pub async fn set_unit_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::inventory::Unit, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::set_unit_active(state.db.pool(), &c, &id, active).await
}
#[tauri::command]
pub async fn delete_unit(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::delete_unit(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn list_stock_items(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::inventory::StockItem>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::list_stock_items(state.db.pool(), &c, search.as_deref())
        .await
}
#[tauri::command]
pub async fn create_stock_item(
    state: tauri::State<'_, AppState>,
    req: crate::inventory::StockItemRequest,
) -> Result<crate::inventory::StockItem, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::create_stock_item(state.db.pool(), &c, req).await
}
#[tauri::command]
pub async fn update_stock_item(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::inventory::StockItemRequest,
) -> Result<crate::inventory::StockItem, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::update_stock_item(state.db.pool(), &c, &id, req).await
}
#[tauri::command]
pub async fn set_stock_item_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::inventory::StockItem, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::set_stock_item_active(state.db.pool(), &c, &id, active)
        .await
}
#[tauri::command]
pub async fn delete_stock_item(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::delete_stock_item(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn list_godowns(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
) -> Result<Vec<crate::inventory::Godown>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::list_godowns(state.db.pool(), &c, search.as_deref()).await
}
#[tauri::command]
pub async fn create_godown(
    state: tauri::State<'_, AppState>,
    req: crate::inventory::GodownRequest,
) -> Result<crate::inventory::Godown, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::create_godown(state.db.pool(), &c, req).await
}
#[tauri::command]
pub async fn update_godown(
    state: tauri::State<'_, AppState>,
    id: String,
    req: crate::inventory::GodownRequest,
) -> Result<crate::inventory::Godown, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::update_godown(state.db.pool(), &c, &id, req).await
}
#[tauri::command]
pub async fn set_godown_active(
    state: tauri::State<'_, AppState>,
    id: String,
    active: bool,
) -> Result<crate::inventory::Godown, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::set_godown_active(state.db.pool(), &c, &id, active).await
}
#[tauri::command]
pub async fn delete_godown(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryService::delete_godown(state.db.pool(), &c, &id).await
}

#[tauri::command]
pub async fn create_stock_voucher(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::StockVoucherRequest,
) -> Result<crate::inventory::StockVoucher, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::create_draft(state.db.pool(), &c, request).await
}
#[tauri::command]
pub async fn get_stock_voucher(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<
    (
        crate::inventory::StockVoucher,
        Vec<crate::inventory::StockVoucherLine>,
    ),
    OpenTallyError,
> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::get(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn update_stock_voucher(
    state: tauri::State<'_, AppState>,
    id: String,
    request: crate::inventory::StockVoucherRequest,
) -> Result<crate::inventory::StockVoucher, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::update_draft(state.db.pool(), &c, &id, request).await
}
#[tauri::command]
pub async fn list_stock_vouchers(
    state: tauri::State<'_, AppState>,
    filter: crate::inventory::StockVoucherListFilter,
) -> Result<Vec<crate::inventory::StockVoucher>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::list(state.db.pool(), &c, filter).await
}
#[tauri::command]
pub async fn post_stock_voucher(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::inventory::StockVoucher, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::post(state.db.pool(), &c, &id).await
}
#[tauri::command]
pub async fn cancel_stock_voucher(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::inventory::StockVoucher, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::cancel_or_reverse(state.db.pool(), &c, &id, false).await
}
#[tauri::command]
pub async fn reverse_stock_voucher(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<crate::inventory::StockVoucher, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockVoucherService::cancel_or_reverse(state.db.pool(), &c, &id, true).await
}
#[tauri::command]
pub async fn get_stock_balance(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::StockBalanceRequest,
    overall: bool,
) -> Result<Vec<crate::inventory::StockBalanceRow>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockBalanceService::query(state.db.pool(), &c, request, overall).await
}
#[tauri::command]
pub async fn get_stock_movement_history(
    state: tauri::State<'_, AppState>,
    stock_item_id: Option<String>,
    godown_id: Option<String>,
) -> Result<Vec<crate::inventory::StockMovementHistory>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::StockBalanceService::history(state.db.pool(), &c, stock_item_id, godown_id)
        .await
}

#[tauri::command]
pub async fn get_inventory_valuation_config(
    state: tauri::State<'_, AppState>,
) -> Result<crate::inventory::ValuationConfig, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::ValuationService::get_config(state.db.pool(), &c).await
}
#[tauri::command]
pub async fn set_inventory_valuation_method(
    state: tauri::State<'_, AppState>,
    method: String,
) -> Result<crate::inventory::ValuationConfig, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::ValuationService::set_config(state.db.pool(), &c, &method).await
}
#[tauri::command]
pub async fn get_stock_valuation(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::ValuationRequest,
) -> Result<Vec<crate::inventory::ValuationRow>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::ValuationService::summary(state.db.pool(), &c, request).await
}
#[tauri::command]
pub async fn get_fifo_layers(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::ValuationRequest,
) -> Result<Vec<crate::inventory::FifoLayer>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::ValuationService::fifo_layers(state.db.pool(), &c, request).await
}
#[tauri::command]
pub async fn get_valuation_history(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::ValuationRequest,
) -> Result<Vec<crate::inventory::ValuationHistoryRow>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::ValuationService::history(state.db.pool(), &c, request).await
}

#[tauri::command]
pub async fn get_stock_summary_report(
    state: tauri::State<'_, AppState>,
    filter: crate::inventory::InventoryReportFilter,
) -> Result<crate::inventory::StockSummaryReport, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryReportsService::stock_summary(state.db.pool(), &c, filter).await
}
#[tauri::command]
pub async fn get_stock_movement_register(
    state: tauri::State<'_, AppState>,
    filter: crate::inventory::InventoryReportFilter,
) -> Result<crate::inventory::StockRegisterReport, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryReportsService::stock_register(state.db.pool(), &c, filter).await
}
#[tauri::command]
pub async fn get_stock_ledger_report(
    state: tauri::State<'_, AppState>,
    filter: crate::inventory::InventoryReportFilter,
) -> Result<crate::inventory::StockLedgerReport, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryReportsService::stock_ledger(state.db.pool(), &c, filter).await
}
#[tauri::command]
pub async fn get_inventory_reconciliation(
    state: tauri::State<'_, AppState>,
    filter: crate::inventory::InventoryReportFilter,
) -> Result<Vec<crate::inventory::ReconciliationFinding>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryReportsService::reconciliation(state.db.pool(), &c, filter).await
}

#[tauri::command]
pub async fn get_inventory_account_mapping(
    state: tauri::State<'_, AppState>,
) -> Result<Option<crate::inventory::InventoryAccountMapping>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::get_mapping(state.db.pool(), &c).await
}
#[tauri::command]
pub async fn configure_inventory_account_mapping(
    state: tauri::State<'_, AppState>,
    request: crate::inventory::InventoryAccountMappingRequest,
) -> Result<crate::inventory::InventoryAccountMapping, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::configure_mapping(state.db.pool(), &c, request)
        .await
}
#[tauri::command]
pub async fn preview_inventory_accounting(
    state: tauri::State<'_, AppState>,
    stock_voucher_id: String,
) -> Result<crate::inventory::InventoryAccountingPreview, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::preview(state.db.pool(), &c, &stock_voucher_id)
        .await
}
#[tauri::command]
pub async fn integrate_inventory_accounting(
    state: tauri::State<'_, AppState>,
    stock_voucher_id: String,
) -> Result<crate::inventory::InventoryAccountingIntegration, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::integrate(state.db.pool(), &c, &stock_voucher_id)
        .await
}
#[tauri::command]
pub async fn get_inventory_accounting_status(
    state: tauri::State<'_, AppState>,
    stock_voucher_id: String,
) -> Result<Option<crate::inventory::InventoryAccountingIntegration>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::status(state.db.pool(), &c, &stock_voucher_id)
        .await
}
#[tauri::command]
pub async fn list_inventory_accounting_history(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::inventory::InventoryAccountingIntegration>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryAccountingService::history(state.db.pool(), &c).await
}
#[tauri::command]
pub async fn get_inventory_valuation_attribution(
    state: tauri::State<'_, AppState>,
    stock_voucher_id: String,
) -> Result<Vec<crate::inventory::InventoryValuationAttribution>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryValuationAttributionService::for_voucher(
        state.db.pool(),
        &c,
        &stock_voucher_id,
    )
    .await
}
#[tauri::command]
pub async fn get_inventory_valuation_attribution_details(
    state: tauri::State<'_, AppState>,
    stock_voucher_id: String,
) -> Result<Vec<crate::inventory::InventoryValuationAttributionDetails>, OpenTallyError> {
    let c = require_active_company(&state).await?;
    crate::inventory::InventoryValuationAttributionService::details_for_voucher(
        state.db.pool(),
        &c,
        &stock_voucher_id,
    )
    .await
}

// ---------------------------------------------------------------------------
// Milestone 14: Security Control & Role-Based Access Control (RBAC) Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_company_security_status(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<crate::security::models::SecurityStatusDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let active_user = {
        let guard = state
            .active_user_sessions
            .read()
            .map_err(|e| OpenTallyError::Internal(format!("Session lock poisoned: {}", e)))?;
        guard.get(&cid).cloned()
    };
    crate::security::service::SecurityService::get_security_status(
        state.db.pool(),
        &cid,
        active_user,
    )
    .await
}

#[tauri::command]
pub async fn enable_company_security(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::EnableSecurityInput,
) -> Result<crate::security::models::AuthSessionInfo, OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await
        .unwrap_or_else(|_| "system".to_string());

    let session = crate::security::service::SecurityService::enable_company_security(
        state.db.pool(),
        input.clone(),
        &actor,
    )
    .await?;

    {
        let mut guard = state
            .active_user_sessions
            .write()
            .map_err(|e| OpenTallyError::Internal(format!("Session lock poisoned: {}", e)))?;
        guard.insert(input.company_id, session.clone());
    }

    Ok(session)
}

#[tauri::command]
pub async fn authenticate_company_user(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::LoginInput,
) -> Result<crate::security::models::AuthSessionInfo, OpenTallyError> {
    let session =
        crate::security::service::SecurityService::authenticate(state.db.pool(), input.clone())
            .await?;

    {
        let mut guard = state
            .active_user_sessions
            .write()
            .map_err(|e| OpenTallyError::Internal(format!("Session lock poisoned: {}", e)))?;
        guard.insert(input.company_id, session.clone());
    }

    Ok(session)
}

#[tauri::command]
pub async fn logout_company_session(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<(), OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let mut guard = state
        .active_user_sessions
        .write()
        .map_err(|e| OpenTallyError::Internal(format!("Session lock poisoned: {}", e)))?;
    guard.remove(&cid);
    Ok(())
}

#[tauri::command]
pub async fn list_company_users(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::security::models::CompanyUser>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(&cid, crate::security::models::ActionPermission::ManageUsers)
        .await?;
    crate::security::service::SecurityService::list_users(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn create_company_user(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::CreateUserInput,
) -> Result<crate::security::models::CompanyUser, OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await?;
    crate::security::service::SecurityService::create_user(state.db.pool(), input, &actor).await
}

#[tauri::command]
pub async fn update_company_user_role(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::UpdateUserRoleInput,
) -> Result<crate::security::models::CompanyUser, OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await?;
    let updated_user = crate::security::service::SecurityService::update_user_role(
        state.db.pool(),
        input.clone(),
        &actor,
    )
    .await?;
    state.evict_user_session_by_id(&input.company_id, &input.user_id);
    Ok(updated_user)
}

#[tauri::command]
pub async fn update_company_user_password(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::UpdateUserPasswordInput,
) -> Result<(), OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await?;
    crate::security::service::SecurityService::update_user_password(state.db.pool(), input, &actor)
        .await
}

#[tauri::command]
pub async fn deactivate_company_user(
    state: tauri::State<'_, AppState>,
    input: crate::security::models::DeactivateUserInput,
) -> Result<(), OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await?;
    crate::security::service::SecurityService::deactivate_user(
        state.db.pool(),
        input.clone(),
        &actor,
    )
    .await?;
    state.evict_user_session_by_id(&input.company_id, &input.user_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Milestone 14: Manufacturing Bill of Materials (BOM) & Production Journal Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_boms(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    finished_stock_item_id: Option<String>,
) -> Result<Vec<crate::inventory::bom::BomDetail>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::inventory::bom::BomService::list_boms(
        state.db.pool(),
        &cid,
        finished_stock_item_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn get_bom_detail(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    bom_id: String,
) -> Result<crate::inventory::bom::BomDetail, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::inventory::bom::BomService::get_bom_detail(state.db.pool(), &cid, &bom_id).await
}

#[tauri::command]
pub async fn create_bom(
    state: tauri::State<'_, AppState>,
    input: crate::inventory::bom::CreateBomInput,
) -> Result<crate::inventory::bom::BomDetail, OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ManageBom,
        )
        .await?;
    crate::inventory::bom::BomService::create_bom(state.db.pool(), input, &actor).await
}

#[tauri::command]
pub async fn execute_manufacturing_journal(
    state: tauri::State<'_, AppState>,
    input: crate::inventory::bom::ExecuteManufacturingJournalInput,
) -> Result<crate::inventory::bom::ManufacturingJournalResult, OpenTallyError> {
    let actor = state
        .resolve_actor(
            &input.company_id,
            crate::security::models::ActionPermission::ExecuteManufacturingJournal,
        )
        .await?;
    crate::inventory::bom::BomService::execute_manufacturing_journal(state.db.pool(), input, &actor)
        .await
}

// ---------------------------------------------------------------------------
// Milestone 15: Indian Payroll Engine, Statutory PF/ESI/PT & Payslip PDF
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_employees(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    include_inactive: Option<bool>,
) -> Result<Vec<crate::payroll::Employee>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::payroll::PayrollService::list_employees(
        state.db.pool(),
        &cid,
        include_inactive.unwrap_or(false),
    )
    .await
}

#[tauri::command]
pub async fn create_employee(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::payroll::CreateEmployeeInput,
) -> Result<crate::payroll::Employee, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManagePayroll,
        )
        .await?;
    crate::payroll::PayrollService::create_employee(state.db.pool(), &cid, input, &actor).await
}

#[tauri::command]
pub async fn deactivate_employee(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    employee_id: String,
) -> Result<(), OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManagePayroll,
        )
        .await?;
    crate::payroll::PayrollService::deactivate_employee(state.db.pool(), &cid, &employee_id, &actor)
        .await
}

#[tauri::command]
pub async fn preview_payroll_run(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::payroll::PayrollRunInput,
) -> Result<crate::payroll::PayrollPreviewResult, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::payroll::PayrollService::preview_payroll_run(state.db.pool(), &cid, &input).await
}

#[tauri::command]
pub async fn execute_payroll_run(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::payroll::PayrollRunInput,
) -> Result<crate::payroll::PayrollRunResult, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManagePayroll,
        )
        .await?;
    crate::payroll::PayrollService::execute_payroll_run(state.db.pool(), &cid, input, &actor).await
}

#[tauri::command]
pub async fn list_payroll_runs(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::payroll::PayrollRun>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::payroll::PayrollService::list_payroll_runs(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn get_payroll_statutory_summary(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    pay_period: String,
) -> Result<crate::payroll::PayrollStatutorySummary, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::reports::PayrollReportService::get_payroll_statutory_summary(
        state.db.pool(),
        &cid,
        &pay_period,
    )
    .await
}

#[tauri::command]
pub async fn export_payslip_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    payslip_id: String,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::pdf::payslip::export_payslip_pdf(state.db.pool(), &cid, &payslip_id).await
}

#[tauri::command]
pub async fn export_payroll_csv(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    pay_period: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::reports::PayrollReportService::export_payroll_csv(state.db.pool(), &cid, &pay_period)
        .await
}

// =========================================================================
// Milestone 16: Multi-Currency & Bank Cheque Printing Commands
// =========================================================================

#[tauri::command]
pub async fn seed_company_currency_defaults(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<crate::currency::CurrencySeedSummary, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManageMultiCurrency,
        )
        .await?;
    let mut tx = state.db.pool().begin().await?;
    let summary =
        crate::currency::CurrencyService::seed_company_currency_defaults_tx(&mut tx, &cid, &actor)
            .await?;
    tx.commit().await?;
    Ok(summary)
}

#[tauri::command]
pub async fn list_currencies(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::currency::Currency>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::currency::CurrencyService::list_currencies(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn create_currency(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::currency::CreateCurrencyInput,
) -> Result<crate::currency::Currency, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManageMultiCurrency,
        )
        .await?;
    crate::currency::CurrencyService::create_currency(state.db.pool(), &cid, input, &actor).await
}

#[tauri::command]
pub async fn set_exchange_rate(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::currency::SetExchangeRateInput,
) -> Result<crate::currency::ExchangeRate, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManageMultiCurrency,
        )
        .await?;
    crate::currency::CurrencyService::set_exchange_rate(state.db.pool(), &cid, input, &actor).await
}

#[tauri::command]
pub async fn list_exchange_rates(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    currency_code: Option<String>,
) -> Result<Vec<crate::currency::ExchangeRate>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::currency::CurrencyService::list_exchange_rates(
        state.db.pool(),
        &cid,
        currency_code.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn lookup_exchange_rate(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    currency_code: String,
    as_of_date: String,
) -> Result<crate::currency::EffectiveRateDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::currency::CurrencyService::lookup_exchange_rate(
        state.db.pool(),
        &cid,
        &currency_code,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn convert_currency_amount(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    currency_code: String,
    foreign_amount: String,
    as_of_date: String,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let f_amt = rust_decimal::Decimal::from_str_exact(foreign_amount.trim())
        .map_err(|_| OpenTallyError::Validation("Invalid foreign amount string".to_string()))?;
    let base = crate::currency::CurrencyService::convert_currency_amount(
        state.db.pool(),
        &cid,
        &currency_code,
        f_amt,
        &as_of_date,
    )
    .await?;
    Ok(base.to_string())
}

#[tauri::command]
pub async fn list_open_foreign_bills(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    party_ledger_id: Option<String>,
) -> Result<Vec<crate::currency::OpenForeignBill>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::currency::CurrencyService::list_open_foreign_bills(
        state.db.pool(),
        &cid,
        party_ledger_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn settle_foreign_bill_with_forex(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::currency::SettleForeignBillInput,
) -> Result<crate::currency::RealizedForexResult, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(&cid, crate::security::models::ActionPermission::PostVoucher)
        .await?;
    crate::currency::CurrencyService::settle_foreign_bill_with_forex(
        state.db.pool(),
        &cid,
        input,
        &actor,
    )
    .await
}

#[tauri::command]
pub async fn get_unrealized_forex_report(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    as_of_date: String,
) -> Result<crate::currency::UnrealizedForexReport, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewFinancialStatements,
        )
        .await?;
    crate::currency::CurrencyService::evaluate_unrealized_forex_revaluation(
        state.db.pool(),
        &cid,
        &as_of_date,
    )
    .await
}

#[tauri::command]
pub async fn list_cheque_templates(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<crate::pdf::ChequeTemplateDto>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::currency::CurrencyService::list_cheque_templates(state.db.pool(), &cid).await
}

#[tauri::command]
pub async fn upsert_cheque_template(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: crate::currency::UpsertChequeTemplateInput,
) -> Result<crate::pdf::ChequeTemplateDto, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let actor = state
        .resolve_actor(
            &cid,
            crate::security::models::ActionPermission::ManageMultiCurrency,
        )
        .await?;
    crate::currency::CurrencyService::upsert_cheque_template(state.db.pool(), &cid, input, &actor)
        .await
}

#[tauri::command]
pub async fn generate_cheque_pdf_bytes(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
    template_id: Option<String>,
    is_account_payee: Option<bool>,
) -> Result<Vec<u8>, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(&cid, crate::security::models::ActionPermission::PrintCheque)
        .await?;
    crate::pdf::generate_cheque_pdf_bytes(
        state.db.pool(),
        &cid,
        &voucher_id,
        template_id.as_deref(),
        is_account_payee,
    )
    .await
}

#[tauri::command]
pub async fn export_cheque_pdf(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    voucher_id: String,
    template_id: Option<String>,
    is_account_payee: Option<bool>,
    output_dir: Option<String>,
) -> Result<String, OpenTallyError> {
    let cid = match company_id {
        Some(c) => c,
        None => require_active_company(&state).await?,
    };
    let _actor = state
        .resolve_actor(&cid, crate::security::models::ActionPermission::PrintCheque)
        .await?;
    crate::pdf::export_cheque_pdf(
        state.db.pool(),
        &cid,
        &voucher_id,
        template_id.as_deref(),
        is_account_payee,
        output_dir.as_deref(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Milestone 17: Multi-User LAN Host / Client Mode Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_lan_server(
    state: tauri::State<'_, AppState>,
    bind_address: Option<String>,
    port: Option<u16>,
) -> Result<crate::lan::models::LanServerStatusDto, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'start_lan_server' command: bind_address={:?}, port={:?}",
        bind_address, port
    );

    // If there is an active company with security enabled, require Admin / ManageLanServer
    let active_cid = state.active_company_id.read().ok().and_then(|g| g.clone());
    if let Some(ref cid) = active_cid {
        let _ = state
            .resolve_actor(
                cid,
                crate::security::models::ActionPermission::ManageLanServer,
            )
            .await?;
    }

    {
        let guard = state
            .lan_server
            .read()
            .map_err(|e| OpenTallyError::Internal(e.to_string()))?;
        if guard.is_some() {
            return Err(OpenTallyError::Validation(
                "LAN Host server is already running".to_string(),
            ));
        }
    }

    let handle =
        crate::lan::start_lan_listener(state.db.pool().clone(), bind_address, port).await?;

    let status = {
        let mut guard = state
            .lan_server
            .write()
            .map_err(|e| OpenTallyError::Internal(e.to_string()))?;
        let status = crate::lan::models::LanServerStatusDto {
            is_running: true,
            bind_address: handle.bind_address.clone(),
            port: handle.port,
            local_ip_addresses: crate::lan::server::detect_local_ips(),
            active_sessions: handle.session_store.list_active(),
        };
        *guard = Some(handle);
        status
    };

    info!(
        "<<< [Tauri IPC] Successfully started LAN Host server on port {}",
        status.port
    );
    Ok(status)
}

#[tauri::command]
pub async fn stop_lan_server(
    state: tauri::State<'_, AppState>,
) -> Result<crate::lan::models::LanServerStatusDto, OpenTallyError> {
    info!(">>> [Tauri IPC] Received 'stop_lan_server' command");

    let mut guard = state
        .lan_server
        .write()
        .map_err(|e| OpenTallyError::Internal(e.to_string()))?;

    if let Some(mut handle) = guard.take() {
        if let Some(tx) = handle.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    let status = crate::lan::models::LanServerStatusDto {
        is_running: false,
        bind_address: "0.0.0.0".to_string(),
        port: 7900,
        local_ip_addresses: crate::lan::server::detect_local_ips(),
        active_sessions: Vec::new(),
    };

    info!("<<< [Tauri IPC] LAN Host server stopped cleanly");
    Ok(status)
}

#[tauri::command]
pub async fn get_lan_server_status(
    state: tauri::State<'_, AppState>,
) -> Result<crate::lan::models::LanServerStatusDto, OpenTallyError> {
    let guard = state
        .lan_server
        .read()
        .map_err(|e| OpenTallyError::Internal(e.to_string()))?;

    let local_ips = crate::lan::server::detect_local_ips();

    if let Some(ref handle) = *guard {
        Ok(crate::lan::models::LanServerStatusDto {
            is_running: true,
            bind_address: handle.bind_address.clone(),
            port: handle.port,
            local_ip_addresses: local_ips,
            active_sessions: handle.session_store.list_active(),
        })
    } else {
        Ok(crate::lan::models::LanServerStatusDto {
            is_running: false,
            bind_address: "0.0.0.0".to_string(),
            port: 7900,
            local_ip_addresses: local_ips,
            active_sessions: Vec::new(),
        })
    }
}

#[tauri::command]
pub async fn revoke_lan_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<crate::lan::models::LanServerStatusDto, OpenTallyError> {
    info!(
        ">>> [Tauri IPC] Received 'revoke_lan_session' for session_id='{}'",
        session_id
    );

    {
        let guard = state
            .lan_server
            .read()
            .map_err(|e| OpenTallyError::Internal(e.to_string()))?;

        if let Some(ref handle) = *guard {
            handle.session_store.revoke_by_id(&session_id);
        }
    }

    get_lan_server_status(state).await
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
                bank_allocations: None,
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
                bank_allocations: None,
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
                bank_allocations: None,
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
                bank_allocations: None,
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

// =========================================================================
// Milestone 18: BYO-GSP Connector & Declarative Custom Report Extensions
// =========================================================================

#[tauri::command]
pub async fn get_company_gsp_config(
    state: tauri::State<'_, AppState>,
) -> Result<crate::gst::gsp_connector::CompanyGspConfigDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let _ = actor;
    crate::gst::gsp_connector::GspConnectorService::get_config(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn save_company_gsp_config(
    state: tauri::State<'_, AppState>,
    input: crate::gst::gsp_connector::SaveCompanyGspConfigInput,
) -> Result<crate::gst::gsp_connector::CompanyGspConfigDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ManageUsers,
        )
        .await?;
    crate::gst::gsp_connector::GspConnectorService::save_config(
        state.db.pool(),
        &company_id,
        input,
        &actor,
    )
    .await
}

#[tauri::command]
pub async fn push_einvoice_to_gsp(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<crate::gst::gsp_connector::GspEinvoiceResponse, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    crate::gst::gsp_connector::GspConnectorService::push_einvoice_to_gsp(
        state.db.pool(),
        &company_id,
        &voucher_id,
        &actor,
    )
    .await
}

#[tauri::command]
pub async fn push_eway_bill_to_gsp(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<crate::gst::gsp_connector::GspEwayBillResponse, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::PostVoucher,
        )
        .await?;
    crate::gst::gsp_connector::GspConnectorService::push_eway_bill_to_gsp(
        state.db.pool(),
        &company_id,
        &voucher_id,
        &actor,
    )
    .await
}

#[tauri::command]
pub async fn list_custom_reports(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::extensions::CustomReportDefinitionDto>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let _ = actor;
    crate::extensions::CustomReportService::list_reports(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn save_custom_report(
    state: tauri::State<'_, AppState>,
    input: crate::extensions::SaveCustomReportInput,
) -> Result<crate::extensions::CustomReportDefinitionDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::extensions::CustomReportService::save_report(state.db.pool(), &company_id, input, &actor)
        .await
}

#[tauri::command]
pub async fn delete_custom_report(
    state: tauri::State<'_, AppState>,
    report_id: String,
) -> Result<(), OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::extensions::CustomReportService::delete_report(
        state.db.pool(),
        &company_id,
        &report_id,
        &actor,
    )
    .await
}

#[tauri::command]
pub async fn run_custom_report(
    state: tauri::State<'_, AppState>,
    report_id: String,
) -> Result<crate::extensions::CustomReportExecutionResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let _ = actor;
    crate::extensions::CustomReportService::run_report(state.db.pool(), &company_id, &report_id)
        .await
}

#[tauri::command]
pub async fn export_custom_report_csv(
    state: tauri::State<'_, AppState>,
    report_id: String,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let _ = actor;
    crate::extensions::CustomReportService::export_report_csv(
        state.db.pool(),
        &company_id,
        &report_id,
    )
    .await
}

#[tauri::command]
pub async fn export_custom_report_template(
    state: tauri::State<'_, AppState>,
    report_id: String,
) -> Result<String, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    let _ = actor;
    crate::extensions::CustomReportService::export_template(
        state.db.pool(),
        &company_id,
        &report_id,
    )
    .await
}

#[tauri::command]
pub async fn import_custom_report_template(
    state: tauri::State<'_, AppState>,
    template_json: String,
) -> Result<crate::extensions::CustomReportDefinitionDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    let actor = state
        .resolve_actor(
            &company_id,
            crate::security::models::ActionPermission::ViewStandardReports,
        )
        .await?;
    crate::extensions::CustomReportService::import_template(
        state.db.pool(),
        &company_id,
        &template_json,
        &actor,
    )
    .await
}

// Milestone 20: Distributor Inventory & Billing Depth
#[tauri::command]
pub async fn create_compound_unit(
    state: tauri::State<'_, AppState>,
    name: String,
    symbol: String,
    first_unit_id: String,
    second_unit_id: String,
    conversion_factor: String,
) -> Result<crate::inventory::units::CompoundUnitDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::units::create_compound_unit(
        state.db.pool(),
        &company_id,
        &name,
        &symbol,
        &first_unit_id,
        &second_unit_id,
        &conversion_factor,
    )
    .await
}

#[tauri::command]
pub async fn list_price_levels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<crate::inventory::pricing::PriceLevelDto>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::pricing::list_price_levels(state.db.pool(), &company_id).await
}

#[tauri::command]
pub async fn create_price_level(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<crate::inventory::pricing::PriceLevelDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::pricing::create_price_level(state.db.pool(), &company_id, &name).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn upsert_price_list_entry(
    state: tauri::State<'_, AppState>,
    price_level_id: String,
    stock_item_id: String,
    effective_from: String,
    min_qty: String,
    max_qty: Option<String>,
    unit_rate: String,
    discount_pct: String,
) -> Result<crate::inventory::pricing::PriceListEntryDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::pricing::upsert_price_list_entry(
        state.db.pool(),
        &company_id,
        &price_level_id,
        &stock_item_id,
        &effective_from,
        &min_qty,
        max_qty.as_deref(),
        &unit_rate,
        &discount_pct,
    )
    .await
}

#[tauri::command]
pub async fn list_price_list_entries(
    state: tauri::State<'_, AppState>,
    price_level_id: Option<String>,
) -> Result<Vec<crate::inventory::pricing::PriceListEntryDto>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::pricing::list_price_list_entries(
        state.db.pool(),
        &company_id,
        price_level_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn resolve_item_price(
    state: tauri::State<'_, AppState>,
    stock_item_id: String,
    party_ledger_id: Option<String>,
    price_level_id: Option<String>,
    quantity: String,
    as_of_date: String,
) -> Result<crate::inventory::pricing::ResolvedItemPriceDto, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::inventory::pricing::resolve_item_price(
        state.db.pool(),
        &company_id,
        &stock_item_id,
        party_ledger_id.as_deref(),
        price_level_id.as_deref(),
        &quantity,
        &as_of_date,
    )
    .await
}
