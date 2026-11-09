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

    if current_version == "0.3.0-sprint2" || current_version == "0.3.1-sprint2" {
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

    if current_version == "0.3.1-sprint2" {
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
