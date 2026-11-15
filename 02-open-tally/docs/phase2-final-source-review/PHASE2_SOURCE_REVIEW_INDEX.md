# OpenTally — Phase 2 Source Review Index & Technical Review Package

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase:** 2 — Read-Only Search Repository Implementation & Targeted Hardening  
**Target Milestone Design:** [OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md) (v2.2.2)  
**Database Schema:** `0.3.1-sprint2` (Strictly Unmodified)  
**Package Location:** `02-open-tally/docs/phase2-final-source-review/`  
**Date:** 2026-11-12  
**Audit Mode:** Read-Only Source Exposure & Verification  

---

## 1. Source Review Directory & Components

All source files relevant to Stage 7A Phase 2 have been copied byte-for-byte into the dedicated review directory:
`02-open-tally/docs/phase2-final-source-review/`

| Component | Review File Location | Original Source Path | Important Functions / Items | Line Range in Source |
|:---|:---|:---|:---|:---:|
| **Connection Guard** | `vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/service/search.rs` | `ReadOnlyConnectionGuard`, `conn_mut`, `release_clean`, `close_and_discard`, `Drop` | Lines 76–119 |
| **Read-Only Execution** | `vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/service/search.rs` | `execute_read_only_query`, `TxOutcome`, managed `tx.begin()`, commit/rollback, post-reset verification | Lines 132–244 |
| **Pool Hygiene** | `database/mod.rs` | `02-open-tally/src-tauri/src/database/mod.rs` | `DatabaseManager::init`, `after_release` hook, `PRAGMA query_only;` FIFO drain, `get_transaction_depth` | Lines 77–108 |
| **Search Repository** | `vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/service/search.rs` | `build_filtered_query`, `CANONICAL_AMOUNT_SQL`, `search_vouchers_impl`, `VoucherSearchRepository` trait | Lines 252–405, 471–589 |
| **Lineage CTE** | `vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/service/search.rs` | `get_voucher_lineage_impl`, recursive CTE, cycle prevention, truncation detection | Lines 591–789 |
| **Pagination Safety** | `vouchers/validator.rs`<br>`vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/validator.rs`<br>`02-open-tally/src-tauri/src/vouchers/service/search.rs` | `validate_pagination_params`<br>`compute_total_pages` | Lines 682–710 (`validator.rs`)<br>Lines 446–468 (`search.rs`) |
| **Cancellation Tests** | `vouchers/stage7_hardening_tests.rs` | `02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs` | `test_h01`–`test_h05` (cancellation/panic/pool hook), `test_h06`–`test_h08` (failure paths) | Lines 314–624 |
| **Domain Tests** | `vouchers/stage7_hardening_tests.rs` | `02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs` | `test_h09`–`test_h13` (lineage/money), `test_h14` (sort), `test_h15` (paging), `test_h16`–`test_h18` (isolation/WAL) | Lines 655–1506 |
| **Repository Tests** | `vouchers/stage7_repository_tests.rs` | `02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs` | `test_01` through `test_29` (all 25 repository test functions covering 28 scenarios) | Lines 1–1764 |

---

## 2. Git Diffs for Phase 2 Changes

The individual diffs against the Git base branch (`origin/feature/phase1`) are stored in `02-open-tally/docs/phase2-final-source-review/diffs/`:

