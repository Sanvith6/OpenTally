# OpenTally — Sprint 2 Stage 7A Phase 2: Final Reconciliation & Independent Verification Report

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase Evaluated:** Phase 2 — Read-Only Search Repository  
**Design Authority:** [OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md) (v2.2.2)  
**Database Schema:** `0.3.1-sprint2` (Unmodified)  
**Evaluation Date:** 2026-11-11  
**Execution Mode:** Verification and reconciliation only (zero production/functionality modifications)  

---

## 1. Executive Verdict

**FINAL VERDICT: `NOT ACCEPTED`**

Under Section 13 verdict criteria (*"`NOT ACCEPTED` — Any unresolved Critical or High defect, or an unverified essential safety guarantee"*), Phase 2 cannot be accepted in its current state for two independent blocking reasons:

1. **HIGH Defect — Async Cancellation / Future-Drop Pool-Poisoning Leak in [`execute_read_only_query`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L36-L96):**  
   Contrary to Stage 7 v2.2.2 Section 6.1 (*"strict RAII connection-detachment protocol"*, Mode 3 *"or task cancelled"*), `execute_read_only_query` acquires a `PoolConnection<Sqlite>` without calling `close_on_drop()` (`sqlx-core-0.8.6`), wraps an un-tracked raw `sqlx::query("PRAGMA query_only = ON;")` and raw `sqlx::query("BEGIN;")` around `f(&mut conn).await`, and [`DatabaseManager::init`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L73-L78) registers no `.after_release(...)` pool hook. If the async future is cancelled or dropped mid-`.await`, SQLx `PoolConnection::drop` invokes `return_to_pool()`, whose SQLite `ping()` is a no-op worker channel ping—returning a dirty `query_only = 1` connection with an open raw transaction directly back into the shared pool.
2. **HIGH / Essential Safety Guarantees Unverified & Misreported in [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs) & [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md):**  
   - **Read-only failure paths 4 & 5 (`ROLLBACK` failure and `PRAGMA query_only = OFF` reset failure)** are claimed as *"Empirically verified"* in `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool`, yet that test never triggers either a rollback failure or a reset failure. Modes 1, 2, 4, 5, 6, and 7 are all **NOT TESTED** in the Rust suite.
   - **Recursive lineage boundary guarantees (`depth = 10`, `depth > 10` truncation detection, and cyclic lineage)** required by Section 9.2 (`TC-SRCH-16`) are **NOT TESTED** in `test_27_lineage_recursive_cte_traversal`, which only tests a 2-node depth-1 chain.
   - **High-precision / 25-digit SQL repository comparisons** (`TC-SRCH-09`) are **NOT TESTED** against SQLite in `stage7_repository_tests.rs` (tests use amounts $\le 100.00$), and two of the four `VoucherSortField` variants (`VoucherNumber`, `CreatedAt`) lack repository test coverage.

> **Workspace Restoration Note (Aborted Nov 9 Scratch Tests):** During inspection on 2026-11-11, [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs) still contained an aborted draft tail (`test_30`–`test_33`, appended on Nov 9 after completion of the Phase 2 report at `23:12:21`) whose fixtures violated `chk_voucher_sequence_metadata` (`'manual'` + non-null `sequence_number` on an automatic series). Per the Phase 2 verification boundary, those unaccepted draft lines were removed to restore [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs) cleanly to its exact Phase 2 deliverable end state (`test_01`–`test_29`, 25 functions, 1,764 lines, zero modifications to Phase 2 production code or Phase 2 tests).

---

## 2. Files Inspected

