use crate::accounting::{GroupService, LedgerService};
use crate::company::features::CompanyFeaturesService;
use crate::company::service::CompanyService;
use crate::errors::OpenTallyError;
use crate::inventory::InventoryService;
use crate::lan::auth::{
    handle_lan_login, handle_lan_logout, validate_lan_session_live, LanSessionStore,
};
use crate::lan::models::{
    LanHealthResponse, LanLoginRequest, LanRpcError, LanRpcRequest, LanRpcResponse,
};
use crate::navigation::NavigationSearchService;
use crate::purchase::PurchaseDocumentService;
use crate::reports::billwise::BillWiseReportService;
use crate::reports::gst::GstReportService;
use crate::reports::{
    BalanceSheetRequest, BalanceSheetService, DayBookRequest, DayBookService, LedgerReportRequest,
    LedgerReportService, ProfitAndLossRequest, ProfitAndLossService, TrialBalanceRequest,
    TrialBalanceService,
};
use crate::sales::SalesDocumentService;
use crate::security::models::ActionPermission;
use crate::vouchers::{
    CancelVoucherRequest, CreateVoucherDraftRequest, ReverseVoucherRequest,
    VoucherCancellationService, VoucherDraftService, VoucherFilter, VoucherPostingService,
    VoucherReversalService,
};
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use sqlx::{Pool, Sqlite};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

/// Shared state for Axum LAN HTTP handlers.
#[derive(Clone)]
pub struct LanServerContext {
    pub pool: Pool<Sqlite>,
    pub session_store: Arc<LanSessionStore>,
    pub write_mutex: Arc<tokio::sync::Mutex<()>>,
}

/// Detects local LAN IPv4 addresses without external dependencies.
pub fn detect_local_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                let ip = local_addr.ip().to_string();
                if ip != "0.0.0.0" && ip != "127.0.0.1" && !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
    }
    if !ips.contains(&"127.0.0.1".to_string()) {
        ips.push("127.0.0.1".to_string());
    }
    ips
}

/// Builds the Axum Router for the embedded OpenTally LAN Server.
pub fn create_lan_router(ctx: LanServerContext) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/companies", get(companies_handler))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .route("/api/v1/rpc", post(rpc_handler))
        .layer(CorsLayer::permissive())
        .with_state(ctx)
}

/// GET /api/v1/health: Lightweight health report without exposing sensitive company details.
async fn health_handler() -> impl IntoResponse {
    Json(LanHealthResponse {
        app: "OpenTally".to_string(),
        version: "2.0.0".to_string(),
        mode: "lan_host".to_string(),
        status: "ok".to_string(),
    })
}

/// GET /api/v1/companies: Lists available companies for pre-login LAN client selection.
async fn companies_handler(State(ctx): State<LanServerContext>) -> Response {
    match CompanyService::list_companies(&ctx.pool).await {
        Ok(companies) => (StatusCode::OK, Json(companies)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LanRpcError {
                code: 500,
                message: e.to_string(),
                details: None,
            }),
        )
            .into_response(),
    }
}

/// POST /api/v1/auth/login: Authenticates client over LAN, issuing a Bearer session token.
async fn login_handler(
    State(ctx): State<LanServerContext>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LanLoginRequest>,
) -> Response {
    let client_ip = Some(addr.ip().to_string());
    match handle_lan_login(&ctx.pool, &ctx.session_store, req, client_ip).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(OpenTallyError::NotFound(msg)) => (
            StatusCode::NOT_FOUND,
            Json(LanRpcError {
                code: 404,
                message: msg,
                details: None,
            }),
        )
            .into_response(),
        Err(OpenTallyError::Unauthorized(msg)) => (
            StatusCode::UNAUTHORIZED,
            Json(LanRpcError {
                code: 401,
                message: msg,
                details: None,
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LanRpcError {
                code: 500,
                message: e.to_string(),
                details: None,
            }),
        )
            .into_response(),
    }
}

