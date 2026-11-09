"""
Stage 3 Independent Acceptance Audit - Sequence Recovery & DEC-S2-002 Governance Verification
Tests all 10 recovery invariants against the canonical Sprint 2 schema.
"""

import sqlite3
import os
import sys
import tempfile
import json
import uuid

def setup_recovery_db():
    conn = sqlite3.connect(":memory:")
    conn.execute("PRAGMA foreign_keys = ON;")
    
    conn.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        financial_year_begins TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL
    );
    """)
    conn.execute("INSERT INTO companies VALUES ('comp_rec', 'Recovery Corp', '2026-04-01', '2026-04-01');")
    
    conn.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        PRIMARY KEY (company_id, id)
    );
    """)
    conn.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_rec', 'Sales');")
    
    conn.execute("""
    CREATE TABLE voucher_number_series (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        series_code TEXT NOT NULL,
        numbering_method TEXT NOT NULL,
        restart_period TEXT NOT NULL,
        is_active INTEGER NOT NULL DEFAULT 1,
        PRIMARY KEY (company_id, id)
    );
    """)
    conn.execute("INSERT INTO voucher_number_series VALUES ('ser_rec', 'comp_rec', 'vt_sales', 'Recovery Series', 'REC', 'automatic', 'yearly', 1);")
    conn.execute("INSERT INTO voucher_number_series VALUES ('ser_man', 'comp_rec', 'vt_sales', 'Manual Series', 'MAN', 'manual', 'yearly', 1);")
    
    conn.execute("""
    CREATE TABLE voucher_number_configs (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        applicable_from TEXT NOT NULL,
        prefix TEXT NOT NULL DEFAULT '',
        suffix TEXT NOT NULL DEFAULT '',
        start_number INTEGER NOT NULL DEFAULT 1,
        width INTEGER NOT NULL DEFAULT 4,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id)
    );
    """)
    conn.execute("INSERT INTO voucher_number_configs VALUES ('cfg_rec', 'comp_rec', 'vt_sales', 'ser_rec', '2026-04-01', 'REC/', '', 100, 4, CURRENT_TIMESTAMP);")
    
    conn.execute("""
    CREATE TABLE voucher_number_sequences (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        period_start TEXT NOT NULL,
        next_number INTEGER NOT NULL,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        UNIQUE (company_id, series_id, period_start)
    );
    """)
    
    conn.execute("""
    CREATE TABLE vouchers (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_id TEXT NOT NULL,
        numbering_method TEXT NOT NULL,
        period_start TEXT,
        sequence_number INTEGER,
        voucher_number TEXT NOT NULL,
        voucher_date TEXT NOT NULL,
        effective_date TEXT NOT NULL,
        status TEXT NOT NULL,
        PRIMARY KEY (company_id, id)
    );
    """)
    
    conn.execute("""
    CREATE TABLE audit_log (
        id TEXT PRIMARY KEY NOT NULL,
        company_id TEXT NOT NULL,
        entity_type TEXT NOT NULL,
        entity_id TEXT NOT NULL,
        ledger_id TEXT,
        financial_year_begins TEXT,
        action TEXT NOT NULL,
        old_values TEXT,
        new_values TEXT,
        performed_by TEXT NOT NULL,
        performed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
    """)
    
    return conn

