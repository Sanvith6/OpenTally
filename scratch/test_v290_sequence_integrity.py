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
        name TEXT NOT NULL,
        financial_year_begins TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL
    );
    """)

    c.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_voucher_types_company UNIQUE (company_id, id),
        CONSTRAINT uq_voucher_types_name UNIQUE (company_id, name)
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
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        effective_date TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        reference_no TEXT,
        reference_date TEXT,
        narration TEXT,
        total_amount TEXT,
        reversal_of_voucher_id TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) 
            REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
        CONSTRAINT chk_voucher_date CHECK (
            date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            AND date(date) IS NOT NULL
            AND date(date) = date
        ),
        CONSTRAINT chk_effective_date CHECK (
            effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
            AND date(effective_date) IS NOT NULL
            AND date(effective_date) = effective_date
        ),
        CONSTRAINT chk_voucher_sequence_metadata CHECK (
            (
                status = 'draft'
                AND (
                    (sequence_number IS NULL AND period_start IS NULL)
                    OR
                    (
                        sequence_number > 0
                        AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                        AND date(period_start) IS NOT NULL
                        AND date(period_start) = period_start
                    )
                )
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND numbering_method = 'automatic'
                AND period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                AND date(period_start) IS NOT NULL
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

    # Triggers protecting immutability of posted vouchers
    c.execute("""
    CREATE TRIGGER trg_prevent_posted_voucher_mutation
    BEFORE UPDATE ON vouchers
    FOR EACH ROW
    WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
    BEGIN
        SELECT CASE
            WHEN NEW.company_id != OLD.company_id
                THEN RAISE(ABORT, 'Cannot mutate company_id of a permanent voucher')
            WHEN NEW.voucher_type_id != OLD.voucher_type_id
                THEN RAISE(ABORT, 'Cannot mutate voucher_type_id of a permanent voucher')
            WHEN NEW.series_id != OLD.series_id
                THEN RAISE(ABORT, 'Cannot mutate series_id of a permanent voucher')
            WHEN NEW.numbering_method != OLD.numbering_method
                THEN RAISE(ABORT, 'Cannot mutate numbering_method of a permanent voucher')
            WHEN (NEW.period_start IS NOT OLD.period_start)
                THEN RAISE(ABORT, 'Cannot mutate period_start of a permanent voucher')
            WHEN (NEW.sequence_number IS NOT OLD.sequence_number)
                THEN RAISE(ABORT, 'Cannot mutate sequence_number of a permanent voucher')
            WHEN NEW.voucher_number != OLD.voucher_number
                THEN RAISE(ABORT, 'Cannot mutate voucher_number of a permanent voucher')
            WHEN NEW.date != OLD.date
                THEN RAISE(ABORT, 'Cannot mutate date of a permanent voucher')
            WHEN NEW.total_amount != OLD.total_amount
                THEN RAISE(ABORT, 'Cannot mutate total_amount of a permanent voucher')
        END;
    END;
    """)

    c.execute("""
    CREATE TRIGGER trg_prevent_posted_voucher_deletion
    BEFORE DELETE ON vouchers
    FOR EACH ROW
    WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
    BEGIN
        SELECT RAISE(ABORT, 'Cannot delete a permanent voucher (posted, cancelled, or reversed). Use cancellation or reversal.');
    END;
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

def derive_period_start(voucher_date, restart_period, fy_begins, books_beginning_from):
    v_parts = voucher_date.split('-')
    if len(v_parts) != 3:
        raise ValueError(f"Invalid voucher date: {voucher_date}")
    v_y, v_m, v_d = int(v_parts[0]), int(v_parts[1]), int(v_parts[2])

    if restart_period == "yearly":
        fy_parts = fy_begins.split('-')
        fy_m, fy_d = int(fy_parts[1]), int(fy_parts[2])
        is_leap = lambda y: (y % 4 == 0 and y % 100 != 0) or (y % 400 == 0)
        if fy_m == 2 and fy_d == 29 and not is_leap(v_y):
            fy_d = 28
        start_year = v_y if (v_m, v_d) >= (fy_m, fy_d) else v_y - 1
        return f"{start_year:04d}-{fy_m:02d}-{fy_d:02d}"
    elif restart_period == "monthly":
        return f"{v_y:04d}-{v_m:02d}-01"
    elif restart_period == "daily":
        return voucher_date
    elif restart_period == "never":
        return books_beginning_from
    else:
        raise ValueError(f"Unsupported restart period: {restart_period}")