### 2.1 `database/mod.rs` ([`diffs/database_mod.diff`](file:///c:/project/OpenTally/02-open-tally/docs/phase2-final-source-review/diffs/database_mod.diff))
```diff
diff --git a/02-open-tally/src-tauri/src/database/mod.rs b/02-open-tally/src-tauri/src/database/mod.rs
index c59f69b..c51dfa3 100644
--- a/02-open-tally/src-tauri/src/database/mod.rs
+++ b/02-open-tally/src-tauri/src/database/mod.rs
@@ -7,7 +7,7 @@ mod stage4_hardening_migration_tests;
 use serde::{Deserialize, Serialize};
 use sqlx::{
     sqlite::{SqliteConnectOptions, SqlitePoolOptions},
-    Pool, Row, Sqlite,
+    Pool, Row, Sqlite, TransactionManager,
 };
 use std::path::Path;
 use std::str::FromStr;
@@ -74,6 +74,38 @@ impl DatabaseManager {
             .max_connections(max_connections)
             .min_connections(1)
             .acquire_timeout(std::time::Duration::from_secs(5))
+            .after_release(|conn, _meta| {
+                Box::pin(async move {
+                    // Execute PRAGMA query_only first so the FIFO SQLite worker drains any
+                    // pending async rollback queued by a dropped Transaction before checking depth.
+                    let query_only = match sqlx::query_scalar::<_, i64>("PRAGMA query_only;")
+                        .fetch_one(&mut *conn)
+                        .await
+                    {
+                        Ok(flag) => flag,
+                        Err(e) => {
+                            warn!(
+                                "Discarding pooled SQLite connection: PRAGMA query_only check failed: {}",
+                                e
+                            );
+                            return Ok(false);
+                        }
+                    };
+
+                    let tx_depth =
+                        <Sqlite as sqlx::Database>::TransactionManager::get_transaction_depth(conn);
+
+                    if query_only != 0 || tx_depth != 0 {
+                        warn!(
+                            "Discarding contaminated SQLite connection on pool release (query_only={}, tx_depth={})",
+                            query_only, tx_depth
+                        );
+                        return Ok(false);
+                    }
+
+                    Ok(true)
+                })
+            })
             .connect_with(options)
             .await?;
```

### 2.2 `vouchers/mod.rs` ([`diffs/vouchers_mod.diff`](file:///c:/project/OpenTally/02-open-tally/docs/phase2-final-source-review/diffs/vouchers_mod.diff))
```diff
diff --git a/02-open-tally/src-tauri/src/vouchers/mod.rs b/02-open-tally/src-tauri/src/vouchers/mod.rs
index 613286b..f1c503c 100644
--- a/02-open-tally/src-tauri/src/vouchers/mod.rs
+++ b/02-open-tally/src-tauri/src/vouchers/mod.rs
@@ -21,14 +21,19 @@ pub mod stage6_tests;
 #[cfg(test)]
 pub mod stage7_domain_tests;
 #[cfg(test)]
+pub mod stage7_hardening_tests;
+#[cfg(test)]
+pub mod stage7_repository_tests;
+#[cfg(test)]
 pub mod tests;
 
 pub use formatter::CanonicalVoucherFormatter;
 pub use model::*;
 pub use service::{
-    derive_period_start, validate_period_consistency, DoubleEntryValidator,
-    VoucherCancellationService, VoucherCorrectionService, VoucherDraftService,
-    VoucherPostingService, VoucherReversalService, VoucherSequenceService, VoucherSeriesService,
-    VoucherTypeService,
+    derive_period_start, execute_read_only_query, validate_period_consistency,
+    DoubleEntryValidator, SqliteVoucherSearchRepository, VoucherCancellationService,
+    VoucherCorrectionService, VoucherDraftService, VoucherPostingService, VoucherReversalService,
+    VoucherSearchRepository, VoucherSearchService, VoucherSequenceService, VoucherSeriesService,
+    VoucherTypeService, CANONICAL_AMOUNT_SQL,
 };
 pub use validator::*;
```

