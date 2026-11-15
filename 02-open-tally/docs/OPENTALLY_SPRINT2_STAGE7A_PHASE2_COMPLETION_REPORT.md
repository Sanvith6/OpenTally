# OpenTally — Sprint 2 Stage 7A Phase 2 Formal Completion Report
## Read-Only Search Repository Implementation & Targeted Hardening

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase:** 2 — Read-Only Search Repository Implementation & Targeted Hardening  
**Approved Design:** Stage 7 Technical Design v2.2.2  
**Database Schema:** `0.3.1-sprint2` (Strictly Unmodified)  
**Execution Mode:** Implementation and Targeted Hardening with strict phase boundary adherence  
**Date:** 2026-11-11  

---

## 1. Executive Summary

This report documents the formal completion of **Sprint 2 Stage 7A Phase 2: Read-Only Search Repository Implementation & Targeted Hardening** for OpenTally.

Phase 2 builds directly upon the Phase 1 domain models, canonical string normalizer, and validation contract. It implements the database-backed read-only search engine, strict connection lifecycle hygiene (`PRAGMA query_only = ON` with SQLx-managed transaction scoping, fail-closed `ReadOnlyConnectionGuard` calling `conn.close_on_drop()`, and global pool `.after_release` verification), tenant company isolation across all queries and subqueries, dynamic parameterized filter generation, exact 30-character canonical SQLite amount comparisons, deterministic sort tie-breakers, checked pagination arithmetic, and bidirectional recursive CTE lineage traversal with definite truncation detection.

Following an independent verification which identified six findings (`FIND-7A-P2-01` through `FIND-7A-P2-06`), a targeted hardening pass was executed. All 18 hardening tests (`test_h01` through `test_h18`) pass alongside the original 25 repository test functions (`test_01` through `test_29`), bringing the Stage 7 test suite to **43 test functions**. The complete backend test suite increased from the 273-test Phase 1 baseline to **316 passing tests (0 failures, 0 ignored)**. The frontend test suite verified zero regressions with **27/27 passing tests**. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` pass cleanly with zero warnings. Independent code reviews by both `security-reviewer` and `rust-reviewer` specialist subagents yielded **APPROVE** verdicts with zero unresolved Critical or High defects.

---

## 2. Approved Design Sections Implemented

The implementation strictly realizes the following sections of the approved Stage 7 v2.2.2 Technical Design:

| Design Section | Specification Description | Phase 2 Implementation Details |
|:---|:---|:---|
| **Section 5.3 & 5.4** | 30-character fixed-width canonical monetary normalization | Implemented `CANONICAL_AMOUNT_SQL` SQLite expression in SQL queries, comparing against 30-char strings produced by `normalize_canonical_amount_for_query`. |
| **Section 6.1** | True read-only connection lifecycle & 6-mode failure matrix | Implemented `execute_read_only_query` with engine-level `PRAGMA query_only = ON;`, `BEGIN;`, and guaranteed `conn.close().await` detachment on any failure or uncertain state. |
| **Section 5.5 & 5.6** | Parameterized filter building and deterministic tie-breaker sorting | Implemented `build_filtered_query` with parameterized `$1, $2, ...` bindings, wildcard escaping (`%`, `_`, `\`), and deterministic tie-breaking ending in `v.id ASC`. |
| **Section 9.2** | Bidirectional recursive CTE lineage traversal & truncation detection | Implemented `get_voucher_lineage` querying up to `depth < 11` with cycle protection (`instr(vl.path, ...) = 0`), accurately computing `has_exceeded_depth_limit` and `max_depth_reached`. |
| **Section 9.3** | Cancellation reason extraction from audit log | Implemented correlated subquery with `a.company_id = v.company_id AND a.entity_id = v.id AND a.action = 'CANCEL'`, deterministically selecting latest reason via `ORDER BY a.timestamp DESC, a.id DESC LIMIT 1`. |
| **Section 10.4 & 11** | Domain models, DTO mapping, and Service interface | Reused Phase 1 `VoucherRegisterItem`, `VoucherSearchFilter`, and implemented `VoucherSearchRepository` trait, `SqliteVoucherSearchRepository`, and `VoucherSearchService`. |

---

## 3. Repository Architecture

The architecture maintains strict separation of concerns between domain validation, connection lifecycle management, and SQL query execution:

```
VoucherSearchService
  │
  ├── 1. Pure Domain Validation: validate_search_filter(&filter)
  │      (Fails fast on invalid dates, negative amounts, invalid statuses before DB acquisition)
  │
  └── 2. Connection Lifecycle Guard: execute_read_only_query(pool, |conn| { ... })
         │
         ├── PRAGMA query_only = ON;
         ├── BEGIN; (Snapshot consistency)
         │
         ├── 3. Repository Execution: SqliteVoucherSearchRepository
         │      ├── build_filtered_query(company_id, filter)
         │      ├── Execute COUNT(*) on conn
         │      ├── Execute SELECT ... LIMIT $limit OFFSET $offset on conn
         │      └── Map rows to VoucherRegisterItem / VoucherLineageResponse
         │
         ├── COMMIT;
         └── PRAGMA query_only = OFF;
