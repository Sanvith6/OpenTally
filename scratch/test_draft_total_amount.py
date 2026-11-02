import sqlite3

def test_draft_lifecycle():
    conn = sqlite3.connect(":memory:")
    cur = conn.cursor()
    cur.execute("PRAGMA foreign_keys = ON;")

    cur.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        voucher_number TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        total_amount TEXT,
        CONSTRAINT chk_voucher_total_amount_canonical CHECK (
            (
                status = 'draft' 
                AND (
                    total_amount IS NULL
                    OR
                    (
                        length(total_amount) >= 1
                        AND length(total_amount) <= 25
                        AND total_amount NOT GLOB '*[^0-9.]*'
                        AND total_amount NOT GLOB '*..*'
                        AND total_amount NOT GLOB '.*'
                        AND total_amount NOT GLOB '*.'
                        AND (
                            (
                                instr(total_amount, '.') > 0
                                AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                                AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                                AND (
                                    (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                                     AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                                    OR
                                    (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                                )
                            )
                            OR
                            (
                                instr(total_amount, '.') = 0
                                AND total_amount GLOB '[1-9]*'
                            )
                        )
                    )
                )
            )
            OR
            (
                status IN ('posted', 'reversed', 'cancelled')
                AND total_amount IS NOT NULL
                AND length(total_amount) >= 1
                AND length(total_amount) <= 25
                AND total_amount NOT GLOB '*[^0-9.]*'
                AND total_amount NOT GLOB '*..*'
                AND total_amount NOT GLOB '.*'
                AND total_amount NOT GLOB '*.'
                AND (
                    (
                        instr(total_amount, '.') > 0
                        AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                        AND instr(substr(total_amount, instr(total_amount, '.') + 1), '.') = 0
                        AND (
                            (substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' 
                             AND substr(total_amount, instr(total_amount, '.') + 1) GLOB '*[1-9]*')
                            OR
                            (substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')
                        )
                    )
                    OR
                    (
                        instr(total_amount, '.') = 0
                        AND total_amount GLOB '[1-9]*'
                    )
                )
            )
        )
    );
    """)

    # 1. Draft with NULL total
    cur.execute("INSERT INTO vouchers VALUES ('v1', 'c1', 'vt1', 'DRAFT-01', 'draft', NULL)")
    print("Test 1: Draft with NULL total -> PASSED")

    # 2. Draft with valid positive total
    cur.execute("INSERT INTO vouchers VALUES ('v2', 'c1', 'vt1', 'DRAFT-02', 'draft', '1500.50')")
    print("Test 2: Draft with valid positive total -> PASSED")

    # 3. Draft with invalid zero or malformed total -> must be REJECTED
    for bad in ['0', '0.00', '01.00', '-10.00', '1abc', '..']:
        try:
            cur.execute("INSERT INTO vouchers VALUES (?, 'c1', 'vt1', 'DRAFT-bad', 'draft', ?)", (f'v_bad_{bad}', bad))
            assert False, f"Draft with bad total {bad} should be rejected!"
        except sqlite3.IntegrityError:
            pass
    print("Test 3: Draft with invalid/malformed total rejected -> PASSED")

    # 4. Posting with NULL total -> must be REJECTED
    try:
        cur.execute("INSERT INTO vouchers VALUES ('v4', 'c1', 'vt1', 'INV-0001', 'posted', NULL)")
        assert False, "Posting with NULL total must be rejected!"
    except sqlite3.IntegrityError:
        pass
    print("Test 4: Posting with NULL total rejected -> PASSED")

    # 5. Posting with valid positive total -> must SUCCEED
    cur.execute("INSERT INTO vouchers VALUES ('v5', 'c1', 'vt1', 'INV-0001', 'posted', '10000.00')")
    print("Test 5: Posting with valid positive total -> PASSED")

    # 6. Posting with invalid zero or malformed total -> must be REJECTED
    for bad in ['0', '0.00', '01.00', '1.', '1.2.3', '10.00001']:
        try:
            cur.execute("INSERT INTO vouchers VALUES (?, 'c1', 'vt1', 'INV-bad', 'posted', ?)", (f'v_post_{bad}', bad))
            assert False, f"Posting with {bad} should be rejected!"
        except sqlite3.IntegrityError:
            pass
    print("Test 6: Posting with zero/malformed total rejected -> PASSED")

    # 7. Cancelled and reversed vouchers preservation
    cur.execute("INSERT INTO vouchers VALUES ('v7_rev', 'c1', 'vt1', 'INV-0002', 'reversed', '10000.00')")
    cur.execute("INSERT INTO vouchers VALUES ('v7_can', 'c1', 'vt1', 'INV-0003', 'cancelled', '10000.00')")
    print("Test 7: Reversed and Cancelled retain valid total -> PASSED")

    # 8. Cancelled or reversed with NULL total -> must be REJECTED
    try:
        cur.execute("INSERT INTO vouchers VALUES ('v8_can', 'c1', 'vt1', 'INV-0004', 'cancelled', NULL)")
        assert False, "Cancelled with NULL total must be rejected!"
    except sqlite3.IntegrityError:
        pass
    print("Test 8: Cancelled with NULL total rejected -> PASSED")

    print("\nAll draft and posting total_amount lifecycle tests PASSED successfully!")

test_draft_lifecycle()

