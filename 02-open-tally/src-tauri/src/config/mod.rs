use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app_name: String,
    pub version: String,
    pub environment: String,
    pub database_path: PathBuf,
    pub port_9000_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "OpenTally".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            environment: std::env::var("OPENTALLY_ENV")
                .unwrap_or_else(|_| "development".to_string()),
            database_path: PathBuf::from("opentally_dev.db"),
            // Interoperability listener disabled pending ADR-002 framework selection
            port_9000_enabled: false,
        }
    }
}

impl AppConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_db_path(mut self, path: PathBuf) -> Self {
        self.database_path = path;
        self
    }
}
