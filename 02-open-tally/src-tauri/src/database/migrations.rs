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
        if current_version == "0.2.0-sprint1"
            || current_version == "0.2.1-sprint1"
            || current_version == "0.3.0-sprint2"
            || current_version == "0.3.1-sprint2"
            || current_version == "0.4.0-sprint4"
            || current_version == "0.4.1-sprint4"
            || current_version == "0.4.2-sprint4"
            || current_version == "0.4.3-sprint5"
            || current_version == "0.4.4-sprint5"
            || current_version == "0.4.5-sprint6"
            || current_version == "0.4.6-sprint6"
        {
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

    if current_version == "0.2.1-sprint1"
        || current_version == "0.3.0-sprint2"
        || current_version == "0.3.1-sprint2"
        || current_version == "0.4.0-sprint4"
        || current_version == "0.4.1-sprint4"
        || current_version == "0.4.2-sprint4"
        || current_version == "0.4.3-sprint5"
        || current_version == "0.4.4-sprint5"
        || current_version == "0.4.5-sprint6"
        || current_version == "0.4.6-sprint6"
    {
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

/// Sprint 2 Database DDL Definition (v2.9.5)
/// Defines voucher_types, voucher_number_series, voucher_number_configs,
/// voucher_number_sequences, vouchers, and voucher_lines
pub const SPRINT2_DDL: &str = r#"
-- 1. VOUCHER TYPES MASTER
CREATE TABLE voucher_types (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    category TEXT NOT NULL CHECK (category IN ('accounting', 'trading', 'order', 'inventory', 'payroll', 'scenario', 'non_financial')),
    parent_type TEXT,
    affects_ledger_balance INTEGER NOT NULL DEFAULT 1 CHECK (affects_ledger_balance IN (0, 1)),
    affects_inventory INTEGER NOT NULL DEFAULT 0 CHECK (affects_inventory IN (0, 1)),
    is_predefined INTEGER NOT NULL DEFAULT 0 CHECK (is_predefined IN (0, 1)),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual', 'multi_series')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    CONSTRAINT uq_voucher_types_company UNIQUE (company_id, id),
    UNIQUE (company_id, name),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
);

-- 2. VOUCHER NUMBERING SERIES MASTER
CREATE TABLE voucher_number_series (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL COLLATE NOCASE,
    series_code TEXT NOT NULL COLLATE NOCASE,
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
    restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    CONSTRAINT uq_series_company_id UNIQUE (company_id, id),
    CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name),
    CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code),
    CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id),
    CONSTRAINT uq_series_full_ownership UNIQUE (company_id, voucher_type_id, id, numbering_method),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
);

-- 2b. VOUCHER NUMBERING CONFIGURATIONS
CREATE TABLE voucher_number_configs (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    applicable_from TEXT NOT NULL,
    prefix TEXT NOT NULL DEFAULT '',
    suffix TEXT NOT NULL DEFAULT '',
    start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
    width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    CONSTRAINT uq_config_series_date UNIQUE (company_id, series_id, applicable_from),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, voucher_type_id, series_id) REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
);

-- 2c. VOUCHER NUMBERING SEQUENCES
CREATE TABLE voucher_number_sequences (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    period_start TEXT NOT NULL,
    next_number INTEGER NOT NULL CHECK (next_number > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    CONSTRAINT uq_seq_period UNIQUE (company_id, series_id, period_start),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, voucher_type_id, series_id) REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
);

