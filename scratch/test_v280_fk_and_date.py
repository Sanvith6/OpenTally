import sqlite3

def test_sqlite_date_validation():
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    c.execute("""
    CREATE TABLE date_test (
        d TEXT NOT NULL,
        CONSTRAINT chk_date CHECK (
            d GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            AND date(d) IS NOT NULL
            AND date(d) = d
        )
    );
    """)

    # Valid dates
    valid = ["2026-04-01", "2026-01-01", "2026-12-31", "2024-02-29", "2000-02-29"]
    for v in valid:
        c.execute("INSERT INTO date_test VALUES (?)", (v,))
    print("All valid dates inserted successfully.")

    # Invalid dates
    invalid = [
        "2026-02-30", # Feb 30
        "2026-02-29", # 2026 is non-leap
        "2026-04-31", # April has 30 days
        "2026-99-99", # Invalid month/day
        "2026-00-01", # Month 00
        "2026-01-00", # Day 00
        "2026-13-01", # Month 13
        "1900-02-29", # 1900 is non-leap (divisible by 100, not 400)
    ]
    for inv in invalid:
        try:
            c.execute("INSERT INTO date_test VALUES (?)", (inv,))
            assert False, f"Expected invalid date {inv} to fail check constraint"
        except sqlite3.IntegrityError:
            pass
    print("All invalid dates rejected successfully by SQLite.")

def test_composite_foreign_keys():
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    # 1. Companies
    c.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
    );
    """)

    # 2. Voucher Types
    c.execute("""
    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
        CONSTRAINT uq_voucher_types_company UNIQUE (company_id, id)
    );
    """)

    # 3. Voucher Number Series
    c.execute("""
    CREATE TABLE voucher_number_series (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        series_code TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
        CONSTRAINT uq_series_company_id UNIQUE (company_id, id),
        CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name),
        CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code),
        CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id),
        CONSTRAINT uq_series_full_ownership UNIQUE (company_id, voucher_type_id, id, numbering_method)
    );
    """)

    # 4. Voucher Number Configs
    c.execute("""
    CREATE TABLE voucher_number_configs (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
        width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
        restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (company_id, voucher_type_id, series_id) 
            REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
        CONSTRAINT uq_config_series_date UNIQUE (company_id, series_id, applicable_from)
    );
    """)

    # 5. Voucher Number Sequences
    c.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        period_start TEXT NOT NULL,
        current_number INTEGER NOT NULL CHECK (current_number >= 0),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (company_id, voucher_type_id, series_id) 
            REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
        CONSTRAINT uq_seq_period UNIQUE (company_id, series_id, period_start)
    );
    """)

    # 6. Vouchers
    c.execute("""
    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) 
            REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
        CONSTRAINT chk_voucher_sequence_metadata CHECK (
            (
                status = 'draft'
                AND (
                    (sequence_number IS NULL AND period_start IS NULL)
                    OR
                    (sequence_number > 0 AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(period_start) = period_start)
                )
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND numbering_method = 'automatic'
                AND period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
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
        )
    );
    """)

    # Setup seed data
    c.execute("INSERT INTO companies VALUES ('comp_1', 'Company 1');")
    c.execute("INSERT INTO companies VALUES ('comp_2', 'Company 2');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales_1', 'comp_1', 'Sales');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_purch_1', 'comp_1', 'Purchase');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales_2', 'comp_2', 'Sales');")

    c.execute("""
    INSERT INTO voucher_number_series 
    (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
    VALUES ('s_auto_sales_1', 'comp_1', 'vt_sales_1', 'Primary', 'PRI', 'automatic');
    """)

    c.execute("""
    INSERT INTO voucher_number_series 
    (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
    VALUES ('s_man_sales_1', 'comp_1', 'vt_sales_1', 'Manual Sales', 'MAN', 'manual');
    """)

    # Test 1: Cross-company series assignment to voucher rejected
    try:
        c.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
        VALUES ('v1', 'comp_2', 'vt_sales_2', 's_auto_sales_1', 'automatic', '2026-04-01', 1, 'PRI/0001', '2026-05-01', 'posted');
        """)
        assert False, "Should reject cross-company series FK"
    except sqlite3.IntegrityError as e:
        print("Cross-company series rejected:", e)

    # Test 2: Mismatched voucher_type_id rejected (Series belongs to Sales, but voucher says Purchase)
    try:
        c.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
        VALUES ('v2', 'comp_1', 'vt_purch_1', 's_auto_sales_1', 'automatic', '2026-04-01', 1, 'PRI/0001', '2026-05-01', 'posted');
        """)
        assert False, "Should reject mismatched voucher_type_id"
    except sqlite3.IntegrityError as e:
        print("Mismatched voucher_type_id rejected:", e)

    # Test 3: Mismatched numbering_method rejected (Series is automatic, but voucher claims manual)
    try:
        c.execute("""
        INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
        VALUES ('v3', 'comp_1', 'vt_sales_1', 's_auto_sales_1', 'manual', NULL, NULL, 'PRI/0001', '2026-05-01', 'posted');
        """)
        assert False, "Should reject mismatched numbering_method"
    except sqlite3.IntegrityError as e:
        print("Mismatched numbering_method rejected:", e)

    # Test 4: Valid automatic voucher inserted cleanly
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v4', 'comp_1', 'vt_sales_1', 's_auto_sales_1', 'automatic', '2026-04-01', 1, 'PRI/26-27/0001', '2026-05-01', 'posted');
    """)
    print("Valid automatic voucher inserted cleanly.")

    # Test 5: Valid manual voucher inserted cleanly
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v5', 'comp_1', 'vt_sales_1', 's_man_sales_1', 'manual', NULL, NULL, 'MAN/1001', '2026-05-01', 'posted');
    """)
    print("Valid manual voucher inserted cleanly.")

    # Test 6: Cross-type config insertion rejected
    try:
        c.execute("""
        INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix)
        VALUES ('cfg_bad', 'comp_1', 'vt_purch_1', 's_auto_sales_1', '2026-04-01', 'PUR/');
        """)
        assert False, "Should reject cross-type config FK"
    except sqlite3.IntegrityError as e:
        print("Cross-type config rejected:", e)

    # Test 7: Cross-type sequence insertion rejected
    try:
        c.execute("""
        INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, current_number)
        VALUES ('seq_bad', 'comp_1', 'vt_purch_1', 's_auto_sales_1', '2026-04-01', 1);
        """)
        assert False, "Should reject cross-type sequence FK"
    except sqlite3.IntegrityError as e:
        print("Cross-type sequence rejected:", e)

    print("\nALL FOREIGN KEY AND INTEGRITY TESTS PASSED 100%!")

if __name__ == "__main__":
    test_sqlite_date_validation()
    test_composite_foreign_keys()