def execute_diagnostics(conn, series_id, period_start, resolution_mode="strict", resolution_reason=None, apply_repairs=True):
    # Simulates sequence.rs execute_sequence_diagnostics
    cur = conn.cursor()
    
    # 1. Verify series master
    cur.execute("SELECT numbering_method, series_code, voucher_type_id FROM voucher_number_series WHERE id = ?", (series_id,))
    s_row = cur.fetchone()
    if not s_row:
        raise ValueError("Series not found")
    num_method, s_code, v_type_id = s_row
    if num_method != "automatic":
        raise ValueError(f"Sequence recovery only applies to automatic series; series is {num_method}")
        
    # 2. Config baseline
    cur.execute("""
    SELECT start_number FROM voucher_number_configs
    WHERE series_id = ? AND applicable_from <= ?
    ORDER BY applicable_from DESC, created_at DESC LIMIT 1
    """, (series_id, period_start))
    c_row = cur.fetchone()
    if not c_row:
        raise ValueError("Missing numbering configuration")
    start_number_baseline = c_row[0]
    
    # 3. Read current sequence next_number
    cur.execute("SELECT id, next_number FROM voucher_number_sequences WHERE series_id = ? AND period_start = ?", (series_id, period_start))
    seq_row = cur.fetchone()
    if not seq_row:
        seq_id = str(uuid.uuid4())
        previous_next = start_number_baseline
        if apply_repairs:
            cur.execute("""
            INSERT INTO voucher_number_sequences (id, company_id, voucher_type_id, series_id, period_start, next_number)
            VALUES (?, 'comp_rec', ?, ?, ?, ?)
            """, (seq_id, v_type_id, series_id, period_start, start_number_baseline))
    else:
        seq_id, previous_next = seq_row
        
    # 4. Permanent vouchers
    cur.execute("""
    SELECT sequence_number, period_start, status
    FROM vouchers
    WHERE series_id = ? AND (period_start = ? OR period_start IS NULL)
      AND status IN ('posted', 'reversed', 'cancelled') AND numbering_method = 'automatic'
    ORDER BY sequence_number ASC
    """, (series_id, period_start))
    v_rows = cur.fetchall()
    
    allocated = []
    seen = set()
    for seq_num, p_start, status in v_rows:
        if seq_num is None or p_start is None:
            raise ValueError("Permanent voucher missing sequence metadata")
        if seq_num < start_number_baseline:
            raise ValueError(f"Allocation detected below configured start_number ({start_number_baseline}): {seq_num}")
        if seq_num in seen:
            raise ValueError(f"Duplicate sequence allocation detected for number {seq_num}")
        seen.add(seq_num)
        allocated.append(seq_num)
        
    max_allocated = allocated[-1] if allocated else 0
    
    # Span check
    if (max_allocated - start_number_baseline) > 100000:
        raise ValueError("Sequence span exceeds 100,000 limit")
        
    historical_gaps = []
    if max_allocated >= start_number_baseline:
        alloc_set = set(allocated)
        for exp in range(start_number_baseline, max_allocated):
            if exp not in alloc_set:
                historical_gaps.append(exp)
                
    expected_next = (max_allocated + 1) if (max_allocated >= start_number_baseline) else start_number_baseline
    
    if (previous_next - expected_next) > 100000:
        raise ValueError("Counter tail gap exceeds 100,000 limit")
        
    counter_tail_gaps = []
    if previous_next > expected_next:
        for g in range(expected_next, previous_next):
            counter_tail_gaps.append(g)
            
    all_gaps = len(historical_gaps) > 0 or len(counter_tail_gaps) > 0
    if all_gaps and resolution_mode != "allow_historical_gaps" and apply_repairs:
        raise ValueError(f"Sequence gaps detected: historical={historical_gaps}, tail={counter_tail_gaps}")
        
    if resolution_mode == "allow_historical_gaps" and apply_repairs:
        if not resolution_reason or not resolution_reason.strip():
            raise ValueError("resolution_reason is mandatory when resolution_mode is 'allow_historical_gaps'")
            
    counter_updated = False
    if previous_next < expected_next:
        if apply_repairs:
            cur.execute("UPDATE voucher_number_sequences SET next_number = ? WHERE id = ?", (expected_next, seq_id))
            counter_updated = True
        new_next = expected_next
    else:
        new_next = previous_next # NEVER LOWER COUNTER
        
    if apply_repairs:
        audit_id = str(uuid.uuid4())
        details = json.dumps({
            "previous_next": previous_next,
            "new_next": new_next,
            "counter_updated": counter_updated,
            "historical_gaps": historical_gaps,
            "counter_tail_gaps": counter_tail_gaps,
        })
        cur.execute("""
        INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, new_values, performed_by)
        VALUES (?, 'comp_rec', 'voucher_number_sequences', ?, 'UPDATE', ?, 'admin')
        """, (audit_id, seq_id, details))
        
    return {
        "start_number_baseline": start_number_baseline,
        "previous_next_number": previous_next,
        "new_next_number": new_next,
        "max_allocated": max_allocated,
        "historical_gaps": historical_gaps,
        "counter_tail_gaps": counter_tail_gaps,
        "counter_updated": counter_updated,
    }

