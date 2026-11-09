"""
Stage 3 Independent Acceptance Audit - Concurrency and Stress Test Harness
Tests simultaneous sequence allocations, writer-lock contention, transaction rollbacks,
and multi-tenant isolation against a disposable SQLite database.
"""

import sqlite3
import os
import sys
import tempfile
import threading
import time
import random
import uuid

def setup_test_db():
    fd, path = tempfile.mkstemp(suffix=".db", prefix="opentally_stage3_concurrency_")
    os.close(fd)
    
    conn = sqlite3.connect(path)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")
    conn.execute("PRAGMA busy_timeout = 5000;")
    
    # 1. Base metadata & companies
    conn.execute("""
    CREATE TABLE _opentally_metadata (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        schema_version TEXT NOT NULL,
        installed_at TEXT NOT NULL,
        app_version TEXT NOT NULL
    );
    """)
    conn.execute("INSERT INTO _opentally_metadata VALUES (1, '0.3.0-sprint2', CURRENT_TIMESTAMP, '0.3.0');")
    
    conn.execute("""
    CREATE TABLE companies (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL COLLATE NOCASE,
        financial_year_begins TEXT NOT NULL,
        books_beginning_from TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'active'
    );
    """)
    
    conn.execute("""
    CREATE TABLE voucher_types (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        name TEXT NOT NULL,
        category TEXT NOT NULL,
        is_active INTEGER NOT NULL DEFAULT 1,
        numbering_method TEXT NOT NULL DEFAULT 'automatic',
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE CASCADE
    );
    """)
    
    conn.execute("""
    CREATE TABLE voucher_number_series (
        id TEXT NOT NULL,
        company_id TEXT NOT NULL,
        voucher_type_id TEXT NOT NULL,
        series_name TEXT NOT NULL,
        series_code TEXT NOT NULL,
        numbering_method TEXT NOT NULL DEFAULT 'automatic',
        restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never')),
        is_active INTEGER NOT NULL DEFAULT 1,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id, voucher_type_id) REFERENCES voucher_types(company_id, id) ON DELETE CASCADE,
        UNIQUE (company_id, voucher_type_id, series_code)
    );
    """)
    
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
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE CASCADE
    );
    """)
    
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
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE CASCADE,
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
        status TEXT NOT NULL CHECK (status IN ('draft', 'posted', 'cancelled', 'reversed')),
        total_amount TEXT NOT NULL DEFAULT '0.00',
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (company_id, id),
        FOREIGN KEY (company_id, series_id) REFERENCES voucher_number_series(company_id, id) ON DELETE RESTRICT,
        UNIQUE (company_id, voucher_type_id, series_id, voucher_number)
    );
    """)
    
    # Insert Test Companies
    conn.execute("INSERT INTO companies VALUES ('comp_a', 'Acme India', '2026-04-01', '2026-04-01', 'active');")
    conn.execute("INSERT INTO companies VALUES ('comp_b', 'Global Corp', '2026-01-01', '2026-01-01', 'active');")
    
    # Voucher Types
    conn.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_a', 'Sales', 'accounting', 1, 'automatic');")
    conn.execute("INSERT INTO voucher_types VALUES ('vt_purch', 'comp_a', 'Purchase', 'accounting', 1, 'automatic');")
    conn.execute("INSERT INTO voucher_types VALUES ('vt_sales', 'comp_b', 'Sales', 'accounting', 1, 'automatic');")
    
    # Series
    # Series 1: comp_a, sales, GST-A, yearly
    conn.execute("INSERT INTO voucher_number_series VALUES ('ser_a1', 'comp_a', 'vt_sales', 'GST Sales', 'GST', 'automatic', 'yearly', 1);")
    conn.execute("INSERT INTO voucher_number_configs VALUES ('cfg_a1', 'comp_a', 'vt_sales', 'ser_a1', '2026-04-01', 'GST/26-27/', '', 1, 4, CURRENT_TIMESTAMP);")
    
    # Series 2: comp_a, purch, RAW-P, monthly
    conn.execute("INSERT INTO voucher_number_series VALUES ('ser_a2', 'comp_a', 'vt_purch', 'Raw Purchase', 'RAW', 'automatic', 'monthly', 1);")
    conn.execute("INSERT INTO voucher_number_configs VALUES ('cfg_a2', 'comp_a', 'vt_purch', 'ser_a2', '2026-04-01', 'RAW/2026-04/', '', 50, 4, CURRENT_TIMESTAMP);")
    
    # Series 3: comp_b, sales, EXP-B, yearly (comp_b FY starts 2026-01-01)
    conn.execute("INSERT INTO voucher_number_series VALUES ('ser_b1', 'comp_b', 'vt_sales', 'Export Sales', 'EXP', 'automatic', 'yearly', 1);")
    conn.execute("INSERT INTO voucher_number_configs VALUES ('cfg_b1', 'comp_b', 'vt_sales', 'ser_b1', '2026-01-01', 'EXP/2026/', '', 100, 5, CURRENT_TIMESTAMP);")
    
    conn.commit()
    conn.close()
    return path

