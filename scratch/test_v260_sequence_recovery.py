import sqlite3
import json
import uuid

def setup_db():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()

    c.execute("""
    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('draft', 'posted', 'cancelled'))
    );
    """)

    c.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        period_start TEXT NOT NULL,
        current_number INTEGER NOT NULL CHECK(current_number >= 0),
        CONSTRAINT uq_sequence UNIQUE (company_id, voucher_type_id, series_name, period_start)
    );
    """)

    c.execute("""
    CREATE TABLE audit_log (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        action TEXT NOT NULL,
        details TEXT NOT NULL,
        performed_at TEXT NOT NULL
    );
    """)

    return conn

def resync_voucher_sequence(conn, company_id, voucher_type_id, series_name, period_start, operator_id="admin"):
    c = conn.cursor()
    # Step 1: Exclusive transaction
    c.execute("BEGIN IMMEDIATE;")
    try:
        # Step 2: Read current counter
        c.execute("""
            SELECT id, current_number FROM voucher_number_sequences
            WHERE company_id = ? AND voucher_type_id = ? AND series_name = ? AND period_start = ?
        """, (company_id, voucher_type_id, series_name, period_start))
        seq_row = c.fetchone()
        if not seq_row:
            # Initialize if not exists
            seq_id = f"seq_{company_id}_{series_name}_{period_start}"
            current_counter = 0
            c.execute("""
                INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_name, period_start, current_number)
                VALUES (?, ?, ?, ?, ?, 0)
            """, (seq_id, company_id, voucher_type_id, series_name, period_start))
        else:
            seq_id, current_counter = seq_row

        # Step 3: Inspect permanent allocations (status != 'draft')
        c.execute("""
            SELECT sequence_number FROM vouchers
            WHERE company_id = ? AND voucher_type_id = ? AND series_name = ? AND period_start = ?
              AND status != 'draft' AND sequence_number IS NOT NULL
            ORDER BY sequence_number ASC
        """, (company_id, voucher_type_id, series_name, period_start))
        rows = c.fetchall()
        allocations = [r[0] for r in rows]

        # Step 4: Compute max sequence number
        max_allocated = max(allocations) if allocations else 0

        # Step 5: Duplicate check
        seen = set()
        duplicates = []
        for s in allocations:
            if s in seen:
                duplicates.append(s)
            seen.add(s)
        if duplicates:
            raise ValueError(f"Integrity error: Duplicate sequence numbers detected: {duplicates}")

        # Step 6: Gap detection
        gaps = []
        if allocations:
            alloc_set = set(allocations)
            for expected in range(1, max_allocated + 1):
                if expected not in alloc_set:
                    gaps.append(expected)

        # Step 7: Sequence counter update policy
        # If counter is lagged (current_counter < max_allocated), advance it.
        # NEVER lower an ahead counter (if current_counter > max_allocated, keep current_counter).
        new_counter = current_counter
        updated = False
        if current_counter < max_allocated:
            new_counter = max_allocated
            c.execute("""
                UPDATE voucher_number_sequences
                SET current_number = ?
                WHERE id = ?
            """, (new_counter, seq_id))
            updated = True

        # Step 8: Log audit trail
        audit_details = {
            "voucher_type_id": voucher_type_id,
            "series_name": series_name,
            "period_start": period_start,
            "previous_counter": current_counter,
            "new_counter": new_counter,
            "max_allocated": max_allocated,
            "gaps_detected": gaps,
            "counter_updated": updated,
            "operator_id": operator_id
        }
        c.execute("""
            INSERT INTO audit_log (id, company_id, action, details, performed_at)
            VALUES (?, ?, ?, ?, datetime('now'))
        """, (str(uuid.uuid4()), company_id, "RESYNC_VOUCHER_SEQUENCE", json.dumps(audit_details)))

        # Step 9: Atomically commit
        c.execute("COMMIT;")
        return audit_details
    except Exception as e:
        try:
            c.execute("ROLLBACK;")
        except Exception as rb_err:
            # Rollback failed: connection must be disposed
            conn.close()
            raise RuntimeError(f"FATAL: Rollback failed: {rb_err}. Connection closed and disposed.") from e
        raise

def test_sequence_recovery():
    conn = setup_db()
    c = conn.cursor()

    # Case A: Lagged counter
    # Vouchers 1, 2, 3, 4 exist, but sequence counter is at 2.
    c.execute("INSERT INTO voucher_number_sequences VALUES ('seq1', 'c1', 'vt1', 'Main', '2026-04-01', 2);")
    c.execute("INSERT INTO vouchers VALUES ('v1', 'c1', 'vt1', 'Main', '2026-04-01', 1, 'INV/26-27/0001', 'posted');")
    c.execute("INSERT INTO vouchers VALUES ('v2', 'c1', 'vt1', 'Main', '2026-04-01', 2, 'INV/26-27/0002', 'posted');")
    c.execute("INSERT INTO vouchers VALUES ('v3', 'c1', 'vt1', 'Main', '2026-04-01', 3, 'INV/26-27/0003', 'cancelled');") # Cancelled counts!
    c.execute("INSERT INTO vouchers VALUES ('v4', 'c1', 'vt1', 'Main', '2026-04-01', 4, 'INV/26-27/0004', 'posted');")

    res = resync_voucher_sequence(conn, 'c1', 'vt1', 'Main', '2026-04-01')
    assert res['previous_counter'] == 2
    assert res['new_counter'] == 4
    assert res['max_allocated'] == 4
    assert res['counter_updated'] is True
    assert res['gaps_detected'] == []

    # Verify counter in DB
    c.execute("SELECT current_number FROM voucher_number_sequences WHERE id = 'seq1';")
    assert c.fetchone()[0] == 4

    # Case B: Correct counter (No-op)
    res_noop = resync_voucher_sequence(conn, 'c1', 'vt1', 'Main', '2026-04-01')
    assert res_noop['previous_counter'] == 4
    assert res_noop['new_counter'] == 4
    assert res_noop['counter_updated'] is False

    # Case C: Ahead counter (e.g. 6) - NEVER lower
    c.execute("UPDATE voucher_number_sequences SET current_number = 6 WHERE id = 'seq1';")
    res_ahead = resync_voucher_sequence(conn, 'c1', 'vt1', 'Main', '2026-04-01')
    assert res_ahead['previous_counter'] == 6
    assert res_ahead['new_counter'] == 6
    assert res_ahead['max_allocated'] == 4
    assert res_ahead['counter_updated'] is False, "Counter must NEVER be lowered!"

    # Case E: Gap detection
    # Insert voucher 6 (so vouchers are 1, 2, 3, 4, 6 - gap at 5)
    c.execute("INSERT INTO vouchers VALUES ('v6', 'c1', 'vt1', 'Main', '2026-04-01', 6, 'INV/26-27/0006', 'posted');")
    c.execute("UPDATE voucher_number_sequences SET current_number = 4 WHERE id = 'seq1';")
    res_gap = resync_voucher_sequence(conn, 'c1', 'vt1', 'Main', '2026-04-01')
    assert res_gap['previous_counter'] == 4
    assert res_gap['new_counter'] == 6
    assert res_gap['max_allocated'] == 6
    assert res_gap['gaps_detected'] == [5]

    # Case F: Duplicate detection
    c.execute("INSERT INTO vouchers VALUES ('v6_dup', 'c1', 'vt1', 'Main', '2026-04-01', 6, 'INV/26-27/0006_dup', 'posted');")
    caught_dup = False
    try:
        resync_voucher_sequence(conn, 'c1', 'vt1', 'Main', '2026-04-01')
    except ValueError as e:
        caught_dup = True
        assert "Duplicate sequence numbers detected: [6]" in str(e)
    assert caught_dup, "Duplicate sequence numbers must trigger integrity exception"

    print("Finding 4: Sequence Recovery Integrity & resync_voucher_sequence Tests Passed 100%!")

if __name__ == "__main__":
    test_sequence_recovery()