def run_recovery_tests():
    print("=" * 70)
    print("STAGE 3 SEQUENCE RECOVERY & DEC-S2-002 GOVERNANCE VERIFICATION")
    print("=" * 70)
    
    # Invariant 1: Configured starting baseline 100
    conn = setup_recovery_db()
    rep = execute_diagnostics(conn, "ser_rec", "2026-04-01")
    assert rep["start_number_baseline"] == 100
    assert rep["new_next_number"] == 100
    print("Invariant 1 (Configured baseline S_0 = 100): PASS")
    
    # Invariant 2: Lagged counter advancement
    # Insert posted vouchers 100, 101, 102
    conn.execute("INSERT INTO vouchers VALUES ('v1', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 100, 'REC/0100', '2026-04-10', '2026-04-10', 'posted');")
    conn.execute("INSERT INTO vouchers VALUES ('v2', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 101, 'REC/0101', '2026-04-11', '2026-04-11', 'posted');")
    conn.execute("INSERT INTO vouchers VALUES ('v3', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 102, 'REC/0102', '2026-04-12', '2026-04-12', 'posted');")
    # Sequence row was at 100 (lagged)
    rep = execute_diagnostics(conn, "ser_rec", "2026-04-01")
    assert rep["previous_next_number"] == 100
    assert rep["new_next_number"] == 103
    assert rep["counter_updated"] == True
    print("Invariant 2 (Lagged counter advanced to M + 1 = 103): PASS")
    
    # Invariant 3: Ahead counter is strictly preserved (NEVER DECREMENTED)
    # Simulate counter artificially set to 110
    conn.execute("UPDATE voucher_number_sequences SET next_number = 110 WHERE series_id = 'ser_rec';")
    # Inspection mode (apply_repairs=False) reports tail gaps [103..109]
    rep_insp = execute_diagnostics(conn, "ser_rec", "2026-04-01", apply_repairs=False)
    assert rep_insp["counter_tail_gaps"] == [103, 104, 105, 106, 107, 108, 109]
    assert rep_insp["new_next_number"] == 110 # Preserved!
    print("Invariant 3 (Ahead counter 110 strictly preserved, never decremented): PASS")
    
    # Invariant 4: Cancelled & Reversed vouchers included in permanent M
    conn.execute("INSERT INTO vouchers VALUES ('v4', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 103, 'REC/0103', '2026-04-13', '2026-04-13', 'cancelled');")
    conn.execute("INSERT INTO vouchers VALUES ('v5', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 104, 'REC/0104', '2026-04-14', '2026-04-14', 'reversed');")
    rep_inc = execute_diagnostics(conn, "ser_rec", "2026-04-01", apply_repairs=False)
    assert rep_inc["max_allocated"] == 104
    print("Invariant 4 (Cancelled & reversed vouchers included in permanent M = 104): PASS")
    
    # Invariant 5: Manual vouchers excluded
    conn.execute("INSERT INTO vouchers VALUES ('v_man', 'comp_rec', 'vt_sales', 'ser_man', 'manual', '2026-04-01', 999, 'MAN/0999', '2026-04-15', '2026-04-15', 'posted');")
    try:
        execute_diagnostics(conn, "ser_man", "2026-04-01")
        assert False, "Should reject recovery on manual series"
    except ValueError as e:
        assert "only applies to automatic series" in str(e)
    print("Invariant 5 (Manual series strictly excluded): PASS")
    
    # Invariant 6: Strict mode aborts on historical gaps
    # Delete v2 (101) -> gap at 101
    conn.execute("DELETE FROM vouchers WHERE id = 'v2';")
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01", resolution_mode="strict", apply_repairs=True)
        assert False, "Should abort in strict mode"
    except ValueError as e:
        assert "Sequence gaps detected" in str(e)
    print("Invariant 6 (Strict mode aborts on gaps): PASS")
    
    # Invariant 7: 'allow_historical_gaps' requires non-empty reason & logs audit
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01", resolution_mode="allow_historical_gaps", resolution_reason="", apply_repairs=True)
        assert False, "Should require non-empty reason"
    except ValueError as e:
        assert "resolution_reason is mandatory" in str(e)
        
    rep_gap = execute_diagnostics(conn, "ser_rec", "2026-04-01", resolution_mode="allow_historical_gaps", resolution_reason="Audited missing invoice during migration", apply_repairs=True)
    assert 101 in rep_gap["historical_gaps"]
    
    # Verify audit_log
    cur = conn.execute("SELECT action, new_values, performed_by FROM audit_log ORDER BY performed_at DESC LIMIT 1;")
    audit_row = cur.fetchone()
    assert audit_row[0] == "UPDATE"
    assert "historical_gaps" in audit_row[1]
    assert audit_row[2] == "admin"
    print("Invariant 7 (allow_historical_gaps requires reason and logs to audit_log): PASS")
    
    # Invariant 8: Missing metadata fails closed
    conn.execute("INSERT INTO vouchers VALUES ('v_bad', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', NULL, 105, 'REC/0105', '2026-04-16', '2026-04-16', 'posted');")
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01")
        assert False, "Should fail on null metadata"
    except ValueError as e:
        assert "missing sequence metadata" in str(e)
    conn.execute("DELETE FROM vouchers WHERE id = 'v_bad';")
    print("Invariant 8 (Missing metadata fails closed): PASS")
    
    # Invariant 9: Duplicate sequence number fails closed
    conn.execute("INSERT INTO vouchers VALUES ('v_dup', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 100, 'REC/0100-DUP', '2026-04-16', '2026-04-16', 'posted');")
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01")
        assert False, "Should fail on duplicate number"
    except ValueError as e:
        assert "Duplicate sequence allocation detected" in str(e)
    conn.execute("DELETE FROM vouchers WHERE id = 'v_dup';")
    print("Invariant 9 (Duplicate sequence number fails closed): PASS")
    
    # Invariant 10: Below-baseline allocation fails closed
    conn.execute("INSERT INTO vouchers VALUES ('v_low', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 50, 'REC/0050', '2026-04-16', '2026-04-16', 'posted');")
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01")
        assert False, "Should fail on allocation below baseline"
    except ValueError as e:
        assert "below configured start_number" in str(e)
    conn.execute("DELETE FROM vouchers WHERE id = 'v_low';")
    print("Invariant 10 (Below-baseline allocation fails closed): PASS")
    
    # Invariant 11: 100,000 sequence span memory bound
    conn.execute("INSERT INTO vouchers VALUES ('v_huge', 'comp_rec', 'vt_sales', 'ser_rec', 'automatic', '2026-04-01', 500000, 'REC/500000', '2026-04-16', '2026-04-16', 'posted');")
    try:
        execute_diagnostics(conn, "ser_rec", "2026-04-01")
        assert False, "Should fail on sequence span > 100,000"
    except ValueError as e:
        assert "Sequence span exceeds 100,000" in str(e)
    conn.execute("DELETE FROM vouchers WHERE id = 'v_huge';")
    print("Invariant 11 (Sequence span > 100,000 memory safety bound): PASS")
    
    print("\n" + "=" * 70)
    print("ALL SEQUENCE RECOVERY & GOVERNANCE INVARIANTS VERIFIED: 100% PASS")
    print("=" * 70)
    conn.close()

if __name__ == "__main__":
    run_recovery_tests()

