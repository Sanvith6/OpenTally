import sqlite3

def setup_db():
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
    
    cur.execute("INSERT INTO companies VALUES ('c1', 'Acme Corp', '2026-04-01', '2026-04-01')")
    cur.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'c1', 'Sales')")
    
    # May config: applicable_from 2026-04-01, start_number 1, prefix 'INV-26/'
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg1', 'c1', 'vt_sales', 'Primary', '2026-04-01', 'INV-26/', '', 1, 4, 'yearly'
    )
    """)
    # August config: applicable_from 2026-08-01, start_number 100, prefix 'GST-26/'
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg2', 'c1', 'vt_sales', 'Primary', '2026-08-01', 'GST-26/', '', 100, 4, 'yearly'
    )
    """)
    conn.commit()
    return conn

def allocate_voucher_number(conn, company_id, voucher_type_id, series_name, voucher_date):
    cur = conn.cursor()
    # 1. Calculate period_start (for yearly Indian FY starting April 1):
    # If voucher_date is e.g. 2026-08-15, period_start is 2026-04-01.
    # If voucher_date is 2026-05-10, period_start is 2026-04-01.
    year = int(voucher_date[:4])
    month = int(voucher_date[5:7])
    if month < 4:
        period_start = f"{year-1:04d}-04-01"
    else:
        period_start = f"{year:04d}-04-01"
        
    # 2. Under Period-Anchor Initialization Invariant:
    # Resolve the start_number defined for this PERIOD (effective on or before period_start):
    cur.execute("""
        SELECT start_number 
        FROM voucher_number_configs
        WHERE company_id = ? AND voucher_type_id = ? AND series_name = ?
          AND applicable_from <= ?
        ORDER BY applicable_from DESC
        LIMIT 1
    """, (company_id, voucher_type_id, series_name, period_start))
    row = cur.fetchone()
    if row:
        period_start_num = row[0]
    else:
        # Fallback to earliest configuration for this series
        cur.execute("""
            SELECT start_number 
            FROM voucher_number_configs
            WHERE company_id = ? AND voucher_type_id = ? AND series_name = ?
            ORDER BY applicable_from ASC
            LIMIT 1
        """, (company_id, voucher_type_id, series_name))
        fb = cur.fetchone()
        period_start_num = fb[0] if fb else 1

    # 3. Atomic UPSERT into pure counter table
    import uuid
    seq_id = str(uuid.uuid4())
    cur.execute("""
        INSERT INTO voucher_number_sequences (
            id, company_id, voucher_type_id, series_name, period_start, current_number
        ) VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT (company_id, voucher_type_id, series_name, period_start)
        DO UPDATE SET current_number = voucher_number_sequences.current_number + 1
        RETURNING current_number;
    """, (seq_id, company_id, voucher_type_id, series_name, period_start, period_start_num))
    allocated_num = cur.fetchone()[0]

    # 4. Resolve the active formatting config effective on voucher_date:
    cur.execute("""
        SELECT prefix, suffix, width 
        FROM voucher_number_configs
        WHERE company_id = ? AND voucher_type_id = ? AND series_name = ?
          AND applicable_from <= ?
        ORDER BY applicable_from DESC
        LIMIT 1
    """, (company_id, voucher_type_id, series_name, voucher_date))
    fmt = cur.fetchone()
    prefix, suffix, width = fmt[0], fmt[1], fmt[2]
    
    formatted_no = f"{prefix}{allocated_num:0{width}d}{suffix}"
    return allocated_num, formatted_no

# Test Case 1: August voucher first, then May voucher
conn1 = setup_db()
num_aug1, fmt_aug1 = allocate_voucher_number(conn1, 'c1', 'vt_sales', 'Primary', '2026-08-15')
num_may1, fmt_may1 = allocate_voucher_number(conn1, 'c1', 'vt_sales', 'Primary', '2026-05-10')

print(f"CASE 1 (August first, then May):")
print(f"  August 2026-08-15 -> Seq: {num_aug1}, Formatted: {fmt_aug1}")
print(f"  May    2026-05-10 -> Seq: {num_may1}, Formatted: {fmt_may1}")

# Test Case 2: May voucher first, then August voucher
conn2 = setup_db()
num_may2, fmt_may2 = allocate_voucher_number(conn2, 'c1', 'vt_sales', 'Primary', '2026-05-10')
num_aug2, fmt_aug2 = allocate_voucher_number(conn2, 'c1', 'vt_sales', 'Primary', '2026-08-15')

print(f"\nCASE 2 (May first, then August):")
print(f"  May    2026-05-10 -> Seq: {num_may2}, Formatted: {fmt_may2}")
print(f"  August 2026-08-15 -> Seq: {num_aug2}, Formatted: {fmt_aug2}")

# Check determinism:
# In both cases, the sequence numbers allocated across the period are 1 and 2!
assert {num_aug1, num_may1} == {1, 2}
assert {num_may2, num_aug2} == {1, 2}
# The period counter is 100% deterministic and initialized to start_number 1 (resolved at period_start 2026-04-01).
# August gets prefix 'GST-26/', May gets prefix 'INV-26/' in both cases!
print("\nAssertion passed: Deterministic period-anchor initialization verified!")

