import sqlite3

con = sqlite3.connect(':memory:')

expr = """(
    -- 1. Allowed characters: only digits '0'-'9' and at most one '.'
    replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(:val, '0', ''), '1', ''), '2', ''), '3', ''), '4', ''), '5', ''), '6', ''), '7', ''), '8', ''), '9', '') IN ('', '.')
    -- 2. Strictly positive: contains at least one non-zero digit
    AND :val GLOB '*[1-9]*'
    -- 3. Structure rules:
    AND (
        -- Form A: Non-zero integer (no dot, must start with non-zero digit)
        (instr(:val, '.') = 0 AND :val GLOB '[1-9]*')
        OR
        -- Form B: Decimal number with '.'
        (
            instr(:val, '.') > 0
            -- Integer part: explicitly either exactly '0' or starts with non-zero digit [1-9]
            AND (substr(:val, 1, instr(:val, '.') - 1) = '0' OR substr(:val, 1, instr(:val, '.') - 1) GLOB '[1-9]*')
            -- Exactly one '.'
            AND length(:val) - length(replace(:val, '.', '')) = 1
            -- Fractional part: 1 to 4 digits
            AND length(substr(:val, instr(:val, '.') + 1)) BETWEEN 1 AND 4
        )
    )
)"""

test_cases = [
    # Valid canonical values
    ("0.10", True),
    ("0.50", True),
    ("1", True),
    ("1.00", True),
    ("10.25", True),
    ("10000.25", True),
    ("0.0001", True),
    ("999999999.9999", True),
    ("5", True),
    ("0.01", True),
    ("0.1", True),
    ("123.456", True),
    
    # Invalid values: zeros
    ("0", False),
    ("00", False),
    ("0.0", False),
    ("0.00", False),
    ("00.00", False),
    ("0.0000", False),
    ("0.00000", False),
    
    # Invalid values: malformed decimals
    ("1.", False),
    (".50", False),
    (".00", False),
    ("1.2.3", False),
    ("0.1.2", False),
    ("1..0", False),
    
    # Invalid values: non-canonical leading zeros
    ("01.00", False),
    ("00.50", False),
    ("0001.25", False),
    ("01", False),
    ("001", False),
    ("000", False),
    ("010.50", False),
    
    # Invalid values: non-digits / signs
    ("+10.00", False),
    ("-10.00", False),
    ("1abc", False),
    ("abc", False),
    ("1 0.00", False),
    ("10.00 ", False),
    (" 10.00", False),
    ("$10.00", False),
    ("10,000.00", False),
    
    # Invalid values: excessive precision (> 4 decimal places)
    ("1.00001", False),
    ("0.00001", False),
    ("10.12345", False),
]

all_passed = True
for val, expected in test_cases:
    sql = f"SELECT CASE WHEN {expr} THEN 1 ELSE 0 END"
    actual = bool(con.execute(sql, {'val': val}).fetchone()[0])
    if actual != expected:
        print(f"FAILED on '{val}': expected {expected}, got {actual}")
        all_passed = False

if all_passed:
    print(f"ALL {len(test_cases)} TEST CASES PASSED PERFECTLY!")

