import sqlite3

print("=== Simulating 0.3.0-sprint2 to 0.3.1-sprint2 Forward Migration ===")

conn = sqlite3.connect(":memory:", isolation_level=None)
conn.execute("PRAGMA foreign_keys = ON;")

# Setup mock baseline Sprint 1 + unhardened Sprint 2
conn.execute("""
CREATE TABLE _opentally_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version TEXT NOT NULL,
    installed_at TEXT NOT NULL,
    app_version TEXT NOT NULL
);
""")
conn.execute("INSERT INTO _opentally_metadata VALUES (1, '0.3.0-sprint2', '2026-11-04T12:00:00Z', '0.1.0');")

conn.execute("""
CREATE TABLE companies (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);
""")
conn.execute("INSERT INTO companies VALUES ('comp_1', 'Acme Corp');")

conn.execute("""
CREATE TABLE account_groups (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id)
);
""")
conn.execute("INSERT INTO account_groups VALUES ('grp_cash', 'comp_1', 'Cash-in-Hand');")

conn.execute("""
CREATE TABLE ledgers (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id),
    FOREIGN KEY (company_id, group_id) REFERENCES account_groups(company_id, id)
);
""")
conn.execute("INSERT INTO ledgers VALUES ('led_cash', 'comp_1', 'grp_cash', 'Cash A/c');")
conn.execute("INSERT INTO ledgers VALUES ('led_sales', 'comp_1', 'grp_cash', 'Sales A/c');")

conn.execute("""
CREATE TABLE voucher_types (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (company_id, id)
);
""")
conn.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_1', 'Sales');")

conn.execute("""
CREATE TABLE voucher_number_series (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_name TEXT NOT NULL,
    numbering_method TEXT NOT NULL,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, voucher_type_id, id, numbering_method)
);
""")
conn.execute("INSERT INTO voucher_number_series VALUES ('ser_1', 'comp_1', 'vt_sales', 'Main', 'automatic');")

# UNHARDENED vouchers table (old check constraint)
conn.execute("""
CREATE TABLE vouchers (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
    period_start TEXT,
    sequence_number INTEGER,
    voucher_number TEXT NOT NULL,
    date TEXT NOT NULL,
    effective_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
    reference_no TEXT,
    reference_date TEXT,
    narration TEXT,
    total_amount TEXT,
    reversal_of_voucher_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT
);
""")

# UNHARDENED voucher_lines table (no line number bounds)
conn.execute("""
CREATE TABLE voucher_lines (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    ledger_id TEXT NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount TEXT NOT NULL,
    narration TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, voucher_id, line_number),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT
);
""")