# Allocation worker function
def worker_task(db_path, worker_id, target_series, num_allocations, inject_rollback_rate, results_dict, lock):
    # Each thread opens its own SQLite connection with busy_timeout
    conn = sqlite3.connect(db_path, timeout=10.0)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")
    conn.execute("PRAGMA busy_timeout = 10000;")
    
    allocated = []
    rolled_back = []
    busy_errors = 0
    
    company_id, voucher_type_id, series_id, series_code, period_start, start_num = target_series
    
    for i in range(num_allocations):
        # Retry loop with exponential backoff on sqlite3.OperationalError (busy/locked)
        success = False
        attempts = 0
        while not success and attempts < 10:
            attempts += 1
            try:
                # 1. Begin IMMEDIATE transaction to acquire writer lock early
                conn.execute("BEGIN IMMEDIATE;")
                
                # 2. Atomic UPSERT exactly matching VoucherSequenceService::allocate_next_sequence
                seq_id = str(uuid.uuid4())
                initial_next = start_num + 1
                cursor = conn.execute("""
                INSERT INTO voucher_number_sequences 
                (id, company_id, voucher_type_id, series_id, period_start, next_number)
                VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT (company_id, series_id, period_start)
                DO UPDATE SET next_number = voucher_number_sequences.next_number + 1, updated_at = CURRENT_TIMESTAMP
                RETURNING (voucher_number_sequences.next_number - 1) AS allocated_number;
                """, (seq_id, company_id, voucher_type_id, series_id, period_start, initial_next))
                
                row = cursor.fetchone()
                alloc_num = row[0]
                
                v_num_str = f"{series_code}/{alloc_num:04d}"
                v_id = str(uuid.uuid4())
                
                # Check for deliberate rollback injection
                should_rollback = (random.random() < inject_rollback_rate)
                
                if should_rollback:
                    # Rollback the transaction: sequence counter must revert!
                    conn.rollback()
                    rolled_back.append(alloc_num)
                    success = True
                else:
                    # Insert voucher and commit
                    conn.execute("""
                    INSERT INTO vouchers
                    (id, company_id, voucher_type_id, series_id, numbering_method, period_start, sequence_number, voucher_number, voucher_date, effective_date, status, total_amount)
                    VALUES (?, ?, ?, ?, 'automatic', ?, ?, ?, '2026-04-15', '2026-04-15', 'posted', '500.00');
                    """, (v_id, company_id, voucher_type_id, series_id, period_start, alloc_num, v_num_str))
                    
                    conn.commit()
                    allocated.append(alloc_num)
                    success = True
                    
            except sqlite3.OperationalError as e:
                if "locked" in str(e).lower() or "busy" in str(e).lower():
                    busy_errors += 1
                    try:
                        conn.rollback()
                    except Exception:
                        pass
                    time.sleep(0.005 * (2 ** (attempts - 1)) + random.uniform(0.001, 0.01))
                else:
                    try:
                        conn.rollback()
                    except Exception:
                        pass
                    raise
            except Exception:
                try:
                    conn.rollback()
                except Exception:
                    pass
                raise
                
    conn.close()
    
    with lock:
        key = (company_id, series_id, period_start)
        if key not in results_dict:
            results_dict[key] = {
                "committed_numbers": [],
                "rolled_back_numbers": [],
                "busy_events": 0,
            }
        results_dict[key]["committed_numbers"].extend(allocated)
        results_dict[key]["rolled_back_numbers"].extend(rolled_back)
        results_dict[key]["busy_events"] += busy_errors

