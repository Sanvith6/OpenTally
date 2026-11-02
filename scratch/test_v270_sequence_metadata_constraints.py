import sqlite3

def test_sequence_metadata_constraints():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()

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
        total_amount TEXT,
        CONSTRAINT chk_voucher_sequence_metadata CHECK (
            (
                status = 'draft'
                AND (
                    (sequence_number IS NULL AND period_start IS NULL)
                    OR
                    (sequence_number > 0 AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]')
                )
            )
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
                AND numbering_method = 'automatic'
                AND period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
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
    CREATE UNIQUE INDEX uq_vouchers_series_sequence
    ON vouchers(company_id, series_id, period_start, sequence_number)
    WHERE status != 'draft' AND sequence_number IS NOT NULL;
    """)

    c.execute("""
    CREATE UNIQUE INDEX uq_vouchers_permanent_number
    ON vouchers(company_id, voucher_type_id, voucher_number)
    WHERE status != 'draft';
    """)

    # 1. Draft voucher with NULL sequence metadata passes
    c.execute("""
    INSERT INTO vouchers VALUES ('d1', 'c1', 'vt1', 's_auto', 'automatic', NULL, NULL, 'DRAFT-uuid-1', '2026-05-10', 'draft', NULL);
    """)

    # 2. Posted automatic voucher WITHOUT sequence_number FAILS
    caught_null_seq = False
    try:
        c.execute("""
        INSERT INTO vouchers VALUES ('v_bad1', 'c1', 'vt1', 's_auto', 'automatic', '2026-04-01', NULL, 'INV/PRI/0001', '2026-05-10', 'posted', '500.00');
        """)
    except sqlite3.IntegrityError as e:
        caught_null_seq = True
        assert "CHECK constraint failed" in str(e)
    assert caught_null_seq, "Posted automatic voucher without sequence_number must fail check constraint"

    # 3. Posted automatic voucher WITHOUT period_start FAILS
    caught_null_period = False
    try:
        c.execute("""
        INSERT INTO vouchers VALUES ('v_bad2', 'c1', 'vt1', 's_auto', 'automatic', NULL, 1, 'INV/PRI/0001', '2026-05-10', 'posted', '500.00');
        """)
    except sqlite3.IntegrityError as e:
        caught_null_period = True
        assert "CHECK constraint failed" in str(e)
    assert caught_null_period, "Posted automatic voucher without period_start must fail check constraint"

    # 4. Posted automatic voucher with valid period_start and positive sequence_number PASSES
    c.execute("""
    INSERT INTO vouchers VALUES ('v1', 'c1', 'vt1', 's_auto', 'automatic', '2026-04-01', 1, 'INV/PRI/26-27/0001', '2026-05-10', 'posted', '500.00');
    """)

    # 5. Cancelled and reversed automatic vouchers PRESERVE original sequence metadata
    c.execute("""
    INSERT INTO vouchers VALUES ('v2', 'c1', 'vt1', 's_auto', 'automatic', '2026-04-01', 2, 'INV/PRI/26-27/0002', '2026-05-11', 'cancelled', '600.00');
    """)
    c.execute("""
    INSERT INTO vouchers VALUES ('v3', 'c1', 'vt1', 's_auto', 'automatic', '2026-04-01', 3, 'INV/PRI/26-27/0003', '2026-05-12', 'reversed', '700.00');
    """)

    # Attempting to set sequence_number to NULL on cancelled voucher FAILS
    caught_cancel_null = False
    try:
        c.execute("UPDATE vouchers SET sequence_number = NULL WHERE id = 'v2';")
    except sqlite3.IntegrityError as e:
        caught_cancel_null = True
        assert "CHECK constraint failed" in str(e)
    assert caught_cancel_null, "Cancelled automatic voucher cannot strip sequence_number"

    # 6. Manual voucher with NULL sequence_number PASSES
    c.execute("""
    INSERT INTO vouchers VALUES ('v_man1', 'c1', 'vt1', 's_man', 'manual', NULL, NULL, 'SALES/MAN/OFFLINE-01', '2026-05-15', 'posted', '1200.00');
    """)

    # 7. Manual voucher with positive sequence_number FAILS (enforcing pure manual model)
    caught_man_seq = False
    try:
        c.execute("""
        INSERT INTO vouchers VALUES ('v_man_bad', 'c1', 'vt1', 's_man', 'manual', '2026-04-01', 1, 'SALES/MAN/OFFLINE-02', '2026-05-15', 'posted', '1200.00');
        """)
    except sqlite3.IntegrityError as e:
        caught_man_seq = True
        assert "CHECK constraint failed" in str(e)
    assert caught_man_seq, "Manual voucher cannot specify sequence_number under pure manual model"

    # 8. Duplicate sequence_number in same series and period_start FAILS
    caught_dup_seq = False
    try:
        c.execute("""
        INSERT INTO vouchers VALUES ('v_dup', 'c1', 'vt1', 's_auto', 'automatic', '2026-04-01', 1, 'INV/PRI/26-27/0001_DUP', '2026-05-10', 'posted', '500.00');
        """)
    except sqlite3.IntegrityError as e:
        caught_dup_seq = True
        assert "UNIQUE constraint failed" in str(e)
    assert caught_dup_seq, "Duplicate sequence_number in same series and period_start must fail uq_vouchers_series_sequence"

    print("Finding 2: Enforce Permanent Voucher Sequence Metadata Tests Passed 100%!")

if __name__ == "__main__":
    test_sequence_metadata_constraints()