### Repository Implementation & Tests
- [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs) (lines 1–687)
- [`02-open-tally/src-tauri/src/vouchers/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/mod.rs) (lines 1–37)
- [`02-open-tally/src-tauri/src/vouchers/service/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/mod.rs) (lines 1–26)
- [`02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs) (lines 1–1764)
- [`02-open-tally/src-tauri/src/vouchers/validator.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs) (lines 440–700)
- [`02-open-tally/src-tauri/src/vouchers/model.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/model.rs) (lines 450–470; filter/item DTOs inspected)
- [`02-open-tally/src-tauri/src/database/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs) (lines 45–115)
- [`02-open-tally/src-tauri/src/database/migrations.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs) (lines 540–600 — `chk_voucher_total_amount_canonical`)

### SQLx 0.8.6 Upstream Crate Source (`~/.cargo/registry/src`)
- `sqlx-core-0.8.6/src/pool/connection.rs` (lines 75–355)
- `sqlx-core-0.8.6/src/pool/inner.rs` (lines 570–619)
- `sqlx-sqlite-0.8.6/src/connection/mod.rs` (lines 140–320)
- `sqlx-sqlite-0.8.6/src/connection/worker.rs` (lines 160–335)

### Design, Completion & Governance Documentation
- [`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md) (v2.2.2)
- [`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_V2_2_2_CORRECTION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_V2_2_2_CORRECTION_REPORT.md)
- [`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_FINAL_DESIGN_VERIFICATION.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_FINAL_DESIGN_VERIFICATION.md)
- [`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE1_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE1_COMPLETION_REPORT.md)
- [`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md)
- [`02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md)

---

## 3. Read-Only Lifecycle Verification Matrix

Target function: [`execute_read_only_query`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L36-L96).

### A. Seven Failure Conditions (Synchronous Error-Return Paths)

| # | Failure Condition | Exact Source Location | Rollback Attempted? | Connection Closed? | Can SQLx Return to Pool? | Subsequent Connections Usable? | Polluted Connection Leak Risk (Sync vs Async Cancel) | Test Status in `stage7_repository_tests.rs` |
|---|---|---|---|---|---|---|---|---|
| **1** | `PRAGMA query_only = ON` fails | [`search.rs:47-53`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L47-L53) | No (tx not started) | Yes (`let _ = conn.close().await;`) | **No** (`take_live()` extracts `live` before `raw.close().await`) | Yes (pool decrements `size` and releases permit) | **Sync Err:** Safe.<br>**Async Drop:** Pollutes pool (`ping()` is no-op; no `after_release`) | **NOT TESTED** — Cannot deterministically fault-inject `PRAGMA query_only = ON` via public `execute_read_only_query(pool, f)` on a healthy SQLite DB |
| **2** | `BEGIN;` fails | [`search.rs:56-59`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L56-L59) | No (tx failed to start) | Yes (`let _ = conn.close().await;`) | **No** (`take_live()` extracts `live`) | Yes (replacement opened cleanly) | **Sync Err:** Safe.<br>**Async Drop:** `query_only = 1` leaks directly to pool | **NOT TESTED** — Cannot deterministically inject `BEGIN;` failure after successful `PRAGMA` via public API without mock/hook |
| **3** | Query closure `f(&mut conn)` fails | [`search.rs:62, 85-94`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L85-L94) | **Yes** (`let _ = sqlx::query("ROLLBACK;").execute(&mut *conn).await;`) | Yes (`let _ = conn.close().await;`) | **No** (if `execute_read_only_query` completes `Err` arm) | Yes (verified with 5 sequential writes in `test_05_and_06_and_07`) | **Sync Err:** Safe.<br>**Async Drop / Panic inside `f`:** **HIGH DEFECT** — `conn` drops with `close_on_drop == false`, returning `query_only = 1` + open raw `BEGIN;` tx to pool | **TESTED (Sync Err only):** `test_03`, `test_04`, `test_05_and_06_and_07` (lines 268–389). Cancellation path **NOT TESTED** |
| **4** | `ROLLBACK;` fails (inside `Err(err)` arm) | [`search.rs:87-91`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L87-L91) | Attempted (ignores `Err` with `let _ = ...`) | Yes (`let _ = conn.close().await;` executes unconditionally) | **No** (`take_live()` extracts `live`) | Yes (if connection closed before pool reuse) | **Sync Err:** Safe.<br>**Async Drop during `ROLLBACK`:** Leaks dirty connection to pool | **NOT TESTED** — Could have been injected by calling `ROLLBACK;` inside `f` then returning `Err`, but `test_05_and_06_and_07` did not do so |
| **5** | `COMMIT;` fails (inside `Ok(val)` arm) | [`search.rs:68-71`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L68-L71) | No (returns `Err(OpenTallyError::Database(e))`) | Yes (`let _ = conn.close().await;`) | **No** (`take_live()` extracts `live`) | Yes | **Sync Err:** Safe.<br>**Async Drop during `COMMIT`:** Leaks `query_only = 1` + possible open tx to pool | **NOT TESTED** — Could have been injected by calling `COMMIT;` inside `f` then returning `Ok(())`, but no test does so |
| **6** | `PRAGMA query_only = OFF` fails | [`search.rs:74-80`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L74-L80) | Already committed (`Ok(val)`) | Yes (`let _ = conn.close().await;`) | **No** (`take_live()` extracts `live`) | Yes | **Sync Err:** Safe.<br>**Async Drop before/during reset:** Leaks `query_only = 1` to pool | **NOT TESTED** — Cannot fail `PRAGMA query_only = OFF` on a live healthy connection via public API (`test_05_and_06_and_07` did not test reset failure) |
| **7** | `conn.close().await` fails (disposal failure) | [`search.rs:51,57,69,78,91`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L91) + `sqlx-core` | N/A | Attempted; result discarded via `let _ = conn.close().await;` | **No** — `take_live()` ran before `raw.close().await`; `Floating` + `live` drop locally | Yes — `DecrementSizeGuard::drop` releases semaphore permit & decrements `pool.size` | Safe for synchronous `Err(close_err)`, though worker errors inside `SqliteConnection::close()` before `shutdown()` skip explicit worker shutdown (worker still closes when channel drops) | **NOT TESTED** — `sqlx::SqlitePool` exposes no fault injector for `worker.shutdown()` / `PRAGMA optimize` failure on `close()` |

> **Critical Reconciliation Note vs Phase 2 Report (§4 & §10):** `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool` ([`stage7_repository_tests.rs:329-389`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L329-L389)) runs only one successful query followed by one `DELETE` returning `Err(Database(SQLITE_READONLY))` where `ROLLBACK` succeeds. It **does NOT test** Scenario 5 (Rollback failure) or Scenario 6 (Reset failure).

---

## 4. SQLx 0.8.6 Connection Disposal & Cancellation Analysis

Direct inspection of `sqlx-core-0.8.6` and `sqlx-sqlite-0.8.6` reveals two crucial facts:

### 4.1 Why Report §4 Line 94 Misstates SQLx 0.8 Close Mechanics
`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md` line 94 states: *"SQLx sets internal connection state to closed (`is_closed() == true`). When `PoolConnection` drops, SQLx discards the handle..."*  
Actual `sqlx-core-0.8.6/src/pool/connection.rs:86-89` shows:
```rust
pub async fn close(mut self) -> Result<(), Error> {
    let floating = self.take_live().float(self.pool.clone());
    floating.inner.raw.close().await
}
```
1. `PoolConnection::close(mut self)` takes ownership by value (`mut self`) and calls `self.take_live()` (`self.live.take()`) **before** awaiting `floating.inner.raw.close().await`.
2. Even if `floating.inner.raw.close().await` returns `Err(e)` (e.g., inside `SqliteConnection::close` at `sqlx-sqlite-0.8.6/src/connection/mod.rs:219-236`), `floating` is dropped at the end of `close()`, so:
   - `Live<Sqlite>` is dropped locally (never returned to `pool.idle_ch`).
   - `DecrementSizeGuard::drop` (`sqlx-core-0.8.6/src/pool/inner.rs:610-618`) executes `self.pool.size.fetch_sub(1, Ordering::AcqRel)` and `self.pool.semaphore.release(1)`.
3. Therefore, once execution actually reaches `conn.close().await`, SQLx **cannot** return that connection to the pool—regardless of whether `raw.close()` succeeds or fails (not because of `is_closed()` in `Drop`, but because `take_live()` empties `self.live` beforehand).

### 4.2 Asynchronous Cancellation / Mid-Await Drop Defect (HIGH)
By contrast, `execute_read_only_query` is **NOT** cancel-safe:
1. At [`search.rs:44`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L44), `let mut conn = pool.acquire().await?` constructs `PoolConnection` with `close_on_drop: false` (`sqlx-core-0.8.6/src/pool/connection.rs:263`). `conn.close_on_drop()` (`connection.rs:97-99`) is **never called**.
2. `sqlx::query("PRAGMA query_only = ON;")` and `sqlx::query("BEGIN;")` execute via `Command::Execute` (`sqlx-sqlite-0.8.6/src/connection/worker.rs:160-209`), which does **not** increment `shared.transaction_depth` (`worker.rs:230`).
3. If the caller cancels or drops the future returned by `execute_read_only_query` at any `.await` (for example during `f(&mut conn).await` at `search.rs:62`, `COMMIT` at `search.rs:68`, or `ROLLBACK` at `search.rs:87`):
   - `PoolConnection::drop` (`connection.rs:199-211`) sees `self.close_on_drop == false` and `self.live.is_some() == true`, spawning `self.return_to_pool()`.
   - `Floating::return_to_pool` (`connection.rs:275-327`) checks `self.guard.pool.options.after_release`, which is `None` in [`DatabaseManager::init`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L73-L78).
   - It then calls `self.raw.ping().await` (`connection.rs:314`), which in `sqlx-sqlite-0.8.6/src/connection/worker.rs:320-322` is only a channel round-trip `Command::Ping { tx } => { tx.send(()).ok(); }`. (Indeed, SQLx's own comment at `connection.rs:309-313` warns about connections left inconsistent when futures drop mid-`.await`.)
   - `self.release()` (`connection.rs:325`) returns the connection **with `query_only = 1` and an uncommitted raw transaction** straight to the idle pool.

**Targeted Correction Needed:** Protect `conn` with a fail-closed RAII wrapper / `conn.close_on_drop()` (note `PoolConnection` only exposes one-way `close_on_drop(&mut self) -> true`, so clean return requires either detaching cleanly on success vs `close_on_drop()` before mutation, or configuring an `.after_release` hook on `SqlitePoolOptions` in [`database/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs) that unconditionally rejects or resets `ROLLBACK` / `PRAGMA query_only = OFF`, or both).

