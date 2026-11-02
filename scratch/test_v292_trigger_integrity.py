import sqlite3
import unittest

class TestVoucherTriggerIntegrityV292(unittest.TestCase):
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

        -- Trigger 1: Header Immutability, Terminal Freeze & Reversal Coupling
        CREATE TRIGGER trg_prevent_posted_voucher_mutation
        BEFORE UPDATE ON vouchers
        FOR EACH ROW
        WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
        BEGIN
            -- 1. Disallow any mutation on terminal states (cancelled and reversed)
            SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition. Cancelled and reversed vouchers are permanently frozen.')
            WHERE (OLD.status = 'cancelled' OR OLD.status = 'reversed');

            -- 2. On posted vouchers, only allow transition to cancelled or reversed (prohibit draft reversion)
            SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition from posted status. Posted vouchers can only transition to CANCELLED or REVERSED.')
            WHERE OLD.status = 'posted' AND NEW.status NOT IN ('posted', 'cancelled', 'reversed');

            -- 3. Core financial, numbering, reference, and series fields are strictly immutable across all non-draft statuses (NULL-safe comparisons)
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

            -- 4. Reversing vouchers (reversal_of_voucher_id IS NOT NULL) can NEVER be cancelled
            SELECT RAISE(ABORT, 'Cannot cancel a reversing voucher. Reversing vouchers are permanently immutable to preserve ledger equilibrium.')
            WHERE OLD.reversal_of_voucher_id IS NOT NULL AND NEW.status = 'cancelled';

            -- 5. Bidirectional Reversal Coupling: Original cannot transition to REVERSED without an existing POSTED reversing voucher
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

        -- Trigger 3: Prohibit Hard Deletion of Non-Draft Vouchers
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

        -- Trigger 5: Prohibit Mutating or Reassigning Lines Belonging to or Assigned to Non-Draft Vouchers (Validates Both OLD and NEW)
        CREATE TRIGGER trg_prevent_posted_voucher_lines_mutation
        BEFORE UPDATE ON voucher_lines
        FOR EACH ROW
        WHEN (
            -- 1. Lines belonging to a non-draft voucher cannot be modified
            EXISTS (
                SELECT 1 FROM vouchers 
                WHERE company_id = OLD.company_id 
                  AND id = OLD.voucher_id 
                  AND status IN ('posted', 'cancelled', 'reversed')
            )
            -- 2. Lines cannot be reassigned into a non-draft voucher
            OR EXISTS (
                SELECT 1 FROM vouchers 
                WHERE company_id = NEW.company_id 
                  AND id = NEW.voucher_id 
                  AND status IN ('posted', 'cancelled', 'reversed')
            )
            -- 3. Lines cannot be reassigned across company boundaries
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
        """)

        # Seed company 1 and company 2
        cur.execute("INSERT INTO companies VALUES ('comp1', 'Company 1')")
        cur.execute("INSERT INTO companies VALUES ('comp2', 'Company 2')")
        cur.execute("INSERT INTO voucher_types VALUES ('type_sales', 'comp1', 'Sales')")
        cur.execute("INSERT INTO voucher_types VALUES ('type_jnl', 'comp1', 'Journal')")
        cur.execute("INSERT INTO voucher_types VALUES ('type_sales_c2', 'comp2', 'Sales')")
        cur.execute("INSERT INTO voucher_number_series VALUES ('ser_sales', 'comp1', 'type_sales', 'Sales', 'SLS', 'automatic')")
        cur.execute("INSERT INTO voucher_number_series VALUES ('ser_jnl', 'comp1', 'type_jnl', 'Journal', 'JNL', 'automatic')")
        cur.execute("INSERT INTO voucher_number_series VALUES ('ser_sales_c2', 'comp2', 'type_sales_c2', 'Sales C2', 'SL2', 'automatic')")
        self.conn.commit()

    def test_finding1_prevent_reassigning_draft_line_to_posted_voucher(self):
        cur = self.conn.cursor()
        # Create draft voucher v_draft with line l_draft
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_draft', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-1', '2026-05-10', '2026-05-10', 'draft', '500.00')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l_draft', 'comp1', 'v_draft', 1, 'led_cash', 'Dr', '500.00', 'Draft line')")

        # Create and post v_posted with line l_posted
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_posted', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-2', '2026-05-10', '2026-05-10', 'draft', '1000.00')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l_posted', 'comp1', 'v_posted', 1, 'led_cash', 'Dr', '1000.00', 'Posted line')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-001', sequence_number = 1, period_start = '2026-04-01' WHERE id = 'v_posted'")
        self.conn.commit()

        # Attempt EXPLOIT: Reassign l_draft to v_posted! (OLD parent is draft, but NEW parent is posted)
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE voucher_lines SET voucher_id = 'v_posted', line_number = 2 WHERE id = 'l_draft'")
        self.assertIn("cannot be modified or reassigned", str(ctx.exception))

    def test_finding1_prevent_moving_line_across_company_boundaries(self):
        cur = self.conn.cursor()
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_draft_c1', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-C1', '2026-05-10', '2026-05-10', 'draft', '500.00')")
        cur.execute("INSERT INTO voucher_lines VALUES ('l_draft_c1', 'comp1', 'v_draft_c1', 1, 'led_cash', 'Dr', '500.00', 'Draft line')")
        self.conn.commit()

        # Attempt to change company_id
        with self.assertRaises(sqlite3.IntegrityError):
            cur.execute("UPDATE voucher_lines SET company_id = 'comp2' WHERE id = 'l_draft_c1'")

    def test_finding2_reversal_cross_company_and_nonexistent(self):
        cur = self.conn.cursor()
        # Attempt reversal referencing nonexistent original voucher
        with self.assertRaises(sqlite3.IntegrityError):
            cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_rev_bad', 'comp1', 'type_jnl', 'ser_jnl', 'DRAFT-REV', '2026-05-11', '2026-05-11', 'draft', '100.00', 'nonexistent_id')")

        # Post original in comp1
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_c1', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-C1', '2026-05-10', '2026-05-10', 'draft', '100.00')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-010', sequence_number = 10, period_start = '2026-04-01' WHERE id = 'v_c1'")
        self.conn.commit()

        # Attempt reversal in comp2 referencing comp1 voucher -> rejected by composite foreign key
        with self.assertRaises(sqlite3.IntegrityError):
            cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_c2', 'comp2', 'type_sales_c2', 'ser_sales_c2', 'DRAFT-C2', '2026-05-11', '2026-05-11', 'draft', '100.00', 'v_c1')")

    def test_finding2_cannot_reverse_draft_or_cancelled_voucher(self):
        cur = self.conn.cursor()
        # Original is draft
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_orig_draft', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-ORIG', '2026-05-10', '2026-05-10', 'draft', '100.00')")
        # Reversal created as draft
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_rev_draft', 'comp1', 'type_jnl', 'ser_jnl', 'DRAFT-REV', '2026-05-11', '2026-05-11', 'draft', '100.00', 'v_orig_draft')")
        self.conn.commit()

        # Attempt to post reversal when original is still draft -> must abort
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'JNL-001', sequence_number = 1, period_start = '2026-04-01' WHERE id = 'v_rev_draft'")
        self.assertIn("Referenced original voucher must exist in the same company with status POSTED", str(ctx.exception))

    def test_finding2_cannot_mark_original_reversed_without_posted_reversal(self):
        cur = self.conn.cursor()
        # Post original
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_orig_post', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-P', '2026-05-10', '2026-05-10', 'draft', '100.00')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-020', sequence_number = 20, period_start = '2026-04-01' WHERE id = 'v_orig_post'")
        self.conn.commit()

        # Attempt to mark v_orig_post as 'reversed' without any reversing voucher existing
        with self.assertRaises(sqlite3.IntegrityError) as ctx:
            cur.execute("UPDATE vouchers SET status = 'reversed' WHERE id = 'v_orig_post'")
        self.assertIn("Cannot mark voucher as REVERSED without an existing POSTED reversing voucher referencing it", str(ctx.exception))

    def test_finding2_valid_atomic_reversal_workflow(self):
        cur = self.conn.cursor()
        # Post original
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount) VALUES ('v_orig_good', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-G', '2026-05-10', '2026-05-10', 'draft', '2000.00')")
        cur.execute("INSERT INTO voucher_lines VALUES ('lg_1', 'comp1', 'v_orig_good', 1, 'led_debtor', 'Dr', '2000.00', 'Original line 1')")
        cur.execute("INSERT INTO voucher_lines VALUES ('lg_2', 'comp1', 'v_orig_good', 2, 'led_sales', 'Cr', '2000.00', 'Original line 2')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-030', sequence_number = 30, period_start = '2026-04-01' WHERE id = 'v_orig_good'")
        self.conn.commit()

        # Atomic Reversal in BEGIN IMMEDIATE
        self.conn.execute("BEGIN IMMEDIATE;")
        # 1. Insert reversal header as draft
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, reversal_of_voucher_id) VALUES ('v_rev_good', 'comp1', 'type_jnl', 'ser_jnl', 'DRAFT-REV-G', '2026-05-12', '2026-05-12', 'draft', '2000.00', 'v_orig_good')")
        # 2. Insert inverted lines
        cur.execute("INSERT INTO voucher_lines VALUES ('lrg_1', 'comp1', 'v_rev_good', 1, 'led_debtor', 'Cr', '2000.00', 'Inverted leg 1')")
        cur.execute("INSERT INTO voucher_lines VALUES ('lrg_2', 'comp1', 'v_rev_good', 2, 'led_sales', 'Dr', '2000.00', 'Inverted leg 2')")
        # 3. Promote reversal to posted (checks original is posted)
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'JNL-030', sequence_number = 30, period_start = '2026-04-01' WHERE id = 'v_rev_good'")
        # 4. Mark original reversed (checks reversal is posted)
        cur.execute("UPDATE vouchers SET status = 'reversed' WHERE id = 'v_orig_good'")
        # 5. Audit logs
        cur.execute("INSERT INTO audit_log VALUES ('a_rev', 'comp1', 'voucher', 'v_orig_good', 'REVERSE', 'Voucher reversed', CURRENT_TIMESTAMP)")
        cur.execute("INSERT INTO audit_log VALUES ('a_post', 'comp1', 'voucher', 'v_rev_good', 'POST', 'Reversal posted', CURRENT_TIMESTAMP)")
        self.conn.execute("COMMIT;")

        # Check final states
        orig_status = cur.execute("SELECT status FROM vouchers WHERE id = 'v_orig_good'").fetchone()[0]
        rev_status = cur.execute("SELECT status FROM vouchers WHERE id = 'v_rev_good'").fetchone()[0]
        self.assertEqual(orig_status, 'reversed')
        self.assertEqual(rev_status, 'posted')

    def test_finding3_header_immutability_inventory(self):
        cur = self.conn.cursor()
        # Post voucher
        cur.execute("INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, effective_date, status, total_amount, reference_no, reference_date, narration) VALUES ('v_imm', 'comp1', 'type_sales', 'ser_sales', 'DRAFT-IMM', '2026-05-10', '2026-05-10', 'draft', '100.00', 'REF-01', '2026-05-09', 'Initial narration')")
        cur.execute("UPDATE vouchers SET status = 'posted', voucher_number = 'SLS-099', sequence_number = 99, period_start = '2026-04-01' WHERE id = 'v_imm'")
        self.conn.commit()

        # Test mutation of every protected field:
        protected_mutations = [
            ("company_id", "'comp2'"),
            ("voucher_type_id", "'type_jnl'"),
            ("series_id", "'ser_jnl'"),
            ("numbering_method", "'manual'"),
            ("period_start", "'2026-05-01'"),
            ("sequence_number", "100"),
            ("voucher_number", "'SLS-100'"),
            ("date", "'2026-05-11'"),
            ("effective_date", "'2026-05-11'"),
            ("reference_no", "'REF-CHANGED'"),
            ("reference_date", "'2026-05-10'"),
            ("narration", "'Mutated narration'"),
            ("total_amount", "'200.00'"),
            ("reversal_of_voucher_id", "'some_other'"),
            ("created_at", "'2025-01-01 00:00:00'")
        ]

        for field, bad_val in protected_mutations:
            with self.assertRaises(sqlite3.IntegrityError, msg=f"Field {field} should be immutable"):
                cur.execute(f"UPDATE vouchers SET {field} = {bad_val} WHERE id = 'v_imm'")

        # Updating system metadata updated_at is permitted:
        cur.execute("UPDATE vouchers SET updated_at = CURRENT_TIMESTAMP WHERE id = 'v_imm'")
        self.conn.commit()

if __name__ == "__main__":
    unittest.main()
