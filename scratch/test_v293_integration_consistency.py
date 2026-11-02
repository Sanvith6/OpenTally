import sqlite3
import unittest
import uuid
import json

def get_v293_db():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None  # Autocommit mode to allow explicit BEGIN IMMEDIATE
    cur = conn.cursor()
    cur.execute("PRAGMA foreign_keys = ON;")

    # 1. Base tables
    cur.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        financial_year_begins TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL
    );
    """)

    cur.execute("""
    CREATE TABLE account_groups (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );
    """)

    cur.execute("""
    CREATE TABLE ledgers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        group_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, group_id) REFERENCES account_groups(company_id, id) ON DELETE RESTRICT
    );
    """)

    # 2. Canonical Sprint 1 audit_log table with Sprint 2 expanded actions
    cur.execute("""
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

    # 3. Numbering tables
    cur.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );
    """)

    cur.execute("""
    CREATE TABLE voucher_number_series (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL COLLATE NOCASE,
        series_code TEXT NOT NULL COLLATE NOCASE,
        numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
        is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_series_company_id UNIQUE (company_id, id),
        CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name),
        CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code),
        CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
    );
    """)

    cur.execute("""
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
        restart_period TEXT NOT NULL DEFAULT 'yearly' CHECK (restart_period IN ('never', 'yearly', 'monthly', 'daily')),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_config_series_date UNIQUE (company_id, series_id, applicable_from),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id, series_id) REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT
    );
    """)

    cur.execute("""
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
        FOREIGN KEY (company_id, voucher_type_id, series_id) REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT
    );
    """)

    # 4. Vouchers header table
    cur.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
        period_start TEXT,
        sequence_number INTEGER CHECK (sequence_number IS NULL OR sequence_number > 0),
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        effective_date TEXT,
        status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        reference_no TEXT,
        reference_date TEXT,
        narration TEXT,
        total_amount TEXT,
        reversal_of_voucher_id TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_vouchers_company_id UNIQUE (company_id, id),
        CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id, series_id) REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT
    );
    """)

    cur.execute("""
    CREATE UNIQUE INDEX uq_vouchers_reversal 
    ON vouchers(company_id, reversal_of_voucher_id) 
    WHERE reversal_of_voucher_id IS NOT NULL;
    """)

    # 5. Voucher lines table
    cur.execute("""
    CREATE TABLE voucher_lines (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_id TEXT NOT NULL,
        line_number INTEGER NOT NULL CHECK (line_number > 0),
        ledger_id TEXT NOT NULL,
        entry_type TEXT NOT NULL CHECK (entry_type IN ('debit', 'credit')),
        amount TEXT NOT NULL,
        narration TEXT,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_voucher_line_order UNIQUE (company_id, voucher_id, line_number),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT
    );
    """)

    # 6. Triggers
    # Trigger 1: Immutability and State Transitions
    cur.execute("""
    CREATE TRIGGER trg_prevent_posted_voucher_mutation
    BEFORE UPDATE ON vouchers
    FOR EACH ROW
    WHEN (
        (OLD.status IN ('cancelled', 'reversed'))
        OR (OLD.status = 'posted' AND NEW.status = 'draft')
        OR (OLD.status = 'posted' AND NEW.status = 'cancelled' AND OLD.reversal_of_voucher_id IS NOT NULL)
        OR (OLD.status = 'posted' AND NEW.status = 'posted' AND (
            (NEW.id IS NOT OLD.id) OR
            (NEW.company_id IS NOT OLD.company_id) OR
            (NEW.voucher_type_id IS NOT OLD.voucher_type_id) OR
            (NEW.series_id IS NOT OLD.series_id) OR
            (NEW.numbering_method IS NOT OLD.numbering_method) OR
            (NEW.period_start IS NOT OLD.period_start) OR
            (NEW.sequence_number IS NOT OLD.sequence_number) OR
            (NEW.voucher_number IS NOT OLD.voucher_number) OR
            (NEW.date IS NOT OLD.date) OR
            (NEW.effective_date IS NOT OLD.effective_date) OR
            (NEW.reference_no IS NOT OLD.reference_no) OR
            (NEW.reference_date IS NOT OLD.reference_date) OR
            (NEW.narration IS NOT OLD.narration) OR
            (NEW.total_amount IS NOT OLD.total_amount) OR
            (NEW.reversal_of_voucher_id IS NOT OLD.reversal_of_voucher_id) OR
            (NEW.created_at IS NOT OLD.created_at)
        ))
        OR (OLD.status = 'posted' AND NEW.status = 'reversed' AND NOT EXISTS (
            SELECT 1 FROM vouchers rev
            WHERE rev.company_id = NEW.company_id
              AND rev.reversal_of_voucher_id = NEW.id
              AND rev.status = 'posted'
        ))
    )
    BEGIN
        SELECT RAISE(ABORT, 'Posted voucher cannot be modified; state transitions restricted');
    END;
    """)

    # Trigger 1b: Validate Reversal Target on Post
    cur.execute("""
    CREATE TRIGGER trg_validate_reversal_target_on_post
    BEFORE UPDATE OF status ON vouchers
    FOR EACH ROW
    WHEN NEW.status = 'posted' 
     AND OLD.status = 'draft' 
     AND NEW.reversal_of_voucher_id IS NOT NULL
    BEGIN
        SELECT RAISE(ABORT, 'Reversal voucher can only be posted against an existing posted voucher in the same company')
        WHERE NOT EXISTS (
            SELECT 1 FROM vouchers
            WHERE company_id = NEW.company_id
              AND id = NEW.reversal_of_voucher_id
              AND status = 'posted'
        );
    END;
    """)

    # Trigger 4: Prevent Line Insertion on Non-Draft Vouchers
    cur.execute("""
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
        SELECT RAISE(ABORT, 'Cannot insert line items into a posted, cancelled, or reversed voucher');
    END;
    """)

    # Trigger 5: Prevent Line Mutation and Reassignment
    cur.execute("""
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
        OR
        EXISTS (
            SELECT 1 FROM vouchers
            WHERE company_id = NEW.company_id
              AND id = NEW.voucher_id
              AND status IN ('posted', 'cancelled', 'reversed')
        )
        OR
        (NEW.company_id IS NOT OLD.company_id)
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot modify or reassign line items of posted, cancelled, or reversed vouchers');
    END;
    """)

    # Seed baseline setup
    cur.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp', '2026-04-01', '2026-04-01');")
    cur.execute("INSERT INTO account_groups VALUES ('grp_assets', 'comp1', 'Current Assets');")
    cur.execute("INSERT INTO account_groups VALUES ('grp_income', 'comp1', 'Sales Accounts');")
    cur.execute("INSERT INTO ledgers VALUES ('led_cash', 'comp1', 'grp_assets', 'Cash');")
    cur.execute("INSERT INTO ledgers VALUES ('led_sales', 'comp1', 'grp_income', 'Domestic Sales');")
    cur.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales Invoice');")
    cur.execute("INSERT INTO voucher_types VALUES ('vt_jnl', 'comp1', 'Journal');")
    cur.execute("INSERT INTO voucher_number_series VALUES ('ser_sales', 'comp1', 'vt_sales', 'Primary', 'SALES', 'automatic', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);")
    cur.execute("INSERT INTO voucher_number_series VALUES ('ser_jnl', 'comp1', 'vt_jnl', 'Primary', 'JNL', 'automatic', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);")
    cur.execute("INSERT INTO voucher_number_configs VALUES ('cfg_sales', 'comp1', 'vt_sales', 'ser_sales', '2026-04-01', 'SALES-', '', 1, 4, 'yearly', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);")
    cur.execute("INSERT INTO voucher_number_configs VALUES ('cfg_jnl', 'comp1', 'vt_jnl', 'ser_jnl', '2026-04-01', 'JNL-', '', 100, 4, 'yearly', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);")

    return conn