/// POST /api/v1/auth/logout: Revokes the Bearer session token.
async fn logout_handler(State(ctx): State<LanServerContext>, headers: HeaderMap) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(LanRpcError {
                    code: 401,
                    message: "Missing Bearer token".to_string(),
                    details: None,
                }),
            )
                .into_response();
        }
    };

    match handle_lan_logout(&ctx.pool, &ctx.session_store, &token).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LanRpcError {
                code: 500,
                message: e.to_string(),
                details: None,
            }),
        )
            .into_response(),
    }
}

/// Extracts Bearer token from the Authorization header.
fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let parts: Vec<&str> = auth.split_whitespace().collect();
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// POST /api/v1/rpc: Unified JSON-RPC command dispatcher over LAN.
async fn rpc_handler(
    State(ctx): State<LanServerContext>,
    headers: HeaderMap,
    Json(req): Json<LanRpcRequest>,
) -> Response {
    let token = match extract_bearer_token(&headers) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(LanRpcResponse {
                    result: None,
                    error: Some(LanRpcError {
                        code: 401,
                        message: "Unauthorized: Missing Bearer token in Authorization header"
                            .to_string(),
                        details: None,
                    }),
                }),
            )
                .into_response();
        }
    };

    match dispatch_rpc(&ctx, &token, req).await {
        Ok(val) => (
            StatusCode::OK,
            Json(LanRpcResponse {
                result: Some(val),
                error: None,
            }),
        )
            .into_response(),
        Err(OpenTallyError::Unauthorized(msg)) => (
            StatusCode::UNAUTHORIZED,
            Json(LanRpcResponse {
                result: None,
                error: Some(LanRpcError {
                    code: 401,
                    message: msg,
                    details: None,
                }),
            }),
        )
            .into_response(),
        Err(OpenTallyError::Forbidden(msg)) => (
            StatusCode::FORBIDDEN,
            Json(LanRpcResponse {
                result: None,
                error: Some(LanRpcError {
                    code: 403,
                    message: msg,
                    details: None,
                }),
            }),
        )
            .into_response(),
        Err(OpenTallyError::Validation(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(LanRpcResponse {
                result: None,
                error: Some(LanRpcError {
                    code: 400,
                    message: msg,
                    details: None,
                }),
            }),
        )
            .into_response(),
        Err(OpenTallyError::NotFound(msg)) => (
            StatusCode::NOT_FOUND,
            Json(LanRpcResponse {
                result: None,
                error: Some(LanRpcError {
                    code: 404,
                    message: msg,
                    details: None,
                }),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(LanRpcResponse {
                result: None,
                error: Some(LanRpcError {
                    code: 500,
                    message: e.to_string(),
                    details: None,
                }),
            }),
        )
            .into_response(),
    }
}

/// Ensures an RPC request only accesses the company the LAN session is authenticated for.
fn resolve_session_company_id<'a>(
    session: &'a crate::lan::models::LanSession,
    params: &'a serde_json::Value,
) -> Result<&'a str, OpenTallyError> {
    if let Some(cid) = params.get("company_id").and_then(|v| v.as_str()) {
        if cid != session.company_id {
            return Err(OpenTallyError::Forbidden(format!(
                "Cross-company access denied: session is scoped to '{}', cannot access '{}'",
                session.company_id, cid
            )));
        }
    }
    Ok(&session.company_id)
}

