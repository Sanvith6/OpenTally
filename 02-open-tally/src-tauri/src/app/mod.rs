use crate::config::AppConfig;
use crate::database::DatabaseManager;
use chrono::{DateTime, Utc};
use std::sync::Arc;

/// Global application state shared across Tauri IPC commands.
pub struct AppState {
    pub config: AppConfig,
    pub db: Arc<DatabaseManager>,
    pub started_at: DateTime<Utc>,
    pub active_company_id: std::sync::RwLock<Option<String>>,
}

impl AppState {
    pub fn new(config: AppConfig, db: Arc<DatabaseManager>) -> Self {
        Self {
            config,
            db,
            started_at: Utc::now(),
            active_company_id: std::sync::RwLock::new(None),
        }
    }
}