-- 3. VOUCHER HEADERS MASTER
CREATE TABLE vouchers (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
    period_start TEXT,
    sequence_number INTEGER,
    voucher_number TEXT NOT NULL,
    date TEXT NOT NULL,
    effective_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
    reference_no TEXT,
    reference_date TEXT,
    narration TEXT,
    total_amount TEXT,
    reversal_of_voucher_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id),
    CONSTRAINT chk_voucher_date CHECK (
        date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(date) IS NOT NULL
        AND date(date) = date
    ),
    CONSTRAINT chk_effective_date CHECK (
        effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(effective_date) IS NOT NULL
        AND date(effective_date) = effective_date
    ),
    CONSTRAINT chk_voucher_sequence_metadata CHECK (
        (status = 'draft' AND period_start IS NULL AND sequence_number IS NULL)
        OR
        (
            status IN ('posted', 'cancelled', 'reversed')
            AND
            (
                (numbering_method = 'automatic' AND period_start IS NOT NULL AND sequence_number > 0)
                OR
                (numbering_method = 'manual' AND sequence_number IS NULL)
            )
        )
    ),
    CONSTRAINT chk_voucher_total_amount_canonical CHECK (
        (
            status = 'draft' 
            AND (
                total_amount IS NULL
                OR
                (
                    length(total_amount) >= 1
                    AND length(total_amount) <= 25
                    AND total_amount NOT GLOB '*[^0-9.]*'
                    AND total_amount NOT GLOB '*..*'
                    AND total_amount NOT GLOB '.*'
                    AND total_amount NOT GLOB '*.'
                    AND (
                        (
                            instr(total_amount, '.') > 0
                            AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                            AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                            AND (
                                (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                                 AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                                OR
                                (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                            )
                        )
                        OR
                        (
                            instr(total_amount, '.') = 0
                            AND total_amount GLOB '[1-9]*'
                        )
                    )
                )
            )
        )
        OR
        (
            status IN ('posted', 'reversed', 'cancelled')
            AND total_amount IS NOT NULL
            AND length(total_amount) >= 1
            AND length(total_amount) <= 25
            AND total_amount NOT GLOB '*[^0-9.]*'
            AND total_amount NOT GLOB '*..*'
            AND total_amount NOT GLOB '.*'
            AND total_amount NOT GLOB '*.'
            AND (
                (
                    instr(total_amount, '.') > 0
                    AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                    AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                    AND (
                        (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                         AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                        OR
                        (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                    )
                )
                OR
                (
                    instr(total_amount, '.') = 0
                    AND total_amount GLOB '[1-9]*'
                )
            )
        )
    )
);

CREATE INDEX idx_vouchers_date ON vouchers(company_id, date);
CREATE INDEX idx_vouchers_type_status ON vouchers(company_id, voucher_type_id, status);
CREATE INDEX idx_voucher_number_series_type ON voucher_number_series(company_id, voucher_type_id);
CREATE INDEX idx_voucher_number_configs_series ON voucher_number_configs(company_id, voucher_type_id, series_id);
CREATE INDEX idx_voucher_number_sequences_series ON voucher_number_sequences(company_id, voucher_type_id, series_id);
CREATE INDEX idx_vouchers_series_ownership ON vouchers(company_id, voucher_type_id, series_id, numbering_method);
CREATE UNIQUE INDEX uq_vouchers_permanent_number ON vouchers(company_id, voucher_type_id, voucher_number) WHERE status != 'draft';
CREATE UNIQUE INDEX uq_vouchers_series_sequence ON vouchers(company_id, series_id, period_start, sequence_number) WHERE status != 'draft' AND sequence_number IS NOT NULL;
CREATE UNIQUE INDEX uq_vouchers_reversal ON vouchers(company_id, reversal_of_voucher_id) WHERE reversal_of_voucher_id IS NOT NULL;

-- 4. VOUCHER LINE ITEMS
CREATE TABLE voucher_lines (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    ledger_id TEXT NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount TEXT NOT NULL,
    narration TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, voucher_id, line_number),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_voucher_line_amount_canonical CHECK (
        amount IS NOT NULL
        AND length(amount) >= 1
        AND length(amount) <= 25
        AND amount NOT GLOB '*[^0-9.]*'
        AND amount NOT GLOB '*..*'
        AND amount NOT GLOB '.*'
        AND amount NOT GLOB '*.'
        AND (
            (
                instr(amount, '.') > 0
                AND length(substr(amount, instr(amount, '.') + 1)) BETWEEN 1 AND 4
                AND instr(substr(amount, instr(amount, '.') + 1), '.') = 0
                AND (
                    (substr(amount, 1, instr(amount, '.') - 1) = '0' 
                     AND substr(amount, instr(amount, '.') + 1) GLOB '*[1-9]*')
                    OR
                    (substr(amount, 1, instr(amount, '.') - 1) GLOB '[1-9]*')
                )
            )
            OR
            (
                instr(amount, '.') = 0
                AND amount GLOB '[1-9]*'
            )
        )
    )
);

CREATE INDEX idx_voucher_lines_ledger ON voucher_lines(company_id, ledger_id);
"#;

/// Sprint 2 Triggers
pub const SPRINT2_TRIGGERS: &str = r#"
-- Trigger 1: Enforce Posted/Cancelled/Reversed Voucher Header Immutability & Reversal Freeze
CREATE TRIGGER trg_prevent_posted_voucher_mutation
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition. Cancelled and reversed vouchers are permanently frozen.')
    WHERE (OLD.status = 'cancelled' OR OLD.status = 'reversed');

    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition from posted status. Posted vouchers can only transition to CANCELLED or REVERSED.')
    WHERE OLD.status = 'posted' AND NEW.status NOT IN ('posted', 'cancelled', 'reversed');

    SELECT RAISE(ABORT, 'Core financial, reference, and numbering fields of a posted voucher are strictly immutable.')
    WHERE (NEW.company_id IS NOT OLD.company_id)
       OR (NEW.voucher_type_id IS NOT OLD.voucher_type_id)
       OR (NEW.series_id IS NOT OLD.series_id)
       OR (NEW.numbering_method IS NOT OLD.numbering_method)
       OR (NEW.period_start IS NOT OLD.period_start)
       OR (NEW.sequence_number IS NOT OLD.sequence_number)
       OR (NEW.voucher_number IS NOT OLD.voucher_number)
       OR (NEW.date IS NOT OLD.date)
       OR (NEW.effective_date IS NOT OLD.effective_date)
       OR (NEW.reference_no IS NOT OLD.reference_no)
       OR (NEW.reference_date IS NOT OLD.reference_date)
       OR (NEW.narration IS NOT OLD.narration)
       OR (NEW.total_amount IS NOT OLD.total_amount)
       OR (NEW.reversal_of_voucher_id IS NOT OLD.reversal_of_voucher_id)
       OR (NEW.created_at IS NOT OLD.created_at);

    SELECT RAISE(ABORT, 'Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable to preserve ledger equilibrium.')
    WHERE OLD.reversal_of_voucher_id IS NOT NULL AND NEW.status = 'cancelled';

    SELECT RAISE(ABORT, 'Cannot mark voucher as REVERSED without an existing POSTED reversing voucher referencing it.')
    WHERE OLD.status = 'posted' AND NEW.status = 'reversed'
      AND NOT EXISTS (
          SELECT 1 FROM vouchers rev
          WHERE rev.company_id = NEW.company_id
            AND rev.reversal_of_voucher_id = NEW.id
            AND rev.status = 'posted'
      );
END;

-- Trigger 1b: Validate Reversing Voucher Target Eligibility on Promotion to POSTED
CREATE TRIGGER trg_validate_reversal_target_on_post
BEFORE UPDATE OF status ON vouchers
FOR EACH ROW
WHEN NEW.reversal_of_voucher_id IS NOT NULL 
 AND NEW.status = 'posted' 
 AND OLD.status = 'draft'
BEGIN
    SELECT RAISE(ABORT, 'Cannot post reversing voucher: Referenced original voucher must exist in the same company with status POSTED.')
    WHERE NOT EXISTS (
        SELECT 1 FROM vouchers orig
        WHERE orig.company_id = NEW.company_id
          AND orig.id = NEW.reversal_of_voucher_id
          AND orig.status = 'posted'
    );
END;

-- Trigger 2: Prohibit Illegal Direct Draft Transitions
CREATE TRIGGER trg_prevent_illegal_draft_transitions
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status = 'draft'
BEGIN
    SELECT RAISE(ABORT, 'Draft vouchers cannot transition directly to cancelled or reversed status. Drafts must be posted first or deleted/purged.')
    WHERE NEW.status NOT IN ('draft', 'posted');
END;

-- Trigger 3: Prohibit Hard Deletion of Non-Draft Vouchers (Unconditional)
CREATE TRIGGER trg_prevent_posted_voucher_deletion
BEFORE DELETE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Hard deletion of financial vouchers is strictly prohibited. Posted, cancelled, and reversed vouchers cannot be deleted, even via company cascade.');
END;

-- Trigger 4: Prohibit Inserting Lines into Non-Draft Vouchers
CREATE TRIGGER trg_prevent_posted_voucher_lines_insert
BEFORE INSERT ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = NEW.company_id 
      AND id = NEW.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot insert line items into a posted, cancelled, or reversed voucher.');
END;

-- Trigger 5: Prohibit Mutating or Reassigning Lines Belonging to or Assigned to Non-Draft Vouchers
CREATE TRIGGER trg_prevent_posted_voucher_lines_mutation
BEFORE UPDATE ON voucher_lines
FOR EACH ROW
WHEN (
    EXISTS (
        SELECT 1 FROM vouchers 
        WHERE company_id = OLD.company_id 
          AND id = OLD.voucher_id 
          AND status IN ('posted', 'cancelled', 'reversed')
    )
    OR EXISTS (
        SELECT 1 FROM vouchers 
        WHERE company_id = NEW.company_id 
          AND id = NEW.voucher_id 
          AND status IN ('posted', 'cancelled', 'reversed')
    )
    OR (NEW.company_id IS NOT OLD.company_id)
)
BEGIN
    SELECT RAISE(ABORT, 'Voucher line items belonging to or assigned to a posted, cancelled, or reversed voucher cannot be modified or reassigned.');
END;

-- Trigger 6: Prohibit Deleting Lines Belonging to Non-Draft Vouchers
CREATE TRIGGER trg_prevent_posted_voucher_lines_deletion
BEFORE DELETE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Voucher line items belonging to a posted, cancelled, or reversed voucher cannot be deleted.');
END;

-- Trigger 7: Prevent Deleting Ledgers Referenced in Vouchers
CREATE TRIGGER trg_prevent_ledger_delete_with_vouchers
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND EXISTS (SELECT 1 FROM voucher_lines WHERE company_id = OLD.company_id AND ledger_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete ledger: it is referenced in existing voucher lines.');
END;

-- Trigger 8: Prohibit Hard Deletion of Companies Containing Financial Vouchers
CREATE TRIGGER trg_prevent_company_delete_with_financial_vouchers
BEFORE DELETE ON companies
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete company: Company contains financial accounting records. As an application-level financial-history protection mechanism (subject to professional legal and statutory review), companies with financial records cannot be deleted and must be soft-archived.');
END;

-- Trigger 8a: Prohibit Mutating Series Code or Numbering Method on Active Series
CREATE TRIGGER trg_prevent_series_code_mutation
BEFORE UPDATE OF series_code, numbering_method ON voucher_number_series
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.id
      AND v.status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot modify series_code or numbering_method: permanent vouchers exist in this series.');
END;

-- Trigger 8b: Prohibit Deleting Series Referencing Permanent Vouchers
CREATE TRIGGER trg_prevent_series_deletion_with_vouchers
BEFORE DELETE ON voucher_number_series
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.id
      AND v.status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete series: permanent vouchers exist in this series.');
END;

-- Trigger 9a: Prohibit Modifying Configuration Effective Date
CREATE TRIGGER trg_prevent_config_applicable_from_mutation
BEFORE UPDATE OF applicable_from ON voucher_number_configs
FOR EACH ROW
WHEN NEW.applicable_from != OLD.applicable_from
BEGIN
    SELECT RAISE(ABORT, 'Configuration effective date (applicable_from) is immutable once created. Delete the unreferenced configuration and re-insert if no vouchers exist.');
END;

-- Trigger 9b: Series-Scoped Immutability on UPDATE for Active Configurations
CREATE TRIGGER trg_prevent_active_config_mutation
BEFORE UPDATE ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= OLD.applicable_from
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot modify active numbering configuration: permanent vouchers exist in this series governed by this configuration. Append a new configuration row with a future applicable_from date instead.');
END;

-- Trigger 10: Series-Scoped Immutability on DELETE
CREATE TRIGGER trg_prevent_active_config_deletion
BEFORE DELETE ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= OLD.applicable_from
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete active numbering configuration: permanent vouchers exist in this series governed by this configuration.');
END;

-- Trigger 11: Prohibit Retroactive Numbering Configuration INSERT
CREATE TRIGGER trg_prevent_retroactive_config_insert
BEFORE INSERT ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = NEW.company_id
      AND v.series_id = NEW.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= NEW.applicable_from
      AND (
          NOT EXISTS (
              SELECT 1 FROM voucher_number_configs c
              WHERE c.company_id = NEW.company_id
                AND c.series_id = NEW.series_id
                AND c.applicable_from > NEW.applicable_from
                AND v.date >= c.applicable_from
          )
      )
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot insert retroactive numbering configuration: permanent vouchers already exist in this series governed by the proposed effective period.');
END;

-- Trigger 12: Prevent Mutation of Series Restart Period
CREATE TRIGGER trg_prevent_series_restart_period_mutation
BEFORE UPDATE OF restart_period ON voucher_number_series
FOR EACH ROW
WHEN NEW.restart_period IS NOT OLD.restart_period
BEGIN
    SELECT RAISE(ABORT, 'Series restart_period is permanently immutable; changing reset frequency requires creating a new numbering series');
END;

-- Trigger 13: Seed Predefined Voucher Types on Company Insert
CREATE TRIGGER trg_seed_predefined_voucher_types_on_company_insert
AFTER INSERT ON companies
FOR EACH ROW
BEGIN
    INSERT OR IGNORE INTO voucher_types (id, company_id, name, category, parent_type, affects_ledger_balance, affects_inventory, is_predefined, is_active, numbering_method)
    VALUES
        ('vt_contra', NEW.id, 'Contra', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_payment', NEW.id, 'Payment', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_receipt', NEW.id, 'Receipt', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_jnl', NEW.id, 'Journal', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_sales', NEW.id, 'Sales', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_pur', NEW.id, 'Purchase', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_credit_note', NEW.id, 'Credit Note', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_debit_note', NEW.id, 'Debit Note', 'accounting', NULL, 1, 0, 1, 1, 'automatic'),
        ('vt_reversing_journal', NEW.id, 'Reversing Journal', 'scenario', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_memorandum', NEW.id, 'Memorandum', 'non_financial', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_sales_order', NEW.id, 'Sales Order', 'order', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_purchase_order', NEW.id, 'Purchase Order', 'order', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_delivery_note', NEW.id, 'Delivery Note', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_receipt_note', NEW.id, 'Receipt Note', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_rejections_in', NEW.id, 'Rejections In', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_rejections_out', NEW.id, 'Rejections Out', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_stock_journal', NEW.id, 'Stock Journal', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_physical_stock', NEW.id, 'Physical Stock', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_material_in', NEW.id, 'Material In', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_material_out', NEW.id, 'Material Out', 'inventory', NULL, 0, 1, 1, 1, 'automatic'),
        ('vt_job_work_in_order', NEW.id, 'Job Work In Order', 'order', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_job_work_out_order', NEW.id, 'Job Work Out Order', 'order', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_attendance', NEW.id, 'Attendance', 'payroll', NULL, 0, 0, 1, 1, 'automatic'),
        ('vt_payroll', NEW.id, 'Payroll', 'payroll', NULL, 1, 0, 1, 1, 'automatic');
END;
"#;

/// Seed query to populate 24 canonical predefined voucher types for all existing companies
pub const SPRINT2_VOUCHER_TYPES_SEED: &str = r#"
INSERT OR IGNORE INTO voucher_types (id, company_id, name, category, parent_type, affects_ledger_balance, affects_inventory, is_predefined, is_active, numbering_method)
SELECT vdef.id, c.id, vdef.name, vdef.category, vdef.parent_type, vdef.affects_ledger_balance, vdef.affects_inventory, 1, 1, 'automatic'
FROM companies c
CROSS JOIN (
    SELECT 'vt_contra' as id, 'Contra' as name, 'accounting' as category, NULL as parent_type, 1 as affects_ledger_balance, 0 as affects_inventory UNION ALL
    SELECT 'vt_payment', 'Payment', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_receipt', 'Receipt', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_jnl', 'Journal', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_sales', 'Sales', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_pur', 'Purchase', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_credit_note', 'Credit Note', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_debit_note', 'Debit Note', 'accounting', NULL, 1, 0 UNION ALL
    SELECT 'vt_reversing_journal', 'Reversing Journal', 'scenario', NULL, 0, 0 UNION ALL
    SELECT 'vt_memorandum', 'Memorandum', 'non_financial', NULL, 0, 0 UNION ALL
    SELECT 'vt_sales_order', 'Sales Order', 'order', NULL, 0, 0 UNION ALL
    SELECT 'vt_purchase_order', 'Purchase Order', 'order', NULL, 0, 0 UNION ALL
    SELECT 'vt_delivery_note', 'Delivery Note', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_receipt_note', 'Receipt Note', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_rejections_in', 'Rejections In', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_rejections_out', 'Rejections Out', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_stock_journal', 'Stock Journal', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_physical_stock', 'Physical Stock', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_material_in', 'Material In', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_material_out', 'Material Out', 'inventory', NULL, 0, 1 UNION ALL
    SELECT 'vt_job_work_in_order', 'Job Work In Order', 'order', NULL, 0, 0 UNION ALL
    SELECT 'vt_job_work_out_order', 'Job Work Out Order', 'order', NULL, 0, 0 UNION ALL
    SELECT 'vt_attendance', 'Attendance', 'payroll', NULL, 0, 0 UNION ALL
    SELECT 'vt_payroll', 'Payroll', 'payroll', NULL, 1, 0
) vdef;
"#;

/// Execute the Sprint 2 migration (0.2.1-sprint1 -> 0.3.0-sprint2).
/// Follows the authoritative 12-step pinned-connection lifecycle with 4-exit-state safety.
pub async fn run_sprint2_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    // STEP 1: Dedicated connection checked out from the pool
    let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;

    // STEP 2: Read and verify source schema version BEFORE touching foreign keys
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

    if matches!(
        current_version.as_str(),
        "0.3.0-sprint2"
            | "0.3.1-sprint2"
            | "0.4.0-sprint4"
            | "0.4.1-sprint4"
            | "0.4.2-sprint4"
            | "0.4.3-sprint5"
            | "0.4.4-sprint5"
            | "0.4.5-sprint6"
            | "0.4.6-sprint6"
    ) {
        tracing::info!(
            "Database already at schema version 0.3.0-sprint2 or later; migration skipped (idempotent)."
        );
        return Ok(());
    }

    if current_version != "0.2.1-sprint1" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.2.1-sprint1' for Sprint 2 migration, found '{}'",
            current_version
        )));
    }

    // Track foreign key and transaction state across the execution scope
    let mut fk_disabled = false;
    let mut in_transaction = false;

    // Guarded execution block: Every operation from PRAGMA foreign_keys = OFF onwards runs here
    let migration_res: Result<(), OpenTallyError> = async {
        // STEP 3: Disable foreign keys OUTSIDE any transaction & verify status
        sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        fk_disabled = true;

        let fk_status: i64 = sqlx::query_scalar("PRAGMA foreign_keys;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if fk_status != 0 {
            return Err(OpenTallyError::Migration(
                "Failed to disable foreign keys prior to migration table recreation (status != 0)".into()
            ));
        }

        // STEP 4: Begin immediate exclusive write transaction (WAL write lock)
        sqlx::raw_sql("BEGIN IMMEDIATE;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        in_transaction = true;

        // STEP 5: Create target temporary audit table (audit_log_dg_tmp)
        sqlx::raw_sql(
            "CREATE TABLE audit_log_dg_tmp (
                id TEXT PRIMARY KEY,
                company_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                ledger_id TEXT,
                financial_year_begins TEXT,
                action TEXT NOT NULL CHECK (action IN ('CREATE', 'UPDATE', 'CLEAR', 'DELETE', 'POST', 'CANCEL', 'REVERSE')),
                old_values TEXT,
                new_values TEXT,
                performed_by TEXT NOT NULL,
                timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT
            );"
        )
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // STEP 6: Copy 100% of existing Sprint 1 audit records
        sqlx::raw_sql(
            "INSERT INTO audit_log_dg_tmp (
                id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
            )
            SELECT id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
            FROM audit_log;"
        )
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // STEP 7: Verify source and destination record counts
        let src_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let dst_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log_dg_tmp;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if src_count != dst_count {
            return Err(OpenTallyError::Migration(format!(
                "Audit log record count mismatch during migration: source={}, destination={}",
                src_count, dst_count
            )));
        }

        // STEP 8: Drop old table and rename temporary table to canonical name
        sqlx::raw_sql("DROP TABLE audit_log;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::raw_sql("ALTER TABLE audit_log_dg_tmp RENAME TO audit_log;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // STEP 9: Recreate canonical indexes & triggers; apply Sprint 2 DDL & seeds
        sqlx::raw_sql(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);
             CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(company_id, timestamp);
             CREATE TRIGGER trg_prevent_audit_log_mutation
             BEFORE UPDATE ON audit_log
             FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated'); END;
             CREATE TRIGGER trg_prevent_audit_log_deletion
             BEFORE DELETE ON audit_log
             FOR EACH ROW WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
             BEGIN SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted'); END;"
        )
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Execute Sprint 2 core DDL, triggers, and voucher type seeding
        sqlx::raw_sql(SPRINT2_DDL).execute(&mut *conn).await.map_err(OpenTallyError::Database)?;
        sqlx::raw_sql(SPRINT2_TRIGGERS).execute(&mut *conn).await.map_err(OpenTallyError::Database)?;
        sqlx::raw_sql(SPRINT2_VOUCHER_TYPES_SEED).execute(&mut *conn).await.map_err(OpenTallyError::Database)?;

        // Update metadata schema version
        let app_version = env!("CARGO_PKG_VERSION");
        sqlx::query(
            "UPDATE _opentally_metadata 
             SET schema_version = '0.3.0-sprint2', app_version = $1
             WHERE id = 1"
        )
        .bind(app_version)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // STEP 10: Run PRAGMA foreign_key_check and verify zero violations
        let violations = sqlx::query(
            "PRAGMA foreign_key_check;"
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if !violations.is_empty() {
            return Err(OpenTallyError::Migration(format!(
                "Foreign key integrity check failed with {} violation(s) after Sprint 2 migration",
                violations.len()
            )));
        }

        // STEP 11a: Commit write transaction
        sqlx::raw_sql("COMMIT;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        in_transaction = false;

        Ok(())
    }.await;

    // STEP 11b & 12: Unconditional Cleanup & Connection State Management Scope
    // Handle rollback if migration execution failed while transaction was active
    if in_transaction {
        tracing::warn!("Migration transaction active upon exit; issuing ROLLBACK;");
        match sqlx::raw_sql("ROLLBACK;").execute(&mut *conn).await {
            Ok(_) => {
                tracing::info!(
                    "State B: ROLLBACK; executed successfully; transaction aborted cleanly."
                );
            }
            Err(rb_err) => {
                // State C: ROLLBACK FAILED! Transaction state is indeterminate.
                tracing::error!(
                    "State C: CRITICAL - ROLLBACK failed ({:?})! Transaction state is indeterminate. Closing and discarding connection immediately.",
                    rb_err
                );
                let _ = conn.close().await; // DISCARD CONNECTION - never return to pool
                return Err(OpenTallyError::Migration(format!(
                    "Fatal migration error: ROLLBACK failed ({:?}); connection closed and discarded to prevent returning indeterminate transaction state to pool",
                    rb_err
                )));
            }
        }
    }

    // Step 11b: Restore foreign keys OUTSIDE transaction (NO `?` operator used)
    if fk_disabled {
        let restore_res = sqlx::raw_sql("PRAGMA foreign_keys = ON;")
            .execute(&mut *conn)
            .await;
        let verify_status: Result<i64, _> = sqlx::query_scalar("PRAGMA foreign_keys;")
            .fetch_one(&mut *conn)
            .await;

        let fk_re_enabled = restore_res.is_ok() && matches!(verify_status, Ok(1));

        if !fk_re_enabled {
            // State D: Post-Commit Connection Cleanup Failure!
            tracing::error!(
                "State D: CRITICAL - Failed to restore or verify foreign keys (status: {:?})! Closing and discarding connection immediately.",
                verify_status
            );
            let _ = conn.close().await; // DISCARD CONNECTION - never return to pool
            return Err(OpenTallyError::Migration(format!(
                "Fatal migration error: Foreign keys could not be restored to ON (1) (status: {:?}); connection closed and discarded to protect pool integrity",
                verify_status
            )));
        }
    }

    // Step 12: Verify final schema version and return healthy connection
    match migration_res {
        Ok(()) => {
            let final_version: String =
                sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

            if final_version != "0.3.0-sprint2" {
                let _ = conn.close().await;
                return Err(OpenTallyError::Migration(format!(
                    "Final schema version verification failed: expected '0.3.0-sprint2', found '{}'",
                    final_version
                )));
            }

            // State A: Clean Success
            tracing::info!("State A: Migration 0.3.0-sprint2 completed successfully. Connection verified healthy (foreign_keys = 1, autocommit active).");
            Ok(())
        }
        Err(err) => {
            // State B: Clean Rollback
            tracing::info!("State B: Migration 0.3.0-sprint2 failed and rolled back cleanly. Connection verified healthy (foreign_keys = 1, autocommit active).");
            Err(err)
        }
    }
}

/// Stage 4 Hardening DDL for temporary vouchers table with hardened chk_voucher_sequence_metadata
pub const STAGE4_HARDENING_VOUCHERS_TABLE_TMP_DDL: &str = r#"
CREATE TABLE vouchers_dg_tmp (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
    period_start TEXT,
    sequence_number INTEGER,
    voucher_number TEXT NOT NULL,
    date TEXT NOT NULL,
    effective_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
    reference_no TEXT,
    reference_date TEXT,
    narration TEXT,
    total_amount TEXT,
    reversal_of_voucher_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id),
    CONSTRAINT chk_voucher_date CHECK (
        date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(date) IS NOT NULL
        AND date(date) = date
    ),
    CONSTRAINT chk_effective_date CHECK (
        effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(effective_date) IS NOT NULL
        AND date(effective_date) = effective_date
    ),
    CONSTRAINT chk_voucher_sequence_metadata CHECK (
        CASE
            WHEN status = 'draft' THEN (
                period_start IS NULL
                AND sequence_number IS NULL
            )
            WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'automatic' THEN (
                period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                AND date(period_start) IS NOT NULL
                AND date(period_start) = period_start
                AND sequence_number IS NOT NULL
                AND sequence_number > 0
            )
            WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'manual' THEN (
                sequence_number IS NULL
                AND (
                    period_start IS NULL
                    OR (
                        period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                        AND date(period_start) IS NOT NULL
                        AND date(period_start) = period_start
                    )
                )
            )
            ELSE 0
        END = 1
    ),
    CONSTRAINT chk_voucher_total_amount_canonical CHECK (
        (
            status = 'draft' 
            AND (
                total_amount IS NULL
                OR
                (
                    length(total_amount) >= 1
                    AND length(total_amount) <= 25
                    AND total_amount NOT GLOB '*[^0-9.]*'
                    AND total_amount NOT GLOB '*..*'
                    AND total_amount NOT GLOB '.*'
                    AND total_amount NOT GLOB '*.'
                    AND (
                        (
                            instr(total_amount, '.') > 0
                            AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                            AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                            AND (
                                (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                                 AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                                OR
                                (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                            )
                        )
                        OR
                        (
                            instr(total_amount, '.') = 0
                            AND total_amount GLOB '[1-9]*'
                        )
                    )
                )
            )
        )
        OR
        (
            status IN ('posted', 'reversed', 'cancelled')
            AND total_amount IS NOT NULL
            AND length(total_amount) >= 1
            AND length(total_amount) <= 25
            AND total_amount NOT GLOB '*[^0-9.]*'
            AND total_amount NOT GLOB '*..*'
            AND total_amount NOT GLOB '.*'
            AND total_amount NOT GLOB '*.'
            AND (
                (
                    instr(total_amount, '.') > 0
                    AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                    AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                    AND (
                        (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                         AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                        OR
                        (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                    )
                )
                OR
                (
                    instr(total_amount, '.') = 0
                    AND total_amount GLOB '[1-9]*'
                )
            )
        )
    )
);
"#;

/// Stage 4 Hardening DDL for temporary voucher lines table with chk_voucher_line_number_bounds
pub const STAGE4_HARDENING_VOUCHER_LINES_TABLE_TMP_DDL: &str = r#"
CREATE TABLE voucher_lines_dg_tmp (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    ledger_id TEXT NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount TEXT NOT NULL,
    narration TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, voucher_id, line_number),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_voucher_line_number_bounds CHECK (
        (line_number BETWEEN 1 AND 10000)
        OR
        (line_number BETWEEN 1000001 AND 1010000)
    ),
    CONSTRAINT chk_voucher_line_amount_canonical CHECK (
        amount IS NOT NULL
        AND length(amount) >= 1
        AND length(amount) <= 25
        AND amount NOT GLOB '*[^0-9.]*'
        AND amount NOT GLOB '*..*'
        AND amount NOT GLOB '.*'
        AND amount NOT GLOB '*.'
        AND (
            (
                instr(amount, '.') > 0
                AND length(substr(amount, instr(amount, '.') + 1)) BETWEEN 1 AND 4
                AND instr(substr(amount, instr(amount, '.') + 1), '.') = 0
                AND (
                    (substr(amount, 1, instr(amount, '.') - 1) = '0' 
                     AND substr(amount, instr(amount, '.') + 1) GLOB '*[1-9]*')
                    OR
                    (substr(amount, 1, instr(amount, '.') - 1) GLOB '[1-9]*')
                )
            )
            OR
            (
                instr(amount, '.') = 0
                AND amount GLOB '[1-9]*'
            )
        )
    )
);
"#;

/// Canonical inventory of the 14 triggers directly or indirectly dependent on vouchers and voucher_lines
pub const CANONICAL_STAGE4_DEPENDENT_TRIGGERS: [&str; 14] = [
    "trg_prevent_posted_voucher_mutation",
    "trg_validate_reversal_target_on_post",
    "trg_prevent_illegal_draft_transitions",
    "trg_prevent_posted_voucher_deletion",
    "trg_prevent_posted_voucher_lines_insert",
    "trg_prevent_posted_voucher_lines_mutation",
    "trg_prevent_posted_voucher_lines_deletion",
    "trg_prevent_ledger_delete_with_vouchers",
    "trg_prevent_company_delete_with_financial_vouchers",
    "trg_prevent_series_code_mutation",
    "trg_prevent_series_deletion_with_vouchers",
    "trg_prevent_active_config_mutation",
    "trg_prevent_active_config_deletion",
    "trg_prevent_retroactive_config_insert",
];

/// Stage 4 Hardening DDL for recreating canonical indexes and triggers on vouchers & lines
pub const STAGE4_HARDENING_INDEXES_AND_TRIGGERS_DDL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_vouchers_date ON vouchers(company_id, date);
CREATE INDEX IF NOT EXISTS idx_vouchers_type_status ON vouchers(company_id, voucher_type_id, status);
CREATE INDEX IF NOT EXISTS idx_vouchers_series_ownership ON vouchers(company_id, voucher_type_id, series_id, numbering_method);
CREATE UNIQUE INDEX IF NOT EXISTS uq_vouchers_permanent_number ON vouchers(company_id, voucher_type_id, voucher_number) WHERE status != 'draft';
CREATE UNIQUE INDEX IF NOT EXISTS uq_vouchers_series_sequence ON vouchers(company_id, series_id, period_start, sequence_number) WHERE status != 'draft' AND sequence_number IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_vouchers_reversal ON vouchers(company_id, reversal_of_voucher_id) WHERE reversal_of_voucher_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_voucher_lines_ledger ON voucher_lines(company_id, ledger_id);

-- Trigger 1: Enforce Posted/Cancelled/Reversed Voucher Header Immutability & Reversal Freeze
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_mutation
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition. Cancelled and reversed vouchers are permanently frozen.')
    WHERE (OLD.status = 'cancelled' OR OLD.status = 'reversed');

    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition from posted status. Posted vouchers can only transition to CANCELLED or REVERSED.')
    WHERE OLD.status = 'posted' AND NEW.status NOT IN ('posted', 'cancelled', 'reversed');

    SELECT RAISE(ABORT, 'Core financial, reference, and numbering fields of a posted voucher are strictly immutable.')
    WHERE (NEW.company_id IS NOT OLD.company_id)
       OR (NEW.voucher_type_id IS NOT OLD.voucher_type_id)
       OR (NEW.series_id IS NOT OLD.series_id)
       OR (NEW.numbering_method IS NOT OLD.numbering_method)
       OR (NEW.period_start IS NOT OLD.period_start)
       OR (NEW.sequence_number IS NOT OLD.sequence_number)
       OR (NEW.voucher_number IS NOT OLD.voucher_number)
       OR (NEW.date IS NOT OLD.date)
       OR (NEW.effective_date IS NOT OLD.effective_date)
       OR (NEW.reference_no IS NOT OLD.reference_no)
       OR (NEW.reference_date IS NOT OLD.reference_date)
       OR (NEW.narration IS NOT OLD.narration)
       OR (NEW.total_amount IS NOT OLD.total_amount)
       OR (NEW.reversal_of_voucher_id IS NOT OLD.reversal_of_voucher_id)
       OR (NEW.created_at IS NOT OLD.created_at);

    SELECT RAISE(ABORT, 'Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable to preserve ledger equilibrium.')
    WHERE OLD.reversal_of_voucher_id IS NOT NULL AND NEW.status = 'cancelled';

    SELECT RAISE(ABORT, 'Cannot mark voucher as REVERSED without an existing POSTED reversing voucher referencing it.')
    WHERE OLD.status = 'posted' AND NEW.status = 'reversed'
      AND NOT EXISTS (
          SELECT 1 FROM vouchers rev
          WHERE rev.company_id = NEW.company_id
            AND rev.reversal_of_voucher_id = NEW.id
            AND rev.status = 'posted'
      );
END;

-- Trigger 1b: Validate Reversing Voucher Target Eligibility on Promotion to POSTED
CREATE TRIGGER IF NOT EXISTS trg_validate_reversal_target_on_post
BEFORE UPDATE OF status ON vouchers
FOR EACH ROW
WHEN NEW.reversal_of_voucher_id IS NOT NULL 
 AND NEW.status = 'posted' 
 AND OLD.status = 'draft'
BEGIN
    SELECT RAISE(ABORT, 'Cannot post reversing voucher: Referenced original voucher must exist in the same company with status POSTED.')
    WHERE NOT EXISTS (
        SELECT 1 FROM vouchers orig
        WHERE orig.company_id = NEW.company_id
          AND orig.id = NEW.reversal_of_voucher_id
          AND orig.status = 'posted'
    );
END;

-- Trigger 2: Prohibit Illegal Direct Draft Transitions
CREATE TRIGGER IF NOT EXISTS trg_prevent_illegal_draft_transitions
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status = 'draft'
BEGIN
    SELECT RAISE(ABORT, 'Draft vouchers cannot transition directly to cancelled or reversed status. Drafts must be posted first or deleted/purged.')
    WHERE NEW.status NOT IN ('draft', 'posted');
END;

-- Trigger 3: Prohibit Hard Deletion of Non-Draft Vouchers (Unconditional)
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_deletion
BEFORE DELETE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Hard deletion of financial vouchers is strictly prohibited. Posted, cancelled, and reversed vouchers cannot be deleted, even via company cascade.');
END;

-- Trigger 4: Prohibit Inserting Lines into Non-Draft Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_lines_insert
BEFORE INSERT ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = NEW.company_id 
      AND id = NEW.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot insert line items into a posted, cancelled, or reversed voucher.');
END;

-- Trigger 5: Prohibit Mutating or Reassigning Lines Belonging to or Assigned to Non-Draft Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_lines_mutation
BEFORE UPDATE ON voucher_lines
FOR EACH ROW
WHEN (
    EXISTS (
        SELECT 1 FROM vouchers 
        WHERE company_id = OLD.company_id 
          AND id = OLD.voucher_id 
          AND status IN ('posted', 'cancelled', 'reversed')
    )
    OR EXISTS (
        SELECT 1 FROM vouchers 
        WHERE company_id = NEW.company_id 
          AND id = NEW.voucher_id 
          AND status IN ('posted', 'cancelled', 'reversed')
    )
    OR (NEW.company_id IS NOT OLD.company_id)
)
BEGIN
    SELECT RAISE(ABORT, 'Voucher line items belonging to or assigned to a posted, cancelled, or reversed voucher cannot be modified or reassigned.');
END;

-- Trigger 6: Prohibit Deleting Lines Belonging to Non-Draft Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_posted_voucher_lines_deletion
BEFORE DELETE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Voucher line items belonging to a posted, cancelled, or reversed voucher cannot be deleted.');
END;

-- Trigger 7: Prevent Deleting Ledgers Referenced in Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_ledger_delete_with_vouchers
BEFORE DELETE ON ledgers
FOR EACH ROW
WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
  AND EXISTS (SELECT 1 FROM voucher_lines WHERE company_id = OLD.company_id AND ledger_id = OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete ledger: it is referenced in existing voucher lines.');
END;

-- Trigger 8: Prohibit Hard Deletion of Companies Containing Financial Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_company_delete_with_financial_vouchers
BEFORE DELETE ON companies
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete company: Company contains financial accounting records. As an application-level financial-history protection mechanism (subject to professional legal and statutory review), companies with financial records cannot be deleted and must be soft-archived.');
END;

-- Trigger 8a: Prohibit Mutating Series Code or Numbering Method on Active Series
CREATE TRIGGER IF NOT EXISTS trg_prevent_series_code_mutation
BEFORE UPDATE OF series_code, numbering_method ON voucher_number_series
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.id
      AND v.status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot modify series_code or numbering_method: permanent vouchers exist in this series.');
END;

-- Trigger 8b: Prohibit Deleting Series Referencing Permanent Vouchers
CREATE TRIGGER IF NOT EXISTS trg_prevent_series_deletion_with_vouchers
BEFORE DELETE ON voucher_number_series
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.id
      AND v.status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete series: permanent vouchers exist in this series.');
END;

-- Trigger 9b: Series-Scoped Immutability on UPDATE for Active Configurations
CREATE TRIGGER IF NOT EXISTS trg_prevent_active_config_mutation
BEFORE UPDATE ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= OLD.applicable_from
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot modify active numbering configuration: permanent vouchers exist in this series governed by this configuration. Append a new configuration row with a future applicable_from date instead.');
END;

-- Trigger 10: Series-Scoped Immutability on DELETE
CREATE TRIGGER IF NOT EXISTS trg_prevent_active_config_deletion
BEFORE DELETE ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = OLD.company_id
      AND v.series_id = OLD.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= OLD.applicable_from
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete active numbering configuration: permanent vouchers exist in this series governed by this configuration.');
END;

-- Trigger 11: Prohibit Retroactive Numbering Configuration INSERT
CREATE TRIGGER IF NOT EXISTS trg_prevent_retroactive_config_insert
BEFORE INSERT ON voucher_number_configs
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers v
    WHERE v.company_id = NEW.company_id
      AND v.series_id = NEW.series_id
      AND v.status IN ('posted', 'reversed', 'cancelled')
      AND v.date >= NEW.applicable_from
      AND (
          NOT EXISTS (
              SELECT 1 FROM voucher_number_configs c
              WHERE c.company_id = NEW.company_id
                AND c.series_id = NEW.series_id
                AND c.applicable_from > NEW.applicable_from
                AND v.date >= c.applicable_from
          )
      )
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot insert retroactive numbering configuration: permanent vouchers already exist in this series governed by the proposed effective period.');
END;
"#;

/// Execute the Stage 4 Hardening Forward Migration (0.3.0-sprint2 -> 0.3.1-sprint2).
/// Upgrades vouchers table with hardened chk_voucher_sequence_metadata and voucher_lines
/// with chk_voucher_line_number_bounds using the 12-step table-rebuild pattern.
pub async fn run_sprint2_stage4_hardening_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    // STEP 1: Dedicated connection checked out from the pool
    let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;

    // STEP 2: Read and verify source schema version
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

    if matches!(
        current_version.as_str(),
        "0.3.1-sprint2"
            | "0.4.0-sprint4"
            | "0.4.1-sprint4"
            | "0.4.2-sprint4"
            | "0.4.3-sprint5"
            | "0.4.4-sprint5"
            | "0.4.5-sprint6"
            | "0.4.6-sprint6"
    ) {
        tracing::info!(
            "Database already at schema version 0.3.1-sprint2; Stage 4 hardening migration skipped (idempotent)."
        );
        return Ok(());
    }

    if current_version != "0.3.0-sprint2" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.3.0-sprint2' for Stage 4 hardening migration, found '{}'",
            current_version
        )));
    }

    let mut fk_disabled = false;
    let mut in_transaction = false;

    let migration_res: Result<(), OpenTallyError> = async {
        // STEP 3: Disable foreign keys OUTSIDE any transaction & verify status
        sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        fk_disabled = true;

        let fk_status: i64 = sqlx::query_scalar("PRAGMA foreign_keys;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if fk_status != 0 {
            return Err(OpenTallyError::Migration(
                "Failed to disable foreign keys prior to Stage 4 hardening table rebuild (status != 0)".into(),
            ));
        }

        // STEP 4: Begin immediate exclusive write transaction (WAL write lock)
        sqlx::raw_sql("BEGIN IMMEDIATE;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        in_transaction = true;

        // STEP 5: Pre-migration fail-closed integrity validation
        // 5a. Validate voucher line bounds (Fail-closed on ANY committed line outside active [1, 10000])
        let corrupted_lines_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM voucher_lines
             WHERE COALESCE(line_number BETWEEN 1 AND 10000, 0) <> 1;",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if corrupted_lines_count > 0 {
            let sample_lines = sqlx::query_as::<_, (String, String, String, i64)>(
                "SELECT company_id, voucher_id, id, line_number
                 FROM voucher_lines
                 WHERE COALESCE(line_number BETWEEN 1 AND 10000, 0) <> 1
                 LIMIT 10;",
            )
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            return Err(OpenTallyError::IntegrityViolation(format!(
                "Pre-migration validation failed: found {} corrupted/staging voucher line(s) outside supported active range [1, 10000]. Sample: {:?}",
                corrupted_lines_count,
                sample_lines
            )));
        }

        // 5b. Validate voucher sequence metadata
        let invalid_vouchers = sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<i64>)>(
            "SELECT company_id, id, status, numbering_method, period_start, sequence_number
             FROM vouchers
             WHERE COALESCE(
                 CASE
                     WHEN status = 'draft' THEN (
                         period_start IS NULL
                         AND sequence_number IS NULL
                     )
                     WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'automatic' THEN (
                         period_start IS NOT NULL
                         AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                         AND date(period_start) IS NOT NULL
                         AND date(period_start) = period_start
                         AND sequence_number IS NOT NULL
                         AND sequence_number > 0
                     )
                     WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'manual' THEN (
                         sequence_number IS NULL
                         AND (
                             period_start IS NULL
                             OR (
                                 period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                                 AND date(period_start) IS NOT NULL
                                 AND date(period_start) = period_start
                             )
                         )
                     )
                     ELSE 0
                 END, 0
             ) != 1
             LIMIT 10;",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if !invalid_vouchers.is_empty() {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Pre-migration validation failed: found {} voucher(s) with invalid sequence metadata. Sample: {:?}",
                invalid_vouchers.len(),
                invalid_vouchers
            )));
        }

        // 5c. Validate voucher dates
        let invalid_dates = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT company_id, id, date, effective_date
             FROM vouchers
             WHERE COALESCE(date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(date) IS NOT NULL AND date(date) = date, 0) != 1
                OR COALESCE(effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(effective_date) IS NOT NULL AND date(effective_date) = effective_date, 0) != 1
             LIMIT 10;",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if !invalid_dates.is_empty() {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Pre-migration validation failed: found {} voucher(s) with invalid calendar dates. Sample: {:?}",
                invalid_dates.len(),
                invalid_dates
            )));
        }

        // STEP 6: Enumerate and drop dependent triggers from sqlite_master
        // Account for:
        // - Triggers attached to vouchers
        // - Triggers attached to voucher_lines
        // - Triggers attached to other tables referencing vouchers
        // - Triggers attached to other tables referencing voucher_lines
        let dynamic_triggers = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM sqlite_master 
             WHERE type = 'trigger' 
               AND (tbl_name IN ('vouchers', 'voucher_lines') OR sql LIKE '%vouchers%' OR sql LIKE '%voucher_lines%');",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let mut triggers_to_drop = std::collections::BTreeSet::new();
        for (name,) in dynamic_triggers {
            triggers_to_drop.insert(name);
        }
        for trg in CANONICAL_STAGE4_DEPENDENT_TRIGGERS {
            triggers_to_drop.insert(trg.to_string());
        }

        for trg_name in &triggers_to_drop {
            let drop_sql = format!("DROP TRIGGER IF EXISTS {};", trg_name);
            sqlx::raw_sql(&drop_sql)
                .execute(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;
        }

        // STEP 7: Rebuild vouchers table
        sqlx::raw_sql(STAGE4_HARDENING_VOUCHERS_TABLE_TMP_DDL)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::raw_sql(
            "INSERT INTO vouchers_dg_tmp (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, reference_no, reference_date, narration, total_amount,
                reversal_of_voucher_id, created_at, updated_at
            )
            SELECT 
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number, date, effective_date,
                status, reference_no, reference_date, narration, total_amount,
                reversal_of_voucher_id, created_at, updated_at
            FROM vouchers;",
        )
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Verify voucher row counts
        let vch_src: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vouchers;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        let vch_dst: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vouchers_dg_tmp;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if vch_src != vch_dst {
            return Err(OpenTallyError::Migration(format!(
                "Voucher count mismatch during Stage 4 hardening: source={}, destination={}",
                vch_src, vch_dst
            )));
        }

        // Bidirectional EXCEPT difference verification for vouchers
        let vch_diff_fwd: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, reference_no, reference_date, narration, total_amount, reversal_of_voucher_id, created_at, updated_at FROM vouchers
                EXCEPT
                SELECT id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, reference_no, reference_date, narration, total_amount, reversal_of_voucher_id, created_at, updated_at FROM vouchers_dg_tmp
            );",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let vch_diff_rev: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, reference_no, reference_date, narration, total_amount, reversal_of_voucher_id, created_at, updated_at FROM vouchers_dg_tmp
                EXCEPT
                SELECT id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, reference_no, reference_date, narration, total_amount, reversal_of_voucher_id, created_at, updated_at FROM vouchers
            );",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if vch_diff_fwd != 0 || vch_diff_rev != 0 {
            return Err(OpenTallyError::Migration(format!(
                "Bidirectional data difference detected for vouchers: forward={}, reverse={}",
                vch_diff_fwd, vch_diff_rev
            )));
        }

        sqlx::raw_sql("DROP TABLE vouchers;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::raw_sql("ALTER TABLE vouchers_dg_tmp RENAME TO vouchers;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // STEP 8: Rebuild voucher_lines table
        sqlx::raw_sql(STAGE4_HARDENING_VOUCHER_LINES_TABLE_TMP_DDL)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::raw_sql(
            "INSERT INTO voucher_lines_dg_tmp (
                id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
            )
            SELECT 
                id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
            FROM voucher_lines;",
        )
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // Verify voucher_lines row counts
        let vl_src: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        let vl_dst: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM voucher_lines_dg_tmp;")
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if vl_src != vl_dst {
            return Err(OpenTallyError::Migration(format!(
                "Voucher lines count mismatch during Stage 4 hardening: source={}, destination={}",
                vl_src, vl_dst
            )));
        }

        // Bidirectional EXCEPT difference verification for voucher_lines
        let vl_diff_fwd: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at FROM voucher_lines
                EXCEPT
                SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at FROM voucher_lines_dg_tmp
            );",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        let vl_diff_rev: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (
                SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at FROM voucher_lines_dg_tmp
                EXCEPT
                SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at FROM voucher_lines
            );",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if vl_diff_fwd != 0 || vl_diff_rev != 0 {
            return Err(OpenTallyError::Migration(format!(
                "Bidirectional data difference detected for voucher lines: forward={}, reverse={}",
                vl_diff_fwd, vl_diff_rev
            )));
        }

        sqlx::raw_sql("DROP TABLE voucher_lines;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::raw_sql("ALTER TABLE voucher_lines_dg_tmp RENAME TO voucher_lines;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // STEP 9: Recreate indexes and triggers
        sqlx::raw_sql(STAGE4_HARDENING_INDEXES_AND_TRIGGERS_DDL)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // Explicit post-rebuild verification: Ensure all 14 canonical triggers were successfully restored
        for trg in CANONICAL_STAGE4_DEPENDENT_TRIGGERS {
            let exists: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'trigger' AND name = $1;",
            )
            .bind(trg)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if !exists {
                return Err(OpenTallyError::Migration(format!(
                    "Post-rebuild verification failed: canonical trigger '{}' was not restored",
                    trg
                )));
            }
        }

        // STEP 10: Update schema version in _opentally_metadata
        let app_version = env!("CARGO_PKG_VERSION");
        sqlx::query(
            "UPDATE _opentally_metadata 
             SET schema_version = '0.3.1-sprint2', app_version = $1
             WHERE id = 1;",
        )
        .bind(app_version)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        // STEP 11: Verify Foreign Key Integrity
        let violations = sqlx::query("PRAGMA foreign_key_check;")
            .fetch_all(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        if !violations.is_empty() {
            return Err(OpenTallyError::Migration(format!(
                "Foreign key integrity check failed with {} violation(s) after Stage 4 hardening migration",
                violations.len()
            )));
        }

        // STEP 12: Commit transaction
        sqlx::raw_sql("COMMIT;")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        in_transaction = false;

        Ok(())
    }
    .await;

    // STEP 13 & 14: Connection Cleanup and Rollback Handling (4 Exit States)
    if in_transaction {
        tracing::warn!("Migration 0.3.1-sprint2 transaction active upon exit; issuing ROLLBACK;");
        match sqlx::raw_sql("ROLLBACK;").execute(&mut *conn).await {
            Ok(_) => {
                tracing::info!(
                    "State B: ROLLBACK executed successfully; transaction aborted cleanly."
                );
            }
            Err(rb_err) => {
                // State C: Critical rollback failure
                tracing::error!(
                    "State C: CRITICAL - ROLLBACK failed ({:?})! Closing and discarding connection.",
                    rb_err
                );
                let _ = conn.close().await;
                return Err(OpenTallyError::Migration(format!(
                    "Fatal migration error: ROLLBACK failed ({:?}); connection closed and discarded",
                    rb_err
                )));
            }
        }
    }

    if fk_disabled {
        let restore_res = sqlx::raw_sql("PRAGMA foreign_keys = ON;")
            .execute(&mut *conn)
            .await;
        let verify_status: Result<i64, _> = sqlx::query_scalar("PRAGMA foreign_keys;")
            .fetch_one(&mut *conn)
            .await;
        let fk_re_enabled = restore_res.is_ok() && matches!(verify_status, Ok(1));

        if !fk_re_enabled {
            // State D: Foreign key restoration failure
            tracing::error!(
                "State D: CRITICAL - Failed to restore or verify foreign keys (status: {:?})! Discarding connection.",
                verify_status
            );
            let _ = conn.close().await;
            return Err(OpenTallyError::Migration(format!(
                "Fatal migration error: Foreign keys could not be restored to ON (status: {:?}); connection closed and discarded",
                verify_status
            )));
        }
    }

    match migration_res {
        Ok(()) => {
            let final_version: String =
                sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

            if final_version != "0.3.1-sprint2" {
                let _ = conn.close().await;
                return Err(OpenTallyError::Migration(format!(
                    "Final schema version verification failed: expected '0.3.1-sprint2', found '{}'",
                    final_version
                )));
            }

            // State A: Clean Success
            tracing::info!("State A: Migration 0.3.1-sprint2 completed successfully.");
            Ok(())
        }
        Err(err) => {
            // State B: Clean Rollback
            tracing::info!("State B: Migration 0.3.1-sprint2 failed and rolled back cleanly.");
            Err(err)
        }
    }
}

