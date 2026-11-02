import sqlite3

def test_series_master_schema():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()

    c.execute("PRAGMA foreign_keys = ON;")

    # 1. Base tables
    c.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL
    );
    """)

    c.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        category TEXT NOT NULL,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );
    """)

    # 2. Normalized Series Master
    c.execute("""
    CREATE TABLE voucher_number_series (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL COLLATE NOCASE,
        series_code TEXT NOT NULL COLLATE NOCASE,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name),
        CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
    );
    """)

    # 3. Effective-Dated Configurations referencing Series Master
    c.execute("""
    CREATE TABLE voucher_number_configs (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1 CHECK (start_number > 0),
        width INTEGER NOT NULL DEFAULT 4 CHECK (width BETWEEN 1 AND 10),
        restart_period TEXT NOT NULL DEFAULT 'yearly' CHECK (restart_period IN ('never', 'yearly', 'monthly', 'daily')),
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        CONSTRAINT uq_config_series_date UNIQUE (company_id, series_id, applicable_from),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE CASCADE,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE
    );
    """)

    # 4. Vouchers header referencing Series Master
    c.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK (numbering_method IN ('automatic', 'manual')),
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        date TEXT NOT NULL,
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        total_amount TEXT,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE RESTRICT,
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE RESTRICT
    );
    """)

    # Triggers for Series Master Immutability
    c.execute("""
    CREATE TRIGGER trg_prevent_series_code_mutation
    BEFORE UPDATE OF series_code, numbering_method ON voucher_number_series
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.series_id = OLD.id
          AND v.status IN ('posted', 'cancelled', 'reversed')
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot modify series_code or numbering_method: permanent vouchers exist in this series.');
    END;
    """)

    c.execute("""
    CREATE TRIGGER trg_prevent_series_deletion_with_vouchers
    BEFORE DELETE ON voucher_number_series
    FOR EACH ROW
    WHEN EXISTS (
        SELECT 1 FROM vouchers v
        WHERE v.company_id = OLD.company_id
          AND v.series_id = OLD.id
          AND v.status IN ('posted', 'cancelled', 'reversed')
    )
    BEGIN
        SELECT RAISE(ABORT, 'Cannot delete series: permanent vouchers exist in this series.');
    END;
    """)

    # Seed company and voucher type
    c.execute("INSERT INTO companies VALUES ('c1', 'Acme Corp');")
    c.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'c1', 'Sales', 'sales');")

    # 1. Create a series master
    c.execute("""
    INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
    VALUES ('s_pri', 'c1', 'vt_sales', 'Primary', 'PRI', 'automatic');
    """)

    # 2. Insert MULTIPLE effective-dated configs for the SAME series
    c.execute("""
    INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width, restart_period)
    VALUES ('cfg1', 'c1', 'vt_sales', 's_pri', '2026-04-01', 'INV/PRI/{YY-YY}/', '', 1, 4, 'yearly');
    """)

    c.execute("""
    INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width, restart_period)
    VALUES ('cfg2', 'c1', 'vt_sales', 's_pri', '2026-08-01', 'GST/PRI/{YY-YY}/', '', 1, 4, 'yearly');
    """)

    c.execute("""
    INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width, restart_period)
    VALUES ('cfg3', 'c1', 'vt_sales', 's_pri', '2027-04-01', 'INV/PRI/{YY-YY}/', '', 1, 4, 'yearly');
    """)

    # Verify all 3 configurations exist under s_pri
    c.execute("SELECT COUNT(*) FROM voucher_number_configs WHERE series_id = 's_pri';")
    assert c.fetchone()[0] == 3, "All 3 effective-dated configurations must be stored successfully for series s_pri"

    # 3. Assert duplicate series_code in same company and voucher type is rejected
    caught_dup_code = False
    try:
        c.execute("""
        INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
        VALUES ('s_pri_dup', 'c1', 'vt_sales', 'Duplicate Primary', 'PRI', 'automatic');
        """)
    except sqlite3.IntegrityError as e:
        caught_dup_code = True
        assert "UNIQUE constraint failed" in str(e)
    assert caught_dup_code, "Duplicate series_code must be rejected by uq_series_code"

    # 4. Insert a posted voucher for s_pri
    c.execute("""
    INSERT INTO vouchers (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, date, status, total_amount)
    VALUES ('v1', 'c1', 'vt_sales', 's_pri', 'automatic', '2026-04-01', 1, 'INV/PRI/26-27/0001', '2026-05-10', 'posted', '1000.00');
    """)

    # 5. Assert updating series_code on s_pri is aborted by trigger
    caught_code_mutation = False
    try:
        c.execute("UPDATE voucher_number_series SET series_code = 'MOD' WHERE id = 's_pri';")
    except sqlite3.IntegrityError as e:
        caught_code_mutation = True
        assert "Cannot modify series_code" in str(e)
    assert caught_code_mutation, "Trigger must prevent mutating series_code once permanent vouchers exist"

    # 6. Assert deleting series s_pri is aborted by trigger
    caught_delete = False
    try:
        c.execute("DELETE FROM voucher_number_series WHERE id = 's_pri';")
    except sqlite3.IntegrityError as e:
        caught_delete = True
        assert "Cannot delete series: permanent vouchers exist" in str(e)
    assert caught_delete, "Trigger must prevent deleting series with permanent vouchers"

    print("Finding 1: Normalized Series Master Schema Tests Passed 100%!")

if __name__ == "__main__":
    test_series_master_schema()

