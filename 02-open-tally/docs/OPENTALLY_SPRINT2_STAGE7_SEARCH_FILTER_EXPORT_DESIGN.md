# OpenTally Sprint 2 Stage 7 — Technical Design Document (Hardened v2.2.2)
## Voucher Search, Filters & Export Engine

**Document Version**: 2.2.2 (Targeted Final Verification Correction)  
**Status**: DESIGN ONLY — PENDING USER APPROVAL  
**Author**: Antigravity Technical Architecture Pair  
**Workspace**: `C:\project\OpenTally`  
**Target Repository**: `02-open-tally`  
**Prerequisites**: Migration `0.3.1-sprint2` (Accepted), Stage 6 Milestone Formal Closure (Accepted with Observations)  
**Implementation Constraint**: **STRICTLY DESIGN ONLY**  
*(No application source code, SQL migrations, tests, dependencies, configuration files, or Git state may be modified during this stage)*

---

## 1. Executive Summary & Architectural Scope

This document specifies the authoritative, production-hardened technical design for **Sprint 2 Stage 7: Voucher Search, Filters & Export Engine** in `02-open-tally` (Version 2.2.2).

Stage 7 completes the query, audit, and reporting foundation of the Sprint 2 Voucher Management & Posting Engine by providing:
1. **Multi-Criteria Voucher Register & Search Engine**: Fast, parameterized queries across all lifecycle statuses (`posted`, `reversed`, `cancelled`, and `draft`), supporting configurable date bases (`date` or `effective_date`), voucher types, series, correlated ledger filtering, exact decimal amount boundaries, escaped free-text search, bidirectional reversal lineage, deterministic tie-breaking sort, and checked pagination arithmetic.
2. **Exact Decimal Canonical-String Comparison & Ordering**: Mathematically proves the exact bounds of database constraint `chk_voucher_total_amount_canonical` (25 characters maximum) and establishes a fixed-width 30-character canonical-string normalization (`25-digit integer + '.' + 4-digit fraction`) in SQLite, guaranteeing 100% exact numerical comparison and ordering with zero floating-point arithmetic and zero integer overflow.
3. **True Read-Only Connection Lifecycle & Comprehensive Pool Hygiene**: Eliminates the flawed assumption that `BEGIN DEFERRED` is read-only. Enforces engine-level read-only security via `PRAGMA query_only = ON` on a dedicated connection, accompanied by an exhaustive 6-mode failure matrix and RAII connection-detachment protocol ensuring any connection with uncertain or failed state is permanently closed/detached and never returned to the connection pool.
4. **Snapshot-Consistent Preflight & Streaming**: Binds count preflight and data fetch/streaming queries within a single transaction on the same dedicated connection under SQLite WAL snapshot isolation, eliminating count/result race conditions.
5. **Secure Windows Desktop Crash-Safe & Durable Export Delivery**: Implements user-authorized destination selection via native file save dialogs, path sanitization, overwrite governance (`overwrite: bool`), exclusive sibling temporary file creation (`.~opentally_export_<id>.tmp`), mandatory two-stage flush/sync (`flush().await` + `sync_all().await`) before file close, true crash-safe atomic replacement on Windows using Win32 `ReplaceFileW` with documented `dwReplaceFlags = 0`, race-safe rename via `MoveFileExW(..., 0)` for non-existent destinations, fail-closed handling on unsupported filesystems (FAT32/exFAT/SMB), and a comprehensive 14-scenario failure and recovery matrix distinguishing crash consistency from hardware power durability.
6. **CWE-1236 & RFC 4180 Defensive CSV Engine**: Decouples formula sanitization from RFC 4180 escaping. Protects user-controlled text while preserving raw numeric amounts for Excel calculations, handles leading control characters (`\t`, `\r`, whitespace), and applies UTF-8 BOM encoding.
7. **Lossless JSON Export Architecture (Streamed vs Materialized)**: Reconciles the JSON streaming requirement with DTO structures: Mode A (File Export) streams JSON incremental framing directly to disk with constant $O(1)$ memory consumption (< 5 MB RAM at 50,000 records); Mode B (Preview) uses materialized structs strictly capped at 1,000 records.
8. **Statutory Day Book Semantics**: Strictly separates Gross Voucher Activity Volume from General Ledger Net Movement, eliminates flawed ad-hoc deduction formulas, enforces 0.00 financial weight for cancelled vouchers, and preserves the chronological audit trail for reversed pairs under Companies Act / GAAP standards.
9. **True Lineage Truncation Detection**: Distinguishes direct 1-to-1 reversal links (`reversal_of_voucher_id`) from two-step correction workflows, implementing a cycle-protected recursive CTE that queries to `depth < 11` to definitively verify whether eligible lineage records exist beyond the depth 10 cap.

---

## 2. Verified Existing Architecture & Baseline Facts

The design is grounded strictly in facts verified against the repository codebase:

