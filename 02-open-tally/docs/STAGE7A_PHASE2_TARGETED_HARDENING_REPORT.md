# OpenTally — Sprint 2 Stage 7A Phase 2: Targeted Hardening Report

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase:** 2 — Read-Only Search Repository Hardening  
**Design Authority:** [OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md) (v2.2.2)  
**Database Schema:** `0.3.1-sprint2` (Strictly Unmodified)  
**Execution Mode:** Targeted Remediation and Hardening  
**Date:** 2026-11-11  

---

## 1. Executive Summary

This report documents the successful execution and verification of **Sprint 2 Stage 7A Phase 2 Targeted Hardening** for OpenTally.

Following the independent verification and reconciliation documented in [`OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md`](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md), targeted remediation was conducted across the codebase to resolve all six identified findings:
- **`FIND-7A-P2-01` (HIGH)**: Async future-drop cancellation safety and pool hygiene.
- **`FIND-7A-P2-02` (HIGH)**: Missing failure-path, lineage truncation/cycle, and extreme monetary boundary integration tests.
- **`FIND-7A-P2-03` (MEDIUM)**: Completion report documentation inaccuracies and test scenario reconciliation.
- **`FIND-7A-P2-04` (MEDIUM)**: Incomplete sort-field integration coverage.
- **`FIND-7A-P2-05` (LOW)**: Repository-level pagination input validation and arithmetic safety.
- **`FIND-7A-P2-06` (LOW)**: Multi-tenant audit collision, cross-company lineage edge, and WAL concurrent-write snapshot isolation tests.

All automated verification gates pass cleanly:
- **Rust Test Suite**: **316 passed, 0 failed, 0 ignored** (+18 targeted integration tests).
- **Frontend Test Suite**: **27 passed, 0 failed** across 3 test files (zero regressions).
- **Rust Linter (`cargo clippy --all-targets -- -D warnings`)**: **0 warnings, 0 errors**.
- **Rust Formatter (`cargo fmt -- --check`)**: **Clean (0 discrepancies)**.
- **Subagent Reviews**: Independent reviews by `security-reviewer` and `rust-reviewer` both concluded with **APPROVE** verdicts and zero unresolved Critical or High defects.

---

## 2. Preflight State and Test Count Baseline

| Metric | Phase 1 Baseline | Phase 2 Completion | Phase 2 Hardening (Current) | Net Delta |
|:---|:---:|:---:|:---:|:---:|
| **Backend Tests (`cargo test`)** | 273 | 298 | **316** | **+43 total Stage 7 tests** (+18 hardening) |
| **Frontend Tests (`vitest run`)** | 27 | 27 | **27** | 0 (Zero regressions) |
| **Stage 7 Test Functions** | 0 | 25 | **43** | 25 initial + 18 hardening |
| **Clippy Warnings (`-D warnings`)** | 0 | 0 | **0** | Clean |
| **Database Migrations** | 4 | 4 | **4** | Unmodified (`0.3.1-sprint2`) |
| **External Dependencies** | 0 new | 0 new | **0 new** | Unmodified |

---

## 3. Remediation Matrix: Findings Resolution

### 3.1 `FIND-7A-P2-01` (HIGH) — Async Cancellation & Pool Contamination Safety
- **Defect Identified**: If an async caller dropped a future mid-`.await` inside `execute_read_only_query`, `sqlx::PoolConnection::drop` default behavior invoked `return_to_pool()`, which on SQLite is a no-op channel ping. A connection left with `PRAGMA query_only = 1` or an open transaction could leak back into the shared pool.
- **Remediation Implemented**:
  1. **Fail-Closed RAII Guard ([`ReadOnlyConnectionGuard`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L76-L119))**:
     Initialized with `clean_for_reuse = false`. If dropped prematurely by async future cancellation or panic unwind, `ReadOnlyConnectionGuard::drop` invokes `conn.close_on_drop()`. In `sqlx-core-0.8.6`, this instructs the `PoolConnection` drop handler to float and permanently discard the connection from the pool, decrementing the pool size and releasing the semaphore permit.
  2. **Managed Transaction Lifecycle**:
     Replaced raw unmanaged `BEGIN;` with SQLx-managed `guard.conn_mut()?.begin().await`. This ensures `shared.transaction_depth` correctly increments.
  3. **Strict Post-Reset Verification**:
     After transaction commit and `PRAGMA query_only = OFF;`, `execute_read_only_query` queries `PRAGMA query_only;` and verifies that `query_only == 0 && tx_depth == 0`. Only upon passing both checks is `guard.release_clean()` called.
  4. **Two-Layer Global Pool Hygiene ([`DatabaseManager::init`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L77-L108))**:
     Registered an `.after_release(...)` hook on `SqlitePoolOptions`. Before checking transaction depth, it queries `PRAGMA query_only;` on the connection to flush SQLite's FIFO worker command channel. It then verifies `query_only == 0 && tx_depth == 0`, returning `Ok(false)` to reject and close any connection that fails.
