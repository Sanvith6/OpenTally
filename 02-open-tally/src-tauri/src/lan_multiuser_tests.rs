//! Integration Test Suite for Milestone 17: Multi-User LAN Host/Client Mode
//!
//! Spins up the embedded `axum` LAN server on `127.0.0.1:0` against a real temporary
//! SQLite WAL database (`PRAGMA busy_timeout = 10000;`) and verifies:
//! 1. Unauthenticated `/api/v1/rpc` rejected with `401 Unauthorized`.
//! 2. `Auditor` login succeeds, queries `get_trial_balance` (`200`), blocked (`403`) from `create_voucher`.
//! 3. `Admin`/`Accountant` login posts vouchers with `audit_log.performed_by == username` and intact SHA-256 chain.
//! 4. 5 concurrent `tokio` LAN clients post balanced vouchers simultaneously -> 5 unique sequential voucher numbers, zero `SQLITE_BUSY`, Trial Balance `difference == "0"`.
//! 5. Revoking a session or deactivating a user immediately returns `401` on subsequent RPC calls.
//! 6. Graceful shutdown stops the TCP listener cleanly.

use crate::audit::certification::AuditCertificationService;
use crate::database::migrations::run_all_migrations;
use crate::database::DatabaseManager;
use crate::lan::start_lan_listener;
use crate::security::models::{
    CreateUserInput, DeactivateUserInput, EnableSecurityInput, UserRole,
};
use crate::security::service::SecurityService;
use serde_json::{json, Value};
use std::collections::HashSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn setup_lan_test_db() -> (tempfile::TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("lan_multiuser_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("init db");
    let pool = db.pool();

    run_all_migrations(pool).await.expect("run all migrations");

    let comp_id = "cmp_lan_01".to_string();
    sqlx::query(
        r#"
        INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_symbol, currency_formal_name, currency_decimal_places)
        VALUES (?, 'LAN MultiUser Corp', 'LAN Corp', 'India', 'Karnataka', '2026-04-01', '2026-04-01', '₹', 'INR', 2)
        "#,
    )
    .bind(&comp_id)
    .execute(pool)
    .await
    .expect("seed company");

    sqlx::query(
        r#"
        INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
        VALUES ('grp_ca_lan1', ?, 'Current Assets', NULL, 'Assets', 1),
               ('grp_cash_lan1', ?, 'Cash-in-Hand', 'grp_ca_lan1', 'Assets', 0),
               ('grp_inc_lan1', ?, 'Indirect Incomes', NULL, 'Income', 1)
        "#,
    )
    .bind(&comp_id)
    .bind(&comp_id)
    .bind(&comp_id)
    .execute(pool)
    .await
    .expect("seed groups");

    sqlx::query(
        r#"
        INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
        VALUES ('led_cash_lan1', ?, 'grp_cash_lan1', 'Cash', 1, 'cash'),
               ('led_income_lan1', ?, 'grp_inc_lan1', 'Consulting Income', 0, 'standard')
        "#,
    )
    .bind(&comp_id)
    .bind(&comp_id)
    .execute(pool)
    .await
    .expect("seed ledgers");

    sqlx::query(
        r#"
        INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
        VALUES ('ser_jnl_lan1', ?, 'vt_jnl', 'Journal LAN Series', 'JNL', 'automatic', 'yearly')
        "#,
    )
    .bind(&comp_id)
    .execute(pool)
    .await
    .expect("seed series");

    sqlx::query(
        r#"
        INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
        VALUES ('cfg_jnl_lan1', ?, 'vt_jnl', 'ser_jnl_lan1', '2026-04-01', 'JNL/', '', 1, 4)
        "#,
    )
    .bind(&comp_id)
    .execute(pool)
    .await
    .expect("seed config");

    SecurityService::enable_company_security(
        pool,
        EnableSecurityInput {
            company_id: comp_id.clone(),
            admin_username: "lan_admin".to_string(),
            admin_password: "AdminPass123!".to_string(),
        },
        "system",
    )
    .await
    .expect("enable security");

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: comp_id.clone(),
            username: "lan_acct".to_string(),
            password: "AcctPass123!".to_string(),
            role: UserRole::Accountant,
        },
        "lan_admin",
    )
    .await
    .expect("create accountant");

    SecurityService::create_user(
        pool,
        CreateUserInput {
            company_id: comp_id.clone(),
            username: "lan_auditor".to_string(),
            password: "AuditPass123!".to_string(),
            role: UserRole::Auditor,
        },
        "lan_admin",
    )
    .await
    .expect("create auditor");

    (dir, db, comp_id)
}