class TestV293IntegrationConsistency(unittest.TestCase):
    def setUp(self):
        self.conn = get_v293_db()
        self.cur = self.conn.cursor()

    def tearDown(self):
        self.conn.close()

    def test_finding1_canonical_audit_log_actions(self):
        """Finding 1: Verify canonical audit_log table with POST and REVERSE actions."""
        # Insert POST action
        post_id = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO audit_log (
            id, company_id, entity_type, entity_id, ledger_id,
            financial_year_begins, action, old_values, new_values, performed_by
        ) VALUES (?, 'comp1', 'voucher', 'v_test', NULL, '2026-04-01', 'POST', NULL, ?, 'admin_user')
        """, (post_id, json.dumps({"status": "posted", "voucher_number": "SALES-0001"})))

        # Insert REVERSE action
        rev_id = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO audit_log (
            id, company_id, entity_type, entity_id, ledger_id,
            financial_year_begins, action, old_values, new_values, performed_by
        ) VALUES (?, 'comp1', 'voucher', 'v_test', NULL, '2026-04-01', 'REVERSE', ?, ?, 'admin_user')
        """, (rev_id, json.dumps({"status": "posted"}), json.dumps({"status": "reversed"})))

        self.cur.execute("SELECT action, entity_id FROM audit_log WHERE company_id = 'comp1'")
        rows = self.cur.fetchall()
        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0][0], 'POST')
        self.assertEqual(rows[1][0], 'REVERSE')

    def test_finding2_period_anchored_config_resolution(self):
        """Finding 2: Period-based config resolution rejects missing configs and handles historical configs."""
        # Case A: Date in FY 2026-27 (2026-05-15) resolves to period_start 2026-04-01
        period_start = "2026-04-01"
        self.cur.execute("""
        SELECT id, prefix, suffix, width, start_number, restart_period, applicable_from
        FROM voucher_number_configs
        WHERE company_id = 'comp1' AND series_id = 'ser_sales' AND applicable_from <= ?
        ORDER BY applicable_from DESC LIMIT 1
        """, (period_start,))
        cfg = self.cur.fetchone()
        self.assertIsNotNone(cfg)
        self.assertEqual(cfg[0], 'cfg_sales')
        self.assertEqual(cfg[4], 1)

        # Case B: Prior period before 2026-04-01 (e.g. 2025-04-01) has no configuration
        prior_period = "2025-04-01"
        self.cur.execute("""
        SELECT id FROM voucher_number_configs
        WHERE company_id = 'comp1' AND series_id = 'ser_sales' AND applicable_from <= ?
        ORDER BY applicable_from DESC LIMIT 1
        """, (prior_period,))
        prior_cfg = self.cur.fetchone()
        self.assertIsNone(prior_cfg)  # Proves strict rejection when missing

    def test_finding3_unified_sequence_upsert(self):
        """Finding 3: Unified sequence allocation handles initialization with start_number and atomic increments."""
        # ser_jnl has start_number = 100
        start_number = 100
        period_start = "2026-04-01"
        seq_id = str(uuid.uuid4())

        # First allocation (Initialization path)
        self.cur.execute("""
        INSERT INTO voucher_number_sequences 
        (id, company_id, voucher_type_id, series_id, period_start, next_number)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', ?, ?)
        ON CONFLICT (company_id, series_id, period_start)
        DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
        RETURNING (voucher_number_sequences.next_number - 1) AS allocated_number;
        """, (seq_id, period_start, start_number + 1))
        alloc1 = self.cur.fetchone()[0]
        self.assertEqual(alloc1, 100)

        # Verify next_number in table is 101
        self.cur.execute("SELECT next_number FROM voucher_number_sequences WHERE series_id = 'ser_jnl'")
        self.assertEqual(self.cur.fetchone()[0], 101)

        # Second allocation (Increment path)
        seq_id2 = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO voucher_number_sequences 
        (id, company_id, voucher_type_id, series_id, period_start, next_number)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', ?, ?)
        ON CONFLICT (company_id, series_id, period_start)
        DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
        RETURNING (voucher_number_sequences.next_number - 1) AS allocated_number;
        """, (seq_id2, period_start, start_number + 1))
        alloc2 = self.cur.fetchone()[0]
        self.assertEqual(alloc2, 101)

        # Verify next_number is now 102
        self.cur.execute("SELECT next_number FROM voucher_number_sequences WHERE series_id = 'ser_jnl'")
        self.assertEqual(self.cur.fetchone()[0], 102)

    def test_finding4_line_inversion_with_uuid_parameters(self):
        """Finding 4: Line inversion preserves line order, ledgers, amounts, and uses UUIDs."""
        # Create draft voucher and lines
        v_orig = "v_orig_10"
        self.cur.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, status, total_amount)
        VALUES (?, 'comp1', 'vt_sales', 'ser_sales', 'SALES-0001', '2026-05-01', 'draft', '15000.00');
        """, (v_orig,))

        orig_lines = [
            ("l1", 1, "led_cash", "debit", "15000.00", "Cash received"),
            ("l2", 2, "led_sales", "credit", "15000.00", "Goods sold")
        ]
        for lid, lnum, led, etype, amt, narr in orig_lines:
            self.cur.execute("""
            INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
            VALUES (?, 'comp1', ?, ?, ?, ?, ?, ?);
            """, (lid, v_orig, lnum, led, etype, amt, narr))

        # Promote to posted
        self.cur.execute("UPDATE vouchers SET status = 'posted' WHERE id = ?", (v_orig,))

        # Fetch original lines ordered by line_number ASC
        self.cur.execute("""
        SELECT line_number, ledger_id, entry_type, amount, narration
        FROM voucher_lines WHERE company_id = 'comp1' AND voucher_id = ? ORDER BY line_number ASC
        """, (v_orig,))
        fetched = self.cur.fetchall()

        # Invert in Python using UUIDs
        v_rev = "v_rev_10"
        self.cur.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, voucher_number, date, status, total_amount, reversal_of_voucher_id)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', 'JNL-0100', '2026-05-02', 'draft', '15000.00', ?);
        """, (v_rev, v_orig))

        for row in fetched:
            lnum, led, etype, amt, narr = row
            inv_etype = "credit" if etype == "debit" else "debit"
            rev_lid = str(uuid.uuid4())
            rev_narr = f"Reversal: {narr}"
            self.cur.execute("""
            INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
            VALUES (?, 'comp1', ?, ?, ?, ?, ?, ?);
            """, (rev_lid, v_rev, lnum, led, inv_etype, amt, rev_narr))

        # Check inverted lines
        self.cur.execute("SELECT line_number, ledger_id, entry_type, amount, narration FROM voucher_lines WHERE voucher_id = ? ORDER BY line_number ASC", (v_rev,))
        inv_rows = self.cur.fetchall()
        self.assertEqual(len(inv_rows), 2)
        self.assertEqual(inv_rows[0], (1, "led_cash", "credit", "15000.00", "Reversal: Cash received"))
        self.assertEqual(inv_rows[1], (2, "led_sales", "debit", "15000.00", "Reversal: Goods sold"))

    def test_finding5_complete_13_step_atomic_reversal(self):
        """Finding 5: Complete 13-step atomic reversal workflow with dual audit logging."""
        # 0. Setup an original posted voucher
        v_orig = "v_orig_complete"
        self.cur.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, sequence_number, voucher_number, date, status, total_amount)
        VALUES (?, 'comp1', 'vt_sales', 'ser_sales', 1, 'SALES-0001', '2026-05-10', 'draft', '10000.00');
        """, (v_orig,))
        self.cur.execute("INSERT INTO voucher_lines VALUES ('l_orig_1', 'comp1', ?, 1, 'led_cash', 'debit', '10000.00', 'Original Dr')", (v_orig,))
        self.cur.execute("INSERT INTO voucher_lines VALUES ('l_orig_2', 'comp1', ?, 2, 'led_sales', 'credit', '10000.00', 'Original Cr')", (v_orig,))
        self.cur.execute("UPDATE vouchers SET status = 'posted' WHERE id = ?", (v_orig,))

        # Begin 13-step workflow
        # Step 1: BEGIN IMMEDIATE
        self.cur.execute("BEGIN IMMEDIATE;")

        # Step 2: Validate eligibility
        self.cur.execute("SELECT status, total_amount FROM vouchers WHERE company_id = 'comp1' AND id = ?", (v_orig,))
        orig_row = self.cur.fetchone()
        self.assertEqual(orig_row[0], 'posted')
        self.cur.execute("SELECT id FROM vouchers WHERE company_id = 'comp1' AND reversal_of_voucher_id = ?", (v_orig,))
        self.assertIsNone(self.cur.fetchone())

        # Step 3: Period derivation (FY begins 2026-04-01 -> period_start = 2026-04-01)
        period_start = "2026-04-01"

        # Step 4: Configuration lookup
        self.cur.execute("""
        SELECT id, prefix, suffix, width, start_number, restart_period
        FROM voucher_number_configs
        WHERE company_id = 'comp1' AND series_id = 'ser_jnl' AND applicable_from <= ?
        ORDER BY applicable_from DESC LIMIT 1
        """, (period_start,))
        cfg = self.cur.fetchone()
        self.assertIsNotNone(cfg)

        # Step 5: Sequence allocation (UPSERT)
        seq_id = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO voucher_number_sequences 
        (id, company_id, voucher_type_id, series_id, period_start, next_number)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', ?, ?)
        ON CONFLICT (company_id, series_id, period_start)
        DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
        RETURNING (voucher_number_sequences.next_number - 1) AS allocated_number;
        """, (seq_id, period_start, cfg[4] + 1))
        alloc_seq = self.cur.fetchone()[0]
        v_rev_number = f"JNL-{alloc_seq:04d}"

        # Step 6: Create reversal header as draft
        v_rev = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO vouchers (
            id, company_id, voucher_type_id, series_id, sequence_number, voucher_number,
            date, status, total_amount, reversal_of_voucher_id
        ) VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', ?, ?, '2026-05-12', 'draft', '10000.00', ?)
        """, (v_rev, alloc_seq, v_rev_number, v_orig))

        # Step 7: Inverted line creation with UUIDs
        self.cur.execute("""
        SELECT line_number, ledger_id, entry_type, amount, narration
        FROM voucher_lines WHERE company_id = 'comp1' AND voucher_id = ? ORDER BY line_number ASC
        """, (v_orig,))
        for lnum, led, etype, amt, narr in self.cur.fetchall():
            rev_lid = str(uuid.uuid4())
            inv_etype = "credit" if etype == "debit" else "debit"
            self.cur.execute("""
            INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration)
            VALUES (?, 'comp1', ?, ?, ?, ?, ?, ?)
            """, (rev_lid, v_rev, lnum, led, inv_etype, amt, f"Reversal: {narr}"))

        # Step 8: Double entry verification
        self.cur.execute("""
        SELECT 
            SUM(CASE WHEN entry_type = 'debit' THEN CAST(amount AS REAL) ELSE 0 END),
            SUM(CASE WHEN entry_type = 'credit' THEN CAST(amount AS REAL) ELSE 0 END)
        FROM voucher_lines WHERE voucher_id = ?
        """, (v_rev,))
        sum_dr, sum_cr = self.cur.fetchone()
        self.assertEqual(sum_dr, sum_cr)

        # Step 9: Promote reversal to posted
        self.cur.execute("UPDATE vouchers SET status = 'posted', updated_at = CURRENT_TIMESTAMP WHERE id = ?", (v_rev,))

        # Step 10: Transition original to reversed
        self.cur.execute("UPDATE vouchers SET status = 'reversed', updated_at = CURRENT_TIMESTAMP WHERE id = ?", (v_orig,))

        # Step 11: Emit POST audit event for reversal voucher
        audit_id_post = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO audit_log (
            id, company_id, entity_type, entity_id, ledger_id,
            financial_year_begins, action, old_values, new_values, performed_by
        ) VALUES (?, 'comp1', 'voucher', ?, NULL, '2026-04-01', 'POST', NULL, ?, 'finance_user')
        """, (audit_id_post, v_rev, json.dumps({"status": "posted", "voucher_number": v_rev_number, "reversal_of_voucher_id": v_orig})))

        # Step 12: Emit REVERSE audit event for original voucher
        audit_id_rev = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO audit_log (
            id, company_id, entity_type, entity_id, ledger_id,
            financial_year_begins, action, old_values, new_values, performed_by
        ) VALUES (?, 'comp1', 'voucher', ?, NULL, '2026-04-01', 'REVERSE', ?, ?, 'finance_user')
        """, (audit_id_rev, v_orig, json.dumps({"status": "posted"}), json.dumps({"status": "reversed", "reversal_voucher_id": v_rev})))

        # Step 13: COMMIT
        self.cur.execute("COMMIT;")

        # Verification of final state
        self.cur.execute("SELECT status FROM vouchers WHERE id = ?", (v_orig,))
        self.assertEqual(self.cur.fetchone()[0], 'reversed')

        self.cur.execute("SELECT status, reversal_of_voucher_id, voucher_number FROM vouchers WHERE id = ?", (v_rev,))
        rev_row = self.cur.fetchone()
        self.assertEqual(rev_row[0], 'posted')
        self.assertEqual(rev_row[1], v_orig)
        self.assertEqual(rev_row[2], 'JNL-0100')

        self.cur.execute("SELECT action, entity_id FROM audit_log WHERE entity_id IN (?, ?)", (v_orig, v_rev))
        audit_rows = self.cur.fetchall()
        self.assertEqual(len(audit_rows), 2)

    def test_finding5_rollback_on_failure_leaves_no_leak(self):
        """Finding 5: Rollback on error leaves original posted, zero sequence counter increment, zero audit log."""
        v_orig = "v_orig_fail"
        self.cur.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, sequence_number, voucher_number, date, status, total_amount)
        VALUES (?, 'comp1', 'vt_sales', 'ser_sales', 2, 'SALES-0002', '2026-05-10', 'draft', '20000.00');
        """, (v_orig,))
        self.cur.execute("INSERT INTO voucher_lines VALUES ('lf1', 'comp1', ?, 1, 'led_cash', 'debit', '20000.00', 'Dr')", (v_orig,))
        self.cur.execute("INSERT INTO voucher_lines VALUES ('lf2', 'comp1', ?, 2, 'led_sales', 'credit', '20000.00', 'Cr')", (v_orig,))
        self.cur.execute("UPDATE vouchers SET status = 'posted' WHERE id = ?", (v_orig,))

        # Begin transaction and simulate a failure at Step 8 (e.g. line imbalance or trigger violation)
        self.cur.execute("BEGIN IMMEDIATE;")
        seq_id = str(uuid.uuid4())
        self.cur.execute("""
        INSERT INTO voucher_number_sequences 
        (id, company_id, voucher_type_id, series_id, period_start, next_number)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', '2026-04-01', 101)
        ON CONFLICT (company_id, series_id, period_start)
        DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
        RETURNING (voucher_number_sequences.next_number - 1);
        """, (seq_id,))

        v_rev = "v_rev_fail"
        self.cur.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, sequence_number, voucher_number, date, status, total_amount, reversal_of_voucher_id)
        VALUES (?, 'comp1', 'vt_jnl', 'ser_jnl', 100, 'JNL-0100', '2026-05-12', 'draft', '20000.00', ?)
        """, (v_rev, v_orig))

        # Rollback due to simulated error
        self.cur.execute("ROLLBACK;")

        # Assert zero side-effects
        self.cur.execute("SELECT status FROM vouchers WHERE id = ?", (v_orig,))
        self.assertEqual(self.cur.fetchone()[0], 'posted')

        self.cur.execute("SELECT id FROM vouchers WHERE id = ?", (v_rev,))
        self.assertIsNone(self.cur.fetchone())

        # Assert sequences table was rolled back
        self.cur.execute("SELECT next_number FROM voucher_number_sequences WHERE series_id = 'ser_jnl'")
        self.assertIsNone(self.cur.fetchone())

        # Assert zero audit logs
        self.cur.execute("SELECT COUNT(*) FROM audit_log WHERE entity_id IN (?, ?)", (v_orig, v_rev))
        self.assertEqual(self.cur.fetchone()[0], 0)

if __name__ == '__main__':
    unittest.main()