# Triggers on vouchers & lines
conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_mutation
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_deletion
BEFORE DELETE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete posted voucher');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_line_mutation
BEFORE UPDATE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot mutate posted lines');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_line_deletion
BEFORE DELETE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete posted lines');
END;
""")

# Insert realistic sample data (both draft and posted)
conn.execute("""
INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status, total_amount)
VALUES ('vch_draft_1', 'comp_1', 'vt_sales', 'ser_1', 'automatic', NULL, NULL, 'DRAFT-001', '2026-11-05', '2026-11-05', 'draft', '1000.00');
""")
conn.execute("""
INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
VALUES ('line_1', 'comp_1', 'vch_draft_1', 1, 'led_cash', 'Dr', '1000.00');
""")
conn.execute("""
INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
VALUES ('line_2', 'comp_1', 'vch_draft_1', 2, 'led_sales', 'Cr', '1000.00');
""")

print("Initial data seeded. Verifying record counts:")
print("vouchers count:", conn.execute("SELECT COUNT(*) FROM vouchers").fetchone()[0])
print("voucher_lines count:", conn.execute("SELECT COUNT(*) FROM voucher_lines").fetchone()[0])

# =========================================================================
# NOW PERFORM THE 0.3.1-sprint2 UPGRADE PROCEDURE
# =========================================================================
print("\nExecuting forward migration 0.3.1-sprint2...")

# Step 1: Version check
ver = conn.execute("SELECT schema_version FROM _opentally_metadata WHERE id = 1").fetchone()[0]
assert ver == "0.3.0-sprint2"

# Step 2: PRAGMA foreign_keys = OFF
conn.execute("PRAGMA foreign_keys = OFF;")
assert conn.execute("PRAGMA foreign_keys;").fetchone()[0] == 0

# Step 3: BEGIN IMMEDIATE
conn.execute("BEGIN IMMEDIATE;")

# Step 4: Validate existing data meets new constraints before migrating
# 4a: voucher sequence metadata validation
invalid_vouchers = conn.execute("""
SELECT id FROM vouchers
WHERE NOT (
    CASE
        WHEN status = 'draft' THEN (period_start IS NULL AND sequence_number IS NULL)
        WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'automatic' THEN (
            period_start IS NOT NULL AND date(period_start) IS NOT NULL AND sequence_number IS NOT NULL AND sequence_number > 0
        )
        WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'manual' THEN (
            sequence_number IS NULL AND (period_start IS NULL OR date(period_start) IS NOT NULL)
        )
        ELSE 0
    END = 1
)
""").fetchall()
if invalid_vouchers:
    raise Exception(f"Pre-migration validation failed for vouchers: {invalid_vouchers}")

# 4b: voucher line bounds validation
invalid_lines = conn.execute("""
SELECT id, line_number FROM voucher_lines
WHERE NOT (
    (line_number BETWEEN 1 AND 10000) OR (line_number BETWEEN 1000001 AND 1010000)
)
""").fetchall()
if invalid_lines:
    raise Exception(f"Pre-migration validation failed for voucher_lines: {invalid_lines}")

# Step 4c: Drop triggers on other tables that reference vouchers or voucher_lines
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_posted_voucher_line_mutation;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_posted_voucher_line_deletion;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_posted_voucher_mutation;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_posted_voucher_deletion;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_company_delete_with_financial_vouchers;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_series_code_mutation;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_series_deletion_with_vouchers;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_active_config_mutation;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_active_config_deletion;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_retroactive_config_insert;")
conn.execute("DROP TRIGGER IF EXISTS trg_prevent_ledger_delete_with_vouchers;")
conn.execute("""
CREATE TABLE vouchers_dg_tmp (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_type_id TEXT NOT NULL,
    series_id TEXT NOT NULL,
    numbering_method TEXT NOT NULL DEFAULT 'automatic' CHECK (numbering_method IN ('automatic', 'manual')),
    period_start TEXT,
    sequence_number INTEGER,
    voucher_number TEXT NOT NULL,
    date TEXT NOT NULL,
    effective_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
    reference_no TEXT,
    reference_date TEXT,
    narration TEXT,
    total_amount TEXT,
    reversal_of_voucher_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method) REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, reversal_of_voucher_id) REFERENCES vouchers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id),
    CONSTRAINT chk_voucher_date CHECK (
        date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(date) IS NOT NULL
        AND date(date) = date
    ),
    CONSTRAINT chk_effective_date CHECK (
        effective_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(effective_date) IS NOT NULL
        AND date(effective_date) = effective_date
    ),
    CONSTRAINT chk_voucher_sequence_metadata CHECK (
        CASE
            WHEN status = 'draft' THEN (
                period_start IS NULL
                AND sequence_number IS NULL
            )
            WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'automatic' THEN (
                period_start IS NOT NULL
                AND period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                AND date(period_start) IS NOT NULL
                AND date(period_start) = period_start
                AND sequence_number IS NOT NULL
                AND sequence_number > 0
            )
            WHEN status IN ('posted', 'cancelled', 'reversed') AND numbering_method = 'manual' THEN (
                sequence_number IS NULL
                AND (
                    period_start IS NULL
                    OR (
                        period_start GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                        AND date(period_start) IS NOT NULL
                        AND date(period_start) = period_start
                    )
                )
            )
            ELSE 0
        END = 1
    )
);
""")

conn.execute("""
INSERT INTO vouchers_dg_tmp (
    id, company_id, voucher_type_id, series_id, numbering_method,
    period_start, sequence_number, voucher_number, date, effective_date,
    status, reference_no, reference_date, narration, total_amount,
    reversal_of_voucher_id, created_at, updated_at
)
SELECT 
    id, company_id, voucher_type_id, series_id, numbering_method,
    period_start, sequence_number, voucher_number, date, effective_date,
    status, reference_no, reference_date, narration, total_amount,
    reversal_of_voucher_id, created_at, updated_at