/// Minimal raw HTTP/1.1 client over `tokio::net::TcpStream` to exercise the real Axum TCP listener.
async fn send_http(
    port: u16,
    method: &str,
    path: &str,
    bearer_token: Option<&str>,
    body: Option<&Value>,
) -> (u16, Value) {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .expect("connect to LAN server");

    let body_str = body.map(|b| b.to_string()).unwrap_or_default();
    let mut req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        method, path, port
    );
    if let Some(tok) = bearer_token {
        req.push_str(&format!("Authorization: Bearer {}\r\n", tok));
    }
    if body.is_some() {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", body_str.len()));
    }
    req.push_str("\r\n");
    if body.is_some() {
        req.push_str(&body_str);
    }

    stream.write_all(req.as_bytes()).await.expect("write req");
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await.expect("read resp");

    let resp_str = String::from_utf8_lossy(&resp_buf);
    let (head, raw_body) = resp_str
        .split_once("\r\n\r\n")
        .expect("valid HTTP response");
    let status_line = head.lines().next().expect("status line");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .expect("status code")
        .parse()
        .expect("numeric status");

    let parsed_body: Value =
        serde_json::from_str(raw_body.trim()).unwrap_or(json!({ "raw": raw_body }));
    (status, parsed_body)
}

async fn lan_login(port: u16, company_id: &str, username: &str, password: &str) -> (u16, Value) {
    send_http(
        port,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(&json!({
            "company_id": company_id,
            "username": username,
            "password": password
        })),
    )
    .await
}

fn build_balanced_voucher_params(company_id: &str, amount: &str, narration: &str) -> Value {
    json!({
        "company_id": company_id,
        "voucher_type_id": "vt_jnl",
        "series_id": "ser_jnl_lan1",
        "date": "2026-04-15",
        "narration": narration,
        "lines": [
            {
                "ledger_id": "led_cash_lan1",
                "entry_type": "Dr",
                "amount": amount,
                "narration": "Cash received"
            },
            {
                "ledger_id": "led_income_lan1",
                "entry_type": "Cr",
                "amount": amount,
                "narration": "Consulting income"
            }
        ]
    })
}

#[tokio::test]
async fn test_lan_unauthenticated_rpc_rejected_401() {
    let (_dir, db, comp_id) = setup_lan_test_db().await;
    let mut handle = start_lan_listener(db.pool().clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");

    // 1. Health check is public & returns mode = lan_host
    let (health_status, health_body) =
        send_http(handle.port, "GET", "/api/v1/health", None, None).await;
    assert_eq!(health_status, 200);
    assert_eq!(health_body["mode"], "lan_host");
    assert_eq!(health_body["status"], "ok");

    // 2. Unauthenticated /api/v1/rpc must return 401
    let (status, body) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        None,
        Some(&json!({
            "command": "get_trial_balance",
            "params": { "company_id": comp_id, "as_of_date": "2026-04-30" }
        })),
    )
    .await;
    assert_eq!(status, 401);
    assert_eq!(body["error"]["code"], 401);

    // 3. Invalid Bearer token must also return 401
    let (bad_status, _) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some("invalid_token_deadbeef"),
        Some(&json!({
            "command": "list_companies",
            "params": {}
        })),
    )
    .await;
    assert_eq!(bad_status, 401);

    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }
}

