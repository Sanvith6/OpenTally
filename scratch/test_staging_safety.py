import sqlite3

print("=== Simulating Staging Safety & Proposed Invariant Test ===")

conn = sqlite3.connect(":memory:")
conn.execute("PRAGMA foreign_keys = ON;")

# Minimal tables
conn.execute("CREATE TABLE companies (id TEXT PRIMARY KEY);")
conn.execute("INSERT INTO companies VALUES ('c1');")
conn.execute("CREATE TABLE ledgers (company_id TEXT, id TEXT PRIMARY KEY);")
conn.execute("INSERT INTO ledgers VALUES ('c1', 'l1');")
conn.execute("CREATE TABLE vouchers (company_id TEXT, id TEXT PRIMARY KEY);")
conn.execute("INSERT INTO vouchers VALUES ('c1', 'v1');")

conn.execute("""
CREATE TABLE voucher_lines (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    ledger_id TEXT NOT NULL,
    UNIQUE (company_id, voucher_id, line_number),
    CONSTRAINT chk_voucher_line_number_bounds CHECK (
        (line_number BETWEEN 1 AND 10000) OR (line_number BETWEEN 1000001 AND 1010000)
    )
);
""")

# Seed 5 lines
for i in range(1, 6):
    conn.execute(f"INSERT INTO voucher_lines VALUES ('line_{i}', 'c1', 'v1', {i}, 'l1');")

def assert_no_staging_values(conn, voucher_id):
    cur = conn.cursor()
    # Check 1: Zero staging values exist
    staging_count = cur.execute("""
        SELECT COUNT(*) FROM voucher_lines 
        WHERE voucher_id = ? AND (line_number < 1 OR line_number > 10000)
    """, (voucher_id,)).fetchone()[0]
    assert staging_count == 0, f"Invariant violation: found {staging_count} staging/out-of-bounds lines!"
    
    # Check 2: Contiguous 1..N indices
    lines = cur.execute("""
        SELECT line_number FROM voucher_lines 
        WHERE voucher_id = ? ORDER BY line_number ASC
    """, (voucher_id,)).fetchall()
    expected = [(i,) for i in range(1, len(lines) + 1)]
    assert lines == expected, f"Invariant violation: lines are not contiguous: {lines} vs {expected}"
    print(f"Verified invariant: {len(lines)} active lines, 0 staging values, contiguous 1..{len(lines)}.")

print("Testing initial state:")
assert_no_staging_values(conn, 'v1')

# Simulate reorder with two-phase staging
print("\nSimulating reorder_lines...")
with conn:
    # Phase 1: Shift to staging
    conn.execute("UPDATE voucher_lines SET line_number = line_number + 1000000 WHERE voucher_id = 'v1';")
    # Phase 2: Reverse order
    order = ['line_5', 'line_4', 'line_3', 'line_2', 'line_1']
    for idx, lid in enumerate(order, start=1):
        conn.execute("UPDATE voucher_lines SET line_number = ? WHERE id = ?;", (idx, lid))

assert_no_staging_values(conn, 'v1')

# Simulate delete_line with two-phase staging
print("\nSimulating delete_line (deleting line_3, which is now at line 3)...")
with conn:
    conn.execute("DELETE FROM voucher_lines WHERE id = 'line_3';")
    remaining = conn.execute("SELECT id FROM voucher_lines WHERE voucher_id = 'v1' ORDER BY line_number ASC;").fetchall()
    # Phase 1
    conn.execute("UPDATE voucher_lines SET line_number = line_number + 1000000 WHERE voucher_id = 'v1';")
    # Phase 2
    for idx, (lid,) in enumerate(remaining, start=1):
        conn.execute("UPDATE voucher_lines SET line_number = ? WHERE id = ?;", (idx, lid))

assert_no_staging_values(conn, 'v1')

# Simulate rollback during staging
print("\nSimulating aborted operation during staging...")
try:
    with conn:
        conn.execute("UPDATE voucher_lines SET line_number = line_number + 1000000 WHERE voucher_id = 'v1';")
        # Deliberate failure
        raise RuntimeError("Simulated crash before Phase 2")
except RuntimeError:
    pass

# Transaction rolled back: verify state is restored to active range!
assert_no_staging_values(conn, 'v1')
print("Rollback verification successful: all lines remain in active range [1, 10000].")