/// Dispatches JSON-RPC command to the corresponding domain service.
async fn dispatch_rpc(
    ctx: &LanServerContext,
    token: &str,
    req: LanRpcRequest,
) -> Result<serde_json::Value, OpenTallyError> {
    let pool = &ctx.pool;

    match req.command.as_str() {
        // -------------------------------------------------------------------
        // Companies & Masters
        // -------------------------------------------------------------------
        "list_companies" => {
            let _session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let companies = CompanyService::list_companies(pool).await?;
            serde_json::to_value(companies).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_company_features" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let features = CompanyFeaturesService::get_company_features(pool, company_id).await?;
            serde_json::to_value(features).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "list_account_groups" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let groups = GroupService::list_groups(pool, company_id, false).await?;
            serde_json::to_value(groups).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "list_ledgers" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let group_id = req.params.get("group_id").and_then(|v| v.as_str());
            let search_query = req.params.get("search_query").and_then(|v| v.as_str());

            let ledgers =
                LedgerService::list_ledgers(pool, company_id, group_id, search_query).await?;
            serde_json::to_value(ledgers).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "list_stock_items" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let search = req.params.get("search").and_then(|v| v.as_str());

            let items = InventoryService::list_stock_items(pool, company_id, search).await?;
            serde_json::to_value(items).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        // -------------------------------------------------------------------
        // Vouchers & Commercial Transactions
        // -------------------------------------------------------------------
        "create_voucher" => {
            let _write_guard = ctx.write_mutex.lock().await;
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::PostVoucher,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let draft_req: CreateVoucherDraftRequest = serde_json::from_value(req.params.clone())
                .map_err(|e| {
                OpenTallyError::Validation(format!("Invalid create_voucher request: {}", e))
            })?;

            // 1. Save draft
            let draft = VoucherDraftService::create_draft(pool, company_id, draft_req).await?;

            // 2. Post draft into general ledger with actor = LAN username
            let posted = VoucherPostingService::post_voucher(
                pool,
                company_id,
                &draft.voucher.id,
                Some(&session.username),
            )
            .await?;

            serde_json::to_value(posted).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "post_sales_document" => {
            let _write_guard = ctx.write_mutex.lock().await;
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::PostVoucher,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let doc_id = req
                .params
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'id' parameter".into()))?;
            let sales_ledger_id = req
                .params
                .get("sales_ledger_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let result = SalesDocumentService::post_sales_document(
                pool,
                company_id,
                doc_id,
                sales_ledger_id,
            )
            .await?;

            serde_json::to_value(result).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "post_purchase_document" => {
            let _write_guard = ctx.write_mutex.lock().await;
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::PostVoucher,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let doc_id = req
                .params
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'id' parameter".into()))?;
            let purchase_ledger_id = req
                .params
                .get("purchase_ledger_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let tds_section_code = req
                .params
                .get("tds_section_code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let result = PurchaseDocumentService::post_purchase_document_with_tds(
                pool,
                company_id,
                doc_id,
                purchase_ledger_id,
                tds_section_code,
            )
            .await?;

            serde_json::to_value(result).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "list_vouchers" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let filter: VoucherFilter =
                serde_json::from_value(req.params.clone()).unwrap_or_default();

            let vouchers = VoucherDraftService::list_drafts(pool, company_id, filter).await?;
            serde_json::to_value(vouchers).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_voucher_full_detail" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let voucher_id = req
                .params
                .get("voucher_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'voucher_id'".into()))?;

            let detail =
                NavigationSearchService::get_voucher_full_detail(pool, company_id, voucher_id)
                    .await?;
            serde_json::to_value(detail).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "cancel_voucher" => {
            let _write_guard = ctx.write_mutex.lock().await;
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::CancelOrReverseVoucher,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let cancel_req: CancelVoucherRequest = serde_json::from_value(req.params.clone())
                .map_err(|e| {
                    OpenTallyError::Validation(format!("Invalid cancel_voucher request: {}", e))
                })?;

            let cancelled =
                VoucherCancellationService::cancel_voucher(pool, company_id, cancel_req).await?;
            serde_json::to_value(cancelled).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "reverse_voucher" => {
            let _write_guard = ctx.write_mutex.lock().await;
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::CancelOrReverseVoucher,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let rev_req: ReverseVoucherRequest = serde_json::from_value(req.params.clone())
                .map_err(|e| {
                    OpenTallyError::Validation(format!("Invalid reverse_voucher request: {}", e))
                })?;

            let reversed =
                VoucherReversalService::reverse_voucher(pool, company_id, rev_req).await?;
            serde_json::to_value(reversed).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        // -------------------------------------------------------------------
        // Financial Reports & Search (Protected ReadOnly Connection Guards)
        // -------------------------------------------------------------------
        "get_trial_balance" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let tb_req: TrialBalanceRequest = serde_json::from_value(req.params.clone())
                .unwrap_or_else(|_| TrialBalanceRequest {
                    as_of_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                    account_group_id: None,
                    ledger_id: None,
                    account_search: None,
                    include_zero_balances: false,
                });

            let report = TrialBalanceService::generate(pool, company_id, tb_req).await?;
            serde_json::to_value(report).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_daybook" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let db_req: DayBookRequest =
                serde_json::from_value(req.params.clone()).map_err(|e| {
                    OpenTallyError::Validation(format!("Invalid DayBookRequest: {}", e))
                })?;

            let report = DayBookService::generate(pool, company_id, db_req).await?;
            serde_json::to_value(report).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_ledger_report" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let lr_req: LedgerReportRequest =
                serde_json::from_value(req.params.clone()).map_err(|e| {
                    OpenTallyError::Validation(format!("Invalid LedgerReportRequest: {}", e))
                })?;

            let report = LedgerReportService::generate(pool, company_id, lr_req).await?;
            serde_json::to_value(report).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_profit_and_loss" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let pl_req: ProfitAndLossRequest = serde_json::from_value(req.params.clone())
                .unwrap_or_else(|_| ProfitAndLossRequest {
                    period: crate::reports::ReportDateRange {
                        date_from: "2026-04-01".to_string(),
                        date_to: "2027-03-31".to_string(),
                    },
                    account_group_id: None,
                    ledger_search: None,
                    include_zero_balances: false,
                });

            let report = ProfitAndLossService::generate(pool, company_id, pl_req).await?;
            serde_json::to_value(report).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_balance_sheet" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let as_of_date = req
                .params
                .get("as_of_date")
                .and_then(|v| v.as_str())
                .unwrap_or("2099-12-31")
                .to_string();

            let bs_req = BalanceSheetRequest {
                as_of_date,
                account_group_id: None,
                ledger_search: None,
                include_zero_balances: false,
            };
            let report = BalanceSheetService::generate(pool, company_id, bs_req).await?;
            serde_json::to_value(report).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_bills_receivable" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let as_of_date = req
                .params
                .get("as_of_date")
                .and_then(|v| v.as_str())
                .unwrap_or("2099-12-31");

            let bills =
                BillWiseReportService::get_bills_receivable(pool, company_id, as_of_date).await?;
            serde_json::to_value(bills).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_bills_payable" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let as_of_date = req
                .params
                .get("as_of_date")
                .and_then(|v| v.as_str())
                .unwrap_or("2099-12-31");

            let bills =
                BillWiseReportService::get_bills_payable(pool, company_id, as_of_date).await?;
            serde_json::to_value(bills).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_gstr1_summary" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let from_date = req
                .params
                .get("from_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'from_date'".into()))?;
            let to_date = req
                .params
                .get("to_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'to_date'".into()))?;

            let summary =
                GstReportService::get_gstr1_summary(pool, company_id, from_date, to_date).await?;
            serde_json::to_value(summary).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "get_gstr3b_summary" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewFinancialStatements,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let from_date = req
                .params
                .get("from_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'from_date'".into()))?;
            let to_date = req
                .params
                .get("to_date")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OpenTallyError::Validation("Missing 'to_date'".into()))?;

            let summary =
                GstReportService::get_gstr3b_summary(pool, company_id, from_date, to_date).await?;
            serde_json::to_value(summary).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        "global_goto_search" => {
            let session = validate_lan_session_live(
                pool,
                &ctx.session_store,
                token,
                ActionPermission::ViewStandardReports,
            )
            .await?;
            let company_id = resolve_session_company_id(&session, &req.params)?;

            let query = req
                .params
                .get("query")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let limit: u32 = req
                .params
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|l| l as u32)
                .unwrap_or(20);

            let results =
                NavigationSearchService::global_goto_search(pool, company_id, query, limit).await?;
            serde_json::to_value(results).map_err(|e| OpenTallyError::Internal(e.to_string()))
        }

        unknown => Err(OpenTallyError::Validation(format!(
            "Unsupported LAN JSON-RPC command: '{}'",
            unknown
        ))),
    }
}