/// Execute the Sprint 4 inventory-foundation migration (0.3.1-sprint2 -> 0.4.0-sprint4).
/// Master data is intentionally isolated from vouchers and stock movements; later stages
/// may add transaction tables without changing these master contracts.
pub async fn run_sprint4_inventory_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;

    if matches!(
        current.as_str(),
        "0.4.0-sprint4"
            | "0.4.1-sprint4"
            | "0.4.2-sprint4"
            | "0.4.3-sprint5"
            | "0.4.4-sprint5"
            | "0.4.5-sprint6"
            | "0.4.6-sprint6"
    ) {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.3.1-sprint2" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.3.1-sprint2' for Sprint 4 inventory migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql(
        r#"
        CREATE TABLE stock_groups (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            parent_id TEXT,
            description TEXT,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, parent_id) REFERENCES stock_groups(company_id, id) ON DELETE RESTRICT,
            CHECK (parent_id IS NULL OR parent_id <> id)
        );
        CREATE INDEX idx_stock_groups_parent ON stock_groups(company_id, parent_id);
        CREATE TABLE stock_categories (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            description TEXT,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        CREATE TABLE units (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            symbol TEXT NOT NULL COLLATE NOCASE,
            formal_name TEXT NOT NULL COLLATE NOCASE,
            unit_type TEXT NOT NULL DEFAULT 'simple' CHECK (unit_type IN ('simple', 'compound')),
            base_unit_id TEXT,
            secondary_unit_id TEXT,
            conversion_factor TEXT,
            decimal_precision INTEGER NOT NULL DEFAULT 2 CHECK (decimal_precision BETWEEN 0 AND 4),
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, symbol),
            UNIQUE (company_id, formal_name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, base_unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, secondary_unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            CHECK ((unit_type = 'simple' AND base_unit_id IS NULL AND secondary_unit_id IS NULL AND conversion_factor IS NULL)
                OR (unit_type = 'compound' AND base_unit_id IS NOT NULL AND secondary_unit_id IS NOT NULL
                    AND base_unit_id <> secondary_unit_id AND conversion_factor IS NOT NULL))
        );
        CREATE TABLE godowns (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            parent_id TEXT,
            description TEXT,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, parent_id) REFERENCES godowns(company_id, id) ON DELETE RESTRICT,
            CHECK (parent_id IS NULL OR parent_id <> id)
        );
        CREATE INDEX idx_godowns_parent ON godowns(company_id, parent_id);
        CREATE TABLE stock_items (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            stock_group_id TEXT NOT NULL,
            stock_category_id TEXT,
            base_unit_id TEXT NOT NULL,
            alternate_unit_id TEXT,
            sku TEXT COLLATE NOCASE,
            description TEXT,
            opening_quantity TEXT NOT NULL DEFAULT '0',
            opening_rate TEXT NOT NULL DEFAULT '0',
            opening_value TEXT NOT NULL DEFAULT '0',
            reorder_level TEXT NOT NULL DEFAULT '0',
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, name),
            UNIQUE (company_id, sku),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_group_id) REFERENCES stock_groups(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, stock_category_id) REFERENCES stock_categories(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, base_unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, alternate_unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            CHECK (opening_quantity NOT GLOB '*[^0-9.]*' AND opening_quantity NOT GLOB '*.*.*' AND length(opening_quantity) > 0),
            CHECK (opening_rate NOT GLOB '*[^0-9.]*' AND opening_rate NOT GLOB '*.*.*' AND length(opening_rate) > 0),
            CHECK (opening_value NOT GLOB '*[^0-9.]*' AND opening_value NOT GLOB '*.*.*' AND length(opening_value) > 0),
            CHECK (reorder_level NOT GLOB '*[^0-9.]*' AND reorder_level NOT GLOB '*.*.*' AND length(reorder_level) > 0)
        );
        CREATE INDEX idx_stock_items_group ON stock_items(company_id, stock_group_id);
        CREATE INDEX idx_stock_items_category ON stock_items(company_id, stock_category_id);
        CREATE INDEX idx_stock_items_active_name ON stock_items(company_id, is_active, name);
        "#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE _opentally_metadata SET schema_version = '0.4.0-sprint4', app_version = ? WHERE id = 1")
        .bind(env!("CARGO_PKG_VERSION"))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Execute the Sprint 4 Stage 11 stock-voucher migration.
pub async fn run_sprint4_stage11_stock_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if matches!(
        current.as_str(),
        "0.4.1-sprint4"
            | "0.4.2-sprint4"
            | "0.4.3-sprint5"
            | "0.4.4-sprint5"
            | "0.4.5-sprint6"
            | "0.4.6-sprint6"
    ) {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.0-sprint4" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.0-sprint4' for Stage 11 migration, found '{}'",
            current
        )));
    }
    sqlx::raw_sql(r#"
        CREATE TABLE stock_vouchers (
            id TEXT NOT NULL, company_id TEXT NOT NULL, voucher_number TEXT NOT NULL,
            voucher_type TEXT NOT NULL CHECK (voucher_type IN ('receipt','delivery','transfer','adjustment')),
            voucher_date TEXT NOT NULL, reference_no TEXT, narration TEXT,
            status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','posted','cancelled','reversed')),
            reversal_of_voucher_id TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id,id), UNIQUE (company_id,voucher_number),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id,reversal_of_voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT,
            CHECK (voucher_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(voucher_date) IS NOT NULL)
        );
        CREATE TABLE stock_voucher_lines (
            id TEXT NOT NULL, company_id TEXT NOT NULL, voucher_id TEXT NOT NULL, line_number INTEGER NOT NULL,
            stock_item_id TEXT NOT NULL, unit_id TEXT NOT NULL, godown_id TEXT,
            source_godown_id TEXT, destination_godown_id TEXT, quantity TEXT NOT NULL,
            billed_quantity TEXT,
            system_quantity TEXT, physical_quantity TEXT, difference_quantity TEXT,
            description TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id,id), UNIQUE (company_id,voucher_id,line_number),
            FOREIGN KEY (company_id,voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE CASCADE,
            FOREIGN KEY (company_id,stock_item_id) REFERENCES stock_items(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,unit_id) REFERENCES units(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,godown_id) REFERENCES godowns(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,source_godown_id) REFERENCES godowns(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,destination_godown_id) REFERENCES godowns(company_id,id) ON DELETE RESTRICT,
            CHECK (length(quantity) > 0 AND quantity NOT GLOB '*[^0-9.]*' AND quantity NOT GLOB '*.*.*')
        );
        CREATE TABLE stock_movements (
            id TEXT NOT NULL, company_id TEXT NOT NULL, stock_item_id TEXT NOT NULL, godown_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL, voucher_line_id TEXT NOT NULL, movement_date TEXT NOT NULL,
            movement_type TEXT NOT NULL CHECK (movement_type IN ('receipt','delivery','transfer_in','transfer_out','adjustment')),
            signed_quantity TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id,id), UNIQUE (company_id,voucher_line_id,godown_id,movement_type),
            FOREIGN KEY (company_id,stock_item_id) REFERENCES stock_items(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,godown_id) REFERENCES godowns(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,voucher_line_id) REFERENCES stock_voucher_lines(company_id,id) ON DELETE RESTRICT,
            CHECK (length(signed_quantity) > 0 AND signed_quantity NOT GLOB '*[^0-9.-]*' AND signed_quantity NOT GLOB '*.*.*'),
            CHECK (movement_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(movement_date) IS NOT NULL)
        );
        CREATE INDEX idx_stock_vouchers_company_date ON stock_vouchers(company_id,voucher_date,id);
        CREATE INDEX idx_stock_vouchers_status ON stock_vouchers(company_id,status,voucher_date);
        CREATE INDEX idx_stock_lines_item ON stock_voucher_lines(company_id,stock_item_id,voucher_id);
        CREATE INDEX idx_stock_movements_item_godown_date ON stock_movements(company_id,stock_item_id,godown_id,movement_date,id);
        CREATE INDEX idx_stock_movements_voucher ON stock_movements(company_id,voucher_id);
    "#).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version='0.4.1-sprint4', app_version=? WHERE id=1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Sprint 4 Stage 12 inventory valuation schema. Additive and backwards compatible.