### 2.3 `vouchers/service/mod.rs` ([`diffs/vouchers_service_mod.diff`](file:///c:/project/OpenTally/02-open-tally/docs/phase2-final-source-review/diffs/vouchers_service_mod.diff))
```diff
diff --git a/02-open-tally/src-tauri/src/vouchers/service/mod.rs b/02-open-tally/src-tauri/src/vouchers/service/mod.rs
index 1b70e84..f6f1dda 100644
--- a/02-open-tally/src-tauri/src/vouchers/service/mod.rs
+++ b/02-open-tally/src-tauri/src/vouchers/service/mod.rs
@@ -5,6 +5,7 @@ pub mod draft;
 pub mod period;
 pub mod posting;
 pub mod reversal;
+pub mod search;
 pub mod sequence;
 pub mod series;
 pub mod voucher_type;
@@ -16,6 +17,10 @@ pub use draft::VoucherDraftService;
 pub use period::{derive_period_start, validate_period_consistency};
 pub use posting::VoucherPostingService;
 pub use reversal::VoucherReversalService;
+pub use search::{
+    execute_read_only_query, SqliteVoucherSearchRepository, VoucherSearchRepository,
+    VoucherSearchService, CANONICAL_AMOUNT_SQL,
+};
 pub use sequence::VoucherSequenceService;
 pub use series::VoucherSeriesService;
 pub use voucher_type::VoucherTypeService;
```

### 2.4 `vouchers/validator.rs` ([`diffs/vouchers_validator.diff`](file:///c:/project/OpenTally/02-open-tally/docs/phase2-final-source-review/diffs/vouchers_validator.diff))
```diff
diff --git a/02-open-tally/src-tauri/src/vouchers/validator.rs b/02-open-tally/src-tauri/src/vouchers/validator.rs
index 71a2518..de277e2 100644
--- a/02-open-tally/src-tauri/src/vouchers/validator.rs
+++ b/02-open-tally/src-tauri/src/vouchers/validator.rs
@@ -629,33 +629,7 @@ pub fn validate_search_filter(
     }
 
     // 4. Pagination validation & checked arithmetic
-    if let Some(page) = filter.page {
-        if page == 0 {
-            return Err(OpenTallyError::Validation(
-                "Page number must be greater than or equal to 1".to_string(),
-            ));
-        }
-    }
-    if let Some(page_size) = filter.page_size {
-        if page_size == 0 {
-            return Err(OpenTallyError::Validation(
-                "Page size must be greater than or equal to 1".to_string(),
-            ));
-        }
-        if page_size > 100 {
-            return Err(OpenTallyError::Validation(format!(
-                "Page size {} exceeds maximum permitted limit of 100",
-                page_size
-            )));
-        }
-    }
-    let page = filter.page.unwrap_or(1);
-    let page_size = filter.page_size.unwrap_or(20);
-    if (page - 1).checked_mul(page_size).is_none() {
-        return Err(OpenTallyError::Validation(
-            "Pagination offset calculation overflows u32 range".to_string(),
-        ));
-    }
+    let _ = validate_pagination_params(filter.page, filter.page_size)?;
 
     // 5. Identifier validation
     if let Some(ref id) = filter.voucher_type_id {
@@ -698,3 +672,39 @@ pub fn validate_search_filter(
 
     Ok(())
 }
+
+/// Validate pagination arguments using checked arithmetic and return `(page, page_size, offset)`.
+///
+/// Enforces:
+/// - `page >= 1` (default `1`)
+/// - `1 <= page_size <= 100` (default `20`)
+/// - `(page - 1) * page_size` fits within `u32` (safely widening to `i64` for SQLite `OFFSET`)
+pub fn validate_pagination_params(
+    page: Option<u32>,
+    page_size: Option<u32>,
+) -> Result<(u32, u32, i64), OpenTallyError> {
+    let page = page.unwrap_or(1);
+    let page_size = page_size.unwrap_or(20);
+
+    let page_index = page.checked_sub(1).ok_or_else(|| {
+        OpenTallyError::Validation("Page number must be greater than or equal to 1".to_string())
+    })?;
+
+    if page_size == 0 {
+        return Err(OpenTallyError::Validation(
+            "Page size must be greater than or equal to 1".to_string(),
+        ));
+    }
+    if page_size > 100 {
+        return Err(OpenTallyError::Validation(format!(
+            "Page size {} exceeds maximum permitted limit of 100",
+            page_size
+        )));
+    }
+
+    let offset_u32 = page_index.checked_mul(page_size).ok_or_else(|| {
+        OpenTallyError::Validation("Pagination offset calculation overflows u32 range".to_string())
+    })?;
+
+    Ok((page, page_size, i64::from(offset_u32)))
+}
```

