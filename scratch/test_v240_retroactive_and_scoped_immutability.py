import sqlite3

def test_retroactive_and_scoped_immutability():
    conn = sqlite3.connect(":memory:")
    c = conn.cursor()
    c.execute("PRAGMA foreign_keys = ON;")

    c.executescript("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
    );

    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        FOREIGN KEY (company_id) REFERENCES companies(id)
    );

    CREATE TABLE voucher_number_configs (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1,
        width INTEGER NOT NULL DEFAULT 4,
        restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
        FOREIGN KEY (company_id) REFERENCES companies(id),
        FOREIGN KEY (voucher_type_id) REFERENCES voucher_types(id),
        CONSTRAINT uq_voucher_number_configs_date UNIQUE (company_id, voucher_type_id, series_name, applicable_from)
    );

    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL DEFAULT 'Primary',
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        FOREIGN KEY (company_id) REFERENCES companies(id),
        FOREIGN KEY (voucher_type_id) REFERENCES voucher_types(id)
    );

    -- Trigger 9: Series-scoped immutability on UPDATE
    CREATE TRIGGER trg_prevent_active_config_mutation
    BEFORE UPDATE ON voucher_number_configs
    WHEN EXISTS (
        SELECT 1 FROM vouchers
        WHERE company_id = OLD.company_id
          AND voucher_type_id = OLD.voucher_type_id
          AND series_name = OLD.series_name
          AND status != 'draft'
          AND date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot modify active numbering configuration: permanent vouchers exist in this series governed by this configuration.');
    END;

    -- Trigger 10: Series-scoped immutability on DELETE
    CREATE TRIGGER trg_prevent_active_config_deletion
    BEFORE DELETE ON voucher_number_configs
    WHEN EXISTS (
        SELECT 1 FROM vouchers
        WHERE company_id = OLD.company_id
          AND voucher_type_id = OLD.voucher_type_id
          AND series_name = OLD.series_name
          AND status != 'draft'
          AND date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot delete active numbering configuration: permanent vouchers exist in this series governed by this configuration.');
    END;

    -- Trigger 11: Retroactive INSERT protection
    CREATE TRIGGER trg_prevent_retroactive_config_insert
    BEFORE INSERT ON voucher_number_configs
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = NEW.company_id
          AND v.voucher_type_id = NEW.voucher_type_id
          AND v.series_name = NEW.series_name
          AND v.status != 'draft'
          AND v.date >= NEW.applicable_from
          AND (
              NOT EXISTS (
                  SELECT 1 FROM voucher_number_configs c
                  WHERE c.company_id = NEW.company_id
                    AND c.voucher_type_id = NEW.voucher_type_id
                    AND c.series_name = NEW.series_name
                    AND c.applicable_from > NEW.applicable_from
                    AND v.date >= c.applicable_from
              )
          )
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot insert retroactive numbering configuration: permanent vouchers already exist in this series governed by the proposed effective period.');
    END;
    """)

    # Seed company & voucher type
    c.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales');")

    # Initial config for FY 2026-27 in Primary series
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg1', 'comp1', 'vt_sales', 'Primary', 'automatic', '2026-04-01', 'INV-26/', '', 1, 4, 'yearly');
    """)

    # Another config for an unused series 'Export-Sales'
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg_exp', 'comp1', 'vt_sales', 'Export-Sales', 'automatic', '2026-04-01', 'EXP-26/', '', 1, 4, 'yearly');
    """)

    # Post vouchers in 'Primary' series
    c.execute("INSERT INTO vouchers VALUES ('v1', 'comp1', 'vt_sales', 'Primary', 'INV-26/0001', '2026-05-10', 'posted');")
    c.execute("INSERT INTO vouchers VALUES ('v2', 'comp1', 'vt_sales', 'Primary', 'INV-26/0002', '2026-08-15', 'posted');")

    # Test 1: Updating 'Primary' cfg1 is BLOCKED because permanent vouchers exist
    blocked_update = False
    try:
        c.execute("UPDATE voucher_number_configs SET prefix = 'HACK/' WHERE id = 'cfg1';")
    except sqlite3.IntegrityError as e:
        blocked_update = True
        assert "Cannot modify active numbering configuration" in str(e)
    assert blocked_update, "Active config update must be blocked"

    # Test 2: Updating 'Export-Sales' cfg_exp is ALLOWED because Export-Sales has NO permanent vouchers!
    c.execute("UPDATE voucher_number_configs SET prefix = 'EXP-NEW/' WHERE id = 'cfg_exp';")
    c.execute("SELECT prefix FROM voucher_number_configs WHERE id = 'cfg_exp';")
    assert c.fetchone()[0] == 'EXP-NEW/', "Unreferenced series config update must succeed"

    # Test 3: Deleting 'Export-Sales' cfg_exp is ALLOWED
    c.execute("DELETE FROM voucher_number_configs WHERE id = 'cfg_exp';")
    c.execute("SELECT count(*) FROM voucher_number_configs WHERE id = 'cfg_exp';")
    assert c.fetchone()[0] == 0, "Unreferenced series config deletion must succeed"

    # Test 4: Inserting a future configuration for FY 2027-28 in 'Primary' is ALLOWED
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg_future', 'comp1', 'vt_sales', 'Primary', 'automatic', '2027-04-01', 'INV-27/', '', 1, 4, 'yearly');
    """)

    # Test 5: Future config can be edited BEFORE vouchers exist on or after 2027-04-01
    c.execute("UPDATE voucher_number_configs SET prefix = 'INV-2027/' WHERE id = 'cfg_future';")
    c.execute("SELECT prefix FROM voucher_number_configs WHERE id = 'cfg_future';")
    assert c.fetchone()[0] == 'INV-2027/', "Future config without vouchers must be editable"

    # Test 6: Inserting retroactive config into 'Primary' at '2026-06-01' is BLOCKED
    # Because vouchers exist on 2026-08-15 without intervening config!
    blocked_retro = False
    try:
        c.execute("""
        INSERT INTO voucher_number_configs VALUES
        ('cfg_retro', 'comp1', 'vt_sales', 'Primary', 'automatic', '2026-06-01', 'INV-MID/', '', 1, 4, 'yearly');
        """)
    except sqlite3.IntegrityError as e:
        blocked_retro = True
        assert "Cannot insert retroactive numbering configuration" in str(e)
    assert blocked_retro, "Retroactive insert into active period must be blocked"

    # Test 7: Inserting config for unused historical period (FY 2024-25 at '2024-04-01') is ALLOWED!
    # Because 2026 vouchers are covered by the intervening config at '2026-04-01'!
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg_hist', 'comp1', 'vt_sales', 'Primary', 'automatic', '2024-04-01', 'INV-24/', '', 1, 4, 'yearly');
    """)
    c.execute("SELECT count(*) FROM voucher_number_configs WHERE id = 'cfg_hist';")
    assert c.fetchone()[0] == 1, "Unused historical period config insert must succeed"

    # Test 8: Duplicate applicable_from date for same series is BLOCKED by UNIQUE constraint
    # (Testing on future date 2027-04-01 where trigger passes, verifying schema UNIQUE constraint)
    blocked_dup_date = False
    try:
        c.execute("""
        INSERT INTO voucher_number_configs VALUES
        ('cfg_dup', 'comp1', 'vt_sales', 'Primary', 'automatic', '2027-04-01', 'INV-DUP/', '', 1, 4, 'yearly');
        """)
    except sqlite3.IntegrityError as e:
        blocked_dup_date = True
        assert "UNIQUE constraint failed" in str(e)
    assert blocked_dup_date, "Duplicate applicable_from must be blocked by UNIQUE constraint"

    conn.close()
    print("Finding 2 & 5: Retroactive Config & Series-Scoped Immutability Passed 100%!")

if __name__ == "__main__":
    test_retroactive_and_scoped_immutability()