pub async fn run_sprint4_stage12_valuation_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
    if matches!(
        current.as_str(),
        "0.4.2-sprint4" | "0.4.3-sprint5" | "0.4.4-sprint5" | "0.4.5-sprint6" | "0.4.6-sprint6"
    ) {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.1-sprint4" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.1-sprint4' for Stage 12 migration, found '{}'",
            current
        )));
    }
    sqlx::raw_sql(r#"
        ALTER TABLE stock_voucher_lines ADD COLUMN unit_cost TEXT;
        ALTER TABLE stock_voucher_lines ADD COLUMN line_value TEXT;
        CREATE TABLE inventory_valuation_config (
            company_id TEXT PRIMARY KEY NOT NULL,
            valuation_method TEXT NOT NULL DEFAULT 'fifo' CHECK (valuation_method IN ('fifo','weighted_average')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        INSERT INTO inventory_valuation_config(company_id) SELECT id FROM companies;
        CREATE TABLE inventory_cost_layers (
            id TEXT NOT NULL, company_id TEXT NOT NULL, stock_item_id TEXT NOT NULL,
            godown_id TEXT NOT NULL, source_voucher_id TEXT, source_line_id TEXT,
            receipt_date TEXT NOT NULL, original_quantity TEXT NOT NULL,
            remaining_quantity TEXT NOT NULL, unit_cost TEXT NOT NULL,
            original_value TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_id,id),
            FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY(company_id,stock_item_id) REFERENCES stock_items(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,godown_id) REFERENCES godowns(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,source_voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,source_line_id) REFERENCES stock_voucher_lines(company_id,id) ON DELETE RESTRICT
        );
        CREATE TABLE inventory_valuation_consumptions (
            id TEXT NOT NULL, company_id TEXT NOT NULL, movement_id TEXT NOT NULL,
            layer_id TEXT NOT NULL, quantity TEXT NOT NULL, unit_cost TEXT NOT NULL,
            value TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_id,id), UNIQUE(company_id,movement_id,layer_id),
            FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY(company_id,layer_id) REFERENCES inventory_cost_layers(company_id,id) ON DELETE RESTRICT
        );
        CREATE INDEX idx_inventory_layers_item_date ON inventory_cost_layers(company_id,stock_item_id,receipt_date,id);
        CREATE INDEX idx_inventory_consumptions_movement ON inventory_valuation_consumptions(company_id,movement_id);
    "#).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version='0.4.2-sprint4', app_version=? WHERE id=1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Sprint 5 Stage 14 durable inventory/accounting integration state.
pub async fn run_sprint5_stage14_integration_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
    if matches!(
        current.as_str(),
        "0.4.3-sprint5" | "0.4.4-sprint5" | "0.4.5-sprint6" | "0.4.6-sprint6"
    ) {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.2-sprint4" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.2-sprint4' for Stage 14 migration, found '{}'",
            current
        )));
    }
    sqlx::raw_sql(r#"
        CREATE TABLE inventory_account_mappings (
            company_id TEXT PRIMARY KEY NOT NULL,
            inventory_asset_ledger_id TEXT NOT NULL,
            adjustment_expense_ledger_id TEXT NOT NULL,
            adjustment_gain_ledger_id TEXT,
            cogs_ledger_id TEXT NOT NULL,
            receipt_offset_ledger_id TEXT NOT NULL,
            journal_series_id TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0,1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id,inventory_asset_ledger_id) REFERENCES ledgers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,adjustment_expense_ledger_id) REFERENCES ledgers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,adjustment_gain_ledger_id) REFERENCES ledgers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,cogs_ledger_id) REFERENCES ledgers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,receipt_offset_ledger_id) REFERENCES ledgers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id,journal_series_id) REFERENCES voucher_number_series(company_id,id) ON DELETE RESTRICT
        );
        CREATE TABLE inventory_accounting_integrations (
            id TEXT NOT NULL, company_id TEXT NOT NULL, stock_voucher_id TEXT NOT NULL,
            stock_voucher_type TEXT NOT NULL, accounting_voucher_id TEXT,
            status TEXT NOT NULL CHECK (status IN ('pending','integrated','failed')),
            valuation_method TEXT, integrated_amount TEXT NOT NULL DEFAULT '0',
            attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
            error_details TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY(company_id,id), UNIQUE(company_id,stock_voucher_id),
            FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY(company_id,stock_voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,accounting_voucher_id) REFERENCES vouchers(company_id,id) ON DELETE RESTRICT
        );
        CREATE INDEX idx_inventory_integrations_status ON inventory_accounting_integrations(company_id,status,updated_at);
    "#).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version='0.4.3-sprint5', app_version=? WHERE id=1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Sprint 5 Stage 15 immutable per-voucher valuation attribution.