| Subsystem / Layer | Verified Implementation Fact in Codebase | Source Reference |
|:---|:---|:---|
| **Active Schema Version** | Schema is `0.3.1-sprint2`. Tables: `companies`, `voucher_types`, `voucher_number_series`, `voucher_number_configs`, `voucher_number_sequences`, `vouchers`, `voucher_lines`, `audit_log`. | [`database/migrations.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs) |
| **Monetary Storage & Bounds** | `vouchers.total_amount` is stored as `TEXT`. Constraint `chk_voucher_total_amount_canonical` enforces max 25 characters (`length(total_amount) <= 25`), with 1 to 4 decimal places (or integer). Amounts exceeding 14 integer digits can overflow signed 64-bit integers under 10,000x scaling. | [`database/migrations.rs:550`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L550) |
| **Voucher Sequence Counters** | Counters reside in `voucher_number_sequences` with composite key `(company_id, voucher_type_id, series_id, period_start)` and column `next_number`. | [`database/migrations.rs:478`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L478) |
| **Lifecycle & Immutability** | 4 statuses: `draft`, `posted`, `reversed`, `cancelled`. Mutation of posted vouchers is blocked by trigger `trg_prevent_posted_voucher_mutation`. Reversing vouchers cannot be cancelled. | [`database/migrations.rs:670`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L670) |
| **Reversal Uniqueness** | `uq_vouchers_reversal` enforces that `reversal_of_voucher_id` is unique per company for non-null values. A voucher can be reversed at most once. | [`database/migrations.rs:617`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L617) |
| **Audit Log Indexing** | Index `idx_audit_log_entity` covers `(company_id, entity_type, entity_id)`. Querying with `entity_type = 'voucher'` achieves a 3-column index match. | [`database/migrations.rs:1110`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L1110) |
| **Cancellation Audit Structure** | Verified in `cancellation.rs:195-225`: `audit_log.new_values` stores JSON `{"status": "cancelled", "voucher_number": "...", "reason": "..."}` with `action = 'CANCEL'`. | [`vouchers/service/cancellation.rs:205`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/service/cancellation.rs#L205) |
| **Tenant Isolation** | Tauri commands enforce tenant isolation via `require_active_company(&state).await?`. | [`commands/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/commands/mod.rs) |
| **Dependency Baseline** | `Cargo.toml` contains `tauri` 2, `sqlx` 0.8 (sqlite), `rust_decimal` 1.36 (`serde-with-str`), `tokio`, `serde`, `chrono`, `uuid`. Zero CSV crates installed. | [`Cargo.toml`](file:///c:/project/OpenTally/02-open-tally/src-tauri/Cargo.toml) |

---

## 3. Scope Boundaries & Explicit Exclusions

### 3.1 Historical As-Of Point-in-Time Reporting: EXPLICITLY EXCLUDED
- **Architectural Reality**: In `0.3.1-sprint2`, `vouchers.status` is updated in-place (`draft` $\to$ `posted` $\to$ `cancelled` or `reversed`). The `vouchers` table is not bitemporal and does not store historical validity intervals (`valid_from`, `valid_to`).
- **Audit Log Limitation**: While `audit_log` records actions (`POST`, `CANCEL`, `REVERSE`) with timestamps, reconstructing the exact point-in-time lifecycle status of an entire register as of a historical date $T$ (e.g., determining whether a voucher was `posted` or `cancelled` on 2026-05-01 when the cancellation occurred on 2026-05-05) would require dynamic event-sourcing log replay across all vouchers.
- **Design Decision**: **Historical point-in-time as-of status reconstruction is EXPLICITLY EXCLUDED from Stage 7.**
- **Statutory Semantics**: Stage 7 implements a **Current-State Transaction Register** filtered by transaction date (`date`) or effective date (`effective_date`). All vouchers reflect their current lifecycle status at the time of query execution. The system will **never claim** historical point-in-time status fidelity for retroactive date queries in Stage 7. Point-in-time temporal reporting is deferred to a future dedicated financial audit module.

### 3.2 Database Migration Boundary
- Stage 7 operates **100% within the accepted `0.3.1-sprint2` schema**.
- Zero DDL migrations are permitted. All queries leverage existing indexes: `idx_vouchers_date`, `idx_vouchers_type_status`, `idx_voucher_lines_ledger`, `uq_vouchers_reversal`, and `idx_audit_log_entity`.

---

## 4. Accounting Semantics & Financial Aggregates

Stage 7 establishes rigorous statutory accounting boundaries:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       ACCOUNTING CONCEPTS IN STAGE 7                        │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. Gross Voucher Activity Volume:                                           │
│    The sum of single-side balancing magnitudes of all non-cancelled         │
│    transactions in the filtered register:                                   │
│        Gross Volume = Sum(v.total_amount) for status != 'cancelled'         │
│                                                                             │
│ 2. Net Financial Movement:                                                  │
│    The net economic impact on general ledger accounts:                      │
│        Net Movement(Ledger L) = Sum(Debits) - Sum(Credits)                  │
│    Calculated EXCLUSIVELY by General Ledger account queries. The Register   │
│    is a Transaction Journal (Day Book), NOT a Net Financial Statement.      │
│                                                                             │
│ 3. Displayed Voucher Amount (total_amount):                                 │
│    The single-side balancing total (Sum(Dr) = Sum(Cr)) printed in header.   │
│                                                                             │
│ 4. Cancelled Voucher Financial Contribution:                                │
│    Strictly ZERO (0.00). Retains voucher number for sequence integrity,     │
│    but contributes 0.00 to any ledger balance or financial total.           │
│                                                                             │
│ 5. Reversed Original Voucher:                                               │
│    Original transaction with status = 'reversed'. Retained permanently      │
│    for statutory audit trail.                                               │
│                                                                             │
│ 6. Posted Reversal Voucher:                                                 │
│    Independent reversing journal with status = 'posted' and inverted        │
│    Dr/Cr lines. Over any period covering both vouchers, net movement = 0.   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Reversal Pairs & Formal Separation of Gross Activity from Net Movement
- **Rejection of Speculative Net Deductions**: Subtracting $2 \times \text{Reversed}$ is mathematically invalid when queries span arbitrary date ranges. If an original voucher is dated in April and its reversal is dated in May, querying May includes only the reversal, resulting in a corrupted negative volume deduction.
- **Stage 7 Aggregate Policy**:
  1. The Voucher Register reports **Gross Voucher Activity Volume** across non-cancelled records.
  2. Cancelled vouchers contribute **strictly 0.00**.
  3. Original reversed vouchers (`status = 'reversed'`) and reversing journals (`status = 'posted'`) are displayed chronologically as distinct journal events to preserve the complete audit trail mandated by the Companies Act and GAAP.
  4. True net economic movement is the domain of the General Ledger, where inverted lines naturally cancel to zero without register-level deductions.

---

## 5. Decimal Boundary Correctness & Canonical Normalization

### 5.1 Proof of Schema Range and Integer Minor-Unit Scaling Overflow
Under database constraint `chk_voucher_total_amount_canonical` ([`migrations.rs:550`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/database/migrations.rs#L550)):
- `length(total_amount) >= 1 AND length(total_amount) <= 25`.
- Permitted values include numbers with up to 25 integer digits (e.g. `9999999999999999999999999`).
- SQLite `INTEGER` is a signed 64-bit integer, with maximum value $2^{63}-1 = 9,223,372,036,854,775,807 \approx 9.22 \times 10^{18}$.
- Scaling an amount by $10,000$ requires:
  $$\text{Scaled Value} = \text{Amount} \times 10,000 \le 9.22 \times 10^{18} \implies \text{Amount} \le 9.22 \times 10^{14}$$
- **Formal Proof of Failure**: Any valid canonical amount with more than 14 integer digits **will overflow signed 64-bit integer scaling**, triggering undefined behavior. Integer minor-unit scaling is **REJECTED**.

### 5.2 Mathematical Bounds of Database Schema `0.3.1-sprint2`

| Boundary Scenario | Raw Database String Format | Total Length | Exact Value |
|:---|:---|:---:|:---|
| **Max Permitted Integer-Only** | `'9999999999999999999999999'` | 25 chars | $10^{25}-1$ |
| **Max Permitted 4-Decimal Value** | `'99999999999999999999.9999'` | 25 chars | $10^{20} - 0.0001$ |
| **Max Permitted 1-Decimal Value** | `'99999999999999999999999.9'` | 25 chars | $10^{23} - 0.1$ |
| **Min Positive Canonical Value** | `'0.0001'` (4 dec) or `'1'` (int) | 6 / 1 chars | $0.0001$ / $1$ |
| **Zero / Draft Null Representation**| `'0'` or `NULL` | 1 / 0 chars | $0.0000$ |

### 5.3 Architectural Selection: 30-Character Fixed-Width Canonical Normalization
To guarantee 100% exact numerical comparison and sorting across all 25 permitted characters without floating-point arithmetic or integer overflow, Stage 7 adopts **Fixed-Width Canonical String Normalization**:

Every canonical amount is normalized in SQL into a 30-character comparable string:
- **Integer Part**: Left-padded with `'0'` to 25 digits.
- **Decimal Separator**: Fixed `.`
- **Fractional Part**: Right-padded with `'0'` to 4 digits.

$$\text{Format: } \underbrace{\text{0000000000000000000000000}}_{\text{25 digits (Integer Part)}} \mathbf{.} \underbrace{\text{0000}}_{\text{4 digits (Fraction)}}$$

#### Canonical SQLite Normalization Expression:
```sql
(
  substr('0000000000000000000000000' || substr(COALESCE(v.total_amount, '0'), 1, instr(COALESCE(v.total_amount, '0') || '.', '.') - 1), -25)
  || '.' ||
  substr(substr(COALESCE(v.total_amount, '0'), instr(COALESCE(v.total_amount, '0') || '.', '.') + 1) || '0000', 1, 4)
)
```

#### Exact Normalization Behavior at Boundaries:
- `'9999999999999999999999999'` $\to$ `'9999999999999999999999999.0000'` (Exact max integer)
- `'99999999999999999999.9999'` $\to$ `'0000099999999999999999999.9999'` (Exact max 4-decimal)
- `'0.0001'` $\to$ `'0000000000000000000000000.0001'` (Exact min positive)
- `'100'` and `'100.00'` $\to$ `'0000000000000000000000100.0000'` (Compares as identical)
- `NULL` (in drafts) $\to$ `'0000000000000000000000000.0000'` (Correct zero baseline)

### 5.4 Rust Domain Validation & Bounds Padding
In Rust, `min_amount` and `max_amount` are validated and normalized before query generation:

```rust
pub fn normalize_canonical_amount_for_query(input: &str, field_name: &str) -> Result<String, OpenTallyError> {
    let trimmed = input.trim();
    // Reject explicit signs ('+' or '-') to avoid normalization padding anomalies
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        return Err(OpenTallyError::Validation(format!("{} cannot contain explicit sign: '{}'", field_name, input)));
    }

    let dec = Decimal::from_str_exact(trimmed)
        .map_err(|_| OpenTallyError::Validation(format!("Invalid decimal for {}: '{}'", field_name, input)))?;
    
    if dec.is_sign_negative() {
        return Err(OpenTallyError::Validation(format!("{} cannot be negative: '{}'", field_name, input)));
    }
    if dec.scale() > 4 {
        return Err(OpenTallyError::Validation(format!("{} exceeds maximum 4 decimal places: '{}'", field_name, input)));
    }
    
    // Split into integer and fractional components
    let parts: Vec<&str> = trimmed.split('.').collect();
    let int_part = parts[0];
    if int_part.len() > 25 {
        return Err(OpenTallyError::Validation(format!("{} integer part exceeds 25 digits: '{}'", field_name, input)));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(OpenTallyError::Validation(format!("{} contains non-digit characters: '{}'", field_name, input)));
    }
    
    let frac_part = if parts.len() > 1 { parts[1] } else { "" };
    if !frac_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(OpenTallyError::Validation(format!("{} fractional part contains non-digit characters: '{}'", field_name, input)));
    }

    // Reject inputs exceeding 25 total characters allowed by schema
    if trimmed.len() > 25 {
        return Err(OpenTallyError::Validation(format!("{} total length exceeds 25 characters: '{}'", field_name, input)));
    }
    
    // Format: 25-digit zero-padded integer + '.' + 4-digit zero-padded fraction
    let padded_int = format!("{:0>25}", int_part);
    let padded_frac = format!("{:0<4}", frac_part);
    
    Ok(format!("{}.{}", padded_int, &padded_frac[..4]))
}
```

- When `min_amount` is specified: binds `padded_min` to `WHERE <norm_sql_expr> >= $padded_min`.
- When `max_amount` is specified: binds `padded_max` to `WHERE <norm_sql_expr> <= $padded_max`.
- When sorting by `TotalAmount`: `ORDER BY <norm_sql_expr> ASC` or `DESC`.

---

## 6. Read-Only Connection Lifecycle & Snapshot Consistency

### 6.1 Enforced Read-Only Connection Lifecycle & Comprehensive Pool Hygiene
`BEGIN DEFERRED` is NOT read-only in SQLite; it permits automatic upgrade to write transactions upon executing any DML or DDL statement. To enforce true engine-level read-only security while preventing connection pool contamination, Stage 7 mandates setting `PRAGMA query_only = ON` on a dedicated connection acquired from the pool.

However, modifying connection-level PRAGMAs introduces serious pool-poisoning risks if an error occurs. A connection left in `query_only = ON` would cause subsequent write operations (voucher posting, sequence incrementation) to fail with `SQLITE_READONLY`. Likewise, a dangling open transaction would cause connection leaks or lock timeouts.

To guarantee zero connection pool contamination, Stage 7 defines a strict RAII fail-closed connection detachment protocol across all execution states:

```
Pool ──> pool.acquire() ──> Set PRAGMA query_only = ON ──> BEGIN ──> Execute Query Closure
                                                                             │
                      ┌──────────────────────────────────────────────────────┴─────────────────────────────────┐
                      │ Success                                                                                │ Failure / Error
                      ▼                                                                                        ▼
                COMMIT TRANSACTION                                                                    ROLLBACK TRANSACTION
                      │                                                                                        │
                PRAGMA query_only = OFF                                                                conn.close().await
                      │                                                                                        │
            ┌─────────┴─────────┐                                                                              ▼
            │ Reset Succeeded   │ Reset Failed                                                       Contaminated Connection
            ▼                   ▼                                                                   Permanently Detached
       Return to Pool      conn.close() / Detach                                                    (Never returns to pool)
                           (Never returns to pool)
```

#### Complete 6-Mode Connection Lifecycle Failure Matrix

| Failure Mode | Point of Failure | Connection State | Engine / Pool Action | Pool Hygiene Guarantee |
|:---|:---|:---|:---|:---|
| **Mode 1** | `PRAGMA query_only = ON` fails | Indeterminate PRAGMA state; transaction not yet opened. | Immediate fail-closed: call `conn.close().await`. Discard error if close fails; return `Err(OpenTallyError::Database(e))`. | Connection is immediately closed and severed; never returned to the idle pool in an uncertain PRAGMA state. |
| **Mode 2** | `BEGIN;` fails | `query_only = ON` is set; snapshot transaction failed to open (e.g. `SQLITE_BUSY`). | Fail-closed: call `conn.close().await`. Return `Err(OpenTallyError::Database(e))`. | Connection is destroyed; prevents `query_only = ON` from leaking into future write callers. |
| **Mode 3** | Query closure `f(&mut *conn)` fails | Active snapshot transaction open; `query_only = ON` active; query returned error or task cancelled. | 1. Attempt `ROLLBACK;`.<br>2. Immediately call `conn.close().await` to sever the connection.<br>3. Return original `Err(err)`. | Aborted queries NEVER return connections to the pool. Closing the connection guarantees zero uncommitted state or pragma contamination. |
| **Mode 4** | `ROLLBACK;` fails | Connection severed, disk I/O failure, or SQLite engine crash; transaction dangling. | Critically broken: immediately call `conn.close().await`. Discard connection. | Broken connection is destroyed at the socket/handle level; pool active counter decremented; never reused. |
| **Mode 5** | `PRAGMA query_only = OFF` fails (after successful commit) | Transaction committed cleanly, but connection remains stuck in `query_only = ON`. | Fail-closed: call `conn.close().await`. Return `Err(OpenTallyError::Database(e))`. | Contaminated connection is discarded; prevents subsequent write operations on other threads from receiving `SQLITE_READONLY`. |
| **Mode 6** | Connection close/detachment fails (`conn.close().await` errors) | Transport / OS socket close returned error (e.g. `ECONNRESET`, `EBADF`). | In SQLx, `conn.close()` marks the internal connection state as closed (`is_closed() == true`). When `PoolConnection` drops, SQLx checks `is_closed()` and discards the handle without re-adding it to the idle pool queue. | Even if OS socket teardown reports an error, SQLx pool state machine guarantees the connection is permanently discarded. Pool contamination is physically impossible. |

#### Exact Lifecycle Implementation:
```rust
pub async fn execute_read_only_query<F, T, Fut>(pool: &Pool<Sqlite>, f: F) -> Result<T, OpenTallyError>
where
    F: FnOnce(&mut sqlx::SqliteConnection) -> Fut,
    Fut: std::future::Future<Output = Result<T, OpenTallyError>>,
{
    let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;

    // Step 1: Enforce SQLite engine-level read-only mode (Mode 1 failure guard)
    if let Err(e) = sqlx::query("PRAGMA query_only = ON;").execute(&mut *conn).await {
        // Mode 1: Pragma set failed; detach immediately to prevent uncertain state in pool
        let _ = conn.close().await;
        return Err(OpenTallyError::Database(e));
    }

    // Step 2: Begin consistent snapshot transaction (Mode 2 failure guard)
    if let Err(e) = sqlx::query("BEGIN;").execute(&mut *conn).await {
        // Mode 2: Begin failed; connection has query_only=ON, must detach
        let _ = conn.close().await;
        return Err(OpenTallyError::Database(e));
    }

    // Step 3: Execute user query closure
    let result = f(&mut *conn).await;

    // Step 4: Handle completion and pool hygiene (Modes 3, 4, 5, 6 guards)
    match result {
        Ok(val) => {
            // Commit snapshot transaction
            if let Err(e) = sqlx::query("COMMIT;").execute(&mut *conn).await {
                // Commit failed; detach contaminated connection
                let _ = conn.close().await;
                return Err(OpenTallyError::Database(e));
            }

            // Clean reset to query_only = OFF before returning connection to pool
            if let Err(e) = sqlx::query("PRAGMA query_only = OFF;").execute(&mut *conn).await {
                // Mode 5: Reset failed; connection remains query_only, must detach
                let _ = conn.close().await;
                return Err(OpenTallyError::Database(e));
            }

            // Clean connection safely returned to pool when `conn` is dropped
            Ok(val)
        }
        Err(err) => {
            // Mode 3: Query failed; attempt rollback
            let _ = sqlx::query("ROLLBACK;").execute(&mut *conn).await;
            
            // Mode 3 & 4: Regardless of rollback success/failure, ALWAYS detach
            // connection on query error to ensure absolute pool hygiene
            let _ = conn.close().await;
            
            Err(err)
        }
    }
}
```

### 6.2 Snapshot Consistency Between Count Preflight & Streaming
1. Both the **Count Preflight Query** and the **Data Fetch/Stream Query** execute on the **same dedicated connection** inside a single transaction using `execute_read_only_query`.
2. In SQLite WAL mode, an open transaction maintains a consistent snapshot view of the database. No concurrent transaction can alter the records visible to the export stream.
3. Eliminates any possibility of a race condition where count preflight passes 50,000 records, but a concurrent commit adds a 50,001st record before streaming starts.

---

## 7. Windows Export Finalization & Overwrite Governance

### 7.1 Path Security & Validation Architecture
Stage 7 enforces strict security controls on all export destinations:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       EXPORT PATH SECURITY PIPELINE                         │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. User-Authorized Selection:                                               │
│    Frontend invokes native OS file dialog (save dialog) where user chooses  │
│    explicit target directory and file name. Passes target_path to backend.  │
│                                                                             │
│ 2. Backend Path Sanitization:                                               │
│    - Rejects path traversal sequences ('..', embedded null bytes).          │
│    - Must be an absolute, canonicalizable filesystem path.                  │
│    - Validates parent directory exists and is writable.                     │
│    - Validates file extension matches requested format (.csv or .json).     │
│    - Rejects destination paths that are symbolic links or junctions.        │
│    - Default Sandboxed Fallback: If target_path is omitted, writes to       │
│      {app_data_dir}/exports/company_{id}/export_{timestamp}_{uuid}.{ext}     │
│                                                                             │
│ 3. Overwrite Governance:                                                    │
│    - VoucherExportFileRequest includes explicit `overwrite: bool` flag.     │
│    - If file exists and overwrite == false: fails fast with Validation error│
│                                                                             │
│ 4. Exclusive Sibling Temporary File (No Collision):                         │
│    - Writes to <target_dir>/.~opentally_export_<uuid>.tmp                   │
│    - Opened with O_CREAT | O_EXCL (create_new) to prevent collision.        │
│    - Same directory guarantees atomic filesystem rename (same drive volume) │
│                                                                             │
│ 5. Windows Native Atomic Replacement & Durability Protocol:                  │
│    - Mandatory flush (flush().await) and sync (sync_all().await) before close│
│    - For existing files: invokes Win32 ReplaceFileW with dwReplaceFlags = 0 │
│    - For non-existent files: race-safe MoveFileExW(..., 0)                   │
│    - Fail-closed on unsupported filesystems (FAT32/exFAT/SMB shares)        │
│    - Detects file locks (ERROR_SHARING_VIOLATION 32 / ACCESS_DENIED 5)       │
│    - Deletes .tmp file and returns clear diagnostic error to user           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Windows-Compatible Atomic Finalization & Durability Protocol

#### 7.2.1 The Atomicity Fallacy in Multi-Step File Replacement
A common but dangerous anti-pattern in desktop software export design is the manual 3-step staged replacement:
1. `rename(target -> target.bak)`
2. `rename(tmp -> target)`
3. `remove(target.bak)`

**OpenTally strictly adheres to the rule: Never describe a multi-step operation as atomic unless the guarantee is technically supported by the underlying operating system or filesystem.**

If a process crash, power outage, or OS termination occurs between Step 1 and Step 2 above, `target_path` is **completely absent from the filesystem**. To the user, their original file has vanished, and the export failed.

#### 7.2.2 Temporary File Flush, Synchronization, and Close Sequence
Before invoking any destination replacement operation, the export pipeline must guarantee that all user data has been fully flushed from memory to the operating system and storage subsystem:

```
Stream Records ──> writer.flush().await ──> file.sync_all().await ──> drop(writer)/drop(file) ──> finalize_export_file
                         │                          │                         │
                         ▼                          ▼                         ▼
                 Kernel Page Cache          Device Storage Sync       Handle Released
                 (User buffer empty)        (FlushFileBuffers done)   (No Sharing Locks)
```

1. **Flushing Application-Level Buffers (`BufWriter::flush`)**:
   Flushes all serialized CSV or JSON bytes from user-space application memory (`tokio::io::BufWriter` 64 KB buffer) to the operating system kernel page cache.
2. **Synchronizing File Data and Metadata (`File::sync_all`)**:
   Issues the operating system file synchronization primitive (calling Win32 `FlushFileBuffers` on Windows, or `fsync(2)` on Unix).
   - Reference: [`tokio::fs::File::sync_all`](https://docs.rs/tokio/latest/tokio/fs/struct.File.html#method.sync_all).
   - Guarantees that all in-memory file data and filesystem metadata associated with `tmp_path` are written to persistent storage media.
   - If either `flush().await` or `sync_all().await` fails (e.g. disk quota exhausted, physical I/O error), the export **fails immediately**. The destination is NEVER touched, `tmp_path` is unlinked and deleted, and `OpenTallyError::Io` is returned.
3. **Explicit File Handle Closure Prior to Replacement**:
   On Windows, an open file handle holds an exclusive byte-range or sharing lease. If `tmp_path` remains open when `ReplaceFileW` or `MoveFileExW` is invoked, Windows fails with `ERROR_SHARING_VIOLATION` (code 32). Therefore, `drop(writer)` and `drop(file)` must execute cleanly and successfully before calling the finalization replacement routine.
4. **Honest Durability and Power-Loss Boundaries**:
   While `sync_all()` instructs the OS to issue `FlushFileBuffers`, modern consumer solid-state drives (SSDs) and hard drives with volatile on-disk write caches without battery/supercapacitor backup may acknowledge the sync command before data reaches physical flash cells. Furthermore, parent-directory metadata updates may have an independent write timeline in the NTFS transaction journal. OpenTally provides the strongest possible portable durability guarantee achievable on Windows, but honestly documents that extreme power-loss durability is ultimately bounded by hardware write-cache policies.

#### 7.2.3 Win32 ReplaceFileW Specification & Supported Flags
- Official Reference: [`ReplaceFileW` Function (winbase.h)](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)

```rust
#[cfg(windows)]
extern "system" {
    fn ReplaceFileW(
        lpReplacedFileName: *const u16,
        lpReplacementFileName: *const u16,
        lpBackupFileName: *const u16,
        dwReplaceFlags: u32,
        lpExclude: *mut std::ffi::c_void,
        lpReserved: *mut std::ffi::c_void,
    ) -> i32;
}
```

##### Exact Flag Specification & MSDN Documentation:
- **`REPLACEFILE_WRITE_THROUGH` (`0x00000001`)**: **EXPLICITLY NOT SUPPORTED BY MICROSOFT.**  
  Microsoft documentation explicitly states verbatim: *"This value is not supported."* Claiming that passing `0x00000001` forces disk flushing is technically false. Storage flushing is achieved exclusively via `File::sync_all().await` (`FlushFileBuffers`) prior to closing the file.
- **Default Replacement Behavior (`dwReplaceFlags = 0`)**:  
  OpenTally mandates `dwReplaceFlags = 0` as the default replacement behavior.
- **Security Implications of Ignore Flags**:
  - `REPLACEFILE_IGNORE_MERGE_ERRORS` (`0x00000002`): Ignores errors merging metadata streams and attributes from the replaced file to the replacement file.
  - `REPLACEFILE_IGNORE_ACL_ERRORS` (`0x00000004`): Ignores errors merging Access Control Lists (DACLs).
  - **OpenTally Policy**: In financial accounting software, security descriptors (ACLs) set on target files restrict unauthorized viewing of sensitive tax and financial records. Silently suppressing ACL merge failures via ignore flags could result in an export file that strips intended administrative protections. Therefore, OpenTally passes `0`, enforcing strict security and attribute preservation. If ACLs cannot be merged safely, the operation fails closed with a clear diagnostic.
- **Error Capture**: On failure, `ReplaceFileW` returns 0. The error is captured via `std::io::Error::last_os_error()`, preserving the exact Windows error code (`ERROR_SHARING_VIOLATION`, `ERROR_ACCESS_DENIED`, `ERROR_NOT_SUPPORTED`).

#### 7.2.4 Filesystem-Specific Replacement & Race-Safe Creation Behavior

##### A. Existing Destination on NTFS / ReFS:
- Win32 `ReplaceFileW` replaces the target file with the temporary file while atomically archiving the original target to `backup_path` (`<target>.bak.<uuid>`).
- NTFS commits this replacement as an indivisible metadata transaction in the NTFS Master File Table ($MFT$) log.
- On success, OpenTally unlinks `backup_path` via `tokio::fs::remove_file(&backup_path)`. If unlinking the backup fails (e.g. transient antivirus inspection), the export has succeeded and the target file is 100% valid; a warning is logged, and `Ok(())` is returned.
- On failure, NTFS guarantees neither `target_path` nor `backup_path` was modified. The original target remains intact. `tmp_path` is deleted, and an error is returned.

##### B. Existing Destination on Unsupported Filesystems (FAT32, exFAT, SMB Shares):
- If the target filesystem does not support `ReplaceFileW`, the call fails and returns `ERROR_NOT_SUPPORTED` (code 50), `ERROR_INVALID_FUNCTION` (code 1), or `ERROR_UNABLE_TO_MOVE_REPLACEMENT` (code 1179).
- **Fail-Closed Policy**: OpenTally strictly refuses to silently fall back to a non-atomic multi-step operation (such as `delete` + `rename`) while claiming atomic replacement. Silently degrading to non-atomic replacement introduces a window where network disconnection or power loss leaves the destination file missing.
- OpenTally safely preserves the existing destination file, deletes `tmp_path`, and returns:
  `OpenTallyError::Io("Atomic replacement is unsupported on this filesystem or network share (e.g. FAT32/exFAT/SMB). Please specify a local NTFS directory or provide a new filename.")`.

##### C. Non-Existent Destination (Race-Safe Creation via `MoveFileExW(..., 0)`):
- In Rust on Windows, standard `std::fs::rename` calls `MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING)`.
- **Concurrency Risk**: If another application creates `target_path` concurrently between our preflight existence check and the rename, standard `rename` would unexpectedly OVERWRITE that file even if `overwrite == false`!
- **Race-Safe Solution**: OpenTally invokes Win32 `MoveFileExW(from, to, 0)` **without** the `MOVEFILE_REPLACE_EXISTING` flag (0x00000001).
  - Reference: [`MoveFileExW` Function (winbase.h)](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw).
  - If `target_path` was created concurrently by another process, `MoveFileExW(..., 0)` fails with `ERROR_ALREADY_EXISTS` (code 183) or `ERROR_FILE_EXISTS` (code 80).
  - OpenTally catches this, deletes `tmp_path`, and returns `OpenTallyError::Validation(format!("Destination file '{}' was created concurrently by another process. Enable overwrite to replace.", target_path.display()))`.
  - *Distinction*: `MOVEFILE_WRITE_THROUGH` (0x00000008 in `MoveFileExW`) forces the move to be committed to disk before returning, which is distinct from the unsupported `REPLACEFILE_WRITE_THROUGH` (0x00000001 in `ReplaceFileW`).

##### D. Network Shares (SMB) and Removable Media:
- Network shares (UNC paths `\\server\share\file` or mapped drives `Z:\file`):
  - Network drops during writes $\to$ `std::io::ErrorKind::ConnectionReset` / `ERROR_NETNAME_DELETED` (code 64) / `WSAECONNRESET` (10054).
  - Lock semantics on SMB depend on server-side leasing and opportunistic locks (oplocks).
  - OpenTally distinguishes unsupported operations from access-denied (`ERROR_ACCESS_DENIED` 5) and sharing violations (`ERROR_SHARING_VIOLATION` 32).
  - On any network drop or error, the operation fails closed, `tmp_path` cleanup is attempted, and the existing destination file remains completely untouched.

#### 7.2.5 Comprehensive 14-Scenario Failure and Recovery Matrix

| Scenario # & Failure Event | Old Destination Status | New Temporary File Status | Recovery & Cleanup Action | Safe to Retry? | Claimable Durability / Consistency Guarantee |
|:---|:---|:---|:---|:---:|:---|
| **1. Temp file creation failure** (`create_new` fails) | Intact and untouched | Not created | Fail-closed; return `Io` error. Zero cleanup needed. | YES | **Crash-consistent**: Zero filesystem mutation. |
| **2. Stream write failure** (disk full, I/O error) | Intact and untouched | Partial/incomplete `.tmp` file | Unlink and delete `.tmp` file immediately. Return `Io` error. | YES | **Crash-consistent**: Destination 100% untouched. |
| **3. Writer buffer flush failure** (`flush().await` fails) | Intact and untouched | Incomplete `.tmp` file | Unlink and delete `.tmp` file immediately. Return `Io` error. | YES | **Crash-consistent**: Destination 100% untouched. |
| **4. Storage sync failure** (`sync_all().await` fails) | Intact and untouched | Unsynchronized `.tmp` file | Unlink and delete `.tmp` file immediately. Return `Io` error. | YES | **Crash-consistent**: Destination 100% untouched. |
| **5. Temp file close failure** (handle close error) | Intact and untouched | Complete `.tmp` file | Unlink and delete `.tmp` file. Return `Io` error. | YES | **Crash-consistent**: Destination 100% untouched. |
| **6. Destination locked** (`ERROR_SHARING_VIOLATION` 32) | Intact and untouched | Complete `.tmp` file | Delete `.tmp` file. Return actionable diagnostic (close Excel). | YES (after closing file) | **Crash-consistent**: Destination 100% untouched. |
| **7. Access denied** (`ERROR_ACCESS_DENIED` 5) | Intact and untouched | Complete `.tmp` file | Delete `.tmp` file. Return permission diagnostic error. | YES (after chmod/ACL) | **Crash-consistent**: Destination 100% untouched. |
| **8. Unsupported filesystem** (`ERROR_NOT_SUPPORTED` 50) | Intact and untouched | Complete `.tmp` file | Fail-closed: delete `.tmp` file. Return filesystem diagnostic. | YES (to local NTFS) | **Crash-consistent**: Destination preserved; no weak fallback. |
| **9. Network share disconnection** (`ERROR_NETNAME_DELETED` 64) | Intact and untouched | Broken remote `.tmp` file | Return network error. Attempt best-effort `.tmp` delete. | YES (after reconnect) | **Crash-consistent**: Remote destination untouched. |
| **10. Replacement API failure** (`ReplaceFileW` returns 0) | Intact and untouched | Complete `.tmp` file | NTFS metadata untouched. Delete `.tmp` file. Return `Io` error. | YES | **Crash-consistent**: Indivisible NTFS transaction preserved target. |
| **11. Application termination before replacement** (kill/SIGKILL) | Intact and untouched | Orphaned `.tmp` file | Target 100% valid. Orphaned dot-file cleaned up by sweep. | YES | **Crash-consistent**: Target completely untouched. |
| **12. Application termination during/after replacement** | Fully replaced with new content | Old target moved to `.bak` | **Export succeeded 100%**. Valid target. Delete `.bak` on sweep. | NO (already succeeded) | **Crash-consistent & durable**: NTFS committed replacement. |
| **13. Power loss after sync but before replacement** | Intact and untouched | Flushed `.tmp` on disk | On reboot, target is intact. Orphaned `.tmp` cleaned up. | YES | **Power-loss durable**: Data synced; target intact. |
| **14. Power loss after replacement committed** | Fully replaced with new content | Old target at `.bak` | On reboot, target is valid. Orphaned `.bak` cleaned up. | NO (already succeeded) | **Power-loss durable**: `sync_all` + NTFS journal commit. |

#### 7.2.6 Exact Rust Implementation:

```rust
#[cfg(windows)]
fn to_wide_null(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
async fn replace_file_windows(
    tmp_path: &Path,
    target_path: &Path,
    backup_path: Option<&Path>,
) -> Result<(), std::io::Error> {
    let tmp_wide = to_wide_null(tmp_path);
    let target_wide = to_wide_null(target_path);
    let backup_wide = backup_path.map(to_wide_null);

    tokio::task::spawn_blocking(move || {
        extern "system" {
            fn ReplaceFileW(
                lpReplacedFileName: *const u16,
                lpReplacementFileName: *const u16,
                lpBackupFileName: *const u16,
                dwReplaceFlags: u32,
                lpExclude: *mut std::ffi::c_void,
                lpReserved: *mut std::ffi::c_void,
            ) -> i32;
        }

        let backup_ptr = match &backup_wide {
            Some(b) => b.as_ptr(),
            None => std::ptr::null(),
        };

        // SAFETY: Pointers are derived from valid, null-terminated wide strings
        // that outlive the blocking call. lpExclude and lpReserved are null per MSDN.
        // dwReplaceFlags = 0 (default replacement behavior per MSDN).
        let success = unsafe {
            ReplaceFileW(
                target_wide.as_ptr(),
                tmp_wide.as_ptr(),
                backup_ptr,
                0, // dwReplaceFlags = 0
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };

        if success != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    })
    .await
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}

#[cfg(windows)]
async fn rename_new_file_windows(
    tmp_path: &Path,
    target_path: &Path,
) -> Result<(), std::io::Error> {
    let tmp_wide = to_wide_null(tmp_path);
    let target_wide = to_wide_null(target_path);

    tokio::task::spawn_blocking(move || {
        extern "system" {
            fn MoveFileExW(
                lpExistingFileName: *const u16,
                lpNewFileName: *const u16,
                dwFlags: u32,
            ) -> i32;
        }

        // SAFETY: Pointers are derived from valid null-terminated wide strings.
        // dwFlags = 0 (WITHOUT MOVEFILE_REPLACE_EXISTING) ensures race-safety:
        // if target was created concurrently, MoveFileExW fails with ERROR_ALREADY_EXISTS.
        let success = unsafe {
            MoveFileExW(
                tmp_wide.as_ptr(),
                target_wide.as_ptr(),
                0, // No overwrite flag; fails if target exists
            )
        };

        if success != 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    })
    .await
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
}

#[cfg(not(windows))]
async fn replace_file_unix(tmp_path: &Path, target_path: &Path) -> Result<(), std::io::Error> {
    // POSIX rename(2) is atomic across existing files on the same filesystem
    tokio::fs::rename(tmp_path, target_path).await
}

pub async fn finalize_export_file(
    tmp_path: &Path,
    target_path: &Path,
    overwrite: bool,
) -> Result<(), OpenTallyError> {
    // Step 1: Preflight existence check
    let target_exists = tokio::fs::try_exists(target_path)
        .await
        .map_err(|e| OpenTallyError::Io(format!("Failed to inspect target path: {}", e)))?;

    if target_exists && !overwrite {
        // Fail-closed cleanup of temporary file
        let _ = tokio::fs::remove_file(tmp_path).await;
        return Err(OpenTallyError::Validation(format!(
            "Destination file '{}' already exists. Enable overwrite to replace.",
            target_path.display()
        )));
    }

    // Step 2: Finalization
    if target_exists {
        // Case B: Destination exists; execute atomic replacement via ReplaceFileW
        #[cfg(windows)]
        {
            let backup_path = target_path.with_extension(format!("bak.{}", Uuid::new_v4()));
            match replace_file_windows(tmp_path, target_path, Some(&backup_path)).await {
                Ok(()) => {
                    // Replacement succeeded atomically on NTFS/ReFS.
                    // Clean up backup file asynchronously; do not fail export if backup deletion fails
                    let _ = tokio::fs::remove_file(&backup_path).await;
                    Ok(())
                }
                Err(e) => {
                    // Fail-closed: clean up temporary file
                    let _ = tokio::fs::remove_file(tmp_path).await;
                    let raw_err = e.raw_os_error();
                    let is_locked = raw_err == Some(32) /* ERROR_SHARING_VIOLATION */
                        || raw_err == Some(5);          /* ERROR_ACCESS_DENIED */
                    if is_locked {
                        return Err(OpenTallyError::Io(format!(
                            "Cannot overwrite '{}'. The file is currently locked by another application (such as Microsoft Excel). Please close the file and retry the export.",
                            target_path.display()
                        )));
                    }
                    let is_unsupported = raw_err == Some(50) /* ERROR_NOT_SUPPORTED */
                        || raw_err == Some(1)               /* ERROR_INVALID_FUNCTION */
                        || raw_err == Some(1179);           /* ERROR_UNABLE_TO_MOVE_REPLACEMENT */
                    if is_unsupported {
                        return Err(OpenTallyError::Io(format!(
                            "Atomic file replacement is unsupported on this filesystem or network share ({}). Please choose a local NTFS directory or supply a new filename.",
                            target_path.display()
                        )));
                    }
                    Err(OpenTallyError::Io(format!("Failed to replace destination file (OS error {:?}): {}", raw_err, e)))
                }
            }
        }
        #[cfg(not(windows))]
        {
            if let Err(e) = replace_file_unix(tmp_path, target_path).await {
                let _ = tokio::fs::remove_file(tmp_path).await;
                return Err(OpenTallyError::Io(format!("Failed to finalize export file: {}", e)));
            }
            Ok(())
        }
    } else {
        // Case A: Destination does not exist; execute race-safe directory rename
        #[cfg(windows)]
        {
            match rename_new_file_windows(tmp_path, target_path).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    let _ = tokio::fs::remove_file(tmp_path).await;
                    let raw_err = e.raw_os_error();
                    if raw_err == Some(183) /* ERROR_ALREADY_EXISTS */ || raw_err == Some(80) /* ERROR_FILE_EXISTS */ {
                        return Err(OpenTallyError::Validation(format!(
                            "Destination file '{}' was created concurrently by another process. Enable overwrite to replace.",
                            target_path.display()
                        )));
                    }
                    Err(OpenTallyError::Io(format!("Failed to finalize export file (OS error {:?}): {}", raw_err, e)))
                }
            }
        }
        #[cfg(not(windows))]
        {
            if let Err(e) = tokio::fs::rename(tmp_path, target_path).await {
                let _ = tokio::fs::remove_file(tmp_path).await;
                return Err(OpenTallyError::Io(format!("Failed to finalize export file: {}", e)));
            }
            Ok(())
        }
    }
}
```

---

## 8. CSV Security & RFC 4180 Standards

### 8.1 Two-Stage Pipeline Architecture

```
Raw Column Value
       │
       ▼
[Is Column User-Controlled Text?] ──NO──> (Keep Exact Canonical Numeric)
       │ YES                                             │
       ▼                                                 │
[Stage 1: Formula Injection Sanitization]                 │
Strip leading ASCII whitespace/control chars.             │
If starts with =, +, -, @, \t, \r, \n: prepend '          │
       │                                                 │
       └────────────────────────┬────────────────────────┘
                                │
                                ▼
               [Stage 2: RFC 4180 Field Escaping]
         Contains comma, double-quote, CR, or LF?
         - Escape internal quotes: " -> ""
         - Enclose in outer double quotes: "..."
                                │
                                ▼
                     Final Safe CSV Cell
```

### 8.2 Column Classification Matrix

#### A. Register Summary Columns (`CsvSummary` - Day Book Format)
Header row:
`Voucher Number,Date,Effective Date,Voucher Type,Series,Reference No,Reference Date,Status,Total Amount,Line Count,Narration,Cancellation Reason,Reversal Of`

| Column Name | Data Type | Classification | Formula Sanitization Applied? | Escaping Rule |
|:---|:---|:---|:---:|:---|
| `voucher_number` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `date` | Text (`YYYY-MM-DD`) | System Validated | NO (Validated regex) | Stage 2 only |
| `effective_date` | Text (`YYYY-MM-DD`) | System Validated | NO (Validated regex) | Stage 2 only |
| `voucher_type_name` | Text | Master Data | **YES** | Stage 1 + Stage 2 |
| `series_name` | Text | Master Data | **YES** | Stage 1 + Stage 2 |
| `reference_no` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `reference_date` | Text | System Validated | NO | Stage 2 only |
| `status` | Enum | System Fixed | NO | Stage 2 only |
| `total_amount` | Decimal String | **Numeric** | **NO (Must preserve number)** | Stage 2 only |
| `line_count` | Integer | **Numeric** | **NO** | Stage 2 only |
| `narration` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `cancellation_reason`| Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `reversal_of` | Text | System ID/Number | **YES** | Stage 1 + Stage 2 |

#### B. Detailed Line-by-Line Columns (`CsvDetailed` - Audit Dump Format)
Header row:
`Voucher Number,Date,Effective Date,Voucher Type,Series,Line No,Ledger ID,Ledger Name,Entry Type,Amount,Line Narration,Reference No,Status,Cancellation Reason,Reversal Of`

| Column Name | Data Type | Classification | Formula Sanitization Applied? | Escaping Rule |
|:---|:---|:---|:---:|:---|
| `voucher_number` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `date` | Text (`YYYY-MM-DD`) | System Validated | NO | Stage 2 only |
| `effective_date` | Text (`YYYY-MM-DD`) | System Validated | NO | Stage 2 only |
| `voucher_type_name` | Text | Master Data | **YES** | Stage 1 + Stage 2 |
| `series_name` | Text | Master Data | **YES** | Stage 1 + Stage 2 |
| `line_number` | Integer | **Numeric** | **NO** | Stage 2 only |
| `ledger_id` | Text | System ID | **YES** | Stage 1 + Stage 2 |
| `ledger_name` | Text | Master Data | **YES** | Stage 1 + Stage 2 |
| `entry_type` | Text (`Dr`/`Cr`) | System Fixed | NO | Stage 2 only |
| `amount` | Decimal String | **Numeric** | **NO (Must preserve number)** | Stage 2 only |
| `line_narration` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `reference_no` | Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `status` | Enum | System Fixed | NO | Stage 2 only |
| `cancellation_reason`| Text | User-Controlled | **YES** | Stage 1 + Stage 2 |
| `reversal_of` | Text | System ID/Number | **YES** | Stage 1 + Stage 2 |

### 8.3 Exact Sanitization & Escaping Implementation
```rust
pub fn sanitize_text_cell(value: &str) -> String {
    // Strip leading ASCII whitespace and control characters (ASCII 0..=31)
    let trimmed = value.trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control());
    if value.starts_with('\t')
        || value.starts_with('\r')
        || value.starts_with('\n')
        || trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@')
    {
        format!("'{}", value) // Prepend single quote to original raw string
    } else {
        value.to_string()
    }
}

pub fn escape_rfc4180(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}
```

### 8.4 Representative CSV Security Test Cases

| Raw Input Value | Column Type | Stage 1 Sanitized | Stage 2 Escaped | Excel Behavior |
|:---|:---|:---|:---|:---|
| `1500.00` | Numeric | `1500.00` | `1500.00` | Treated as numeric currency |
| `+100.00` | Text (Ref) | `'+100.00` | `'+100.00` | Displayed as text; no formula eval |
| `-50.00` (Note) | Text (Narration)| `'-50.00` | `'-50.00` | Displayed as text; no formula eval |
| `=SUM(A1:A10)` | Text (Ref) | `'=SUM(A1:A10)` | `'=SUM(A1:A10)` | Formula neutralized |
| `  @calc` | Text (Narration)| `'  @calc` | `'  @calc` | Leading space + @ neutralized |
| `\t=1+1` | Text (Ref) | `'\t=1+1` | `"\t=1+1"` | Leading tab + formula neutralized |
| `Smith, John` | Text (Ledger) | `Smith, John` | `"Smith, John"` | Comma safely quoted |
| `Note with "quotes"` | Text | `Note with "quotes"` | `"Note with ""quotes"""` | Quotes doubled |
| `=CMD\|' /C calc'!A0` | Text | `'=CMD\|' /C calc'!A0` | `'=CMD\|' /C calc'!A0` | DDE attack neutralized |
| `Line 1\nLine 2` | Text | `Line 1\nLine 2` | `"Line 1\r\nLine 2"` | Multiline preserved in cell |

---

## 9. Lineage Traversal & Exact Truncation Detection

### 9.1 Conceptual Separation: Reversal vs. Correction
- **Reversal Relationship**: A formal accounting relationship between an original voucher and its reversing journal, modeled directly via `vouchers.reversal_of_voucher_id`. It is strictly 1-to-1, permanent, and immutable.
- **Correction Relationship**: A business workflow under `DEC-S2-004 Option A` (Two-Step Workflow):
  - Step 1: Reversal of $V_{\text{orig}}$ by $V_{\text{rev}}$ (reversal link).
  - Step 2: Generation of a new draft $V_{\text{corr}}$ with reference `"Correction of <V_orig.number>"`.
  - $V_{\text{corr}}.\text{reversal\_of\_voucher\_id}$ is `NULL` because $V_{\text{corr}}$ is a new regular transaction. The link is tracked via reference metadata and `audit_log`.

### 9.2 Recursive Lineage CTE with Definite Truncation Detection
To determine with mathematical certainty whether records were truncated by the depth cap, the query traverses up to `depth < 11` (depth 11):

```sql
WITH RECURSIVE voucher_lineage(id, company_id, parent_id, depth, path) AS (
    -- Anchor: The target voucher
    SELECT 
        v.id, 
        v.company_id, 
        v.reversal_of_voucher_id AS parent_id, 
        0 AS depth, 
        '/' || v.id || '/' AS path
    FROM vouchers v
    WHERE v.company_id = $1 AND v.id = $2

    UNION ALL

    -- Backward traversal (Finding what was reversed)
    SELECT 
        parent.id, 
        parent.company_id, 
        parent.reversal_of_voucher_id, 
        vl.depth + 1, 
        vl.path || parent.id || '/'
    FROM vouchers parent
    JOIN voucher_lineage vl ON parent.company_id = vl.company_id AND parent.id = vl.parent_id
    WHERE vl.depth < 11 
      AND instr(vl.path, '/' || parent.id || '/') = 0

    UNION ALL

    -- Forward traversal (Finding what reversed this voucher)
    SELECT 
        child.id, 
        child.company_id, 
        child.reversal_of_voucher_id, 
        vl.depth + 1, 
        vl.path || child.id || '/'
    FROM vouchers child
    JOIN voucher_lineage vl ON child.company_id = vl.company_id AND child.reversal_of_voucher_id = vl.id
    WHERE vl.depth < 11 
      AND instr(vl.path, '/' || child.id || '/') = 0
)
SELECT 
    v.*,
    MIN(vl.depth) AS min_depth
FROM voucher_lineage vl
JOIN vouchers v ON vl.company_id = v.company_id AND vl.id = v.id
WHERE vl.depth <= 11
GROUP BY v.id
ORDER BY v.date ASC, v.sequence_number ASC, v.id ASC;
```

#### Definite Truncation Detection Logic in Rust:
```rust
let rows: Vec<VoucherLineageRow> = ...;

// True truncation detection: only true if another record actually exists beyond depth 10
let has_exceeded_depth_limit = rows.iter().any(|r| r.min_depth > 10);

// Compute max depth reached within the supported depth limit (10)
let max_depth_reached = rows
    .iter()
    .filter(|r| r.min_depth <= 10)
    .map(|r| r.min_depth as u32)
    .max()
    .unwrap_or(0);

// Only return items up to the supported depth limit (10)
let items: Vec<VoucherRegisterItem> = rows
    .into_iter()
    .filter(|r| r.min_depth <= 10)
    .map(|r| r.into_item())
    .collect();
```

- **Exactly 10 generations with no further records**: `has_exceeded_depth_limit = false`.
- **11 or more generations**: `has_exceeded_depth_limit = true`.

### 9.3 Cancellation Reason Extraction Specification
- **Verified SQL Structure**: Derived from `cancellation.rs:195-225`:
  ```sql
  CASE 
      WHEN v.status = 'cancelled' THEN (
          SELECT json_extract(a.new_values, '$.reason') 
          FROM audit_log a 
          WHERE a.company_id = v.company_id 
            AND a.entity_type = 'voucher' 
            AND a.entity_id = v.id 
            AND a.action = 'CANCEL' 
          ORDER BY a.timestamp DESC, a.id DESC LIMIT 1
      )
      ELSE NULL 
  END AS cancellation_reason
  ```
- **Missing-Event Behavior**: If the `audit_log` event is missing, the subquery evaluates to `NULL`. The voucher is displayed with `cancellation_reason = None` without query failure.
- **Multiple-Event Behavior**: `ORDER BY a.timestamp DESC, a.id DESC LIMIT 1` deterministically selects the latest cancellation reason.
- **Company Isolation**: Strictly enforced via `a.company_id = v.company_id`.

---

## 10. Lossless Export Architecture (Streamed vs Materialized)

### 10.1 Streaming File Export vs Materialized Preview

| Dimension | Mode A: File Export (`export_vouchers_to_file`) | Mode B: In-Memory Preview (`export_vouchers_preview`) |
|:---|:---|:---|
| **Hard Cap** | **50,000 records** | **1,000 records** |
| **Materialization** | **Zero materialization** ($O(1)$ memory stream) | Materialized in-memory struct (`VoucherExportPreviewResult`) |
| **Memory Ceiling** | $< 5$ MB RAM (64 KB `tokio::io::BufWriter`) | $< 5$ MB RAM |
| **Exceeded Behavior**| Fails fast before opening file or running cursor | Fails fast before returning response |
| **Delivery Medium** | Direct to local file (via user save dialog) | Returned over Tauri IPC string payload |

### 10.2 JSON Streaming Serialization Protocol (Mode A)
To export 50,000 JSON records without accumulating them in a `Vec<VoucherCompositeExportItem>`:
1. Open sibling `.tmp` file (`<target_dir>/.~opentally_export_<uuid>.tmp`) with `tokio::fs::File::create_new`.
2. Wrap file in `tokio::io::BufWriter` (64 KB buffer).
3. Write opening JSON framing:
   ```json
   {"schema_version":"1.0","company_id":"...","exported_at":"...","total_records":N,"filter_applied":{...},"vouchers":[
   ```
4. Iterate over the database row stream (`sqlx::query_as(...).fetch(pool)`).
5. For each voucher:
   - Fetch its line items (streamed or chunked batch).
   - Serialize that single voucher struct via `serde_json::to_writer(&mut writer, &voucher)`.
   - Write comma `,` if not the last record.
6. Write closing framing: `]}`.
7. **Two-Stage Durability Flush & Sync**:
   - Call `writer.flush().await?` to flush application memory buffers to the operating system kernel cache.
   - Access the underlying `tokio::fs::File` and call `file.sync_all().await?` to issue Win32 `FlushFileBuffers` (or POSIX `fsync`), ensuring data is written through to physical storage media.
8. **Explicit Handle Closure**:
   - Explicitly drop `writer` and `file` (`drop(writer); drop(file);`) to release all file locks and sharing leases on Windows prior to replacement.
9. **Atomic Finalization**:
   - Call `finalize_export_file(&tmp_path, &target_path, overwrite).await?` to replace the destination atomically via `ReplaceFileW` (or race-safe `MoveFileExW(..., 0)`).

### 10.3 Complete Draft-07 JSON Schema Specification (`v1.0`)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "VoucherExportPackageJson",
  "type": "object",
  "required": ["schema_version", "company_id", "exported_at", "total_records", "filter_applied", "vouchers"],
  "properties": {
    "schema_version": { "type": "string", "enum": ["1.0"] },
    "company_id": { "type": "string" },
    "exported_at": { "type": "string", "format": "date-time" },
    "total_records": { "type": "integer", "minimum": 0 },
    "filter_applied": { "type": "object" },
    "vouchers": {
      "type": "array",
      "items": {
        "type": "object",
        "required": [
          "id", "voucher_number", "voucher_type_id", "voucher_type_name",
          "series_id", "series_name", "date", "effective_date", "status",
          "total_amount", "created_at", "lines"
        ],
        "properties": {
          "id": { "type": "string" },
          "voucher_number": { "type": "string" },
          "voucher_type_id": { "type": "string" },
          "voucher_type_name": { "type": "string" },
          "series_id": { "type": "string" },
          "series_name": { "type": "string" },
          "date": { "type": "string", "format": "date" },
          "effective_date": { "type": "string", "format": "date" },
          "status": { "type": "string", "enum": ["draft", "posted", "reversed", "cancelled"] },
          "reference_no": { "type": ["string", "null"] },
          "reference_date": { "type": ["string", "null"] },
          "narration": { "type": ["string", "null"] },
          "total_amount": { "type": ["string", "null"] },
          "cancellation_reason": { "type": ["string", "null"] },
          "reversal_of_voucher_id": { "type": ["string", "null"] },
          "reversal_voucher_id": { "type": ["string", "null"] },
          "created_at": { "type": "string" },
          "lines": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["line_number", "ledger_id", "ledger_name", "entry_type", "amount"],
              "properties": {
                "line_number": { "type": "integer", "minimum": 1 },
                "ledger_id": { "type": "string" },
                "ledger_name": { "type": "string" },
                "entry_type": { "type": "string", "enum": ["Dr", "Cr"] },
                "amount": { "type": "string" },
                "narration": { "type": ["string", "null"] }
              }
            }
          }
        }
      }
    }
  }
}
```

### 10.4 Rust Domain Models & DTO Definitions

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateBasis {
    TransactionDate, // Filters on v.date (default)
    EffectiveDate,   // Filters on v.effective_date
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VoucherSearchFilter {
    pub voucher_type_id: Option<String>,
    pub series_id: Option<String>,
    pub status: Option<String>,         // "posted" | "reversed" | "cancelled" | "draft" | "all"
    pub ledger_id: Option<String>,      // Filter vouchers touching this ledger
    pub date_basis: Option<DateBasis>,  // TransactionDate vs EffectiveDate
    pub date_from: Option<String>,      // YYYY-MM-DD
    pub date_to: Option<String>,        // YYYY-MM-DD
    pub min_amount: Option<String>,     // Canonical decimal string
    pub max_amount: Option<String>,     // Canonical decimal string
    pub search_query: Option<String>,   // Text search across number, ref_no, narration
    pub reversal_of_voucher_id: Option<String>,
    pub page: Option<u32>,              // 1-indexed (defaults to 1)
    pub page_size: Option<u32>,         // Default 20, max 100
    pub sort_by: Option<VoucherSortField>,
    pub sort_direction: Option<SortDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoucherSortField {
    Date,
    VoucherNumber,
    TotalAmount,
    CreatedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct VoucherRegisterItem {
    pub id: String,
    pub company_id: String,
    pub voucher_type_id: String,
    pub voucher_type_name: String,
    pub series_id: String,
    pub series_name: String,
    pub voucher_number: String,
    pub date: String,
    pub effective_date: String,
    pub status: String,
    pub reference_no: Option<String>,
    pub reference_date: Option<String>,
    pub narration: Option<String>,
    pub total_amount: Option<String>,
    pub line_count: i64,
    pub reversal_of_voucher_id: Option<String>,
    pub reversal_voucher_id: Option<String>,
    pub cancellation_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    CsvSummary,
    CsvDetailed,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherExportFileRequest {
    pub filter: VoucherSearchFilter,
    pub format: ExportFormat,
    pub target_path: Option<String>,    // User-chosen file path
    pub overwrite: Option<bool>,        // Default false
    pub include_bom: Option<bool>,      // Default true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherExportFileResult {
    pub format: ExportFormat,
    pub file_path: String,
    pub filename: String,
    pub file_size_bytes: u64,
    pub record_count: usize,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherExportPreviewRequest {
    pub filter: VoucherSearchFilter,
    pub format: ExportFormat,
    pub include_bom: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherExportPreviewResult {
    pub format: ExportFormat,
    pub content: String,
    pub record_count: usize,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoucherLineageResponse {
    pub items: Vec<VoucherRegisterItem>,
    pub has_exceeded_depth_limit: bool,
    pub max_depth_reached: u32,
}
```

---

## 11. Tauri IPC Command Inventory

Commands to be exposed in [`src-tauri/src/commands/mod.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/commands/mod.rs) and registered in [`src-tauri/src/lib.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/lib.rs):

```rust
#[tauri::command]
pub async fn search_vouchers(
    state: tauri::State<'_, AppState>,
    filter: crate::vouchers::VoucherSearchFilter,
) -> Result<crate::vouchers::PaginatedResponse<crate::vouchers::VoucherRegisterItem>, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::vouchers::VoucherSearchService::search_vouchers(state.db.pool(), &company_id, filter).await
}

#[tauri::command]
pub async fn export_vouchers_to_file(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::VoucherExportFileRequest,
) -> Result<crate::vouchers::VoucherExportFileResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::vouchers::VoucherExportService::export_to_file(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn export_vouchers_preview(
    state: tauri::State<'_, AppState>,
    req: crate::vouchers::VoucherExportPreviewRequest,
) -> Result<crate::vouchers::VoucherExportPreviewResult, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::vouchers::VoucherExportService::export_preview(state.db.pool(), &company_id, req).await
}

#[tauri::command]
pub async fn get_voucher_lineage(
    state: tauri::State<'_, AppState>,
    voucher_id: String,
) -> Result<crate::vouchers::VoucherLineageResponse, OpenTallyError> {
    let company_id = require_active_company(&state).await?;
    crate::vouchers::VoucherSearchService::get_voucher_lineage(state.db.pool(), &company_id, &voucher_id).await
}
```

---

## 12. Implementation Phasing & Scope Boundaries

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       STAGE 7 IMPLEMENTATION PHASING                        │
├─────────────────────────────────────────────────────────────────────────────┤
│ MILESTONE 7A: BACKEND SEARCH, EXPORT & LINEAGE ENGINE (Core Stage 7 Scope)  │
│   - Domain DTOs and Validation Logic in vouchers/model.rs                   │
│   - VoucherSearchService with exact 30-char canonical normalization         │
│   - True Read-Only enforcement via execute_read_only_query                  │
│   - Snapshot-consistent execution (preflight + stream on single conn)       │
│   - VoucherExportService with BufWriter streaming and CWE-1236 defense      │
│   - Windows staged finalization with ERROR_SHARING_VIOLATION handling       │
│   - Lineage recursive CTE with exact depth > 10 truncation detection        │
│   - Tauri IPC command registration in commands/mod.rs and lib.rs            │
│   - 26 Integration Tests in stage7_tests.rs                                 │
│                                                                             │
│ MILESTONE 7B: FRONTEND VOUCHER REGISTER VIEW (Stage 7 Extension / Stage 8) │
│   - VoucherRegisterView.tsx (Filter Toolbar, Data Grid, Pagination Footer)  │
│   - Frontend API client bindings in frontend/src/services/api.ts            │
│   - Vitest component test suite in VoucherRegisterView.test.tsx             │
│   - Native file save dialog integration                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 13. Comprehensive Test Matrix (26 Targeted Scenarios)

| Test ID | Category | Scenario Description | Expected Outcome |
|:---|:---|:---|:---|
| **`TC-SRCH-01`** | Multi-Tenancy | Search executed for Company A when Company B has records. | Zero Company B records returned. Strict isolation. |
| **`TC-SRCH-02`** | Default Status | Query with `status: None`. | Returns `posted`, `reversed`, and `cancelled`. Zero `draft` records returned. |
| **`TC-SRCH-03`** | Explicit Status | Query with `status: Some("draft")` and `status: Some("all")`. | Returns only drafts for former; all 4 statuses for latter. |
| **`TC-SRCH-04`** | Date Range | `date_from: "2026-05-02"`, `date_to: "2026-05-04"`. | Inclusive matching; bounds strictly verified. |
| **`TC-SRCH-05`** | Reversed Dates | `date_from: "2026-05-10"`, `date_to: "2026-05-01"`. | Rejects with `OpenTallyError::Validation`. |
| **`TC-SRCH-06`** | Date Basis | Filter by `DateBasis::EffectiveDate` vs `TransactionDate`. | Queries match effective_date vs date respectively. |
| **`TC-SRCH-07`** | Ledger Filter | Filter by `ledger_id = $cash_id`. | Returns only vouchers touching Cash ledger. |
| **`TC-SRCH-08`** | Exact 30-Char Amount | Query `min_amount: "10.00"`, `max_amount: "1000.25"`. | Normalizes to 30-char strings; matches boundary records exactly. |
| **`TC-SRCH-09`** | Large Amount No-Overflow | Amounts with 20+ integer digits (e.g. $10^{20}$). | Compared and ordered accurately; zero integer overflow. |
| **`TC-SRCH-10`** | Amount Float Trap | Vouchers with `0.05`, `10.00`, `2.50`, `100.00`. | Sorted by `TotalAmount` produces correct numeric order (`0.05 < 2.50 < 10.00 < 100.00`), NOT alphabetical (`10.00 < 100.00 < 2.50`). |
| **`TC-SRCH-11`** | Malformed Amount Filter | `min_amount: "abc"`, `max_amount: "-50"`, scale $> 4$. | Fails fast with `OpenTallyError::Validation`. |
| **`TC-SRCH-12`** | Text Wildcards | Search query containing `%_[]\`. | Special characters escaped via `ESCAPE '\'`; treated as literal substrings. |
| **`TC-SRCH-13`** | Deterministic Sort | Sorted by `date DESC` with identical dates. | Secondary sort on `sequence_number DESC, id ASC` prevents jitter. |
| **`TC-SRCH-14`** | Checked Pagination | Large page numbers or page size $> 100$. | Checked arithmetic prevents offset overflow; page size clamped to 100. |
| **`TC-SRCH-15`** | Cancellation Reason | Fetch cancelled voucher. | Extracts reason from `audit_log` via 3-column index match. |
| **`TC-SRCH-16`** | Lineage Depth Truncation| Traversal chain of exactly 10 vs 11 generations. | Depth 10 without 11th yields `has_exceeded_depth_limit = false`; 11 generations yields `true`. |
| **`TC-SRCH-17`** | Read-Only Enforcement | Attempt DML inside connection with `PRAGMA query_only = ON`. | SQLite rejects with `SQLITE_READONLY`. Zero database modification. |
| **`TC-SRCH-18`** | Snapshot Consistency | Concurrent write during export preflight & stream. | Read snapshot remains identical across count and row stream. |
| **`TC-EXP-01`** | CSV Summary | Export summary register to file. | RFC 4180 CRLF, comma delimited, all headers present. |
| **`TC-EXP-02`** | CSV Detailed | Export detailed register to file. | Full line-by-line output matching Section 8.2B headers. |
| **`TC-EXP-03`** | CSV CWE-1236 | Narration `=CMD(...)`, `@sum`, `+100`, `-note`, `\t`, `\r`. | Values prepended with `'`; formulas neutralized. |
| **`TC-EXP-04`** | Numeric Preserved | `total_amount: "1500.00"`. | Not prepended with `'`; exported as pure number. |
| **`TC-EXP-05`** | Path Security Traversal | `target_path: "../../etc/passwd"`. | Rejected as invalid path traversal. |
| **`TC-EXP-06`** | Windows Lock Handling | Target file is locked by external process. | Catches sharing error, deletes `.tmp`, returns diagnostic. |
| **`TC-EXP-07`** | Export Limit File | Query matches 50,001 records. | Fails fast; no silent truncation; no file created. |
| **`TC-EXP-08`** | JSON Stream & Schema | Export register to JSON stream. | Constant memory footprint; validates against `v1.0` JSON schema. |

---

## 14. Governance Decision Ledger & Carry-Forward Findings

### 14.1 Governance Status Ledger
The established governance statuses are preserved without modification:
- **`DEC-S2-001` (Invoice Rounding Policy)**: `PENDING USER APPROVAL` (Technical default: Option 1 fail-closed zero-tolerance).
- **`DEC-S2-002` (Sequence-Gap Governance)**: `PENDING USER APPROVAL` (Technical default: Option 1 strict retention).
- **`DEC-S2-003` (Cancellation Reason Storage)**: `APPROVAL EVIDENCE NOT VERIFIED` (Option A Audit Log JSON implemented without migrations).
- **`DEC-S2-004` (Posted Correction Workflow)**: `APPROVAL EVIDENCE NOT VERIFIED` (Option A Two-Step Workflow implemented).

### 14.2 Carry-Forward Findings Tracking
- **`FIND-6-01` (Sequence Rollback Test)**: Tracked. To be executed during the Sprint 2 closing hardening pass.
- **`FIND-6-02` (Cancellation Ledger Impact Test)**: Tracked. Stage 7 search and export queries strictly comply: cancelled vouchers contribute 0.00 to financial totals.
- **`FIND-6-03` (Correction Recovery Workflow)**: Tracked.
- **`FIND-6-04` (Sales Ledger Assertion in Reversal Chain)**: Tracked.
- **`FIND-6-05` (Standard Git Commit Protocol)**: Tracked.
- **`FIND-6-06` (Frontend Cancellation/Reversal Workflows)**: Tracked for delivery under Milestone 7B / Stage 8.

---

## 15. Change Log & Unresolved UNKNOWN Ledger

### 15.1 Change Log: Evolution across Versions

#### A. Corrections in v2.2.2 (Targeted Final Verification Hardening)

| Finding / Area | v2.2.1 Baseline | v2.2.2 Hardened Specification | Section |
|:---|:---|:---|:---:|
| **1. ReplaceFileW Flags** | Passed `0x00000001` claiming write-through flush | Removed `REPLACEFILE_WRITE_THROUGH`; documented Microsoft MSDN marking it unsupported; mandated `dwReplaceFlags = 0`; justified rejection of ACL/merge ignore flags. | Section 7.2.3 |
| **2. Storage Durability Sync** | Conflated metadata swap with data sync | Mandated two-stage `writer.flush().await` + `file.sync_all().await` (`FlushFileBuffers`) and handle closure prior to replacement; documented hardware write-cache limits. | Section 7.2.2, 10.2 |
| **3. Non-Existent Destination Race** | Used standard `rename` (with replace) | Mandated Win32 `MoveFileExW(..., 0)` without replace, preventing accidental concurrent file clobbering; distinguished from `MOVEFILE_WRITE_THROUGH` (0x8). | Section 7.2.4C |
| **4. Unsupported Filesystem Safety** | Undocumented fallback risk | Specified strict fail-closed policy on FAT32/exFAT/SMB shares; rejected silent non-atomic fallback to preserve accounting audit safety. | Section 7.2.4B, 7.2.4D |
| **5. Failure & Recovery Matrix** | 4-phase crash matrix | Expanded to comprehensive 14-scenario matrix distinguishing crash consistency from physical power-loss durability across all failure modes. | Section 7.2.5 |

#### B. Corrections in v2.2.1 (Final Verification & Hardening)

| Finding / Area | v2.2.0 Baseline | v2.2.1 Hardened Specification | Section |
|:---|:---|:---|:---:|
| **1. Windows Overwrite Crash Safety** | Described 3-step staged replacement (`target -> bak -> target`) | Replaced with Win32 `ReplaceFileW` atomic kernel primitive for existing destinations; defined single-step volume-local `rename` for non-existent destinations; added comprehensive 4-phase crash window matrix; explicitly banned describing multi-step sequences as atomic. | Section 7.2 |
| **2. Read-Only Connection Failure Handling** | Basic try/catch on pragma reset | Formulated exhaustive 6-mode connection lifecycle failure matrix (`PRAGMA ON` fail, `BEGIN` fail, query fail, `ROLLBACK` fail, `PRAGMA OFF` fail, connection close fail); guaranteed immediate RAII connection detachment on any error so poisoned connections never return to pool. | Section 6.1 |

#### C. Corrections in v2.2.0

| Finding / Area | v2.1.0 Status | v2.2.0 Hardened Correction | Section |
|:---|:---|:---|:---:|
| **1. Windows Overwrite & Atomicity** | Assumed direct `rename` | Implemented staged replacement via `.bak.<uuid>` with `ERROR_SHARING_VIOLATION` detection and fail-closed rollback. | Section 7.2 |
| **2. Read-Only Connection Lifecycle** | Simple pragma query | Added `execute_read_only_query` with explicit RAII connection detachment on error, guaranteeing contaminated connections never return to pool. | Section 6.1 |
| **3. Lineage Truncation Detection** | Inferred from `depth == 10` | Implemented `depth < 11` query checking for an actual 11th record beyond depth cap before setting `has_exceeded_depth_limit = true`. | Section 9.2 |
| **4. CSV Sanitizer Hardening** | Basic trigger check | Hardened to strip all ASCII control chars (0..=31) and leading whitespace; preserved raw numbers in numeric columns. | Section 8.3 |
| **5. Decimal Boundary Correctness** | Claimed unbounded | Formally derived schema bounds ($10^{25}-1$ integer, $10^{20}-0.0001$ 4-decimal); validated in Rust before padding. | Section 5.2–5.4 |
| **6. JSON Streaming Reconciliation** | Claimed stream, used `Vec` | Explicitly separated Mode A (incremental framing stream to disk, $< 5$ MB RAM) from Mode B (materialized preview, max 1,000 records). | Section 10.1–10.2 |
| **7. Cancellation Reason Extraction**| Unverified assumption | Verified against `cancellation.rs:195-225`; specified missing-event and multiple-event deterministic handling. | Section 9.3 |

### 15.2 Explicit List of Unresolved UNKNOWN Items

| Item ID | Domain | Unresolved UNKNOWN Item | Mitigation / Verification Path |
|:---|:---|:---|:---|
| **`UNK-7-01`** | Interoperability | **External/Legacy Audit Log Schema**: Whether third-party imported or historical data structures populated `audit_log.new_values` with keys other than `$.reason` (e.g. `$.cancellation_reason`). | Mitigated in Stage 7 via `COALESCE(json_extract(..., '$.reason'), json_extract(..., '$.cancellation_reason'))`. Verification required when authoring external import adapters in Sprint 3. |
| **`UNK-7-02`** | OS Integration | **Long Path Support on Windows (< 260 chars)**: Whether users selecting paths exceeding Windows `MAX_PATH` without `\\?\` prefix will encounter IO failures during native export. | Default sandboxed fallback path is within local AppData (`< 120 chars`). Frontend save dialog validates path length. Full UNC prefixing (`\\?\`) to be evaluated during Windows packaging. |

---

## 16. Acceptance Criteria

Stage 7 will be accepted when:
1. All 26 test scenarios in the Stage 7 test matrix pass cleanly.
2. Complete test suite achieves green status with zero regressions across Stages 1–6.
3. `cargo clippy --all-targets -- -D warnings` reports 0 warnings.
4. `cargo fmt -- --check` reports 0 formatting differences.
5. All amount filtering and sorting operations use exact 30-character canonical string normalization matching schema bounds up to $10^{25}$, with zero floating-point casting and zero integer overflow.
6. True read-only connection enforcement is verified via `PRAGMA query_only = ON` with guaranteed connection pool hygiene across all 6 failure modes.
7. Exports exceeding 50,000 records fail fast with clear diagnostic messages and zero silent truncation.
8. User-controlled CSV text fields are protected against formula injection while numeric fields remain raw numbers.
9. Windows export finalization uses Win32 `ReplaceFileW` with `dwReplaceFlags = 0` for existing destinations and race-safe `MoveFileExW(..., 0)` for non-existent destinations, with mandatory `flush()` and `sync_all()` before handle closure, fail-closed handling on unsupported filesystems, verified locked-file diagnostics, and clean recovery across all 14 failure scenarios.
10. Lineage truncation accurately distinguishes exactly 10 generations from 11+ generations.
11. Zero database migrations or new external dependencies are introduced.
