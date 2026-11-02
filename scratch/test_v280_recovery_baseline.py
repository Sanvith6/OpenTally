import sqlite3
import json
import uuid

def setup_db():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    c.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
    );
    """)

    c.execute("""
    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        CONSTRAINT uq_vt_comp UNIQUE (company_id, id)
    );
    """)

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

    c.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        period_start TEXT NOT NULL,
        next_number INTEGER NOT NULL CHECK (next_number > 0),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (company_id, voucher_type_id, series_id) 
            REFERENCES voucher_number_series(company_id, voucher_type_id, id) ON DELETE RESTRICT,
        CONSTRAINT uq_seq_period UNIQUE (company_id, series_id, period_start)
    );
    """)

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

def resync_voucher_sequence_v280(conn, company_id, voucher_type_id, series_id, period_start,
                                 operator_id="admin", resolution_mode="strict", resolution_reason=None):
    c = conn.cursor()
    c.execute("BEGIN IMMEDIATE;")
    try:
        # Step 0: Verify series existence, active state, and numbering method
        c.execute("""
            SELECT series_name, series_code, numbering_method, is_active
            FROM voucher_number_series
            WHERE company_id = ? AND voucher_type_id = ? AND id = ?
        """, (company_id, voucher_type_id, series_id))
        series_row = c.fetchone()
        if not series_row:
            raise ValueError(f"Series '{series_id}' not found for company '{company_id}' and type '{voucher_type_id}'")
        s_name, s_code, method, is_active = series_row
        if method != 'automatic':
            raise ValueError(f"Cannot resynchronize sequence for non-automatic series '{s_name}' (method: '{method}')")

        # Step 1: Resolve the configured start_number baseline for this period
        # Query configuration effective as of period_start
        c.execute("""
            SELECT start_number
            FROM voucher_number_configs
            WHERE company_id = ? AND voucher_type_id = ? AND series_id = ? AND applicable_from <= ?
            ORDER BY applicable_from DESC LIMIT 1
        """, (company_id, voucher_type_id, series_id, period_start))
        cfg_row = c.fetchone()
        start_number = cfg_row[0] if cfg_row else 1

        # Step 2: Read current sequence counter (next_number)
        c.execute("""
            SELECT id, next_number FROM voucher_number_sequences
            WHERE company_id = ? AND series_id = ? AND period_start = ?
        """, (company_id, series_id, period_start))
        seq_row = c.fetchone()
        if not seq_row:
            seq_id = str(uuid.uuid4())
            current_next = start_number
            c.execute("""
                INSERT INTO voucher_number_sequences 
                (id, company_id, voucher_type_id, series_id, period_start, next_number)
                VALUES (?, ?, ?, ?, ?, ?)
            """, (seq_id, company_id, voucher_type_id, series_id, period_start, current_next))
        else:
            seq_id, current_next = seq_row

        # Step 3: Inspect permanent automatic vouchers (status != 'draft')
        c.execute("""
            SELECT id, sequence_number, period_start, status FROM vouchers
            WHERE company_id = ? AND series_id = ? AND numbering_method = 'automatic' AND status != 'draft'
              AND (period_start = ? OR period_start IS NULL)
            ORDER BY sequence_number ASC
        """, (company_id, series_id, period_start))
        vouchers = c.fetchall()

        # Step 4: Anomaly check - missing sequence metadata on permanent automatic vouchers
        missing_metadata_vouchers = [vid for vid, snum, pstart, _ in vouchers if snum is None or pstart is None]
        if missing_metadata_vouchers:
            raise ValueError(
                f"SequenceIntegrityAnomaly: Permanent automatic vouchers detected with NULL sequence metadata: {missing_metadata_vouchers}."
            )

        allocations = [v[1] for v in vouchers]

        # Step 5: Anomaly check - duplicate allocations
        seen = set()
        duplicates = []
        for s in allocations:
            if s in seen:
                duplicates.append(s)
            seen.add(s)
        if duplicates:
            raise ValueError(f"SequenceIntegrityAnomaly: Duplicate sequence numbers detected: {duplicates}.")

        # Step 6: Anomaly check - allocations below start_number
        below_start = [s for s in allocations if s < start_number]
        if below_start:
            raise ValueError(
                f"SequenceIntegrityAnomaly: Allocations detected below configured start_number ({start_number}): {below_start}."
            )

        # Step 7: Compute max sequence number and detect gaps relative to start_number
        max_allocated = max(allocations) if allocations else 0
        gaps = []
        if allocations:
            alloc_set = set(allocations)
            # Authoritative expected baseline: start_number .. max_allocated
            for expected in range(start_number, max_allocated + 1):
                if expected not in alloc_set:
                    gaps.append(expected)

        # Step 8: Enforce gap resolution policy
        if gaps and resolution_mode == "strict":
            raise ValueError(
                f"SequenceIntegrityAnomaly: Sequence gaps detected {gaps} within expected baseline {start_number}..{max_allocated}. "
                "Automatic recovery is strictly blocked; manual review or explicit 'allow_historical_gaps' policy required."
            )
        if gaps and resolution_mode == "allow_historical_gaps" and not resolution_reason:
            raise ValueError("allow_historical_gaps resolution mode requires a non-empty administrative resolution_reason.")

        # Step 9: Counter update policy (under next_number semantics)
        # Expected next number based on permanent vouchers:
        expected_next = (max_allocated + 1) if allocations else start_number
        updated = False
        new_next = current_next
        if current_next < expected_next:
            # Lagged counter: advance to expected_next
            new_next = expected_next
            c.execute("""
                UPDATE voucher_number_sequences
                SET next_number = ?, updated_at = CURRENT_TIMESTAMP
                WHERE id = ?
            """, (new_next, seq_id))
            updated = True
        # If current_next >= expected_next: NEVER LOWER COUNTER (keep current_next)

        # Step 10: Immutable audit log and atomic commit
        audit_details = {
            "voucher_type_id": voucher_type_id,
            "series_id": series_id,
            "period_start": period_start,
            "start_number_baseline": start_number,
            "previous_next_number": current_next,
            "new_next_number": new_next,
            "max_allocated": max_allocated,
            "total_permanent_allocations": len(allocations),
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
            pass
        raise e

def test_recovery_with_start_number_100():
    conn = setup_db()
    c = conn.cursor()

    # Setup company, type, series with start_number = 100
    c.execute("INSERT INTO companies VALUES ('comp_1', 'Test Corp');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_1', 'Sales');")
    c.execute("""
    INSERT INTO voucher_number_series 
    (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
    VALUES ('s_auto', 'comp_1', 'vt_sales', 'Primary', 'PRI', 'automatic');
    """)
    c.execute("""
    INSERT INTO voucher_number_configs
    (id, company_id, voucher_type_id, series_id, applicable_from, prefix, start_number, width, restart_period)
    VALUES ('cfg_1', 'comp_1', 'vt_sales', 's_auto', '2026-04-01', 'INV/PRI/', 100, 4, 'yearly');
    """)

    # Scenario 1: No previous allocations
    rep1 = resync_voucher_sequence_v280(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert rep1["start_number_baseline"] == 100
    assert rep1["previous_next_number"] == 100
    assert rep1["new_next_number"] == 100
    assert rep1["max_allocated"] == 0
    assert rep1["gaps_detected"] == []
    print("Scenario 1 (No allocations): next_number correctly seeded at 100 with zero gaps.")

    # Scenario 2: Consecutive allocations 100, 101 (posted), 102 (cancelled), 103 (reversed)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v100', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 100, 'INV/PRI/0100', '2026-05-01', 'posted');
    """)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v101', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 101, 'INV/PRI/0101', '2026-05-02', 'posted');
    """)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v102', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 102, 'INV/PRI/0102', '2026-05-03', 'cancelled');
    """)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v103', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 103, 'INV/PRI/0103', '2026-05-04', 'reversed');
    """)

    # Suppose counter in sequence table is lagged at 100
    c.execute("UPDATE voucher_number_sequences SET next_number = 100 WHERE series_id = 's_auto';")
    rep2 = resync_voucher_sequence_v280(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert rep2["start_number_baseline"] == 100
    assert rep2["previous_next_number"] == 100
    assert rep2["new_next_number"] == 104
    assert rep2["max_allocated"] == 103
    assert rep2["gaps_detected"] == []
    assert rep2["counter_updated"] is True
    print("Scenario 2 (Lagged counter with posted, cancelled, reversed): advanced next_number to 104 with zero gaps; numbers < 100 not treated as missing.")

    # Scenario 3: Ahead counter (e.g. 110)
    c.execute("UPDATE voucher_number_sequences SET next_number = 110 WHERE series_id = 's_auto';")
    rep3 = resync_voucher_sequence_v280(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert rep3["previous_next_number"] == 110
    assert rep3["new_next_number"] == 110
    assert rep3["counter_updated"] is False
    print("Scenario 3 (Ahead counter): preserved next_number at 110 without lowering.")

    # Scenario 4: Sequence gap (e.g. insert 105, leaving 104 missing)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status)
    VALUES ('v105', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 105, 'INV/PRI/0105', '2026-05-05', 'posted');
    """)
    caught_gap = False
    try:
        resync_voucher_sequence_v280(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01', resolution_mode="strict")
    except ValueError as e:
        caught_gap = True
        assert "[104]" in str(e)
    assert caught_gap, "Strict mode must reject gap [104]"
    print("Scenario 4 (Gap detection in strict mode): successfully detected and blocked on missing [104].")

    # Scenario 5: Administrative override of historical gap
    rep5 = resync_voucher_sequence_v280(
        conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01',
        resolution_mode="allow_historical_gaps",
        resolution_reason="Court order cancelled 104 before import"
    )
    assert rep5["gaps_detected"] == [104]
    assert rep5["new_next_number"] == 110 # Ahead counter preserved
    print("Scenario 5 (Administrative override with reason): successfully logged gap [104] and allowed resumption.")

    print("\nALL RECOVERY BASELINE TESTS PASSED 100%!")

if __name__ == "__main__":
    test_recovery_with_start_number_100()