- **Status**: **RESOLVED & VERIFIED** via tests `test_h01`, `test_h02`, `test_h03`, `test_h04`, and `test_h05`.

---

### 3.2 `FIND-7A-P2-02` (HIGH) — Missing Failure-Path, Lineage, and Monetary Boundary Tests
- **Defect Identified**: Real SQLite failure paths (unauthorized DML write rejection, rollback failure, commit failure), recursive lineage truncation (`depth = 10`, `depth > 10`), cyclic graphs, and high-precision monetary SQL comparisons ($10^{-4}$ to $10^{25}-1$) were unverified in the repository test suite.
- **Remediation Implemented**:
  1. Added `test_h06`: Proves unauthorized write DML inside read-only query is rejected with `SQLITE_READONLY` and rolled back, and the connection is closed.
  2. Added `test_h07`: Simulates and proves SQLite `ROLLBACK` failure cleanly discards the connection.
  3. Added `test_h08`: Simulates and proves SQLite `COMMIT` failure cleanly discards the connection.
  4. Added `test_h09`: Verifies 4-hop (5-node) lineage traversal within depth limit (`exceeded = false`).
  5. Added `test_h10`: Verifies 10-hop (11-node) lineage traversal at exact depth boundary (`depth = 10`, `exceeded = false`).
  6. Added `test_h11`: Verifies 12-hop (13-node) lineage traversal in both forward and backward directions, confirming truncation to exactly 11 items and `has_exceeded_depth_limit = true`.
  7. Added `test_h12`: Verifies cyclic lineage graph (3-cycle) cleanly terminates without infinite looping, and verifies that prefix voucher ID substrings (`v1`, `v10`, `v100`) do not cause false cycle detections due to delimiter-padded path matching (`'/' || id || '/'`).
  8. Added `test_h13`: Verifies SQLite `CANONICAL_AMOUNT_SQL` exact boundary comparisons across $0.0001$, $1.0000$, 20-digit integers, max schema 25-character decimal `99999999999999999999.9999`, max 25-digit integer `9999999999999999999999999`, and 30-character upper query filter `9999999999999999999999999.9999`.
- **Status**: **RESOLVED & VERIFIED** via tests `test_h06` through `test_h13`.

---

### 3.3 `FIND-7A-P2-03` (MEDIUM) — Completion Report Inaccuracies & Reconciliation
- **Defect Identified**: The Phase 2 completion report overstated failure-path coverage, misstated SQLx 0.8 `close()` mechanics, and omitted disclosure of early pagination inclusion.
- **Remediation Implemented**:
  1. Fully reconciled test counts and test functions (25 original + 18 hardening = 43 Stage 7 test functions; 316 total backend tests).
  2. Documented exact SQLx 0.8.6 `close(mut self)` mechanics: `self.take_live()` empties `self.live` before `raw.close().await`, ensuring that once `close()` is called, the connection cannot return to the pool regardless of error outcome.
  3. Distinguished testable failure paths from uninjectable engine-level failure modes (Modes 1, 2, and 6 cannot be faulted on a healthy SQLite pool without byte-level memory corruption).
  4. Explicitly documented pagination as delivered early in Phase 2 to prevent redundant implementation in Phase 3.
- **Status**: **RESOLVED & RECONCILED** in documentation and reports.

---

### 3.4 `FIND-7A-P2-04` (MEDIUM) — Incomplete Sort-Field Integration Coverage
- **Defect Identified**: Only `TotalAmount` and `Date DESC` were tested in `stage7_repository_tests.rs`; `VoucherNumber`, `CreatedAt`, `Date ASC`, and explicit tie-breaker precedence were unverified.
- **Remediation Implemented**:
  - Implemented `test_h14`: Exercises all 4 `VoucherSortField` variants in both `Ascending` and `Descending` directions, tests draft vouchers with `NULL` total amount sorting to the bottom/top cleanly, tests same-date tie-breaking by sequence number, and verifies final tie-breaker stability on `v.id ASC`.
- **Status**: **RESOLVED & VERIFIED** via test `test_h14`.

---