pub async fn run_sprint5_stage15_valuation_attribution_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id=1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "0.4.4-sprint5" {
        sqlx::raw_sql(r#"
            CREATE TRIGGER IF NOT EXISTS trg_inventory_attr_no_update BEFORE UPDATE ON inventory_valuation_attributions BEGIN SELECT RAISE(ABORT,'valuation attribution is immutable'); END;
            CREATE TRIGGER IF NOT EXISTS trg_inventory_attr_no_delete BEFORE DELETE ON inventory_valuation_attributions BEGIN SELECT RAISE(ABORT,'valuation attribution is immutable'); END;
            CREATE TRIGGER IF NOT EXISTS trg_inventory_attr_layer_no_update BEFORE UPDATE ON inventory_valuation_layer_uses BEGIN SELECT RAISE(ABORT,'valuation layer use is immutable'); END;
            CREATE TRIGGER IF NOT EXISTS trg_inventory_attr_layer_no_delete BEFORE DELETE ON inventory_valuation_layer_uses BEGIN SELECT RAISE(ABORT,'valuation layer use is immutable'); END;
        "#).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(());
    }
    if matches!(current.as_str(), "0.4.5-sprint6" | "0.4.6-sprint6") {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.3-sprint5" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.3-sprint5' for Stage 15 migration, found '{}'",
            current
        )));
    }
    sqlx::raw_sql(r#"
        CREATE TABLE inventory_valuation_attributions (
            id TEXT NOT NULL, company_id TEXT NOT NULL, stock_voucher_id TEXT NOT NULL,
            voucher_type TEXT NOT NULL, voucher_line_id TEXT NOT NULL, stock_item_id TEXT NOT NULL,
            stock_movement_id TEXT NOT NULL, valuation_method TEXT NOT NULL,
            quantity TEXT NOT NULL, unit_cost TEXT NOT NULL, total_value TEXT NOT NULL,
            context TEXT NOT NULL, posting_timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            reversal_of_attribution_id TEXT,
            PRIMARY KEY(company_id,id), UNIQUE(company_id,stock_movement_id),
            FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY(company_id,stock_voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,voucher_line_id) REFERENCES stock_voucher_lines(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,stock_item_id) REFERENCES stock_items(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,stock_movement_id) REFERENCES stock_movements(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,reversal_of_attribution_id) REFERENCES inventory_valuation_attributions(company_id,id) ON DELETE RESTRICT
        );
        CREATE TABLE inventory_valuation_layer_uses (
            id TEXT NOT NULL, company_id TEXT NOT NULL, attribution_id TEXT NOT NULL,
            source_movement_id TEXT, source_voucher_id TEXT, quantity TEXT NOT NULL,
            unit_cost TEXT NOT NULL, total_value TEXT NOT NULL,
            PRIMARY KEY(company_id,id), UNIQUE(company_id,attribution_id,source_movement_id,quantity,unit_cost),
            FOREIGN KEY(company_id,attribution_id) REFERENCES inventory_valuation_attributions(company_id,id) ON DELETE CASCADE,
            FOREIGN KEY(company_id,source_movement_id) REFERENCES stock_movements(company_id,id) ON DELETE RESTRICT,
            FOREIGN KEY(company_id,source_voucher_id) REFERENCES stock_vouchers(company_id,id) ON DELETE RESTRICT
        );
        CREATE INDEX idx_inventory_attr_voucher ON inventory_valuation_attributions(company_id,stock_voucher_id,voucher_line_id);
        CREATE INDEX idx_inventory_attr_item ON inventory_valuation_attributions(company_id,stock_item_id,posting_timestamp);
        CREATE INDEX idx_inventory_attr_layer ON inventory_valuation_layer_uses(company_id,attribution_id);
        CREATE TRIGGER trg_inventory_attr_no_update BEFORE UPDATE ON inventory_valuation_attributions BEGIN SELECT RAISE(ABORT,'valuation attribution is immutable'); END;
        CREATE TRIGGER trg_inventory_attr_no_delete BEFORE DELETE ON inventory_valuation_attributions BEGIN SELECT RAISE(ABORT,'valuation attribution is immutable'); END;
        CREATE TRIGGER trg_inventory_attr_layer_no_update BEFORE UPDATE ON inventory_valuation_layer_uses BEGIN SELECT RAISE(ABORT,'valuation layer use is immutable'); END;
        CREATE TRIGGER trg_inventory_attr_layer_no_delete BEFORE DELETE ON inventory_valuation_layer_uses BEGIN SELECT RAISE(ABORT,'valuation layer use is immutable'); END;
    "#).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version='0.4.4-sprint5',app_version=? WHERE id=1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Additive Stage 17 Party Masters migration. Party roles are extension metadata;
/// the existing ledger classification CHECK constraint is intentionally unchanged.
pub async fn run_sprint6_stage17_party_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if matches!(current.as_str(), "0.4.5-sprint6" | "0.4.6-sprint6") {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.4-sprint5" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.4-sprint5' for Stage 17, found '{}'",
            current
        )));
    }

    sqlx::raw_sql(
        "CREATE TABLE party_profiles (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            ledger_id TEXT NOT NULL,
            party_code TEXT NOT NULL COLLATE NOCASE,
            display_name TEXT NOT NULL COLLATE NOCASE,
            customer_enabled INTEGER NOT NULL DEFAULT 0 CHECK (customer_enabled IN (0,1)),
            supplier_enabled INTEGER NOT NULL DEFAULT 0 CHECK (supplier_enabled IN (0,1)),
            contact_person TEXT,
            email TEXT,
            phone TEXT,
            billing_address TEXT,
            shipping_address TEXT,
            notes TEXT,
            credit_period_days INTEGER NOT NULL DEFAULT 0 CHECK (credit_period_days >= 0),
            credit_limit TEXT NOT NULL DEFAULT '0',
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, party_code),
            UNIQUE (company_id, ledger_id),
            FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            CHECK (customer_enabled = 1 OR supplier_enabled = 1)
        );
        CREATE INDEX idx_party_profiles_company_name
            ON party_profiles(company_id, display_name, is_active);
        CREATE INDEX idx_party_profiles_company_roles
            ON party_profiles(company_id, customer_enabled, supplier_enabled, is_active);")
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = '0.4.5-sprint6', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Additive Stage 18 sales-document foundation. The existing voucher draft is
/// the lifecycle and future numbering source of truth; these tables hold only
/// sales-specific document and line metadata.
pub async fn run_sprint6_stage18_sales_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "0.4.6-sprint6" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.5-sprint6" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.5-sprint6' for Stage 18, found '{}'",
            current
        )));
    }
    sqlx::raw_sql(
        "CREATE TABLE sales_documents (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            party_id TEXT NOT NULL,
            invoice_date TEXT NOT NULL,
            document_reference TEXT,
            due_date TEXT NOT NULL,
            due_date_source TEXT NOT NULL CHECK (due_date_source IN ('party_default','explicit_override')),
            due_date_override_reason TEXT,
            narration TEXT,
            status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft')),
            subtotal TEXT NOT NULL DEFAULT '0',
            discount_total TEXT NOT NULL DEFAULT '0',
            additional_charges TEXT NOT NULL DEFAULT '0',
            net_total TEXT NOT NULL DEFAULT '0',
            tax_status TEXT NOT NULL DEFAULT 'inactive' CHECK (tax_status = 'inactive'),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, voucher_id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_id) REFERENCES party_profiles(company_id, id) ON DELETE RESTRICT
        );
        CREATE TABLE sales_document_lines (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            sales_document_id TEXT NOT NULL,
            line_number INTEGER NOT NULL,
            description TEXT NOT NULL,
            stock_item_id TEXT,
            unit_id TEXT,
            godown_id TEXT,
            quantity TEXT NOT NULL,
            billed_quantity TEXT,
            rate TEXT NOT NULL,
            discount_kind TEXT NOT NULL CHECK (discount_kind IN ('none','amount','percentage')),
            discount_value TEXT NOT NULL,
            discount_amount TEXT NOT NULL,
            line_amount TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, sales_document_id, line_number),
            FOREIGN KEY (company_id, sales_document_id) REFERENCES sales_documents(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, godown_id) REFERENCES godowns(company_id, id) ON DELETE RESTRICT
        );
        CREATE INDEX idx_sales_documents_company_date ON sales_documents(company_id, invoice_date, updated_at);
        CREATE INDEX idx_sales_documents_party ON sales_documents(company_id, party_id, invoice_date);
        CREATE INDEX idx_sales_lines_document ON sales_document_lines(company_id, sales_document_id, line_number);")
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = '0.4.6-sprint6', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Milestone 1 commercial posting migration.
/// Expands sales_documents.status check to ('draft','posted','cancelled','reversed')
/// and creates purchase_documents and purchase_document_lines.
pub async fn run_milestone1_commercial_posting_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "0.4.7-commercial" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.6-sprint6" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.6-sprint6' for Commercial Posting migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(r#"
        CREATE TABLE sales_documents_dg_tmp (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            party_id TEXT NOT NULL,
            invoice_date TEXT NOT NULL,
            document_reference TEXT,
            due_date TEXT NOT NULL,
            due_date_source TEXT NOT NULL CHECK (due_date_source IN ('party_default','explicit_override')),
            due_date_override_reason TEXT,
            narration TEXT,
            status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','posted','cancelled','reversed')),
            subtotal TEXT NOT NULL DEFAULT '0',
            discount_total TEXT NOT NULL DEFAULT '0',
            additional_charges TEXT NOT NULL DEFAULT '0',
            net_total TEXT NOT NULL DEFAULT '0',
            tax_status TEXT NOT NULL DEFAULT 'inactive',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, voucher_id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_id) REFERENCES party_profiles(company_id, id) ON DELETE RESTRICT
        );

        INSERT INTO sales_documents_dg_tmp SELECT
            id, company_id, voucher_id, party_id, invoice_date, document_reference,
            due_date, due_date_source, due_date_override_reason, narration, status,
            subtotal, discount_total, additional_charges, net_total, tax_status,
            created_at, updated_at
        FROM sales_documents;

        DROP TABLE sales_documents;

        ALTER TABLE sales_documents_dg_tmp RENAME TO sales_documents;

        CREATE INDEX idx_sales_documents_company_date ON sales_documents(company_id, invoice_date, updated_at);
        CREATE INDEX idx_sales_documents_party ON sales_documents(company_id, party_id, invoice_date);

        CREATE TABLE purchase_documents (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            party_id TEXT NOT NULL,
            bill_date TEXT NOT NULL,
            supplier_invoice_no TEXT,
            due_date TEXT NOT NULL,
            due_date_source TEXT NOT NULL CHECK (due_date_source IN ('party_default','explicit_override')),
            due_date_override_reason TEXT,
            narration TEXT,
            status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','posted','cancelled','reversed')),
            subtotal TEXT NOT NULL DEFAULT '0',
            discount_total TEXT NOT NULL DEFAULT '0',
            additional_charges TEXT NOT NULL DEFAULT '0',
            net_total TEXT NOT NULL DEFAULT '0',
            tax_status TEXT NOT NULL DEFAULT 'inactive',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, voucher_id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_id) REFERENCES party_profiles(company_id, id) ON DELETE RESTRICT
        );

        CREATE TABLE purchase_document_lines (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            purchase_document_id TEXT NOT NULL,
            line_number INTEGER NOT NULL,
            description TEXT NOT NULL,
            stock_item_id TEXT,
            unit_id TEXT,
            godown_id TEXT,
            quantity TEXT NOT NULL,
            billed_quantity TEXT,
            rate TEXT NOT NULL,
            discount_kind TEXT NOT NULL CHECK (discount_kind IN ('none','amount','percentage')),
            discount_value TEXT NOT NULL,
            discount_amount TEXT NOT NULL,
            line_amount TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, purchase_document_id, line_number),
            FOREIGN KEY (company_id, purchase_document_id) REFERENCES purchase_documents(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, unit_id) REFERENCES units(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, godown_id) REFERENCES godowns(company_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX idx_purchase_documents_company_date ON purchase_documents(company_id, bill_date, updated_at);
        CREATE INDEX idx_purchase_documents_party ON purchase_documents(company_id, party_id, bill_date);
        CREATE INDEX idx_purchase_lines_document ON purchase_document_lines(company_id, purchase_document_id, line_number);
    "#).execute(&mut *tx).await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = '0.4.7-commercial', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 2 Bill-Wise Outstanding Tracking (AR/AP) migration.
/// Creates table bill_allocations and indexes for party bill tracking.
pub async fn run_milestone2_billwise_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "0.5.0-billwise" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.4.7-commercial" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.4.7-commercial' for Bill-Wise Tracking migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(r#"
        CREATE TABLE IF NOT EXISTS bill_allocations (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            voucher_line_id TEXT NOT NULL,
            party_ledger_id TEXT NOT NULL,
            bill_name TEXT NOT NULL,
            allocation_type TEXT NOT NULL CHECK (allocation_type IN ('NewRef', 'AgstRef', 'Advance', 'OnAccount')),
            amount TEXT NOT NULL CHECK (
                length(amount) >= 1
                AND length(amount) <= 30
                AND amount NOT GLOB '*[^0-9.]*'
                AND amount NOT GLOB '*..*'
                AND amount NOT GLOB '.*'
                AND amount NOT GLOB '*.'
            ),
            dr_cr TEXT NOT NULL CHECK (dr_cr IN ('Dr', 'Cr')),
            bill_date TEXT NOT NULL CHECK (bill_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            due_date TEXT CHECK (due_date IS NULL OR due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_line_id) REFERENCES voucher_lines(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_bill_allocations_party ON bill_allocations(company_id, party_ledger_id, bill_name);
        CREATE INDEX IF NOT EXISTS idx_bill_allocations_voucher ON bill_allocations(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_bill_allocations_due ON bill_allocations(company_id, due_date);
        CREATE INDEX IF NOT EXISTS idx_bill_allocations_status ON bill_allocations(company_id, is_cancelled, allocation_type);
    "#).execute(&mut *tx).await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = '0.5.0-billwise', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 3 Indian GST Core Engine migration.
/// Creates tables company_gst_profiles, ledger_gst_profiles, hsn_sac_codes,
/// item_gst_mappings, gst_ledger_mappings, voucher_gst_details and associated indexes.
pub async fn run_milestone3_gst_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v0.6.0-gst" || current == "0.6.0-gst" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "0.5.0-billwise" && current != "v0.5.0-billwise" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version '0.5.0-billwise' for GST migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS company_gst_profiles (
            company_id TEXT NOT NULL PRIMARY KEY,
            gstin TEXT CHECK (gstin IS NULL OR length(gstin) = 15),
            state_code TEXT NOT NULL CHECK (length(state_code) = 2 AND state_code NOT GLOB '*[^0-9]*'),
            registration_type TEXT NOT NULL CHECK (registration_type IN ('Regular', 'Composition', 'Unregistered')),
            gst_enabled INTEGER NOT NULL DEFAULT 1 CHECK (gst_enabled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS ledger_gst_profiles (
            id TEXT NOT NULL PRIMARY KEY,
            company_id TEXT NOT NULL,
            ledger_id TEXT NOT NULL,
            gstin TEXT CHECK (gstin IS NULL OR length(gstin) = 15),
            state_code TEXT NOT NULL CHECK (length(state_code) = 2 AND state_code NOT GLOB '*[^0-9]*'),
            registration_type TEXT NOT NULL CHECK (registration_type IN ('Regular', 'Composition', 'Unregistered', 'Consumer')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE CASCADE,
            UNIQUE (company_id, ledger_id)
        );

        CREATE TABLE IF NOT EXISTS hsn_sac_codes (
            id TEXT NOT NULL PRIMARY KEY,
            company_id TEXT NOT NULL,
            code TEXT NOT NULL,
            description TEXT,
            code_type TEXT NOT NULL CHECK (code_type IN ('HSN', 'SAC')),
            default_gst_rate TEXT NOT NULL CHECK (length(default_gst_rate) > 0 AND default_gst_rate NOT GLOB '*[^0-9.]*' AND default_gst_rate GLOB '*.*' AND length(substr(default_gst_rate, instr(default_gst_rate, '.') + 1)) <= 4),
            cess_rate TEXT NOT NULL DEFAULT '0.00' CHECK (length(cess_rate) > 0 AND cess_rate NOT GLOB '*[^0-9.]*' AND cess_rate GLOB '*.*' AND length(substr(cess_rate, instr(cess_rate, '.') + 1)) <= 4),
            effective_from TEXT NOT NULL CHECK (effective_from GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            UNIQUE (company_id, code)
        );

        CREATE TABLE IF NOT EXISTS item_gst_mappings (
            id TEXT NOT NULL PRIMARY KEY,
            company_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            hsn_sac_code_id TEXT NOT NULL,
            gst_rate TEXT NOT NULL CHECK (length(gst_rate) > 0 AND gst_rate NOT GLOB '*[^0-9.]*' AND gst_rate GLOB '*.*' AND length(substr(gst_rate, instr(gst_rate, '.') + 1)) <= 4),
            cess_rate TEXT NOT NULL DEFAULT '0.00' CHECK (length(cess_rate) > 0 AND cess_rate NOT GLOB '*[^0-9.]*' AND cess_rate GLOB '*.*' AND length(substr(cess_rate, instr(cess_rate, '.') + 1)) <= 4),
            taxability TEXT NOT NULL CHECK (taxability IN ('Taxable', 'Exempt', 'NilRated', 'NonGST')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (hsn_sac_code_id) REFERENCES hsn_sac_codes(id) ON DELETE RESTRICT,
            UNIQUE (company_id, stock_item_id)
        );

        CREATE TABLE IF NOT EXISTS gst_ledger_mappings (
            company_id TEXT NOT NULL PRIMARY KEY,
            output_cgst_ledger_id TEXT NOT NULL,
            output_sgst_ledger_id TEXT NOT NULL,
            output_igst_ledger_id TEXT NOT NULL,
            output_cess_ledger_id TEXT,
            input_cgst_ledger_id TEXT NOT NULL,
            input_sgst_ledger_id TEXT NOT NULL,
            input_igst_ledger_id TEXT NOT NULL,
            input_cess_ledger_id TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, output_cgst_ledger_id) REFERENCES ledgers(company_id, id),
            FOREIGN KEY (company_id, output_sgst_ledger_id) REFERENCES ledgers(company_id, id),
            FOREIGN KEY (company_id, output_igst_ledger_id) REFERENCES ledgers(company_id, id),
            FOREIGN KEY (company_id, input_cgst_ledger_id) REFERENCES ledgers(company_id, id),
            FOREIGN KEY (company_id, input_sgst_ledger_id) REFERENCES ledgers(company_id, id),
            FOREIGN KEY (company_id, input_igst_ledger_id) REFERENCES ledgers(company_id, id)
        );

        CREATE TABLE IF NOT EXISTS voucher_gst_details (
            id TEXT NOT NULL PRIMARY KEY,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            party_ledger_id TEXT NOT NULL,
            party_gstin TEXT CHECK (party_gstin IS NULL OR length(party_gstin) = 15),
            hsn_sac_code TEXT NOT NULL,
            place_of_supply_state_code TEXT NOT NULL CHECK (length(place_of_supply_state_code) = 2 AND place_of_supply_state_code NOT GLOB '*[^0-9]*'),
            supply_type TEXT NOT NULL CHECK (supply_type IN ('IntraState', 'InterState')),
            taxability TEXT NOT NULL CHECK (taxability IN ('Taxable', 'Exempt', 'NilRated', 'NonGST')),
            taxable_value TEXT NOT NULL CHECK (length(taxable_value) > 0 AND taxable_value NOT GLOB '*[^0-9.]*' AND taxable_value GLOB '*.*' AND length(substr(taxable_value, instr(taxable_value, '.') + 1)) <= 4),
            gst_rate TEXT NOT NULL CHECK (length(gst_rate) > 0 AND gst_rate NOT GLOB '*[^0-9.]*' AND gst_rate GLOB '*.*' AND length(substr(gst_rate, instr(gst_rate, '.') + 1)) <= 4),
            cgst_amount TEXT NOT NULL CHECK (length(cgst_amount) > 0 AND cgst_amount NOT GLOB '*[^0-9.]*' AND cgst_amount GLOB '*.*' AND length(substr(cgst_amount, instr(cgst_amount, '.') + 1)) <= 4),
            sgst_amount TEXT NOT NULL CHECK (length(sgst_amount) > 0 AND sgst_amount NOT GLOB '*[^0-9.]*' AND sgst_amount GLOB '*.*' AND length(substr(sgst_amount, instr(sgst_amount, '.') + 1)) <= 4),
            igst_amount TEXT NOT NULL CHECK (length(igst_amount) > 0 AND igst_amount NOT GLOB '*[^0-9.]*' AND igst_amount GLOB '*.*' AND length(substr(igst_amount, instr(igst_amount, '.') + 1)) <= 4),
            cess_amount TEXT NOT NULL CHECK (length(cess_amount) > 0 AND cess_amount NOT GLOB '*[^0-9.]*' AND cess_amount GLOB '*.*' AND length(substr(cess_amount, instr(cess_amount, '.') + 1)) <= 4),
            is_reverse_charge INTEGER NOT NULL DEFAULT 0 CHECK (is_reverse_charge IN (0, 1)),
            is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
            supply_category TEXT NOT NULL DEFAULT 'Regular' CHECK (supply_category IN ('Regular', 'ExportWithLUT', 'ExportWithTax', 'SEZWithLUT', 'SEZWithTax', 'DeemedExport', 'Composition')),
            itc_eligibility TEXT NOT NULL DEFAULT 'Eligible' CHECK (itc_eligibility IN ('Eligible', 'IneligibleSec17_5', 'Rule42_43Reversed')),
            port_code TEXT,
            shipping_bill_no TEXT,
            shipping_bill_date TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_ledger_id) REFERENCES ledgers(company_id, id)
        );

        CREATE INDEX IF NOT EXISTS idx_ledger_gst_profiles_lookup ON ledger_gst_profiles(company_id, ledger_id);
        CREATE INDEX IF NOT EXISTS idx_hsn_sac_codes_lookup ON hsn_sac_codes(company_id, code);
        CREATE INDEX IF NOT EXISTS idx_item_gst_mappings_lookup ON item_gst_mappings(company_id, stock_item_id);
        CREATE INDEX IF NOT EXISTS idx_voucher_gst_details_company_vch ON voucher_gst_details(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_voucher_gst_details_party ON voucher_gst_details(company_id, party_ledger_id);
        CREATE INDEX IF NOT EXISTS idx_voucher_gst_details_hsn ON voucher_gst_details(company_id, hsn_sac_code);
        CREATE INDEX IF NOT EXISTS idx_voucher_gst_details_supply ON voucher_gst_details(company_id, supply_type, is_cancelled);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v0.6.0-gst', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 5 Commercial Notes (Credit Note / Debit Note) migration.
/// Creates table commercial_note_linkages and bumps schema_version to 'v0.7.0-notes'.
pub async fn run_milestone5_notes_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v0.7.0-notes" || current == "0.7.0-notes" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v0.6.0-gst" && current != "0.6.0-gst" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v0.6.0-gst' for Notes migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS commercial_note_linkages (
            id TEXT NOT NULL PRIMARY KEY,
            company_id TEXT NOT NULL,
            note_voucher_id TEXT NOT NULL,
            original_voucher_id TEXT NOT NULL,
            note_type TEXT NOT NULL CHECK (note_type IN ('credit_note', 'debit_note')),
            original_invoice_number TEXT NOT NULL,
            original_invoice_date TEXT NOT NULL,
            reason TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, note_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, original_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
            UNIQUE (company_id, note_voucher_id)
        );

        CREATE INDEX IF NOT EXISTS idx_commercial_note_orig ON commercial_note_linkages(company_id, original_voucher_id);

        CREATE TABLE IF NOT EXISTS database_backups (
            id TEXT NOT NULL PRIMARY KEY,
            backup_file TEXT NOT NULL,
            destination_path TEXT NOT NULL,
            sha256_checksum TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            action TEXT NOT NULL CHECK (action IN ('BACKUP_CREATED', 'RESTORE_COMPLETED')),
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_database_backups_action ON database_backups(action, created_at);

        CREATE TABLE IF NOT EXISTS database_backups (
            id TEXT NOT NULL PRIMARY KEY,
            backup_file TEXT NOT NULL,
            destination_path TEXT NOT NULL,
            sha256_checksum TEXT NOT NULL,
            schema_version TEXT NOT NULL,
            action TEXT NOT NULL CHECK (action IN ('BACKUP_CREATED', 'RESTORE_COMPLETED')),
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_database_backups_action ON database_backups(action, created_at);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v0.7.0-notes', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 8 Banking and Bank Reconciliation Statement (BRS) migration.
/// Creates table bank_allocations and bumps schema_version to 'v1.1.0-banking'.
pub async fn run_milestone8_banking_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.1.0-banking" || current == "1.1.0-banking" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v0.7.0-notes" && current != "0.7.0-notes" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v0.7.0-notes' for Banking migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS bank_allocations (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            voucher_line_id TEXT NOT NULL,
            bank_ledger_id TEXT NOT NULL,
            transaction_type TEXT NOT NULL CHECK (transaction_type IN ('Cheque', 'NEFT', 'RTGS', 'IMPS', 'UPI', 'DD', 'Card', 'ECS', 'Other')),
            instrument_number TEXT,
            instrument_date TEXT CHECK (instrument_date IS NULL OR instrument_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            favouring_name TEXT,
            bank_date TEXT CHECK (bank_date IS NULL OR bank_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            amount TEXT NOT NULL CHECK (
                length(amount) >= 1
                AND length(amount) <= 30
                AND amount NOT GLOB '*[^0-9.]*'
                AND amount NOT GLOB '*..*'
                AND amount NOT GLOB '.*'
                AND amount NOT GLOB '*.'
            ),
            dr_cr TEXT NOT NULL CHECK (dr_cr IN ('Dr', 'Cr')),
            is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_line_id) REFERENCES voucher_lines(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, bank_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_bank_alloc_reconcile ON bank_allocations(company_id, bank_ledger_id, bank_date);
        CREATE INDEX IF NOT EXISTS idx_bank_alloc_voucher ON bank_allocations(company_id, voucher_id);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.1.0-banking', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 9 Indian TDS & TCS Engine, Challan Tracking & Form 26Q migration.
/// Creates tables company_tds_profiles, ledger_tds_profiles, tds_nature_of_payments,
/// and voucher_tds_details, and stamps schema_version to 'v1.2.0-tds'.
pub async fn run_milestone9_tds_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.2.0-tds" || current == "1.2.0-tds" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.1.0-banking" && current != "1.1.0-banking" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.1.0-banking' for TDS migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS company_tds_profiles (
            company_id TEXT PRIMARY KEY NOT NULL,
            tan TEXT CHECK (tan IS NULL OR (length(tan) = 10 AND tan GLOB '[A-Z][A-Z][A-Z][A-Z][0-9][0-9][0-9][0-9][0-9][A-Z]')),
            deductor_type TEXT NOT NULL DEFAULT 'Company' CHECK (deductor_type IN ('Company', 'Individual_HUF', 'Firm', 'Government', 'Other')),
            tds_enabled INTEGER NOT NULL DEFAULT 0 CHECK (tds_enabled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS ledger_tds_profiles (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            ledger_id TEXT NOT NULL,
            pan TEXT CHECK (pan IS NULL OR (length(pan) = 10 AND pan GLOB '[A-Z][A-Z][A-Z][A-Z][A-Z][0-9][0-9][0-9][0-9][A-Z]')),
            deductee_type TEXT NOT NULL DEFAULT 'Company_Domestic' CHECK (deductee_type IN ('Individual_HUF', 'Company_Domestic', 'Company_Foreign', 'Firm', 'Other')),
            lower_deduction_cert_no TEXT,
            lower_deduction_rate TEXT CHECK (
                lower_deduction_rate IS NULL OR (
                    length(lower_deduction_rate) >= 1
                    AND length(lower_deduction_rate) <= 30
                    AND lower_deduction_rate NOT GLOB '*[^0-9.]*'
                    AND lower_deduction_rate NOT GLOB '*..*'
                    AND lower_deduction_rate NOT GLOB '.*'
                    AND lower_deduction_rate NOT GLOB '*.'
                )
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE CASCADE,
            CONSTRAINT uq_ledger_tds_profile UNIQUE (company_id, ledger_id)
        );

        CREATE TABLE IF NOT EXISTS tds_nature_of_payments (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            section_code TEXT NOT NULL,
            name TEXT NOT NULL,
            rate_individual_huf TEXT NOT NULL CHECK (
                length(rate_individual_huf) >= 1
                AND length(rate_individual_huf) <= 30
                AND rate_individual_huf NOT GLOB '*[^0-9.]*'
                AND rate_individual_huf NOT GLOB '*..*'
                AND rate_individual_huf NOT GLOB '.*'
                AND rate_individual_huf NOT GLOB '*.'
            ),
            rate_others TEXT NOT NULL CHECK (
                length(rate_others) >= 1
                AND length(rate_others) <= 30
                AND rate_others NOT GLOB '*[^0-9.]*'
                AND rate_others NOT GLOB '*..*'
                AND rate_others NOT GLOB '.*'
                AND rate_others NOT GLOB '*.'
            ),
            rate_without_pan TEXT NOT NULL DEFAULT '20.00' CHECK (
                length(rate_without_pan) >= 1
                AND length(rate_without_pan) <= 30
                AND rate_without_pan NOT GLOB '*[^0-9.]*'
                AND rate_without_pan NOT GLOB '*..*'
                AND rate_without_pan NOT GLOB '.*'
                AND rate_without_pan NOT GLOB '*.'
            ),
            single_threshold_limit TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(single_threshold_limit) >= 1
                AND length(single_threshold_limit) <= 30
                AND single_threshold_limit NOT GLOB '*[^0-9.]*'
                AND single_threshold_limit NOT GLOB '*..*'
                AND single_threshold_limit NOT GLOB '.*'
                AND single_threshold_limit NOT GLOB '*.'
            ),
            aggregate_threshold_limit TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(aggregate_threshold_limit) >= 1
                AND length(aggregate_threshold_limit) <= 30
                AND aggregate_threshold_limit NOT GLOB '*[^0-9.]*'
                AND aggregate_threshold_limit NOT GLOB '*..*'
                AND aggregate_threshold_limit NOT GLOB '.*'
                AND aggregate_threshold_limit NOT GLOB '*.'
            ),
            tds_payable_ledger_id TEXT,
            is_tcs INTEGER NOT NULL DEFAULT 0 CHECK (is_tcs IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, tds_payable_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_tds_nature_of_payment UNIQUE (company_id, section_code)
        );

        CREATE TABLE IF NOT EXISTS voucher_tds_details (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            party_ledger_id TEXT NOT NULL,
            party_pan TEXT,
            section_code TEXT NOT NULL,
            deductee_type TEXT NOT NULL,
            gross_amount TEXT NOT NULL CHECK (
                length(gross_amount) >= 1
                AND length(gross_amount) <= 30
                AND gross_amount NOT GLOB '*[^0-9.]*'
                AND gross_amount NOT GLOB '*..*'
                AND gross_amount NOT GLOB '.*'
                AND gross_amount NOT GLOB '*.'
            ),
            tds_rate TEXT NOT NULL CHECK (
                length(tds_rate) >= 1
                AND length(tds_rate) <= 30
                AND tds_rate NOT GLOB '*[^0-9.]*'
                AND tds_rate NOT GLOB '*..*'
                AND tds_rate NOT GLOB '.*'
                AND tds_rate NOT GLOB '*.'
            ),
            tds_amount TEXT NOT NULL CHECK (
                length(tds_amount) >= 1
                AND length(tds_amount) <= 30
                AND tds_amount NOT GLOB '*[^0-9.]*'
                AND tds_amount NOT GLOB '*..*'
                AND tds_amount NOT GLOB '.*'
                AND tds_amount NOT GLOB '*.'
            ),
            net_payable_amount TEXT NOT NULL CHECK (
                length(net_payable_amount) >= 1
                AND length(net_payable_amount) <= 30
                AND net_payable_amount NOT GLOB '*[^0-9.]*'
                AND net_payable_amount NOT GLOB '*..*'
                AND net_payable_amount NOT GLOB '.*'
                AND net_payable_amount NOT GLOB '*.'
            ),
            is_pan_missing_higher_rate INTEGER NOT NULL DEFAULT 0 CHECK (is_pan_missing_higher_rate IN (0, 1)),
            challan_bsr_code TEXT CHECK (challan_bsr_code IS NULL OR length(challan_bsr_code) = 7),
            challan_date TEXT CHECK (challan_date IS NULL OR challan_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            challan_serial_no TEXT,
            payment_voucher_id TEXT,
            is_paid INTEGER NOT NULL DEFAULT 0 CHECK (is_paid IN (0, 1)),
            is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, payment_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_voucher_tds_reconcile ON voucher_tds_details(company_id, section_code, is_paid, is_cancelled);
        CREATE INDEX IF NOT EXISTS idx_voucher_tds_voucher ON voucher_tds_details(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_voucher_tds_party_threshold ON voucher_tds_details(company_id, party_ledger_id, section_code, is_cancelled);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.2.0-tds', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 10: GSTR-2B Purchase ITC Reconciliation, e-Invoice JSON & e-Way Bill JSON
/// Creates gstr2b_import_batches, gstr2b_portal_invoices, and voucher_einvoice_details,
/// and stamps schema_version to 'v1.3.0-gst-compliance'.
pub async fn run_milestone10_gst_compliance_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.3.0-gst-compliance" || current == "1.3.0-gst-compliance" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.2.0-tds" && current != "1.2.0-tds" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.2.0-tds' for GST compliance migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS gstr2b_import_batches (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            return_period TEXT NOT NULL CHECK (length(return_period) = 6 AND return_period GLOB '[0-1][0-9][0-9][0-9][0-9][0-9]'),
            gstin TEXT NOT NULL CHECK (length(gstin) = 15),
            imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            total_portal_invoices INTEGER NOT NULL DEFAULT 0,
            total_portal_itc TEXT NOT NULL CHECK (
                length(total_portal_itc) >= 1
                AND length(total_portal_itc) <= 30
                AND total_portal_itc NOT GLOB '*[^0-9.]*'
                AND total_portal_itc NOT GLOB '*..*'
                AND total_portal_itc NOT GLOB '.*'
                AND total_portal_itc NOT GLOB '*.'
            ),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_gstr2b_batch_period ON gstr2b_import_batches(company_id, return_period);

        CREATE TABLE IF NOT EXISTS gstr2b_portal_invoices (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            batch_id TEXT NOT NULL,
            supplier_gstin TEXT NOT NULL CHECK (length(supplier_gstin) = 15),
            supplier_name TEXT,
            invoice_number TEXT NOT NULL,
            invoice_date TEXT NOT NULL CHECK (invoice_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
            invoice_value TEXT NOT NULL CHECK (
                length(invoice_value) >= 1
                AND length(invoice_value) <= 30
                AND invoice_value NOT GLOB '*[^0-9.]*'
                AND invoice_value NOT GLOB '*..*'
                AND invoice_value NOT GLOB '.*'
                AND invoice_value NOT GLOB '*.'
            ),
            taxable_value TEXT NOT NULL CHECK (
                length(taxable_value) >= 1
                AND length(taxable_value) <= 30
                AND taxable_value NOT GLOB '*[^0-9.]*'
                AND taxable_value NOT GLOB '*..*'
                AND taxable_value NOT GLOB '.*'
                AND taxable_value NOT GLOB '*.'
            ),
            cgst_amount TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(cgst_amount) >= 1
                AND length(cgst_amount) <= 30
                AND cgst_amount NOT GLOB '*[^0-9.]*'
                AND cgst_amount NOT GLOB '*..*'
                AND cgst_amount NOT GLOB '.*'
                AND cgst_amount NOT GLOB '*.'
            ),
            sgst_amount TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(sgst_amount) >= 1
                AND length(sgst_amount) <= 30
                AND sgst_amount NOT GLOB '*[^0-9.]*'
                AND sgst_amount NOT GLOB '*..*'
                AND sgst_amount NOT GLOB '.*'
                AND sgst_amount NOT GLOB '*.'
            ),
            igst_amount TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(igst_amount) >= 1
                AND length(igst_amount) <= 30
                AND igst_amount NOT GLOB '*[^0-9.]*'
                AND igst_amount NOT GLOB '*..*'
                AND igst_amount NOT GLOB '.*'
                AND igst_amount NOT GLOB '*.'
            ),
            cess_amount TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(cess_amount) >= 1
                AND length(cess_amount) <= 30
                AND cess_amount NOT GLOB '*[^0-9.]*'
                AND cess_amount NOT GLOB '*..*'
                AND cess_amount NOT GLOB '.*'
                AND cess_amount NOT GLOB '*.'
            ),
            itc_availability TEXT NOT NULL DEFAULT 'Y' CHECK (itc_availability IN ('Y', 'N')),
            reconciliation_status TEXT NOT NULL CHECK (reconciliation_status IN ('Matched', 'Mismatch', 'MissingInBooks', 'MissingIn2B')),
            matched_voucher_id TEXT,
            tax_difference TEXT NOT NULL DEFAULT '0.00' CHECK (
                length(tax_difference) >= 1
                AND length(tax_difference) <= 30
                AND (
                    (tax_difference NOT GLOB '*[^0-9.]*' AND tax_difference NOT GLOB '*..*' AND tax_difference NOT GLOB '.*' AND tax_difference NOT GLOB '*.')
                    OR (tax_difference GLOB '-*' AND length(tax_difference) >= 2 AND substr(tax_difference, 2) NOT GLOB '*[^0-9.]*' AND substr(tax_difference, 2) NOT GLOB '*..*' AND substr(tax_difference, 2) NOT GLOB '.*' AND substr(tax_difference, 2) NOT GLOB '*.')
                )
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (batch_id) REFERENCES gstr2b_import_batches(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, matched_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_gstr2b_invoices_batch ON gstr2b_portal_invoices(company_id, batch_id);
        CREATE INDEX IF NOT EXISTS idx_gstr2b_invoices_match ON gstr2b_portal_invoices(company_id, supplier_gstin, invoice_number);
        CREATE INDEX IF NOT EXISTS idx_gstr2b_invoices_status ON gstr2b_portal_invoices(company_id, reconciliation_status);

        CREATE TABLE IF NOT EXISTS voucher_einvoice_details (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            irn TEXT CHECK (irn IS NULL OR length(irn) = 64),
            ack_no TEXT,
            ack_date TEXT,
            signed_qr_code TEXT,
            einvoice_status TEXT NOT NULL DEFAULT 'Pending' CHECK (einvoice_status IN ('Pending', 'Generated', 'Cancelled')),
            eway_bill_no TEXT CHECK (eway_bill_no IS NULL OR length(eway_bill_no) = 12),
            eway_bill_date TEXT,
            eway_valid_upto TEXT,
            transporter_id TEXT CHECK (transporter_id IS NULL OR length(transporter_id) = 15),
            transporter_name TEXT,
            transport_mode TEXT CHECK (transport_mode IS NULL OR transport_mode IN ('Road', 'Rail', 'Air', 'Ship')),
            vehicle_no TEXT,
            distance_km INTEGER CHECK (distance_km IS NULL OR distance_km >= 0),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            CONSTRAINT uq_voucher_einvoice_voucher UNIQUE (company_id, voucher_id)
        );

        CREATE INDEX IF NOT EXISTS idx_voucher_einvoice_status ON voucher_einvoice_details(company_id, einvoice_status);
        CREATE INDEX IF NOT EXISTS idx_voucher_einvoice_irn ON voucher_einvoice_details(company_id, irn);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.3.0-gst-compliance', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 11: Multi-Location Godowns, Stock Journals (Alt+F7), Batch/Expiry Tracking & Cost Centres.
/// Extends godowns with address and is_external, creates stock_item_batches,
/// extends inventory_cost_layers/stock_movements/stock_voucher_lines with batch_id,
/// creates cost_categories, cost_centres, and voucher_cost_allocations,
/// and stamps schema_version to 'v1.4.0-godowns-costing'.
pub async fn run_milestone11_multi_location_and_costing_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.4.0-godowns-costing" || current == "1.4.0-godowns-costing" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.3.0-gst-compliance" && current != "1.3.0-gst-compliance" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.3.0-gst-compliance' for Godowns & Costing migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    // Safe column additions via pragma inspection
    let godown_cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('godowns')")
            .fetch_all(&mut *tx)
            .await?;
    if !godown_cols.contains(&"address".to_string()) {
        sqlx::raw_sql("ALTER TABLE godowns ADD COLUMN address TEXT;")
            .execute(&mut *tx)
            .await?;
    }
    if !godown_cols.contains(&"is_external".to_string()) {
        sqlx::raw_sql("ALTER TABLE godowns ADD COLUMN is_external INTEGER NOT NULL DEFAULT 0;")
            .execute(&mut *tx)
            .await?;
    }

    let icl_cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('inventory_cost_layers')")
            .fetch_all(&mut *tx)
            .await?;
    if !icl_cols.contains(&"batch_id".to_string()) {
        sqlx::raw_sql("ALTER TABLE inventory_cost_layers ADD COLUMN batch_id TEXT;")
            .execute(&mut *tx)
            .await?;
    }

    let sm_cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('stock_movements')")
            .fetch_all(&mut *tx)
            .await?;
    if !sm_cols.contains(&"batch_id".to_string()) {
        sqlx::raw_sql("ALTER TABLE stock_movements ADD COLUMN batch_id TEXT;")
            .execute(&mut *tx)
            .await?;
    }

    let svl_cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('stock_voucher_lines')")
            .fetch_all(&mut *tx)
            .await?;
    if !svl_cols.contains(&"batch_id".to_string()) {
        sqlx::raw_sql("ALTER TABLE stock_voucher_lines ADD COLUMN batch_id TEXT;")
            .execute(&mut *tx)
            .await?;
    }

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS stock_item_batches (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            batch_number TEXT NOT NULL COLLATE NOCASE,
            mfg_date TEXT CHECK (mfg_date IS NULL OR (mfg_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(mfg_date) IS NOT NULL)),
            expiry_date TEXT CHECK (expiry_date IS NULL OR (expiry_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(expiry_date) IS NOT NULL)),
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_stock_item_batch UNIQUE (company_id, stock_item_id, batch_number)
        );

        CREATE INDEX IF NOT EXISTS idx_stock_batches_item ON stock_item_batches(company_id, stock_item_id);
        CREATE INDEX IF NOT EXISTS idx_stock_batches_expiry ON stock_item_batches(company_id, expiry_date);

        CREATE TABLE IF NOT EXISTS cost_categories (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            allocate_revenue INTEGER NOT NULL DEFAULT 1 CHECK (allocate_revenue IN (0, 1)),
            allocate_non_revenue INTEGER NOT NULL DEFAULT 0 CHECK (allocate_non_revenue IN (0, 1)),
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            CONSTRAINT uq_cost_category_name UNIQUE (company_id, name)
        );

        CREATE INDEX IF NOT EXISTS idx_cost_categories_company ON cost_categories(company_id);

        CREATE TABLE IF NOT EXISTS cost_centres (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            category_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            parent_id TEXT,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (category_id) REFERENCES cost_categories(id) ON DELETE RESTRICT,
            FOREIGN KEY (parent_id) REFERENCES cost_centres(id) ON DELETE RESTRICT,
            CONSTRAINT uq_cost_centre_name UNIQUE (company_id, category_id, name),
            CHECK (parent_id IS NULL OR parent_id <> id)
        );

        CREATE INDEX IF NOT EXISTS idx_cost_centres_cat ON cost_centres(company_id, category_id);
        CREATE INDEX IF NOT EXISTS idx_cost_centres_parent ON cost_centres(company_id, parent_id);

        CREATE TABLE IF NOT EXISTS voucher_cost_allocations (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            voucher_line_id TEXT NOT NULL,
            cost_centre_id TEXT NOT NULL,
            amount TEXT NOT NULL CHECK (
                length(amount) >= 1
                AND length(amount) <= 30
                AND amount NOT GLOB '*[^0-9.]*'
                AND amount NOT GLOB '*..*'
                AND amount NOT GLOB '.*'
                AND amount NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_line_id) REFERENCES voucher_lines(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (cost_centre_id) REFERENCES cost_centres(id) ON DELETE RESTRICT,
            CONSTRAINT uq_voucher_cost_alloc UNIQUE (company_id, voucher_line_id, cost_centre_id)
        );

        CREATE INDEX IF NOT EXISTS idx_vch_cost_alloc_vch ON voucher_cost_allocations(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_vch_cost_alloc_line ON voucher_cost_allocations(company_id, voucher_line_id);
        CREATE INDEX IF NOT EXISTS idx_vch_cost_alloc_centre ON voucher_cost_allocations(company_id, cost_centre_id);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.4.0-godowns-costing', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 12: Ratio Analysis, Budgets & Variance, Automated Financial Year-End Closing & MCA Audit Certification.
/// Extends audit_log with prev_hash and entry_hash, creates budgets, budget_lines, and financial_year_closings,
/// and stamps schema_version to 'v1.5.0-closing-budgets'.
pub async fn run_milestone12_closing_budgets_audit_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.5.0-closing-budgets" || current == "1.5.0-closing-budgets" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.4.0-godowns-costing" && current != "1.4.0-godowns-costing" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.4.0-godowns-costing' for Closing & Budgets migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    // Safe column additions to audit_log via pragma inspection
    let audit_cols: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('audit_log')")
            .fetch_all(&mut *tx)
            .await?;
    if !audit_cols.contains(&"prev_hash".to_string()) {
        sqlx::raw_sql("ALTER TABLE audit_log ADD COLUMN prev_hash TEXT;")
            .execute(&mut *tx)
            .await?;
    }
    if !audit_cols.contains(&"entry_hash".to_string()) {
        sqlx::raw_sql("ALTER TABLE audit_log ADD COLUMN entry_hash TEXT;")
            .execute(&mut *tx)
            .await?;
    }

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS budgets (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            from_date TEXT NOT NULL CHECK (from_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(from_date) IS NOT NULL),
            to_date TEXT NOT NULL CHECK (to_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(to_date) IS NOT NULL),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            CONSTRAINT uq_company_budget_name UNIQUE (company_id, name),
            CHECK (to_date >= from_date)
        );

        CREATE INDEX IF NOT EXISTS idx_budgets_company ON budgets(company_id);
        CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(company_id, from_date, to_date);

        CREATE TABLE IF NOT EXISTS budget_lines (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            budget_id TEXT NOT NULL,
            target_type TEXT NOT NULL CHECK (target_type IN ('Group', 'Ledger', 'CostCenter')),
            target_id TEXT NOT NULL,
            budget_type TEXT NOT NULL CHECK (budget_type IN ('OnNetTransactions', 'OnClosingBalance')),
            amount TEXT NOT NULL CHECK (
                length(amount) >= 1
                AND length(amount) <= 30
                AND amount NOT GLOB '*[^0-9.]*'
                AND amount NOT GLOB '*..*'
                AND amount NOT GLOB '.*'
                AND amount NOT GLOB '*.'
            ),
            dr_cr TEXT NOT NULL CHECK (dr_cr IN ('Dr', 'Cr')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (budget_id) REFERENCES budgets(id) ON DELETE CASCADE,
            CONSTRAINT uq_budget_target UNIQUE (company_id, budget_id, target_type, target_id)
        );

        CREATE INDEX IF NOT EXISTS idx_budget_lines_budget ON budget_lines(company_id, budget_id);
        CREATE INDEX IF NOT EXISTS idx_budget_lines_target ON budget_lines(company_id, target_type, target_id);

        CREATE TABLE IF NOT EXISTS financial_year_closings (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            financial_year_label TEXT NOT NULL,
            period_start TEXT NOT NULL CHECK (period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(period_start) IS NOT NULL),
            period_end TEXT NOT NULL CHECK (period_end GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(period_end) IS NOT NULL),
            next_fy_start TEXT NOT NULL CHECK (next_fy_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(next_fy_start) IS NOT NULL),
            closing_voucher_id TEXT NOT NULL,
            net_profit_or_loss TEXT NOT NULL CHECK (
                length(net_profit_or_loss) >= 1
                AND net_profit_or_loss NOT GLOB '*[^0-9.-]*'
                AND net_profit_or_loss NOT GLOB '*..*'
            ),
            is_locked INTEGER NOT NULL DEFAULT 1 CHECK (is_locked IN (0, 1)),
            closed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            closed_by TEXT NOT NULL DEFAULT 'system',
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, closing_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_company_fy_closing UNIQUE (company_id, period_end),
            CHECK (period_end >= period_start)
        );

        CREATE INDEX IF NOT EXISTS idx_fy_closings_company ON financial_year_closings(company_id, period_end);
        CREATE INDEX IF NOT EXISTS idx_fy_closings_lock ON financial_year_closings(company_id, is_locked);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.5.0-closing-budgets', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Executes Milestone 13 Order Processing migration:
/// - purchase_orders & purchase_order_lines
/// - sales_orders & sales_order_lines
/// - order_fulfillments
///
/// Stamps schema_version to 'v1.6.0-orders'.
pub async fn run_milestone13_order_processing_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.6.0-orders" || current == "1.6.0-orders" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.5.0-closing-budgets" && current != "1.5.0-closing-budgets" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.5.0-closing-budgets' for Order Processing migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS purchase_orders (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            order_number TEXT NOT NULL COLLATE NOCASE,
            order_date TEXT NOT NULL CHECK (order_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(order_date) IS NOT NULL),
            due_date TEXT CHECK (due_date IS NULL OR (due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(due_date) IS NOT NULL)),
            party_ledger_id TEXT NOT NULL,
            reference_no TEXT,
            narration TEXT,
            status TEXT NOT NULL DEFAULT 'Open' CHECK (status IN ('Open', 'PartiallyFulfilled', 'Fulfilled', 'Cancelled')),
            total_amount TEXT NOT NULL CHECK (
                length(total_amount) >= 1
                AND length(total_amount) <= 30
                AND total_amount NOT GLOB '*[^0-9.]*'
                AND total_amount NOT GLOB '*..*'
                AND total_amount NOT GLOB '.*'
                AND total_amount NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_company_po_number UNIQUE (company_id, order_number)
        );

        CREATE INDEX IF NOT EXISTS idx_po_company ON purchase_orders(company_id);
        CREATE INDEX IF NOT EXISTS idx_po_party ON purchase_orders(company_id, party_ledger_id);
        CREATE INDEX IF NOT EXISTS idx_po_status ON purchase_orders(company_id, status);
        CREATE INDEX IF NOT EXISTS idx_po_date ON purchase_orders(company_id, order_date);

        CREATE TABLE IF NOT EXISTS purchase_order_lines (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            order_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            godown_id TEXT,
            ordered_qty TEXT NOT NULL CHECK (
                length(ordered_qty) >= 1
                AND length(ordered_qty) <= 20
                AND ordered_qty NOT GLOB '*[^0-9.]*'
                AND ordered_qty NOT GLOB '*..*'
                AND ordered_qty NOT GLOB '.*'
                AND ordered_qty NOT GLOB '*.'
            ),
            fulfilled_qty TEXT NOT NULL DEFAULT '0' CHECK (
                length(fulfilled_qty) >= 1
                AND length(fulfilled_qty) <= 20
                AND fulfilled_qty NOT GLOB '*[^0-9.]*'
                AND fulfilled_qty NOT GLOB '*..*'
                AND fulfilled_qty NOT GLOB '.*'
                AND fulfilled_qty NOT GLOB '*.'
            ),
            unit_rate TEXT NOT NULL CHECK (
                length(unit_rate) >= 1
                AND length(unit_rate) <= 20
                AND unit_rate NOT GLOB '*[^0-9.]*'
                AND unit_rate NOT GLOB '*..*'
                AND unit_rate NOT GLOB '.*'
                AND unit_rate NOT GLOB '*.'
            ),
            discount_kind TEXT CHECK (discount_kind IS NULL OR discount_kind IN ('Percentage', 'FixedAmount')),
            discount_value TEXT CHECK (
                discount_value IS NULL
                OR (
                    length(discount_value) >= 1
                    AND length(discount_value) <= 20
                    AND discount_value NOT GLOB '*[^0-9.]*'
                    AND discount_value NOT GLOB '*..*'
                    AND discount_value NOT GLOB '.*'
                    AND discount_value NOT GLOB '*.'
                )
            ),
            line_amount TEXT NOT NULL CHECK (
                length(line_amount) >= 1
                AND length(line_amount) <= 30
                AND line_amount NOT GLOB '*[^0-9.]*'
                AND line_amount NOT GLOB '*..*'
                AND line_amount NOT GLOB '.*'
                AND line_amount NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (order_id) REFERENCES purchase_orders(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_po_lines_order ON purchase_order_lines(company_id, order_id);
        CREATE INDEX IF NOT EXISTS idx_po_lines_item ON purchase_order_lines(company_id, stock_item_id);

        CREATE TABLE IF NOT EXISTS sales_orders (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            order_number TEXT NOT NULL COLLATE NOCASE,
            order_date TEXT NOT NULL CHECK (order_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(order_date) IS NOT NULL),
            due_date TEXT CHECK (due_date IS NULL OR (due_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(due_date) IS NOT NULL)),
            party_ledger_id TEXT NOT NULL,
            reference_no TEXT,
            narration TEXT,
            status TEXT NOT NULL DEFAULT 'Open' CHECK (status IN ('Open', 'PartiallyFulfilled', 'Fulfilled', 'Cancelled')),
            total_amount TEXT NOT NULL CHECK (
                length(total_amount) >= 1
                AND length(total_amount) <= 30
                AND total_amount NOT GLOB '*[^0-9.]*'
                AND total_amount NOT GLOB '*..*'
                AND total_amount NOT GLOB '.*'
                AND total_amount NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, party_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_company_so_number UNIQUE (company_id, order_number)
        );

        CREATE INDEX IF NOT EXISTS idx_so_company ON sales_orders(company_id);
        CREATE INDEX IF NOT EXISTS idx_so_party ON sales_orders(company_id, party_ledger_id);
        CREATE INDEX IF NOT EXISTS idx_so_status ON sales_orders(company_id, status);
        CREATE INDEX IF NOT EXISTS idx_so_date ON sales_orders(company_id, order_date);

        CREATE TABLE IF NOT EXISTS sales_order_lines (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            order_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            godown_id TEXT,
            ordered_qty TEXT NOT NULL CHECK (
                length(ordered_qty) >= 1
                AND length(ordered_qty) <= 20
                AND ordered_qty NOT GLOB '*[^0-9.]*'
                AND ordered_qty NOT GLOB '*..*'
                AND ordered_qty NOT GLOB '.*'
                AND ordered_qty NOT GLOB '*.'
            ),
            fulfilled_qty TEXT NOT NULL DEFAULT '0' CHECK (
                length(fulfilled_qty) >= 1
                AND length(fulfilled_qty) <= 20
                AND fulfilled_qty NOT GLOB '*[^0-9.]*'
                AND fulfilled_qty NOT GLOB '*..*'
                AND fulfilled_qty NOT GLOB '.*'
                AND fulfilled_qty NOT GLOB '*.'
            ),
            unit_rate TEXT NOT NULL CHECK (
                length(unit_rate) >= 1
                AND length(unit_rate) <= 20
                AND unit_rate NOT GLOB '*[^0-9.]*'
                AND unit_rate NOT GLOB '*..*'
                AND unit_rate NOT GLOB '.*'
                AND unit_rate NOT GLOB '*.'
            ),
            discount_kind TEXT CHECK (discount_kind IS NULL OR discount_kind IN ('Percentage', 'FixedAmount')),
            discount_value TEXT CHECK (
                discount_value IS NULL
                OR (
                    length(discount_value) >= 1
                    AND length(discount_value) <= 20
                    AND discount_value NOT GLOB '*[^0-9.]*'
                    AND discount_value NOT GLOB '*..*'
                    AND discount_value NOT GLOB '.*'
                    AND discount_value NOT GLOB '*.'
                )
            ),
            line_amount TEXT NOT NULL CHECK (
                length(line_amount) >= 1
                AND length(line_amount) <= 30
                AND line_amount NOT GLOB '*[^0-9.]*'
                AND line_amount NOT GLOB '*..*'
                AND line_amount NOT GLOB '.*'
                AND line_amount NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (order_id) REFERENCES sales_orders(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_so_lines_order ON sales_order_lines(company_id, order_id);
        CREATE INDEX IF NOT EXISTS idx_so_lines_item ON sales_order_lines(company_id, stock_item_id);

        CREATE TABLE IF NOT EXISTS order_fulfillments (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            order_type TEXT NOT NULL CHECK (order_type IN ('PurchaseOrder', 'SalesOrder')),
            order_id TEXT NOT NULL,
            order_line_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            commercial_doc_id TEXT,
            fulfilled_qty TEXT NOT NULL CHECK (
                length(fulfilled_qty) >= 1
                AND length(fulfilled_qty) <= 20
                AND fulfilled_qty NOT GLOB '*[^0-9.]*'
                AND fulfilled_qty NOT GLOB '*..*'
                AND fulfilled_qty NOT GLOB '.*'
                AND fulfilled_qty NOT GLOB '*.'
            ),
            fulfillment_date TEXT NOT NULL CHECK (fulfillment_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(fulfillment_date) IS NOT NULL),
            is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_fulfillments_order ON order_fulfillments(company_id, order_id);
        CREATE INDEX IF NOT EXISTS idx_fulfillments_voucher ON order_fulfillments(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_fulfillments_line ON order_fulfillments(company_id, order_line_id);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.6.0-orders', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Applies Milestone 14 migration: Company Security (Users & RBAC) and Manufacturing Bill of Materials (BOM).
///
/// Stamps schema_version to 'v1.7.0-security-bom'.
pub async fn run_milestone14_security_and_bom_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.7.0-security-bom" || current == "1.7.0-security-bom" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.6.0-orders" && current != "1.6.0-orders" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.6.0-orders' for Security & BOM migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS company_security_config (
            company_id TEXT PRIMARY KEY NOT NULL,
            security_enabled INTEGER NOT NULL DEFAULT 0 CHECK (security_enabled IN (0, 1)),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS company_users (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            username TEXT NOT NULL COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('Admin', 'Accountant', 'DataEntry', 'Auditor')),
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_login_at TEXT,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            CONSTRAINT uq_company_username UNIQUE (company_id, username)
        );

        CREATE INDEX IF NOT EXISTS idx_company_users_comp ON company_users(company_id);
        CREATE INDEX IF NOT EXISTS idx_company_users_role ON company_users(company_id, role);

        CREATE TABLE IF NOT EXISTS bill_of_materials (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            finished_stock_item_id TEXT NOT NULL,
            bom_name TEXT NOT NULL COLLATE NOCASE,
            base_output_qty TEXT NOT NULL CHECK (
                length(base_output_qty) >= 1
                AND length(base_output_qty) <= 20
                AND base_output_qty NOT GLOB '*[^0-9.]*'
                AND base_output_qty NOT GLOB '*..*'
                AND base_output_qty NOT GLOB '.*'
                AND base_output_qty NOT GLOB '*.'
            ),
            default_godown_id TEXT,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, finished_stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_company_item_bom UNIQUE (company_id, finished_stock_item_id, bom_name)
        );

        CREATE INDEX IF NOT EXISTS idx_bom_company ON bill_of_materials(company_id);
        CREATE INDEX IF NOT EXISTS idx_bom_item ON bill_of_materials(company_id, finished_stock_item_id);

        CREATE TABLE IF NOT EXISTS bom_components (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            bom_id TEXT NOT NULL,
            raw_stock_item_id TEXT NOT NULL,
            source_godown_id TEXT,
            required_qty TEXT NOT NULL CHECK (
                length(required_qty) >= 1
                AND length(required_qty) <= 20
                AND required_qty NOT GLOB '*[^0-9.]*'
                AND required_qty NOT GLOB '*..*'
                AND required_qty NOT GLOB '.*'
                AND required_qty NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (bom_id) REFERENCES bill_of_materials(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, raw_stock_item_id) REFERENCES stock_items(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_bom_raw_item UNIQUE (bom_id, raw_stock_item_id)
        );

        CREATE INDEX IF NOT EXISTS idx_bom_comp_bom ON bom_components(company_id, bom_id);
        CREATE INDEX IF NOT EXISTS idx_bom_comp_item ON bom_components(company_id, raw_stock_item_id);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.7.0-security-bom', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 15: Indian Payroll Engine, Statutory PF/ESI/PT & Monthly Payroll GL Posting.
/// Creates employee_groups, employees, payroll_ledger_mappings, payroll_runs, and payroll_payslips
/// and stamps schema_version to 'v1.8.0-payroll'.
pub async fn run_milestone15_payroll_migration(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;
    if current == "v1.8.0-payroll" || current == "1.8.0-payroll" {
        tx.commit().await?;
        return Ok(());
    }
    if current != "v1.7.0-security-bom" && current != "1.7.0-security-bom" {
        return Err(OpenTallyError::Migration(format!(
            "Expected schema version 'v1.7.0-security-bom' for Payroll migration, found '{}'",
            current
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS employee_groups (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL COLLATE NOCASE,
            parent_id TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, parent_id) REFERENCES employee_groups(company_id, id) ON DELETE RESTRICT,
            UNIQUE (company_id, id),
            CONSTRAINT uq_company_emp_grp_name UNIQUE (company_id, name)
        );

        CREATE INDEX IF NOT EXISTS idx_emp_groups_company ON employee_groups(company_id);

        CREATE TABLE IF NOT EXISTS employees (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            employee_code TEXT NOT NULL COLLATE NOCASE,
            name TEXT NOT NULL,
            department_group_id TEXT,
            designation TEXT,
            date_of_joining TEXT NOT NULL CHECK (date_of_joining GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(date_of_joining) IS NOT NULL),
            date_of_exit TEXT CHECK (date_of_exit IS NULL OR (date_of_exit GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(date_of_exit) IS NOT NULL)),
            pan TEXT CHECK (pan IS NULL OR length(pan) = 10),
            uan TEXT CHECK (uan IS NULL OR (length(uan) = 12 AND uan NOT GLOB '*[^0-9]*')),
            esic_number TEXT,
            bank_account_no TEXT,
            bank_ifsc TEXT,
            basic_salary TEXT NOT NULL CHECK (
                length(basic_salary) >= 1 AND length(basic_salary) <= 30
                AND basic_salary NOT GLOB '*[^0-9.]*' AND basic_salary NOT GLOB '*..*'
                AND basic_salary NOT GLOB '.*' AND basic_salary NOT GLOB '*.'
            ),
            hra TEXT NOT NULL DEFAULT '0' CHECK (
                length(hra) >= 1 AND length(hra) <= 30
                AND hra NOT GLOB '*[^0-9.]*' AND hra NOT GLOB '*..*'
                AND hra NOT GLOB '.*' AND hra NOT GLOB '*.'
            ),
            other_allowances TEXT NOT NULL DEFAULT '0' CHECK (
                length(other_allowances) >= 1 AND length(other_allowances) <= 30
                AND other_allowances NOT GLOB '*[^0-9.]*' AND other_allowances NOT GLOB '*..*'
                AND other_allowances NOT GLOB '.*' AND other_allowances NOT GLOB '*.'
            ),
            pf_applicable INTEGER NOT NULL DEFAULT 1 CHECK (pf_applicable IN (0, 1)),
            esi_applicable INTEGER NOT NULL DEFAULT 0 CHECK (esi_applicable IN (0, 1)),
            pt_applicable INTEGER NOT NULL DEFAULT 1 CHECK (pt_applicable IN (0, 1)),
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, department_group_id) REFERENCES employee_groups(company_id, id) ON DELETE SET NULL,
            UNIQUE (company_id, id),
            CONSTRAINT uq_company_emp_code UNIQUE (company_id, employee_code)
        );

        CREATE INDEX IF NOT EXISTS idx_employees_company ON employees(company_id);
        CREATE INDEX IF NOT EXISTS idx_employees_active ON employees(company_id, is_active);

        CREATE TABLE IF NOT EXISTS payroll_ledger_mappings (
            company_id TEXT PRIMARY KEY NOT NULL,
            salary_expense_ledger_id TEXT NOT NULL,
            employer_pf_expense_ledger_id TEXT NOT NULL,
            employer_esi_expense_ledger_id TEXT NOT NULL,
            salary_payable_ledger_id TEXT NOT NULL,
            pf_payable_ledger_id TEXT NOT NULL,
            esi_payable_ledger_id TEXT NOT NULL,
            pt_payable_ledger_id TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, salary_expense_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, employer_pf_expense_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, employer_esi_expense_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, salary_payable_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, pf_payable_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, esi_payable_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, pt_payable_ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT
        );

        CREATE TABLE IF NOT EXISTS payroll_runs (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            pay_period TEXT NOT NULL CHECK (pay_period GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]'),
            run_date TEXT NOT NULL CHECK (run_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(run_date) IS NOT NULL),
            voucher_id TEXT NOT NULL,
            total_gross TEXT NOT NULL CHECK (
                length(total_gross) >= 1 AND length(total_gross) <= 30
                AND total_gross NOT GLOB '*[^0-9.]*' AND total_gross NOT GLOB '*..*'
                AND total_gross NOT GLOB '.*' AND total_gross NOT GLOB '*.'
            ),
            total_deductions TEXT NOT NULL CHECK (
                length(total_deductions) >= 1 AND length(total_deductions) <= 30
                AND total_deductions NOT GLOB '*[^0-9.]*' AND total_deductions NOT GLOB '*..*'
                AND total_deductions NOT GLOB '.*' AND total_deductions NOT GLOB '*.'
            ),
            total_employer_contributions TEXT NOT NULL CHECK (
                length(total_employer_contributions) >= 1 AND length(total_employer_contributions) <= 30
                AND total_employer_contributions NOT GLOB '*[^0-9.]*' AND total_employer_contributions NOT GLOB '*..*'
                AND total_employer_contributions NOT GLOB '.*' AND total_employer_contributions NOT GLOB '*.'
            ),
            total_net_pay TEXT NOT NULL CHECK (
                length(total_net_pay) >= 1 AND length(total_net_pay) <= 30
                AND total_net_pay NOT GLOB '*[^0-9.]*' AND total_net_pay NOT GLOB '*..*'
                AND total_net_pay NOT GLOB '.*' AND total_net_pay NOT GLOB '*.'
            ),
            status TEXT NOT NULL DEFAULT 'Posted' CHECK (status IN ('Posted', 'Cancelled')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
            UNIQUE (company_id, id)
        );

        CREATE INDEX IF NOT EXISTS idx_payroll_runs_company ON payroll_runs(company_id);
        CREATE INDEX IF NOT EXISTS idx_payroll_runs_voucher ON payroll_runs(company_id, voucher_id);
        CREATE UNIQUE INDEX IF NOT EXISTS uq_active_payroll_run ON payroll_runs(company_id, pay_period) WHERE status = 'Posted';

        CREATE TABLE IF NOT EXISTS payroll_payslips (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            payroll_run_id TEXT NOT NULL,
            employee_id TEXT NOT NULL,
            total_working_days TEXT NOT NULL CHECK (
                length(total_working_days) >= 1 AND length(total_working_days) <= 20
                AND total_working_days NOT GLOB '*[^0-9.]*' AND total_working_days NOT GLOB '*..*'
                AND total_working_days NOT GLOB '.*' AND total_working_days NOT GLOB '*.'
            ),
            days_present TEXT NOT NULL CHECK (
                length(days_present) >= 1 AND length(days_present) <= 20
                AND days_present NOT GLOB '*[^0-9.]*' AND days_present NOT GLOB '*..*'
                AND days_present NOT GLOB '.*' AND days_present NOT GLOB '*.'
            ),
            lop_days TEXT NOT NULL CHECK (
                length(lop_days) >= 1 AND length(lop_days) <= 20
                AND lop_days NOT GLOB '*[^0-9.]*' AND lop_days NOT GLOB '*..*'
                AND lop_days NOT GLOB '.*' AND lop_days NOT GLOB '*.'
            ),
            prorated_basic TEXT NOT NULL CHECK (
                length(prorated_basic) >= 1 AND length(prorated_basic) <= 30
                AND prorated_basic NOT GLOB '*[^0-9.]*' AND prorated_basic NOT GLOB '*..*'
                AND prorated_basic NOT GLOB '.*' AND prorated_basic NOT GLOB '*.'
            ),
            prorated_hra TEXT NOT NULL CHECK (
                length(prorated_hra) >= 1 AND length(prorated_hra) <= 30
                AND prorated_hra NOT GLOB '*[^0-9.]*' AND prorated_hra NOT GLOB '*..*'
                AND prorated_hra NOT GLOB '.*' AND prorated_hra NOT GLOB '*.'
            ),
            prorated_allowances TEXT NOT NULL CHECK (
                length(prorated_allowances) >= 1 AND length(prorated_allowances) <= 30
                AND prorated_allowances NOT GLOB '*[^0-9.]*' AND prorated_allowances NOT GLOB '*..*'
                AND prorated_allowances NOT GLOB '.*' AND prorated_allowances NOT GLOB '*.'
            ),
            gross_earnings TEXT NOT NULL CHECK (
                length(gross_earnings) >= 1 AND length(gross_earnings) <= 30
                AND gross_earnings NOT GLOB '*[^0-9.]*' AND gross_earnings NOT GLOB '*..*'
                AND gross_earnings NOT GLOB '.*' AND gross_earnings NOT GLOB '*.'
            ),
            employee_pf TEXT NOT NULL CHECK (
                length(employee_pf) >= 1 AND length(employee_pf) <= 30
                AND employee_pf NOT GLOB '*[^0-9.]*' AND employee_pf NOT GLOB '*..*'
                AND employee_pf NOT GLOB '.*' AND employee_pf NOT GLOB '*.'
            ),
            employer_epf TEXT NOT NULL CHECK (
                length(employer_epf) >= 1 AND length(employer_epf) <= 30
                AND employer_epf NOT GLOB '*[^0-9.]*' AND employer_epf NOT GLOB '*..*'
                AND employer_epf NOT GLOB '.*' AND employer_epf NOT GLOB '*.'
            ),
            employer_eps TEXT NOT NULL CHECK (
                length(employer_eps) >= 1 AND length(employer_eps) <= 30
                AND employer_eps NOT GLOB '*[^0-9.]*' AND employer_eps NOT GLOB '*..*'
                AND employer_eps NOT GLOB '.*' AND employer_eps NOT GLOB '*.'
            ),
            employee_esi TEXT NOT NULL CHECK (
                length(employee_esi) >= 1 AND length(employee_esi) <= 30
                AND employee_esi NOT GLOB '*[^0-9.]*' AND employee_esi NOT GLOB '*..*'
                AND employee_esi NOT GLOB '.*' AND employee_esi NOT GLOB '*.'
            ),
            employer_esi TEXT NOT NULL CHECK (
                length(employer_esi) >= 1 AND length(employer_esi) <= 30
                AND employer_esi NOT GLOB '*[^0-9.]*' AND employer_esi NOT GLOB '*..*'
                AND employer_esi NOT GLOB '.*' AND employer_esi NOT GLOB '*.'
            ),
            professional_tax TEXT NOT NULL CHECK (
                length(professional_tax) >= 1 AND length(professional_tax) <= 30
                AND professional_tax NOT GLOB '*[^0-9.]*' AND professional_tax NOT GLOB '*..*'
                AND professional_tax NOT GLOB '.*' AND professional_tax NOT GLOB '*.'
            ),
            other_deductions TEXT NOT NULL DEFAULT '0' CHECK (
                length(other_deductions) >= 1 AND length(other_deductions) <= 30
                AND other_deductions NOT GLOB '*[^0-9.]*' AND other_deductions NOT GLOB '*..*'
                AND other_deductions NOT GLOB '.*' AND other_deductions NOT GLOB '*.'
            ),
            total_deductions TEXT NOT NULL CHECK (
                length(total_deductions) >= 1 AND length(total_deductions) <= 30
                AND total_deductions NOT GLOB '*[^0-9.]*' AND total_deductions NOT GLOB '*..*'
                AND total_deductions NOT GLOB '.*' AND total_deductions NOT GLOB '*.'
            ),
            net_pay TEXT NOT NULL CHECK (
                length(net_pay) >= 1 AND length(net_pay) <= 30
                AND net_pay NOT GLOB '*[^0-9.]*' AND net_pay NOT GLOB '*..*'
                AND net_pay NOT GLOB '.*' AND net_pay NOT GLOB '*.'
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, payroll_run_id) REFERENCES payroll_runs(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, employee_id) REFERENCES employees(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT uq_run_employee UNIQUE (payroll_run_id, employee_id)
        );

        CREATE INDEX IF NOT EXISTS idx_payslips_run ON payroll_payslips(company_id, payroll_run_id);
        CREATE INDEX IF NOT EXISTS idx_payslips_emp ON payroll_payslips(company_id, employee_id);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v1.8.0-payroll', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Milestone 16: Multi-Currency Accounting, Realized/Unrealized Forex Gain & Loss & Bank Cheque Printing PDFs.
///
/// Introduces:
/// - `currencies`: ISO 4217 currency master with unique code per company and single base currency guarantee.
/// - `exchange_rates`: Canonical decimal standard, selling, and buying rates by effective date.
/// - `voucher_currency_details`: Foreign currency amounts, exchange rates, and base amounts tied to vouchers/lines.
/// - `cheque_templates`: Millimeter-based CTS-2010 cheque printing templates for Indian banks.
///
/// Stamps schema version `v2.1.0-multicurrency-cheque` in `_opentally_metadata`.
pub async fn run_milestone16_multicurrency_cheque_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }

    // Precondition check
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;

    if current_version == "v2.1.0-multicurrency-cheque" {
        tx.rollback().await?;
        return Ok(());
    }

    if current_version != "v1.8.0-payroll" {
        return Err(OpenTallyError::Migration(format!(
            "Milestone 16 migration requires schema version 'v1.8.0-payroll', but found '{}'",
            current_version
        )));
    }

    sqlx::raw_sql("PRAGMA foreign_keys = OFF;")
        .execute(&mut *tx)
        .await?;

    sqlx::raw_sql(
        r#"
        -- 1. Currencies Master
        CREATE TABLE IF NOT EXISTS currencies (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            code TEXT NOT NULL,
            symbol TEXT NOT NULL,
            formal_name TEXT NOT NULL,
            decimal_places INTEGER NOT NULL DEFAULT 2 CHECK (decimal_places BETWEEN 0 AND 4),
            is_base INTEGER NOT NULL DEFAULT 0 CHECK (is_base IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (id),
            UNIQUE (company_id, id),
            UNIQUE (company_id, code),
            CHECK (length(code) = 3 AND code = upper(code) AND code GLOB '[A-Z][A-Z][A-Z]'),
            CHECK (length(trim(symbol)) > 0 AND length(symbol) <= 12),
            CHECK (length(trim(formal_name)) > 0 AND length(formal_name) <= 100),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_currencies_single_base_per_company
            ON currencies(company_id) WHERE is_base = 1;

        -- 2. Daily Exchange Rates
        CREATE TABLE IF NOT EXISTS exchange_rates (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            currency_code TEXT NOT NULL,
            effective_date TEXT NOT NULL CHECK (
                length(effective_date) = 10
                AND effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            ),
            standard_rate TEXT NOT NULL CHECK (
                length(standard_rate) BETWEEN 1 AND 25
                AND standard_rate NOT LIKE '%-%'
                AND standard_rate NOT LIKE '%+%'
                AND standard_rate NOT LIKE '.%'
                AND standard_rate NOT LIKE '%.'
                AND standard_rate NOT GLOB '*[^0-9.]*'
                AND standard_rate NOT GLOB '*.*.*'
                AND CAST(standard_rate AS REAL) > 0
                AND (instr(standard_rate, '.') = 0 OR length(substr(standard_rate, instr(standard_rate, '.') + 1)) BETWEEN 1 AND 6)
            ),
            selling_rate TEXT NOT NULL CHECK (
                length(selling_rate) BETWEEN 1 AND 25
                AND selling_rate NOT LIKE '%-%'
                AND selling_rate NOT LIKE '%+%'
                AND selling_rate NOT LIKE '.%'
                AND selling_rate NOT LIKE '%.'
                AND selling_rate NOT GLOB '*[^0-9.]*'
                AND selling_rate NOT GLOB '*.*.*'
                AND CAST(selling_rate AS REAL) > 0
                AND (instr(selling_rate, '.') = 0 OR length(substr(selling_rate, instr(selling_rate, '.') + 1)) BETWEEN 1 AND 6)
            ),
            buying_rate TEXT NOT NULL CHECK (
                length(buying_rate) BETWEEN 1 AND 25
                AND buying_rate NOT LIKE '%-%'
                AND buying_rate NOT LIKE '%+%'
                AND buying_rate NOT LIKE '.%'
                AND buying_rate NOT LIKE '%.'
                AND buying_rate NOT GLOB '*[^0-9.]*'
                AND buying_rate NOT GLOB '*.*.*'
                AND CAST(buying_rate AS REAL) > 0
                AND (instr(buying_rate, '.') = 0 OR length(substr(buying_rate, instr(buying_rate, '.') + 1)) BETWEEN 1 AND 6)
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (id),
            UNIQUE (company_id, id),
            UNIQUE (company_id, currency_code, effective_date),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, currency_code) REFERENCES currencies(company_id, code) ON DELETE RESTRICT
        );

        CREATE INDEX IF NOT EXISTS idx_exchange_rates_lookup
            ON exchange_rates(company_id, currency_code, effective_date DESC);

        -- 3. Voucher Currency Details
        CREATE TABLE IF NOT EXISTS voucher_currency_details (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            voucher_line_id TEXT,
            currency_code TEXT NOT NULL,
            foreign_amount TEXT NOT NULL CHECK (
                length(foreign_amount) BETWEEN 1 AND 25
                AND foreign_amount NOT LIKE '%-%'
                AND foreign_amount NOT LIKE '%+%'
                AND foreign_amount NOT LIKE '.%'
                AND foreign_amount NOT LIKE '%.'
                AND foreign_amount NOT GLOB '*[^0-9.]*'
                AND foreign_amount NOT GLOB '*.*.*'
                AND CAST(foreign_amount AS REAL) > 0
                AND (instr(foreign_amount, '.') = 0 OR length(substr(foreign_amount, instr(foreign_amount, '.') + 1)) BETWEEN 1 AND 4)
            ),
            exchange_rate TEXT NOT NULL CHECK (
                length(exchange_rate) BETWEEN 1 AND 25
                AND exchange_rate NOT LIKE '%-%'
                AND exchange_rate NOT LIKE '%+%'
                AND exchange_rate NOT LIKE '.%'
                AND exchange_rate NOT LIKE '%.'
                AND exchange_rate NOT GLOB '*[^0-9.]*'
                AND exchange_rate NOT GLOB '*.*.*'
                AND CAST(exchange_rate AS REAL) > 0
                AND (instr(exchange_rate, '.') = 0 OR length(substr(exchange_rate, instr(exchange_rate, '.') + 1)) BETWEEN 1 AND 6)
            ),
            base_amount TEXT NOT NULL CHECK (
                length(base_amount) BETWEEN 1 AND 25
                AND base_amount NOT LIKE '%-%'
                AND base_amount NOT LIKE '%+%'
                AND base_amount NOT LIKE '.%'
                AND base_amount NOT LIKE '%.'
                AND base_amount NOT GLOB '*[^0-9.]*'
                AND base_amount NOT GLOB '*.*.*'
                AND CAST(base_amount AS REAL) > 0
                AND (instr(base_amount, '.') = 0 OR length(substr(base_amount, instr(base_amount, '.') + 1)) BETWEEN 1 AND 4)
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (id),
            UNIQUE (company_id, id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, voucher_line_id) REFERENCES voucher_lines(company_id, id) ON DELETE CASCADE,
            FOREIGN KEY (company_id, currency_code) REFERENCES currencies(company_id, code) ON DELETE RESTRICT
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_vcd_unique_line
            ON voucher_currency_details(company_id, voucher_line_id)
            WHERE voucher_line_id IS NOT NULL;

        CREATE INDEX IF NOT EXISTS idx_vcd_company_voucher
            ON voucher_currency_details(company_id, voucher_id);

        -- 4. CTS-2010 Cheque Printing Templates
        CREATE TABLE IF NOT EXISTS cheque_templates (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            bank_name TEXT NOT NULL CHECK (length(trim(bank_name)) > 0 AND length(bank_name) <= 120),
            width_mm REAL NOT NULL CHECK (width_mm > 50.0 AND width_mm <= 400.0),
            height_mm REAL NOT NULL CHECK (height_mm > 30.0 AND height_mm <= 200.0),
            date_x_mm REAL NOT NULL CHECK (date_x_mm >= 0.0 AND date_x_mm <= width_mm),
            date_y_mm REAL NOT NULL CHECK (date_y_mm >= 0.0 AND date_y_mm <= height_mm),
            payee_x_mm REAL NOT NULL CHECK (payee_x_mm >= 0.0 AND payee_x_mm <= width_mm),
            payee_y_mm REAL NOT NULL CHECK (payee_y_mm >= 0.0 AND payee_y_mm <= height_mm),
            words_x_mm REAL NOT NULL CHECK (words_x_mm >= 0.0 AND words_x_mm <= width_mm),
            words_y_mm REAL NOT NULL CHECK (words_y_mm >= 0.0 AND words_y_mm <= height_mm),
            amount_x_mm REAL NOT NULL CHECK (amount_x_mm >= 0.0 AND amount_x_mm <= width_mm),
            amount_y_mm REAL NOT NULL CHECK (amount_y_mm >= 0.0 AND amount_y_mm <= height_mm),
            is_account_payee_default INTEGER NOT NULL DEFAULT 1 CHECK (is_account_payee_default IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (id),
            UNIQUE (company_id, id),
            UNIQUE (company_id, bank_name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v2.1.0-multicurrency-cheque', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

fn schema_version_rank(ver: &str) -> usize {
    match ver.trim_start_matches('v') {
        "0.1.0-sprint0" => 0,
        "0.2.0-sprint1" => 1,
        "0.2.1-sprint1" => 2,
        "0.3.0-sprint2" => 3,
        "0.3.1-sprint2" => 4,
        "0.4.0-sprint4" => 5,
        "0.4.1-sprint4" => 6,
        "0.4.2-sprint4" => 7,
        "0.4.3-sprint5" => 8,
        "0.4.4-sprint5" => 9,
        "0.4.5-sprint6" => 10,
        "0.4.6-sprint6" => 11,
        "0.4.7-commercial" => 12,
        "0.5.0-billwise" => 13,
        "0.6.0-gst" => 14,
        "0.7.0-notes" => 15,
        "1.1.0-banking" => 16,
        "1.2.0-tds" => 17,
        "1.3.0-gst-compliance" => 18,
        "1.4.0-godowns-costing" => 19,
        "1.5.0-closing-budgets" => 20,
        "1.6.0-orders" => 21,
        "1.7.0-security-bom" => 22,
        "1.8.0-payroll" => 23,
        "2.1.0-multicurrency-cheque" => 24,
        "3.0.0-enterprise" => 25,
        "3.1.0-gstn-strict" => 26,
        "3.2.0-inventory-depth" => 27,
        "3.4.0-pan-india-statutory" => 28,
        "3.5.0-scale-indexes" => 29,
        "4.1.0-factory-extensibility" => 30,
        _ => 0,
    }
}

/// Idempotently runs all pending migrations from the current schema_version up to `v1.8.0-payroll`.
pub async fn run_all_migrations(pool: &Pool<Sqlite>) -> Result<(), OpenTallyError> {
    let current: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await?;
    let rank = schema_version_rank(&current);

    if rank < 1 {
        run_sprint1_migration(pool).await?;
    }
    if rank < 2 {
        run_stage3_migration(pool).await?;
    }
    if rank < 3 {
        run_sprint2_migration(pool).await?;
    }
    if rank < 4 {
        run_sprint2_stage4_hardening_migration(pool).await?;
    }
    if rank < 5 {
        run_sprint4_inventory_migration(pool).await?;
    }
    if rank < 6 {
        run_sprint4_stage11_stock_migration(pool).await?;
    }
    if rank < 7 {
        run_sprint4_stage12_valuation_migration(pool).await?;
    }
    if rank < 8 {
        run_sprint5_stage14_integration_migration(pool).await?;
    }
    if rank < 9 {
        run_sprint5_stage15_valuation_attribution_migration(pool).await?;
    }
    if rank < 10 {
        run_sprint6_stage17_party_migration(pool).await?;
    }
    if rank < 11 {
        run_sprint6_stage18_sales_migration(pool).await?;
    }
    if rank < 12 {
        run_milestone1_commercial_posting_migration(pool).await?;
    }
    if rank < 13 {
        run_milestone2_billwise_migration(pool).await?;
    }
    if rank < 14 {
        run_milestone3_gst_migration(pool).await?;
    }
    if rank < 15 {
        run_milestone5_notes_migration(pool).await?;
    }
    if rank < 16 {
        run_milestone8_banking_migration(pool).await?;
    }
    if rank < 17 {
        run_milestone9_tds_migration(pool).await?;
    }
    if rank < 18 {
        run_milestone10_gst_compliance_migration(pool).await?;
    }
    if rank < 19 {
        run_milestone11_multi_location_and_costing_migration(pool).await?;
    }
    if rank < 20 {
        run_milestone12_closing_budgets_audit_migration(pool).await?;
    }
    if rank < 21 {
        run_milestone13_order_processing_migration(pool).await?;
    }
    if rank < 22 {
        run_milestone14_security_and_bom_migration(pool).await?;
    }
    if rank < 23 {
        run_milestone15_payroll_migration(pool).await?;
    }
    if rank < 24 {
        run_milestone16_multicurrency_cheque_migration(pool).await?;
    }
    if rank < 25 {
        run_milestone18_gsp_extensions_migration(pool).await?;
    }
    if rank < 26 {
        run_milestone19_gstn_strictness_migration(pool).await?;
    }
    if rank < 27 {
        run_milestone20_inventory_billing_depth_migration(pool).await?;
    }
    if rank < 28 {
        run_milestone22_pan_india_payroll_tds_migration(pool).await?;
    }
    if rank < 29 {
        run_milestone23_scale_indexes_migration(pool).await?;
    }
    if rank < 30 {
        run_milestone25_factory_extensibility_migration(pool).await?;
    }
    Ok(())
}

/// Milestone 18: BYO-GSP e-Invoice/e-Way Bill Connector & Declarative Custom Report Extensions.
/// Stamps schema version `v3.0.0-enterprise` in `_opentally_metadata`.
pub async fn run_milestone18_gsp_extensions_migration(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(), crate::errors::OpenTallyError> {
    use crate::errors::OpenTallyError;

    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .map_err(OpenTallyError::Database)?;

    if current_version == "v3.0.0-enterprise" {
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS company_gsp_configs (
            company_id TEXT PRIMARY KEY NOT NULL,
            provider_mode TEXT NOT NULL DEFAULT 'Sandbox' CHECK (provider_mode IN ('Sandbox', 'HttpGsp')),
            provider_name TEXT NOT NULL DEFAULT 'OpenTally Sandbox',
            base_url TEXT,
            client_id TEXT,
            api_secret_ref TEXT,
            is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS custom_report_definitions (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            data_source TEXT NOT NULL CHECK (data_source IN ('Vouchers', 'Ledgers', 'StockItems', 'OutstandingBills', 'GstInvoices')),
            columns_json TEXT NOT NULL,
            filters_json TEXT NOT NULL DEFAULT '[]',
            sort_by TEXT,
            sort_dir TEXT NOT NULL DEFAULT 'ASC' CHECK (sort_dir IN ('ASC', 'DESC')),
            created_by TEXT NOT NULL DEFAULT 'system',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (company_id, name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v3.0.0-enterprise', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    tx.commit().await.map_err(OpenTallyError::Database)?;
    Ok(())
}

/// Milestone 19: Official GSTN Portal Schema Strictness & Complex Indian GST Edge Cases.
/// Stamps schema version `v3.1.0-gstn-strict` in `_opentally_metadata`.
pub async fn run_milestone19_gstn_strictness_migration(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(), crate::errors::OpenTallyError> {
    use crate::errors::OpenTallyError;

    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .map_err(OpenTallyError::Database)?;

    if current_version == "v3.1.0-gstn-strict" {
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

    // Helper to check existing columns on a table
    let gst_cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(voucher_gst_details)")
            .fetch_all(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    let has_gst_col = |name: &str| gst_cols.iter().any(|c| c.1 == name);

    if !has_gst_col("supply_category") {
        sqlx::query(
            "ALTER TABLE voucher_gst_details ADD COLUMN supply_category TEXT NOT NULL DEFAULT 'Regular' CHECK (supply_category IN ('Regular', 'ExportWithLUT', 'ExportWithTax', 'SEZWithLUT', 'SEZWithTax', 'DeemedExport', 'Composition'))",
        )
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;
    }
    if !has_gst_col("itc_eligibility") {
        sqlx::query(
            "ALTER TABLE voucher_gst_details ADD COLUMN itc_eligibility TEXT NOT NULL DEFAULT 'Eligible' CHECK (itc_eligibility IN ('Eligible', 'IneligibleSec17_5', 'Rule42_43Reversed'))",
        )
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;
    }
    if !has_gst_col("port_code") {
        sqlx::query("ALTER TABLE voucher_gst_details ADD COLUMN port_code TEXT")
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    }
    if !has_gst_col("shipping_bill_no") {
        sqlx::query("ALTER TABLE voucher_gst_details ADD COLUMN shipping_bill_no TEXT")
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    }
    if !has_gst_col("shipping_bill_date") {
        sqlx::query("ALTER TABLE voucher_gst_details ADD COLUMN shipping_bill_date TEXT")
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    }

    let profile_cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(company_gst_profiles)")
            .fetch_all(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    let has_prof_col = |name: &str| profile_cols.iter().any(|c| c.1 == name);

    if !has_prof_col("composition_rate") {
        sqlx::query(
            "ALTER TABLE company_gst_profiles ADD COLUMN composition_rate TEXT DEFAULT '1.00'",
        )
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;
    }
    if !has_prof_col("lut_arn") {
        sqlx::query("ALTER TABLE company_gst_profiles ADD COLUMN lut_arn TEXT")
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
    }

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v3.1.0-gstn-strict', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    tx.commit().await.map_err(OpenTallyError::Database)?;
    Ok(())
}

/// Milestone 20: Distributor Inventory & Billing Depth (Compound/Alternate UoMs, Actual vs Billed Qty, Price Levels, Multi-Method Valuation).
/// Stamps schema version `v3.2.0-inventory-depth` in `_opentally_metadata`.
pub async fn run_milestone20_inventory_billing_depth_migration(
    pool: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(), crate::errors::OpenTallyError> {
    use crate::errors::OpenTallyError;

    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await
            .map_err(OpenTallyError::Database)?;

    if current_version == "v3.2.0-inventory-depth" {
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        table: &str,
        col: &str,
        ddl: &str,
    ) -> Result<(), OpenTallyError> {
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&format!("PRAGMA table_info({})", table))
                .fetch_all(&mut **tx)
                .await
                .map_err(OpenTallyError::Database)?;
        if !cols.iter().any(|c| c.1 == col) {
            sqlx::query(&format!("ALTER TABLE {} ADD COLUMN {}", table, ddl))
                .execute(&mut **tx)
                .await
                .map_err(OpenTallyError::Database)?;
        }
        Ok(())
    }

    add_col_if_missing(
        &mut tx,
        "units",
        "unit_type",
        "unit_type TEXT NOT NULL DEFAULT 'Simple' CHECK (unit_type IN ('Simple', 'Compound'))",
    )
    .await?;
    add_col_if_missing(&mut tx, "units", "first_unit_id", "first_unit_id TEXT").await?;
    add_col_if_missing(&mut tx, "units", "second_unit_id", "second_unit_id TEXT").await?;
    add_col_if_missing(
        &mut tx,
        "units",
        "conversion_factor",
        "conversion_factor TEXT",
    )
    .await?;

    add_col_if_missing(
        &mut tx,
        "stock_items",
        "alternate_unit_id",
        "alternate_unit_id TEXT",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "stock_items",
        "alternate_conversion_num",
        "alternate_conversion_num TEXT",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "stock_items",
        "alternate_conversion_den",
        "alternate_conversion_den TEXT",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "stock_items",
        "valuation_method",
        "valuation_method TEXT NOT NULL DEFAULT 'FIFO' CHECK (valuation_method IN ('FIFO', 'LIFO', 'WeightedAverage'))",
    )
    .await?;

    for tbl in [
        "sales_document_lines",
        "purchase_document_lines",
        "stock_voucher_lines",
    ] {
        add_col_if_missing(&mut tx, tbl, "billed_quantity", "billed_quantity TEXT").await?;
        add_col_if_missing(&mut tx, tbl, "entered_unit_id", "entered_unit_id TEXT").await?;
    }

    add_col_if_missing(&mut tx, "ledgers", "price_level_id", "price_level_id TEXT").await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS price_levels (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (company_id, name),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS price_list_entries (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            price_level_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            effective_from TEXT NOT NULL,
            min_qty TEXT NOT NULL DEFAULT '0.0000',
            max_qty TEXT,
            unit_rate TEXT NOT NULL,
            discount_pct TEXT NOT NULL DEFAULT '0.00',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (price_level_id) REFERENCES price_levels(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v3.2.0-inventory-depth', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await
    .map_err(OpenTallyError::Database)?;

    tx.commit().await.map_err(OpenTallyError::Database)?;
    Ok(())
}

/// Milestone 22: Pan-India Statutory Payroll & TDS Depth.
/// Stamps schema version `v3.4.0-pan-india-statutory` in `_opentally_metadata`.
/// Milestone 23: High-Volume SQLite Scale Hardening (100k Vouchers).
/// Adds covering/composite indexes for hot report paths and runs `ANALYZE;`.
/// Stamps schema version `v3.5.0-scale-indexes` in `_opentally_metadata`.
/// Milestone 25: Factory Job-Work (GST ITC-04), Multi-Level BOM + Scrap Costing & Clean-Room TDL Replacement.
/// Stamps schema version `v4.1.0-factory-extensibility` in `_opentally_metadata`.
pub async fn run_milestone25_factory_extensibility_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await?;

    if current_version == "v4.1.0-factory-extensibility" {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS job_work_challans (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            challan_number TEXT NOT NULL,
            challan_type TEXT NOT NULL CHECK (challan_type IN ('MaterialOut', 'MaterialIn')),
            challan_date TEXT NOT NULL,
            job_worker_ledger_id TEXT NOT NULL,
            external_godown_id TEXT NOT NULL,
            linked_out_challan_id TEXT,
            goods_category TEXT NOT NULL CHECK (goods_category IN ('Input', 'CapitalGood')),
            statutory_due_date TEXT NOT NULL,
            stock_voucher_id TEXT,
            status TEXT NOT NULL DEFAULT 'Open' CHECK (status IN ('Open', 'PartiallyReturned', 'Closed', 'Cancelled')),
            narration TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (company_id, challan_number),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_jw_challans_company_type
            ON job_work_challans(company_id, challan_type, challan_date);

        CREATE TABLE IF NOT EXISTS job_work_challan_lines (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            challan_id TEXT NOT NULL,
            stock_item_id TEXT NOT NULL,
            dispatched_qty TEXT NOT NULL DEFAULT '0' CHECK (
                dispatched_qty NOT GLOB '*[^0-9.]*'
                AND dispatched_qty NOT LIKE '%.%.%'
                AND dispatched_qty != ''
                AND dispatched_qty != '.'
            ),
            returned_qty TEXT NOT NULL DEFAULT '0' CHECK (
                returned_qty NOT GLOB '*[^0-9.]*'
                AND returned_qty NOT LIKE '%.%.%'
                AND returned_qty != ''
                AND returned_qty != '.'
            ),
            scrap_qty TEXT NOT NULL DEFAULT '0' CHECK (
                scrap_qty NOT GLOB '*[^0-9.]*'
                AND scrap_qty NOT LIKE '%.%.%'
                AND scrap_qty != ''
                AND scrap_qty != '.'
            ),
            unit_cost TEXT NOT NULL DEFAULT '0.00' CHECK (
                unit_cost NOT GLOB '*[^0-9.]*'
                AND unit_cost NOT LIKE '%.%.%'
                AND unit_cost != ''
                AND unit_cost != '.'
            ),
            line_value TEXT NOT NULL DEFAULT '0.00' CHECK (
                line_value NOT GLOB '*[^0-9.]*'
                AND line_value NOT LIKE '%.%.%'
                AND line_value != ''
                AND line_value != '.'
            ),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (challan_id) REFERENCES job_work_challans(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS bom_byproducts (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            bom_id TEXT NOT NULL,
            byproduct_stock_item_id TEXT NOT NULL,
            expected_qty TEXT NOT NULL CHECK (
                expected_qty NOT GLOB '*[^0-9.]*'
                AND expected_qty NOT LIKE '%.%.%'
                AND expected_qty != ''
                AND expected_qty != '.'
            ),
            recovery_rate TEXT NOT NULL CHECK (
                recovery_rate NOT GLOB '*[^0-9.]*'
                AND recovery_rate NOT LIKE '%.%.%'
                AND recovery_rate != ''
                AND recovery_rate != '.'
            ),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS custom_field_definitions (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            entity_scope TEXT NOT NULL CHECK (entity_scope IN ('Voucher', 'Ledger', 'StockItem')),
            field_key TEXT NOT NULL,
            label TEXT NOT NULL,
            field_type TEXT NOT NULL CHECK (field_type IN ('Text', 'Decimal', 'Date', 'Boolean')),
            is_required INTEGER NOT NULL DEFAULT 0 CHECK (is_required IN (0, 1)),
            show_on_pdf INTEGER NOT NULL DEFAULT 0 CHECK (show_on_pdf IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (company_id, entity_scope, field_key),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS custom_field_values (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            definition_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            value_text TEXT NOT NULL,
            UNIQUE (company_id, definition_id, entity_id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (definition_id) REFERENCES custom_field_definitions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS webhook_subscriptions (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL,
            event_type TEXT NOT NULL CHECK (event_type IN (
                'voucher.posted', 'voucher.cancelled', 'stock.moved',
                'jobwork.dispatched', 'jobwork.received'
            )),
            target_url TEXT NOT NULL,
            hmac_secret_ref TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS webhook_delivery_logs (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            subscription_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            signature_header TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('Pending', 'Delivered', 'Failed')),
            http_status INTEGER,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
            FOREIGN KEY (subscription_id) REFERENCES webhook_subscriptions(id) ON DELETE CASCADE
        );
        "#,
    )
    .execute(&mut *tx)
    .await?;

    let has_barcode: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('stock_items') WHERE name = 'barcode'",
    )
    .fetch_one(&mut *tx)
    .await?;
    if has_barcode == 0 {
        sqlx::query("ALTER TABLE stock_items ADD COLUMN barcode TEXT")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_stock_items_barcode ON stock_items(company_id, barcode)",
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v4.1.0-factory-extensibility', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn run_milestone23_scale_indexes_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await?;

    if current_version == "v3.5.0-scale-indexes" {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_vouchers_scale_hot
            ON vouchers(company_id, status, date, id);
        CREATE INDEX IF NOT EXISTS idx_voucher_lines_scale_hot
            ON voucher_lines(company_id, ledger_id, voucher_id, entry_type, amount);
        CREATE INDEX IF NOT EXISTS idx_voucher_lines_voucher_covering
            ON voucher_lines(company_id, voucher_id, entry_type, amount);
        CREATE INDEX IF NOT EXISTS idx_voucher_gst_details_scale_hot
            ON voucher_gst_details(company_id, voucher_id);
        CREATE INDEX IF NOT EXISTS idx_inventory_cost_layers_scale_hot
            ON inventory_cost_layers(company_id, stock_item_id, godown_id, remaining_quantity);
        CREATE INDEX IF NOT EXISTS idx_ledgers_scale_name
            ON ledgers(company_id, name, id);
        ANALYZE;
        "#,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v3.5.0-scale-indexes', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn run_milestone22_pan_india_payroll_tds_migration(
    pool: &Pool<Sqlite>,
) -> Result<(), OpenTallyError> {
    let current_version: String =
        sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
            .fetch_one(pool)
            .await?;

    if current_version == "v3.4.0-pan-india-statutory" {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    #[allow(dead_code)]
    async fn add_col_if_missing(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        table: &str,
        col: &str,
        def: &str,
    ) -> Result<(), OpenTallyError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
            sqlx::query_as(&pragma).fetch_all(&mut **tx).await?;
        if !cols.iter().any(|c| c.1 == col) {
            let alter = format!("ALTER TABLE {} ADD COLUMN {}", table, def);
            sqlx::query(&alter).execute(&mut **tx).await?;
        }
        Ok(())
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS professional_tax_slabs (
            id TEXT PRIMARY KEY NOT NULL,
            company_id TEXT NOT NULL,
            state_code TEXT NOT NULL,
            min_gross TEXT NOT NULL DEFAULT '0.00' CHECK (
                min_gross NOT GLOB '*[^0-9.]*'
                AND min_gross NOT LIKE '%.%.%'
                AND min_gross != ''
                AND min_gross != '.'
            ),
            max_gross TEXT CHECK (
                max_gross IS NULL OR (
                    max_gross NOT GLOB '*[^0-9.]*'
                    AND max_gross NOT LIKE '%.%.%'
                    AND max_gross != ''
                    AND max_gross != '.'
                )
            ),
            monthly_tax TEXT NOT NULL DEFAULT '0.00' CHECK (
                monthly_tax NOT GLOB '*[^0-9.]*'
                AND monthly_tax NOT LIKE '%.%.%'
                AND monthly_tax != ''
                AND monthly_tax != '.'
            ),
            february_tax TEXT CHECK (
                february_tax IS NULL OR (
                    february_tax NOT GLOB '*[^0-9.]*'
                    AND february_tax NOT LIKE '%.%.%'
                    AND february_tax != ''
                    AND february_tax != '.'
                )
            ),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_pt_slabs_company_state
            ON professional_tax_slabs(company_id, state_code);
        "#,
    )
    .execute(&mut *tx)
    .await?;

    add_col_if_missing(
        &mut tx,
        "employees",
        "work_state_code",
        "work_state_code TEXT NOT NULL DEFAULT '29'",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "employees",
        "tax_regime",
        "tax_regime TEXT NOT NULL DEFAULT 'New' CHECK (tax_regime IN ('New', 'Old'))",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "employees",
        "annual_declared_deductions",
        "annual_declared_deductions TEXT NOT NULL DEFAULT '0.00'",
    )
    .await?;

    add_col_if_missing(
        &mut tx,
        "payroll_payslips",
        "tds_sec192",
        "tds_sec192 TEXT NOT NULL DEFAULT '0.00'",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "payroll_ledger_mappings",
        "salary_tds_payable_ledger_id",
        "salary_tds_payable_ledger_id TEXT",
    )
    .await?;
    add_col_if_missing(
        &mut tx,
        "tds_nature_of_payments",
        "is_tcs",
        "is_tcs INTEGER NOT NULL DEFAULT 0 CHECK (is_tcs IN (0, 1))",
    )
    .await?;

    sqlx::query(
        "UPDATE _opentally_metadata SET schema_version = 'v3.4.0-pan-india-statutory', app_version = ? WHERE id = 1",
    )
    .bind(env!("CARGO_PKG_VERSION"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
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
