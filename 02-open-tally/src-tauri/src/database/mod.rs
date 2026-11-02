use crate::errors::OpenTallyError;
pub mod migrations;
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Row, Sqlite,
};
use std::path::Path;
use std::str::FromStr;
use tracing::{info, warn};

/// Database health status separating operational engine status from security/encryption readiness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseHealth {
    pub is_connected: bool,
    pub engine: String,
    pub journal_mode: String,
    pub foreign_keys_enabled: bool,
    pub schema_version: String,
    // Security & Encryption status (strictly separated from operational health)
    pub encryption_enabled: bool,
    pub encryption_status: String,
    pub key_management: String,
}

/// Core Database Manager handling connection lifecycle, connection pooling,
/// PRAGMA setup (WAL mode), and schema migrations.
#[derive(Clone)]
pub struct DatabaseManager {
    pool: Pool<Sqlite>,
}

impl DatabaseManager {
    /// Initialize a database connection pool from a file path or in-memory URI.
    ///
    /// Applies required PRAGMAs in accordance with ADR-001:
    /// - `PRAGMA journal_mode = WAL;`
    /// - `PRAGMA synchronous = NORMAL;`
    /// - `PRAGMA foreign_keys = ON;`
    /// - `PRAGMA busy_timeout = 5000;`
    pub async fn init(db_path: &Path) -> Result<Self, OpenTallyError> {
        let path_str = db_path.to_str().ok_or_else(|| {
            OpenTallyError::Config("Database path contains invalid UTF-8 characters".to_string())
        })?;

        // Note regarding SQLCipher integration:
        // ADR-001 accepted SQLite + WAL + SQLCipher. While full page-level encryption
        // requires formalizing OS keyring passphrase derivation (e.g., Windows DPAPI),
        // this manager abstracts the connection lifecycle and enforces WAL mode.
        if !path_str.starts_with("sqlite:") && path_str != ":memory:" {
            info!("Initializing SQLite storage at: {}", path_str);
        }

        let is_in_memory = path_str == ":memory:" || path_str.contains("mode=memory");
        let options = if is_in_memory {
            SqliteConnectOptions::from_str(":memory:")?
        } else {
            SqliteConnectOptions::new().filename(db_path)
        }
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_millis(5000));

        // For in-memory testing, restrict pool to 1 connection to prevent isolated memory DBs
        let max_connections = if is_in_memory { 1 } else { 10 };

        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(options)
            .await?;

        let manager = Self { pool };
        manager.run_initial_migrations().await?;

        info!("Database initialized successfully with WAL journal mode.");
        Ok(manager)
    }

    /// Access the underlying SQLx connection pool.
    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }

    /// Run minimal initial foundational schema migrations within an explicit ACID transaction.
    async fn run_initial_migrations(&self) -> Result<(), OpenTallyError> {
        let mut tx = self.pool.begin().await?;

        // 1. Create schema version metadata table and app state table if not exists
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _opentally_metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                schema_version TEXT NOT NULL,
                installed_at TEXT NOT NULL,
                app_version TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS _opentally_app_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // 2. Check if foundational metadata entry exists
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM _opentally_metadata WHERE id = 1")
                .fetch_one(&mut *tx)
                .await?;

        if count == 0 {
            let now = chrono::Utc::now().to_rfc3339();
            let app_version = env!("CARGO_PKG_VERSION");
            sqlx::query(
                r#"
                INSERT INTO _opentally_metadata (id, schema_version, installed_at, app_version)
                VALUES (1, '0.1.0-sprint0', $1, $2);
                "#,
            )
            .bind(now)
            .bind(app_version)
            .execute(&mut *tx)
            .await?;
            info!(
                "Applied initial migration: seeded _opentally_metadata with schema 0.1.0-sprint0"
            );
        } else {
            info!("Found existing schema metadata; database preserved without mutation.");
        }

        tx.commit().await?;
        Ok(())
    }

    /// Verify database connectivity and verify runtime PRAGMA settings.
    pub async fn check_health(&self) -> Result<DatabaseHealth, OpenTallyError> {
        // Check WAL mode
        let journal_mode_row = sqlx::query("PRAGMA journal_mode;")
            .fetch_one(&self.pool)
            .await?;
        let journal_mode: String = journal_mode_row.get(0);

        // Check foreign keys enforcement
        let fk_row = sqlx::query("PRAGMA foreign_keys;")
            .fetch_one(&self.pool)
            .await?;
        let foreign_keys_enabled: bool = fk_row.get::<i64, _>(0) == 1;

        // Check metadata record
        let version_row =
            sqlx::query("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_optional(&self.pool)
                .await?;

        let schema_version = match version_row {
            Some(row) => row.get("schema_version"),
            None => {
                warn!("Metadata record missing from database");
                "unversioned".to_string()
            }
        };

        Ok(DatabaseHealth {
            is_connected: true,
            engine: "SQLite 3 (via sqlx)".to_string(),
            journal_mode,
            foreign_keys_enabled,
            schema_version,
            // Strictly distinguish operational health from encryption status
            encryption_enabled: false,
            encryption_status: "unencrypted (standard SQLite file; SQLCipher not active)"
                .to_string(),
            key_management: "unconfigured (OS keyring / DPAPI derivation pending ADR)".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_in_memory_database_initialization() {
        let db = DatabaseManager::init(&PathBuf::from(":memory:"))
            .await
            .expect("Failed to initialize in-memory database");

        let health = db.check_health().await.expect("Failed health check");
        assert!(health.is_connected);
        assert_eq!(health.schema_version, "0.1.0-sprint0");
        assert_eq!(health.foreign_keys_enabled, true);
        assert_eq!(health.encryption_enabled, false);
        assert!(!health.journal_mode.is_empty());
    }

    #[tokio::test]
    async fn test_metadata_table_persistence() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("test_opentally.db");

        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to initialize file database");

        let health = db.check_health().await.expect("Failed health check");
        assert!(health.is_connected);
        assert_eq!(health.journal_mode.to_lowercase(), "wal");
        assert_eq!(health.foreign_keys_enabled, true);
        assert_eq!(health.schema_version, "0.1.0-sprint0");
        assert_eq!(health.encryption_enabled, false);
    }

    #[tokio::test]
    async fn test_database_preservation_across_restarts() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("test_preservation.db");

        // 1. Initial initialization
        {
            let db = DatabaseManager::init(&db_path)
                .await
                .expect("Initial init failed");

            // Create test user table and insert dummy record
            sqlx::query("CREATE TABLE user_records (id INTEGER PRIMARY KEY, note TEXT);")
                .execute(db.pool())
                .await
                .expect("Failed to create user table");

            sqlx::query(
                "INSERT INTO user_records (id, note) VALUES (42, 'Existing accounting data');",
            )
            .execute(db.pool())
            .await
            .expect("Failed to insert user record");
        } // Connection pool dropped here

        // 2. Restart and re-initialize against the exact same file
        {
            let db = DatabaseManager::init(&db_path)
                .await
                .expect("Second init failed");

            let health = db.check_health().await.expect("Second health check failed");
            assert_eq!(health.schema_version, "0.1.0-sprint0");

            // Verify user data was preserved without deletion or recreation
            let note: String = sqlx::query_scalar("SELECT note FROM user_records WHERE id = 42;")
                .fetch_one(db.pool())
                .await
                .expect("Failed to query user record after restart");

            assert_eq!(note, "Existing accounting data");
        }
    }
}