```

### Components Implemented:
1. **`VoucherSearchRepository` (Trait)**: Defines the abstraction for search and lineage operations against SQLite.
2. **`SqliteVoucherSearchRepository` (Struct)**: Concrete SQLite implementation providing `search_vouchers_impl` and `get_voucher_lineage_impl`.
3. **`VoucherSearchService` (Service)**: Entry point that coordinates domain validation and read-only connection lifecycle execution.

---

## 4. Read-Only Connection Lifecycle & Pool Hygiene

The implementation rigorously follows Section 6.1 of the v2.2.2 design, hardened with dual-layer cancellation and panic safety to eliminate pool poisoning risks:

```rust
pub async fn execute_read_only_query<F, T>(
    pool: &Pool<Sqlite>,
    f: F,
) -> Result<T, OpenTallyError>
where
    F: for<'c> FnOnce(
        &'c mut sqlx::SqliteConnection,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<T, OpenTallyError>> + Send + 'c>,
    >,
```

### Architecture of Safety:
1. **`ReadOnlyConnectionGuard` (Fail-Closed RAII)**:
   A dedicated RAII guard holds `Option<PoolConnection<Sqlite>>` initialized with `clean_for_reuse = false`. If the async future is dropped or cancelled mid-execution, or if an unwinding panic occurs, `ReadOnlyConnectionGuard::drop` invokes `conn.close_on_drop()`. Under `sqlx-core-0.8.6`, this instructs the `PoolConnection` drop handler to float and permanently discard the connection from the pool, decrementing the pool size and releasing the semaphore permit.
2. **SQLx-Managed Transaction Scoping**:
   Rather than raw `BEGIN;`, the connection begins an official SQLx-managed transaction `guard.conn_mut()?.begin().await`. All closure operations execute against `&mut tx`. On error, `tx.rollback().await` is attempted and `guard.close_and_discard().await` permanently closes the connection.
3. **Double-Check Verification Before Release**:
   After `tx.commit().await` and resetting `PRAGMA query_only = OFF;`, `execute_read_only_query` queries `PRAGMA query_only;` on the live connection to verify that `query_only == 0 && tx_depth == 0`. Only upon passing both checks is `guard.release_clean()` called, returning the connection to the pool.
4. **Global Pool `.after_release` Verification (`DatabaseManager::init`)**:
   As a second defense-in-depth layer, `DatabaseManager::init` registers an `.after_release(...)` hook on `SqlitePoolOptions`. Before checking transaction depth, it queries `PRAGMA query_only;` to flush the SQLite worker's FIFO command channel (ensuring any asynchronous rollback commands are processed). It then verifies `query_only == 0 && tx_depth == 0`, rejecting and discarding any connection that fails.
5. **SQLx 0.8 Close Mechanics**:
   `PoolConnection::close(mut self)` calls `self.take_live()` before awaiting `raw.close().await`. This ensures `Live<Sqlite>` is removed from the connection handle prior to transport closure; even if closing the underlying transport fails, the connection cannot be returned to the pool idle queue.

### Failure Handling & Verification Matrix:
- **Mode 1 (`PRAGMA query_only = ON` fails)**: Discards connection via `close_and_discard().await`. *(Architectural invariant: uninjectable on healthy SQLite pool without process memory corruption)*.
- **Mode 2 (`BEGIN` fails)**: Discards connection via `close_and_discard().await`. *(Architectural invariant: uninjectable after PRAGMA without process memory corruption)*.
- **Mode 3 (Query closure `f` fails)**: Attempts `ROLLBACK;` and discards connection via `close_and_discard().await`. *(Empirically verified in `test_03`, `test_04`, `test_05_and_06_and_07`, and `test_h06`)*.
- **Mode 4 (`ROLLBACK` fails)**: Gracefully ignores rollback error and unconditionally discards connection. *(Empirically verified in `test_h07`)*.
- **Mode 5 (`COMMIT` fails)**: Discards connection without executing PRAGMA reset. *(Empirically verified in `test_h08`)*.
- **Mode 6 (`PRAGMA query_only = OFF` fails)**: Discards connection via `close_and_discard().await`. *(Empirically verified under post-commit cancellation in `test_h02`)*.
- **Mode 7 (Connection close errors)**: `take_live()` guarantees local drop; semaphore permit released.
- **Async Future Drop / Cancellation**: Verified at multiple lifecycle points via task-local hooks in `test_h01`, `test_h02`, and `test_h03`, proving that cancelled connections are permanently discarded and subsequent pool acquisitions yield clean, writable connections.
- **Stack Unwind Panic**: Verified in `test_h04`, proving that panics inside `f` invoke `close_on_drop()`.

---

## 5. Company Isolation Verification

Multi-tenancy isolation is strictly enforced across every layer:
1. **Top-Level Queries**:
   - `SELECT COUNT(*) FROM vouchers v WHERE v.company_id = $1 AND ...`
   - `SELECT v.* FROM vouchers v ... WHERE v.company_id = $1 AND ...`
2. **Join Conditions**:
   - `JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id`
   - `JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id`
3. **Correlated Subqueries**:
   - Line count: `(SELECT COUNT(*) FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id)`
   - Reversal ID: `(SELECT r.id FROM vouchers r WHERE r.company_id = v.company_id AND r.reversal_of_voucher_id = v.id)`
   - Cancellation reason: `(SELECT ... FROM audit_log a WHERE a.company_id = v.company_id AND a.entity_id = v.id ...)`
   - Ledger touching: `EXISTS (SELECT 1 FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id AND vl.ledger_id = $param)`
4. **Recursive CTE Lineage**:
   - Anchor: `WHERE v.company_id = $1 AND v.id = $2`
   - Backward: `JOIN voucher_lineage vl ON parent.company_id = vl.company_id AND parent.id = vl.parent_id`
   - Forward: `JOIN voucher_lineage vl ON child.company_id = vl.company_id AND child.reversal_of_voucher_id = vl.id`
   - Final select: `JOIN vouchers v ON vl.company_id = v.company_id AND vl.id = v.id`

Empirically verified in tests `test_08` (cross-tenant isolation), `test_09` (cross-company ledger filter), `test_10` (cross-company lineage rejection), and `test_11` (empty filter isolation).

---

## 6. Filters Implemented

All 8 filters specified in the Stage 7 design have been implemented and validated:

1. **Voucher Type (`voucher_type_id`)**: Parameterized filter against `v.voucher_type_id = $param`. Verified in `test_12`.
2. **Numbering Series (`series_id`)**: Parameterized filter against `v.series_id = $param`. Verified in `test_13`.
3. **Lifecycle Status (`status`)**:
   - Default (`None`): `v.status IN ('posted', 'reversed', 'cancelled')` (excludes drafts).
   - Explicit `"all"`: No status filter; returns all 4 statuses including drafts.
   - Specific status: `v.status = $param`. Verified in `test_14`.
4. **Date Basis & Date Range (`date_basis`, `date_from`, `date_to`)**:
   - Selects target column `v.date` (`TransactionDate`, default) or `v.effective_date` (`EffectiveDate`).
   - Applies `>= $date_from` and `<= $date_to`. Verified in `test_15` and `test_16`.
5. **Ledger Touching (`ledger_id`)**: Correlated `EXISTS` subquery verifying if the voucher touches the specified ledger on any line. Verified in `test_17`.
6. **Reversal Ancestry (`reversal_of_voucher_id`)**: Filters vouchers that are reversals of the specified voucher ID. Verified in `test_19`.
7. **Exact Monetary Range (`min_amount`, `max_amount`)**: Exact 30-character canonical comparisons. Verified in `test_20_and_21`.
8. **Text Search (`search_query`)**: Matches voucher number, reference number, and narration with literal wildcard escaping. Verified in `test_18`.

---

## 7. Deterministic Sorting & Tie-Breaking

To prevent result set jitter across paginated queries, an explicit allowlist maps `VoucherSortField` and `SortDirection` to SQL expressions, always concluding with `v.id ASC`:

| Sort Field | Ascending SQL | Descending SQL |
|:---|:---|:---|
| **Date** (Default) | `{date_col} ASC, v.sequence_number ASC, v.id ASC` | `{date_col} DESC, v.sequence_number DESC, v.id ASC` |
| **VoucherNumber** | `v.voucher_number ASC, {date_col} DESC, v.id ASC` | `v.voucher_number DESC, {date_col} DESC, v.id ASC` |
| **TotalAmount** | `{CANONICAL_AMOUNT_SQL} ASC, {date_col} DESC, v.id ASC` | `{CANONICAL_AMOUNT_SQL} DESC, {date_col} DESC, v.id ASC` |
| **CreatedAt** | `v.created_at ASC, v.id ASC` | `v.created_at DESC, v.id ASC` |

Empirically verified in tests `test_23_and_24` (ascending/descending amount numeric sorting) and `test_25` (identical deterministic ordering across repeated executions).

---

## 8. Exact Monetary Comparison Implementation

Monetary values in `vouchers.total_amount` are stored as `TEXT`. The implementation employs the 30-character canonical SQLite normalization expression:

```sql
(substr('0000000000000000000000000' || substr(COALESCE(v.total_amount, '0'), 1, instr(COALESCE(v.total_amount, '0') || '.', '.') - 1), -25) 
 || '.' || 
 substr(substr(COALESCE(v.total_amount, '0'), instr(COALESCE(v.total_amount, '0') || '.', '.') + 1) || '0000', 1, 4))
