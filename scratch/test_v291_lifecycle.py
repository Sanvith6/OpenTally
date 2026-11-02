import sqlite3
import unittest

class TestVoucherLifecycleV291(unittest.TestCase):
    def setUp(self):
        self.conn = sqlite3.connect(":memory:")
        self.conn.execute("PRAGMA foreign_keys = ON;")
        self._init_schema()

    def _init_schema(self):
        cur = self.conn.cursor()
        cur.executescript("""
        CREATE TABLE companies (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE voucher_types (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY (company_id, id),
            CONSTRAINT uq_voucher_types_company UNIQUE (company_id, id)
        );

        CREATE TABLE voucher_number_series (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_type_id TEXT NOT NULL,
            series_name TEXT NOT NULL,
            series_code TEXT NOT NULL,
            numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
            PRIMARY KEY (company_id, id),
            CONSTRAINT uq_series_company_id UNIQUE (company_id, id),
            CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id),
            CONSTRAINT uq_series_full_ownership UNIQUE (company_id, voucher_type_id, id, numbering_method),
            FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT
        );

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
            total_amount TEXT,
            reversal_of_voucher_id TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (company_id, id),
            FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
            FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
            CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id)
        );

        CREATE UNIQUE INDEX uq_vouchers_permanent_number ON vouchers(company_id, voucher_type_id, voucher_number) WHERE status != 'draft';
        CREATE UNIQUE INDEX uq_vouchers_series_sequence ON vouchers(company_id, series_id, period_start, sequence_number) WHERE status != 'draft' AND sequence_number IS NOT NULL;
        CREATE UNIQUE INDEX uq_vouchers_reversal ON vouchers(company_id, reversal_of_voucher_id) WHERE reversal_of_voucher_id IS NOT NULL;

        CREATE TABLE voucher_lines (
            id TEXT NOT NULL,
            company_id TEXT NOT NULL,
            voucher_id TEXT NOT NULL,
            line_number INTEGER NOT NULL CHECK (line_number > 0),
            ledger_id TEXT NOT NULL,
            entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
            amount TEXT NOT NULL,
            narration TEXT,
            PRIMARY KEY (company_id, id),
            UNIQUE (company_id, voucher_id, line_number),
            FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE
        );

        CREATE TABLE audit_log (
            id TEXT PRIMARY KEY,
            company_id TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            action TEXT NOT NULL CHECK (action IN ('CREATE', 'UPDATE', 'DELETE', 'POST', 'CANCEL', 'REVERSE')),
            details TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        -- Trigger 1: Header Immutability & Reversal Freeze
        CREATE TRIGGER trg_prevent_posted_voucher_mutation
        BEFORE UPDATE ON vouchers
        FOR EACH ROW
        WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
        BEGIN
            SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition. Cancelled and reversed vouchers are permanently frozen.')
            WHERE (OLD.status = 'cancelled' OR OLD.status = 'reversed');

            SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition from posted status. Posted vouchers can only transition to CANCELLED or REVERSED.')
            WHERE OLD.status = 'posted' AND NEW.status NOT IN ('posted', 'cancelled', 'reversed');

            SELECT RAISE(ABORT, 'Core financial and numbering fields of a posted voucher are strictly immutable.')
            WHERE NEW.company_id != OLD.company_id
               OR NEW.voucher_type_id != OLD.voucher_type_id
               OR NEW.series_id != OLD.series_id
               OR NEW.numbering_method != OLD.numbering_method
               OR (NEW.period_start IS NOT OLD.period_start)
               OR (NEW.sequence_number IS NOT OLD.sequence_number)
               OR NEW.voucher_number != OLD.voucher_number
               OR NEW.date != OLD.date
               OR NEW.effective_date != OLD.effective_date
               OR NEW.total_amount != OLD.total_amount
               OR NEW.reversal_of_voucher_id IS NOT OLD.reversal_of_voucher_id;

            SELECT RAISE(ABORT, 'Cannot cancel a reversing voucher.')
            WHERE OLD.reversal_of_voucher_id IS NOT NULL AND NEW.status = 'cancelled';
        END;

        -- Trigger 2: Illegal Draft Transitions
        CREATE TRIGGER trg_prevent_illegal_draft_transitions
        BEFORE UPDATE ON vouchers
        FOR EACH ROW
        WHEN OLD.status = 'draft'
        BEGIN
            SELECT RAISE(ABORT, 'Draft vouchers cannot transition directly to cancelled or reversed status.')
            WHERE NEW.status NOT IN ('draft', 'posted');
        END;

        -- Trigger 3: Prohibit Deletion of Non-Draft Vouchers
        CREATE TRIGGER trg_prevent_posted_voucher_deletion
        BEFORE DELETE ON vouchers
        FOR EACH ROW
        WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
        BEGIN
            SELECT RAISE(ABORT, 'Hard deletion of financial vouchers is strictly prohibited.');
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

        -- Trigger 5: Prohibit Mutating Lines of Non-Draft Vouchers
        CREATE TRIGGER trg_prevent_posted_voucher_lines_mutation
        BEFORE UPDATE ON voucher_lines
        FOR EACH ROW
        WHEN EXISTS (
            SELECT 1 FROM vouchers 
            WHERE company_id = OLD.company_id 
              AND id = OLD.voucher_id 
              AND status IN ('posted', 'cancelled', 'reversed')
        )
        BEGIN
            SELECT RAISE(ABORT, 'Voucher line items belonging to a posted, cancelled, or reversed voucher cannot be modified.');
        END;

        -- Trigger 6: Prohibit Deleting Lines of Non-Draft Vouchers
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

        # Seed company, voucher type, series
        cur.execute("INSERT INTO companies VALUES ('comp1', 'Test Company')")
        cur.execute("INSERT INTO voucher_types VALUES ('type_sales', 'comp1', 'Sales')")
        cur.execute("INSERT INTO voucher_types VALUES ('type_journal', 'comp1', 'Journal')")
        cur.execute("INSERT INTO voucher_number_series VALUES ('ser_sales', 'comp1', 'type_sales', 'Sales Series', 'SLS', 'automatic')")
        cur.execute("INSERT INTO voucher_number_series VALUES ('ser_jnl', 'comp1', 'type_journal', 'Journal Series', 'JNL', 'automatic')")
        self.conn.commit()

    def test_line_item_protection_after_posting(self):
        cur = self.conn.cursor()
        # Create draft voucher with 2 lines
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v1', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-1', '2026-05-10', '2026-05-10', 'draft', '100.00')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l1', 'comp1', 'v1', 1, 'led_cash', 'Dr', '100.00', 'Sale')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l2', 'comp1', 'v1', 2, 'led_sales', 'Cr', '100.00', 'Sale')")
        
        # Post voucher
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-001', sequence_number = 1, period_start = '2026-04-01' WHERE id = 'v1'")
        self.conn.commit()

        # 1. Attempt to insert line into posted voucher -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("INSERT INTO voucher_lines VALUES ('l3', 'comp1', 'v1', 3, 'led_tax', 'Cr', '18.00', 'Tax')")
        self.assertIn("Cannot insert line items into a posted, cancelled, or reversed voucher", str(ctx.exception))

        # 2. Attempt to update line in posted voucher -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE voucher_lines SET amount = '200.00' WHERE id = 'l1'")
        self.assertIn("Voucher line items belonging to a posted, cancelled, or reversed voucher cannot be modified", str(ctx.exception))

        # 3. Attempt to delete line from posted voucher -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("DELETE FROM voucher_lines WHERE id = 'l1'")
        self.assertIn("Voucher line items belonging to a posted, cancelled, or reversed voucher cannot be deleted", str(ctx.exception))

    def test_terminal_state_immutability_and_draft_reversion(self):
        cur = self.conn.cursor()
        # Post a voucher
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v2', 'comp1', 'type_sales', 'ser_sales', 'SLS-002', '2026-05-10', '2026-05-10', 'posted', '500.00', 2, '2026-04-01')")
        self.conn.commit()

        # Attempt to revert posted to draft -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET status = 'draft' WHERE id = 'v2'")
        self.assertIn("Illegal voucher lifecycle transition from posted status", str(ctx.exception))

        # Cancel voucher
        cur.execute("UPDATE vouchers SET status = 'cancelled' WHERE id = 'v2'")
        self.conn.commit()

        # Attempt to mutate cancelled voucher (e.g. status or date) -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET status = 'posted' WHERE id = 'v2'")
        self.assertIn("Cancelled and reversed vouchers are permanently frozen", str(ctx.exception))

        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET date = '2026-05-11' WHERE id = 'v2'")
        self.assertIn("Cancelled and reversed vouchers are permanently frozen", str(ctx.exception))

    def test_duplicate_reversal_and_self_reversal_prevention(self):
        cur = self.conn.cursor()
        # Post original voucher
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_orig', 'comp1', 'type_sales', 'ser_sales', 'SLS-010', '2026-05-10', '2026-05-10', 'posted', '1000.00', 10, '2026-04-01')")
        self.conn.commit()

        # 1. Attempt self-reversal -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start, reversal_of_voucher_id) VALUES ('v_self', 'comp1', 'type_journal', 'ser_jnl', 'JNL-001', '2026-05-11', '2026-05-11', 'posted', '1000.00', 1, '2026-04-01', 'v_self')")
        self.assertIn("CHECK constraint failed", str(ctx.exception))

        # 2. First reversal -> success
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start, reversal_of_voucher_id) VALUES ('v_rev1', 'comp1', 'type_journal', 'ser_jnl', 'JNL-001', '2026-05-11', '2026-05-11', 'posted', '1000.00', 1, '2026-04-01', 'v_orig')")
        cur.execute("UPDATE vouchers SET status = 'reversed' WHERE id = 'v_orig'")
        self.conn.commit()

        # 3. Duplicate reversal of v_orig -> must abort via uq_vouchers_reversal
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start, reversal_of_voucher_id) VALUES ('v_rev2', 'comp1', 'type_journal', 'ser_jnl', 'JNL-002', '2026-05-12', '2026-05-12', 'posted', '1000.00', 2, '2026-04-01', 'v_orig')")
        self.assertIn("UNIQUE constraint failed", str(ctx.exception))

    def test_reversing_voucher_cannot_be_cancelled(self):
        cur = self.conn.cursor()
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_orig2', 'comp1', 'type_sales', 'ser_sales', 'SLS-020', '2026-05-10', '2026-05-10', 'reversed', '1000.00', 20, '2026-04-01')")
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start, reversal_of_voucher_id) VALUES ('v_rev_entry', 'comp1', 'type_journal', 'ser_jnl', 'JNL-020', '2026-05-11', '2026-05-11', 'posted', '1000.00', 20, '2026-04-01', 'v_orig2')")
        self.conn.commit()

        # Attempt to cancel reversing voucher -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET status = 'cancelled' WHERE id = 'v_rev_entry'")
        self.assertIn("Cannot cancel a reversing voucher", str(ctx.exception))

    def test_atomic_reversal_transaction_with_audit_logging(self):
        cur = self.conn.cursor()
        # Post original voucher: insert as draft, insert lines, then promote to posted
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_orig3', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-30', '2026-05-10', '2026-05-10', 'draft', '1500.00', NULL, NULL)")
        cur.execute("INSERT INTO voucher_lines VALUES ('l30_1', 'comp1', 'v_orig3', 1, 'led_debtor', 'Dr', '1500.00', 'Original line 1')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l30_2', 'comp1', 'v_orig3', 2, 'led_sales', 'Cr', '1500.00', 'Original line 2')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-030', sequence_number = 30, period_start = '2026-04-01' WHERE id = 'v_orig3'")
        self.conn.commit()

        # Atomic Reversal Workflow
        try:
            self.conn.execute("BEGIN IMMEDIATE;")
            # 1. Verify original is posted
            orig = cur.execute("SELECT status, total_amount FROM vouchers WHERE id = 'v_orig3'").fetchone()
            self.assertEqual(orig[0], 'posted')

            # 2. Insert reversing voucher as draft
            cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start, reversal_of_voucher_id) VALUES ('v_rev3', 'comp1', 'type_journal', 'ser_jnl', 'DRAFT-REV', '2026-05-12', '2026-05-12', 'draft', '1500.00', NULL, NULL, 'v_orig3')")
            
            # 3. Insert inverted lines
            cur.execute("INSERT INTO voucher_lines VALUES ('l_rev1', 'comp1', 'v_rev3', 1, 'led_debtor', 'Cr', '1500.00', 'Reversal leg 1')")
            cur.execute("INSERT INTO voucher_lines VALUES ('l_rev2', 'comp1', 'v_rev3', 2, 'led_sales', 'Dr', '1500.00', 'Reversal leg 2')")

            # 4. Promote reversing voucher to posted
            cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'JNL-030', sequence_number = 30, period_start = '2026-04-01' WHERE id = 'v_rev3'")

            # 5. Transition original to reversed
            cur.execute("UPDATE vouchers SET status = 'reversed' WHERE id = 'v_orig3'")

            # 6. Write audit records in same transaction
            cur.execute("INSERT INTO audit_log VALUES ('a1', 'comp1', 'voucher', 'v_orig3', 'REVERSE', 'Voucher reversed by v_rev3', CURRENT_TIMESTAMP)")
            cur.execute("INSERT INTO audit_log VALUES ('a2', 'comp1', 'voucher', 'v_rev3', 'POST', 'Reversal voucher posted', CURRENT_TIMESTAMP)")

            self.conn.execute("COMMIT;")
        except Exception:
            self.conn.execute("ROLLBACK;")
            raise

        # Verify state
        orig_after = cur.execute("SELECT status FROM vouchers WHERE id = 'v_orig3'").fetchone()
        self.assertEqual(orig_after[0], 'reversed')
        rev_after = cur.execute("SELECT status, reversal_of_voucher_id FROM vouchers WHERE id = 'v_rev3'").fetchone()
        self.assertEqual(rev_after[0], 'posted')
        self.assertEqual(rev_after[1], 'v_orig3')

        # Audit logs present
        audits = cur.execute("SELECT action FROM audit_log WHERE entity_id IN ('v_orig3', 'v_rev3') ORDER BY created_at").fetchall()
        self.assertEqual(len(audits), 2)
        self.assertEqual(audits[0][0], 'REVERSE')
        self.assertEqual(audits[1][0], 'POST')

    def test_cancelled_and_reversed_included_in_sequence_recovery_evidence(self):
        cur = self.conn.cursor()
        # Insert voucher 1 (posted), voucher 2 (cancelled), voucher 3 (reversed)
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_seq1', 'comp1', 'type_sales', 'ser_sales', 'SLS-001', '2026-05-10', '2026-05-10', 'posted', '100.00', 1, '2026-04-01')")
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_seq2', 'comp1', 'type_sales', 'ser_sales', 'SLS-002', '2026-05-11', '2026-05-11', 'cancelled', '200.00', 2, '2026-04-01')")
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, sequence_number, period_start) VALUES ('v_seq3', 'comp1', 'type_sales', 'ser_sales', 'SLS-003', '2026-05-12', '2026-05-12', 'reversed', '300.00', 3, '2026-04-01')")
        self.conn.commit()

        # Recovery query: COALESCE(MAX(sequence_number), 0) over ('posted', 'cancelled', 'reversed')
        row = cur.execute("""
            SELECT COALESCE(MAX(sequence_number), 0)
            FROM vouchers
            WHERE company_id = 'comp1'
              AND series_id = 'ser_sales'
              AND period_start = '2026-04-01'
              AND numbering_method = 'automatic'
              AND status IN ('posted', 'cancelled', 'reversed')
        """).fetchone()

        max_alloc = row[0]
        self.assertEqual(max_alloc, 3, "M must include cancelled (2) and reversed (3) vouchers")
        expected_next = max_alloc + 1
        self.assertEqual(expected_next, 4)


if __name__ == "__main__":
    unittest.main()
