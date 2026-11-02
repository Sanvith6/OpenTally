use crate::errors::OpenTallyError;
use sqlx::{Pool, Sqlite};

/// Sprint 1 Database DDL Definition (v1.0.2)
/// Defines companies, company_settings, account_groups, ledgers, and opening_balances
pub const SPRINT1_DDL: &str = r#"
-- 1. COMPANIES MASTER
CREATE TABLE companies (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    mailing_name TEXT NOT NULL,
    country TEXT NOT NULL,
    state TEXT NOT NULL,
    financial_year_begins TEXT NOT NULL,
    books_beginning_from TEXT NOT NULL,
    currency_symbol TEXT NOT NULL DEFAULT '₹',
    currency_formal_name TEXT NOT NULL DEFAULT 'INR',
    currency_decimal_places INTEGER NOT NULL DEFAULT 2 CHECK (currency_decimal_places BETWEEN 0 AND 4),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived', 'closed')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_company_name UNIQUE (name),
    CONSTRAINT chk_fy_date CHECK (financial_year_begins GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    CONSTRAINT chk_books_date CHECK (books_beginning_from GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    CONSTRAINT chk_books_after_fy CHECK (books_beginning_from >= financial_year_begins),
    CONSTRAINT chk_books_within_fy CHECK (books_beginning_from <= date(financial_year_begins, '+1 year', '-1 day'))
);

-- 2. COMPANY SETTINGS (KEY-VALUE)
CREATE TABLE company_settings (
    company_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, key),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
);

-- 3. ACCOUNT GROUPS HIERARCHY
CREATE TABLE account_groups (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    parent_id TEXT,
    nature TEXT NOT NULL CHECK (nature IN ('Assets', 'Liabilities', 'Income', 'Expenses')),
    is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
    is_predefined INTEGER NOT NULL DEFAULT 0 CHECK (is_predefined IN (0, 1)),
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK (is_hidden IN (0, 1)),
    affects_gross_profit INTEGER NOT NULL DEFAULT 0 CHECK (affects_gross_profit IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, name),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, parent_id) REFERENCES account_groups(company_id, id) ON DELETE CASCADE,
    CONSTRAINT chk_primary_parent CHECK (
        (is_primary = 1 AND parent_id IS NULL) OR
        (is_primary = 0 AND parent_id IS NOT NULL)
    )
);

-- 4. LEDGERS MASTER
CREATE TABLE ledgers (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    description TEXT,
    is_reserved INTEGER NOT NULL DEFAULT 0 CHECK (is_reserved IN (0, 1)),
    ledger_classification TEXT NOT NULL DEFAULT 'standard' CHECK (ledger_classification IN ('standard', 'cash', 'profit_and_loss')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, name),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, group_id) REFERENCES account_groups(company_id, id) ON DELETE CASCADE,
    CONSTRAINT chk_reserved_classification CHECK (
        (is_reserved = 1 AND ledger_classification IN ('cash', 'profit_and_loss')) OR
        (is_reserved = 0 AND ledger_classification = 'standard')
    )
);

CREATE UNIQUE INDEX uq_company_reserved_ledger
ON ledgers(company_id, ledger_classification)
WHERE ledger_classification IN ('cash', 'profit_and_loss');

-- 5. OPENING BALANCES
CREATE TABLE opening_balances (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    ledger_id TEXT NOT NULL,
    financial_year_begins TEXT NOT NULL,
    balance_type TEXT NOT NULL CHECK (balance_type IN ('Dr', 'Cr')),
    amount TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, ledger_id, financial_year_begins),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE CASCADE,
    CONSTRAINT chk_opening_amount_non_negative CHECK (
        length(amount) > 0
        AND amount GLOB '[0-9]*'
        AND amount GLOB '*[0-9]'
        AND amount NOT GLOB '*[^0-9.]*'
        AND amount NOT GLOB '*.*.*'
    )
);
"#;

/// Sprint 1 Triggers for Reserved Ledgers
pub const SPRINT1_TRIGGERS: &str = r#"
CREATE TRIGGER trg_validate_reserved_ledger_insert
BEFORE INSERT ON ledgers
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Cash ledger must have is_reserved=1 and belong to Cash-in-Hand group')
    WHERE NEW.ledger_classification = 'cash'
      AND (
          NEW.is_reserved != 1
          OR NEW.group_id IS NOT (SELECT id FROM account_groups WHERE company_id = NEW.company_id AND name = 'Cash-in-Hand')
      );

    SELECT RAISE(ABORT, 'Profit & Loss A/c ledger must have is_reserved=1 and belong to Profit & Loss A/c (System) group')
    WHERE NEW.ledger_classification = 'profit_and_loss'
      AND (
          NEW.is_reserved != 1
          OR NEW.group_id IS NOT (SELECT id FROM account_groups WHERE company_id = NEW.company_id AND name = 'Profit & Loss A/c (System)')
      );

    SELECT RAISE(ABORT, 'Standard ledgers cannot belong to hidden system groups')
    WHERE NEW.ledger_classification = 'standard'
      AND EXISTS (
          SELECT 1 FROM account_groups 
          WHERE company_id = NEW.company_id 
            AND id = NEW.group_id 
            AND is_hidden = 1
      );
END;

