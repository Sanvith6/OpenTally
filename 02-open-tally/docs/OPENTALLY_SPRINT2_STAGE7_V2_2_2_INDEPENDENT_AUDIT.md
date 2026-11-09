# OpenTally Sprint 2 Stage 7 — Independent Technical Audit Report (v2.2.2)
## Voucher Search, Filters & Export Engine

**Audit Target**: `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md` (Version 2.2.2)  
**Correction Report**: `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_V2_2_2_CORRECTION_REPORT.md`  
**Target Repository**: `02-open-tally`  
**Audit Date**: 2026-11-09  
**Auditing Authority**: Lead Independent Auditor for OpenTally Sprint 2  
**Audit Scope**: Strict Independent Technical Verification of v2.2.2 Hardened Design  
**Execution Boundary**: **STRICTLY DESIGN AUDIT ONLY**  
*(Zero application code changes, zero migrations, zero dependency modifications)*  

---

## 1. Executive Audit Summary

Following the previous verification cycle which rendered a verdict of **NOT READY** against version 2.2.1 due to an unsupported Win32 API flag (`REPLACEFILE_WRITE_THROUGH`) and conflation of NTFS metadata crash consistency with physical drive payload durability, this independent technical audit conducted an exhaustive re-examination of the corrected design document (**Version 2.2.2**).

The audit confirms that all identified deficiencies have been rigorously and accurately remediated. The design now adheres strictly to official Microsoft Win32 API specifications, enforces an uncompromising five-stage flush/sync/close/replace pipeline, provides race-safe file creation for non-existent destinations, enforces fail-closed safety on non-NTFS volumes and network shares, and provides an honest, comprehensive 14-scenario failure and recovery matrix distinguishing software crash consistency from physical NAND write-cache durability.

### Formal Verdict: **READY WITH LOW-RISK OBSERVATIONS**

| Severity Level | Count | Summary |
|:---|:---:|:---|
| **CRITICAL** | 0 | Zero security vulnerabilities, data corruption paths, or unsafe memory patterns. |
| **HIGH (Blocker)** | 0 | The previous blocker regarding `REPLACEFILE_WRITE_THROUGH` is completely eliminated. |
| **MEDIUM** | 0 | Missing file data sync (`sync_all`) is fully resolved and properly ordered before handle drop. |
| **LOW-RISK OBSERVATIONS** | 3 | Minor implementation-phase ergonomic and testing observations (BufWriter sync idiomatic pattern, Unix CI fallback, orphaned dot-file startup sweep). |
| **INFORMATIONAL** | 1 | Windows Long Path (`MAX_PATH`) tracking under `UNK-7-02` reaffirmed for packaging phase. |

### Key Audit Highlights:
1. **Unsupported Flag Eradication**: `REPLACEFILE_WRITE_THROUGH` (0x01) has been completely removed from active code and parameter listings. MSDN documentation marking it unsupported is accurately cited verbatim. `dwReplaceFlags = 0` is established as the authoritative default.
2. **Fail-Closed Security on Windows ACLs**: The design explicitly evaluates and rejects `REPLACEFILE_IGNORE_MERGE_ERRORS` (0x02) and `REPLACEFILE_IGNORE_ACL_ERRORS` (0x04), proving that suppressing ACL merge failures in financial accounting software risks exposing confidential records.
3. **Flawless Five-Stage Durability Ordering**: The sequence of Application Buffer Flush -> Kernel Storage Sync (`FlushFileBuffers`) -> Explicit Handle Closure -> Atomic Finalization is strictly documented and enforced.
4. **Race-Safe Creation**: For non-existent destinations, `MoveFileExW(..., 0)` without replace eliminates the file clobbering race condition inherent in standard `rename`.
5. **Fail-Closed Volume Policy**: The system strictly refuses to silently degrade to non-atomic multi-step operations (`delete` + `rename`) on FAT32, exFAT, or SMB shares, preserving destination files on error.
6. **Domain & Governance Preservation**: All 6 other technical domains (read-only SQLite connection lifecycle, exact 30-char decimal normalization, Day Book gross vs net separation, defensive CSV & lossless JSON streaming, recursive lineage CTE, search pagination) remain 100% intact with zero migrations and zero dependency additions.

---

## 2. Criterion-by-Criterion Evaluation

