import sqlite3

con = sqlite3.connect(':memory:')

# Test cases: (value, should_pass)
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

# Let's test expressions!

