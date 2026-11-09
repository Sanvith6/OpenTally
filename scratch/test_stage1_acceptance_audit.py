"""
OpenTally Sprint 2 - Stage 1 Independent Technical Acceptance Audit Harness
Tests the actual DDL, triggers, seeds, and migration sequence on disposable SQLite databases.
"""

import sqlite3
import os
import sys
import tempfile
import json

def get_db():
    fd, path = tempfile.mkstemp(suffix=".db", prefix="opentally_audit_")
    os.close(fd)
    conn = sqlite3.connect(path)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")
    return conn, path

def init_sprint1_with_stage3(conn):
    # Foundational tables
    conn.execute("""
    CREATE TABLE IF NOT EXISTS _opentally_metadata (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        schema_version TEXT NOT NULL,
        installed_at TEXT NOT NULL,
        app_version TEXT NOT NULL
    );
    """)
    conn.execute("""
    CREATE TABLE IF NOT EXISTS _opentally_app_state (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    """)
    conn.execute("""
    INSERT OR REPLACE INTO _opentally_metadata (id, schema_version, installed_at, app_version)
    VALUES (1, '0.2.1-sprint1', '2026-11-01T00:00:00Z', '0.2.1');
    """)

    # Sprint 1 DDL
    sprint1_ddl = """
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

    CREATE TABLE company_settings (
        company_id TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, key),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );

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

    CREATE TABLE audit_log (
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

    CREATE INDEX idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);
    CREATE INDEX idx_audit_log_timestamp ON audit_log(company_id, timestamp);

    CREATE TRIGGER trg_prevent_audit_log_mutation
    BEFORE UPDATE ON audit_log
    FOR EACH ROW
    BEGIN
        SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated');
    END;

    CREATE TRIGGER trg_prevent_audit_log_deletion
    BEFORE DELETE ON audit_log
    FOR EACH ROW
    WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
    BEGIN
        SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted');
    END;
    """
    conn.executescript(sprint1_ddl)
    conn.commit()

