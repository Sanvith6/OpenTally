import sqlite3
import re

con = sqlite3.connect(':memory:')
con.execute("PRAGMA foreign_keys = ON;")

# Setup companies and ledgers dummy tables for FKs
con.executescript("""
CREATE TABLE companies (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE,
    books_beginning_from TEXT DEFAULT '2026-04-01',
    financial_year_begins TEXT DEFAULT '2026-04-01'
);

CREATE TABLE ledgers (
    company_id TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id)
);

INSERT INTO companies (id, name) VALUES ('comp_1', 'Acme Corp');
INSERT INTO ledgers (company_id, id, name) VALUES ('comp_1', 'led_cash', 'Cash A/c');
INSERT INTO ledgers (company_id, id, name) VALUES ('comp_1', 'led_sales', 'Sales A/c');
""")

# Extract DDL from document
with open("02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md", "r", encoding="utf-8") as f:
    text = f.read()

# Execute DDL for Sprint 2 tables
ddl_match = re.search(r"```sql\n(-- 1\. VOUCHER TYPES MASTER.*?)\n```", text, re.DOTALL)
assert ddl_match, "Could not find Sprint 2 DDL block"
ddl_sql = ddl_match.group(1)

con.executescript(ddl_sql)
print("Sprint 2 DDL executed cleanly in SQLite!")

# Execute triggers
trg_match = re.search(r"```sql\n(-- Trigger 1: Enforce Posted/Cancelled/Reversed Voucher Header Immutability.*?)\n```", text, re.DOTALL)
assert trg_match, "Could not find triggers block"
trg_sql = trg_match.group(1)

con.executescript(trg_sql)
print("Sprint 2 Triggers executed cleanly in SQLite!")

# Insert voucher type
con.execute("""
INSERT INTO voucher_types (id, company_id, name, category, is_predefined)
VALUES ('vt_sales', 'comp_1', 'Sales', 'accounting', 1)
""")

# Insert numbering config
con.execute("""
INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_name, applicable_from, prefix, width, restart_period)
VALUES ('cfg_1', 'comp_1', 'vt_sales', 'Primary', '2026-04-01', 'SALES/26-27/', 4, 'yearly')
""")

# Insert sequence counter
con.execute("""
INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_name, period_start, current_number, prefix, suffix, width, config_id)
VALUES ('seq_1', 'comp_1', 'vt_sales', 'Primary', '2026-04-01', 0, 'SALES/26-27/', '', 4, 'cfg_1')
""")

# Test Canonical amounts on voucher_lines
from test_grammar_eval import test_cases

# Create a draft voucher
con.execute("""
INSERT INTO vouchers (id, company_id, voucher_type_id, voucher_number, date, effective_date, status, total_amount)
VALUES ('vch_draft_1', 'comp_1', 'vt_sales', 'DRAFT-01', '2026-05-10', '2026-05-10', 'draft', NULL)
""")

passed_lines = 0
for val, expected in test_cases:
    try:
        con.execute("""
            INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
            VALUES (?, 'comp_1', 'vch_draft_1', ?, 'led_cash', 'Dr', ?)
        """, (f"l_{val}", passed_lines + 1, val))
        inserted = True
    except sqlite3.IntegrityError:
        inserted = False
    assert inserted == expected, f"voucher_lines failed on {val}: expected {expected}, got {inserted}"
    if inserted:
        passed_lines += 1

print(f"Direct insertion test for voucher_lines passed 100% on all {len(test_cases)} cases!")

# Test Canonical amounts on vouchers.total_amount
passed_vouchers = 0
for val, expected in test_cases:
    try:
        con.execute("""
            INSERT INTO vouchers (id, company_id, voucher_type_id, voucher_number, date, effective_date, status, total_amount)
            VALUES (?, 'comp_1', 'vt_sales', ?, '2026-05-10', '2026-05-10', 'posted', ?)
        """, (f"v_{val}", f"VCH-{passed_vouchers+1}", val))
        inserted = True
    except sqlite3.IntegrityError:
        inserted = False
    assert inserted == expected, f"vouchers failed on {val}: expected {expected}, got {inserted}"
    if inserted:
        passed_vouchers += 1

print(f"Direct insertion test for vouchers passed 100% on all {len(test_cases)} cases!")
print("ALL SQLITE VERIFICATIONS SUCCEEDED FLAWLESSLY!")

