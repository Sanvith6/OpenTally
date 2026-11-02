import sqlite3

def compute_sequence_period_start(restart_period, fy_begins, books_beginning, voucher_date):
    v_parts = [int(p) for p in voucher_date.split('-')]
    v_year, v_month, v_day = v_parts[0], v_parts[1], v_parts[2]
    
    if restart_period == "yearly":
        fy_parts = [int(p) for p in fy_begins.split('-')]
        fy_m, fy_d = fy_parts[1], fy_parts[2]
        
        # Leap year normalization
        is_leap_year = lambda y: (y % 4 == 0 and y % 100 != 0) or (y % 400 == 0)
        if fy_m == 2 and fy_d == 29 and not is_leap_year(v_year):
            fy_d = 28
            
        start_year = v_year if (v_month, v_day) >= (fy_m, fy_d) else v_year - 1
        return f"{start_year:04d}-{fy_m:02d}-{fy_d:02d}"
    elif restart_period == "monthly":
        return f"{v_year:04d}-{v_month:02d}-01"
    elif restart_period == "daily":
        return voucher_date
    elif restart_period == "never":
        return books_beginning
    else:
        raise ValueError(f"Unsupported restart period: {restart_period}")

def test_partial_year():
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()

    c.executescript("""
    CREATE TABLE voucher_number_configs (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        applicable_from TEXT NOT NULL,
        start_number INTEGER NOT NULL
    );
    """)

    # Scenario:
    # FY starts: 2026-04-01
    # Books begin: 2026-08-01 (partial inaugural FY)
    # Config applicable_from: 2026-08-01, start_number: 1
    # First voucher: 2026-08-15
    fy_begins = "2026-04-01"
    books_beginning = "2026-08-01"
    voucher_date = "2026-08-15"

    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg1', 'comp1', 'vt_sales', 'Primary', '2026-08-01', 1);
    """)

    # 1. Compute standard period_start
    period_start = compute_sequence_period_start("yearly", fy_begins, books_beginning, voucher_date)
    assert period_start == "2026-04-01", f"Expected 2026-04-01, got {period_start}"

    # 2. Compute effective anchor date distinguishing FY start, books beginning, and period anchor:
    # effective_anchor = max(period_start, books_beginning_from)
    effective_anchor = max(period_start, books_beginning)
    assert effective_anchor == "2026-08-01", f"Expected 2026-08-01, got {effective_anchor}"

    # 3. Query configuration using effective_anchor
    c.execute("""
    SELECT start_number 
    FROM voucher_number_configs 
    WHERE company_id = 'comp1' AND voucher_type_id = 'vt_sales' AND series_name = 'Primary'
      AND applicable_from <= ?
    ORDER BY applicable_from DESC LIMIT 1;
    """, (effective_anchor,))
    row = c.fetchone()
    assert row is not None, "Configuration must be resolved for partial year!"
    assert row[0] == 1, "start_number must be 1"

    # 4. Now test subsequent full year: FY 2027-28
    v2_date = "2027-05-10"
    period_start_y2 = compute_sequence_period_start("yearly", fy_begins, books_beginning, v2_date)
    assert period_start_y2 == "2027-04-01"
    effective_anchor_y2 = max(period_start_y2, books_beginning)
    assert effective_anchor_y2 == "2027-04-01"

    c.execute("""
    SELECT start_number 
    FROM voucher_number_configs 
    WHERE company_id = 'comp1' AND voucher_type_id = 'vt_sales' AND series_name = 'Primary'
      AND applicable_from <= ?
    ORDER BY applicable_from DESC LIMIT 1;
    """, (effective_anchor_y2,))
    row = c.fetchone()
    assert row is not None, "Year 2 must resolve Year 1 configuration!"
    assert row[0] == 1

    conn.close()
    print("Finding 3: Partial-Year Books Beginning Test Passed 100%!")

if __name__ == "__main__":
    test_partial_year()

