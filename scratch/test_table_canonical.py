import sqlite3

def run_table_test():
    conn = sqlite3.connect(":memory:")
    cur = conn.cursor()

    cur.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        status TEXT NOT NULL,
        total_amount TEXT,
        CONSTRAINT chk_voucher_total_amount_canonical CHECK (
            (status = 'draft' AND total_amount IS NULL)
            OR
            (
                status IN ('posted', 'cancelled', 'reversed')
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

    cur.execute("""
    CREATE TABLE voucher_lines (
        id TEXT NOT NULL,
        amount TEXT NOT NULL,
        CONSTRAINT chk_voucher_line_amount_canonical CHECK (
            amount IS NOT NULL
            AND length(amount) >= 1
            AND length(amount) <= 25
            AND amount NOT GLOB '*[^0-9.]*'
            AND amount NOT GLOB '*..*'
            AND amount NOT GLOB '.*'
            AND amount NOT GLOB '*.'
            AND (
                (
                    instr(amount, '.') > 0
                    AND length(substr(amount, instr(amount, '.') + 1)) BETWEEN 1 AND 4
                    AND instr(substr(amount, instr(amount, '.') + 1), '.') = 0
                    AND (
                        (substr(amount, 1, instr(amount, '.') - 1) = '0' 
                         AND substr(amount, instr(amount, '.') + 1) GLOB '*[1-9]*')
                        OR
                        (substr(amount, 1, instr(amount, '.') - 1) GLOB '[1-9]*')
                    )
                )
                OR
                (
                    instr(amount, '.') = 0
                    AND amount GLOB '[1-9]*'
                )
            )
        )
    );
    """)

    # Test drafts can have NULL total_amount
    cur.execute("INSERT INTO vouchers VALUES ('v1', 'c1', 'draft', NULL)")
    print("Draft with NULL total_amount -> OK")

    # Test draft with non-NULL total_amount fails
    try:
        cur.execute("INSERT INTO vouchers VALUES ('v2', 'c1', 'draft', '10.00')")
        assert False, "Draft with non-NULL total_amount should fail"
    except sqlite3.IntegrityError:
        print("Draft with non-NULL total_amount rejected -> OK")

    # Test posted vouchers with canonical amounts
    cur.execute("INSERT INTO vouchers VALUES ('v3', 'c1', 'posted', '1.00')")
    cur.execute("INSERT INTO vouchers VALUES ('v4', 'c1', 'posted', '10.00')")
    cur.execute("INSERT INTO vouchers VALUES ('v5', 'c1', 'posted', '0.50')")
    print("Posted with '1.00', '10.00', '0.50' -> OK")

    # Test posted voucher with invalid amounts fails
    for bad in ['0', '0.00', '01.00', '1.', '1.2.3', '10.00001']:
        try:
            cur.execute("INSERT INTO vouchers VALUES (?, 'c1', 'posted', ?)", (f'v_{bad}', bad))
            assert False, f"Posted voucher with {bad} should fail"
        except sqlite3.IntegrityError:
            pass
    print("Posted with invalid amounts rejected -> OK")

    # Test voucher lines
    for ok in ['0.10', '0.50', '0.0001', '1', '1.0', '1.00', '1.000', '1.0000', '10.00', '10.25']:
        cur.execute("INSERT INTO voucher_lines VALUES (?, ?)", (f'l_{ok}', ok))
    print("Voucher lines with accepted amounts -> OK")

    for bad in ['0', '0.0', '0.00', '01.00', '00.50', '0001.25', '010.50', '.50', '1.', '1.2.3', '10.00001', '0.00001', '-10.00', '+10.00', '', ' ', '00']:
        try:
            cur.execute("INSERT INTO voucher_lines VALUES (?, ?)", (f'l_{bad}', bad))
            assert False, f"Voucher line with {bad} should fail"
        except sqlite3.IntegrityError:
            pass
    print("Voucher lines with rejected amounts rejected -> OK")

run_table_test()