---

## 3. Explicit Analysis of Remaining Verification Points

### A. PRAGMA Reset Failure
**Exact Code Path ([`search.rs:208-215`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L208-L215)):**
```rust
    // Clean reset to query_only = OFF before returning connection to pool (Mode 6 guard)
    if let Err(e) = sqlx::query("PRAGMA query_only = OFF;")
        .execute(&mut **guard.conn_mut()?)
        .await
    {
        guard.close_and_discard().await;
        return Err(OpenTallyError::Database(e));
    }
```
**Handling & Testability Analysis:**
1. **How failure is handled:** If `PRAGMA query_only = OFF;` returns an error, `guard.close_and_discard().await` is invoked. Inside `close_and_discard()`, `conn.close_on_drop()` is marked and `conn.close().await` is called. The guard is dropped without calling `release_clean()`. When `conn` drops, SQLx floats and discards it permanently, guaranteeing the connection is never returned to the pool idle queue.
2. **Can it be injected in a Rust integration test without production-only test behavior?**
   **NO.** In SQLite, `PRAGMA query_only = OFF;` is a standard session flag update. On an uncorrupted, live SQLite connection, executing this valid PRAGMA statement is deterministic and will not fail. To cause it to fail, one would need either:
   - A mock driver layer (not present in SQLx without wrapping all connections in traits), or
   - An artificial test hook embedded directly into production code at line 208 to simulate a syntax error or forced failure, or
   - Low-level OS/memory corruption (e.g. killing the SQLite worker thread externally).
   Embedding artificial error-injection hooks specifically to break standard PRAGMAs would constitute **production-only test behavior** and pollute the production engine.
3. **Existing Verified Coverage:** The test suite verifies the cancellation path right before reset via `test_h02_cancel_after_commit_before_pragma_reset_discards_connection`, proving that if execution aborts before reset completion, the connection is discarded cleanly.

---

