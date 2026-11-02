import sqlite3
import uuid

con = sqlite3.connect(':memory:')

ddl = """
-- Numbering Configurations
CREATE TABLE voucher_number_configs (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL DEFAULT 'Primary',
    applicable_from TEXT NOT NULL, -- YYYY-MM-DD
    prefix TEXT NOT NULL DEFAULT '',
    suffix TEXT NOT NULL DEFAULT '',
    start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
    width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
    restart_period TEXT NOT NULL DEFAULT 'yearly' CHECK (restart_period IN ('never', 'yearly', 'monthly', 'daily')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CONSTRAINT uq_config_applicable UNIQUE (company_id, voucher_type_id, series_name, applicable_from)
);

-- Sequence Period Counters (Pure Counter, no duplicate format columns)
CREATE TABLE voucher_number_sequences (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL DEFAULT 'Primary',
    period_start TEXT NOT NULL, -- YYYY-MM-DD
    current_number INTEGER NOT NULL DEFAULT 0 CHECK (current_number >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CONSTRAINT uq_seq_period UNIQUE (company_id, voucher_type_id, series_name, period_start)
);
"""

con.executescript(ddl)

# Setup company & voucher type
company_id = "comp_1"
voucher_type_id = "vt_sales"
fy_begins_m, fy_begins_d = 4, 1

# Config 1: Active from 2026-04-01: prefix = 'INV-26/'
con.execute("""
INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_name, applicable_from, prefix, width, restart_period, start_number)
VALUES ('cfg_apr', 'comp_1', 'vt_sales', 'Primary', '2026-04-01', 'INV-26/', 4, 'yearly', 1)
""")

# Config 2: Mid-year statutory update on 2026-07-01: prefix = 'GST-26/'
con.execute("""
INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_name, applicable_from, prefix, width, restart_period, start_number)
VALUES ('cfg_jul', 'comp_1', 'vt_sales', 'Primary', '2026-07-01', 'GST-26/', 4, 'yearly', 1)
""")

def allocate_number(v_date):
    # 1. Resolve configuration applicable on v_date
    row = con.execute("""
        SELECT id, prefix, suffix, width, start_number, restart_period, applicable_from
        FROM voucher_number_configs
        WHERE company_id = ? AND voucher_type_id = ? AND series_name = 'Primary' AND applicable_from <= ?
        ORDER BY applicable_from DESC LIMIT 1
    """, (company_id, voucher_type_id, v_date)).fetchone()
    assert row is not None
    cfg_id, prefix, suffix, width, start_number, restart_period, app_from = row
    
    # 2. Compute period_start
    y, m, d = int(v_date[:4]), int(v_date[5:7]), int(v_date[8:10])
    if restart_period == 'yearly':
        if (m, d) >= (fy_begins_m, fy_begins_d):
            period_start = f"{y:04d}-{fy_begins_m:02d}-{fy_begins_d:02d}"
        else:
            period_start = f"{y-1:04d}-{fy_begins_m:02d}-{fy_begins_d:02d}"
    
    # 3. Atomic UPSERT counter
    new_uuid = str(uuid.uuid4())
    allocated_num = con.execute("""
        INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_name, period_start, current_number)
        VALUES (?, ?, ?, 'Primary', ?, ?)
        ON CONFLICT (company_id, voucher_type_id, series_name, period_start)
        DO UPDATE SET current_number = voucher_number_sequences.current_number + 1,
                      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        RETURNING current_number
    """, (new_uuid, company_id, voucher_type_id, period_start, start_number)).fetchone()[0]
    
    # 4. Format
    num_str = f"{allocated_num:0{width}d}"
    formatted = f"{prefix}{num_str}{suffix}"
    return formatted, period_start, cfg_id

# Test sequence:
# 1. Voucher in May 2026 (2026-05-10) -> Should be INV-26/0001
v1, p1, c1 = allocate_number("2026-05-10")
print("V1:", v1, "Period:", p1, "Config:", c1)
assert v1 == "INV-26/0001" and p1 == "2026-04-01" and c1 == "cfg_apr"

# 2. Voucher in August 2026 (2026-08-15) -> Should be GST-26/0002!
v2, p2, c2 = allocate_number("2026-08-15")
print("V2:", v2, "Period:", p2, "Config:", c2)
assert v2 == "GST-26/0002" and p2 == "2026-04-01" and c2 == "cfg_jul"

# 3. Backdated voucher in August for June 15 (2026-06-15) -> Should be INV-26/0003!
v3, p3, c3 = allocate_number("2026-06-15")
print("V3 (backdated):", v3, "Period:", p3, "Config:", c3)
assert v3 == "INV-26/0003" and p3 == "2026-04-01" and c3 == "cfg_apr"

# 4. Another current voucher on 2026-08-20 -> Should be GST-26/0004!
v4, p4, c4 = allocate_number("2026-08-20")
print("V4:", v4, "Period:", p4, "Config:", c4)
assert v4 == "GST-26/0004" and p4 == "2026-04-01" and c4 == "cfg_jul"

print("MID-YEAR CONFIGURATION AND SEQUENCE SEPARATION TEST PASSED 100%!")