CREATE TRIGGER trg_prevent_reserved_ledger_mutation
BEFORE UPDATE ON ledgers
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Reserved system ledgers cannot be renamed, re-parented, re-assigned, or de-reserved')
    WHERE OLD.is_reserved = 1
      AND (
          NEW.name != OLD.name
          OR NEW.group_id != OLD.group_id
          OR NEW.company_id != OLD.company_id
          OR NEW.is_reserved != 1
          OR NEW.ledger_classification != OLD.ledger_classification
          OR NEW.id != OLD.id
      );

    SELECT RAISE(ABORT, 'Standard ledgers cannot be converted to reserved ledgers')
    WHERE OLD.is_reserved = 0
      AND (
          NEW.is_reserved != 0
          OR NEW.ledger_classification != 'standard'
          OR EXISTS (
              SELECT 1 FROM account_groups 
              WHERE company_id = NEW.company_id 
                AND id = NEW.group_id 
                AND is_hidden = 1
          )
      );
END;

CREATE TRIGGER trg_prevent_reserved_ledger_deletion
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN OLD.is_reserved = 1 AND EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
BEGIN
    SELECT RAISE(ABORT, 'Reserved system ledgers (Cash, Profit & Loss A/c) cannot be deleted');
END;

CREATE TRIGGER trg_prevent_group_delete_with_children
BEFORE DELETE ON account_groups
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND (
      EXISTS (SELECT 1 FROM account_groups WHERE company_id = OLD.company_id AND parent_id = OLD.id)
      OR EXISTS (SELECT 1 FROM ledgers WHERE company_id = OLD.company_id AND group_id = OLD.id)
  )
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete account group with child groups or ledgers');
END;

CREATE TRIGGER trg_prevent_ledger_delete_with_opening_balances
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND EXISTS (SELECT 1 FROM opening_balances WHERE company_id = OLD.company_id AND ledger_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete ledger with existing opening balances');
END;
"#;

/// Execute the Sprint 1 migration (0.1.0-sprint0 -> 0.2.0-sprint1).
pub async fn run_sprint1_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    run_sprint1_migration_with_payload(pool, SPRINT1_DDL, SPRINT1_TRIGGERS).await
}

/// Execute custom migration DDL/triggers for testing atomic rollback semantics.
pub async fn run_sprint1_migration_with_payload(
    pool: &Pool<Sqlite>,
    ddl: &str,
    triggers: &str,
) -> Result<(), OpenTallyError> {
    // 1. Acquire dedicated connection from pool
    // (foreign_keys = true is guaranteed active via SqliteConnectOptions on pool initialization)
    let mut conn = pool.acquire().await?;

    // 2. Read and verify current metadata version
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *conn)
            .await?;

    if current_version != "0.1.0-sprint0" {
        if current_version == "0.2.0-sprint1" || current_version == "0.2.1-sprint1" {
            // Already migrated
            return Ok(());
        }
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.1.0-sprint0', found '{}'",
            current_version
        )));
    }

    // 3. Begin IMMEDIATE transaction directly on acquired connection
    // Guarantees exclusive write lock acquisition before DDL execution
    sqlx::raw_sql("BEGIN IMMEDIATE;")
        .execute(&mut *conn)
        .await?;

    // 4. Execute migration payload within guarded block
    let migration_res: Result<(), OpenTallyError> = async {
        // Multi-statement DDL execution using sqlx::raw_sql (calls sqlite3_exec under the hood)
        sqlx::raw_sql(ddl).execute(&mut *conn).await?;

        // Multi-statement triggers and partial unique indexes execution
        if !triggers.trim().is_empty() {
            sqlx::raw_sql(triggers).execute(&mut *conn).await?;
        }

        // 5. Update metadata table (preserving original installed_at timestamp)
        let app_version = env!("CARGO_PKG_VERSION");
        sqlx::query(
            "UPDATE _opentally_metadata 
             SET schema_version = '0.2.0-sprint1', app_version = $1 
             WHERE id = 1",
        )
        .bind(app_version)
        .execute(&mut *conn)
        .await?;

        // 6. Verify Foreign Key Integrity
        let violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await?;

        if !violations.is_empty() {
            return Err(OpenTallyError::Migration(format!(
                "Foreign key integrity check failed with {} violations",
                violations.len()
            )));
        }

        Ok(())
    }
    .await;

    // 7. Commit or Rollback based on execution result
    match migration_res {
        Ok(()) => {
            sqlx::raw_sql("COMMIT;").execute(&mut *conn).await?;
            tracing::info!("Migration 0.2.0-sprint1 committed successfully.");
            Ok(())
        }
        Err(e) => {
            tracing::warn!("Migration failed; rolling back transaction: {:?}", e);
            let _ = sqlx::raw_sql("ROLLBACK;").execute(&mut *conn).await;
            Err(e)
        }
    }
}

pub const STAGE3_PATCH_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    ledger_id TEXT,
    financial_year_begins TEXT,
    action TEXT NOT NULL CHECK (action IN ('CREATE', 'UPDATE', 'CLEAR', 'DELETE')),
    old_values TEXT,
    new_values TEXT,
    performed_by TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_audit_log_entity
ON audit_log(company_id, entity_type, entity_id);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
ON audit_log(company_id, timestamp);

CREATE TRIGGER IF NOT EXISTS trg_prevent_audit_log_mutation
BEFORE UPDATE ON audit_log
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_audit_log_deletion
BEFORE DELETE ON audit_log
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
BEGIN
    SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_predefined_group_deletion