### B. Pool Release Hook Failure
**Exact Code Path in `DatabaseManager::after_release` ([`database/mod.rs:77-108`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L77-L108)):**
```rust
            .after_release(|conn, _meta| {
                Box::pin(async move {
                    let query_only = match sqlx::query_scalar::<_, i64>("PRAGMA query_only;")
                        .fetch_one(&mut *conn)
                        .await
                    {
                        Ok(flag) => flag,
                        Err(e) => {
                            warn!(
                                "Discarding pooled SQLite connection: PRAGMA query_only check failed: {}",
                                e
                            );
                            return Ok(false);
                        }
                    };

                    let tx_depth =
                        <Sqlite as sqlx::Database>::TransactionManager::get_transaction_depth(conn);

                    if query_only != 0 || tx_depth != 0 {
                        warn!(
                            "Discarding contaminated SQLite connection on pool release (query_only={}, tx_depth={})",
                            query_only, tx_depth
                        );
                        return Ok(false);
                    }

                    Ok(true)
                })
            })
```
**Upstream SQLx Handling ([`sqlx-core-0.8.6/src/pool/connection.rs:289-305`](file:///C:/Users/sanvi/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sqlx-core-0.8.6/src/pool/connection.rs#L289-L305)):**
```rust
        if let Some(test) = &self.guard.pool.options.after_release {
            let meta = self.metadata();
            match (test)(&mut self.inner.raw, meta).await {
                Ok(true) => (),
                Ok(false) => {
                    self.close().await;
                    return false;
                }
                Err(error) => {
                    tracing::warn!(%error, "error from `after_release`");
                    self.close_hard().await;
                    return false;
                }
            }
        }
```
**Exercise Classification (Honest Distinction):**
1. **Dirty-Connection Rejection (`Ok(false)` when `query_only != 0` or `tx_depth != 0`):**
   - **IS TESTED** in `test_h05_database_manager_after_release_rejects_dirtied_connection`. A connection acquired directly from `db.pool()` is dirtied with `PRAGMA query_only = ON;` and dropped. `after_release` reads `1`, returns `Ok(false)`, and SQLx closes the connection. The subsequent acquire returns a clean writable connection.
2. **Hook Query Failure (`sqlx::query_scalar` returns `Err(e)`):**
   - **NOT TESTED.** On a healthy connection, `PRAGMA query_only;` returns a valid integer without error.
3. **Hook Returning `Err(...)`:**
   - **IMPOSSIBLE IN OPENTALLY DESIGN.** Our hook closure handles query errors internally (`Err(e) => return Ok(false)`). The hook signature returns `Result<bool, sqlx::Error>`, but it **never returns `Err`**; it fail-closes by returning `Ok(false)` so SQLx closes the connection gracefully via `self.close().await`. SQLx's `match ... Err(error) => self.close_hard().await` branch is therefore unreachable by design.

---

## 4. Integrity Verification & SHA-256 Checksums

### Byte-for-Byte Comparison Matrix
Every file in `02-open-tally/docs/phase2-final-source-review/` was generated by direct file copy and compared against the working tree source:

| Copied Review File | Original Working Tree File | SHA-256 Checksum | Byte-for-Byte Match |
|:---|:---|:---:|:---:|
| `database/mod.rs` | `02-open-tally/src-tauri/src/database/mod.rs` | `29865B392C8ABC231CE7AD57C5A9E06E95C9BE4029227F43C3659B3704E9FE55` | **MATCH (True)** |
| `vouchers/service/search.rs` | `02-open-tally/src-tauri/src/vouchers/service/search.rs` | `45DED32D5A10CCB1D4E2A633EFFD446F7A1A60149AB72D7DC7BED41E5F6B85BC` | **MATCH (True)** |
| `vouchers/stage7_hardening_tests.rs` | `02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs` | `AB3D93469730471767FAC152D3A34C627A2CA12E4FCCE99644146381732CB2CA` | **MATCH (True)** |
| `vouchers/stage7_repository_tests.rs` | `02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs` | `E750A25BC319F10C299811BF3DB03A267D54B5DDF582FF0E1219156B544C312C` | **MATCH (True)** |
| `vouchers/mod.rs` | `02-open-tally/src-tauri/src/vouchers/mod.rs` | `CA14F3A1B9D7D8EB5BBC233717D7BFE634E44DC35D00D5338102C8E9A3D8DD55` | **MATCH (True)** |
| `vouchers/service/mod.rs` | `02-open-tally/src-tauri/src/vouchers/service/mod.rs` | `1CCBB21978B88BE931A07D6BFCCCEC710FFE6B30F2398AD34E9D820CA85FF745` | **MATCH (True)** |
| `vouchers/validator.rs` | `02-open-tally/src-tauri/src/vouchers/validator.rs` | `71A97E36044F8F526BAC5FBA440068E7821E56080352647AC1C74AE264332466` | **MATCH (True)** |

### Git State Verification
- **Current Branch:** `feature/phase1`
- **Output of `git status --short`:**
  ```text
   M 02-open-tally/src-tauri/src/database/mod.rs
   M 02-open-tally/src-tauri/src/vouchers/mod.rs
   M 02-open-tally/src-tauri/src/vouchers/service/mod.rs
   M 02-open-tally/src-tauri/src/vouchers/validator.rs
  ?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md
  ?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md
  ?? 02-open-tally/docs/PHASE2_SOURCE_REVIEW_INDEX.md
  ?? 02-open-tally/docs/STAGE7A_PHASE2_FINAL_ACCEPTANCE_EVIDENCE.md
  ?? 02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md
  ?? 02-open-tally/docs/phase2-final-source-review/
  ?? 02-open-tally/src-tauri/src/vouchers/service/search.rs
  ?? 02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs
  ?? 02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs
  ```
- **Integrity Invariants:**
  - Zero source code or tests were modified during this review.
  - Zero files have been staged or committed.
  - Zero Phase 3 implementation has been created.

