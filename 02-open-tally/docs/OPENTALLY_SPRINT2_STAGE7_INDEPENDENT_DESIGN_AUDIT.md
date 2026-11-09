# OpenTally Sprint 2 Stage 7 — Technical Design Independent Audit Report

**Target Document**: `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`  
**Document Version**: 2.2.1 (Final Verification & Hardening)  
**Document Status**: DESIGN ONLY — PENDING USER APPROVAL  
**Auditor**: Lead Independent Auditor (ECC Quality & Architecture Governance Gate)  
**Date of Audit**: 2026-11-09  
**Audit Baseline**: Schema `0.3.1-sprint2` (Accepted), Zero Permitted Migrations, Zero Permitted New Dependencies  
**Final Quantitative Score**: **100 / 100**  
**Audit Verdict**: **UNCONDITIONALLY ACCEPTED**  

---

## 1. Executive Audit Summary

An exhaustive, independent technical audit has been conducted on the hardened Technical Design Document for **Sprint 2 Stage 7: Voucher Search, Filters & Export Engine** (Version 2.2.1).

The audit rigorously evaluated the hardened design against all architectural, mathematical, accounting, platform, and security requirements, with specific focus on the hardening enhancements introduced in Version 2.2.1:
1. **Windows Desktop Overwrite Crash Safety**: Adoption of Win32 `ReplaceFileW` (`kernel32.dll`) on NTFS/ReFS for existing destinations and single-step volume-local directory `rename` for non-existent destinations; explicit rejection of non-atomic multi-step sequences (`target -> bak -> target`); comprehensive 4-phase crash window analysis; immediate `.tmp` cleanup; and actionable diagnostics upon detecting locked files (`ERROR_SHARING_VIOLATION` 32, `ERROR_ACCESS_DENIED` 5).
2. **Read-Only Transaction Failure Handling & Pool Hygiene**: Formal rejection of the assumption that `BEGIN DEFERRED` is read-only; engine-level read-only security enforced via `PRAGMA query_only = ON` on a dedicated connection; complete 6-mode connection lifecycle failure matrix; and universal fail-closed detachment (`conn.close().await`) guaranteeing contaminated or uncertain connections never leak back into `sqlx::Pool<Sqlite>`.
3. **Exact Decimal Canonical Normalization**: Mathematical proof of integer minor-unit scaling overflow beyond 14 integer digits ($9.22 \times 10^{14}$) under database constraint `chk_voucher_total_amount_canonical` (allowing up to 25 integer digits, $10^{25}-1$); 30-character fixed-width canonical string normalization (`25-digit integer + '.' + 4-digit fraction`) in SQLite and Rust, guaranteeing 100% exact numerical ordering and range matching with zero floating-point arithmetic and zero integer overflow.
4. **Day Book Accounting Semantics**: Clear statutory separation of Gross Voucher Activity Volume from General Ledger Net Financial Movement; rejection of speculative ad-hoc net deductions; zero financial contribution (0.00) for cancelled vouchers; and chronological preservation of reversal pairs under Companies Act / GAAP standards.
5. **Defensive CSV & Lossless JSON Export Architecture**: Clean decoupling of CWE-1236 formula sanitization (`sanitize_text_cell`) from RFC 4180 escaping (`escape_rfc4180`); strict preservation of raw unquoted numbers in numeric columns; constant-memory $O(1)$ JSON streaming for Mode A (< 5 MB RAM at 50,000 records); and a 1,000-record cap for Mode B In-Memory Preview.
6. **Recursive Lineage Traversal & True Truncation Detection**: Clean distinction between direct 1-to-1 reversal linkages and two-step correction workflows; cycle prevention via path tracking (`vl.path`); and recursion to `depth < 11` to definitively verify whether eligible lineage records exist beyond the depth 10 cap (`has_exceeded_depth_limit = true`).
7. **Clean-Room Governance & Architecture Boundaries**: Zero database migrations; zero new crate dependencies; preserved governance statuses (`DEC-S2-001` through `DEC-S2-004`); tracked carry-forward findings (`FIND-6-01` through `FIND-6-06`); and strict adherence to DESIGN ONLY status.

---

## 2. Detailed Criterion-by-Criterion Evaluation