def resync_voucher_sequence_v290(conn, company_id, voucher_type_id, series_id, period_start,
                                 operator_id="admin", resolution_mode="strict", resolution_reason=None):
    c = conn.cursor()
    c.execute("BEGIN IMMEDIATE;")
    try:
        # Step 0: Series verification
        c.execute("""
            SELECT series_name, series_code, numbering_method, is_active
            FROM voucher_number_series
            WHERE company_id = ? AND voucher_type_id = ? AND id = ?
        """, (company_id, voucher_type_id, series_id))
        series_row = c.fetchone()
        if not series_row:
            raise ValueError(f"Series '{series_id}' not found")
        s_name, s_code, method, is_active = series_row
        if method != 'automatic':
            raise ValueError(f"Cannot resync sequence for non-automatic series '{s_name}'")

        # Step 1: Strictly require effective configuration (NO SILENT DEFAULT)
        c.execute("""
            SELECT start_number
            FROM voucher_number_configs
            WHERE company_id = ? AND voucher_type_id = ? AND series_id = ? AND applicable_from <= ?
            ORDER BY applicable_from DESC LIMIT 1
        """, (company_id, voucher_type_id, series_id, period_start))
        cfg_row = c.fetchone()
        if not cfg_row:
            raise ValueError(
                f"MissingConfigurationError: Missing numbering configuration for series '{s_name}' "
                f"applicable as of period start {period_start}. Silent fallback to start_number = 1 is prohibited."
            )
        start_number = cfg_row[0]

        # Step 2: Read current next_number (N)
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

        # Step 3: Inspect permanent automatic vouchers (status != 'draft', numbering_method = 'automatic')
        # Includes: posted, cancelled, reversed. Excludes: draft and manual.
        c.execute("""
            SELECT id, sequence_number, period_start, status FROM vouchers
            WHERE company_id = ? AND series_id = ? AND numbering_method = 'automatic' AND status != 'draft'
              AND (period_start = ? OR period_start IS NULL)
            ORDER BY sequence_number ASC
        """, (company_id, series_id, period_start))
        vouchers = c.fetchall()

        # Step 4: Missing metadata anomaly
        missing_metadata = [v[0] for v in vouchers if v[1] is None or v[2] is None]
        if missing_metadata:
            raise ValueError(f"SequenceIntegrityAnomaly: Permanent automatic vouchers detected with NULL metadata: {missing_metadata}")

        allocations = [v[1] for v in vouchers]

        # Step 5: Duplicate anomaly
        seen = set()
        duplicates = []
        for s in allocations:
            if s in seen:
                duplicates.append(s)
            seen.add(s)
        if duplicates:
            raise ValueError(f"SequenceIntegrityAnomaly: Duplicate sequence numbers detected: {duplicates}")

        # Step 6: Below-baseline anomaly
        below_start = [s for s in allocations if s < start_number]
        if below_start:
            raise ValueError(f"SequenceIntegrityAnomaly: Allocations detected below configured start_number ({start_number}): {below_start}")

        # Step 7: Compute max allocated M and detect historical gaps in [S_0, M]
        max_allocated = max(allocations) if allocations else 0
        historical_gaps = []
        if allocations:
            alloc_set = set(allocations)
            for expected in range(start_number, max_allocated + 1):
                if expected not in alloc_set:
                    historical_gaps.append(expected)

        # Step 8: Detect Counter-Tail Gaps in [M + 1, N - 1] when N > M + 1 (or [S_0, N - 1] if M = 0)
        counter_tail_gaps = []
        expected_next = (max_allocated + 1) if allocations else start_number
        if current_next > expected_next:
            # The counter is ahead of the maximum permanent allocation!
            # The tail interval from expected_next to current_next - 1 has NO permanent allocations.
            counter_tail_gaps = list(range(expected_next, current_next))

        all_gaps = historical_gaps + counter_tail_gaps

        # Enforce gap policy across both historical gaps and counter-tail gaps
        if all_gaps and resolution_mode == "strict":
            raise ValueError(
                f"SequenceIntegrityAnomaly: Gaps detected: historical_gaps={historical_gaps}, counter_tail_gaps={counter_tail_gaps}. "
                f"Automatic recovery strictly blocked; manual review or explicit 'allow_historical_gaps' policy required."
            )
        if all_gaps and resolution_mode == "allow_historical_gaps" and not resolution_reason:
            raise ValueError("allow_historical_gaps resolution mode requires a non-empty administrative resolution_reason.")

        # Step 9: Counter update policy under next_number semantics
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
        # If current_next >= expected_next: NEVER LOWER COUNTER (preserve current_next)

        # Step 10: Audit log and commit
        audit_details = {
            "voucher_type_id": voucher_type_id,
            "series_id": series_id,
            "period_start": period_start,
            "start_number_baseline": start_number,
            "previous_next_number": current_next,
            "new_next_number": new_next,
            "max_allocated": max_allocated,
            "total_permanent_allocations": len(allocations),
            "historical_gaps": historical_gaps,
            "counter_tail_gaps": counter_tail_gaps,
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

def run_tests():
    conn = setup_db()
    c = conn.cursor()

    # Seed company and voucher type
    c.execute("INSERT INTO companies VALUES ('comp_1', 'Acme Corp', '2026-04-01', '2026-04-01');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_1', 'Sales');")
    c.execute("""
    INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
    VALUES ('s_auto', 'comp_1', 'vt_sales', 'Primary', 'PRI', 'automatic');
    """)

    # Test 1: Missing Configuration Rejection
    try:
        resync_voucher_sequence_v290(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
        assert False, "Should have rejected missing configuration"
    except ValueError as e:
        assert "MissingConfigurationError" in str(e)
        print("PASS Finding 1: Missing configuration strictly rejected; zero silent fallback to 1.")

    # Seed configuration with start_number = 100
    c.execute("""
    INSERT INTO voucher_number_configs 
    (id, company_id, voucher_type_id, series_id, applicable_from, prefix, start_number, width, restart_period)
    VALUES ('cfg_1', 'comp_1', 'vt_sales', 's_auto', '2026-04-01', 'INV/PRI/', 100, 4, 'yearly');
    """)

    # Test 2: Example A - Normal Aligned Counter
    # Pre-create sequence at 100, insert voucher 100
    c.execute("INSERT INTO voucher_number_sequences VALUES ('seq_1', 'comp_1', 'vt_sales', 's_auto', '2026-04-01', 101, 'now', 'now');")
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
    VALUES ('v1', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 100, 'INV/PRI/0100', '2026-05-01', '2026-05-01', 'posted');
    """)
    res_a = resync_voucher_sequence_v290(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert res_a['new_next_number'] == 101
    assert res_a['historical_gaps'] == []
    assert res_a['counter_tail_gaps'] == []
    assert res_a['counter_updated'] is False
    print("PASS Finding 2 (Example A): Normal aligned counter verified.")

    # Test 3: Example B - Lagging Counter
    # Add vouchers 101, 102, 103, but set sequence table next_number = 102
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
    VALUES 
      ('v2', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 101, 'INV/PRI/0101', '2026-05-02', '2026-05-02', 'posted'),
      ('v3', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 102, 'INV/PRI/0102', '2026-05-03', '2026-05-03', 'cancelled'),
      ('v4', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 103, 'INV/PRI/0103', '2026-05-04', '2026-05-04', 'posted');
    """)
    c.execute("UPDATE voucher_number_sequences SET next_number = 102 WHERE id = 'seq_1';")
    res_b = resync_voucher_sequence_v290(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert res_b['new_next_number'] == 104
    assert res_b['counter_updated'] is True
    print("PASS Finding 2 (Example B): Lagging counter advanced to M + 1 (104).")

    # Test 4: Example C - Ahead Counter Explained by Cancelled/Reversed Vouchers
    # Vouchers: 100 (posted), 101 (posted), 102 (cancelled), 103 (posted), 104 (cancelled), 105 (reversed)
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
    VALUES 
      ('v5', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 104, 'INV/PRI/0104', '2026-05-05', '2026-05-05', 'cancelled'),
      ('v6', 'comp_1', 'vt_sales', 's_auto', 'automatic', '2026-04-01', 105, 'INV/PRI/0105', '2026-05-06', '2026-05-06', 'reversed');
    """)
    c.execute("UPDATE voucher_number_sequences SET next_number = 106 WHERE id = 'seq_1';")
    res_c = resync_voucher_sequence_v290(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01')
    assert res_c['max_allocated'] == 105
    assert res_c['new_next_number'] == 106
    assert res_c['historical_gaps'] == []
    assert res_c['counter_tail_gaps'] == []
    print("PASS Finding 2 (Example C): Cancelled/reversed vouchers included as evidence; tail empty; counter aligned at 106.")

    # Test 5: Example D - Ahead Counter with Unexplained Missing Allocations (Counter-Tail Gaps)
    # Suppose someone manually or errantly updated next_number = 110!
    # Expected next is 106, so [106, 107, 108, 109] are unexplained counter-tail gaps.
    c.execute("UPDATE voucher_number_sequences SET next_number = 110 WHERE id = 'seq_1';")
    try:
        resync_voucher_sequence_v290(conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01', resolution_mode="strict")
        assert False, "Strict mode should abort on counter-tail gaps"
    except ValueError as e:
        assert "counter_tail_gaps=[106, 107, 108, 109]" in str(e)
        print("PASS Finding 2 (Example D - Strict): Strict mode rejected unexplained counter-tail gaps [106..109].")

    # In allow_historical_gaps mode with reason:
    res_d = resync_voucher_sequence_v290(
        conn, 'comp_1', 'vt_sales', 's_auto', '2026-04-01',
        resolution_mode="allow_historical_gaps",
        resolution_reason="Administrative counter advancement during system migration audit"
    )
    assert res_d['counter_tail_gaps'] == [106, 107, 108, 109]
    assert res_d['new_next_number'] == 110  # Preserved! Never lowered!
    assert res_d['resolution_reason'] == "Administrative counter advancement during system migration audit"
    print("PASS Finding 2 (Example D - Resolved): Counter preserved at 110 (never lowered); tail gaps logged in audit trail.")

    # Test 6: Period/Date Consistency Helper (Finding 3)
    p_yearly = derive_period_start("2026-08-15", "yearly", "2026-04-01", "2026-04-01")
    assert p_yearly == "2026-04-01"
    p_yearly_cal = derive_period_start("2026-08-15", "yearly", "2026-01-01", "2026-01-01")
    assert p_yearly_cal == "2026-01-01"
    p_monthly = derive_period_start("2026-08-15", "monthly", "2026-04-01", "2026-04-01")
    assert p_monthly == "2026-08-01"
    p_daily = derive_period_start("2026-08-15", "daily", "2026-04-01", "2026-04-01")
    assert p_daily == "2026-08-15"
    p_never = derive_period_start("2026-08-15", "never", "2026-04-01", "2026-04-01")
    assert p_never == "2026-04-01"
    print("PASS Finding 3: Period/date consistency helper verified across yearly, monthly, daily, and never resets.")

    # Test 7: Immutability Triggers (Finding 4)
    # Attempt to mutate posted voucher v1
    try:
        c.execute("UPDATE vouchers SET voucher_number = 'MUTATED' WHERE id = 'v1';")
        assert False, "Should have blocked mutating voucher_number on posted voucher"
    except sqlite3.IntegrityError as e:
        assert "Cannot mutate voucher_number" in str(e)
        print("PASS Finding 4: Trigger prevented mutating voucher_number on posted voucher.")

    try:
        c.execute("UPDATE vouchers SET series_id = 'other_series' WHERE id = 'v1';")
        assert False, "Should have blocked mutating series_id on posted voucher"
    except sqlite3.IntegrityError as e:
        assert "Cannot mutate series_id" in str(e)
        print("PASS Finding 4: Trigger prevented mutating series_id on posted voucher.")

    try:
        c.execute("UPDATE vouchers SET date = '2026-09-01' WHERE id = 'v1';")
        assert False, "Should have blocked mutating date on posted voucher"
    except sqlite3.IntegrityError as e:
        assert "Cannot mutate date" in str(e)
        print("PASS Finding 4: Trigger prevented mutating date on posted voucher.")

    try:
        c.execute("DELETE FROM vouchers WHERE id = 'v1';")
        assert False, "Should have blocked deleting posted voucher"
    except sqlite3.IntegrityError as e:
        assert "Cannot delete a permanent voucher" in str(e)
        print("PASS Finding 4: Trigger prevented deleting posted voucher.")

    print("\nALL V2.9.0 TEST SCENARIOS PASSED 100%!")

if __name__ == "__main__":
    run_tests()

