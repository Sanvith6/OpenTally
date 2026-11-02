import sqlite3

con = sqlite3.connect(':memory:')

ddl = """
CREATE TABLE voucher_lines (
    line_id TEXT PRIMARY KEY,
    amount TEXT NOT NULL,
    CONSTRAINT chk_voucher_line_amount_canonical CHECK (
        replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(amount, '0', ''), '1', ''), '2', ''), '3', ''), '4', ''), '5', ''), '6', ''), '7', ''), '8', ''), '9', '') IN ('', '.')
        AND amount GLOB '*[1-9]*'
        AND (
            (instr(amount, '.') = 0 AND amount NOT GLOB '0*')
            OR
            (
                instr(amount, '.') > 0
                AND (amount GLOB '0.*' OR (amount NOT GLOB '0*' AND amount NOT GLOB '.*'))
                AND length(amount) - length(replace(amount, '.', '')) = 1
                AND length(substr(amount, instr(amount, '.') + 1)) BETWEEN 1 AND 4
            )
        )
    )
);

CREATE TABLE vouchers (
    voucher_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    total_amount TEXT,
    CONSTRAINT chk_voucher_total_amount_canonical CHECK (
        total_amount IS NULL OR (
            replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(total_amount, '0', ''), '1', ''), '2', ''), '3', ''), '4', ''), '5', ''), '6', ''), '7', ''), '8', ''), '9', '') IN ('', '.')
            AND total_amount GLOB '*[1-9]*'
            AND (
                (instr(total_amount, '.') = 0 AND total_amount NOT GLOB '0*')
                OR
                (
                    instr(total_amount, '.') > 0
                    AND (total_amount GLOB '0.*' OR (total_amount NOT GLOB '0*' AND total_amount NOT GLOB '.*'))
                    AND length(total_amount) - length(replace(total_amount, '.', '')) = 1
                    AND length(substr(total_amount, instr(total_amount, '.') + 1)) BETWEEN 1 AND 4
                )
            )
        )
    )
);
"""

con.executescript(ddl)

from test_grammar_eval import test_cases

for val, expected in test_cases:
    # Test voucher_lines.amount
    try:
        con.execute("INSERT INTO voucher_lines (line_id, amount) VALUES (?, ?)", (f"line_{val}", val))
        inserted = True
    except sqlite3.IntegrityError:
        inserted = False
    assert inserted == expected, f"voucher_lines failed on {val}: expected {expected}, got {inserted}"
    
    # Test vouchers.total_amount
    try:
        con.execute("INSERT INTO vouchers (voucher_id, status, total_amount) VALUES (?, 'posted', ?)", (f"vch_{val}", val))
        inserted = True
    except sqlite3.IntegrityError:
        inserted = False
    assert inserted == expected, f"vouchers failed on {val}: expected {expected}, got {inserted}"

# Test vouchers with NULL total_amount (allowed for drafts)
con.execute("INSERT INTO vouchers (voucher_id, status, total_amount) VALUES ('draft_1', 'draft', NULL)")
print("Direct table insertion tests for voucher_lines and vouchers PASSED 100%!")

