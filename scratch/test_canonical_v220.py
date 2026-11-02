import sqlite3

def run_test():
    conn = sqlite3.connect(":memory:")
    cur = conn.cursor()

    cur.execute("""
    CREATE TABLE test_amounts (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        amount TEXT NOT NULL,
        CONSTRAINT chk_test_amount_canonical CHECK (
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

    accepted = [
        "0.10", "0.50", "0.0001", "1", "1.0", "1.00", "1.000", "1.0000", "10.00", "10.25",
        "0.01", "0.001", "0.9999", "10", "10000.25", "999999999999.9999", "100.5"
    ]

    rejected = [
        "0", "0.0", "0.00", "01.00", "00.50", "0001.25", "010.50", ".50", "1.", "1.2.3",
        "10.00001", "0.00001", "-10.00", "+10.00", "", " ", "  ", "\t", "10.00 ", " 10.00",
        "00", "00.00", "0.000", "0.0000", "00.000", "0.00000", "1abc", "abc", "000", "01",
        "007", "10. ", "1.0.0", "1..00"
    ]

    print("--- TESTING ACCEPTED ---")
    for val in accepted:
        try:
            cur.execute("INSERT INTO test_amounts (name, amount) VALUES (?, ?)", ("acc", val))
            print(f"ACCEPTED: {val!r:15} -> OK")
        except sqlite3.IntegrityError as e:
            print(f"ACCEPTED FAILED: {val!r:15} -> {e}")
            assert False, f"Value {val} should be accepted but was rejected!"

    print("\n--- TESTING REJECTED ---")
    for val in rejected:
        try:
            cur.execute("INSERT INTO test_amounts (name, amount) VALUES (?, ?)", ("rej", val))
            print(f"REJECTED FAILED: {val!r:15} -> Was accepted!")
            assert False, f"Value {val} should be rejected but was accepted!"
        except sqlite3.IntegrityError as e:
            print(f"REJECTED: {val!r:15} -> OK ({e})")

    print("\nAll tests passed successfully!")

run_test()