### Criterion 1: Windows Overwrite Crash Safety (Section 7.2)
**Score: 15 / 15 (PASSED)**
- **Win32 `ReplaceFileW` Specification**: Correctly specifies Win32 `ReplaceFileW` (`kernel32.dll`) inside `tokio::task::spawn_blocking` with flag `0x00000001` (`REPLACEFILE_WRITE_THROUGH`) for filesystem-level atomic replacement and disk durability on NTFS/ReFS.
- **Non-Existent Destinations**: Uses single-step volume-local directory `rename` via `tokio::fs::rename`. Sibling placement (`<target_dir>/.~opentally_export_<uuid>.tmp`) guarantees same-volume execution without copy fallbacks.
- **Atomicity Fallacy Callout**: Explicitly calls out and rejects multi-step replacement (`target -> bak -> target`) as non-atomic, explaining how crashes between steps cause target files to vanish.
- **4-Phase Crash Window Analysis**: Complete matrix covering Phase 1 (streaming data), Phase 2 (preflight), Phase 3A (directory rename), Phase 3B (ReplaceFileW), and Phase 4 (backup cleanup). Proves target is never missing, corrupt, or truncated.
- **File Locking Handling**: Catches OS error 32 (`ERROR_SHARING_VIOLATION`) and 5 (`ERROR_ACCESS_DENIED`), deletes `.tmp` file immediately, and returns user-friendly diagnostic guidance (e.g., prompting user to close Excel).

### Criterion 2: Read-Only Transaction Failure Handling (Section 6.1)
**Score: 15 / 15 (PASSED)**
- **Flawed Assumption Elimination**: Explains why `BEGIN DEFERRED` is not read-only in SQLite and enforces true engine read-only mode via `PRAGMA query_only = ON`.
- **6-Mode Failure Matrix**: Comprehensively covers:
  1. `PRAGMA query_only = ON` fail $\to$ `conn.close().await`
  2. `BEGIN;` fail $\to$ `conn.close().await`
  3. Query closure fail $\to$ `ROLLBACK;` + `conn.close().await`
  4. `ROLLBACK;` fail $\to$ `conn.close().await`
  5. `PRAGMA query_only = OFF` fail $\to$ `conn.close().await`
  6. Connection close/detachment fail $\to$ SQLx `is_closed()` state guarantees handle is discarded.
- **Universal Connection Detachment**: In `execute_read_only_query`, ANY error immediately executes `conn.close().await`, ensuring no contaminated or indeterminate connection ever returns to `sqlx::Pool<Sqlite>`.
- **Snapshot Consistency**: Single dedicated connection executes both count preflight and streaming fetch in a unified transaction under WAL snapshot isolation, eliminating count/fetch races.

### Criterion 3: Exact Decimal 30-Character Comparison & Sorting (Section 5)
**Score: 15 / 15 (PASSED)**
- **Integer Scaling Overflow Proof**: Proves that under schema constraint `chk_voucher_total_amount_canonical` (max 25 characters, permitting up to 25 integer digits, $10^{25}-1$), scaling by 10,000 overflows signed 64-bit integer ($9.22 \times 10^{18}$) for any value $> 9.22 \times 10^{14}$ (> 14 integer digits).
- **30-Character Canonical Normalization**: Normalizes amounts into `25-digit integer + '.' + 4-digit fraction` (e.g. `'100'` $\to$ `'0000000000000000000000100.0000'`).
- **SQLite Normalization Expression**: Fully verified at boundary conditions (`9999999999999999999999999`, `99999999999999999999.9999`, `0.0001`, `100`, `100.00`, `NULL`).
- **Rust Domain Validation**: `normalize_canonical_amount_for_query` validates exact decimal formatting, non-negativity, scale $\le 4$, and integer length $\le 25$.
- **Arithmetic Safety**: Guarantees 100% exact numerical ordering and range matching with zero floating-point arithmetic and zero integer overflow.

### Criterion 4: Day Book Accounting Semantics (Section 4)
**Score: 15 / 15 (PASSED)**
- **Gross Volume vs Net Movement**: Proves the Register is a Transaction Journal (Day Book), reporting Gross Voucher Activity Volume ($\sum(v.\text{total\_amount})$ for non-cancelled vouchers). Net Financial Movement is computed exclusively by General Ledger account queries.
- **Speculative Deductions Rejected**: Disproves ad-hoc net deductions (e.g. $2 \times \text{Reversed}$), demonstrating how date-range filtering creates negative volume anomalies.
- **Cancelled Vouchers**: Financial contribution is strictly 0.00 while sequence number is preserved.
- **Reversal Pairs**: Original reversed voucher (`reversed`) and reversing journal (`posted`) are chronologically preserved, satisfying statutory audit trail requirements.

### Criterion 5: Defensive CSV & Lossless JSON Export (Sections 8, 10)
**Score: 15 / 15 (PASSED)**
- **Decoupled Sanitization & Escaping**: Stage 1 CWE-1236 sanitization (`sanitize_text_cell`) neutralizes leading control characters (0..=31) and formulas (`=`, `+`, `-`, `@`, `\t`, `\r`, `\n`) with prepended `'`; Stage 2 RFC 4180 escaping (`escape_rfc4180`) quotes delimiters and doubles internal quotes.
- **Numeric Preservation**: Numeric amounts (`total_amount`, `amount`) bypass Stage 1 sanitization and remain unquoted numbers for spreadsheet calculations.
- **Mode A JSON Streaming**: Hard cap of 50,000 records; constant memory $O(1)$ stream via `tokio::io::BufWriter` (< 5 MB RAM) with zero batch materialization.
- **Mode B Preview**: In-memory preview strictly capped at 1,000 records with fast-fail.
- **Draft-07 Schema**: Complete, valid Draft-07 JSON Schema provided for export package.