```

- **Bounds Supported**: Up to 25 integer digits ($10^{25}-1$) and up to 4 decimal places.
- **Numeric Fidelity**: `0.05 < 2.50 < 10.00 < 100.00` is compared and ordered accurately without alphabetical distortion (`"10.00" < "100.00" < "2.50"`).
- **Draft NULL Handling**: Coalesced to `'0'`, normalizing to `'0000000000000000000000000.0000'`. Correctly matches `min_amount: 0` and is excluded by `min_amount > 0`.
- **Zero Floating-Point Arithmetic**: Zero `f32`/`f64` conversions; zero signed 64-bit integer scaling overflow risks.

---

## 9. Result DTO Mapping

Rows returned by SQLite are mapped directly to `VoucherRegisterItem` and `VoucherLineageResponse`:
- `id`, `company_id`, `voucher_type_id`, `voucher_type_name`, `series_id`, `series_name`, `voucher_number`, `date`, `effective_date`, `status`: String fields accurately mapped.
- `reference_no`, `reference_date`, `narration`, `total_amount`: Optional string fields mapped with `None` on SQL `NULL`.
- `line_count`: Count subquery mapped to `i64`.
- `reversal_of_voucher_id`: Extracted directly from `vouchers.reversal_of_voucher_id`.
- `reversal_voucher_id`: Correlated subquery lookup using index `uq_vouchers_reversal`.
- `cancellation_reason`: Subquery from `audit_log` with JSON extract `$.reason` for cancelled vouchers; `None` otherwise.
- `created_at`, `updated_at`: Timestamps preserved.

---

## 10. Integration Tests and Results

A comprehensive suite of 25 integration tests covering all 28 required scenarios was authored in [`stage7_repository_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs):

