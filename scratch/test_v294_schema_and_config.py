import sqlite3
import unittest
import uuid
import json

class TestV294SchemaAndConfig(unittest.TestCase):
    def setUp(self):
        self.conn = sqlite3.connect(":memory:")
        self.conn.isolation_level = None  # Autocommit mode for explicit transaction control
        self.cur = self.conn.cursor()
        self.cur.execute("PRAGMA foreign_keys = ON;")

    def tearDown(self):
        self.conn.close()

    def test_finding1_audit_log_migration_transformation(self):
        """Finding 1: Verify exact 12-step transformation from existing Sprint 1 audit_log to target Sprint 2 schema."""
        # 1. Setup exact Sprint 1 schema
        self.cur.execute("""
        CREATE TABLE companies (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );
        """)
        self.cur.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp');")

        # Actual existing Sprint 1 audit_log DDL
        self.cur.execute("""
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
        """)
        self.cur.execute("CREATE INDEX idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);")
        self.cur.execute("CREATE INDEX idx_audit_log_timestamp ON audit_log(company_id, timestamp);")
        self.cur.execute("""
        CREATE TRIGGER trg_prevent_audit_log_mutation
        BEFORE UPDATE ON audit_log
        FOR EACH ROW BEGIN
            SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated');
        END;
        """)
        self.cur.execute("""
        CREATE TRIGGER trg_prevent_audit_log_deletion
        BEFORE DELETE ON audit_log
        FOR EACH ROW
        WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
        BEGIN
            SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted');
        END;
        """)

        # Insert existing Sprint 1 records
        self.cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
        VALUES ('audit_s1_1', 'comp1', 'opening_balance', 'ob_1', 'CREATE', NULL, '{"amount":"1000.00"}', 'admin_user');
        """)
        self.cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
        VALUES ('audit_s1_2', 'comp1', 'opening_balance', 'ob_1', 'UPDATE', '{"amount":"1000.00"}', '{"amount":"2000.00"}', 'admin_user');
        """)

        # 2. Execute exact migration transformation (12-step SQLite pattern)
        # Outside transaction:
        self.cur.execute("PRAGMA foreign_keys = OFF;")
        # Inside transaction:
        self.cur.execute("BEGIN IMMEDIATE;")

        # Target Sprint 2 audit_log temporary table DDL (expanded actions and ON DELETE RESTRICT)
        self.cur.execute("""
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

        # Copy 100% of existing records
        self.cur.execute("""
        INSERT INTO audit_log_dg_tmp (
            id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
        )
        SELECT id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
        FROM audit_log;
        """)

        self.cur.execute("DROP TABLE audit_log;")
        self.cur.execute("ALTER TABLE audit_log_dg_tmp RENAME TO audit_log;")

        # Recreate indexes with canonical Sprint 1 names
        self.cur.execute("CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);")
        self.cur.execute("CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(company_id, timestamp);")

        # Recreate immutable triggers
        self.cur.execute("""
        CREATE TRIGGER trg_prevent_audit_log_mutation
        BEFORE UPDATE ON audit_log
        FOR EACH ROW BEGIN
            SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated');
        END;
        """)
        self.cur.execute("""
        CREATE TRIGGER trg_prevent_audit_log_deletion
        BEFORE DELETE ON audit_log
        FOR EACH ROW
        WHEN EXISTS (SELECT 1 FROM companies WHERE id = OLD.company_id)
        BEGIN
            SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted');
        END;
        """)

        # Verify zero FK violations
        self.cur.execute("PRAGMA foreign_key_check;")
        self.assertEqual(len(self.cur.fetchall()), 0)

        self.cur.execute("COMMIT;")
        # Outside transaction: restore foreign keys
        self.cur.execute("PRAGMA foreign_keys = ON;")

        # 3. Verify Target Schema Capabilities
        # A. Existing records preserved
        self.cur.execute("SELECT id, action FROM audit_log ORDER BY id")
        rows = self.cur.fetchall()
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0], ('audit_s1_1', 'CREATE'))
        self.assertEqual(rows[1], ('audit_s1_2', 'UPDATE'))

        # B. New Sprint 2 actions permitted
        self.cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
        VALUES ('audit_s2_1', 'comp1', 'voucher', 'v_1', 'POST', NULL, '{"status":"posted"}', 'user1');
        """)
        self.cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
        VALUES ('audit_s2_2', 'comp1', 'voucher', 'v_1', 'CANCEL', '{"status":"posted"}', '{"status":"cancelled"}', 'user1');
        """)
        self.cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
        VALUES ('audit_s2_3', 'comp1', 'voucher', 'v_1', 'REVERSE', '{"status":"posted"}', '{"status":"reversed"}', 'user1');
        """)

        # C. Immutability trigger active
        with self.assertRaises(sqlite3.IntegrityError):
            self.cur.execute("UPDATE audit_log SET performed_by = 'hacker' WHERE id = 'audit_s2_1';")

        # D. Company deletion rejected by ON DELETE RESTRICT (audit history preserved)
        with self.assertRaises(sqlite3.IntegrityError):
            self.cur.execute("DELETE FROM companies WHERE id = 'comp1';")

    def test_finding2_immutable_series_restart_period(self):
        """Finding 2: restart_period is an immutable property of voucher_number_series."""
        self.cur.execute("CREATE TABLE companies (id TEXT PRIMARY KEY, name TEXT);")
        self.cur.execute("INSERT INTO companies VALUES ('comp1', 'Acme');")
        self.cur.execute("CREATE TABLE voucher_types (id TEXT NOT NULL, company_id TEXT NOT NULL, name TEXT, PRIMARY KEY (company_id, id));")
        self.cur.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales');")

        # Table voucher_number_series with immutable restart_period
        self.cur.execute("""
        CREATE TABLE voucher_number_series (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_type_id TEXT NOT NULL,
            series_name TEXT NOT NULL,
            series_code TEXT NOT NULL,
            numbering_method TEXT NOT NULL DEFAULT 'automatic',
            restart_period TEXT NOT NULL DEFAULT 'yearly' CHECK (restart_period IN ('never', 'yearly', 'monthly', 'daily')),
            is_active INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (company_id, id)
        );
        """)

        self.cur.execute("""
        CREATE TRIGGER trg_prevent_series_restart_period_mutation
        BEFORE UPDATE OF restart_period ON voucher_number_series
        FOR EACH ROW
        WHEN NEW.restart_period IS NOT OLD.restart_period
        BEGIN
            SELECT RAISE(ABORT, 'Series restart_period is permanently immutable; changing reset frequency requires creating a new numbering series');
        END;
        """)

        # Create series
        self.cur.execute("INSERT INTO voucher_number_series VALUES ('ser_1', 'comp1', 'vt_sales', 'Primary', 'SALES', 'automatic', 'yearly', 1);")

        # Attempt to mutate restart_period directly
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            self.cur.execute("UPDATE voucher_number_series SET restart_period = 'monthly' WHERE id = 'ser_1';")
        self.assertIn("Series restart_period is permanently immutable", str(ctx.exception))

    def test_finding3_reversal_voucher_type_and_retention(self):
        """Finding 3: Reversals use dedicated Reversing Journal type, separate series, and retain original type."""
        self.cur.execute("CREATE TABLE companies (id TEXT PRIMARY KEY, name TEXT);")
        self.cur.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp');")

        self.cur.execute("""
        CREATE TABLE voucher_types (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL,
            type_code TEXT NOT NULL,
            is_predefined INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (company_id, id)
        );
        """)
        # Seed predefined types
        self.cur.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales Invoice', 'SALES', 1);")
        self.cur.execute("INSERT INTO voucher_types VALUES ('vt_pur', 'comp1', 'Purchase Invoice', 'PUR', 1);")
        self.cur.execute("INSERT INTO voucher_types VALUES ('vt_jnl', 'comp1', 'Journal', 'JNL', 1);")
        self.cur.execute("INSERT INTO voucher_types VALUES ('vt_revjnl', 'comp1', 'Reversing Journal', 'REVJNL', 1);")

        self.cur.execute("""
        CREATE TABLE voucher_number_series (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_type_id TEXT NOT NULL,
            series_name TEXT NOT NULL,
            series_code TEXT NOT NULL,
            PRIMARY KEY (company_id, id),
            FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id)
        );
        """)
        self.cur.execute("INSERT INTO voucher_number_series VALUES ('ser_sales', 'comp1', 'vt_sales', 'Primary', 'SALES');")
        self.cur.execute("INSERT INTO voucher_number_series VALUES ('ser_rev', 'comp1', 'vt_revjnl', 'Primary', 'REV');")

        self.cur.execute("""
        CREATE TABLE vouchers (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_type_id TEXT NOT NULL,
            series_id TEXT NOT NULL,
            voucher_number TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
            reversal_of_voucher_id TEXT,
            PRIMARY KEY (company_id, id),
            FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id),
            FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id)
        );
        """)

        # Trigger preventing modification of voucher_type_id on posted/reversed vouchers
        self.cur.execute("""
        CREATE TRIGGER trg_prevent_posted_voucher_type_mutation
        BEFORE UPDATE ON vouchers
        FOR EACH ROW
        WHEN OLD.status IN ('posted', 'reversed') AND (NEW.voucher_type_id IS NOT OLD.voucher_type_id)
        BEGIN
            SELECT RAISE(ABORT, 'voucher_type_id is permanently immutable on permanent vouchers');
        END;
        """)

        # 1. Post original Sales Invoice
        v_sales = "v_sales_42"
        self.cur.execute("INSERT INTO vouchers VALUES (?, 'comp1', 'vt_sales', 'ser_sales', 'SALES-0042', 'posted', NULL);", (v_sales,))

        # 2. Reversal is posted as Reversing Journal
        v_rev = "v_rev_1"
        self.cur.execute("INSERT INTO vouchers VALUES (?, 'comp1', 'vt_revjnl', 'ser_rev', 'REV-0001', 'posted', ?);", (v_rev, v_sales))
        # Original transitions to reversed
        self.cur.execute("UPDATE vouchers SET status = 'reversed' WHERE id = ?;", (v_sales,))

        # 3. Assert original voucher type was NOT modified
        self.cur.execute("SELECT voucher_type_id, status FROM vouchers WHERE id = ?;", (v_sales,))
        orig_row = self.cur.fetchone()
        self.assertEqual(orig_row[0], 'vt_sales')  # STILL SALES!
        self.assertEqual(orig_row[1], 'reversed')

        # 4. Assert reversal voucher is Reversing Journal referencing original
        self.cur.execute("SELECT voucher_type_id, reversal_of_voucher_id FROM vouchers WHERE id = ?;", (v_rev,))
        rev_row = self.cur.fetchone()
        self.assertEqual(rev_row[0], 'vt_revjnl')  # Reversing Journal!
        self.assertEqual(rev_row[1], v_sales)

        # 5. Attempting to mutate original voucher type is aborted
        with self.assertRaises(sqlite3.IntegrityError):
            self.cur.execute("UPDATE vouchers SET voucher_type_id = 'vt_revjnl' WHERE id = ?;", (v_sales,))

if __name__ == '__main__':
    unittest.main()