### 3.5 `FIND-7A-P2-05` (LOW) — Repository-Level Pagination Input Safety
- **Defect Identified**: `SqliteVoucherSearchRepository` relied on caller validation in `VoucherSearchService`; direct repository callers could trigger arithmetic overflow or panic on `page = 0` or `page_size = 0`.
- **Remediation Implemented**:
  1. Exported `pub fn validate_pagination_params(page: Option<u32>, page_size: Option<u32>) -> Result<(u32, u32, i64), OpenTallyError>` with checked `page.checked_sub(1)` and checked `page_index.checked_mul(page_size)` in [`validator.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L682-L710).
  2. Implemented `pub(crate) fn compute_total_pages(total: i64, page_size: u32) -> Result<u32, OpenTallyError>` with `u64::try_from(total)` + ceiling division + `u32::try_from` bounds checking.
  3. Hardened `search_vouchers_impl` to invoke `validate_pagination_params` and `compute_total_pages` directly at the repository boundary.
  4. Hardened `get_voucher_lineage_impl` depth mapping with `u32::try_from(*depth)`.
- **Status**: **RESOLVED & VERIFIED** via test `test_h15`.

---

### 3.6 `FIND-7A-P2-06` (LOW) — Audit Log Subquery Collision and Snapshot Isolation Tests
- **Defect Identified**: Multi-tenant isolation for correlated cancellation reason subqueries and WAL concurrent-write snapshot isolation lacked explicit adversarial tests.
- **Remediation Implemented**:
  1. Added `test_h16`: Creates identical voucher IDs in Company A and Company B, inserts cancellation audit log records with identical timestamps and IDs, and verifies that querying Company A never exposes Company B's cancellation reasons. Also verifies cancelled vouchers without audit records return `None` without error.
  2. Added `test_h17`: Creates a cross-company lineage edge pointing to a foreign company voucher and verifies that lineage traversal terminates strictly at the company boundary.
  3. Added `test_h18`: Spawns a concurrent writer task on a multi-connection pool while `execute_read_only_query` holds an active transaction. Verifies that newly committed rows during read execution do not alter the `COUNT(*)` or `SELECT` results (true WAL snapshot isolation).
- **Status**: **RESOLVED & VERIFIED** via tests `test_h16`, `test_h17`, and `test_h18`.

---

## 4. Hardening Test Suite Mapping

All 18 hardening tests are implemented in [`02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs):

| Test Identifier | Finding Addressed | Verification Objective | Result |
|:---|:---:|:---|:---:|
| `test_h01_async_cancellation_inside_closure_discards_connection_safely` | `FIND-7A-P2-01` | Cancel inside closure discards connection #1, opens fresh writable connection #2 | **PASS** |
| `test_h02_async_cancellation_between_commit_and_pragma_off` | `FIND-7A-P2-01` | Cancel after commit before PRAGMA reset discards connection | **PASS** |
| `test_h03_async_cancellation_between_pragma_on_and_begin` | `FIND-7A-P2-01` | Cancel after PRAGMA ON before BEGIN discards connection | **PASS** |
| `test_h04_panic_inside_closure_discards_connection` | `FIND-7A-P2-01` | Stack unwind panic inside closure discards connection | **PASS** |
| `test_h05_pool_after_release_hook_rejects_dirty_connection` | `FIND-7A-P2-01` | `DatabaseManager` `.after_release` hook rejects dirty connection | **PASS** |
| `test_h06_read_only_mode_rejects_dml_and_discards_connection` | `FIND-7A-P2-02` | DML in read-only mode rejected with `SQLITE_READONLY` & discarded | **PASS** |
| `test_h07_rollback_failure_discards_connection` | `FIND-7A-P2-02` | Rollback failure during error handling discards connection | **PASS** |
| `test_h08_commit_failure_discards_connection` | `FIND-7A-P2-02` | Commit failure discards connection | **PASS** |
| `test_h09_recursive_lineage_depth_below_limit_success` | `FIND-7A-P2-02` | Lineage 4 hops (5 nodes) completes with `exceeded = false` | **PASS** |
| `test_h10_recursive_lineage_exact_depth_ten_success` | `FIND-7A-P2-02` | Lineage 10 hops (11 nodes) completes with `exceeded = false` | **PASS** |
| `test_h11_recursive_lineage_depth_exceeded_truncates_at_ten` | `FIND-7A-P2-02` | Lineage 12 hops truncates at 11 nodes with `has_exceeded_depth_limit = true` | **PASS** |
| `test_h12_recursive_lineage_cyclic_terminates_cleanly` | `FIND-7A-P2-02` | Cyclic graph terminates; prefix ID collisions (`v1`, `v10`, `v100`) immune | **PASS** |
| `test_h13_monetary_sql_boundary_conditions` | `FIND-7A-P2-02` | SQLite 30-char canonical comparisons: $0.0001$ to $10^{25}-1$ and max query strings | **PASS** |
| `test_h14_comprehensive_sort_fields_and_tie_breaking` | `FIND-7A-P2-04` | All 4 sort fields Asc/Desc, NULL amounts, date and sequence tie-breaking | **PASS** |
| `test_h15_repository_level_pagination_bounds_and_arithmetic` | `FIND-7A-P2-05` | Repository rejects `page=0`, `page_size=0, 101`, offset overflow; verified `div_ceil` | **PASS** |
| `test_h16_cross_company_audit_log_collision_isolation` | `FIND-7A-P2-06` | Identical voucher IDs in separate companies never leak cancellation reasons | **PASS** |
| `test_h17_cross_company_lineage_edges_isolated` | `FIND-7A-P2-06` | Foreign company lineage references terminate traversal at company border | **PASS** |
| `test_h18_wal_concurrent_write_snapshot_consistency` | `FIND-7A-P2-06` | Concurrent commit during read query does not leak phantom rows or split count | **PASS** |