FROM vouchers;
""")

vch_src = conn.execute("SELECT COUNT(*) FROM vouchers").fetchone()[0]
vch_dst = conn.execute("SELECT COUNT(*) FROM vouchers_dg_tmp").fetchone()[0]
assert vch_src == vch_dst

conn.execute("DROP TABLE vouchers;")
conn.execute("ALTER TABLE vouchers_dg_tmp RENAME TO vouchers;")

# Step 6: Rebuild voucher_lines table
conn.execute("""
CREATE TABLE voucher_lines_dg_tmp (
    id TEXT NOT NULL,
    company_id TEXT NOT NULL,
    voucher_id TEXT NOT NULL,
    line_number INTEGER NOT NULL CHECK (line_number > 0),
    ledger_id TEXT NOT NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN ('Dr', 'Cr')),
    amount TEXT NOT NULL,
    narration TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (company_id, id),
    UNIQUE (company_id, voucher_id, line_number),
    FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
    FOREIGN KEY (company_id, voucher_id) REFERENCES vouchers(company_id, id) ON DELETE CASCADE,
    FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
    CONSTRAINT chk_voucher_line_number_bounds CHECK (
        (line_number BETWEEN 1 AND 10000)
        OR
        (line_number BETWEEN 1000001 AND 1010000)
    )
);
""")

conn.execute("""
INSERT INTO voucher_lines_dg_tmp (
    id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
)
SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
FROM voucher_lines;
""")

vl_src = conn.execute("SELECT COUNT(*) FROM voucher_lines").fetchone()[0]
vl_dst = conn.execute("SELECT COUNT(*) FROM voucher_lines_dg_tmp").fetchone()[0]
assert vl_src == vl_dst

conn.execute("DROP TABLE voucher_lines;")
conn.execute("ALTER TABLE voucher_lines_dg_tmp RENAME TO voucher_lines;")

# Step 7: Recreate indexes
conn.execute("CREATE INDEX idx_vouchers_date ON vouchers(company_id, date);")
conn.execute("CREATE INDEX idx_vouchers_type_status ON vouchers(company_id, voucher_type_id, status);")
conn.execute("CREATE INDEX idx_vouchers_series_ownership ON vouchers(company_id, voucher_type_id, series_id, numbering_method);")
conn.execute("CREATE UNIQUE INDEX uq_vouchers_permanent_number ON vouchers(company_id, voucher_type_id, voucher_number) WHERE status != 'draft';")
conn.execute("CREATE UNIQUE INDEX uq_vouchers_series_sequence ON vouchers(company_id, series_id, period_start, sequence_number) WHERE status != 'draft' AND sequence_number IS NOT NULL;")
conn.execute("CREATE UNIQUE INDEX uq_vouchers_reversal ON vouchers(company_id, reversal_of_voucher_id) WHERE reversal_of_voucher_id IS NOT NULL;")
conn.execute("CREATE INDEX idx_voucher_lines_ledger ON voucher_lines(company_id, ledger_id);")

# Step 8: Recreate triggers
conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_mutation
BEFORE UPDATE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Illegal voucher lifecycle transition');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_deletion
BEFORE DELETE ON vouchers
FOR EACH ROW
WHEN OLD.status IN ('posted', 'cancelled', 'reversed')
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete posted voucher');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_line_mutation
BEFORE UPDATE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot mutate posted lines');
END;
""")

conn.execute("""
CREATE TRIGGER trg_prevent_posted_voucher_line_deletion
BEFORE DELETE ON voucher_lines
FOR EACH ROW
WHEN EXISTS (
    SELECT 1 FROM vouchers 
    WHERE company_id = OLD.company_id 
      AND id = OLD.voucher_id 
      AND status IN ('posted', 'cancelled', 'reversed')
)
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete posted lines');
END;
""")

# Step 9: Update schema version in _opentally_metadata
conn.execute("UPDATE _opentally_metadata SET schema_version = '0.3.1-sprint2' WHERE id = 1;")

# Step 10: Foreign key check
fk_violations = conn.execute("PRAGMA foreign_key_check;").fetchall()
assert len(fk_violations) == 0, f"FK violations: {fk_violations}"

# Step 11: COMMIT
conn.execute("COMMIT;")

# Step 12: Re-enable foreign keys OUTSIDE transaction
conn.execute("PRAGMA foreign_keys = ON;")
assert conn.execute("PRAGMA foreign_keys;").fetchone()[0] == 1

print("Migration completed successfully!")
print("Final schema version:", conn.execute("SELECT schema_version FROM _opentally_metadata").fetchone()[0])
print("vouchers count:", conn.execute("SELECT COUNT(*) FROM vouchers").fetchone()[0])
print("voucher_lines count:", conn.execute("SELECT COUNT(*) FROM voucher_lines").fetchone()[0])

# Verify new constraints are active on rebuilt tables:
# Test A: line_number > 10000 rejected
try:
    conn.execute("""
    INSERT INTO voucher_lines (id, company_id, voucher_id, line_number, ledger_id, entry_type, amount)
    VALUES ('line_bad', 'comp_1', 'vch_draft_1', 10001, 'led_cash', 'Dr', '100.00');
    """)
    print("FAILED: line 10001 was allowed!")
except sqlite3.IntegrityError as e:
    print("SUCCESS: line 10001 properly rejected by new constraint:", e)

# Test B: ternary null draft bypass rejected
try:
    conn.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, effective_date, status)
    VALUES ('vch_bad_draft', 'comp_1', 'vt_sales', 'ser_1', 'automatic', '2026-04-01', NULL, 'DRAFT-BAD', '2026-11-05', '2026-11-05', 'draft');
    """)
    print("FAILED: draft with period_start was allowed!")
except sqlite3.IntegrityError as e:
    print("SUCCESS: draft with period_start properly rejected by new constraint:", e)