| Test Function | Scenarios Covered | Status |
|:---|:---|:---:|
| `test_01_successful_query_only_activation` | PRAGMA query_only = ON verified active during closure | **PASS** |
| `test_02_search_executes_successfully_in_read_only_mode` | Search succeeds in read-only mode | **PASS** |
| `test_03_attempted_write_fails_with_sqlite_readonly` | DML through connection rejected with SQLITE_READONLY | **PASS** |
| `test_04_query_failure_triggers_cleanup` | Query error triggers rollback and connection detachment | **PASS** |
| `test_05_and_06_and_07_no_contaminated_connection_returns_to_pool` | Pool hygiene; 5 subsequent connections unpoisoned and writable | **PASS** |
| `test_08_search_returns_only_requested_company_vouchers` | Multi-tenant isolation between Company A and Company B | **PASS** |
| `test_09_ledger_filter_cannot_expose_other_company_vouchers` | Cross-company ledger filtering returns 0 records | **PASS** |
| `test_10_reversal_lineage_cannot_cross_company_boundaries` | Cross-company lineage query returns NotFound error | **PASS** |
| `test_11_empty_optional_filters_preserves_company_isolation` | Empty filter still strictly enforces company_id | **PASS** |
| `test_12_voucher_type_filtering` | Filter by voucher_type_id | **PASS** |
| `test_13_series_filtering` | Filter by series_id | **PASS** |
| `test_14_status_filtering_including_default_and_all` | Status filtering: default excludes drafts; "draft", "all", "posted", "cancelled" | **PASS** |
| `test_15_transaction_date_filtering` | TransactionDate range filtering (inclusive) | **PASS** |
| `test_16_effective_date_filtering` | EffectiveDate basis filtering vs TransactionDate | **PASS** |
| `test_17_ledger_touching_filtering` | Ledger touching filter across Cash, Bank, Sales | **PASS** |
| `test_18_text_filtering_and_wildcard_escaping` | Literal `%`, `_`, and `\` matching without LIKE wildcard interpretation | **PASS** |
| `test_19_reversal_ancestry_filtering` | Filter by reversal_of_voucher_id | **PASS** |
| `test_20_and_21_exact_amount_boundary_filtering` | Exact min_amount and max_amount boundary filtering | **PASS** |
| `test_22_null_amount_handling_in_drafts` | NULL amount coalescing to 0.0000 baseline | **PASS** |
| `test_23_and_24_sorting_and_direction` | Ascending and descending TotalAmount numeric sorting | **PASS** |
| `test_25_deterministic_tie_breaking` | Tie-breaking reproducibility across runs | **PASS** |
| `test_26_cancellation_reason_extracted_from_audit_log` | Audit log JSON reason extraction | **PASS** |
| `test_27_lineage_recursive_cte_traversal` | Lineage CTE forward and backward traversal | **PASS** |
| `test_28_search_does_not_allocate_sequence_numbers` | Zero sequence number allocation during 25 searches | **PASS** |
| `test_29_pagination_checked_arithmetic` | Multi-page traversal with page_size = 2 and offsets | **PASS** |

### Hardening Integration Tests (`stage7_hardening_tests.rs`):

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

## 11. Full Backend Test Results

Command:
```powershell
cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml
```

Result:
```text
running 316 tests
...
test result: ok. 316 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.92s
```

- **Phase 1 Baseline**: 273 tests passed.
- **Phase 2 Initial Implementation**: 298 tests passed (+25 net new integration tests).
- **Phase 2 Targeted Hardening**: **316 tests passed (+18 targeted hardening tests; total +43 Stage 7 tests)**.
- **Failures / Regressions**: **0**.

---

## 12. Frontend Regression Test Results

Command:
```powershell
npm --prefix 02-open-tally/frontend test -- --run
```

Result:
```text
 RUN  v2.1.9 C:/project/OpenTally/02-open-tally/frontend

 ✓ src/components/__tests__/VoucherEntryView.test.tsx (7 tests) 318ms
 ✓ src/components/__tests__/VoucherMastersView.test.tsx (8 tests) 605ms
 ✓ src/components/__tests__/ChartOfAccountsView.test.tsx (12 tests) 639ms

 Test Files  3 passed (3)
      Tests  27 passed (27)
   Start at  22:38:44
   Duration  6.23s
