# Simulation of the 4 migration exit states in Python / SQLite
import sqlite3

def run_migration_sim(scenario):
    con = sqlite3.connect(':memory:')
    fk_disabled = False
    in_tx = False
    mig_res = None
    conn_discarded = False
    
    try:
        # Step 4a: Disable foreign keys
        if scenario == 'fail_disable_fk':
            raise RuntimeError("Failed to disable FK")
        con.execute("PRAGMA foreign_keys = OFF;")
        fk_disabled = True
        
        status = con.execute("PRAGMA foreign_keys;").fetchone()[0]
        if status != 0:
            raise RuntimeError("FK status not 0")
            
        # Step 4b: BEGIN IMMEDIATE
        if scenario == 'fail_begin':
            raise RuntimeError("Failed BEGIN IMMEDIATE")
        con.execute("BEGIN IMMEDIATE;")
        in_tx = True
        
        # Step 4c: DDL
        if scenario in ('fail_ddl', 'fail_rollback'):
            raise RuntimeError("DDL statement error")
            
        # Step 4d: COMMIT
        if scenario == 'fail_commit':
            raise RuntimeError("COMMIT failed")
        con.execute("COMMIT;")
        in_tx = False
        mig_res = 'OK'
    except Exception as e:
        mig_res = f"ERR: {e}"
        
    # Section 5: Cleanup
    cleanup_err = None
    
    # 5a. Rollback if transaction was started and not committed
    if in_tx:
        try:
            if scenario == 'fail_rollback':
                raise RuntimeError("Disk I/O error during rollback")
            con.execute("ROLLBACK;")
            in_tx = False
        except Exception as rb_err:
            # State C: Rollback failure -> Discard connection
            conn_discarded = True
            con.close()
            return "State C: Rollback Failed -> Connection Discarded"
            
    # 5b. Foreign key restoration
    if fk_disabled:
        fk_ok = False
        try:
            if scenario == 'fail_restore_fk':
                raise RuntimeError("Failed to re-enable FK")
            con.execute("PRAGMA foreign_keys = ON;")
            fk_status = con.execute("PRAGMA foreign_keys;").fetchone()[0]
            if scenario == 'fail_verify_fk':
                fk_status = 0
            if fk_status == 1:
                fk_ok = True
        except Exception as e:
            fk_ok = False
            
        if not fk_ok:
            # State D: FK Restoration / Verification Failed -> Discard connection
            conn_discarded = True
            con.close()
            if mig_res == 'OK':
                return "State D: State A Restoration Failure -> Connection Discarded"
            else:
                return "State D: FK Restoration Failed -> Connection Discarded"
                
    if mig_res == 'OK':
        return "State A: Clean Migration Success -> Connection Returned Healthy"
    else:
        return "State B: Clean Rollback -> Connection Returned Healthy"

print(run_migration_sim('normal_success'))
print(run_migration_sim('fail_ddl'))
print(run_migration_sim('fail_rollback'))
print(run_migration_sim('fail_restore_fk'))
print(run_migration_sim('fail_verify_fk'))
print(run_migration_sim('fail_begin'))
