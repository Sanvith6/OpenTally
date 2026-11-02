import sqlite3
import re

def test_formal_namespace():
    conn = sqlite3.connect(":memory:")
    conn.isolation_level = None
    c = conn.cursor()

    # Create tables
    c.execute("""
    CREATE TABLE voucher_types (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        nature TEXT NOT NULL
    );
    """)

    c.execute("""
    CREATE TABLE voucher_number_configs (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        series_code TEXT NOT NULL,
        numbering_method TEXT NOT NULL CHECK(numbering_method IN ('automatic', 'manual', 'manual_override', 'none')),
        restart_period TEXT NOT NULL CHECK(restart_period IN ('never', 'yearly', 'monthly', 'daily')),
        prefix_template TEXT,
        suffix_template TEXT,
        zero_padding INTEGER CHECK(zero_padding IS NULL OR (zero_padding >= 0 AND zero_padding <= 10)),
        applicable_from TEXT NOT NULL,
        created_at TEXT NOT NULL,
        CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name),
        CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code)
    );
    """)

    c.execute("""
    CREATE TABLE vouchers (
        id TEXT PRIMARY KEY,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        voucher_date TEXT NOT NULL,
        status TEXT NOT NULL CHECK(status IN ('draft', 'posted', 'cancelled'))
    );
    """)

    c.execute("""
    CREATE UNIQUE INDEX uq_vouchers_permanent_number
    ON vouchers(company_id, voucher_type_id, voucher_number)
    WHERE status != 'draft';
    """)

    c.execute("""
    CREATE UNIQUE INDEX uq_vouchers_series_sequence
    ON vouchers(company_id, voucher_type_id, series_name, period_start, sequence_number)
    WHERE status != 'draft' AND sequence_number IS NOT NULL;
    """)

    # Populate voucher type
    c.execute("INSERT INTO voucher_types VALUES ('vt1', 'c1', 'Sales', 'sales');")

    # 1. Insert auto series
    c.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg1', 'c1', 'vt1', 'Primary Automatic', 'AUTO', 'automatic', 'yearly',
        'SALES/AUTO/{YY-YY}/', '', 4, '2026-04-01', '2026-04-01T00:00:00Z'
    );
    """)

    # 2. Assert unique series_code constraint prevents duplicate series_code in same company & type
    caught_dup_code = False
    try:
        c.execute("""
        INSERT INTO voucher_number_configs VALUES (
            'cfg2', 'c1', 'vt1', 'Duplicate Code Series', 'AUTO', 'manual', 'never',
            'SALES/DUP/', '', 0, '2026-04-01', '2026-04-01T00:00:00Z'
        );
        """)
    except sqlite3.IntegrityError as e:
        caught_dup_code = True
        assert "UNIQUE constraint failed" in str(e)
    assert caught_dup_code, "Duplicate series_code must be rejected by uq_series_code"

    # 3. Insert valid manual series with distinct series_code 'MAN'
    c.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg2', 'c1', 'vt1', 'Manual Sales', 'MAN', 'manual', 'never',
        'SALES/MAN/', '', 0, '2026-04-01', '2026-04-01T00:00:00Z'
    );
    """)

    # Mathematical language disjointness check:
    # Pattern for cfg1: ^SALES/AUTO/\d{2}-\d{2}/\d{4,}$
    # Pattern for cfg2: ^SALES/MAN/[A-Za-z0-9\-]+$
    regex_auto = r"^SALES/AUTO/\d{2}-\d{2}/\d{4,}$"
    regex_man = r"^SALES/MAN/[A-Za-z0-9\-]+$"

    sample_auto = "SALES/AUTO/26-27/0001"
    sample_man = "SALES/MAN/OFFLINE-101"

    assert re.match(regex_auto, sample_auto) is not None
    assert re.match(regex_man, sample_auto) is None, "Auto number must NOT match manual series pattern"
    assert re.match(regex_man, sample_man) is not None
    assert re.match(regex_auto, sample_man) is None, "Manual number must NOT match auto series pattern"

    # Validation function for manual voucher creation
    def validate_manual_voucher(company_id, voucher_type_id, series_name, input_voucher_number):
        c.execute("""
            SELECT series_code, numbering_method, prefix_template, suffix_template
            FROM voucher_number_configs
            WHERE company_id = ? AND voucher_type_id = ? AND series_name = ?
        """, (company_id, voucher_type_id, series_name))
        row = c.fetchone()
        if not row:
            raise ValueError(f"Unknown series '{series_name}'")
        series_code, method, pfx, sfx = row
        if method not in ('manual', 'manual_override'):
            raise ValueError(f"Series '{series_name}' is not configured for manual entry")

        expected_prefix = f"SALES/{series_code}/"
        if not input_voucher_number.startswith(expected_prefix):
            raise ValueError(f"Manual voucher number '{input_voucher_number}' must start with series prefix '{expected_prefix}'")
        
        # Check against all auto series prefixes to reject hijacking
        c.execute("""
            SELECT series_code, prefix_template FROM voucher_number_configs
            WHERE company_id = ? AND voucher_type_id = ? AND numbering_method = 'automatic'
        """, (company_id, voucher_type_id))
        auto_series = c.fetchall()
        for auto_code, auto_pfx in auto_series:
            # Check if input starts with auto code
            if f"SALES/{auto_code}/" in input_voucher_number:
                raise ValueError(f"Manual voucher cannot hijack automatic series prefix '{auto_code}'")

        return True

    # Test valid manual voucher
    assert validate_manual_voucher('c1', 'vt1', 'Manual Sales', 'SALES/MAN/INV-99') is True

    # Test rejection of manual voucher with incorrect series prefix
    caught_bad_prefix = False
    try:
        validate_manual_voucher('c1', 'vt1', 'Manual Sales', 'INV-99')
    except ValueError as e:
        caught_bad_prefix = True
        assert "must start with series prefix" in str(e)
    assert caught_bad_prefix

    # Test rejection of manual voucher attempting to use auto series prefix
    caught_hijack = False
    try:
        validate_manual_voucher('c1', 'vt1', 'Manual Sales', 'SALES/AUTO/26-27/0001')
    except ValueError as e:
        caught_hijack = True
        assert "must start with series prefix 'SALES/MAN/'" in str(e) or "hijack" in str(e)
    assert caught_hijack

    print("Finding 2: Formal Numbering Namespace Isolation Tests Passed 100%!")

if __name__ == "__main__":
    test_formal_namespace()

