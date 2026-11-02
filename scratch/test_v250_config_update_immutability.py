import sqlite3

def test_config_update_immutability():
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

    -- Trigger 9a: Strict Immutability of applicable_from on UPDATE
    CREATE TRIGGER trg_prevent_config_applicable_from_mutation
    BEFORE UPDATE OF applicable_from ON voucher_number_configs
    FOR EACH ROW
    WHEN NEW.applicable_from != OLD.applicable_from
    BEGIN
        SELECT RAISE(ABORT, 'Configuration effective date (applicable_from) is immutable once created. Delete the unreferenced configuration and re-insert if no vouchers exist.');
    END;

    -- Trigger 9b: Immutability of Formatting/Counter Parameters for Active Configurations
    CREATE TRIGGER trg_prevent_active_config_mutation
    BEFORE UPDATE ON voucher_number_configs
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.voucher_type_id = OLD.voucher_type_id
          AND v.series_name = OLD.series_name
          AND v.status IN ('posted', 'reversed', 'cancelled')
          AND v.date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot modify active numbering configuration: permanent vouchers exist in this series governed by this configuration. Append a new configuration row with a future applicable_from date instead.');
    END;

    -- Trigger 10: Series-Scoped Immutability on DELETE
    CREATE TRIGGER trg_prevent_active_config_deletion
    BEFORE DELETE ON voucher_number_configs
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.voucher_type_id = OLD.voucher_type_id
          AND v.series_name = OLD.series_name
          AND v.status IN ('posted', 'reversed', 'cancelled')
          AND v.date >= OLD.applicable_from
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot delete active numbering configuration: permanent vouchers exist in this series governed by this configuration.');
    END;

    -- Trigger 11: Prohibit Retroactive Numbering Configuration INSERT
    CREATE TRIGGER trg_prevent_retroactive_config_insert
    BEFORE INSERT ON voucher_number_configs
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = NEW.company_id
          AND v.voucher_type_id = NEW.voucher_type_id
          AND v.series_name = NEW.series_name
          AND v.status IN ('posted', 'reversed', 'cancelled')
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

    # Setup base data
    c.execute("INSERT INTO companies VALUES ('comp1', 'Acme Corp');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp1', 'Sales');")

    # Step 1: Configuration A: applicable_from = 2026-04-01
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg_A', 'comp1', 'vt_sales', 'Primary', 'automatic', '2026-04-01', 'INV-26/', '', 1, 4, 'yearly');
    """)

    # Step 2: Permanent voucher posted on 2026-05-15
    c.execute("INSERT INTO vouchers VALUES ('v1', 'comp1', 'vt_sales', 'Primary', 'INV-26/0001', '2026-05-15', 'posted');")

    # Step 3: Configuration B created with applicable_from = 2026-08-01 (future from vouchers)
    c.execute("""
    INSERT INTO voucher_number_configs VALUES
    ('cfg_B', 'comp1', 'vt_sales', 'Primary', 'automatic', '2026-08-01', 'GST-26/', '', 1, 4, 'yearly');
    """)

    # Step 4: Configuration B is attempted to be updated to applicable_from = 2026-05-01!
    # Under our Trigger 9a, this MUST be strictly aborted!
    blocked_update = False
    try:
        c.execute("UPDATE voucher_number_configs SET applicable_from = '2026-05-01' WHERE id = 'cfg_B';")
    except sqlite3.IntegrityError as e:
        blocked_update = True
        assert "Configuration effective date (applicable_from) is immutable once created" in str(e)
    assert blocked_update, "Trigger 9a MUST block updating applicable_from!"

    # Step 5: Can user delete unreferenced cfg_B and re-insert if no vouchers exist on/after 2026-08-01?
    # Deleting cfg_B is permitted because no vouchers exist on or after 2026-08-01!
    c.execute("DELETE FROM voucher_number_configs WHERE id = 'cfg_B';")
    c.execute("SELECT count(*) FROM voucher_number_configs WHERE id = 'cfg_B';")
    assert c.fetchone()[0] == 0, "Deleting unreferenced cfg_B must succeed"

    # Step 6: But attempting to INSERT cfg_B with applicable_from = '2026-05-01' is BLOCKED by Trigger 11!
    blocked_insert = False
    try:
        c.execute("""
        INSERT INTO voucher_number_configs VALUES
        ('cfg_B_new', 'comp1', 'vt_sales', 'Primary', 'automatic', '2026-05-01', 'GST-26/', '', 1, 4, 'yearly');
        """)
    except sqlite3.IntegrityError as e:
        blocked_insert = True
        assert "Cannot insert retroactive numbering configuration" in str(e)
    assert blocked_insert, "Trigger 11 MUST block inserting retroactive configuration for 2026-05-01 when May 15 voucher exists!"

    conn.close()
    print("Finding 1: Retroactive Configuration UPDATE Bypass Protection Passed 100%!")

if __name__ == "__main__":
    test_config_update_immutability()