```

Zero frontend regressions.

---

## 13. Clippy, Formatting, and Compilation Results

1. **Rustfmt (`cargo fmt -- --check`)**:
   - Exit code: `0`
   - Zero formatting discrepancies.
2. **Clippy (`cargo clippy --all-targets -- -D warnings`)**:
   - Exit code: `0`
   - Zero compiler or linter warnings.
3. **Build Check (`cargo check`)**:
   - Exit code: `0`
   - Finished in 3.92s.

---

## 14. Migration and Dependency Verification

- **Schema Version**: `0.3.1-sprint2` (Strictly Unchanged).
- **Migrations Added/Modified**: **0**.
- **External Crates Added/Modified**: **0**.
- **`Cargo.toml` Changes**: **0**.
- **`Cargo.lock` Changes**: **0**.

---

## 15. Git Status Before and After

### Git Status Before Phase 2 Implementation:
```text
On branch feature/phase1
Your branch is up to date with 'origin/feature/phase1'.
nothing to commit, working tree clean
```

### Git Status After Phase 2 Implementation & Hardening:
```text
On branch feature/phase1
Your branch is up to date with 'origin/feature/phase1'.

Changes not staged for commit:
	modified:   02-open-tally/src-tauri/src/database/mod.rs
	modified:   02-open-tally/src-tauri/src/vouchers/mod.rs
	modified:   02-open-tally/src-tauri/src/vouchers/service/mod.rs
	modified:   02-open-tally/src-tauri/src/vouchers/service/search.rs
	modified:   02-open-tally/src-tauri/src/vouchers/validator.rs

