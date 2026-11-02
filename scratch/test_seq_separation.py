import sqlite3
import uuid

con = sqlite3.connect(':memory:')

ddl = """
-- 1. Numbering Configurations (Configuration effective dates)
CREATE TABLE voucher_number_configs (
    config_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL DEFAULT 'Main',
    applicable_from TEXT NOT NULL, -- ISO date YYYY-MM-DD
    restart_frequency TEXT NOT NULL CHECK (restart_frequency IN ('never', 'yearly', 'monthly', 'daily')),
    prefix TEXT NOT NULL DEFAULT '',
    suffix TEXT NOT NULL DEFAULT '',
    width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
    start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number >= 1),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CONSTRAINT uq_config_applicable UNIQUE (company_id, voucher_type_id, series_name, applicable_from)
);

-- 2. Numbering Counters (Sequence period starts)
CREATE TABLE voucher_number_sequences (
    sequence_id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL DEFAULT 'Main',
    period_start TEXT NOT NULL, -- ISO date YYYY-MM-DD
    current_number INTEGER NOT NULL CHECK (current_number >= 1),
    prefix TEXT NOT NULL,
    suffix TEXT NOT NULL,
    width INTEGER NOT NULL CHECK (width BETWEEN 1 AND 10),
    config_id TEXT NOT NULL REFERENCES voucher_number_configs(config_id),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CONSTRAINT uq_seq_period UNIQUE (company_id, voucher_type_id, series_name, period_start)
);
"""

con.executescript(ddl)

# Test Scenario 1: Setup configurations
# Config 1: FY 2025-26 prefix 'SALES-25/', yearly reset, effective 2025-04-01
con.execute("""
INSERT INTO voucher_number_configs (
    config_id, company_id, voucher_type_id, series_name, applicable_from, restart_frequency, prefix, suffix, width, start_number
) VALUES ('cfg_25', 'comp_1', 'vtype_sales', 'Main', '2025-04-01', 'yearly', 'SALES-25/', '', 4, 1)
""")

# Config 2: Changed on 2026-04-01 for FY 2026-27 to 'SALES-26/'
con.execute("""
INSERT INTO voucher_number_configs (
    config_id, company_id, voucher_type_id, series_name, applicable_from, restart_frequency, prefix, suffix, width, start_number
) VALUES ('cfg_26', 'comp_1', 'vtype_sales', 'Main', '2026-04-01', 'yearly', 'SALES-26/', '', 4, 1)
""")

# Helper to allocate next voucher number
def allocate_number(company_id, voucher_type_id, series_name, voucher_date, fy_start_month=4, fy_start_day=1):
    # 1. Resolve applicable configuration
    cfg = con.execute("""
        SELECT config_id, prefix, suffix, width, start_number, restart_frequency, applicable_from
        FROM voucher_number_configs
        WHERE company_id = ? AND voucher_type_id = ? AND series_name = ? AND applicable_from <= ?
        ORDER BY applicable_from DESC LIMIT 1
    """, (company_id, voucher_type_id, series_name, voucher_date)).fetchone()
    
    assert cfg is not None, f"No configuration found for date {voucher_date}"
    config_id, prefix, suffix, width, start_number, restart_freq, applicable_from = cfg
    
    # 2. Compute period_start
    # For yearly:
    v_year = int(voucher_date[0:4])
    v_month = int(voucher_date[5:7])
    v_day = int(voucher_date[8:10])
    
    if restart_freq == 'yearly':
        if (v_month, v_day) >= (fy_start_month, fy_start_day):
            period_start = f"{v_year:04d}-{fy_start_month:02d}-{fy_start_day:02d}"
        else:
            period_start = f"{v_year-1:04d}-{fy_start_month:02d}-{fy_start_day:02d}"
    elif restart_freq == 'monthly':
        period_start = f"{v_year:04d}-{v_month:02d}-01"
    elif restart_freq == 'daily':
        period_start = voucher_date
    else: # never
        period_start = "2025-04-01"
        
    # 3. Atomic UPSERT counter
    new_uuid = str(uuid.uuid4())
    row = con.execute("""
        INSERT INTO voucher_number_sequences (
            sequence_id, company_id, voucher_type_id, series_name,
            period_start, current_number, prefix, suffix, width, config_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (company_id, voucher_type_id, series_name, period_start)
        DO UPDATE SET current_number = voucher_number_sequences.current_number + 1,
                      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        RETURNING current_number, prefix, suffix, width
    """, (new_uuid, company_id, voucher_type_id, series_name, period_start, start_number, prefix, suffix, width, config_id)).fetchone()
    
    cur_num, pfx, sfx, w = row
    formatted = f"{pfx}{cur_num:0{w}d}{sfx}"
    return formatted, period_start, config_id

# Test 1: Post voucher on 2026-04-15 (FY 2026-27)
num1, p1, c1 = allocate_number('comp_1', 'vtype_sales', 'Main', '2026-04-15')
print("Voucher 1 (2026-04-15):", num1, "Period:", p1, "Config:", c1)
assert num1 == "SALES-26/0001" and p1 == "2026-04-01" and c1 == "cfg_26"

# Test 2: Post another voucher on 2026-04-20 (FY 2026-27)
num2, p2, c2 = allocate_number('comp_1', 'vtype_sales', 'Main', '2026-04-20')
print("Voucher 2 (2026-04-20):", num2, "Period:", p2, "Config:", c2)
assert num2 == "SALES-26/0002" and p2 == "2026-04-01" and c2 == "cfg_26"

# Test 3: Backdated voucher for 2026-03-25 (FY 2025-26!)
# It should inherit cfg_25 and create period 2025-04-01 counter starting at 1!
num3, p3, c3 = allocate_number('comp_1', 'vtype_sales', 'Main', '2026-03-25')
print("Voucher 3 (2026-03-25, backdated):", num3, "Period:", p3, "Config:", c3)
assert num3 == "SALES-25/0001" and p3 == "2025-04-01" and c3 == "cfg_25"

# Test 4: Second backdated voucher for 2026-03-28 (FY 2025-26!)
# It should increment the 2025-04-01 period counter to 2!
num4, p4, c4 = allocate_number('comp_1', 'vtype_sales', 'Main', '2026-03-28')
print("Voucher 4 (2026-03-28, backdated):", num4, "Period:", p4, "Config:", c4)
assert num4 == "SALES-25/0002" and p4 == "2025-04-01" and c4 == "cfg_25"

print("All sequence configuration & period separation tests PASSED 100%!")