#[tokio::test]
async fn test_lan_auditor_rbac_read_allowed_write_denied_403() {
    let (_dir, db, comp_id) = setup_lan_test_db().await;
    let mut handle = start_lan_listener(db.pool().clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");

    let (login_status, login_body) =
        lan_login(handle.port, &comp_id, "lan_auditor", "AuditPass123!").await;
    assert_eq!(login_status, 200);
    let token = login_body["token"].as_str().expect("token");

    // Auditor CAN read Trial Balance (200)
    let (tb_status, tb_body) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(token),
        Some(&json!({
            "command": "get_trial_balance",
            "params": { "company_id": comp_id, "as_of_date": "2026-04-30" }
        })),
    )
    .await;
    assert_eq!(tb_status, 200);
    assert!(tb_body["result"].is_object());

    // Auditor CANNOT post a voucher (403 Forbidden)
    let (vch_status, vch_body) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(token),
        Some(&json!({
            "command": "create_voucher",
            "params": build_balanced_voucher_params(&comp_id, "500.00", "Unauthorized attempt")
        })),
    )
    .await;
    assert_eq!(vch_status, 403);
    assert_eq!(vch_body["error"]["code"], 403);

    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }
}

#[tokio::test]
async fn test_lan_admin_posts_voucher_with_audit_log_attribution_and_sha256_chain() {
    let (_dir, db, comp_id) = setup_lan_test_db().await;
    let pool = db.pool();
    let mut handle = start_lan_listener(pool.clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");

    let (login_status, login_body) =
        lan_login(handle.port, &comp_id, "lan_acct", "AcctPass123!").await;
    assert_eq!(login_status, 200);
    let token = login_body["token"].as_str().expect("token");

    let (vch_status, vch_body) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(token),
        Some(&json!({
            "command": "create_voucher",
            "params": build_balanced_voucher_params(&comp_id, "2500.00", "LAN Accountant Entry")
        })),
    )
    .await;
    assert_eq!(vch_status, 200);
    let voucher_id = vch_body["result"]["voucher"]["id"]
        .as_str()
        .expect("voucher id");
    assert_eq!(vch_body["result"]["voucher"]["status"], "posted");

    // Verify audit_log.performed_by == "lan_acct"
    let performed_by: String = sqlx::query_scalar(
        "SELECT performed_by FROM audit_log WHERE company_id = ? AND entity_id = ? AND action = 'POST' LIMIT 1",
    )
    .bind(&comp_id)
    .bind(voucher_id)
    .fetch_one(pool)
    .await
    .expect("audit_log row");
    assert_eq!(performed_by, "lan_acct");

    // Verify intact SHA-256 hash chain across audit_log
    let chain_report = AuditCertificationService::verify_audit_trail_integrity(pool, &comp_id)
        .await
        .expect("verify audit chain");
    assert!(chain_report.is_valid);

    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }
}

#[tokio::test]
async fn test_lan_concurrent_multiuser_stress_5_clients_sequential_vouchers() {
    let (_dir, db, comp_id) = setup_lan_test_db().await;
    let pool = db.pool();
    let mut handle = start_lan_listener(pool.clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");
    let port = handle.port;

    // Login 5 concurrent sessions (using Accountant + Admin)
    let mut tokens = Vec::with_capacity(5);
    for i in 0..5 {
        let (user, pass) = if i % 2 == 0 {
            ("lan_acct", "AcctPass123!")
        } else {
            ("lan_admin", "AdminPass123!")
        };
        let (st, body) = lan_login(port, &comp_id, user, pass).await;
        assert_eq!(st, 200);
        tokens.push(body["token"].as_str().unwrap().to_string());
    }

    // Fire 5 concurrent create_voucher RPC requests simultaneously
    let mut tasks = Vec::with_capacity(5);
    for (idx, tok) in tokens.into_iter().enumerate() {
        let cid = comp_id.clone();
        tasks.push(tokio::spawn(async move {
            let params = build_balanced_voucher_params(
                &cid,
                "1000.00",
                &format!("Concurrent LAN Client #{}", idx + 1),
            );
            send_http(
                port,
                "POST",
                "/api/v1/rpc",
                Some(&tok),
                Some(&json!({ "command": "create_voucher", "params": params })),
            )
            .await
        }));
    }

    let mut voucher_numbers = HashSet::new();
    for task in tasks {
        let (status, body) = task.await.expect("task join");
        assert_eq!(status, 200, "concurrent voucher post failed: {:?}", body);
        let vnum = body["result"]["voucher"]["voucher_number"]
            .as_str()
            .expect("voucher_number")
            .to_string();
        assert!(
            voucher_numbers.insert(vnum.clone()),
            "duplicate voucher number detected: {}",
            vnum
        );
    }
    assert_eq!(voucher_numbers.len(), 5);

    // Verify Trial Balance Equilibrium across all 5 concurrent postings
    let (_st, login_body) = lan_login(port, &comp_id, "lan_auditor", "AuditPass123!").await;
    let aud_tok = login_body["token"].as_str().unwrap();
    let (tb_st, tb_body) = send_http(
        port,
        "POST",
        "/api/v1/rpc",
        Some(aud_tok),
        Some(&json!({
            "command": "get_trial_balance",
            "params": { "company_id": comp_id, "as_of_date": "2026-04-30" }
        })),
    )
    .await;
    assert_eq!(tb_st, 200);
    assert_eq!(tb_body["result"]["difference"], "0.00");
    assert_eq!(tb_body["result"]["total_debit"], "5000.00");
    assert_eq!(tb_body["result"]["total_credit"], "5000.00");

    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }
}

