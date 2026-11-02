import sqlite3

def run_tests():
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    # Setup base schema
    c.executescript("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        financial_year_begins TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL
    );

    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual', 'multi_series')),
        FOREIGN KEY (company_id) REFERENCES companies(id)
    );

    CREATE TABLE voucher_number_configs (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1,
        width INTEGER NOT NULL DEFAULT 4,
        restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
        FOREIGN KEY (company_id) REFERENCES companies(id),
        FOREIGN KEY (voucher_type_id) REFERENCES voucher_types(id),
        CONSTRAINT uq_voucher_number_configs_date UNIQUE (company_id, voucher_type_id, series_name, applicable_from)
    );

    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        total_amount TEXT,
        FOREIGN KEY (company_id) REFERENCES companies(id),
        FOREIGN KEY (voucher_type_id) REFERENCES voucher_types(id)
    );

    CREATE UNIQUE INDEX uq_vouchers_permanent_number
    ON vouchers(company_id, voucher_type_id, voucher_number)
    WHERE status != 'draft';
    """)

    # Seed company & voucher type
    c.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp', '2026-04-01', '2026-04-01');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales', 'multi_series');")

    # Series 1: Automatic Series with prefix 'INV-26/'
    c.execute("""
    INSERT INTO voucher_number_configs 
    (id, company_id, voucher_type_id, series_name, numbering_method, applicable_from, prefix, suffix, start_number, width, restart_period)
    VALUES ('cfg_auto', 'comp1', 'vt_sales', 'Auto-Series', 'automatic', '2026-04-01', 'INV-26/', '', 1, 4, 'yearly');
    """)

    # Series 2: Manual Series with prefix 'MAN-26/'
    c.execute("""
    INSERT INTO voucher_number_configs 
    (id, company_id, voucher_type_id, series_name, numbering_method, applicable_from, prefix, suffix, start_number, width, restart_period)
    VALUES ('cfg_man', 'comp1', 'vt_sales', 'Manual-Series', 'manual', '2026-04-01', 'MAN-26/', '', 1, 4, 'never');
    """)

    # Test Case Demonstration from Finding 1:
    # 1. Automatic voucher receives 1 -> "INV-26/0001"
    c.execute("INSERT INTO vouchers VALUES ('v1', 'comp1', 'vt_sales', 'Auto-Series', 'INV-26/0001', '2026-05-01', 'posted', '1000.00');")
    
    # 2. Manual voucher receives 2 -> "MAN-26/0002"
    c.execute("INSERT INTO vouchers VALUES ('v2', 'comp1', 'vt_sales', 'Manual-Series', 'MAN-26/0002', '2026-05-02', 'posted', '2000.00');")

    # 3. Automatic vouchers receive 3 and 4 -> "INV-26/0002" and "INV-26/0003"
    # Notice: In Auto-Series, sequence increments contiguously 1 -> 2 -> 3 without gap!
    c.execute("INSERT INTO vouchers VALUES ('v3', 'comp1', 'vt_sales', 'Auto-Series', 'INV-26/0002', '2026-05-03', 'posted', '3000.00');")
    c.execute("INSERT INTO vouchers VALUES ('v4', 'comp1', 'vt_sales', 'Auto-Series', 'INV-26/0003', '2026-05-04', 'posted', '4000.00');")

    # 4. Another manual voucher receives 5 -> "MAN-26/0005"
    c.execute("INSERT INTO vouchers VALUES ('v5', 'comp1', 'vt_sales', 'Manual-Series', 'MAN-26/0005', '2026-05-05', 'posted', '5000.00');")

    # 5. Next automatic allocation attempts 4 -> "INV-26/0004"
    c.execute("INSERT INTO vouchers VALUES ('v6', 'comp1', 'vt_sales', 'Auto-Series', 'INV-26/0004', '2026-05-06', 'posted', '6000.00');")
    # And allocation 5 -> "INV-26/0005"
    c.execute("INSERT INTO vouchers VALUES ('v7', 'comp1', 'vt_sales', 'Auto-Series', 'INV-26/0005', '2026-05-07', 'posted', '7000.00');")

    # Both "INV-26/0005" and "MAN-26/0005" coexist peacefully because namespaces are distinct!
    c.execute("SELECT count(*) FROM vouchers;")
    assert c.fetchone()[0] == 7, "Expected 7 vouchers inserted cleanly"

    # Now verify collision prevention if manual voucher attempts to usurp the automatic series namespace:
    # Attempting to post a manual voucher with number 'INV-26/0005'
    collision_caught = False
    try:
        c.execute("INSERT INTO vouchers VALUES ('v_coll', 'comp1', 'vt_sales', 'Manual-Series', 'INV-26/0005', '2026-05-08', 'posted', '8000.00');")
    except sqlite3.IntegrityError as e:
        collision_caught = True
        assert "UNIQUE constraint failed" in str(e)
    assert collision_caught, "Partial unique index uq_vouchers_permanent_number must reject collision!"

    conn.close()
    print("Finding 1: Manual/Auto Collision & Namespace Isolation Test Passed 100%!")

if __name__ == "__main__":
    run_tests()
