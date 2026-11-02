import sqlite3

def run_numbering_integrity_tests():
    conn = sqlite3.connect(":memory:")
    cur = conn.cursor()
    cur.execute("PRAGMA foreign_keys = ON;")

    cur.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        financial_year_start TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL
    );
    """)

    cur.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id)
    );
    """)

    cur.execute("""
    CREATE TABLE voucher_number_configs (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
        width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
        restart_period TEXT NOT NULL DEFAULT 'yearly' CHECK (restart_period IN ('never', 'yearly', 'monthly', 'daily')),
        PRIMARY KEY (company_id, id),
        UNIQUE (company_id, voucher_type_id, series_name, applicable_from),
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id)
    );
    """)

    cur.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        period_start TEXT NOT NULL,
        current_number INTEGER NOT NULL DEFAULT 0 CHECK (current_number >= 0),
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_seq_period UNIQUE (company_id, voucher_type_id, series_name, period_start),
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id)
    );
    """)

    cur.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        total_amount TEXT,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id)
    );
    """)

    # Permanent Voucher Number Unique Index (Finding 4)
    cur.execute("""
    CREATE UNIQUE INDEX uq_vouchers_permanent_number 
    ON vouchers(company_id, voucher_type_id, voucher_number) 
    WHERE status != 'draft';
    """)

    # Triggers protecting historical configuration immutability (Finding 3)
    cur.execute("""
    CREATE TRIGGER trg_prevent_active_config_mutation
    BEFORE UPDATE ON voucher_number_configs
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.voucher_type_id = OLD.voucher_type_id
          AND v.status IN ('posted', 'reversed', 'cancelled')
          AND v.date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot modify a voucher numbering configuration that has already been used to issue permanent financial vouchers.');
    END;
    """)

    cur.execute("""
    CREATE TRIGGER trg_prevent_active_config_deletion
    BEFORE DELETE ON voucher_number_configs
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.voucher_type_id = OLD.voucher_type_id
          AND v.status IN ('posted', 'reversed', 'cancelled')
          AND v.date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot delete a voucher numbering configuration that has already been used to issue permanent financial vouchers.');
    END;
    """)

    # Seed company & voucher type
    cur.execute("INSERT INTO companies VALUES ('c1', 'Acme', '2026-04-01', '2026-04-01')")
    cur.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'c1', 'Sales')")

    # TEST A: Missing anchor configuration rejection (Finding 2)
    # Config created with applicable_from = '2026-08-01' (no config on or before 2026-04-01)
    cur.execute("INSERT INTO voucher_number_configs VALUES ('cfg_aug', 'c1', 'vt_sales', 'Primary', '2026-08-01', 'GST-26/', '', 1, 4, 'yearly')")
    conn.commit()

    # Function simulating period anchor check in posting service
    def post_voucher(voucher_date, provisional_no, total_amt):
        period_start = "2026-04-01" # FY begins April 1
        # Check if period-anchor config exists:
        cur.execute("""
            SELECT id, prefix, width, start_number 
            FROM voucher_number_configs 
            WHERE company_id = 'c1' AND voucher_type_id = 'vt_sales' AND series_name = 'Primary'
              AND applicable_from <= ?
            ORDER BY applicable_from DESC LIMIT 1
        """, (period_start,))
        row = cur.fetchone()
        if not row:
            raise ValueError(f"Missing numbering configuration for period starting {period_start}. An initial numbering rule must be configured on or before {period_start}.")
        
        # If exists, allocate sequence
        cur.execute("""
            INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_name, period_start, current_number)
            VALUES (?, 'c1', 'vt_sales', 'Primary', ?, 1)
            ON CONFLICT (company_id, voucher_type_id, series_name, period_start)
            DO UPDATE SET current_number = voucher_number_sequences.current_number + 1
            RETURNING current_number;
        """, (f'seq_{voucher_date}', period_start))
        seq = cur.fetchone()[0]
        
        # Format using active config on voucher_date:
        cur.execute("""
            SELECT prefix, width FROM voucher_number_configs 
            WHERE company_id = 'c1' AND voucher_type_id = 'vt_sales' AND series_name = 'Primary'
              AND applicable_from <= ?
            ORDER BY applicable_from DESC LIMIT 1
        """, (voucher_date,))
        fmt = cur.fetchone()
        v_num = f"{fmt[0]}{seq:0{fmt[1]}d}"
        
        cur.execute("INSERT INTO vouchers VALUES (?, 'c1', 'vt_sales', ?, ?, 'posted', ?)",
                    (f'v_{seq}', v_num, voucher_date, total_amt))
        return v_num

    # 1. Posting dated 2026-08-15 fails because no anchor config exists on 2026-04-01!
    try:
        post_voucher("2026-08-15", "D-01", "100.00")
        assert False, "Should have failed due to missing anchor config!"
    except ValueError as e:
        print(f"Test A (Missing anchor rejection): PASSED ({e})")

    # 2. Add valid anchor config on 2026-04-01:
    cur.execute("INSERT INTO voucher_number_configs VALUES ('cfg_apr', 'c1', 'vt_sales', 'Primary', '2026-04-01', 'INV-26/', '', 1, 4, 'yearly')")
    conn.commit()

    # Now posting succeeds!
    v1_no = post_voucher("2026-08-15", "D-01", "100.00")
    print(f"Test A (Valid anchor posting): PASSED -> Allocated {v1_no}")
    assert v1_no == "GST-26/0001"

    # TEST B: Configuration Immutability (Finding 3)
    # Attempt to edit cfg_apr or cfg_aug while vouchers exist:
    try:
        cur.execute("UPDATE voucher_number_configs SET prefix = 'MODIFIED/' WHERE id = 'cfg_apr'")
        assert False, "Editing cfg_apr should be blocked by trigger!"
    except sqlite3.IntegrityError as e:
        print(f"Test B (Config mutation blocked): PASSED ({e})")

    try:
        cur.execute("DELETE FROM voucher_number_configs WHERE id = 'cfg_apr'")
        assert False, "Deleting cfg_apr should be blocked by trigger!"
    except sqlite3.IntegrityError as e:
        print(f"Test B (Config deletion blocked): PASSED ({e})")

    # TEST C: Permanent Voucher Number Uniqueness & Collision Protection (Finding 4)
    # Attempt direct insert with identical permanent number 'GST-26/0001' on another posted voucher:
    try:
        cur.execute("INSERT INTO vouchers VALUES ('v_coll', 'c1', 'vt_sales', 'GST-26/0001', '2026-08-16', 'posted', '200.00')")
        assert False, "Duplicate permanent voucher number must be rejected!"
    except sqlite3.IntegrityError as e:
        print(f"Test C (Duplicate permanent number blocked): PASSED ({e})")

    # Multiple drafts with the same provisional number CAN coexist:
    cur.execute("INSERT INTO vouchers VALUES ('d1', 'c1', 'vt_sales', 'DRAFT-01', '2026-08-16', 'draft', NULL)")
    cur.execute("INSERT INTO vouchers VALUES ('d2', 'c1', 'vt_sales', 'DRAFT-01', '2026-08-16', 'draft', NULL)")
    print("Test C (Multiple drafts coexisting): PASSED")

    # TEST D: Gapless Numbering Scope (Finding 1)
    # Post voucher 2 -> GST-26/0002
    v2_no = post_voucher("2026-08-17", "D-02", "300.00")
    assert v2_no == "GST-26/0002"
    # Now cancel voucher 2:
    cur.execute("UPDATE vouchers SET status = 'cancelled' WHERE voucher_number = 'GST-26/0002'")
    # Post voucher 3 -> GST-26/0003
    v3_no = post_voucher("2026-08-18", "D-03", "400.00")
    assert v3_no == "GST-26/0003"
    
    # Query all permanent records (posted, reversed, cancelled):
    cur.execute("SELECT voucher_number, status FROM vouchers WHERE status != 'draft' ORDER BY voucher_number")
    records = cur.fetchall()
    print("Test D Permanent Audit Records:", records)
    # Full permanent sequence is [('GST-26/0001', 'posted'), ('GST-26/0002', 'cancelled'), ('GST-26/0003', 'posted')]
    # Numbers 1, 2, 3 are 100% gapless in permanent audit history!
    assert [r[0] for r in records] == ['GST-26/0001', 'GST-26/0002', 'GST-26/0003']
    print("Test D (Gapless committed allocation including cancelled): PASSED")

    print("\nAll Numbering and Integrity Tests PASSED!")

run_numbering_integrity_tests()