### 2.1 Criterion 1: Correctness of `ReplaceFileW` Flags
- **Requirement**: Verify removal of `REPLACEFILE_WRITE_THROUGH` (0x01), accurate citation of MSDN marking it unsupported, specification of `dwReplaceFlags = 0` as default, and evaluation/rejection of ACL/Merge ignore flags for fail-closed security.
- **Audit Findings**:
  - In Section 7.2.3, `REPLACEFILE_WRITE_THROUGH` (0x00000001) is explicitly branded: *"EXPLICITLY NOT SUPPORTED BY MICROSOFT. Microsoft documentation explicitly states verbatim: 'This value is not supported.' Claiming that passing 0x00000001 forces disk flushing is technically false."*
  - Section 7.2.3 and Section 7.2.6 mandate `dwReplaceFlags = 0` as the default parameter.
  - In Section 7.2.3, `REPLACEFILE_IGNORE_MERGE_ERRORS` (0x00000002) and `REPLACEFILE_IGNORE_ACL_ERRORS` (0x00000004) are thoroughly analyzed. The design explains that accounting exports contain sensitive business intelligence and tax data. Suppressing DACL merge errors could allow an unprivileged user to access files that were intended to inherit restricted permissions from the replaced file. Failing closed when ACLs cannot be cleanly merged preserves the principle of least privilege.
  - Section 7.2.6 implements `ReplaceFileW` in unsafe Rust with `0` as the 4th argument, complete with clear `// SAFETY:` comments.
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

### 2.2 Criterion 2: Correct Ordering of Flush, Sync, Close, and Replacement
- **Requirement**: Verify that the finalization sequence in Section 7.2.2 and Section 10.2 strictly follows: (1) Stream write, (2) `writer.flush().await`, (3) `file.sync_all().await`, (4) Explicit handle closure (`drop(writer); drop(file);`), (5) Atomic replacement invocation. Verify that failure at flush or sync terminates export and unlinks `.tmp` without touching destination.
- **Audit Findings**:
  - Section 7.2.2 details the sequence both in an ASCII architectural pipeline diagram and numbered text:
    1. `BufWriter::flush()` flushes application memory buffer (64 KB) to kernel page cache.
    2. `File::sync_all()` issues Win32 `FlushFileBuffers` (or POSIX `fsync`) to synchronize kernel dirty pages to physical drive controller storage.
    3. Failure at either `flush().await` or `sync_all().await` immediately aborts the export, unlinks `tmp_path`, and leaves the destination file 100% untouched.
    4. Handles are explicitly dropped (`drop(writer); drop(file);`) to release all Windows sharing leases before attempting `ReplaceFileW` or `MoveFileExW`.
    5. Atomic replacement is invoked on the closed files.
  - Section 10.2 steps 7, 8, and 9 reiterate this exact sequence for JSON streaming serialization.
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

### 2.3 Criterion 3: Existing vs. Non-Existent Destination Behavior
- **Requirement**: Verify Win32 `ReplaceFileW(target, tmp, backup, 0, NULL, NULL)` for existing destinations, and Win32 `MoveFileExW(from, to, 0)` without replace for non-existent destinations to prevent race-condition overwriting.
- **Audit Findings**:
  - **Existing Destination**:
    - Preflight check via `tokio::fs::try_exists(target_path).await`.
    - If `overwrite == false`: fails fast with `OpenTallyError::Validation`, unlinks `.tmp`, and leaves target untouched.
    - If `overwrite == true`: generates a unique backup path `<target>.bak.<uuid>` on the same volume and invokes `ReplaceFileW(target, tmp, backup, 0, NULL, NULL)`.
    - On success: asynchronously unlinks backup; backup deletion errors do not invalidate the successful export.
    - On failure: catches raw OS error, unlinks `.tmp`, and leaves the original target untouched by virtue of NTFS transaction semantics.
  - **Non-Existent Destination**:
    - Avoids standard Rust `std::fs::rename` because it passes `MOVEFILE_REPLACE_EXISTING`.
    - Section 7.2.6 defines `rename_new_file_windows` invoking `MoveFileExW(tmp, target, 0)`.
    - Passing `0` ensures that if another process created `target_path` concurrently between the preflight existence check and the rename, `MoveFileExW` fails with `ERROR_ALREADY_EXISTS` (183) or `ERROR_FILE_EXISTS` (80).
    - `finalize_export_file` explicitly traps codes 183 and 80, unlinks `.tmp`, and returns an actionable validation error (`"Destination file '...' was created concurrently by another process. Enable overwrite to replace."`).
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