#[tokio::test]
async fn test_lan_session_revocation_and_user_deactivation_eviction_401() {
    let (_dir, db, comp_id) = setup_lan_test_db().await;
    let pool = db.pool();
    let mut handle = start_lan_listener(pool.clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");

    // 1. Login as lan_acct and verify RPC succeeds
    let (_, login_acct) = lan_login(handle.port, &comp_id, "lan_acct", "AcctPass123!").await;
    let acct_token = login_acct["token"].as_str().unwrap();
    let acct_session_id = login_acct["session_id"].as_str().unwrap();

    let (ok_st, _) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(acct_token),
        Some(&json!({ "command": "list_ledgers", "params": { "company_id": comp_id } })),
    )
    .await;
    assert_eq!(ok_st, 200);

    // 2. Host revokes session -> immediate 401
    assert!(handle.session_store.revoke_by_id(acct_session_id).is_some());
    let (revoked_st, _) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(acct_token),
        Some(&json!({ "command": "list_ledgers", "params": { "company_id": comp_id } })),
    )
    .await;
    assert_eq!(revoked_st, 401);

    // 3. Login as lan_auditor, then deactivate user in DB -> immediate 401 on live check
    let (_, login_aud) = lan_login(handle.port, &comp_id, "lan_auditor", "AuditPass123!").await;
    let aud_token = login_aud["token"].as_str().unwrap();
    let aud_user_id = login_aud["user_id"].as_str().unwrap();

    SecurityService::deactivate_user(
        pool,
        DeactivateUserInput {
            company_id: comp_id.clone(),
            user_id: aud_user_id.to_string(),
        },
        "lan_admin",
    )
    .await
    .expect("deactivate auditor");

    let (deact_st, _) = send_http(
        handle.port,
        "POST",
        "/api/v1/rpc",
        Some(aud_token),
        Some(&json!({ "command": "list_ledgers", "params": { "company_id": comp_id } })),
    )
    .await;
    assert_eq!(deact_st, 401);

    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }
}

#[tokio::test]
async fn test_lan_graceful_shutdown_stops_listener_cleanly() {
    let (_dir, db, _comp_id) = setup_lan_test_db().await;
    let mut handle = start_lan_listener(db.pool().clone(), Some("127.0.0.1".to_string()), Some(0))
        .await
        .expect("start LAN listener");
    let port = handle.port;

    let (st, _) = send_http(port, "GET", "/api/v1/health", None, None).await;
    assert_eq!(st, 200);

    // Trigger graceful shutdown
    let tx = handle.shutdown_tx.take().expect("shutdown_tx");
    tx.send(()).expect("send shutdown");

    // Yield briefly for Tokio task to close listener
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let conn_res = TcpStream::connect(format!("127.0.0.1:{}", port)).await;
    assert!(
        conn_res.is_err(),
        "TCP port should be closed after graceful shutdown"
    );
}
