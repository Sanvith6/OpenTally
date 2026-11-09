import sqlite3
import unittest
import json

class TestV295SignoffCorrections(unittest.TestCase):
    def setUp(self):
        self.conn = sqlite3.connect(":memory:")
        self.conn.execute("PRAGMA foreign_keys = ON;")
        
        # Setup base companies table
        self.conn.execute("""
            CREATE TABLE companies (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                financial_year_begins TEXT NOT NULL DEFAULT '2026-04-01',
                books_beginning_from TEXT NOT NULL DEFAULT '2026-04-01',
                currency_decimal_places INTEGER NOT NULL DEFAULT 2,
                is_archived INTEGER NOT NULL DEFAULT 0
            );
        """)
        self.conn.execute("""
            INSERT INTO companies (id, name, financial_year_begins, books_beginning_from)
            VALUES ('comp1', 'Acme Corp', '2026-04-01', '2026-04-01');
        """)
        self.conn.commit()

    def tearDown(self):
        self.conn.close()

    def test_explicit_restart_period_no_default(self):
        """Test Finding 2: voucher_number_series requires explicit restart_period without default."""
        self.conn.execute("""
            CREATE TABLE voucher_types (
                id TEXT NOT NULL,
                company_id TEXT NOT NULL,
                name TEXT NOT NULL,
                PRIMARY KEY (company_id, id),
                FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
            );
        """)
        self.conn.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales');")
        
        # New DDL: restart_period TEXT NOT NULL CHECK (...) WITHOUT DEFAULT
        self.conn.execute("""
            CREATE TABLE voucher_number_series (
                id TEXT NOT NULL,
                company_id TEXT NOT NULL,
                voucher_type_id TEXT NOT NULL,
                series_name TEXT NOT NULL,
                series_code TEXT NOT NULL,
                numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
                restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
                is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
                PRIMARY KEY (company_id, id),
                FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
            );
        """)

        # Inserting without restart_period must FAIL
        with self.assertRaises(sqlite3.IntegrityError):
            self.conn.execute("""
                INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code)
                VALUES ('s1', 'comp1', 'vt_sales', 'Primary', 'PRI');
            """)

        # Inserting with explicit restart_period must SUCCEED
        self.conn.execute("""
            INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, restart_period)
            VALUES ('s1', 'comp1', 'vt_sales', 'Primary', 'PRI', 'yearly');
        """)
        
        row = self.conn.execute("SELECT restart_period FROM voucher_number_series WHERE id = 's1'").fetchone()
        self.assertEqual(row[0], 'yearly')

    def test_complete_12_step_migration_with_row_count_check(self):
        """Test Finding 1: Complete 12-step migration with source and target row count verification."""
        # 1. Existing Sprint 1 audit_log DDL
        self.conn.execute("""
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
        # Seed 3 existing Sprint 1 records
        self.conn.execute("""
            INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by)
            VALUES ('a1', 'comp1', 'company', 'comp1', 'CREATE', 'system'),
                   ('a2', 'comp1', 'ledger', 'led1', 'CREATE', 'user1'),
                   ('a3', 'comp1', 'opening_balance', 'ob1', 'UPDATE', 'user1');
        """)
        self.conn.commit()

        # Step 1: Pinned connection already active
        # Step 2: Verify current records count
        src_count = self.conn.execute("SELECT COUNT(*) FROM audit_log;").fetchone()[0]
        self.assertEqual(src_count, 3)

        # Step 3: Disable foreign keys outside transaction
        self.conn.execute("PRAGMA foreign_keys = OFF;")
        fk_stat = self.conn.execute("PRAGMA foreign_keys;").fetchone()[0]
        self.assertEqual(fk_stat, 0)

        # Step 4: Begin immediate transaction
        self.conn.execute("BEGIN IMMEDIATE;")

        # Step 5: Create temporary table with Target Sprint 2 DDL
        self.conn.execute("""
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

        # Step 6: Copy existing audit records
        self.conn.execute("""
            INSERT INTO audit_log_dg_tmp (
                id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
            )
            SELECT id, company_id, entity_type, entity_id, ledger_id, financial_year_begins, action, old_values, new_values, performed_by, timestamp
            FROM audit_log;
        """)

        # Step 7: Verify source and destination record counts
        dst_count = self.conn.execute("SELECT COUNT(*) FROM audit_log_dg_tmp;").fetchone()[0]
        self.assertEqual(src_count, dst_count)

        # Step 8: Drop old table and rename temporary table
        self.conn.execute("DROP TABLE audit_log;")
        self.conn.execute("ALTER TABLE audit_log_dg_tmp RENAME TO audit_log;")

        # Step 9: Recreate canonical indexes and triggers
        self.conn.execute("CREATE INDEX idx_audit_log_entity ON audit_log(company_id, entity_type, entity_id);")
        self.conn.execute("CREATE INDEX idx_audit_log_timestamp ON audit_log(company_id, timestamp);")
        self.conn.execute("""
            CREATE TRIGGER trg_prevent_audit_log_mutation
            BEFORE UPDATE ON audit_log
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'Audit log entries are immutable and cannot be updated');
            END;
        """)
        self.conn.execute("""
            CREATE TRIGGER trg_prevent_audit_log_deletion
            BEFORE DELETE ON audit_log
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'Audit log entries are permanent and cannot be deleted');
            END;
        """)

        # Step 10: Run PRAGMA foreign_key_check
        violations = self.conn.execute("PRAGMA foreign_key_check;").fetchall()
        self.assertEqual(len(violations), 0)

        # Step 11: Commit transaction and restore foreign keys
        self.conn.execute("COMMIT;")
        self.conn.execute("PRAGMA foreign_keys = ON;")
        self.assertEqual(self.conn.execute("PRAGMA foreign_keys;").fetchone()[0], 1)

        # Step 12: Verify final schema and actions
        final_count = self.conn.execute("SELECT COUNT(*) FROM audit_log;").fetchone()[0]
        self.assertEqual(final_count, 3)

        # Verify POST, CANCEL, REVERSE are now permitted
        self.conn.execute("""
            INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, performed_by)
            VALUES ('a4', 'comp1', 'voucher', 'v1', 'POST', 'user1'),
                   ('a5', 'comp1', 'voucher', 'v2', 'CANCEL', 'user1'),
                   ('a6', 'comp1', 'voucher', 'v1', 'REVERSE', 'user1');
        """)
        self.assertEqual(self.conn.execute("SELECT COUNT(*) FROM audit_log;").fetchone()[0], 6)

        # Verify ON DELETE RESTRICT blocks dropping company
        with self.assertRaises(sqlite3.IntegrityError):
            self.conn.execute("DELETE FROM companies WHERE id = 'comp1';")

    def test_sprint2_single_reversal_series_scope(self):
        """Test Finding 3: Sprint 2 uses one dedicated REV series under vt_reversing_journal for all reversals."""
        self.conn.execute("""
            CREATE TABLE voucher_types (
                id TEXT NOT NULL,
                company_id TEXT NOT NULL,
                name TEXT NOT NULL,
                PRIMARY KEY (company_id, id)
            );
        """)
        self.conn.execute("""
            INSERT INTO voucher_types VALUES 
                ('vt_sales', 'comp1', 'Sales Invoice'),
                ('vt_pur', 'comp1', 'Purchase Invoice'),
                ('vt_jnl', 'comp1', 'Journal'),
                ('vt_reversing_journal', 'comp1', 'Reversing Journal');
        """)
        self.conn.execute("""
            CREATE TABLE voucher_number_series (
                id TEXT NOT NULL,
                company_id TEXT NOT NULL,
                voucher_type_id TEXT NOT NULL,
                series_name TEXT NOT NULL,
                series_code TEXT NOT NULL,
                restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
                PRIMARY KEY (company_id, id),
                UNIQUE (company_id, voucher_type_id, series_code),
                FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id)
            );
        """)
        # Single REV series under vt_reversing_journal
        self.conn.execute("""
            INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, restart_period)
            VALUES ('s_rev', 'comp1', 'vt_reversing_journal', 'Primary', 'REV', 'yearly');
        """)

        # Reversals across Sales, Purchase, and Journal all reference s_rev and vt_reversing_journal
        self.conn.execute("""
            CREATE TABLE vouchers (
                id TEXT NOT NULL,
                company_id TEXT NOT NULL,
                voucher_type_id TEXT NOT NULL,
                series_id TEXT NOT NULL,
                voucher_number TEXT NOT NULL,
                status TEXT NOT NULL,
                reversal_of_voucher_id TEXT,
                PRIMARY KEY (company_id, id),
                FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id),
                FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id)
            );
        """)

        # Original Sales voucher
        self.conn.execute("""
            INSERT INTO vouchers VALUES ('v_sales_1', 'comp1', 'vt_sales', 's_rev', 'SALES/26-27/0001', 'reversed', NULL);
        """)
        # Reversal voucher for Sales
        self.conn.execute("""
            INSERT INTO vouchers VALUES ('v_rev_1', 'comp1', 'vt_reversing_journal', 's_rev', 'REV/26-27/0001', 'posted', 'v_sales_1');
        """)
        # Reversal voucher for Purchase
        self.conn.execute("""
            INSERT INTO vouchers VALUES ('v_pur_1', 'comp1', 'vt_pur', 's_rev', 'PUR/26-27/0001', 'reversed', NULL);
        """)
        self.conn.execute("""
            INSERT INTO vouchers VALUES ('v_rev_2', 'comp1', 'vt_reversing_journal', 's_rev', 'REV/26-27/0002', 'posted', 'v_pur_1');
        """)

        # Verify all reversals are under vt_reversing_journal and s_rev
        reversals = self.conn.execute("SELECT id, voucher_type_id, series_id, voucher_number, reversal_of_voucher_id FROM vouchers WHERE reversal_of_voucher_id IS NOT NULL;").fetchall()
        self.assertEqual(len(reversals), 2)
        for r in reversals:
            self.assertEqual(r[1], 'vt_reversing_journal')
            self.assertEqual(r[2], 's_rev')

if __name__ == '__main__':
    unittest.main()