BEFORE DELETE ON account_groups
FOR EACH ROW
WHEN OLD.is_predefined = 1 AND EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
BEGIN
    SELECT RAISE(ABORT, 'Predefined system account groups cannot be deleted');
END;
"#;

/// Execute the Stage 3 patch migration (0.2.0-sprint1 -> 0.2.1-sprint1).
pub async fn run_stage3_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut conn = pool.acquire().await?;
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *conn)
            .await?;

    if current_version == "0.2.1-sprint1" {
        return Ok(());
    }

    if current_version != "0.2.0-sprint1" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.2.0-sprint1' for Stage 3 patch, found '{}'",
            current_version
        )));
    }

    sqlx::raw_sql("BEGIN IMMEDIATE;")
        .execute(&mut *conn)
        .await?;

    let migration_res: Result<(), OpenTallyError> = async {
        sqlx::raw_sql(STAGE3_PATCH_DDL).execute(&mut *conn).await?;

        let app_version = env!("CARGO_PKG_VERSION");
        sqlx::query(
            "UPDATE _opentally_metadata 
             SET schema_version = '0.2.1-sprint1', app_version = $1 
             WHERE id = 1",
        )
        .bind(app_version)
        .execute(&mut *conn)
        .await?;

        let violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await?;

        if !violations.is_empty() {
            return Err(OpenTallyError::Migration(format!(
                "Foreign key integrity check failed with {} violations",
                violations.len()
            )));
        }

        Ok(())
    }
    .await;

    match migration_res {
        Ok(()) => {
            sqlx::raw_sql("COMMIT;").execute(&mut *conn).await?;
            tracing::info!("Migration 0.2.1-sprint1 committed successfully.");
            Ok(())
        }
        Err(e) => {
            let _ = sqlx::raw_sql("ROLLBACK;").execute(&mut *conn).await;
            tracing::error!(
                "Migration 0.2.1-sprint1 failed, transaction rolled back: {}",
                e
            );
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseManager;

    struct TestDb {
        pub db: DatabaseManager,
        _temp_dir: tempfile::TempDir,
    }

    impl TestDb {
        pub async fn new() -> Self {
            let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
            let db_path = temp_dir.path().join("opentally_sprint1_test.db");
            let db = DatabaseManager::init(&db_path)
                .await
                .expect("Failed to init database");
            Self {
                db,
                _temp_dir: temp_dir,
            }
        }
    }

    /// TEST-MIG-01: Migration execution against an existing Sprint 0 database,
    /// upgrading schema_version to '0.2.0-sprint1', preserving existing installed_at
    /// timestamp and existing data, ensuring PRAGMA foreign_key_check passes, and
    /// verifying that all 5 tables exist and have composite primary keys.
    #[tokio::test]
    async fn test_mig_01_migration_execution_and_schema_verification() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();

        // 1. Verify existing Sprint 0 database
        let initial_version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch initial schema version");
        assert_eq!(initial_version, "0.1.0-sprint0");

        let initial_installed_at: String =
            sqlx::query_scalar("SELECT installed_at FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch initial installed_at");

        // Insert pre-existing user data to verify data preservation
        sqlx::query("CREATE TABLE pre_existing_user_data (id INTEGER PRIMARY KEY, note TEXT);")
            .execute(pool)
            .await
            .expect("Create pre-existing table");
        sqlx::query(
            "INSERT INTO pre_existing_user_data (id, note) VALUES (42, 'Preserve Existing Data');",
        )
        .execute(pool)
        .await
        .expect("Insert pre-existing data");

        // 2. Run migration
        run_sprint1_migration(pool)
            .await
            .expect("Migration from 0.1.0-sprint0 to 0.2.0-sprint1 must succeed");

        // 3. Verify schema_version upgraded to '0.2.0-sprint1'
        let updated_version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch updated schema version");
        assert_eq!(updated_version, "0.2.0-sprint1");

        // 4. Verify installed_at timestamp is preserved
        let updated_installed_at: String =
            sqlx::query_scalar("SELECT installed_at FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch updated installed_at");
        assert_eq!(initial_installed_at, updated_installed_at);

        // 5. Verify pre-existing data preserved
        let note: String =
            sqlx::query_scalar("SELECT note FROM pre_existing_user_data WHERE id = 42")
                .fetch_one(pool)
                .await
                .expect("Fetch note");
        assert_eq!(note, "Preserve Existing Data");

        // 6. Verify PRAGMA foreign_key_check passes with 0 violations
        let violations: Vec<(String, Option<i64>, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check;")
                .fetch_all(pool)
                .await
                .expect("Run foreign_key_check");
        assert!(
            violations.is_empty(),
            "Expected 0 FK violations, found {:?}",
            violations
        );

        // 7. Verify all 5 tables exist in sqlite_master
        let expected_tables = vec![
            "companies",
            "company_settings",
            "account_groups",
            "ledgers",
            "opening_balances",
        ];
        for table in &expected_tables {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $1",
            )
            .bind(table)
            .fetch_one(pool)
            .await
            .expect("Check table existence");
            assert_eq!(exists, 1, "Table {} must exist", table);
        }

        // 8. Verify composite primary keys on all tables
        async fn get_pk_columns(pool: &Pool<Sqlite>, table: &str) -> Vec<String> {
            let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
                sqlx::query_as(&format!("PRAGMA table_info({});", table))
                    .fetch_all(pool)
                    .await
                    .expect("Fetch table_info");

            let mut pk_cols: Vec<(i64, String)> = rows
                .into_iter()
                .filter(|r| r.5 > 0)
                .map(|r| (r.5, r.1))
                .collect();
            pk_cols.sort_by_key(|k| k.0);
            pk_cols.into_iter().map(|k| k.1).collect()
        }

        assert_eq!(get_pk_columns(pool, "companies").await, vec!["id"]);
        assert_eq!(
            get_pk_columns(pool, "company_settings").await,
            vec!["company_id", "key"]
        );
        assert_eq!(
            get_pk_columns(pool, "account_groups").await,
            vec!["company_id", "id"]
        );
        assert_eq!(
            get_pk_columns(pool, "ledgers").await,
            vec!["company_id", "id"]
        );
        assert_eq!(
            get_pk_columns(pool, "opening_balances").await,
            vec!["company_id", "id"]
        );
    }

    /// TEST-MIG-02: Migration failure and atomic rollback: inject an intentional failure
    /// (malformed DDL syntax), assert that the transaction rolls back, schema_version
    /// remains '0.1.0-sprint0', and zero partial Sprint 1 tables are left behind in sqlite_master.
    #[tokio::test]
    async fn test_mig_02_migration_failure_and_atomic_rollback() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();

        let initial_version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch initial schema version");
        assert_eq!(initial_version, "0.1.0-sprint0");

        // Inject intentional malformed DDL syntax error mid-migration
        let malformed_ddl = format!(
            "{}\nCREATE TABLE intentional_crash_table (;;; INVALID SQL SYNTAX",
            SPRINT1_DDL
        );

        let result =
            run_sprint1_migration_with_payload(pool, &malformed_ddl, SPRINT1_TRIGGERS).await;
        assert!(
            result.is_err(),
            "Migration with malformed DDL must return Err"
        );
        let err = result.unwrap_err();
        // In the green phase, this must be a SQL execution error, not a stub error
        assert!(
            err.to_string().to_lowercase().contains("syntax")
                || err.to_string().to_lowercase().contains("near")
                || err.to_string().contains("Database error"),
            "Expected SQL syntax or database error from mid-migration failure, got: {}",
            err
        );

        // Verify schema_version remains '0.1.0-sprint0'
        let current_version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .expect("Fetch current schema version");
        assert_eq!(current_version, "0.1.0-sprint0");

        // Assert zero partial Sprint 1 tables are left behind in sqlite_master
        let partial_tables: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('companies', 'company_settings', 'account_groups', 'ledgers', 'opening_balances', 'intentional_crash_table')"
        )
        .fetch_one(pool)
        .await
        .expect("Count partial tables");
        assert_eq!(
            partial_tables, 0,
            "No partial Sprint 1 tables should exist after rollback"
        );
    }

    /// TEST-ISO-01: Isolation test: verify composite foreign key prevents assigning
    /// a ledger to an account group of a different company.
    #[tokio::test]
    async fn test_iso_01_composite_fk_prevents_cross_company_ledger_group() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Company A Mailing', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                    ('cmp_b', 'Company B', 'Company B Mailing', 'India', 'Karnataka', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert companies");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_a', 'cmp_a', 'Direct Expenses', NULL, 'Expenses', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert group in Company A");

        // Attempt to insert ledger belonging to Company B referencing Group in Company A
        let res = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_b1', 'cmp_b', 'grp_a', 'Freight Charges', 0, 'standard');"
        )
        .execute(pool)
        .await;

        assert!(
            res.is_err(),
            "Cross-company foreign key assignment must fail"
        );
        let err = res.unwrap_err();
        assert!(
            err.to_string()
                .to_lowercase()
                .contains("foreign key constraint failed"),
            "Expected foreign key constraint failure, got: {}",
            err
        );

        // Verify intra-company ledger insert succeeds
        let valid_res = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_a1', 'cmp_a', 'grp_a', 'Freight Charges', 0, 'standard');"
        )
        .execute(pool)
        .await;
        assert!(
            valid_res.is_ok(),
            "Valid intra-company ledger insert must succeed"
        );
    }

    /// TEST-ISO-02: Isolation test: verify composite foreign key prevents assigning
    /// an opening balance to a ledger of a different company.
    #[tokio::test]
    async fn test_iso_02_composite_fk_prevents_cross_company_opening_balance() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Company A Mailing', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                    ('cmp_b', 'Company B', 'Company B Mailing', 'India', 'Karnataka', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert companies");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_a', 'cmp_a', 'Sundry Debtors', NULL, 'Assets', 1),
                    ('grp_b', 'cmp_b', 'Sundry Debtors', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert groups");

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_a1', 'cmp_a', 'grp_a', 'Customer Alpha', 0, 'standard'),
                    ('led_b1', 'cmp_b', 'grp_b', 'Customer Beta', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledgers");

        // Attempt to insert opening balance in Company B pointing to ledger in Company A ('led_a1')
        let res = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_cross', 'cmp_b', 'led_a1', '2026-04-01', 'Dr', '1500.00');"
        )
        .execute(pool)
        .await;

        assert!(
            res.is_err(),
            "Cross-company opening balance assignment must fail"
        );
        let err = res.unwrap_err();
        assert!(
            err.to_string()
                .to_lowercase()
                .contains("foreign key constraint failed"),
            "Expected foreign key constraint failure, got: {}",
            err
        );

        // Verify valid intra-company opening balance succeeds
        let valid_res = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_valid', 'cmp_a', 'led_a1', '2026-04-01', 'Dr', '1500.00');"
        )
        .execute(pool)
        .await;
        assert!(
            valid_res.is_ok(),
            "Valid intra-company opening balance must succeed"
        );
    }

    /// TEST-RES-01: Partial unique index test: verify a company cannot have more
    /// than one reserved 'cash' ledger, while other companies may each have their own.
    #[tokio::test]
    async fn test_res_01_partial_unique_index_prevents_duplicate_cash_ledger() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01'),
                    ('cmp_b', 'Company B', 'Mailing B', 'India', 'Karnataka', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert companies");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cash_a', 'cmp_a', 'Cash-in-Hand', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert Cash group in Company A");

        // Insert first reserved cash ledger in Company A
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash_1', 'cmp_a', 'grp_cash_a', 'Cash', 1, 'cash');"
        )
        .execute(pool)
        .await
        .expect("Insert first cash ledger");

        // Attempt to insert second reserved cash ledger in Company A
        let res = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash_2', 'cmp_a', 'grp_cash_a', 'Petty Cash Reserved', 1, 'cash');"
        )
        .execute(pool)
        .await;

        assert!(
            res.is_err(),
            "Second reserved cash ledger in same company must fail unique index"
        );
        let err = res.unwrap_err();
        assert!(
            err.to_string()
                .to_lowercase()
                .contains("unique constraint failed"),
            "Expected unique constraint violation, got: {}",
            err
        );

        // Verify Company B CAN insert its own reserved cash ledger
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cash_b', 'cmp_b', 'Cash-in-Hand', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert Cash group in Company B");

        let res_b = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash_b', 'cmp_b', 'grp_cash_b', 'Cash', 1, 'cash');"
        )
        .execute(pool)
        .await;
        assert!(
            res_b.is_ok(),
            "Company B must be able to have its own reserved cash ledger"
        );
    }

    /// TEST-RES-02: Partial unique index test: verify a company cannot have more
    /// than one reserved 'profit_and_loss' ledger.
    #[tokio::test]
    async fn test_res_02_partial_unique_index_prevents_duplicate_pnl_ledger() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        // Insert hidden system group Profit & Loss A/c (System)
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_hidden)
             VALUES ('grp_pnl_a', 'cmp_a', 'Profit & Loss A/c (System)', NULL, 'Expenses', 1, 1);"
        )
        .execute(pool)
        .await
        .expect("Insert P&L group");

        // Insert first reserved P&L ledger
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_pnl_1', 'cmp_a', 'grp_pnl_a', 'Profit & Loss A/c', 1, 'profit_and_loss');"
        )
        .execute(pool)
        .await
        .expect("Insert first P&L ledger");

        // Attempt to insert second reserved P&L ledger
        let res = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_pnl_2', 'cmp_a', 'grp_pnl_a', 'P&L Secondary', 1, 'profit_and_loss');"
        )
        .execute(pool)
        .await;

        assert!(
            res.is_err(),
            "Second reserved P&L ledger in same company must fail unique index"
        );
        let err = res.unwrap_err();
        assert!(
            err.to_string()
                .to_lowercase()
                .contains("unique constraint failed"),
            "Expected unique constraint violation, got: {}",
            err
        );
    }

    /// TEST-RES-03: Trigger test: verify reserved Cash or P&L ledger cannot be renamed,
    /// re-parented, reclassified, or converted from standard.
    #[tokio::test]
    async fn test_res_03_trigger_prevents_mutation_of_reserved_ledger() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cash', 'cmp_a', 'Cash-in-Hand', NULL, 'Assets', 1),
                    ('grp_bank', 'cmp_a', 'Bank Accounts', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert groups");

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash', 'cmp_a', 'grp_cash', 'Cash', 1, 'cash'),
                    ('led_std', 'cmp_a', 'grp_bank', 'HDFC Bank', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledgers");

        // 1. Rename attempt
        let res_rename =
            sqlx::query("UPDATE ledgers SET name = 'Petty Cash' WHERE id = 'led_cash'")
                .execute(pool)
                .await;
        assert!(res_rename.is_err(), "Renaming reserved ledger must fail");
        assert!(res_rename
            .unwrap_err()
            .to_string()
            .contains("cannot be renamed, re-parented, re-assigned, or de-reserved"));

        // 2. Re-parent attempt
        let res_reparent =
            sqlx::query("UPDATE ledgers SET group_id = 'grp_bank' WHERE id = 'led_cash'")
                .execute(pool)
                .await;
        assert!(
            res_reparent.is_err(),
            "Re-parenting reserved ledger must fail"
        );
        assert!(res_reparent
            .unwrap_err()
            .to_string()
            .contains("cannot be renamed, re-parented, re-assigned, or de-reserved"));

        // 3. De-reserve attempt
        let res_dereserve = sqlx::query("UPDATE ledgers SET is_reserved = 0 WHERE id = 'led_cash'")
            .execute(pool)
            .await;
        assert!(
            res_dereserve.is_err(),
            "De-reserving reserved ledger must fail"
        );
        assert!(res_dereserve
            .unwrap_err()
            .to_string()
            .contains("cannot be renamed, re-parented, re-assigned, or de-reserved"));

        // 4. Change classification attempt
        let res_class = sqlx::query(
            "UPDATE ledgers SET ledger_classification = 'standard' WHERE id = 'led_cash'",
        )
        .execute(pool)
        .await;
        assert!(
            res_class.is_err(),
            "Changing classification of reserved ledger must fail"
        );
        assert!(res_class
            .unwrap_err()
            .to_string()
            .contains("cannot be renamed, re-parented, re-assigned, or de-reserved"));

        // 5. Converting standard ledger to reserved attempt
        let res_convert = sqlx::query(
            "UPDATE ledgers SET is_reserved = 1, ledger_classification = 'cash' WHERE id = 'led_std'",
        )
        .execute(pool)
        .await;
        assert!(
            res_convert.is_err(),
            "Converting standard ledger to reserved must fail"
        );
        assert!(res_convert
            .unwrap_err()
            .to_string()
            .contains("Standard ledgers cannot be converted to reserved ledgers"));
    }

    /// TEST-RES-04: Trigger test: verify reserved Cash or P&L ledger cannot be deleted,
    /// while standard ledgers can be deleted.
    #[tokio::test]
    async fn test_res_04_trigger_prevents_deletion_of_reserved_ledger() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cash', 'cmp_a', 'Cash-in-Hand', NULL, 'Assets', 1),
                    ('grp_std', 'cmp_a', 'Current Assets', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert groups");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_hidden)
             VALUES ('grp_pnl', 'cmp_a', 'Profit & Loss A/c (System)', NULL, 'Expenses', 1, 1);"
        )
        .execute(pool)
        .await
        .expect("Insert P&L group");

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash', 'cmp_a', 'grp_cash', 'Cash', 1, 'cash'),
                    ('led_pnl', 'cmp_a', 'grp_pnl', 'Profit & Loss A/c', 1, 'profit_and_loss'),
                    ('led_std', 'cmp_a', 'grp_std', 'Test Standard', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledgers");

        // Attempt to delete reserved Cash
        let res_del_cash = sqlx::query("DELETE FROM ledgers WHERE id = 'led_cash'")
            .execute(pool)
            .await;
        assert!(
            res_del_cash.is_err(),
            "Deleting reserved Cash ledger must fail"
        );
        assert!(res_del_cash
            .unwrap_err()
            .to_string()
            .contains("Reserved system ledgers (Cash, Profit & Loss A/c) cannot be deleted"));

        // Attempt to delete reserved P&L
        let res_del_pnl = sqlx::query("DELETE FROM ledgers WHERE id = 'led_pnl'")
            .execute(pool)
            .await;
        assert!(
            res_del_pnl.is_err(),
            "Deleting reserved P&L ledger must fail"
        );
        assert!(res_del_pnl
            .unwrap_err()
            .to_string()
            .contains("Reserved system ledgers (Cash, Profit & Loss A/c) cannot be deleted"));

        // Deleting standard ledger should succeed
        let res_del_std = sqlx::query("DELETE FROM ledgers WHERE id = 'led_std'")
            .execute(pool)
            .await;
        assert!(res_del_std.is_ok(), "Deleting standard ledger must succeed");
    }

    /// TEST-RES-05: Trigger test: verify Cash ledger must belong to Cash-in-Hand group,
    /// P&L ledger must belong to Profit & Loss A/c (System) group, and standard ledgers
    /// cannot belong to hidden system groups.
    #[tokio::test]
    async fn test_res_05_trigger_enforces_group_rules_for_reserved_and_standard_ledgers() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cash', 'cmp_a', 'Cash-in-Hand', NULL, 'Assets', 1),
                    ('grp_bank', 'cmp_a', 'Bank Accounts', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert groups");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_hidden)
             VALUES ('grp_pnl', 'cmp_a', 'Profit & Loss A/c (System)', NULL, 'Expenses', 1, 1);"
        )
        .execute(pool)
        .await
        .expect("Insert hidden group");

        // 1. Insert Cash ledger into wrong group (Bank Accounts instead of Cash-in-Hand)
        let res_cash_wrong = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cash_err', 'cmp_a', 'grp_bank', 'Cash', 1, 'cash');"
        )
        .execute(pool)
        .await;
        assert!(
            res_cash_wrong.is_err(),
            "Cash ledger in non-Cash-in-Hand group must fail"
        );
        assert!(res_cash_wrong
            .unwrap_err()
            .to_string()
            .contains("Cash ledger must have is_reserved=1 and belong to Cash-in-Hand group"));

        // 2. Insert P&L ledger into wrong group
        let res_pnl_wrong = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_pnl_err', 'cmp_a', 'grp_cash', 'Profit & Loss A/c', 1, 'profit_and_loss');"
        )
        .execute(pool)
        .await;
        assert!(
            res_pnl_wrong.is_err(),
            "P&L ledger in non-P&L group must fail"
        );
        assert!(res_pnl_wrong.unwrap_err().to_string().contains(
            "Profit & Loss A/c ledger must have is_reserved=1 and belong to Profit & Loss A/c (System) group"
        ));

        // 3. Insert standard ledger into hidden system group
        let res_std_hidden = sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_std_err', 'cmp_a', 'grp_pnl', 'Illegal Standard Ledger', 0, 'standard');"
        )
        .execute(pool)
        .await;
        assert!(
            res_std_hidden.is_err(),
            "Standard ledger in hidden group must fail"
        );
        assert!(res_std_hidden
            .unwrap_err()
            .to_string()
            .contains("Standard ledgers cannot belong to hidden system groups"));
    }

    /// TEST-BAL-01: Foreign key RESTRICT test: verify attempting to delete a ledger
    /// with existing opening balances fails with foreign key violation (ON DELETE RESTRICT).
    #[tokio::test]
    async fn test_bal_01_fk_restrict_prevents_deleting_ledger_with_opening_balance() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_debtors', 'cmp_a', 'Sundry Debtors', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert group");

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_customer', 'cmp_a', 'grp_debtors', 'Customer 1', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledger");

        sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_cust', 'cmp_a', 'led_customer', '2026-04-01', 'Dr', '5000.00');"
        )
        .execute(pool)
        .await
        .expect("Insert opening balance");

        // Attempt to delete ledger with existing opening balance
        let res_del =
            sqlx::query("DELETE FROM ledgers WHERE company_id = 'cmp_a' AND id = 'led_customer'")
                .execute(pool)
                .await;
        assert!(
            res_del.is_err(),
            "Deleting ledger with opening balances must fail with RESTRICT"
        );
        let err = res_del.unwrap_err().to_string().to_lowercase();
        assert!(
            err.contains("cannot delete ledger with existing opening balances")
                || err.contains("foreign key constraint failed"),
            "Expected restriction error, got: {}",
            err
        );

        // Delete opening balance, then ledger deletion must succeed
        sqlx::query("DELETE FROM opening_balances WHERE company_id = 'cmp_a' AND ledger_id = 'led_customer'")
            .execute(pool)
            .await
            .expect("Delete opening balance");

        let res_del_after =
            sqlx::query("DELETE FROM ledgers WHERE company_id = 'cmp_a' AND id = 'led_customer'")
                .execute(pool)
                .await;
        assert!(
            res_del_after.is_ok(),
            "Deleting ledger after removing opening balance must succeed"
        );
    }

    /// TEST-BAL-02: Amount constraint check: verify inserting a negative opening balance
    /// is rejected by chk_opening_amount_non_negative.
    #[tokio::test]
    async fn test_bal_02_amount_constraint_rejects_negative_and_invalid_amounts() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Company A', 'Mailing A', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_debtors', 'cmp_a', 'Sundry Debtors', NULL, 'Assets', 1);",
        )
        .execute(pool)
        .await
        .expect("Insert group");

        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cust', 'cmp_a', 'grp_debtors', 'Customer 1', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledger");

        // Negative amount: "-50.00"
        // Test case: "-100.00" (invalid: negative)
        let res_neg = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_neg', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '-100.00');"
        )
        .execute(pool)
        .await;
        assert!(
            res_neg.is_err(),
            "Negative amount -100.00 must fail check constraint"
        );

        // Test case: empty string "" (invalid)
        let res_empty = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_empty', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '');"
        )
        .execute(pool)
        .await;
        assert!(
            res_empty.is_err(),
            "Empty amount must fail check constraint"
        );

        // Test case: "abc" (invalid: alphabetic)
        let res_text = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_text', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', 'abc');"
        )
        .execute(pool)
        .await;
        assert!(
            res_text.is_err(),
            "Alphabetic amount 'abc' must fail check constraint"
        );

        // Test case: "12.3.4" (invalid: multiple decimal points)
        let res_multidot = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_dot', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '12.3.4');"
        )
        .execute(pool)
        .await;
        assert!(
            res_multidot.is_err(),
            "Multiple decimal points '12.3.4' must fail check constraint"
        );

        // Test case: "." (invalid: decimal point without digits)
        let res_dot = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_bare_dot', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '.');"
        )
        .execute(pool)
        .await;
        assert!(
            res_dot.is_err(),
            "Bare decimal point '.' must fail check constraint"
        );

        // Test case: "100." (invalid: decimal point without following digits)
        let res_trailing_dot = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_trailing_dot', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '100.');"
        )
        .execute(pool)
        .await;
        assert!(
            res_trailing_dot.is_err(),
            "Trailing decimal point '100.' must fail check constraint"
        );

        // Test case: ".50" (invalid: decimal point without leading digits)
        let res_leading_dot = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_leading_dot', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '.50');"
        )
        .execute(pool)
        .await;
        assert!(
            res_leading_dot.is_err(),
            "Leading decimal point '.50' must fail check constraint"
        );

        // Test case: "100" (valid: integer format)
        let res_int = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_int', 'cmp_a', 'led_cust', '2026-04-01', 'Dr', '100');"
        )
        .execute(pool)
        .await;
        assert!(res_int.is_ok(), "Integer amount '100' must succeed");

        // Test case: "100.50" (valid: decimal format)
        let res_dec = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_dec', 'cmp_a', 'led_cust', '2026-04-02', 'Dr', '100.50');"
        )
        .execute(pool)
        .await;
        assert!(res_dec.is_ok(), "Decimal amount '100.50' must succeed");

        // Test case: "0" (valid: zero balance)
        let res_zero = sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_zero', 'cmp_a', 'led_cust', '2026-04-03', 'Dr', '0');"
        )
        .execute(pool)
        .await;
        assert!(res_zero.is_ok(), "Zero amount '0' must succeed");
    }

    /// TEST-CASCADE-01: Integration test for company cascade deletion and reserved ledger protection.
    /// Verifies that:
    /// - Direct deletion of reserved Cash is rejected.
    /// - Direct deletion of reserved P&L is rejected.
    /// - Direct deletion of a ledger with opening balances is rejected (ON DELETE RESTRICT).
    /// - Company deletion successfully cascades through all intended dependent tables
    ///   (companies, company_settings, account_groups, ledgers, opening_balances).
    /// - No unexpected foreign-key violations occur during company cascade.
    #[tokio::test]
    async fn test_company_cascade_deletion_and_reserved_ledger_protection() {
        let fixture = TestDb::new().await;
        let pool = fixture.db.pool();
        run_sprint1_migration(pool).await.expect("Migration failed");

        // 1. Create company
        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_cas', 'Cascade Corp', 'Cascade Mailing', 'India', 'Delhi', '2026-04-01', '2026-04-01');"
        )
        .execute(pool)
        .await
        .expect("Insert company");

        // 2. Insert company settings
        sqlx::query(
            "INSERT INTO company_settings (company_id, key, value)
             VALUES ('cmp_cas', 'rounding_method', 'half_up');",
        )
        .execute(pool)
        .await
        .expect("Insert company settings");

        // 3. Insert account groups (primary and child)
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_cas_assets', 'cmp_cas', 'Current Assets', NULL, 'Assets', 1),
                    ('grp_cas_cash', 'cmp_cas', 'Cash-in-Hand', 'grp_cas_assets', 'Assets', 0);",
        )
        .execute(pool)
        .await
        .expect("Insert asset groups");

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_hidden)
             VALUES ('grp_cas_pnl', 'cmp_cas', 'Profit & Loss A/c (System)', NULL, 'Liabilities', 1, 1);"
        )
        .execute(pool)
        .await
        .expect("Insert P&L system group");

        // 4. Insert ledgers: reserved Cash, reserved P&L, and a standard ledger
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_cas_cash', 'cmp_cas', 'grp_cas_cash', 'Cash', 1, 'cash'),
                    ('led_cas_pnl', 'cmp_cas', 'grp_cas_pnl', 'Profit & Loss A/c', 1, 'profit_and_loss'),
                    ('led_cas_std', 'cmp_cas', 'grp_cas_cash', 'Petty Cash Standard', 0, 'standard');"
        )
        .execute(pool)
        .await
        .expect("Insert ledgers");

        // 5. Insert opening balance referencing the standard ledger
        sqlx::query(
            "INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
             VALUES ('ob_cas_1', 'cmp_cas', 'led_cas_std', '2026-04-01', 'Dr', '500.00');"
        )
        .execute(pool)
        .await
        .expect("Insert opening balance");

        // 6. Direct deletion tests:
        // Direct deletion of reserved Cash must fail
        let del_cash = sqlx::query("DELETE FROM ledgers WHERE id = 'led_cas_cash'")
            .execute(pool)
            .await;
        assert!(del_cash.is_err(), "Direct deletion of Cash must fail");

        // Direct deletion of reserved P&L must fail
        let del_pnl = sqlx::query("DELETE FROM ledgers WHERE id = 'led_cas_pnl'")
            .execute(pool)
            .await;
        assert!(del_pnl.is_err(), "Direct deletion of P&L must fail");

        // Direct deletion of standard ledger having opening balance must fail (ON DELETE RESTRICT)
        let del_std = sqlx::query("DELETE FROM ledgers WHERE id = 'led_cas_std'")
            .execute(pool)
            .await;
        assert!(
            del_std.is_err(),
            "Direct deletion of ledger with opening balance must fail (ON DELETE RESTRICT)"
        );

        // 7. Company-level cascade deletion:
        // Deleting the parent company must cascade and delete all dependent records without error
        let del_cmp = sqlx::query("DELETE FROM companies WHERE id = 'cmp_cas'")
            .execute(pool)
            .await;
        assert!(
            del_cmp.is_ok(),
            "Deleting company must cascade cleanly without foreign key violations: {:?}",
            del_cmp.err()
        );

        // 8. Verify all dependent tables have zero rows remaining for 'cmp_cas'
        let cnt_settings: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM company_settings WHERE company_id = 'cmp_cas'",
        )
        .fetch_one(pool)
        .await
        .expect("Count settings");
        assert_eq!(
            cnt_settings, 0,
            "company_settings must be empty after cascade"
        );

        let cnt_groups: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = 'cmp_cas'")
                .fetch_one(pool)
                .await
                .expect("Count groups");
        assert_eq!(cnt_groups, 0, "account_groups must be empty after cascade");

        let cnt_ledgers: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ledgers WHERE company_id = 'cmp_cas'")
                .fetch_one(pool)
                .await
                .expect("Count ledgers");
        assert_eq!(cnt_ledgers, 0, "ledgers must be empty after cascade");

        let cnt_balances: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM opening_balances WHERE company_id = 'cmp_cas'",
        )
        .fetch_one(pool)
        .await
        .expect("Count balances");
        assert_eq!(
            cnt_balances, 0,
            "opening_balances must be empty after cascade"
        );

        // 9. Run PRAGMA foreign_key_check to verify total database referential integrity
        let fk_violations: Vec<(String, Option<i64>, String, i64)> =
            sqlx::query_as("PRAGMA foreign_key_check;")
                .fetch_all(pool)
                .await
                .expect("PRAGMA foreign_key_check");
        assert!(
            fk_violations.is_empty(),
            "Database must have 0 foreign key violations after company cascade"
        );
    }
}
