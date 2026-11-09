import sqlite3
import re

print("=== OpenTally Stage 4 Hardening Migration Compatibility Audit ===")

# Test 1: Verify runner version check logic
conn = sqlite3.connect(":memory:")
conn.execute("""
    CREATE TABLE _opentally_metadata (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        schema_version TEXT NOT NULL,
        installed_at TEXT NOT NULL,
        app_version TEXT NOT NULL
    );
""")
conn.execute("""
    INSERT INTO _opentally_metadata (id, schema_version, installed_at, app_version)
    VALUES (1, '0.3.0-sprint2', '2026-11-04T12:00:00Z', '0.1.0');
""")

# Simulate run_sprint2_migration Step 2:
cur = conn.cursor()
cur.execute("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
current_version = cur.fetchone()[0]

print(f"Step 2 Check: current_version = '{current_version}'")
if current_version == "0.3.0-sprint2":
    print("-> Migration runner encounters idempotent skip branch!")
    print("-> Database is returned unchanged without executing any DDL.")

# Verify what this means for table constraints:
# If table was created under pre-hardening 0.3.0-sprint2:
# 1. voucher_lines had NO chk_voucher_line_number_bounds
# 2. vouchers had the old chk_voucher_sequence_metadata (without CASE WHEN ... = 1)
print("\nConclusion for Finding 1:")
print("1. Original SPRINT2_DDL was modified in-place.")
print("2. No new migration function or version was registered.")
print("3. Existing databases at 0.3.0-sprint2 skip run_sprint2_migration entirely.")
print("4. Existing databases remain on the unhardened schema lacking TD-S2-002 and TD-S2-003 constraints.")

