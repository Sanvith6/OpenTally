import sqlite3

def test_representation_last_allocated():
    print("\n--- Testing Representation A: last_allocated_number ---")
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("""
    CREATE TABLE seq_last (
        series_id TEXT PRIMARY KEY,
        period_start TEXT NOT NULL,
        last_allocated INTEGER NOT NULL
    );
    """)

    start_number = 100

    # Allocation query for last_allocated:
    # On first insert: last_allocated = start_number
    # On update: last_allocated = last_allocated + 1
    # RETURNING last_allocated
    upsert_sql = """
    INSERT INTO seq_last (series_id, period_start, last_allocated)
    VALUES (?, ?, ?)
    ON CONFLICT(series_id) DO UPDATE SET
        last_allocated = seq_last.last_allocated + 1
    RETURNING last_allocated;
    """

    # Post 1
    v1 = c.execute(upsert_sql, ('s1', '2026-04-01', start_number)).fetchone()[0]
    print(f"Post 1 allocated: {v1}, table last_allocated: {c.execute('SELECT last_allocated FROM seq_last').fetchone()[0]}")
    assert v1 == 100

    # Post 2
    v2 = c.execute(upsert_sql, ('s1', '2026-04-01', start_number)).fetchone()[0]
    print(f"Post 2 allocated: {v2}, table last_allocated: {c.execute('SELECT last_allocated FROM seq_last').fetchone()[0]}")
    assert v2 == 101

    # What if a sequence row was created BEFORE post 1?
    conn2 = sqlite3.connect(":memory:")
    c2 = conn2.cursor()
    c2.execute("""
    CREATE TABLE seq_last (
        series_id TEXT PRIMARY KEY,
        period_start TEXT NOT NULL,
        last_allocated INTEGER NOT NULL
    );
    """)
    # Suppose someone pre-created the row with start_number:
    c2.execute("INSERT INTO seq_last VALUES ('s1', '2026-04-01', 100)")
    # Then Post 1 runs:
    v1_alt = c2.execute(upsert_sql, ('s1', '2026-04-01', start_number)).fetchone()[0]
    print(f"Pre-created with 100 -> Post 1 allocated: {v1_alt} (SKIPPED 100!)")

def test_representation_next_to_allocate():
    print("\n--- Testing Representation B: next_number_to_allocate ---")
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("""
    CREATE TABLE seq_next (
        series_id TEXT PRIMARY KEY,
        period_start TEXT NOT NULL,
        next_number INTEGER NOT NULL
    );
    """)

    start_number = 100

    # If table stores next_number:
    # On first insert: next_number = start_number + 1, allocated = start_number
    # On update: next_number = next_number + 1, allocated = old next_number
    # In SQLite UPSERT with RETURNING:
    # DO UPDATE SET next_number = seq_next.next_number + 1
    # RETURNING (seq_next.next_number - 1) as allocated
    # Wait, what does RETURNING return during INSERT vs UPDATE?
    # During INSERT: next_number in VALUES is (start_number + 1). So (next_number - 1) is start_number!
    # During UPDATE: next_number becomes old + 1. So (next_number - 1) is old!
    upsert_sql = """
    INSERT INTO seq_next (series_id, period_start, next_number)
    VALUES (?, ?, ?)
    ON CONFLICT(series_id) DO UPDATE SET
        next_number = seq_next.next_number + 1
    RETURNING (next_number - 1);
    """

    # Post 1
    v1 = c.execute(upsert_sql, ('s1', '2026-04-01', start_number + 1)).fetchone()[0]
    curr = c.execute('SELECT next_number FROM seq_next').fetchone()[0]
    print(f"Post 1 allocated: {v1}, table next_number: {curr}")
    assert v1 == 100
    assert curr == 101

    # Post 2
    v2 = c.execute(upsert_sql, ('s1', '2026-04-01', start_number + 1)).fetchone()[0]
    curr = c.execute('SELECT next_number FROM seq_next').fetchone()[0]
    print(f"Post 2 allocated: {v2}, table next_number: {curr}")
    assert v2 == 101
    assert curr == 102

    # What if a sequence row was pre-created before Post 1?
    conn2 = sqlite3.connect(":memory:")
    c2 = conn2.cursor()
    c2.execute("""
    CREATE TABLE seq_next (
        series_id TEXT PRIMARY KEY,
        period_start TEXT NOT NULL,
        next_number INTEGER NOT NULL
    );
    """)
    # Pre-created row with next_number = start_number (100)
    c2.execute("INSERT INTO seq_next VALUES ('s1', '2026-04-01', 100)")
    # Post 1 runs:
    v1_alt = c2.execute(upsert_sql, ('s1', '2026-04-01', start_number + 1)).fetchone()[0]
    curr_alt = c2.execute('SELECT next_number FROM seq_next').fetchone()[0]
    print(f"Pre-created with 100 -> Post 1 allocated: {v1_alt}, table next_number: {curr_alt}")
    assert v1_alt == 100
    assert curr_alt == 101
    print("Zero skipped! Pre-creation works perfectly under next_number representation!")

if __name__ == "__main__":
    test_representation_last_allocated()
    test_representation_next_to_allocate()

