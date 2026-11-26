use crate::security::models::UserRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lightweight health check response for LAN clients (no sensitive information).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanHealthResponse {
    pub app: String,
    pub version: String,
    pub mode: String,
    pub status: String,
}

/// Request to login to a company over LAN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanLoginRequest {
    pub company_id: String,
    pub username: String,
    pub password: Option<String>,
}

/// Successful login response providing the Bearer session token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanLoginResponse {
    pub session_id: String,
    pub token: String,
    pub company_id: String,
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub expires_at: DateTime<Utc>,
}

/// Logout response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanLogoutResponse {
    pub status: String,
}

/// In-memory representation of an active LAN session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanSession {
    pub session_id: String,
    pub token: String,
    pub company_id: String,
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub client_ip: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
}

/// DTO for displaying connected client sessions in Host UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanSessionDto {
    pub session_id: String,
    pub company_id: String,
    pub user_id: String,
    pub username: String,
    pub role: String,
    pub client_ip: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub last_active_at: String,
}

/// Host LAN server status reported to Tauri IPC and frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanServerStatusDto {
    pub is_running: bool,
    pub bind_address: String,
    pub port: u16,
    pub local_ip_addresses: Vec<String>,
    pub active_sessions: Vec<LanSessionDto>,
}

/// Incoming JSON-RPC request over LAN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanRpcRequest {
    pub command: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Outgoing JSON-RPC response over LAN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanRpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LanRpcError>,
}

/// Structured error in JSON-RPC response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}