### 2.4 Criterion 4: Filesystem Fallback Guarantees & Network Share Limitations
- **Requirement**: Verify fail-closed behavior on FAT32/exFAT/SMB shares returning `ERROR_NOT_SUPPORTED` / `ERROR_INVALID_FUNCTION` rather than silent degradation to non-atomic multi-step operations. Verify network disconnect handling (`ERROR_NETNAME_DELETED` 64 / `WSAECONNRESET` 10054).
- **Audit Findings**:
  - Section 7.2.4B formally establishes the **Fail-Closed Policy**: OpenTally explicitly rejects falling back to a non-atomic `delete(target)` followed by `rename(tmp, target)`. Doing so would create a vulnerability where a crash between delete and rename permanently destroys the original destination file.
  - In `finalize_export_file` (lines 620-628), error codes `ERROR_NOT_SUPPORTED` (50), `ERROR_INVALID_FUNCTION` (1), and `ERROR_UNABLE_TO_MOVE_REPLACEMENT` (1179) are trapped, `.tmp` is cleaned up, and a clear diagnostic is returned advising the user to select an NTFS directory or provide a new filename.
  - In Section 7.2.4D, network share disconnect errors (`ERROR_NETNAME_DELETED` 64, `WSAECONNRESET` 10054) are evaluated. The operation fails closed, best-effort cleanup of `.tmp` is executed, and the existing target on the network share remains intact.
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

### 2.5 Criterion 5: Process-Crash vs. Power-Loss Claims & 14-Scenario Matrix
- **Requirement**: Verify the comprehensive 14-scenario failure and recovery matrix in Section 7.2.5. Verify honest distinction between NTFS metadata crash consistency and hardware NAND write-cache durability boundaries. Verify that the destination can never be lost or corrupted on any failure path.
- **Audit Findings**:
  - Section 7.2.2 paragraph 4 and Section 7.2.5 honestly and accurately distinguish:
    - **Crash Consistency**: Guaranteed by NTFS Master File Table ($MFT$) transaction logging and journal replay. Process crashes, thread terminations, or OS blue-screens never leave destination files half-written or missing.
    - **Physical Power-Loss Durability**: Bounded by storage device controller write caches. While `File::sync_all()` issues Win32 `FlushFileBuffers` to push dirty pages to the disk subsystem, consumer SSDs lacking power-loss protection (PLP supercapacitors) may volatile-cache blocks. The design explicitly avoids making false physical durability claims.
  - The 14-scenario matrix (Section 7.2.5) covers all failure points from initial file creation through stream writes, buffer flush, drive sync, handle closure, locked files, access restrictions, unsupported filesystems, network disconnects, API errors, SIGKILL crashes, and power losses.
  - Across all 14 scenarios, the old destination is either 100% intact (scenarios 1-11, 13) or cleanly and completely replaced with the verified new export (scenarios 12, 14). In zero scenarios is the old destination lost or corrupted.
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

### 2.6 Criterion 6: Domain Preservation & Governance Integrity
- **Requirement**: Verify that all 6 other domains remain intact (read-only SQLite lifecycle, exact 30-char decimal normalization, gross vs net Day Book separation, CSV CWE-1236 & RFC 4180 escaping, JSON streaming, lineage recursive CTE, search pagination). Verify zero migrations, zero new dependencies, and preservation of governance decisions `DEC-S2-001` through `DEC-S2-004` and findings `FIND-6-01` through `FIND-6-06`.
- **Audit Findings**:
  - **Domain 1 (SQLite Read-Only Lifecycle)**: Section 6 retains `PRAGMA query_only = ON;`, explicit snapshot transaction protocol, and the full 6-mode connection failure matrix with mandatory `conn.close().await` detachment, guaranteeing zero pool contamination.
  - **Domain 2 (Exact Decimal Normalization)**: Section 5 retains the 30-character canonical normalization ($25$ integer digits zero-padded $+$ `.` $+$ $4$ fraction digits zero-padded). Exact schema bounds ($length \le 25$) and integer scaling overflow proofs remain unchanged.
  - **Domain 3 (Accounting Semantics & Aggregates)**: Section 4 preserves the Day Book Gross Activity Volume ($\sum v.\text{total\_amount}$) vs General Ledger Net Movement separation, retains reversal pairs chronologically, assigns 0.00 weight to cancelled vouchers, and explicitly excludes historical as-of reporting.
  - **Domain 4 (Defensive CSV Pipeline)**: Section 8 preserves the decoupled two-stage architecture: Stage 1 formula injection sanitization (stripping ASCII 0..=31, prepending `'` for formula triggers) on user-controlled text, preserving raw numeric columns, followed by Stage 2 RFC 4180 escaping.
  - **Domain 5 (Lossless JSON Streaming)**: Section 10 preserves Mode A direct-to-file streaming (< 5 MB RAM ceiling, 50,000 hard cap) and Mode B materialized preview (1,000 cap), with Draft-07 JSON schema v1.0.
  - **Domain 6 (Lineage Recursive CTE)**: Section 9 preserves the cycle-protected recursive CTE, depth cap of 10, true 11th-record truncation detection (`any(|r| r.min_depth > 10)`), and cancellation reason extraction via indexed `audit_log` lookup.
  - **Domain 7 (Search & Pagination Determinism)**: Section 2, 6, 11 preserve deterministic tie-breaking sorts and checked arithmetic offset calculations.
  - **Zero Migrations & Zero Dependencies**: Confirmed in Section 3.2 and Section 16. No schema modifications or Cargo crate additions are introduced.
  - **Governance Ledger**: Section 14 preserves all four governance decisions (`DEC-S2-001` through `DEC-S2-004`) and all six carry-forward findings (`FIND-6-01` through `FIND-6-06`).
