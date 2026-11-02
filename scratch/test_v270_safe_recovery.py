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
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL,
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('draft', 'posted', 'cancelled', 'reversed'))
    );
    """)

    c.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        period_start TEXT NOT NULL,
        current_number INTEGER NOT NULL CHECK(current_number >= 0),
        CONSTRAINT uq_sequence UNIQUE (company_id, series_id, period_start)
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

def resync_voucher_sequence(conn, company_id, voucher_type_id, series_id, period_start,
                            operator_id="admin", resolution_mode="strict", resolution_reason=None):
    c = conn.cursor()
    c.execute("BEGIN IMMEDIATE;")
    try:
        # Step 1: Read current sequence counter
        c.execute("""
            SELECT id, current_number FROM voucher_number_sequences
            WHERE company_id = ? AND series_id = ? AND period_start = ?
        """, (company_id, series_id, period_start))
        seq_row = c.fetchone()
        if not seq_row:
            seq_id = f"seq_{company_id}_{series_id}_{period_start}"
            current_counter = 0
            c.execute("""
                INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, current_number)
                VALUES (?, ?, ?, ?, ?, 0)
            """, (seq_id, company_id, voucher_type_id, series_id, period_start))
        else:
            seq_id, current_counter = seq_row

        # Step 2: Inspect permanent vouchers (status != 'draft')
        c.execute("""
            SELECT id, sequence_number, period_start FROM vouchers
            WHERE company_id = ? AND series_id = ? AND status != 'draft'
              AND (period_start = ? OR period_start IS NULL)
            ORDER BY sequence_number ASC
        """, (company_id, series_id, period_start))
        vouchers = c.fetchall()

        # Step 3: Anomaly check - missing sequence metadata on permanent automatic vouchers
        missing_metadata_vouchers = []
        for vid, snum, pstart in vouchers:
            if snum is None or pstart is None:
                missing_metadata_vouchers.append(vid)

        if missing_metadata_vouchers:
            raise ValueError(
                f"SequenceIntegrityAnomaly: Permanent vouchers detected with NULL sequence metadata: {missing_metadata_vouchers}. "
                "Automatic recovery is strictly blocked; manual database administrative review required."
            )

        allocations = [v[1] for v in vouchers]

        # Step 4: Anomaly check - duplicate allocations
        seen = set()
        duplicates = []
        for s in allocations:
            if s in seen:
                duplicates.append(s)
            seen.add(s)
        if duplicates:
            raise ValueError(
                f"SequenceIntegrityAnomaly: Duplicate sequence numbers detected: {duplicates}. "
                "Automatic recovery is strictly blocked; manual database administrative review required."
            )

        # Step 5: Compute max sequence number
        max_allocated = max(allocations) if allocations else 0

        # Step 6: Gap detection
        gaps = []
        if allocations:
            alloc_set = set(allocations)
            for expected in range(1, max_allocated + 1):
                if expected not in alloc_set:
                    gaps.append(expected)

        # Step 7: Gap handling policy
        if gaps and resolution_mode == "strict":
            raise ValueError(
                f"SequenceIntegrityAnomaly: Sequence gaps detected {gaps}. Normal posting cannot automatically resume without "
                "explicit administrative resolution policy (mode 'allow_historical_gaps' with administrative reason)."
            )

        # Step 8: Update policy
        # If counter is lagged (current_counter < max_allocated), advance it.
        # NEVER lower an ahead counter.
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

        # Step 9: Audit log
        audit_details = {
            "voucher_type_id": voucher_type_id,
            "series_id": series_id,
            "period_start": period_start,
            "previous_counter": current_counter,
            "new_counter": new_counter,
            "max_allocated": max_allocated,
            "gaps_detected": gaps,
            "counter_updated": updated,
            "operator_id": operator_id,
            "resolution_mode": resolution_mode,
            "resolution_reason": resolution_reason
        }
        c.execute("""
            INSERT INTO audit_log (id, company_id, action, details, performed_at)
            VALUES (?, ?, ?, ?, datetime('now'))
        """, (str(uuid.uuid4()), company_id, "RESYNC_VOUCHER_SEQUENCE", json.dumps(audit_details)))

        c.execute("COMMIT;")
        return audit_details
    except Exception as e:
        try:
            c.execute("ROLLBACK;")
        except Exception:
            conn.close()
        raise

def test_safe_recovery():
    conn = setup_db()
    c = conn.cursor()

    # 1. Consistent Lagged Counter (C=2, Vouchers 1, 2, 3, 4 exist)
    c.execute("INSERT INTO voucher_number_sequences VALUES ('seq1', 'c1', 'vt1', 's1', '2026-04-01', 2);")
    c.execute("INSERT INTO vouchers VALUES ('v1', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 1, 'SALES/PRI/0001', 'posted');")
    c.execute("INSERT INTO vouchers VALUES ('v2', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 2, 'SALES/PRI/0002', 'posted');")
    c.execute("INSERT INTO vouchers VALUES ('v3', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 3, 'SALES/PRI/0003', 'cancelled');")
    c.execute("INSERT INTO vouchers VALUES ('v4', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 4, 'SALES/PRI/0004', 'posted');")

    res1 = resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01')
    assert res1['previous_counter'] == 2
    assert res1['new_counter'] == 4
    assert res1['counter_updated'] is True
    assert res1['gaps_detected'] == []

    # 2. Ahead Counter (C=6, Vouchers 1..4) -> NEVER lowered
    c.execute("UPDATE voucher_number_sequences SET current_number = 6 WHERE id = 'seq1';")
    res2 = resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01')
    assert res2['previous_counter'] == 6
    assert res2['new_counter'] == 6
    assert res2['counter_updated'] is False

    # 3. Missing Metadata Anomaly (Permanent voucher with sequence_number = NULL)
    c.execute("INSERT INTO vouchers VALUES ('v_bad', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', NULL, 'SALES/PRI/BAD', 'posted');")
    caught_null = False
    try:
        resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01')
    except ValueError as e:
        caught_null = True
        assert "Permanent vouchers detected with NULL sequence metadata" in str(e)
    assert caught_null, "Must abort on missing sequence metadata"
    c.execute("DELETE FROM vouchers WHERE id = 'v_bad';")

    # 4. Duplicate Allocations Anomaly
    c.execute("INSERT INTO vouchers VALUES ('v4_dup', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 4, 'SALES/PRI/0004_DUP', 'posted');")
    caught_dup = False
    try:
        resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01')
    except ValueError as e:
        caught_dup = True
        assert "Duplicate sequence numbers detected: [4]" in str(e)
    assert caught_dup, "Must abort on duplicate sequence numbers"
    c.execute("DELETE FROM vouchers WHERE id = 'v4_dup';")

    # 5. Gap Detection & Strict Mode Rejection
    # Add voucher 6, leaving gap at 5
    c.execute("INSERT INTO vouchers VALUES ('v6', 'c1', 'vt1', 's1', 'automatic', '2026-04-01', 6, 'SALES/PRI/0006', 'posted');")
    c.execute("UPDATE voucher_number_sequences SET current_number = 4 WHERE id = 'seq1';")
    caught_gap = False
    try:
        resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01', resolution_mode="strict")
    except ValueError as e:
        caught_gap = True
        assert "Sequence gaps detected [5]" in str(e)
    assert caught_gap, "Strict mode must block automatic resumption when gaps exist"

    # 6. Explicit Administrative Override Mode with Reason
    res_override = resync_voucher_sequence(conn, 'c1', 'vt1', 's1', '2026-04-01',
                                          operator_id="admin_user",
                                          resolution_mode="allow_historical_gaps",
                                          resolution_reason="Gap 5 was an external cancelled test voucher")
    assert res_override['new_counter'] == 6
    assert res_override['gaps_detected'] == [5]
    assert res_override['resolution_mode'] == "allow_historical_gaps"

    print("Finding 4: Safe Sequence Recovery Tests Passed 100%!")

if __name__ == "__main__":
    test_safe_recovery()