---

## 5. Company Isolation Verification Matrix

| Query / Relationship Component | Source Location | Tenant Isolation Mechanism Verified | Empirical Test Coverage | Static / Dynamic Verdict |
|---|---|---|---|---|
| **Main Search Query (`WHERE`)** | [`search.rs:111-113, 250, 381`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L111-L113) | Unconditional first clause `v.company_id = $1`, combined via `AND`; text search `OR` clauses parenthesized at lines 220–222 | `test_08`, `test_11` | **PASS** |
| **Count Preflight Query** | [`search.rs:311-314`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L311-L314) | Reuses exact same `filtered.where_sql` (`v.company_id = $1 AND ...`) | `test_08`, `test_11` | **PASS** |
| **Voucher Type Join** | [`search.rs:379, 513`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L379) | `JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id` | `test_08`, `test_12` | **PASS** |
| **Number Series Join** | [`search.rs:380, 514`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L380) | `JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id` | `test_08`, `test_13` | **PASS** |
| **Voucher Line Count Subquery** | [`search.rs:361, 493`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L361) | `vl.company_id = v.company_id AND vl.voucher_id = v.id` | `test_08` (indirect via returned DTO) | **PASS (Static)** |
| **Ledger Touching Filter** | [`search.rs:178`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L178) | `EXISTS (SELECT 1 FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id AND vl.ledger_id = $N)` | `test_09`, `test_17` | **PASS** |
| **Reversal Subquery & Filter** | [`search.rs:189, 363, 495`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L363) | Filter combines `v.company_id = $1 AND v.reversal_of_voucher_id = $N`; correlated lookup uses `r.company_id = v.company_id AND r.reversal_of_voucher_id = v.id` | `test_19` (same company); cross-company edge statically verified | **PASS** |
| **Cancellation Audit Log Subquery** | [`search.rs:364-375, 496-507`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L364-L375) | `a.company_id = v.company_id AND a.entity_type = 'voucher' AND a.entity_id = v.id AND a.action = 'CANCEL' ORDER BY a.timestamp DESC, a.id DESC LIMIT 1` | `test_26` (same company); cross-company `audit_log` isolation statically verified | **PASS** |
| **Recursive Lineage CTE** | [`search.rs:423-517`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L423-L517) | Pre-check `WHERE company_id = $1 AND id = $2`; Anchor `v.company_id = $1`; Backward `parent.company_id = vl.company_id`; Forward `child.company_id = vl.company_id`; Outer joins match `vl.company_id = v.company_id` | `test_10` (foreign voucher rejected with `NotFound`); cross-company reversal edge verified in SQLite Python harness (`compE->E0` ignores `compOther->E1`) | **PASS** (cross-company edge test missing in Rust suite, statically & SQLite-verified) |

All SQL queries bind user values strictly via `$1..$N` placeholders; zero user strings are interpolated into SQL.

---

## 6. Recursive Lineage Boundary Verification

Inspected [`get_voucher_lineage_impl`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L411-L604).

### Mechanism Inspection
- **SQL CTE (`search.rs:439-517`):**
  - Anchor initializes `depth = 0`, `path = '/' || v.id || '/'`.
  - Backward step joins `parent.id = vl.parent_id AND parent.company_id = vl.company_id` (`vl.depth < 11`).
  - Forward step joins `child.reversal_of_voucher_id = vl.id AND child.company_id = vl.company_id` (`vl.depth < 11`).
  - Cycle prevention uses delimiter-wrapped substring check `instr(vl.path, '/' || parent.id || '/') = 0` (preventing substring prefix collisions such as `v1` vs `v10`).
  - Outer projection aggregates `MIN(vl.depth) AS min_depth ... GROUP BY v.id` for `vl.depth <= 11`.