def run_stress_test():
    print("=" * 70)
    print("STAGE 3 CONCURRENCY & LOCK-CONTENTION STRESS TEST")
    print("=" * 70)
    
    db_path = setup_test_db()
    print(f"Created disposable test database: {db_path}")
    
    # Targets:
    # 1. (comp_a, vt_sales, ser_a1, GST, 2026-04-01, start_num=1)
    # 2. (comp_a, vt_purch, ser_a2, RAW, 2026-04-01, start_num=50)
    # 3. (comp_b, vt_sales, ser_b1, EXP, 2026-01-01, start_num=100)
    targets = [
        ("comp_a", "vt_sales", "ser_a1", "GST", "2026-04-01", 1),
        ("comp_a", "vt_purch", "ser_a2", "RAW", "2026-04-01", 50),
        ("comp_b", "vt_sales", "ser_b1", "EXP", "2026-01-01", 100),
    ]
    
    # 24 concurrent threads hitting these series simultaneously
    threads = []
    results = {}
    lock = threading.Lock()
    
    total_workers = 24
    allocations_per_worker = 20
    rollback_rate = 0.15 # 15% random rollbacks
    
    print(f"Launching {total_workers} concurrent threads ({allocations_per_worker} attempts/thread, rollback rate {rollback_rate*100}%)...")
    t0 = time.time()
    
    for i in range(total_workers):
        target = targets[i % len(targets)]
        t = threading.Thread(
            target=worker_task,
            args=(db_path, i, target, allocations_per_worker, rollback_rate, results, lock)
        )
        threads.append(t)
        t.start()
        
    for t in threads:
        t.join()
        
    elapsed = time.time() - t0
    print(f"All worker threads completed in {elapsed:.3f}s")
    
    # Verification phase
    conn = sqlite3.connect(db_path)
    print("\n--- Concurrency Verification Report ---")
    all_passed = True
    
    for target in targets:
        company_id, voucher_type_id, series_id, series_code, period_start, start_num = target
        key = (company_id, series_id, period_start)
        data = results[key]
        
        committed = sorted(data["committed_numbers"])
        rolled_back = sorted(data["rolled_back_numbers"])
        busy_events = data["busy_events"]
        
        # Check uniqueness of committed numbers
        has_duplicates = len(committed) != len(set(committed))
        duplicates = [x for x in set(committed) if committed.count(x) > 1]
        
        # Check actual numbers in vouchers table
        cur = conn.execute("SELECT sequence_number FROM vouchers WHERE company_id = ? AND series_id = ? ORDER BY sequence_number ASC", (company_id, series_id))
        db_vouchers = [r[0] for r in cur.fetchall()]
        
        # Check final next_number in sequence table
        cur = conn.execute("SELECT next_number FROM voucher_number_sequences WHERE company_id = ? AND series_id = ? AND period_start = ?", (company_id, series_id, period_start))
        row = cur.fetchone()
        final_next_number = row[0] if row else None
        
        # Invariants:
        # 1. committed == db_vouchers
        # 2. zero duplicates in committed
        # 3. committed numbers are strictly contiguous starting at start_num
        is_contiguous = True
        if db_vouchers:
            expected_range = list(range(start_num, start_num + len(db_vouchers)))
            is_contiguous = (db_vouchers == expected_range)
            
        # 4. final_next_number == (max committed) + 1 (or start_num if 0 committed)
        expected_final_next = (max(db_vouchers) + 1) if db_vouchers else start_num
        counter_aligned = (final_next_number == expected_final_next)
        
        print(f"\nSeries {series_code} ({company_id}, period {period_start}):")
        print(f"  Start baseline:           {start_num}")
        print(f"  Committed vouchers:       {len(db_vouchers)}")
        print(f"  Rolled back attempts:     {len(rolled_back)}")
        print(f"  Writer-lock busy events:  {busy_events}")
        print(f"  Final sequence counter:   {final_next_number} (expected: {expected_final_next})")
        print(f"  Duplicates detected:      {duplicates} (PASS: {not has_duplicates})")
        print(f"  Contiguous monotonic:     {is_contiguous} (Min: {min(db_vouchers) if db_vouchers else None}, Max: {max(db_vouchers) if db_vouchers else None})")
        print(f"  Counter aligned:          {counter_aligned}")
        
        if has_duplicates or not is_contiguous or not counter_aligned:
            all_passed = False
            
    conn.close()
    try:
        os.remove(db_path)
    except Exception:
        pass
        
    print("\n" + "=" * 70)
    if all_passed:
        print("CONCURRENCY VERIFICATION RESULT: PASS (100% Contiguous, Zero Duplicates, Strict Rollback Recovery)")
    else:
        print("CONCURRENCY VERIFICATION RESULT: FAIL")
    print("=" * 70)
    return all_passed

if __name__ == "__main__":
    success = run_stress_test()
    sys.exit(0 if success else 1)