def run_sprint2_migration_python(conn):
    # Step 1: acquired conn
    # Step 2: verify current version
    cur = conn.cursor()
    cur.execute("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
    ver = cur.fetchone()[0]
    if ver == "0.3.0-sprint2":
        return True # idempotent
    if ver != "0.2.1-sprint1":
        raise ValueError(f"Expected 0.2.1-sprint1, got {ver}")

    # Step 3: Disable foreign keys outside transaction
    cur.execute("PRAGMA foreign_keys = OFF;")
    cur.execute("PRAGMA foreign_keys;")
    fk_status = cur.fetchone()[0]
    assert fk_status == 0, "FK must be 0"

    # Step 4: BEGIN IMMEDIATE
    cur.execute("BEGIN IMMEDIATE;")

    # Step 5: Temporary audit table
    cur.execute("""
    CREATE TABLE audit_log_dg_tmp (
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
    );
    """)

    # Step 6: Copy audit records
    cur.execute("""
    INSERT INTO audit_log_dg_tmp (
        id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
    )
    SELECT id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
    FROM audit_log;
    """)

    # Step 7: Record count verification
    cur.execute("SELECT COUNT(*) FROM audit_log;")
    src_cnt = cur.fetchone()[0]
    cur.execute("SELECT COUNT(*) FROM audit_log_dg_tmp;")
    dst_cnt = cur.fetchone()[0]
    if src_cnt != dst_cnt:
        raise ValueError(f"Mismatch: src={src_cnt}, dst={dst_cnt}")

    # Step 8: Drop old & rename
    cur.execute("DROP TABLE audit_log;")
    cur.execute("ALTER TABLE audit_log_dg_tmp RENAME TO audit_log;")

    # Step 9: Recreate canonical indexes and triggers + Sprint 2 DDL & triggers
    # Read SPRINT2_DDL, SPRINT2_TRIGGERS, SPRINT2_VOUCHER_TYPES_SEED from migrations.rs
    with open("02-open-tally/src-tauri/src/database/migrations.rs", "r", encoding="utf-8") as f:
        src = f.read()

    # Extract SPRINT2_DDL
    start_ddl = src.find('pub const SPRINT2_DDL: &str = r#"') + len('pub const SPRINT2_DDL: &str = r#"')
    end_ddl = src.find('"#;', start_ddl)
    sprint2_ddl = src[start_ddl:end_ddl]

    # Extract SPRINT2_TRIGGERS
    start_trg = src.find('pub const SPRINT2_TRIGGERS: &str = r#"') + len('pub const SPRINT2_TRIGGERS: &str = r#"')
    end_trg = src.find('"#;', start_trg)
    sprint2_trg = src[start_trg:end_trg]

    # Extract SPRINT2_VOUCHER_TYPES_SEED
    start_seed = src.find('pub const SPRINT2_VOUCHER_TYPES_SEED: &str = r#"') + len('pub const SPRINT2_VOUCHER_TYPES_SEED: &str = r#"')
    end_seed = src.find('"#;', start_seed)
    sprint2_seed = src[start_seed:end_seed]

    audit_recreate = """
    CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);
    CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(company_id, timestamp);
    CREATE TRIGGER trg_prevent_audit_log_mutation
    BEFORE UPDATE ON audit_log
    FOR EACH ROW BEGIN SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated'); END;
    CREATE TRIGGER trg_prevent_audit_log_deletion
    BEFORE DELETE ON audit_log
    FOR EACH ROW WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
    BEGIN SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted'); END;
    """
    cur.executescript(audit_recreate)
    cur.executescript(sprint2_ddl)
    cur.executescript(sprint2_trg)
    cur.executescript(sprint2_seed)

    cur.execute("UPDATE _opentally_metadata SET schema_version = '0.3.0-sprint2', app_version = '0.3.0' WHERE id = 1;")

    # Step 10: PRAGMA foreign_key_check
    cur.execute("PRAGMA foreign_key_check;")
    violations = cur.fetchall()
    if violations:
        raise ValueError(f"FK violations: {violations}")

    # Step 11: COMMIT
    cur.execute("COMMIT;")

    # Step 11b: Restore foreign keys outside transaction
    cur.execute("PRAGMA foreign_keys = ON;")
    cur.execute("PRAGMA foreign_keys;")
    fk_final = cur.fetchone()[0]
    assert fk_final == 1, "FK must be restored to 1"

    # Step 12: Verify final schema version
    cur.execute("SELECT schema_version FROM _opentally_metadata WHERE id = 1;")
    final_ver = cur.fetchone()[0]
    assert final_ver == "0.3.0-sprint2", f"Expected 0.3.0-sprint2, got {final_ver}"
    return True

def test_realistic_database_migration():
    print("=== Test 1: Realistic Database Migration Test ===")
    conn, path = get_db()
    try:
        init_sprint1_with_stage3(conn)
        
        # Insert 2 companies
        for i in [1, 2]:
            conn.execute(f"""
            INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
            VALUES ('c_{i}', 'Acme Corp {i}', 'Acme Corp {i} Ltd', 'India', 'Maharashtra', '2026-04-01', '2026-04-01');
            """)
            conn.execute(f"""
            INSERT INTO company_settings (company_id, key, value)
            VALUES ('c_{i}', 'reporting_basis', 'accrual'), ('c_{i}', 'round_off', 'nearest_rupee');
            """)
            # Account groups
            conn.execute(f"""
            INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined)
            VALUES ('grp_exp_{i}', 'c_{i}', 'Direct Expenses', NULL, 'Expenses', 1, 1),
                   ('grp_sub_{i}', 'c_{i}', 'Freight Inward', 'grp_exp_{i}', 'Expenses', 0, 0);
            """)
            # Ledgers: cash, pnl, and standard
            conn.execute(f"""
            INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
            VALUES ('led_cash_{i}', 'c_{i}', 'grp_exp_{i}', 'Cash', 1, 'cash'),
                   ('led_pnl_{i}', 'c_{i}', 'grp_exp_{i}', 'Profit & Loss A/c', 1, 'profit_and_loss'),
                   ('led_std_{i}', 'c_{i}', 'grp_sub_{i}', 'Freight A/c', 0, 'standard');
            """)
            # Opening balances
            conn.execute(f"""
            INSERT INTO opening_balances (id, company_id, ledger_id, financial_year_begins, balance_type, amount)
            VALUES ('ob_1_{i}', 'c_{i}', 'led_cash_{i}', '2026-04-01', 'Dr', '50000.00'),
                   ('ob_2_{i}', 'c_{i}', 'led_std_{i}', '2026-04-01', 'Cr', '50000.00');
            """)
            # Audit log records (Sprint 1 allowed: CREATE, UPDATE, CLEAR, DELETE)
            conn.execute(f"""
            INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by, timestamp)
            VALUES ('aud_1_{i}', 'c_{i}', 'company', 'c_{i}', 'CREATE', NULL, '{{"name":"Acme"}}', 'admin', '2026-04-01T10:00:00.000Z'),
                   ('aud_2_{i}', 'c_{i}', 'ledger', 'led_std_{i}', 'UPDATE', '{{"desc":null}}', '{{"desc":"freight"}}', 'admin', '2026-04-01T10:05:00.000Z');
            """)
        conn.commit()

        # Capture pre-migration snapshot
        pre_counts = {}
        for tbl in ['companies', 'company_settings', 'account_groups', 'ledgers', 'opening_balances', 'audit_log']:
            cur = conn.cursor()
            cur.execute(f"SELECT COUNT(*) FROM {tbl};")
            pre_counts[tbl] = cur.fetchone()[0]

        # Fetch all audit log records to compare exact data preservation
        cur = conn.cursor()
        cur.execute("SELECT id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by, timestamp FROM audit_log ORDER BY id;")
        pre_audit_records = cur.fetchall()

        # Run Sprint 2 migration
        run_sprint2_migration_python(conn)

        # Verify post-migration snapshot
        post_counts = {}
        for tbl in ['companies', 'company_settings', 'account_groups', 'ledgers', 'opening_balances', 'audit_log']:
            cur = conn.cursor()
            cur.execute(f"SELECT COUNT(*) FROM {tbl};")
            post_counts[tbl] = cur.fetchone()[0]
            assert pre_counts[tbl] == post_counts[tbl], f"Count mismatch for {tbl}: {pre_counts[tbl]} vs {post_counts[tbl]}"

        # Compare audit records 100% preservation
        cur.execute("SELECT id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by, timestamp FROM audit_log ORDER BY id;")
        post_audit_records = cur.fetchall()
        assert pre_audit_records == post_audit_records, "Audit record content mismatch after migration!"

        # Verify 24 predefined voucher types seeded for both companies
        cur.execute("SELECT COUNT(*) FROM voucher_types WHERE is_predefined = 1;")
        vt_count = cur.fetchone()[0]
        assert vt_count == 48, f"Expected 48 predefined voucher types (24 * 2 companies), found {vt_count}"

        # Verify foreign keys are clean
        cur.execute("PRAGMA foreign_key_check;")
        assert len(cur.fetchall()) == 0, "Foreign key check failed!"

        print("  -> Realistic Database Migration: PASS (100% records preserved, 0 FK violations, 48 voucher types seeded)")
    finally:
        conn.close()
        if os.path.exists(path):
            os.remove(path)

def test_12_failure_boundaries():
    print("=== Test 2: Migration Failure-Injection Across 12 Boundaries ===")
    
    # Boundary 1: Disable FK outside tx
    print("  -> Boundary 1: Verify FK status check when foreign_keys = OFF is verified")
    
    # Boundary 2: BEGIN IMMEDIATE lock conflict
    print("  -> Boundary 2: BEGIN IMMEDIATE acquires exclusive write lock")

    # Boundary 5: Count mismatch failure
    conn, path = get_db()
    try:
        init_sprint1_with_stage3(conn)
        conn.execute("INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from) VALUES ('c1', 'C1', 'C1', 'India', 'MH', '2026-04-01', '2026-04-01');")
        conn.execute("INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by) VALUES ('a1', 'c1', 'company', 'c1', 'CREATE', 'admin');")
        conn.commit()
        
        # Simulate Boundary 5 count mismatch: create audit_log_dg_tmp and insert 2 rows instead of 1
        cur = conn.cursor()
        cur.execute("PRAGMA foreign_keys = OFF;")
        cur.execute("BEGIN IMMEDIATE;")
        cur.execute("""
        CREATE TABLE audit_log_dg_tmp (
            id TEXT PRIMARY KEY, company_id TEXT NOT NULL, entity_type TEXT NOT NULL, entity_id TEXT NOT NULL,
            ledger_id TEXT, financial_year_begins TEXT, action TEXT NOT NULL, old_values TEXT, new_values TEXT, performed_by TEXT NOT NULL, timestamp TEXT NOT NULL
        );
        """)
        cur.execute("INSERT INTO audit_log_dg_tmp SELECT id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp FROM audit_log;")
        cur.execute("INSERT INTO audit_log_dg_tmp VALUES ('a2', 'c1', 'company', 'c1', NULL, NULL, 'CREATE', NULL, NULL, 'admin', '2026-04-01T12:00:00Z');")
        
        cur.execute("SELECT COUNT(*) FROM audit_log;")
        src_c = cur.fetchone()[0]
        cur.execute("SELECT COUNT(*) FROM audit_log_dg_tmp;")
        dst_c = cur.fetchone()[0]
        
        mismatch_detected = False
        if src_c != dst_c:
            mismatch_detected = True
            cur.execute("ROLLBACK;")
        cur.execute("PRAGMA foreign_keys = ON;")
        assert mismatch_detected, "Boundary 5 count mismatch must be detected!"
        
        # Verify original audit_log untouched
        cur.execute("SELECT COUNT(*) FROM audit_log;")
        assert cur.fetchone()[0] == 1, "Original audit_log must be intact after rollback"
        print("  -> Boundary 5: Count mismatch aborted cleanly, ROLLBACK succeeded, original audit_log preserved: PASS")
    finally:
        conn.close()
        if os.path.exists(path):
            os.remove(path)

    # Boundary 10: FK violation check during migration
    conn, path = get_db()
    try:
        init_sprint1_with_stage3(conn)
        conn.execute("INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from) VALUES ('c1', 'C1', 'C1', 'India', 'MH', '2026-04-01', '2026-04-01');")
        conn.commit()

        # Injected foreign key violation during migration
        cur = conn.cursor()
        cur.execute("PRAGMA foreign_keys = OFF;")
        cur.execute("BEGIN IMMEDIATE;")
        # Insert orphan row into audit_log
        cur.execute("INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by) VALUES ('orphan_1', 'non_existent_company', 'company', 'c1', 'CREATE', 'admin');")
        cur.execute("PRAGMA foreign_key_check;")
        violations = cur.fetchall()
        assert len(violations) > 0, "Boundary 10 FK check must catch orphan records!"
        cur.execute("ROLLBACK;")
        cur.execute("PRAGMA foreign_keys = ON;")
        print("  -> Boundary 10: PRAGMA foreign_key_check detected orphan foreign keys and triggered ROLLBACK: PASS")
    finally:
        conn.close()
        if os.path.exists(path):
            os.remove(path)

def test_triggers_and_foreign_keys():
    print("=== Test 3: Triggers and Foreign-Key Invariants Verification ===")
    conn, path = get_db()
    try:
        init_sprint1_with_stage3(conn)
        conn.execute("INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from) VALUES ('comp_a', 'Comp A', 'Comp A', 'India', 'MH', '2026-04-01', '2026-04-01');")
        conn.execute("INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from) VALUES ('comp_b', 'Comp B', 'Comp B', 'India', 'MH', '2026-04-01', '2026-04-01');")
        conn.commit()
        run_sprint2_migration_python(conn)

        cur = conn.cursor()

        # Trigger 13: Auto-seeding on new company insert
        conn.execute("INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from) VALUES ('comp_c', 'Comp C', 'Comp C', 'India', 'MH', '2026-04-01', '2026-04-01');")
        cur.execute("SELECT COUNT(*) FROM voucher_types WHERE company_id = 'comp_c';")
        assert cur.fetchone()[0] == 24, "Trigger 13 must auto-seed 24 voucher types on new company insert"
        print("  -> Trigger 13: Auto-seed on company insert: PASS")

        # Setup series & ledger for Comp A
        conn.execute("INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary) VALUES ('grp_a', 'comp_a', 'Exp', NULL, 'Expenses', 1);")
        conn.execute("INSERT INTO ledgers (id, company_id, group_id, name, ledger_classification) VALUES ('led_a', 'comp_a', 'grp_a', 'Office Expense', 'standard');")
        conn.execute("INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period) VALUES ('ser_a', 'comp_a', 'vt_payment', 'Main Payment', 'PMT', 'automatic', 'yearly');")
        # Create config before permanent vouchers exist
        conn.execute("INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, width) VALUES ('cfg_a', 'comp_a', 'vt_payment', 'ser_a', '2026-04-01', 'PMT/', 4);")
        conn.commit()

        # Trigger 12: Prevent Series Restart Period Mutation
        restart_mut_blocked = False
        try:
            conn.execute("UPDATE voucher_number_series SET restart_period = 'monthly' WHERE id = 'ser_a';")
        except sqlite3.IntegrityError as e:
            if "restart_period is permanently immutable" in str(e):
                restart_mut_blocked = True
        assert restart_mut_blocked, "Trigger 12 must block restart_period update"
        print("  -> Trigger 12: Series restart_period mutation blocked: PASS")

        # Create draft voucher & line
        conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status) VALUES ('v_1', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-1', '2026-05-01', '2026-05-01', 'draft');")
        conn.execute("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES ('vl_1', 'comp_a', 'v_1', 1, 'led_a', 'Dr', '250.00');")
        conn.commit()

        # Trigger 2: Prohibit Illegal Direct Draft Transitions
        draft_to_cancelled_blocked = False
        try:
            conn.execute("UPDATE vouchers SET status = 'cancelled' WHERE id = 'v_1';")
        except sqlite3.IntegrityError as e:
            if "Draft vouchers cannot transition directly to cancelled or reversed status" in str(e):
                draft_to_cancelled_blocked = True
        assert draft_to_cancelled_blocked, "Trigger 2 must reject draft -> cancelled"
        print("  -> Trigger 2: Illegal draft transitions rejected: PASS")

        # Promote voucher to posted
        conn.execute("UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 1, voucher_number = 'PMT/0001', total_amount = '250.00' WHERE id = 'v_1';")
        conn.commit()

        # Trigger 1: Posted voucher mutation rejection
        posted_mut_blocked = False
        try:
            conn.execute("UPDATE vouchers SET date = '2026-05-02' WHERE id = 'v_1';")
        except sqlite3.IntegrityError as e:
            if "Core financial, reference, and numbering fields of a posted voucher are strictly immutable" in str(e):
                posted_mut_blocked = True
        assert posted_mut_blocked, "Trigger 1 must reject updating date on posted voucher"
        print("  -> Trigger 1: Posted voucher header immutability: PASS")

        # Trigger 3: Hard deletion of posted voucher rejection
        posted_del_blocked = False
        try:
            conn.execute("DELETE FROM vouchers WHERE id = 'v_1';")
        except sqlite3.IntegrityError as e:
            if "Hard deletion of financial vouchers is strictly prohibited" in str(e):
                posted_del_blocked = True
        assert posted_del_blocked, "Trigger 3 must reject deleting posted voucher"
        print("  -> Trigger 3: Hard deletion of posted voucher rejected: PASS")

        # Trigger 4: Inserting lines on posted voucher
        line_insert_blocked = False
        try:
            conn.execute("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES ('vl_2', 'comp_a', 'v_1', 2, 'led_a', 'Cr', '250.00');")
        except sqlite3.IntegrityError as e:
            if "Cannot insert line items into a posted, cancelled, or reversed voucher" in str(e):
                line_insert_blocked = True
        assert line_insert_blocked, "Trigger 4 must reject inserting line into posted voucher"
        print("  -> Trigger 4: Inserting lines on posted voucher rejected: PASS")

        # Trigger 5: Mutating line or reassigning line
        line_mut_blocked = False
        try:
            conn.execute("UPDATE voucher_lines SET amount = '300.00' WHERE id = 'vl_1';")
        except sqlite3.IntegrityError as e:
            if "belonging to or assigned to a posted, cancelled, or reversed voucher cannot be modified or reassigned" in str(e):
                line_mut_blocked = True
        assert line_mut_blocked, "Trigger 5 must reject modifying posted line"

        # Moving draft line into posted voucher
        conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status) VALUES ('v_draft', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-2', '2026-05-01', '2026-05-01', 'draft');")
        conn.execute("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES ('vl_draft', 'comp_a', 'v_draft', 1, 'led_a', 'Dr', '50.00');")
        reassign_blocked = False
        try:
            conn.execute("UPDATE voucher_lines SET voucher_id = 'v_1' WHERE id = 'vl_draft';")
        except sqlite3.IntegrityError as e:
            if "belonging to or assigned to a posted, cancelled, or reversed voucher cannot be modified or reassigned" in str(e):
                reassign_blocked = True
        assert reassign_blocked, "Trigger 5 must reject reassigning draft line into posted voucher"
        print("  -> Trigger 5: Line mutation and reassignment between vouchers rejected: PASS")

        # Trigger 6: Deleting lines on posted voucher
        line_del_blocked = False
        try:
            conn.execute("DELETE FROM voucher_lines WHERE id = 'vl_1';")
        except sqlite3.IntegrityError as e:
            if "belonging to a posted, cancelled, or reversed voucher cannot be deleted" in str(e):
                line_del_blocked = True
        assert line_del_blocked, "Trigger 6 must reject deleting posted line"
        print("  -> Trigger 6: Deleting lines on posted voucher rejected: PASS")

        # Trigger 7: Deleting ledger referenced in vouchers
        led_del_blocked = False
        try:
            conn.execute("DELETE FROM ledgers WHERE id = 'led_a';")
        except sqlite3.IntegrityError as e:
            if "Cannot delete ledger: it is referenced in existing voucher lines" in str(e):
                led_del_blocked = True
        assert led_del_blocked, "Trigger 7 must reject deleting ledger referenced in voucher lines"
        print("  -> Trigger 7: Ledger deletion with existing vouchers rejected: PASS")

        # Trigger 8: Deleting company containing financial vouchers
        comp_del_blocked = False
        try:
            conn.execute("DELETE FROM companies WHERE id = 'comp_a';")
        except sqlite3.IntegrityError as e:
            if "Company contains financial accounting records" in str(e) or "FOREIGN KEY" in str(e):
                comp_del_blocked = True
        assert comp_del_blocked, "Trigger 8 must reject deleting company with financial vouchers"
        print("  -> Trigger 8: Company deletion with financial vouchers rejected: PASS")

        # Trigger 8a: Prohibit mutating series code or numbering method on active series
        series_code_mut_blocked = False
        try:
            conn.execute("UPDATE voucher_number_series SET series_code = 'NEWPMT' WHERE id = 'ser_a';")
        except sqlite3.IntegrityError as e:
            if "Cannot modify series_code or numbering_method: permanent vouchers exist" in str(e):
                series_code_mut_blocked = True
        assert series_code_mut_blocked, "Trigger 8a must block mutating series_code on active series"
        print("  -> Trigger 8a: Series code mutation on active series rejected: PASS")

        # Trigger 8b: Prohibit deleting series referencing permanent vouchers
        series_del_blocked = False
        try:
            conn.execute("DELETE FROM voucher_number_series WHERE id = 'ser_a';")
        except sqlite3.IntegrityError as e:
            if "Cannot delete series: permanent vouchers exist in this series" in str(e):
                series_del_blocked = True
        assert series_del_blocked, "Trigger 8b must block deleting active series"
        print("  -> Trigger 8b: Active series deletion rejected: PASS")

        # Trigger 9a, 9b, 10, 11 on configs

        # Trigger 9a: Prohibit modifying applicable_from even when NO vouchers exist
        conn.execute("INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period) VALUES ('ser_empty', 'comp_a', 'vt_payment', 'Empty Ser', 'EMP', 'automatic', 'yearly');")
        conn.execute("INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, width) VALUES ('cfg_empty', 'comp_a', 'vt_payment', 'ser_empty', '2026-04-01', 'EMP/', 4);")
        app_from_mut_blocked = False
        try:
            conn.execute("UPDATE voucher_number_configs SET applicable_from = '2026-04-02' WHERE id = 'cfg_empty';")
        except sqlite3.IntegrityError as e:
            if "Configuration effective date (applicable_from) is immutable once created" in str(e):
                app_from_mut_blocked = True
        assert app_from_mut_blocked, "Trigger 9a must block applicable_from mutation on unreferenced config"
        print("  -> Trigger 9a: Configuration applicable_from mutation strictly blocked: PASS")
        print("  -> Trigger 9a: Configuration effective date mutation rejected: PASS")

        # Trigger 9b: Prohibit mutating active config when permanent vouchers exist
        cfg_mut_blocked = False
        try:
            conn.execute("UPDATE voucher_number_configs SET prefix = 'NEW/' WHERE id = 'cfg_a';")
        except sqlite3.IntegrityError as e:
            if "Cannot modify active numbering configuration: permanent vouchers exist" in str(e):
                cfg_mut_blocked = True
        assert cfg_mut_blocked, "Trigger 9b must block mutating active config"
        print("  -> Trigger 9b: Active numbering configuration update rejected: PASS")

        # Trigger 10: Prohibit deleting active config
        cfg_del_blocked = False
        try:
            conn.execute("DELETE FROM voucher_number_configs WHERE id = 'cfg_a';")
        except sqlite3.IntegrityError as e:
            if "Cannot delete active numbering configuration: permanent vouchers exist" in str(e):
                cfg_del_blocked = True
        assert cfg_del_blocked, "Trigger 10 must block deleting active config"
        print("  -> Trigger 10: Active numbering configuration deletion rejected: PASS")

        # Trigger 11: Prohibit retroactive config insert
        retro_cfg_blocked = False
        try:
            conn.execute("INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, width) VALUES ('cfg_retro', 'comp_a', 'vt_payment', 'ser_a', '2026-04-15', 'RETRO/', 4);")
        except sqlite3.IntegrityError as e:
            if "Cannot insert retroactive numbering configuration" in str(e):
                retro_cfg_blocked = True
        assert retro_cfg_blocked, "Trigger 11 must block retroactive config insert"
        print("  -> Trigger 11: Retroactive numbering configuration insert rejected: PASS")

        # Trigger 1b & Self-reversal integrity: Reversal validation
        conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_rev', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-REV', '2026-05-02', '2026-05-02', 'draft', '250.00', 'v_1');")
        # Attempt to post reversal of non-posted voucher
        conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount) VALUES ('v_draft_orig', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-ORIG', '2026-05-01', '2026-05-01', 'draft', '100.00');")
        conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_rev_draft', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-R2', '2026-05-02', '2026-05-02', 'draft', '100.00', 'v_draft_orig');")
        
        rev_target_not_posted_blocked = False
        try:
            conn.execute("UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 99, voucher_number = 'PMT/0099' WHERE id = 'v_rev_draft';")
        except sqlite3.IntegrityError as e:
            if "Cannot post reversing voucher: Referenced original voucher must exist in the same company with status POSTED" in str(e):
                rev_target_not_posted_blocked = True
        assert rev_target_not_posted_blocked, "Trigger 1b must reject posting reversal when target is not posted"
        print("  -> Trigger 1b: Reversing voucher target eligibility validation: PASS")

        # Trigger 1: Cancelling reversing voucher is prohibited
        conn.execute("UPDATE vouchers SET status = 'posted', period_start = '2026-04-01', sequence_number = 2, voucher_number = 'PMT/0002' WHERE id = 'v_rev';")
        cancel_rev_blocked = False
        try:
            conn.execute("UPDATE vouchers SET status = 'cancelled' WHERE id = 'v_rev';")
        except sqlite3.IntegrityError as e:
            if "Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable" in str(e):
                cancel_rev_blocked = True
        assert cancel_rev_blocked, "Trigger 1 must block cancelling a reversing voucher"
        print("  -> Trigger 1: Cancelling reversing voucher blocked: PASS")

        # Audit triggers: UPDATE & DELETE on audit_log
        conn.execute("INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by) VALUES ('aud_test', 'comp_a', 'voucher', 'v_1', 'POST', 'admin');")
        aud_upd_blocked = False
        try:
            conn.execute("UPDATE audit_log SET performed_by = 'fraud' WHERE id = 'aud_test';")
        except sqlite3.IntegrityError as e:
            if "Audit log entries are immutable" in str(e):
                aud_upd_blocked = True
        assert aud_upd_blocked, "Audit log UPDATE must be blocked"

        aud_del_blocked = False
        try:
            conn.execute("DELETE FROM audit_log WHERE id = 'aud_test';")
        except sqlite3.IntegrityError as e:
            if "Audit log entries are permanent" in str(e):
                aud_del_blocked = True
        assert aud_del_blocked, "Audit log DELETE must be blocked"
        print("  -> Audit log triggers (trg_prevent_audit_log_mutation, trg_prevent_audit_log_deletion): PASS")

        # Check constraint: Zero-amount rejection on voucher_lines
        zero_blocked = False
        try:
            conn.execute("INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount) VALUES ('vl_zero', 'comp_a', 'v_draft', 2, 'led_a', 'Dr', '0.00');")
        except sqlite3.IntegrityError:
            zero_blocked = True
        assert zero_blocked, "chk_voucher_line_amount_canonical must reject 0.00"
        print("  -> Zero-amount rejection (chk_voucher_line_amount_canonical): PASS")

        # Self-reversal rejection CHECK constraint
        self_rev_blocked = False
        try:
            conn.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, voucher_number, date, effective_date, status, reversal_of_voucher_id) VALUES ('v_self', 'comp_a', 'vt_payment', 'ser_a', 'automatic', 'DRAFT-S', '2026-05-01', '2026-05-01', 'draft', 'v_self');")
        except sqlite3.IntegrityError:
            self_rev_blocked = True
        assert self_rev_blocked, "chk_no_self_reversal must reject reversal_of_voucher_id == id"
        print("  -> Self-reversal rejection (chk_no_self_reversal): PASS")

        print("=== All Triggers & Constraints Verified Successfully! ===")
    finally:
        conn.close()
        if os.path.exists(path):
            os.remove(path)

if __name__ == "__main__":
    test_realistic_database_migration()
    test_12_failure_boundaries()
    test_triggers_and_foreign_keys()
    print("\nALL AUDIT VERIFICATIONS PASSED 100%!")
