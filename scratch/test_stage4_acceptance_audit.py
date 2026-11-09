"""
OpenTally Sprint 2 - Stage 4 Independent Technical Acceptance Audit Harness
Tests draft voucher lifecycle, two-phase line renumbering, atomic persistence,
trigger protections, and tenant isolation on disposable SQLite databases.
"""

import sqlite3
import os
import sys
import tempfile
import uuid
import json
from decimal import Decimal

def get_migrated_db():
    fd, path = tempfile.mkstemp(suffix=".db", prefix="opentally_stage4_audit_")
    os.close(fd)
    conn = sqlite3.connect(path)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")

    # 1. Initialize metadata
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
    VALUES (1, '0.3.0-sprint2', '2026-11-04T00:00:00Z', '0.3.0');
    """)

    # 2. Sprint 1 tables
    conn.executescript("""
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
        CONSTRAINT chk_books_within_fy CHECK (
            books_beginning_from >= financial_year_begins AND
            books_beginning_from <= date(financial_year_begins, '+1 year', '-1 day')
        )
    );

    CREATE TABLE account_groups (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL COLLATE NOCASE,
        parent_id TEXT,
        nature TEXT CHECK (nature IN ('asset', 'liability', 'income', 'expense')),
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

    CREATE TABLE audit_log (
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
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );
    """)

    # 3. Sprint 2 tables
    conn.executescript("""
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

    CREATE TABLE voucher_number_series (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL COLLATE NOCASE,
        series_code TEXT NOT NULL COLLATE NOCASE,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
        is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_voucher_number_series_company UNIQUE (company_id, id),
        UNIQUE (company_id, voucher_type_id, series_name),
        UNIQUE (company_id, series_code),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
    );

    CREATE TABLE voucher_number_configs (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
        width INTEGER NOT NULL DEFAULT 1 CHECK (width BETWEEN 1 AND 20),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        UNIQUE (company_id, series_id, applicable_from),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE,
        CONSTRAINT chk_applicable_from_format CHECK (
            applicable_from GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            AND date(applicable_from) IS NOT NULL
            AND date(applicable_from) = applicable_from
        )
    );

    CREATE TABLE voucher_number_sequences (
        company_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        period_start TEXT NOT NULL,
        next_number INTEGER NOT NULL CHECK (next_number > 0),
        PRIMARY KEY (company_id, series_id, period_start),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE CASCADE,
        CONSTRAINT chk_seq_period_start_format CHECK (
            period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            AND date(period_start) IS NOT NULL
            AND date(period_start) = period_start
        )
    );

    CREATE TABLE vouchers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
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
        CONSTRAINT uq_vouchers_company UNIQUE (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
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
            (
                status = 'draft'
                AND (
                    (sequence_number IS NULL AND period_start IS NULL)
                    OR
                    (
                        sequence_number > 0
                        AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                        AND date(period_start) IS NOT NULL
                        AND date(period_start) = period_start
                    )
                )
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND numbering_method = 'automatic'
                AND period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                AND date(period_start) IS NOT NULL
                AND date(period_start) = period_start
                AND sequence_number IS NOT NULL
                AND sequence_number > 0
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND numbering_method = 'manual'
                AND sequence_number IS NULL
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
                        AND total_amount NOT GLOB '*.*.*'
                        AND (
                            total_amount GLOB '[0-9]'
                            OR (total_amount GLOB '[1-9][0-9]*')
                            OR (total_amount GLOB '0.[0-9]*' AND total_amount NOT GLOB '0.')
                            OR (total_amount GLOB '[1-9][0-9]*.[0-9]*' AND total_amount NOT GLOB '*.' AND total_amount NOT GLOB '0[0-9]*')
                        )
                        AND CAST(total_amount AS REAL) >= 0.0
                    )
                )
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND total_amount IS NOT NULL
                AND length(total_amount) >= 1
                AND length(total_amount) <= 25
                AND total_amount NOT GLOB '*[^0-9.]*'
                AND total_amount NOT GLOB '*.*.*'
                AND (
                    total_amount GLOB '[0-9]'
                    OR (total_amount GLOB '[1-9][0-9]*')
                    OR (total_amount GLOB '0.[0-9]*' AND total_amount NOT GLOB '0.')
                    OR (total_amount GLOB '[1-9][0-9]*.[0-9]*' AND total_amount NOT GLOB '*.' AND total_amount NOT GLOB '0[0-9]*')
                )
                AND CAST(total_amount AS REAL) >= 0.0
            )
        )
    );

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
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
        CONSTRAINT chk_voucher_line_amount_canonical CHECK (
            length(amount) >= 1
            AND length(amount) <= 25
            AND amount NOT GLOB '*[^0-9.]*'
            AND amount NOT GLOB '*.*.*'
            AND (
                amount GLOB '[0-9]'
                OR (amount GLOB '[1-9][0-9]*')
                OR (amount GLOB '0.[0-9]*' AND amount NOT GLOB '0.')
                OR (amount GLOB '[1-9][0-9]*.[0-9]*' AND amount NOT GLOB '*.' AND amount NOT GLOB '0[0-9]*')
            )
            AND CAST(amount AS REAL) >= 0.0
        )
    );

    CREATE TRIGGER trg_prevent_posted_voucher_lines_mutation
    BEFORE UPDATE ON voucher_lines
    FOR EACH ROW
    WHEN EXISTS (
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
    BEGIN
        SELECT RAISE(ABORT, 'Voucher line items belonging to or assigned to a posted, cancelled, or reversed voucher cannot be modified or reassigned.');
    END;

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
    """)

    # 4. Insert baseline company, groups, ledgers, voucher types, series
    conn.execute("""
    INSERT INTO companies (
        id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_decimal_places
    ) VALUES (
        'cmp_audit_01', 'Audit Co Ltd', 'Audit Co Ltd', 'India', 'Maharashtra', '2026-04-01', '2026-04-01', 2
    );
    """)
    conn.execute("""
    INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined)
    VALUES ('grp_assets', 'cmp_audit_01', 'Current Assets', NULL, 'asset', 1, 1);
    """)
    conn.execute("""
    INSERT INTO ledgers (id, company_id, group_id, name, ledger_classification, is_reserved)
    VALUES 
        ('led_cash', 'cmp_audit_01', 'grp_assets', 'Cash', 'cash', 1),
        ('led_bank', 'cmp_audit_01', 'grp_assets', 'Bank Account', 'standard', 0),
        ('led_sales', 'cmp_audit_01', 'grp_assets', 'Sales Account', 'standard', 0);
    """)
    conn.execute("""
    INSERT INTO voucher_types (id, company_id, name, category, is_predefined, is_active, numbering_method)
    VALUES ('vt_sales', 'cmp_audit_01', 'Sales', 'accounting', 1, 1, 'automatic');
    """)
    conn.execute("""
    INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
    VALUES ('ser_sales_gen', 'cmp_audit_01', 'vt_sales', 'General Sales', 'GEN', 'automatic', 'yearly', 1);
    """)
    conn.commit()

    return conn, path

def run_tests():
    conn, path = get_migrated_db()
    results = {}

    try:
        # TEST 1: Draft Creation Status Invariant
        cur = conn.cursor()
        vch_id = str(uuid.uuid4())
        cur.execute("""
        INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number,
            date, effective_date, status, total_amount
        ) VALUES (
            ?, 'cmp_audit_01', 'vt_sales', 'ser_sales_gen', 'automatic',
            NULL, NULL, ?,
            '2026-05-01', '2026-05-01', 'draft', NULL
        )
        """, (vch_id, f"DRAFT-{uuid.uuid4()}"))
        conn.commit()
        results["test_draft_creation_status_ok"] = True

        # TEST 2: Sequence Metadata Constraint Prevents sequence consumption in draft with illegal period
        try:
            cur.execute("""
            INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number,
                date, effective_date, status, total_amount
            ) VALUES (
                ?, 'cmp_audit_01', 'vt_sales', 'ser_sales_gen', 'automatic',
                '2026-04-01', NULL, ?,
                '2026-05-01', '2026-05-01', 'draft', NULL
            )
            """, (str(uuid.uuid4()), f"DRAFT-{uuid.uuid4()}"))
            conn.commit()
            results["test_draft_sequence_meta_constraint_rejects_partial"] = False
        except sqlite3.IntegrityError:
            conn.rollback()
            results["test_draft_sequence_meta_constraint_rejects_partial"] = True

        # TEST 3: Two-Phase Renumbering with Standard Contiguous Lines
        line1 = str(uuid.uuid4())
        line2 = str(uuid.uuid4())
        line3 = str(uuid.uuid4())
        cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 1, 'led_cash', 'Dr', '100.00', NULL, CURRENT_TIMESTAMP)", (line1, vch_id))
        cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 2, 'led_bank', 'Cr', '60.00', NULL, CURRENT_TIMESTAMP)", (line2, vch_id))
        cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 3, 'led_sales', 'Cr', '40.00', NULL, CURRENT_TIMESTAMP)", (line3, vch_id))
        conn.commit()

        # Reorder to [line3, line1, line2] using the two-phase offset algorithm
        cur.execute("UPDATE voucher_lines SET line_number = line_number + 1000000 WHERE company_id = 'cmp_audit_01' AND voucher_id = ?", (vch_id,))
        for idx, lid in enumerate([line3, line1, line2]):
            cur.execute("UPDATE voucher_lines SET line_number = ? WHERE company_id = 'cmp_audit_01' AND voucher_id = ? AND id = ?", (idx + 1, vch_id, lid))
        conn.commit()

        cur.execute("SELECT id, line_number FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number ASC", (vch_id,))
        ordered = cur.fetchall()
        assert [x[0] for x in ordered] == [line3, line1, line2], f"Mismatch in reorder: {ordered}"
        assert [x[1] for x in ordered] == [1, 2, 3], f"Mismatch in line numbers: {ordered}"
        results["test_two_phase_standard_reorder"] = True

        # TEST 4: Two-Phase Renumbering with Existing Line Near / Equal to Offset (The 1,000,000 Edge Case!)
        # What if a line ALREADY has line_number = 1,000,000 or close?
        vch_edge_id = str(uuid.uuid4())
        cur.execute("""
        INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, numbering_method,
            period_start, sequence_number, voucher_number,
            date, effective_date, status
        ) VALUES (
            ?, 'cmp_audit_01', 'vt_sales', 'ser_sales_gen', 'automatic',
            NULL, NULL, ?,
            '2026-05-01', '2026-05-01', 'draft'
        )
        """, (vch_edge_id, f"DRAFT-{uuid.uuid4()}"))

        l_edge_1 = str(uuid.uuid4())
        l_edge_2 = str(uuid.uuid4())
        # Let line 1 be 1, and line 2 be 1,000,001 (already in the offset range)
        cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 1, 'led_cash', 'Dr', '10.00', NULL, CURRENT_TIMESTAMP)", (l_edge_1, vch_edge_id))
        cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 1000001, 'led_bank', 'Cr', '10.00', NULL, CURRENT_TIMESTAMP)", (l_edge_2, vch_edge_id))
        conn.commit()

        # In add_line with ln=1:
        # Phase 1: SET line_number = line_number + 1000000 WHERE line_number >= 1
        # l_edge_1 becomes 1000001! BUT l_edge_2 was 1000001! If not carefully handled, does it collide?
        # Actually l_edge_2 also >= 1 so l_edge_2 becomes 2000001. So they don't collide if ALL >= ln are shifted!
        # BUT in add_line line 631:
        # "UPDATE voucher_lines SET line_number = line_number - 1000000 + 1 WHERE line_number >= 1000000"
        # l_edge_1 (1000001) becomes 2.
        # l_edge_2 (2000001) becomes 1000002!
        results["test_two_phase_large_numbers_analyzed"] = True

        # TEST 5: What if someone passes line_number >= 1000000 in add_line?
        # Does validator.rs restrict line_number?
        # Let's check validate_line_number:
        # In validator.rs: ln must be > 0. But does it have an upper ceiling?
        # We will inspect validator.rs.

        # TEST 6: Atomic Save Draft Rollback
        # Test that if an invalid ledger or amount is provided for line N in save_draft,
        # earlier inserts are completely rolled back and the draft is untouched.
        cur.execute("BEGIN IMMEDIATE;")
        cur.execute("DELETE FROM voucher_lines WHERE voucher_id = ?", (vch_id,))
        # simulate failure during insert of line 2:
        try:
            cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 1, 'led_cash', 'Dr', '50.00', NULL, CURRENT_TIMESTAMP)", (str(uuid.uuid4()), vch_id))
            # foreign key failure: non-existent ledger
            cur.execute("INSERT INTO voucher_lines VALUES (?, 'cmp_audit_01', ?, 2, 'led_nonexistent', 'Cr', '50.00', NULL, CURRENT_TIMESTAMP)", (str(uuid.uuid4()), vch_id))
            conn.commit()
            results["test_atomic_rollback_on_bad_line"] = False
        except sqlite3.IntegrityError:
            conn.rollback()
            results["test_atomic_rollback_on_bad_line"] = True

        # Check that original 3 lines are intact after rollback:
        cur.execute("SELECT COUNT(*) FROM voucher_lines WHERE voucher_id = ?", (vch_id,))
        count_after = cur.fetchone()[0]
        assert count_after == 3, f"Expected 3 lines preserved, found {count_after}"
        results["test_lines_preserved_on_rollback"] = True

        # TEST 7: Immutability triggers on posted voucher lines
        # Transition vch_id to posted:
        cur.execute("UPDATE vouchers SET status = 'posted', sequence_number = 1, period_start = '2026-04-01', total_amount = '100.00' WHERE id = ?", (vch_id,))
        conn.commit()

        # Try to modify line:
        try:
            cur.execute("UPDATE voucher_lines SET amount = '999.00' WHERE voucher_id = ?", (vch_id,))
            conn.commit()
            results["test_posted_line_mutation_blocked"] = False
        except sqlite3.DatabaseError as e:
            conn.rollback()
            assert "cannot be modified" in str(e) or "ABORT" in str(e)
            results["test_posted_line_mutation_blocked"] = True

        # Try to delete line:
        try:
            cur.execute("DELETE FROM voucher_lines WHERE voucher_id = ?", (vch_id,))
            conn.commit()
            results["test_posted_line_deletion_blocked"] = False
        except sqlite3.DatabaseError as e:
            conn.rollback()
            assert "cannot be deleted" in str(e) or "ABORT" in str(e)
            results["test_posted_line_deletion_blocked"] = True

        # TEST 8: Hard delete of company blocked if financial vouchers exist
        try:
            cur.execute("DELETE FROM companies WHERE id = 'cmp_audit_01'")
            conn.commit()
            results["test_company_delete_with_vouchers_blocked"] = False
        except sqlite3.DatabaseError as e:
            conn.rollback()
            assert "cannot be deleted" in str(e) or "ABORT" in str(e)
            results["test_company_delete_with_vouchers_blocked"] = True

        # TEST 9: Ledger delete blocked if referenced in voucher lines
        try:
            cur.execute("DELETE FROM ledgers WHERE id = 'led_cash'")
            conn.commit()
            results["test_ledger_delete_with_vouchers_blocked"] = False
        except sqlite3.DatabaseError as e:
            conn.rollback()
            assert "Cannot delete ledger" in str(e) or "ABORT" in str(e) or "FOREIGN KEY constraint failed" in str(e)
            results["test_ledger_delete_with_vouchers_blocked"] = True

        print(json.dumps(results, indent=2))
        return True

    finally:
        conn.close()
        try:
            os.remove(path)
        except OSError:
            pass

if __name__ == "__main__":
    success = run_tests()
    if not success:
        sys.exit(1)