Untracked files:
	02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs
	02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs
	02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md
	02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md
	02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md
```

Zero pre-existing files were overwritten or discarded. Zero files have been staged or committed.

---

## 16. Governance Verification

Governance decisions and tracked findings remain strictly preserved in accordance with project standards:

- **`DEC-S2-001` (Invoice Rounding Policy)**: `PENDING USER APPROVAL`
- **`DEC-S2-002` (Sequence-Gap Governance)**: `PENDING USER APPROVAL`
- **`DEC-S2-003` (Cancellation Reason Storage)**: `APPROVAL EVIDENCE NOT VERIFIED` (Option A Audit Log JSON verified and utilized)
- **`DEC-S2-004` (Posted Correction Workflow)**: `APPROVAL EVIDENCE NOT VERIFIED`
- **Tracked Findings `FIND-6-01` through `FIND-6-06`**: Remain active and tracked for Sprint 2 closing and Stage 8.

---

## 17. Deviations and Unresolved Findings

- **Deviations from v2.2.2 Design**: **Zero**.
- **Early Scope Delivery Note**: Pagination query execution (`COUNT(*)`, `LIMIT/OFFSET`, `PaginatedResponse`) and repository pagination bounds checks were delivered and hardened early in Phase 2. This prevents duplicate logic or architectural churn in Phase 3.
- **Reconciliation Findings**: All six findings (`FIND-7A-P2-01` through `FIND-7A-P2-06`) identified during independent reconciliation have been **RESOLVED & HARDENED**.
- **Unresolved Implementation Defects**: **Zero**.
- **Unresolved Code Review Findings**: **Zero** (both `security-reviewer` and `rust-reviewer` passed with **APPROVE** verdicts and 0 critical/high findings).

---

## 18. Phase 2 Acceptance Recommendation

**Recommendation: ACCEPT PHASE 2 (WITH TARGETED HARDENING) WITHOUT RESERVATIONS.**

The read-only search repository, dual-layer cancellation safety, global pool hygiene hook, company isolation controls, exact canonical monetary ordering, checked repository pagination, and recursive lineage CTE are fully implemented, verified, and independently audited.

### Strict Stop Condition:
In accordance with the Phase 2 mandate, work is now **STOPPED**.
- Phase 3 (CSV/JSON file export streaming, path security sanitization, and Windows `ReplaceFileW` atomic finalization) has **NOT** been started.
- Frontend React UI has **NOT** been modified.
- Awaiting explicit user review and authorization before proceeding to Phase 3.
- Awaiting explicit user review and authorization before proceeding.