---

## 5. Independent Subagent Review Verdicts

### 5.1 Security Review (`security-reviewer`)
- **Reviewer:** Dedicated Security Reviewer Subagent
- **Verdict:** **APPROVE** (0 Critical, 0 High, 0 Medium)
- **Key Findings & Validations**:
  - Confirmed SQL injection prevention: All user-supplied filters, dates, and amounts use parameterized SQLx bindings (`$1, $2, ...`) or safe hardcoded SQLite primitives.
  - Confirmed tenant boundary isolation: Every query and subquery (including `audit_log` reason extraction and recursive CTE lineage traversal) enforces `company_id = $1`.
  - Confirmed fail-closed pool hygiene: `ReadOnlyConnectionGuard` and `DatabaseManager::after_release` prevent connection pool poisoning under async cancellation, panics, or SQLite errors.
  - Confirmed safe arithmetic: Zero unchecked integer casting; offset calculations and page counts use checked operations.

### 5.2 Rust Specialist Review (`rust-reviewer`)
- **Reviewer:** Dedicated Rust Reviewer Subagent
- **Verdict:** **APPROVE** (0 Critical, 0 High)
- **Key Findings & Validations**:
  - Validated SQLx 0.8 cancellation safety and `ReadOnlyConnectionGuard` RAII lifecycle.
  - Validated worker channel drain sequence: Awaiting `PRAGMA query_only;` inside `.after_release` ensures asynchronous `Command::Rollback` is processed before checking transaction depth.
  - Validated clean borrow scopes: Transaction borrow of `guard` terminates before post-commit PRAGMA execution, eliminating lifetime conflicts.
  - Validated delimiter-padded CTE cycle detection (`'/' || id || '/'`) preventing substring collisions.
  - Minor Medium notes for future refactoring (non-blocking): Consider splitting `search.rs` into submodules (`guard.rs`, `query.rs`, `repository.rs`) and extracting row mapping helpers to reduce function length.

---

## 6. Git Working Tree and Boundary Verification

- **Branch:** `feature/phase1`
- **Modified Files:**
  - `02-open-tally/src-tauri/src/database/mod.rs`
  - `02-open-tally/src-tauri/src/vouchers/mod.rs`
  - `02-open-tally/src-tauri/src/vouchers/service/mod.rs`
  - `02-open-tally/src-tauri/src/vouchers/service/search.rs`
  - `02-open-tally/src-tauri/src/vouchers/validator.rs`
- **Untracked Files:**
  - `02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs`
  - `02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs`
  - `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md`
  - `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md`
  - `02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md`
- **Git State:** Clean; zero files staged or committed.

---

## 7. Governance Invariants

All governance decisions and tracked findings remain strictly intact:
- **`DEC-S2-001` (Invoice Rounding Policy)**: `PENDING USER APPROVAL`
- **`DEC-S2-002` (Sequence-Gap Governance)**: `PENDING USER APPROVAL`
- **`DEC-S2-003` (Cancellation Reason Storage)**: `APPROVAL EVIDENCE NOT VERIFIED` (Option A Audit Log JSON verified and utilized)
- **`DEC-S2-004` (Posted Correction Workflow)**: `APPROVAL EVIDENCE NOT VERIFIED`
- **Tracked Findings `FIND-6-01` through `FIND-6-06`**: Remain active for Sprint 2 closing and Stage 8.

---

## 8. Statement of Readiness for Phase 3

With the resolution and verification of all findings `FIND-7A-P2-01` through `FIND-7A-P2-06`, the Phase 2 Read-Only Search Repository is **fully hardened, verified, and complete**.

**Readiness Statement**:
- The backend search engine, connection lifecycle management, tenant company isolation, and recursive lineage traversal are robust and production-ready.
- Pagination is fully implemented and hardened at the repository boundary; Phase 3 does not need to re-implement pagination.
- Stage 7A is now **READY** to proceed to **Phase 3 (Streaming Export Engine & Windows Atomic File Replacement)** upon explicit user authorization.