- **Classification**: **COMPLIANT (PASS — Zero Defects)**

---

## 3. Deep Technical Analysis & Verification

### 3.1 Win32 API Implementation Correctness
The signature declared in Section 7.2.6:
```rust
extern "system" {
    fn ReplaceFileW(
        lpReplacedFileName: *const u16,
        lpReplacementFileName: *const u16,
        lpBackupFileName: *const u16,
        dwReplaceFlags: u32,
        lpExclude: *mut std::ffi::c_void,
        lpReserved: *mut std::ffi::c_void,
    ) -> i32;
    fn MoveFileExW(
        lpExistingFileName: *const u16,
        lpNewFileName: *const u16,
        dwFlags: u32,
    ) -> i32;
}
```
- Both signatures use `extern "system"`, matching the Win32 `STDCALL` ABI on x86 and the standard Microsoft x64 calling convention.
- All path pointers are constructed using `to_wide_null`, which correctly appends a 16-bit null terminator (`chain(std::iter::once(0))`).
- `ReplaceFileW` receives `0` for `dwReplaceFlags`, null pointers for `lpExclude` and `lpReserved`, matching official Win32 requirements.
- Error codes are retrieved via `std::io::Error::last_os_error()`, ensuring high-fidelity mapping of raw OS error numbers.

### 3.2 Durability Protocol & Handle Lifecycle
The execution sequence in Section 7.2.2 and Section 10.2 guarantees complete isolation from Windows file locking bugs:
```
[Application BufWriter (64 KB)]
           |
           v  writer.flush().await
[OS Kernel Page Cache (Dirty Pages)]
           |
           v  file.sync_all().await (Win32 FlushFileBuffers)
[Physical Drive Controller Cache / Media]
           |
           v  drop(writer); drop(file);
[File Handles Closed & Sharing Leases Released]
           |
           v  ReplaceFileW (dwReplaceFlags = 0)
[Atomic Metadata Swap Committed to NTFS MFT Log]
```
If `ReplaceFileW` were invoked while `writer` or `file` remained open, Windows would immediately return `ERROR_SHARING_VIOLATION` (code 32). Mandating explicit handle closure before invoking `ReplaceFileW` ensures the call succeeds reliably.

### 3.3 Race-Safe Non-Existent File Creation
Standard `rename` implementations on Windows typically default to `MOVEFILE_REPLACE_EXISTING`. If a concurrent process creates `target_path` between OpenTally's preflight check and the rename operation, standard `rename` would overwrite that file silently. By specifying `MoveFileExW(tmp, target, 0)`, OpenTally guarantees that Windows will reject the move with `ERROR_ALREADY_EXISTS` (183) or `ERROR_FILE_EXISTS` (80), allowing OpenTally to abort without overwriting the third-party file.

---

## 4. Evaluation of Low-Risk Observations

While the design is technically complete, sound, and fully approved for implementation, the auditor notes three low-risk practical observations to guide engineers during Milestone 7A:

