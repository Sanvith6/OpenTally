use serde::{Serialize, Serializer};
use thiserror::Error;

/// Core error types for the OpenTally application foundation.
#[derive(Debug, Error)]
pub enum OpenTallyError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Integrity violation: {0}")]
    IntegrityViolation(String),

    #[error("Voucher number collision: {0}")]
    VoucherNumberCollision(String),

    #[error("Internal system error: {0}")]
    Internal(String),
}

// Serialize errors for Tauri IPC boundary transmission
impl Serialize for OpenTallyError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
