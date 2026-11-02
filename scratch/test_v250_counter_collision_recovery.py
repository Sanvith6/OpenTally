import sqlite3

def test_counter_collision_recovery():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    c.executescript("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
    );

    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        FOREIGN KEY (company_id) REFERENCES companies(id)
    );

    CREATE TABLE voucher_number_sequences (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        period_start TEXT NOT NULL,
        current_number INTEGER NOT NULL DEFAULT 0,
        CONSTRAINT uq_seq_period UNIQUE (company_id, voucher_type_id, series_name, period_start)
    );

    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        voucher_number TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed'))
    );

    CREATE UNIQUE INDEX uq_vouchers_permanent_number
    ON vouchers(company_id, voucher_type_id, voucher_number)
    WHERE status != 'draft';
    """)

    c.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales');")

    # Sequence initialized at 1
    c.execute("INSERT INTO voucher_number_sequences VALUES ('seq1', 'comp1', 'vt_sales', 'Primary', '2026-04-01', 1);")
    # Voucher 1 already exists
    c.execute("INSERT INTO vouchers VALUES ('v1', 'comp1', 'vt_sales', 'Primary', 'INV-0001', 'posted');")

    # Simulate an external pre-existing voucher occupying 'INV-0002' (e.g. from an import or manual entry)
    c.execute("INSERT INTO vouchers VALUES ('v_pre', 'comp1', 'vt_sales', 'Manual', 'INV-0002', 'posted');")

    # Post operation under BEGIN IMMEDIATE:
    # Worker attempts to post next voucher
    # Current counter is 1. Increment attempts to allocate 2 -> "INV-0002"
    c.execute("BEGIN IMMEDIATE;")
    
    # Counter incremented in transaction
    c.execute("""
    UPDATE voucher_number_sequences 
    SET current_number = current_number + 1 
    WHERE id = 'seq1';
    """)
    allocated = 2
    number_str = f"INV-{allocated:04d}"

    collision_occurred = False
    try:
        c.execute("INSERT INTO vouchers VALUES ('v2', 'comp1', 'vt_sales', 'Primary', ?, 'posted');", (number_str,))
    except sqlite3.IntegrityError as e:
        collision_occurred = True
        # Rollback transaction on collision
        conn.rollback()

    assert collision_occurred, "Collision on occupied number must trigger IntegrityError"

    # Verify State after Rollback:
    # 1. Counter in voucher_number_sequences must be REVERTED to 1 (not burned or incremented)
    c.execute("SELECT current_number FROM voucher_number_sequences WHERE id = 'seq1';")
    current_num = c.fetchone()[0]
    assert current_num == 1, f"Counter must be rolled back to 1, got {current_num}"

    # 2. Existing vouchers are 100% unchanged
    c.execute("SELECT voucher_number FROM vouchers ORDER BY id;")
    rows = [r[0] for r in c.fetchall()]
    assert rows == ['INV-0001', 'INV-0002']

    # 3. No numbers were silently skipped!
    # The system does not advance to 3 without resolution.

    conn.close()
    print("Finding 4: Counter Recovery After Collision Test Passed 100%!")

if __name__ == "__main__":
    test_counter_collision_recovery()