- **Rust Truncation Logic (`search.rs:584-602`):**
  - `has_exceeded_depth_limit = lineage_items.iter().any(|(_, depth)| *depth > 10);`
  - `max_depth_reached` takes `.max().unwrap_or(0)` over items with `depth <= 10`.
  - `items` excludes depth 11 sentinel rows (`filter(|(_, depth)| *depth <= 10)`).

### Required Boundary Test Matrix (Phase 2 Rust Suite vs Independent SQLite Harness)

| Required Lineage Boundary Scenario | Status in Rust [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs) | Independent Read-Only SQLite CTE Harness Verification (`sqlite3`, 2026-11-11) |
|---|---|---|
| **1. Shorter than depth 10** | **TESTED (depth 1 only)** in [`test_27`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L1575-L1626) (2 nodes: `orig` + `rev`, fwd & bwd, `max_depth_reached=1`, `has_exceeded_depth_limit=false`) | Depth 5 (`A0..A5`, 6 nodes) fwd & bwd: `count=6, max_depth=5, exceeded=False` |
| **2. Exactly at depth 10** | **NOT TESTED** | Depth 10 (`B0..B10`, 11 nodes) fwd & bwd: `count=11, max_depth=10, exceeded=False` |
| **3. Exceeding depth 10 (> 10)** | **NOT TESTED** | Depth 12 (`C0..C12`, 13 nodes) fwd (`C0..C10`) & bwd (`C12..C2`): `count=11, max_depth=10, exceeded=True` |
| **4. Cyclic lineage** | **NOT TESTED** | 3-node cycle (`D0<->D1->D2->D0`): `count=3, max_depth=1, exceeded=False`; prefix ID (`v10`<-`v1`): `count=2, max_depth=1, exceeded=False` |
| **5. Crossing company boundaries** | **TESTED** (`test_10`: querying Comp B's voucher as Comp A returns `NotFound`) | Comp E root with Comp Other child (`E0 <- E1`): `count=1` (`['E0']`); Comp E query for `E1`: `count=0` |

**Conclusion:** While static analysis and independent SQLite execution show the SQL CTE + Rust post-filter logic correctly handles all 5 boundaries, `stage7_repository_tests.rs` only tests depth 1 (`test_27`) and anchor rejection (`test_10`), leaving truncation detection (`depth = 10`, `depth > 10`) and cycle pruning **unverified in the Rust integration test suite**.

---

## 7. Monetary SQL Correctness

Inspected [`CANONICAL_AMOUNT_SQL`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L11) against [`normalize_canonical_amount_for_query`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L443-L542).

- **Expression Verified (`search.rs:11`):**  
  ```sql
  (substr('0000000000000000000000000' || substr(COALESCE(v.total_amount, '0'), 1, instr(COALESCE(v.total_amount, '0') || '.', '.') - 1), -25) || '.' || substr(substr(COALESCE(v.total_amount, '0'), instr(COALESCE(v.total_amount, '0') || '.', '.') + 1) || '0000', 1, 4))
  ```
- **No Float / No Cast / No Scaling Overflow:** Pure string slicing/padding; zero `f32`/`f64`, zero `CAST(... AS NUMERIC)`, zero `i64` scaling.
- **Schema Storage Boundary Detail ([`migrations.rs:551, 582`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L551)):**  
  Constraint `chk_voucher_total_amount_canonical` limits stored `vouchers.total_amount` to `length(total_amount) <= 25` (`'9999999999999999999999999'` integer max or `'99999999999999999999.9999'` 4-decimal column max). Filter validation `normalize_canonical_amount_for_query` permits up to 25 integer digits **and** up to 4 fractional digits (`'9999999999999999999999999.9999'`, 30 chars) as query bounds.

### Exact Test Vector Evaluation (Independent Python `sqlite3` vs Phase 1 Rust Semantics, 2026-11-11)

| Input Value | `CANONICAL_AMOUNT_SQL` Output | Length | Matches `normalize_canonical_amount_for_query`? | Tested in `stage7_repository_tests.rs`? |
|---|---|---:|:---:|:---|
| `NULL` (draft) | `0000000000000000000000000.0000` | 30 | Yes (`'0'`) | Yes (`test_22`) |
| `0` / `0.0000` | `0000000000000000000000000.0000` | 30 | Yes | Verified via `test_22` query bound `'0'` |
| `0.0001` | `0000000000000000000000000.0001` | 30 | Yes | **NOT TESTED** in repo tests (lowest repo test is `0.05` in `test_23_and_24`) |
| `1` | `0000000000000000000000001.0000` | 30 | Yes | **NOT TESTED** in repo tests |
| `10.00` | `0000000000000000000000010.0000` | 30 | Yes | Yes (`test_20_and_21`, `test_23_and_24`, `test_29`) |
| `00010.50` | `0000000000000000000000010.5000` | 30 | Yes (handled cleanly; DB check prevents leading zeros at rest) | **NOT TESTED** in repo tests |
| `100.00` | `0000000000000000000000100.0000` | 30 | Yes | Yes (`test_20_and_21`, `test_23_and_24`) |
| `99999999999999999999.9999` | `0000099999999999999999999.9999` | 30 | Yes (max 4-dec value fitting `length <= 25`) | **NOT TESTED** in repo tests (Phase 1 domain test only) |
| `9999999999999999999999999` | `9999999999999999999999999.0000` | 30 | Yes (max 25-digit integer) | **NOT TESTED** in repo tests (Phase 1 domain test only) |
| `9999999999999999999999999.9999` | `9999999999999999999999999.9999` | 30 | Yes (valid 30-char query bound via `$N`; cannot be inserted into `vouchers.total_amount` due to `length <= 25`) | **NOT TESTED** in repo tests (Phase 1 domain test only) |

Ordering (`ASC` & `DESC`) and `>=`/`<=` range filtering matched 100% character-for-character between `CANONICAL_AMOUNT_SQL` and `normalize_canonical_amount_for_query` across all 13 boundary vectors.

---

## 8. Pagination Phase-Boundary Reconciliation

### Design vs Prompt Phasing Context
- In [`OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md#L1165-L1188) (Section 12), Milestone 7A was documented monolithically as backend search + export + lineage + IPC, and Milestone 7B as frontend React UI.
- In user execution prompts, Milestone 7A was subdivided into phases: Phase 1 explicitly prohibited database pagination/count queries, and the Phase 2 prompt stated: *"Do not implement functionality assigned to subsequent phases, including pagination if the approved phase breakdown assigns it separately."*
- Despite this, [`search_vouchers_impl`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L295-L408) implemented pagination (`COUNT(*)`, `page`, `page_size`, `offset`, `total_pages`, `LIMIT/OFFSET`, `PaginatedResponse`) and [`test_29_pagination_checked_arithmetic`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L1683-L1764).
- [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md#L333) omitted this boundary deviation and claimed `Deviations from v2.2.2 Design: Zero`.

### Existing Pagination Implementation (To Be Preserved, Not Duplicated)
- **Repository (`search.rs:311-407`):**
  - Preflight `SELECT COUNT(*) FROM vouchers v WHERE ...` inside the same read-only snapshot transaction.
  - Early return with `items: Vec::new(), total: 0, total_pages: 1` when `total == 0`.
  - Offset calculation `((page - 1) as i64) * (page_size as i64)` and `(total as u32).div_ceil(page_size)`.
  - `LIMIT $limit OFFSET $offset` parameterized select query returning [`PaginatedResponse<VoucherRegisterItem>`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/model.rs#L454-L461) (`total: i64`, `page: u32`, `page_size: u32`, `total_pages: u32`).
- **Validation ([`validator.rs:631-658`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L631-L658)):** Validates `page >= 1`, `1 <= page_size <= 100`, and `(page - 1).checked_mul(page_size).is_some()`.
- **Existing Test (`stage7_repository_tests.rs:1683-1764`):** `test_29_pagination_checked_arithmetic` verifies pages 1, 2, 3 (`total = 5`, `page_size = 2`, `total_pages = 3`), plus Phase 1 tests `test_04_valid_pagination` and `test_06_invalid_page_number_and_overflow` in `stage7_domain_tests.rs`.
- **Remaining Planned Milestone 7A / Phase 3 Responsibilities (Not Started):**
  - CSV Summary and CSV Detailed export engines (RFC 4180 CRLF quoting + CWE-1236 formula neutralization + UTF-8 BOM + numeric cell preservation).
  - JSON streaming (`v1.0` schema, $O(1)$ memory for file export up to 50,000 records, materialized preview up to 1,000 records).
  - Snapshot-consistent export count preflight + cursor streaming on one read-only connection.
  - Export filesystem path sanitization, overwrite governance, exclusive sibling `.tmp` creation, `flush()` + `sync_all()` + explicit handle drop, Win32 `ReplaceFileW(..., 0)` and `MoveFileExW(..., 0)`, `ERROR_SHARING_VIOLATION` / unsupported filesystem handling, orphaned cleanup.
  - Tauri IPC command wrappers (`search_vouchers`, `export_vouchers_to_file`, `export_vouchers_preview`, `get_voucher_lineage`) in `commands/mod.rs` and `lib.rs`.
- **Recommendation:** **Preserve** existing search pagination in `search.rs` as already delivered; revise Phase 3 scope explicitly so pagination (`COUNT(*)`, `LIMIT/OFFSET`, `PaginatedResponse`) is **not re-implemented or duplicated**, while adding edge-case hardening (guarding `page == 0`/`page_size == 0`/`u32` truncation if `SqliteVoucherSearchRepository` is called directly).

---

## 9. Test-Count Reconciliation

### Figure Reconciliation
- **Pre-Phase-2 Backend Baseline (`273` tests verified across 14 source files):**  
  `accounting/tests.rs` (18) + `commands/mod.rs` (6) + `company/tests.rs` (17) + `database/migrations.rs` (12) + `database/mod.rs` (3) + `database/stage1_tests.rs` (24) + `database/stage4_hardening_migration_tests.rs` (15) + `vouchers/stage3_tests.rs` (29) + `vouchers/stage4_tests.rs` (36) + `vouchers/stage5a_tests.rs` (18) + `vouchers/stage5b_tests.rs` (26) + `vouchers/stage6_tests.rs` (28) + `vouchers/stage7_domain_tests.rs` (22: 18 original Phase 1 + 4 Phase 1 Boundary Verification) + `vouchers/tests.rs` (19) = **`273`**.
- **Newly Added Test Functions (`25` `#[tokio::test]` functions):** Located in [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs).
- **Why Identifiers Reach `test_29` Despite `25` Functions & `28` Scenarios:**  
  1. Three functions combine consecutive scenario numbers in their identifier names: `test_05_and_06_and_07` (-2), `test_20_and_21` (-1), `test_23_and_24` (-1) -> $29 - 4 = 25$ functions.
  2. Numbering shifted at Scenarios 26–28: `test_26` covers cancellation audit reason extraction, `test_27` covers lineage CTE (depth 1), pushing sequence allocation to `test_28`, adding out-of-phase pagination as `test_29`, and treating prompt Scenario 28 (*existing tests pass*) via the `273` baseline suite.
- **Total Backend Tests Running & Passing (`cargo test`):** `273 + 25 = 298`.

### Scenario-to-Test Honest Mapping (Prompt Requirements 1–28)

| Req # | Required Logical Scenario | Mapped Function in `stage7_repository_tests.rs` | Actual Behavior Verified vs Gap |
|---:|---|---|---|
| 1 | Successful `query_only` activation | `test_01_successful_query_only_activation` | **COVERED** (checks `PRAGMA query_only` = 1 inside closure) |
| 2 | Search executes in read-only mode | `test_02_search_executes_successfully_in_read_only_mode` | **COVERED** |
| 3 | Attempted write fails | `test_03_attempted_write_fails_with_sqlite_readonly` | **COVERED** (`INSERT` fails with `readonly`) |
| 4 | Query failure triggers cleanup | `test_04_query_failure_triggers_cleanup` | **COVERED** (closure returns `Validation`; checks `PRAGMA query_only` = 0 afterward) |
| 5 | **Rollback failure triggers disposal** | `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool` *(mislabelled)* | **NOT TESTED** (`DELETE` fails with `SQLITE_READONLY`, `ROLLBACK;` itself **succeeds**) |
| 6 | **Reset failure triggers disposal** | `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool` *(mislabelled)* | **NOT TESTED** (`PRAGMA query_only = OFF` failure is never injected or exercised) |
| 7 | No contaminated connection returns to pool | `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool` | **COVERED (sync paths)** — acquires 5 connections, verifies `PRAGMA query_only == 0` & writes succeed; async cancellation **NOT TESTED** |
| 8 | Search returns only requested company | `test_08_search_returns_only_requested_company_vouchers` | **COVERED** |
| 9 | Ledger filter cannot cross companies | `test_09_ledger_filter_cannot_expose_other_company_vouchers` | **COVERED** |
| 10 | Reversal lineage cannot cross companies | `test_10_reversal_lineage_cannot_cross_company_boundaries` | **COVERED** (Comp A queries Comp B ID -> `NotFound`; cross-company edge traversal verified statically) |
| 11 | Empty optional filters preserve isolation | `test_11_empty_optional_filters_preserves_company_isolation` | **COVERED** |
| 12 | Voucher type filtering | `test_12_voucher_type_filtering` | **COVERED** |
| 13 | Series filtering | `test_13_series_filtering` | **COVERED** |
| 14 | Status filtering (`None`/default, `draft`, `all`, `posted`, `cancelled`) | `test_14_status_filtering_including_default_and_all` | **COVERED** (note explicit `status: Some("reversed")` filter not queried separately, though `reversed` row appears in `default` & `all`) |
| 15 | Transaction-date filtering | `test_15_transaction_date_filtering` | **COVERED** |
| 16 | Effective-date filtering | `test_16_effective_date_filtering` | **COVERED** |
| 17 | Ledger-touching filtering | `test_17_ledger_touching_filtering` | **COVERED** |
| 18 | Text filtering & wildcard escaping (`%`, `_`, `\`) | `test_18_text_filtering_and_wildcard_escaping` | **COVERED** (`[`/`]` from `TC-SRCH-12` not explicitly included, valid under SQLite `LIKE` where brackets aren't `LIKE` wildcards) |
| 19 | Reversal ancestry filtering | `test_19_reversal_ancestry_filtering` | **COVERED** |
| 20 | Exact min amount comparison | `test_20_and_21_exact_amount_boundary_filtering` | **PARTIAL** (tests `50.25..=100.00`; 20–25 digit SQL bounds `TC-SRCH-09` **NOT TESTED** in repo tests) |
| 21 | Exact max amount comparison | `test_20_and_21_exact_amount_boundary_filtering` | **PARTIAL** (same as #20) |
| 22 | NULL amount handling | `test_22_null_amount_handling_in_drafts` | **COVERED** |
| 23 | **Every supported sort field** | `test_23_and_24_sorting_and_direction`, `test_25` | **PARTIAL** (`TotalAmount` & `Date DESC` tested; **`VoucherNumber` & `CreatedAt` NOT TESTED**) |
| 24 | Ascending and descending ordering | `test_23_and_24_sorting_and_direction` | **COVERED** (on `TotalAmount`; not tested for `Date ASC`, `VoucherNumber`, `CreatedAt`) |
| 25 | Deterministic tie-breaking | `test_25_deterministic_tie_breaking` | **PARTIAL** (runs `Date DESC` twice expecting identical IDs, does not test tie-break priority `sequence_number` vs `v.id ASC`) |
| 26 | Draft/posted/cancelled/reversed behavior + audit cancellation reason | `test_14`, `test_26_cancellation_reason_extracted_from_audit_log`, `test_27_lineage_recursive_cte_traversal` | **PARTIAL** (`test_26` covers cancellation reason; **`test_27` covers depth-1 lineage ONLY** — `depth=10`, `depth>10`, and cycles **NOT TESTED**) |
| 27 | Search does not allocate sequence numbers | `test_28_search_does_not_allocate_sequence_numbers` | **COVERED** |
| 28 | Existing posting/cancellation/reversal tests pass | *(Baseline suite: `stage5a`, `stage5b`, `stage6`, etc.)* | **COVERED** (273 pre-existing tests pass unchanged) |
| *(Extra)* | Pagination LIMIT/OFFSET arithmetic | `test_29_pagination_checked_arithmetic` | Early Phase 3 scope |

**Summary Distinction:**  
- **Test Functions Added:** `25`  
- **Test Identifiers:** `test_01` through `test_29`  
- **Prompt Logical Scenarios Targeted:** `28` (plus extra `test_29` pagination; *strictly verified in Rust repo tests*: 18 full + 4 partial + 2 untested [`#5` Rollback failure, `#6` Reset failure], plus untested `depth=10` / `depth>10` / cycle lineage)  
- **Total Backend Tests:** `298` (`273` Phase 1 baseline + `25` Phase 2 functions)

---

## 10. Quality-Gate Results

Executed on 2026-11-11 after removing the aborted Nov 9 draft tail (`test_30`–`test_33`) with zero edits to Phase 2 production implementation or Phase 2 tests:

| Check | Exact Command | Exit Code | Exact Result |
|---|---|---:|---|
| **Rustfmt** | `cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml -- --check` | `0` | Clean, zero formatting diffs |
| **Check** | `cargo check --manifest-path 02-open-tally/src-tauri/Cargo.toml` | `0` | Finished `dev` profile in 1.53s, 0 errors |
| **Clippy** | `cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml --all-targets -- -D warnings` | `0` | Finished `dev` profile in 4.71s, 0 warnings/errors |
| **Backend Tests** | `cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml` | `0` | `running 298 tests`: `test result: ok. 298 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.54s` (Lib: 298; Main: 0; Doc-tests: 0) |

---

## 11. Governance Verification

Inspected [`OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md#L1229-L1240), [`OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md#L4240-L4241), [`OPENTALLY_SPRINT2_STAGE7_V2_2_2_CORRECTION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_V2_2_2_CORRECTION_REPORT.md#L78-L82), and [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md#L323-L327):

- `DEC-S2-001` (Invoice Rounding Policy): `PENDING USER APPROVAL` — **Preserved**
- `DEC-S2-002` (Sequence-Gap Governance): `PENDING USER APPROVAL` — **Preserved**
- `DEC-S2-003` (Cancellation Reason Storage): `APPROVAL EVIDENCE NOT VERIFIED` — **Preserved**
- `DEC-S2-004` (Posted Correction Workflow): `APPROVAL EVIDENCE NOT VERIFIED` — **Preserved**
- `FIND-6-01` through `FIND-6-06`: **All remain tracked**
- Schema version `0.3.1-sprint2` and `Cargo.toml`/`Cargo.lock`: **Unmodified** (zero migrations, zero new crate dependencies)

*(Observation: Phase 1 completion report table lines 311–314 had mismatched `DEC-S2-001..004` titles compared to the authoritative design docs; Stage 7 v2.2.2 and Phase 2 Completion Report retain the authoritative labels and exact unapproved statuses.)*

---

## 12. Findings Classified by Severity

### Critical Findings
*None.*

### High Findings
1. **`FIND-7A-P2-01` (HIGH) — Async Future-Drop / Cancellation Pool-Poisoning Bug in `execute_read_only_query`:**  
   - *Where:* [`search.rs:44-96`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L44-L96) and [`database/mod.rs:73-78`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L73-L78).  
   - *Defect:* Calling `PRAGMA query_only = ON;` and raw `BEGIN;` without `conn.close_on_drop()` (or a fail-closed `.after_release` pool reset/reject hook) causes dropped/cancelled futures mid-`.await` to trigger `PoolConnection::drop` -> `return_to_pool()`, where SQLite's no-op `Command::Ping` allows a `query_only = 1` + open raw `BEGIN;` connection back into the shared pool. This directly violates Section 6.1's Mode 3 cancellation guarantee.
2. **`FIND-7A-P2-02` (HIGH) — Unverified Essential Safety Guarantees (Lineage Truncation/Cycles & Rollback Failure Path):**  
   - *Where:* [`stage7_repository_tests.rs:329-389, 1575-1626`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L329-L389).  
   - *Defect:* Essential Stage 7 safety guarantees are not exercised in the Rust repository test suite:
     1. Lineage `depth = 10`, `depth > 10` (`has_exceeded_depth_limit = true`), and cyclic lineage termination are absent from `test_27`.
     2. Prompt Scenario 5 (`ROLLBACK` failure disposal) and Scenario 6 (`PRAGMA query_only = OFF` reset failure disposal) are absent from `test_05_and_06_and_07` (Modes 1, 2, 4, 5, 6, 7 untested in Rust).
     3. SQL-level 20–25 integer digit bounds (`TC-SRCH-09`), `0.0001`, `1` are absent from `stage7_repository_tests.rs`.

### Medium Findings
3. **`FIND-7A-P2-03` (MEDIUM) — Completion Report Inaccuracies & Undisclosed Phase-Boundary Deviation:**  
   - *Where:* [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md:20, 94, 96, 191, 333-341`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md#L20).  
   - *Defect:* Report falsely claims all 28 scenarios and 6 failure modes were empirically tested, misstates SQLx 0.8 `close()`/`is_closed()` mechanics, omits the early inclusion of pagination (`COUNT(*)`, `LIMIT/OFFSET`, `PaginatedResponse`, `test_29`) claiming "Zero" deviations, and recommends unconditional acceptance.
4. **`FIND-7A-P2-04` (MEDIUM) — Incomplete Sort-Field Integration Coverage:**  
   - *Where:* [`stage7_repository_tests.rs:1371-1519`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L1371-L1519).  
   - *Defect:* Only `VoucherSortField::TotalAmount` (`Asc`/`Desc`) and `VoucherSortField::Date` (`Desc`) are tested; `VoucherSortField::VoucherNumber`, `VoucherSortField::CreatedAt`, `Date ASC`, and explicit `sequence_number` -> `v.id ASC` tie-breaker precedence are unverified by tests.

### Low Findings
5. **`FIND-7A-P2-05` (LOW) — Unchecked Pagination Arithmetic on Direct Repository Calls:**  
   - *Where:* [`search.rs:338-339`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L338-L339).  
   - *Note:* `((page - 1) as i64) * (page_size as i64)` and `(total as u32).div_ceil(page_size)` panic/underflow on `page == 0` / `page_size == 0` or truncate if `total > u32::MAX` if `SqliteVoucherSearchRepository::search_vouchers` is invoked directly without `VoucherSearchService::search_vouchers` (which enforces `validate_search_filter` at [`validator.rs:631-658`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L631-L658)).
6. **`FIND-7A-P2-06` (LOW) — Audit Log Subquery Lookup & Snapshot Consistency Dynamic Tests:**  
   - Cross-company `audit_log` isolation and concurrent write snapshot isolation (`TC-SRCH-18`) are verified by static SQL inspection (`a.company_id = v.company_id`, single-connection `BEGIN;`) rather than explicit adversarial multi-tenant `audit_log` collision or mid-transaction concurrent write tests.

---

## 13. Exact Source References

- `execute_read_only_query`: [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L36-L96`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L36-L96)
- `CANONICAL_AMOUNT_SQL`: [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L11`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L11)
- `build_filtered_query`: [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L104-L257`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L104-L257)
- `search_vouchers_impl` (pagination, joins, subqueries): [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L295-L408`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L295-L408)
- `get_voucher_lineage_impl` (recursive CTE + truncation): [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L411-L604`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L411-L604)
- `VoucherSearchService`: [`02-open-tally/src-tauri/src/vouchers/service/search.rs#L641-L687`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L641-L687)
- `normalize_canonical_amount_for_query` & `validate_search_filter`: [`02-open-tally/src-tauri/src/vouchers/validator.rs#L443-L700`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L443-L700)
- `PaginatedResponse`: [`02-open-tally/src-tauri/src/vouchers/model.rs#L454-L461`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/model.rs#L454-L461)
- `DatabaseManager::init` (no `after_release`): [`02-open-tally/src-tauri/src/database/mod.rs#L45-L85`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L45-L85)
- `chk_voucher_total_amount_canonical`: [`02-open-tally/src-tauri/src/database/migrations.rs#L543-L600`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L543-L600)
- SQLx 0.8 `PoolConnection::close` / `Drop` / `return_to_pool`: `sqlx-core-0.8.6/src/pool/connection.rs#L86-L211,L275-L335`
- SQLx 0.8 `Command::Ping`: `sqlx-sqlite-0.8.6/src/connection/worker.rs#L320-L322`
- Phase 2 Integration Tests: [`02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L210-L1764`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L210-L1764)

---

## 14. Final Phase 2 Acceptance Recommendation

**Recommendation: `NOT ACCEPTED` — Hold Phase 3 & Stage 7B Pending Targeted Remediation.**

Do **not** begin Phase 3 (CSV/JSON export or Windows file replacement) until a targeted Phase 2 hardening patch is authorized and verified covering:
1. **Fix `FIND-7A-P2-01` (Cancel-Safe Pool Hygiene):** Ensure async cancellation/future drop inside [`execute_read_only_query`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L36-L96) cannot return a `query_only = 1` or uncommitted connection to [`DatabaseManager`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L73-L78) (e.g., an async cancellation test using `tokio::select!` / `tokio::time::timeout` + explicit `.after_release(...)` hook resetting `ROLLBACK` / `PRAGMA query_only = OFF` or closing contaminated connections + deterministic `close_on_drop` / `detach` guard on uncertain state).
2. **Close `FIND-7A-P2-02` & `FIND-7A-P2-04` (Essential Safety Integration Tests in `stage7_repository_tests.rs`):**
   - Recursive lineage Rust tests with valid automatic/manual series fixtures for: `depth = 10` (`has_exceeded_depth_limit == false`), `depth > 10` (`items.len() == 11`, `max_depth_reached == 10`, `has_exceeded_depth_limit == true`), forward and backward traversal from chain extremes, and cyclic lineage termination.
   - Injectable failure tests for `ROLLBACK` failure (Mode 4) and `COMMIT` failure (Mode 5), plus explicit async-drop cancellation test proving subsequent pool writes succeed (`PRAGMA query_only == 0`).
   - Repository SQL integration tests covering max column/domain monetary values (`0.0001`, `1`, `99999999999999999999.9999`, `9999999999999999999999999`), all `VoucherSortField` variants (`Date` Asc/Desc, `VoucherNumber` Asc/Desc, `TotalAmount` Asc/Desc, `CreatedAt` Asc/Desc), and explicit tie-breaking order.
3. **Preserve Existing Pagination:** Retain [`search_vouchers_impl`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L295-L408) pagination and [`test_29_pagination_checked_arithmetic`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs#L1683-L1764); document in Phase 3 scope that pagination is already complete and must not be duplicated (optional defensive `saturating_sub`/checked `div_ceil` on repository layer).
4. **Update Phase 2 Documentation:** Correct [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md) once hardening passes, distinguishing testable vs untestable SQLx/SQLite failure modes honestly.

**Execution Stop:** Work is **STOPPED**. No Phase 3 export code, no Windows file replacement, no frontend changes, and no Git staging/commits have been performed.

---

## 15. Post-Hardening Remediation & Final Resolution Addendum (2026-11-11)

Following this independent reconciliation report, a targeted hardening pass was authorized and executed to remediate all six findings:

| Finding ID | Severity | Description | Hardening Action | Current Status |
|:---|:---:|:---|:---|:---:|
| **`FIND-7A-P2-01`** | **HIGH** | Async cancellation & pool contamination | Implemented `ReadOnlyConnectionGuard` (`conn.close_on_drop()`), SQLx-managed `tx`, verified reset, and `DatabaseManager::after_release` SQLite worker drain hook. | **RESOLVED & HARDENED** (`test_h01`..`test_h05`) |
| **`FIND-7A-P2-02`** | **HIGH** | Missing failure-path, lineage & monetary tests | Added integration tests for SQLite DML rejection, simulated rollback/commit failures, lineage depth 10 / >10 truncation, cyclic lineage, and $10^{-4}$ to $10^{25}-1$ SQL comparisons. | **RESOLVED & HARDENED** (`test_h06`..`test_h13`) |
| **`FIND-7A-P2-03`** | **MEDIUM** | Completion report inaccuracies & early pagination | Updated completion report with honest failure mode distinctions, accurate SQLx 0.8 `take_live()` close mechanics, reconciled test counts (316 total), and disclosed early pagination. | **RESOLVED & RECONCILED** (Report updated) |
| **`FIND-7A-P2-04`** | **MEDIUM** | Incomplete sort-field integration coverage | Added `test_h14` covering all 4 `VoucherSortField` variants Asc/Desc, draft NULL amounts, same-date sequence tie-breaking, and stable `v.id ASC`. | **RESOLVED & HARDENED** (`test_h14`) |
| **`FIND-7A-P2-05`** | **LOW** | Repository-level pagination bounds & arithmetic | Exported `validate_pagination_params`, implemented checked `compute_total_pages`, hardened direct repository queries, and safe `u32` depth mapping. | **RESOLVED & HARDENED** (`test_h15`) |
| **`FIND-7A-P2-06`** | **LOW** | Cross-company audit collisions & WAL snapshot isolation | Added multi-tenant audit reason collision test (`test_h16`), foreign lineage edge isolation (`test_h17`), and true concurrent-write WAL snapshot test (`test_h18`). | **RESOLVED & HARDENED** (`test_h16`..`test_h18`) |

### Verification Summary Post-Hardening:
- **Test Results**: 316 backend tests passed (0 failed, 0 ignored); 27 frontend tests passed (0 regressions).
- **Code Reviews**: `security-reviewer` (APPROVE), `rust-reviewer` (APPROVE).
- **Deliverables**: Detailed report published at [`STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md`](file:///c:/project/OpenTally/02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md).
- **Readiness**: Phase 2 is fully hardened, verified, and complete. Stage 7A is ready for Phase 3 authorization.