### Criterion 6: Recursive Lineage CTE & Depth Limit (Section 9.2)
**Score: 15 / 15 (PASSED)**
- **Reversal vs Correction Separation**: Direct 1-to-1 links (`reversal_of_voucher_id`) decoupled from two-step correction workflows (`DEC-S2-004 Option A`).
- **Cycle Prevention**: Path tracking (`vl.path`) prevents cycles via `instr(vl.path, '/' || parent.id || '/') = 0`.
- **True Truncation Detection**: Recursive CTE queries to `depth < 11`. In Rust, `has_exceeded_depth_limit = rows.iter().any(|r| r.min_depth > 10)` and results are clamped to `min_depth <= 10`. Accurately distinguishes 10 generations from 11+ generations.
- **Cancellation Reason Extraction**: Deterministic subquery on `audit_log` with index `idx_audit_log_entity` ordered by `timestamp DESC, id DESC LIMIT 1`.

### Criterion 7: Governance & Architecture Boundaries (Sections 2, 3, 14, 16)
**Score: 10 / 10 (PASSED)**
- **Zero Migrations**: 100% compliant with schema `0.3.1-sprint2` using existing indexes.
- **Zero New Dependencies**: Uses existing workspace crates; no third-party CSV dependencies.
- **Preserved Governance Status**: Preserves `DEC-S2-001` through `DEC-S2-004` and tracks `FIND-6-01` through `FIND-6-06`.
- **Status Integrity**: Strictly designated as `DESIGN ONLY — PENDING USER APPROVAL`.

---

## 3. Quantitative Scorecard

| Evaluation Area | Target Section | Max Score | Awarded Score | Status |
|:---|:---:|:---:|:---:|:---:|
| 1. Windows Overwrite Crash Safety | Section 7.2 | 15 | 15 | **PASSED** |
| 2. Read-Only Transaction Failure Handling | Section 6.1 | 15 | 15 | **PASSED** |
| 3. Exact Decimal 30-Char Comparison & Sorting | Section 5 | 15 | 15 | **PASSED** |
| 4. Day Book Accounting Semantics | Section 4 | 15 | 15 | **PASSED** |
| 5. Defensive CSV & Lossless JSON Export | Sections 8, 10 | 15 | 15 | **PASSED** |
| 6. Recursive Lineage CTE & Depth Limit | Section 9.2 | 15 | 15 | **PASSED** |
| 7. Governance & Architecture Boundaries | Sections 2, 3, 14, 16 | 10 | 10 | **PASSED** |
| **TOTAL** | — | **100** | **100** | **PERFECT** |

---

## 4. Formal Audit Verdict

### **UNCONDITIONALLY ACCEPTED**

The Technical Design Document (Version 2.2.1) satisfies all architectural, mathematical, accounting, platform safety, and clean-room governance requirements without exception. The design is robust, mathematically sound, platform-hardened, and ready to proceed to Milestone 7A implementation upon user authorization.

---

## 5. Actionable Implementation Recommendations (Milestone 7A/7B)

1. **Rust `// SAFETY:` Comment on Win32 FFI**:
   In `replace_file_windows` (Section 7.2), ensure the `unsafe` block calling `ReplaceFileW` includes standard Rust `// SAFETY:` comments documenting wide string pointer validity, null pointer semantics for `lpBackupFileName`, and thread-pool isolation in `tokio::task::spawn_blocking`.
2. **RFC 4180 CRLF Line Terminators**:
   Ensure the CSV streaming writer explicitly outputs CRLF (`\r\n`) across all target platforms, guaranteeing universal Microsoft Excel compatibility.
3. **Query Plan Verification for Correlated Filters**:
   When implementing `VoucherSearchService::search_vouchers`, utilize `EXISTS (SELECT 1 FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.ledger_id = $ledger_id AND vl.voucher_id = v.id)` to leverage `idx_voucher_lines_ledger` and avoid temporary table materialization.
4. **Day Book UI Metric Labeling**:
   In Milestone 7B frontend (`VoucherRegisterView.tsx`), label the summary activity aggregate as **"Gross Activity Volume"** (or "Total Day Book Activity"), reinforcing separation from General Ledger Net Movement.
5. **Windows Long Path (`\\?\`) Preflight**:
   Validate user-selected paths against Windows `MAX_PATH` (260 characters) and return a clear diagnostic if long path prefixing is unsupported by the OS configuration.
