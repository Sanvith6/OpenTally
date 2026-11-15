# OpenTally — Stage 7A Phase 2 Final Acceptance Evidence Package

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase:** 2 — Read-Only Search Repository Hardening  
**Target Milestone Design:** [OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md](file:///c:/project/OpenTally/02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md) (v2.2.2)  
**Database Schema:** `0.3.1-sprint2` (Strictly Unmodified)  
**Audit Date:** 2026-11-11  
**Audit Mode:** Source-backed read-only verification  

---

## 1. Executive Verdict

### **VERDICT: READY FOR INDEPENDENT ACCEPTANCE REVIEW**

All six findings (`FIND-7A-P2-01` through `FIND-7A-P2-06`) identified during the independent reconciliation audit have been remediated with concrete, source-backed implementations and verified by 18 targeted integration tests (`test_h01` through `test_h18`).

- **Cancellation & Panic Safety:** Proven fail-closed via RAII `ReadOnlyConnectionGuard` invoking `conn.close_on_drop()` on any future-drop or panic, reinforced by a global pool `.after_release` verification hook draining SQLite worker commands via `PRAGMA query_only;`.
- **Failure-Path & Boundary Coverage:** Proven across unauthorized write DML rejection, simulated rollback/commit failures, recursive lineage depth 10 / >10 truncation, cyclic graph termination, prefix ID collision immunity, and exact 30-character canonical SQL monetary comparisons ($10^{-4}$ to $10^{25}-1$).
- **Deterministic Sort & Tie-Breaking:** All 4 sort fields (`Date`, `VoucherNumber`, `TotalAmount`, `CreatedAt`) Ascending and Descending, draft NULL total amounts, same-date sequence number tie-breaking, and stable `v.id ASC` tie-breaking verified.
- **Repository-Level Parameter Bounds:** Directly validated at the repository boundary before database execution.
- **Multi-Tenant Isolation & WAL Snapshot Consistency:** Verified against cross-company audit log ID collisions, foreign lineage references, and mid-transaction concurrent write commits.
- **Quality Gates:** 100% green across compilation (`cargo check`), formatting (`cargo fmt --check`), linter (`cargo clippy --all-targets -- -D warnings`), backend tests (**316 passed, 0 failed, 0 ignored**), and frontend tests (**27 passed, 0 failed**).
- **Governance & Boundaries:** Zero modifications to database migrations, dependencies, or git staging/commits. Zero Phase 3 export code implemented.

*Procedural Notice: In accordance with project governance, this evidence package establishes technical readiness; formal acceptance of Stage 7A Phase 2 is reserved for the independent acceptance review authority.*

---

## 2. Exact Implementation Areas & Source Locations

| # | Subsystem / Mechanism | Source File Path | Line Range |
|:---|:---|:---|:---:|
| 1 | `ReadOnlyConnectionGuard` | [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L76-L119) | Lines 76–119 |
| 2 | `execute_read_only_query` | [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L132-L244) | Lines 132–244 |
| 3 | SQLx transaction creation, commit & rollback | [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L162-L190) | Lines 162–190 |
| 4 | `close_on_drop()` activation & lifecycle | [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L102-L118)<br>`sqlx-core-0.8.6/src/pool/connection.rs` | Lines 102–118<br>Lines 97–99, 157–176, 199–211 |
| 5 | `DatabaseManager::after_release` | [`02-open-tally/src-tauri/src/database/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L77-L108) | Lines 77–108 |
| 6 | SQLite worker command queue & FIFO draining | `sqlx-sqlite-0.8.6/src/connection/worker.rs` | Lines 109, 144–146, 220–290 |
| 7 | Normal connection reset & verification logic | [`02-open-tally/src-tauri/src/vouchers/service/search.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L209-L243) | Lines 209–243 |
| 8 | Targeted Hardening tests `test_h01`–`test_h18` | [`02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs#L314-L1506) | Lines 314–1506 |

---

## 3. Relevant Source Excerpts

### Excerpt 1: `ReadOnlyConnectionGuard` (`search.rs:76-119`)
```rust
struct ReadOnlyConnectionGuard {
    conn: Option<sqlx::pool::PoolConnection<Sqlite>>,
    clean_for_reuse: bool,
}

impl ReadOnlyConnectionGuard {
    fn new(conn: sqlx::pool::PoolConnection<Sqlite>) -> Self {
        Self {
            conn: Some(conn),
            clean_for_reuse: false,
        }
    }

    fn conn_mut(&mut self) -> Result<&mut sqlx::pool::PoolConnection<Sqlite>, OpenTallyError> {
        self.conn.as_mut().ok_or_else(|| {
            OpenTallyError::Internal(
                "Read-only connection guard accessed after connection disposal".to_string(),
            )
        })
    }

    fn release_clean(mut self) {
        self.clean_for_reuse = true;
        drop(self);
    }

    async fn close_and_discard(mut self) {
        self.clean_for_reuse = false;
        if let Some(mut conn) = self.conn.take() {
            conn.close_on_drop();
            let _ = conn.close().await;
        }
    }
}

impl Drop for ReadOnlyConnectionGuard {
    fn drop(&mut self) {
        if !self.clean_for_reuse {
            if let Some(mut conn) = self.conn.take() {
                conn.close_on_drop();
            }
        }
    }
}
```

### Excerpt 2: `execute_read_only_query` Execution Scoping & Verification (`search.rs:132-244`)
```rust
pub async fn execute_read_only_query<F, T>(pool: &Pool<Sqlite>, f: F) -> Result<T, OpenTallyError>
where
    F: for<'c> FnOnce(
        &'c mut sqlx::SqliteConnection,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<T, OpenTallyError>> + Send + 'c>,
    >,
{
    let conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
    let mut guard = ReadOnlyConnectionGuard::new(conn);

    // Step 1: Enforce SQLite engine-level read-only mode (Mode 1 failure guard)
    if let Err(e) = sqlx::query("PRAGMA query_only = ON;")
        .execute(&mut **guard.conn_mut()?)
        .await
    {
        guard.close_and_discard().await;
        return Err(OpenTallyError::Database(e));
    }

    enum TxOutcome<T> {
        Committed(T),
        ClosureFailed(OpenTallyError),
        TxLifecycleFailed(sqlx::Error),
    }

    // Step 2 & 3: Scoped managed SQLx transaction so all borrows of `guard` end at `}`.
    let outcome = {
        match guard.conn_mut()?.begin().await {
            Err(e) => TxOutcome::TxLifecycleFailed(e),
            Ok(mut tx) => {
                match f(&mut tx).await {
                    Ok(val) => match tx.commit().await {
                        Ok(()) => TxOutcome::Committed(val),
                        Err(e) => TxOutcome::TxLifecycleFailed(e),
                    },
                    Err(err) => {
                        // Mode 3 & 4: Query failed; attempt rollback before discarding connection.
                        let _ = tx.rollback().await;
                        TxOutcome::ClosureFailed(err)
                    }
                }
            }
        }
    };

    // Step 4: Finalize pool hygiene (Modes 3, 4, 5, 6 guards + cancellation/panic)
    let val = match outcome {
        TxOutcome::Committed(v) => v,
        TxOutcome::ClosureFailed(err) => {
            guard.close_and_discard().await;
            return Err(err);
        }
        TxOutcome::TxLifecycleFailed(e) => {
            guard.close_and_discard().await;
            return Err(OpenTallyError::Database(e));
        }
    };

    // Clean reset to query_only = OFF before returning connection to pool (Mode 6 guard)
    if let Err(e) = sqlx::query("PRAGMA query_only = OFF;")
        .execute(&mut **guard.conn_mut()?)
        .await
    {
        guard.close_and_discard().await;
        return Err(OpenTallyError::Database(e));
    }

    // Explicit post-reset hygiene check before marking connection clean for reuse
    let query_only = match sqlx::query_scalar::<_, i64>("PRAGMA query_only;")
        .fetch_one(&mut **guard.conn_mut()?)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            guard.close_and_discard().await;
            return Err(OpenTallyError::Database(e));
        }
    };

    let tx_depth = <sqlx::Sqlite as sqlx::Database>::TransactionManager::get_transaction_depth(
        &**guard.conn_mut()?,
    );

    if query_only != 0 || tx_depth != 0 {
        guard.close_and_discard().await;
        return Err(OpenTallyError::Internal(format!(
            "SQLite connection failed post-query hygiene verification (query_only={}, tx_depth={})",
            query_only, tx_depth
        )));
    }

    // Only now is the connection guaranteed clean for pool reuse.
    guard.release_clean();
    Ok(val)
}
```

### Excerpt 3: `DatabaseManager::after_release` Hook (`database/mod.rs:77-108`)
```rust
            .after_release(|conn, _meta| {
                Box::pin(async move {
                    // Execute PRAGMA query_only first so the FIFO SQLite worker drains any
                    // pending async rollback queued by a dropped Transaction before checking depth.
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

### Excerpt 4: Upstream SQLx 0.8.6 `PoolConnection::drop` & `return_to_pool` (`sqlx-core-0.8.6/src/pool/connection.rs`)
```rust
// Lines 97-99
#[inline(always)]
pub fn close_on_drop(&mut self) {
    self.close_on_drop = true;
}

// Lines 199-211
impl<DB: Database> Drop for PoolConnection<DB> {
    fn drop(&mut self) {
        if self.close_on_drop {
            crate::rt::spawn(self.take_and_close());
            return;
        }

        // We still need to spawn a task to maintain `min_connections`.
        if self.live.is_some() || self.pool.options.min_connections > 0 {
            crate::rt::spawn(self.return_to_pool());
        }
    }
}

// Lines 289-305 (Inside Floating::return_to_pool)
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

### Excerpt 5: Upstream SQLite FIFO Worker Execution Loop (`sqlx-sqlite-0.8.6/src/connection/worker.rs`)
```rust
// Line 109 & Line 144
let (command_tx, command_rx) = flume::bounded(params.command_channel_size);
// ...
for (cmd, span) in command_rx {
    let _guard = span.enter();
    match cmd {
        // ...
        Command::Rollback { tx } => {
            let depth = shared.transaction_depth.load(Ordering::Acquire);
            let res = if depth > 0 {
                conn.handle
                    .exec(rollback_ansi_transaction_sql(depth))
                    .map(|_| {
                        shared.transaction_depth.fetch_sub(1, Ordering::Release);
                    })
            } else {
                Ok(())
            };
            // ...
        }
    }
}
```

---

## 4. Resolution of the PRAGMA Question

The exact behavior of `PRAGMA query_only` across both `execute_read_only_query` and `DatabaseManager::after_release` has been verified at the byte level:

### 4.1 What does `after_release` execute?
[`DatabaseManager::after_release`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/mod.rs#L81) executes:
```rust
sqlx::query_scalar::<_, i64>("PRAGMA query_only;").fetch_one(&mut *conn).await
```
It executes `PRAGMA query_only;` (with `fetch_one`), **NOT** `PRAGMA query_only = OFF;`.

### 4.2 Does it read the current value or change the connection setting?
It **reads** the current value (`flag: i64`). It does **not** change the connection setting.
- `PRAGMA query_only;` is a read query that inspects whether SQLite is currently enforcing read-only mode (`0` = read-write, `1` = read-only).
- `PRAGMA query_only = OFF;` is an assignment statement that alters SQLite's engine mode.
- `after_release` acts strictly as an **independent auditor/gatekeeper**. It never repairs or mutates a dirty connection; if a connection is not already clean (`query_only == 0 && tx_depth == 0`), it returns `Ok(false)` so SQLx permanently closes and discards the connection.

### 4.3 Where is `query_only` disabled during normal cleanup?
`query_only` is disabled in [`execute_read_only_query`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/search.rs#L209-L215) immediately following transaction commitment:
```rust
if let Err(e) = sqlx::query("PRAGMA query_only = OFF;")
    .execute(&mut **guard.conn_mut()?)
    .await
{
    guard.close_and_discard().await;
    return Err(OpenTallyError::Database(e));
}
```

### 4.4 What happens if the cleanup command fails?
If `PRAGMA query_only = OFF;` fails:
1. `execute_read_only_query` immediately enters the `Err(e)` branch and calls `guard.close_and_discard().await`.
2. Inside `close_and_discard`, `conn.close_on_drop()` is invoked, and `conn.close().await` is awaited.
3. The guard is consumed without ever calling `guard.release_clean()`.
4. SQLx's `PoolConnection::close` executes `self.take_live()`, extracting `live` so the connection handle is empty and can never be re-queued into `pool.idle_ch`.
5. Even if `close_and_discard` were somehow bypassed, `after_release` would execute `PRAGMA query_only;`, receive `1`, log a warning, and return `Ok(false)`, discarding the connection.

### 4.5 Can a connection with `query_only=ON` return to the pool?
**NO.** A connection with `query_only = ON` cannot return to the pool under any execution path for two independent fail-closed reasons:
1. **Primary Defense (`ReadOnlyConnectionGuard`):** `clean_for_reuse` is initialized to `false`. It is only flipped to `true` by `guard.release_clean()` after lines 218–239 explicitly read `PRAGMA query_only;` and confirm it equals `0` (and `tx_depth == 0`). On any error, panic, or async future-drop, `guard.release_clean()` is skipped, causing `Drop` to invoke `conn.close_on_drop()`, which permanently floats and discards the connection.
2. **Secondary Defense (`DatabaseManager::after_release`):** If an ad-hoc connection outside `execute_read_only_query` is released with `PRAGMA query_only = ON`, `after_release` executes `PRAGMA query_only;`, reads `1`, logs a warning, and returns `Ok(false)`, instructing SQLx to close and discard the connection.

### 4.6 Does the FIFO drain guarantee that all previously queued worker commands have completed?
**YES.**
In `sqlx-sqlite-0.8.6`, the underlying SQLite database handle is owned by a dedicated background OS worker thread (`thread::Builder::new().spawn(...)`). Commands (`Prepare`, `Execute`, `Begin`, `Commit`, `Rollback`, `Ping`) are submitted via a FIFO channel (`flume::Sender<Command>`).
When a SQLx `Transaction` drops without being committed, its `Drop` implementation queues an asynchronous `Command::Rollback { tx: None }` to this FIFO channel.
Because the channel is strictly FIFO:
1. When `after_release` executes `sqlx::query_scalar("PRAGMA query_only;").fetch_one(&mut *conn).await`, it sends a `Command::Execute` message to the worker's channel.
2. The worker thread must process and execute all preceding items in the queue—specifically the pending `Command::Rollback`—before it can process `Command::Execute`.
3. Executing `Command::Rollback` decrements `shared.transaction_depth` via `fetch_sub(1, Ordering::Release)` on SQLite.
4. Only then does the worker process `PRAGMA query_only;` and send the response back across the oneshot reply channel.
5. Therefore, awaiting `fetch_one` guarantees that all previously queued commands (including asynchronous rollbacks) have finished executing before `TransactionManager::get_transaction_depth(conn)` is evaluated on the subsequent line.

### 4.7 What happens if the hook itself fails?
If the hook encounters an error (e.g. SQLite error or disconnected worker), it logs:
`"Discarding pooled SQLite connection: PRAGMA query_only check failed: ..."` and returns `Ok(false)`.
In SQLx 0.8.6 `Floating::return_to_pool` (`connection.rs:293-296`), returning `Ok(false)` causes SQLx to immediately invoke `self.close().await` and return `false`. The connection is never returned to the idle pool, and `DecrementSizeGuard` releases the pool permit and decrements the pool size.

### 4.8 Technical Distinction: Read vs Set vs FIFO Drain
- **Reading a PRAGMA (`PRAGMA query_only;`)**: An inspection query (`SELECT`/`fetch_one`) that returns the current scalar integer flag. It verifies whether read-only mode is active without mutating state.
- **Setting a PRAGMA (`PRAGMA query_only = OFF;`)**: A mutating configuration statement (`execute`) that commands SQLite to exit read-only mode.
- **Draining queued worker commands**: A concurrency synchronization barrier achieved by awaiting a command roundtrip through SQLite's single-threaded FIFO worker channel, ensuring any asynchronous rollbacks dispatched by dropped transactions are fully applied before inspecting shared memory atomics.

---

## 5. SQLx 0.8.6 Cancellation Safety Verification

Direct source analysis of `sqlx-core-0.8.6` and `sqlx-sqlite-0.8.6` installed in `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` proves the cancellation-safety guarantees:

### 5.1 When is `close_on_drop()` activated?
- `ReadOnlyConnectionGuard` initializes with `clean_for_reuse = false`.
- If the guard is dropped at any time without `guard.release_clean()` having been called, `ReadOnlyConnectionGuard::drop` executes:
  ```rust
  if !self.clean_for_reuse {
      if let Some(mut conn) = self.conn.take() {
          conn.close_on_drop();
      }
  }
  ```
- Calling `conn.close_on_drop()` sets `self.close_on_drop = true` on SQLx's `PoolConnection` (`sqlx-core-0.8.6/src/pool/connection.rs:97-99`).

### 5.2 What happens when a future is cancelled while awaiting a query?
1. The future returned by `execute_read_only_query` is dropped mid-`.await`.
2. As the task stack unwinds, `ReadOnlyConnectionGuard::drop` executes.
3. Because `clean_for_reuse` is `false`, it invokes `conn.close_on_drop()`.
4. When `conn: PoolConnection<Sqlite>` drops, its `Drop` implementation (`connection.rs:200-204`) executes:
   ```rust
   if self.close_on_drop {
       crate::rt::spawn(self.take_and_close());
       return;
   }
   ```
5. `take_and_close()` calls `self.live.take().map(|live| live.float(self.pool.clone()))`.
6. This extracts `live` into a `Floating` wrapped with `DecrementSizeGuard`.
7. `floating.close()` closes the connection transport. When `DecrementSizeGuard` drops, it calls `pool.size.fetch_sub(1)` and `pool.semaphore.release(1)`.
8. The dirty connection is **never** sent to `pool.idle_ch`. The pool opens a fresh replacement connection on the next acquire. (Verified in `test_h01`).

### 5.3 What happens when cancellation occurs during transaction cleanup?
- If cancellation occurs after `tx.commit().await` but before `PRAGMA query_only = OFF;` completes, `clean_for_reuse` is still `false`.
- Guard `Drop` invokes `conn.close_on_drop()`, permanently discarding the connection despite the committed transaction. (Verified in `test_h02`).
- If cancellation occurs after `PRAGMA query_only = ON;` before `tx.begin().await`, guard `Drop` invokes `conn.close_on_drop()`, permanently discarding the connection before any transaction begins. (Verified in `test_h03`).

### 5.4 Is the guard's drop behavior sufficient to prevent pool reuse?
**YES.** Under `sqlx-core-0.8.6`, setting `self.close_on_drop = true` bypasses `self.return_to_pool()`. `take_and_close` drains and drops the connection, guaranteeing that `pool.idle_ch` never receives the handle.

### 5.5 Can the successful path safely release the connection?
**YES.**
On a successful query:
1. `tx.commit().await` succeeds.
2. `PRAGMA query_only = OFF;` executes successfully.
3. `PRAGMA query_only;` is queried and confirmed equal to `0`.
4. `TransactionManager::get_transaction_depth` is confirmed equal to `0`.
5. Only then is `guard.release_clean()` called, setting `clean_for_reuse = true`.
6. When `conn` drops normally, `close_on_drop` is `false`, allowing `return_to_pool()` to execute.
7. Inside `return_to_pool()`, `DatabaseManager::after_release` executes, verifies `query_only == 0 && tx_depth == 0`, and returns `Ok(true)`.
8. The clean connection returns to `pool.idle_ch` and is reused without reopening. (Verified in `test_h01` warmup queries: `connect_count == 1`).

### 5.6 Are panic and early-return paths protected?
**YES.**
- **Early returns (`?` or `return Err`):** The guard drops, `clean_for_reuse` is `false`, and `conn.close_on_drop()` is called.
- **Unwind panics (`panic!` inside closure):** Tokio task unwinding drops all local variables, including `ReadOnlyConnectionGuard`. Its `Drop` implementation calls `conn.close_on_drop()`, ensuring the connection is discarded. (Verified in `test_h04`).

---

## 6. Test-by-Test Evidence Verification (`test_h01` through `test_h18`)

| Test Identifier | Category | Behavior Exercised | Actual Assertion Verified | Pass Evidence |
|:---|:---:|:---|:---|:---:|
| `test_h01_cancel_inside_closure_discards_contaminated_connection` | Real Integration Test (Async Cancellation) | Tokio task aborted while awaiting inside read-only query closure on single-connection pool | `connect_count == 2`; `assert_pool_writable_and_clean` succeeds (`query_only == 0`, DDL & DML write succeeds) | `ok` (11.19s suite) |
| `test_h02_cancel_after_commit_before_pragma_reset_discards_connection` | Real Integration Test (Async Cancellation via Hook) | Tokio task aborted at `AfterCommitBeforeReset` before `PRAGMA query_only = OFF` executes | `connect_count == 2`; `assert_pool_writable_and_clean` succeeds on connection #2 | `ok` (11.19s suite) |
| `test_h03_cancel_after_pragma_on_before_begin_discards_connection` | Real Integration Test (Async Cancellation via Hook) | Tokio task aborted at `AfterPragmaOnBeforeBegin` before `tx.begin()` executes | `connect_count == 2`; `assert_pool_writable_and_clean` succeeds on connection #2 | `ok` (11.19s suite) |
| `test_h04_panic_inside_closure_discards_contaminated_connection` | Real Integration Test (Panic Unwinding) | Unwind `panic!` triggered inside read-only closure | `join_err.is_panic() == true`; `connect_count == 2`; `assert_pool_writable_and_clean` succeeds | `ok` (11.19s suite) |
| `test_h05_database_manager_after_release_rejects_dirtied_connection` | Real Integration Test (Pool Hook) | Direct connection manually dirtied with `PRAGMA query_only = ON;` and dropped into `DatabaseManager` pool | `assert_pool_writable_and_clean` succeeds (`after_release` rejected dirty conn; returned conn has `query_only == 0`) | `ok` (11.19s suite) |
| `test_h06_closure_error_and_readonly_dml_failure_rollback_and_discard` | Real Integration Test (SQLite Engine Error) | Unauthorized write DML inside read-only query and domain `Err` | `write_err` is `OpenTallyError::Database` containing `"readonly"`; `connect_count` increments to 2, then 3; pool writable | `ok` (11.19s suite) |
| `test_h07_real_sqlite_rollback_failure_still_discards_connection` | Simulated Failure Test (Simulated Rollback Error) | Closure prematurely executes `ROLLBACK;`, forcing SQLx inner `tx.rollback().await` to fail with "cannot rollback - no transaction is active" | `res` is `Err(Validation)`; `connect_count == 2`; connection #1 discarded and #2 clean | `ok` (11.19s suite) |
| `test_h08_real_sqlite_commit_failure_discards_connection` | Simulated Failure Test (Simulated Commit Error) | Closure prematurely executes `COMMIT;`, forcing SQLx inner `tx.commit().await` to fail while `query_only = 1` active | `res` is `Err(Database)`; `connect_count == 2`; connection #1 discarded and #2 clean | `ok` (11.19s suite) |
| `test_h09_lineage_depth_under_ten_exact_boundaries` | Real Integration Test | Recursive lineage on 4-hop (5-node) chain from root and leaf | `items.len() == 5`; `has_exceeded_depth_limit == false`; `max_depth_reached == 4` | `ok` (11.19s suite) |
| `test_h10_lineage_depth_exactly_ten_not_truncated` | Real Integration Test | Recursive lineage on exact 10-hop (11-node) chain | `items.len() == 11`; `max_depth_reached == 10`; `has_exceeded_depth_limit == false` | `ok` (11.19s suite) |
| `test_h11_lineage_depth_exceeds_ten_truncates_forward_and_backward` | Real Integration Test | Recursive lineage on 12-hop (13-node) chain | Forward from root: `items.len() == 11`, `max_depth == 10`, `exceeded == true`; Backward from leaf: `items.len() == 11`, `max_depth == 10`, `exceeded == true`; Midpoint: `items.len() == 13`, `max_depth == 6`, `exceeded == false` | `ok` (11.19s suite) |
| `test_h12_lineage_cyclic_graph_and_prefix_id_collision` | Real Integration Test | 3-node cyclic graph (`v100 -> v10 -> v1 -> v100`) and prefix IDs (`v1`, `v10`, `v100`) | Query terminates cleanly returning all 3 nodes; `exceeded == false`; delimiter path matching prevents substring false positives | `ok` (11.19s suite) |
| `test_h13_monetary_sql_boundaries_and_invalid_query_rejections` | Real Integration Test | 30-char canonical SQL comparisons on $0.0001$, $1$, 20-digit int, max stored decimal (`99999999999999999999.9999`), max stored int (`9999999999999999999999999`) | Ascending sort ordered correctly across all magnitudes; exact `1.0000` matches `1`; upper query filter `9999999999999999999999999.9999` matches max rows; invalid amounts fail validation | `ok` (11.19s suite) |
| `test_h14_sort_all_fields_asc_desc_tiebreakers_nulls_and_stability` | Real Integration Test | All 4 sort fields Asc/Desc, draft NULL total_amount, sequence number tie-breaking, and stable `v.id ASC` | `Date` Desc/Asc, `VoucherNumber` Asc/Desc, `TotalAmount` Asc/Desc (NULL at bottom/top), and `CreatedAt` Asc/Desc match exact deterministic sequences | `ok` (11.19s suite) |
| `test_h15_repository_pagination_guards_direct_calls_and_total_overflow` | Real Integration Test | Direct calls to `SqliteVoucherSearchRepository` bypassing service validation | Rejects `page=0`, `page_size=0`, `page_size=101`, offset overflow with `Validation` error on empty and populated DB; `compute_total_pages` handles 0, ceil, max, and overflow safely | `ok` (11.19s suite) |
| `test_h16_cross_company_audit_log_collision_and_isolation` | Real Integration Test | Companies A & B share identical voucher ID `shared_v1`; Comp B cancels later with secret reason; Comp A cancelled row with no audit entry | Comp A query returns strictly Comp A's reason; Comp A row with no audit returns `None` without leaking Comp B's colliding entry | `ok` (11.19s suite) |
| `test_h17_cross_company_lineage_edges_and_shared_ids` | Real Integration Test | Cross-company lineage edge pointing to foreign voucher ID | Traversal terminates at company boundary; foreign voucher lookup returns `NotFound`; ledger filter across companies returns 0 | `ok` (11.19s suite) |
| `test_h18_true_concurrent_write_snapshot_isolation_count_and_select` | Real Integration Test | Multi-connection WAL pool; concurrent writer commits 4th voucher while reader is suspended between `COUNT(*)` and `SELECT` | Reader returns `total: 3` and `items.len(): 3` without phantom row; subsequent search sees 4 rows | `ok` (11.19s suite) |

---

## 7. Quality-Gate Results

All verification commands executed cleanly on 2026-11-11 without modifying the repository:

| Verification Check | Exact Command Line | Exit Code | Result Details |
|:---|:---|:---:|:---|
| **Compilation Check** | `cargo check --manifest-path 02-open-tally/src-tauri/Cargo.toml` | **0** | Clean, finished in 25.73s |
| **Code Formatting** | `cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml -- --check` | **0** | Clean, 0 formatting discrepancies |
| **Clippy Linter** | `cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml --all-targets -- -D warnings` | **0** | Clean, 0 warnings across library and test targets |
| **Backend Test Suite** | `cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml` | **0** | **316 passed; 0 failed; 0 ignored** in 11.19s |
| **Frontend Test Suite** | `npm --prefix 02-open-tally/frontend test -- --run` | **0** | **3 test files passed, 27 tests passed** in 5.32s |

---

## 8. Current Git State

```text
## feature/phase1...origin/feature/phase1
 M 02-open-tally/src-tauri/src/database/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/service/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/validator.rs
?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md
?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md
?? 02-open-tally/docs/STAGE7A_PHASE2_FINAL_ACCEPTANCE_EVIDENCE.md
?? 02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md
?? 02-open-tally/src-tauri/src/vouchers/service/search.rs
?? 02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs
?? 02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs
```

- **Current Branch:** `feature/phase1`
- **Tracked Modified Files:**
  - `02-open-tally/src-tauri/src/database/mod.rs` (Added `.after_release` FIFO drain & pool hygiene hook)
  - `02-open-tally/src-tauri/src/vouchers/mod.rs` (Registered test modules)
  - `02-open-tally/src-tauri/src/vouchers/service/mod.rs` (Exported search service)
  - `02-open-tally/src-tauri/src/vouchers/validator.rs` (Exported pagination validator)
- **Untracked Deliverables:**
  - `02-open-tally/src-tauri/src/vouchers/service/search.rs` (Search engine & read-only guard)
  - `02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs` (25 repository tests)
  - `02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs` (18 hardening tests)
  - `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md` (Updated report)
  - `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md` (Reconciliation audit + addendum)
  - `02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md` (Hardening remediation report)
  - `02-open-tally/docs/STAGE7A_PHASE2_FINAL_ACCEPTANCE_EVIDENCE.md` (This evidence package)
- **Git Staging / Commit Invariant:** Zero files staged; zero commits made; working tree preserved exactly.

---

## 9. Remaining Risks, Maintainability Notes & Limitations

1. **File Line Count Soft Ceiling (872 lines in `search.rs`):**
   - Soft maintainability guideline is 800 lines. `search.rs` is currently 872 lines (~40 lines are test instrumentation `#[cfg(test)] enum ReadOnlyLifecyclePoint`). In a subsequent refactoring pass (e.g. Stage 8 or Sprint 2 closing), `search.rs` should be decomposed into submodules (`guard.rs`, `query.rs`, `repository.rs`).
2. **Function Length Ceilings:**
   - Functions `execute_read_only_query`, `build_filtered_query`, `search_vouchers_impl`, and `get_voucher_lineage_impl` exceed 50 lines due to exhaustive 20-column SQL mapping and multi-state error handling. These are cohesive and linear, but helper extraction (`map_voucher_row`) is recommended for future polish.
3. **Standalone Test Pools:**
   - Tests that construct ad-hoc `SqlitePoolOptions::new()` outside `DatabaseManager::init` rely on `ReadOnlyConnectionGuard`'s fail-closed `close_on_drop()` rather than `.after_release`. Both layers are completely effective, but all production pools go through `DatabaseManager::init` and have dual-layer protection.
4. **Phase Boundary Scope:**
   - Phase 3 (CSV/JSON file streaming, directory path security sanitization, and Win32 `ReplaceFileW` atomic finalization) is not implemented.