### Observation L-01: Idiomatic Tokio `BufWriter` Sync Pattern
In Section 10.2, the design specifies:
> *"Access the underlying `tokio::fs::File` and call `file.sync_all().await?`... Explicitly drop `writer` and `file` (`drop(writer); drop(file);`)"*
- **Implementation Nuance**: In Tokio, `BufWriter<tokio::fs::File>` wraps and owns the underlying `File`. To avoid needing to unpack or re-wrap the writer, the implementer can use:
  ```rust
  writer.flush().await?;
  writer.get_mut().sync_all().await?;
  drop(writer); // Automatically drops the inner tokio::fs::File
  ```
  This achieves the exact specified sequence in three concise lines.

### Observation L-02: Unix & Cross-Platform CI Runner Fallback
In Section 7.2.6, `finalize_export_file` specifies:
```rust
#[cfg(not(windows))]
{
    if let Err(e) = tokio::fs::rename(tmp_path, target_path).await {
        let _ = tokio::fs::remove_file(tmp_path).await;
        return Err(OpenTallyError::Io(format!("Failed to finalize export file: {}", e)));
    }
    Ok(())
}
```
- **Implementation Nuance**: On Linux development machines or CI runners, `tokio::fs::rename` calls `rename(2)`. In Case A (non-existent target), `rename(2)` would overwrite if another process concurrently created the target. While OpenTally's target distribution is Windows desktop (where `MoveFileExW(..., 0)` is strictly enforced), Linux developers and CI pipelines should be aware that Unix `rename(2)` replaces existing destinations by default. For pure cross-platform parity on modern Linux kernels (>= 3.15), `renameat2(..., RENAME_NOREPLACE)` could optionally be used if multi-platform desktop support is planned in future sprints. For Sprint 2 Windows target, the current specification is completely sound.

### Observation L-03: Startup Hygiene Sweep for Orphaned Dot-Files
In Scenario 11 of the 14-scenario matrix (application termination via `SIGKILL` or power cut before replacement), an orphaned sibling file `.~opentally_export_<uuid>.tmp` remains on disk.
- **Implementation Nuance**: While sibling temporary files are hidden dot-files and harmless, adding a background startup sweep during app initialization to delete `.~opentally_export_*.tmp` files older than 24 hours in the sandboxed export directory will ensure pristine long-term filesystem hygiene.

### Informational I-01: Windows Long Path (`MAX_PATH`) Prefixing
Tracked in `UNK-7-02`: Windows paths longer than 260 characters without `\\?\`. The design notes that default sandboxed fallback is `< 120` chars, frontend validates paths, and full UNC prefixing will be evaluated during packaging. This is properly tracked.

---

## 5. Formal Verdict & Sign-Off

### Final Verdict: **READY WITH LOW-RISK OBSERVATIONS**

The Technical Design Document `OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md` (**Version 2.2.2**) has successfully resolved all prior verification defects. The specification represents an exceptionally high standard of software engineering, combining strict accounting correctness, bulletproof database transaction hygiene, robust CSV/JSON export streaming, and native Windows filesystem crash safety.

Stage 7 is formally **APPROVED** to proceed immediately to **Milestone 7A: Backend Search & Export Implementation**.

---

## 6. Actionable Recommendations for Milestone 7A

1. **Service Layer Organization**:
   - Implement search and filtering logic in `02-open-tally/src-tauri/src/vouchers/service/search.rs`.
   - Implement CSV and JSON export logic in `02-open-tally/src-tauri/src/vouchers/service/export.rs`.
   - Implement Windows-specific file finalization in a dedicated private module `02-open-tally/src-tauri/src/vouchers/service/export/windows.rs` guarded by `#[cfg(windows)]`.
2. **Strict Test Fixture Isolation**:
   - For `TC-EXP-06` (Windows locked file handling), create an integration test that opens an exclusive `std::fs::OpenOptions::new().write(true).open(...)` file handle in a background thread, executes an export targeting that path, and asserts that `OpenTallyError::Io` is returned with the locked-file diagnostic without crashing.
3. **Canonical Normalization Verification**:
   - Implement unit tests covering `TC-SRCH-08`, `TC-SRCH-09`, and `TC-SRCH-10` asserting that sorting 10,000 generated amounts spanning $0.0001$ to $10^{24}$ matches exact mathematical sorting order with zero floating-point conversions.
4. **Pool Hygiene Integration Test**:
   - Implement a test asserting that injected errors during `execute_read_only_query` result in immediate connection detachment, verifying that subsequent write transactions on the pool succeed without receiving `SQLITE_READONLY`.

---
*Report Authoritative Sign-Off:*  
**Lead Independent Technical Auditor — OpenTally Sprint 2 Stage 7**
