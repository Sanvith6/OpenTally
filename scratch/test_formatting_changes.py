import sqlite3

def run_formatting_tests():
    from test_period_anchor import setup_db, allocate_voucher_number
    
    conn = setup_db()
    cur = conn.cursor()
    
    # Sequence starts in May 2026:
    # Initial config: applicable_from 2026-04-01, prefix 'INV-26/', suffix '', width 4, start_number 1
    # 1. Post V1 on 2026-05-10
    n1, fmt1 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-05-10')
    print(f"V1 (2026-05-10): seq={n1}, formatted={fmt1}")
    assert n1 == 1 and fmt1 == "INV-26/0001"
    
    # 2. Prefix change: on 2026-07-01, add config with prefix 'GST-26/', width 4, suffix ''
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg_prefix', 'c1', 'vt_sales', 'Primary', '2026-07-01', 'GST-26/', '', 1, 4, 'yearly'
    )
    """)
    conn.commit()
    
    # Post V2 on 2026-07-15
    n2, fmt2 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-07-15')
    print(f"V2 (2026-07-15) Prefix change: seq={n2}, formatted={fmt2}")
    assert n2 == 2 and fmt2 == "GST-26/0002"
    
    # 3. Suffix change: on 2026-09-01, add config with prefix 'GST-26/', suffix '/DEL', width 4
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg_suffix', 'c1', 'vt_sales', 'Primary', '2026-09-01', 'GST-26/', '/DEL', 1, 4, 'yearly'
    )
    """)
    conn.commit()
    
    # Post V3 on 2026-09-10
    n3, fmt3 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-09-10')
    print(f"V3 (2026-09-10) Suffix change: seq={n3}, formatted={fmt3}")
    assert n3 == 3 and fmt3 == "GST-26/0003/DEL"
    
    # 4. Width change: on 2026-11-01, add config with prefix 'GST-26/', suffix '/DEL', width 6
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg_width', 'c1', 'vt_sales', 'Primary', '2026-11-01', 'GST-26/', '/DEL', 1, 6, 'yearly'
    )
    """)
    conn.commit()
    
    # Post V4 on 2026-11-05
    n4, fmt4 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-11-05')
    print(f"V4 (2026-11-05) Width change: seq={n4}, formatted={fmt4}")
    assert n4 == 4 and fmt4 == "GST-26/000004/DEL"
    
    # 5. Start-number change after initialization: on 2026-12-01, add config with start_number = 500
    cur.execute("""
    INSERT INTO voucher_number_configs VALUES (
        'cfg_startnum', 'c1', 'vt_sales', 'Primary', '2026-12-01', 'GST-26/', '/DEL', 500, 6, 'yearly'
    )
    """)
    conn.commit()
    
    # Post V5 on 2026-12-10
    n5, fmt5 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-12-10')
    print(f"V5 (2026-12-10) Post-init start_number change: seq={n5}, formatted={fmt5}")
    # Counter must NOT reset or jump! It must continue sequentially: 4 -> 5!
    assert n5 == 5 and fmt5 == "GST-26/000005/DEL"
    
    # 6. Backdated voucher post: on 2026-06-15 (between V1 and V2)
    n6, fmt6 = allocate_voucher_number(conn, 'c1', 'vt_sales', 'Primary', '2026-06-15')
    print(f"V6 (2026-06-15) Backdated voucher: seq={n6}, formatted={fmt6}")
    # Sequence increments to 6, formatting resolves active config on 2026-06-15 (prefix 'INV-26/', suffix '', width 4)
    assert n6 == 6 and fmt6 == "INV-26/0006"

    print("\nAll formatting and sequence continuation assertions passed successfully!")

run_formatting_tests()

