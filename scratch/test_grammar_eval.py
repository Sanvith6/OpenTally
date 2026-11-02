import sqlite3

con = sqlite3.connect(':memory:')

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
    ("01", False),
    ("001", False),
    
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

# Proposed SQL Expression:
expr = """(
    -- 1. Strictly digits and at most one dot: replacing digits leaves at most '.'
    replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(:val, '0', ''), '1', ''), '2', ''), '3', ''), '4', ''), '5', ''), '6', ''), '7', ''), '8', ''), '9', '') IN ('', '.')
    -- 2. Strictly positive: contains at least one non-zero digit
    AND :val GLOB '*[1-9]*'
    -- 3. Structure rules:
    AND (
        -- Case A: Integer without decimal point (e.g. '1', '10')
        (instr(:val, '.') = 0 AND :val NOT GLOB '0*')
        OR
        -- Case B: Decimal number with '.'
        (
            instr(:val, '.') > 0
            -- Integer part: either exactly '0' (starts with '0.') or non-zero start (not starting with '0' or '.')
            AND (:val GLOB '0.*' OR (:val NOT GLOB '0*' AND :val NOT GLOB '.*'))
            -- Exactly one '.' (length difference when removing '.' is 1)
            AND length(:val) - length(replace(:val, '.', '')) = 1
            -- Fractional part: 1 to 4 digits
            AND length(substr(:val, instr(:val, '.') + 1)) BETWEEN 1 AND 4
        )
    )
)"""

all_passed = True
for val, expected in test_cases:
    sql = f"SELECT CASE WHEN {expr} THEN 1 ELSE 0 END"
    actual = bool(con.execute(sql, {'val': val}).fetchone()[0])
    if actual != expected:
        print(f"FAILED on '{val}': expected {expected}, got {actual}")
        all_passed = False

if all_passed:
    print(f"ALL {len(test_cases)} TEST CASES PASSED PERFECTLY!")
