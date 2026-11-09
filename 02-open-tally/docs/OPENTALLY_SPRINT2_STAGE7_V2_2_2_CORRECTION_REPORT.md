# OpenTally Sprint 2 Stage 7 — Technical Design Correction Report (v2.2.2)
## Voucher Search, Filters & Export Engine

**Document Version**: 2.2.2 Correction Report  
**Target Document**: `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`  
**Target Repository**: `02-open-tally`  
**Date**: 2026-11-09  
**Execution Boundary**: **STRICTLY DESIGN CORRECTION ONLY**  
*(Zero application code, zero migrations, zero test execution, zero dependency changes, zero Git modifications)*  

---

## 1. Executive Summary & Correction Background

Following the final independent design verification (`OPENTALLY_SPRINT2_STAGE7_FINAL_DESIGN_VERIFICATION.md`), which rendered a verdict of **NOT READY** based on one HIGH blocker and one MEDIUM finding in Section 7.2 (Windows export finalization), this document records the targeted technical design corrections applied to establish **Version 2.2.2**.

All previous accepted design decisions across the other six verification domains (read-only SQLite connection lifecycle, exact 30-character decimal normalization, gross vs net Day Book accounting semantics, defensive CSV & lossless JSON streaming, recursive lineage CTE, and search/sort/pagination determinism) have been strictly preserved without modification.

---

## 2. Summary of Applied Corrections

### 2.1 Correction 1: Elimination of Unsupported `ReplaceFileW` Flag
- **Prior Defect (v2.2.1)**: Section 7.2 specified `dwReplaceFlags = 0x00000001 (REPLACEFILE_WRITE_THROUGH forces flush to disk)`.
- **Correction Applied**:
  - Removed `0x00000001` and any claim that this flag flushes file data or guarantees disk durability.
  - Formally documented official Microsoft Win32 documentation ([MSDN `ReplaceFileW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)) confirming that `REPLACEFILE_WRITE_THROUGH` is **explicitly unsupported** (*"This value is not supported"*).
  - Specified `dwReplaceFlags = 0` as the authoritative default replacement behavior.
  - Documented why `REPLACEFILE_IGNORE_MERGE_ERRORS` (`0x02`) and `REPLACEFILE_IGNORE_ACL_ERRORS` (`0x04`) are deliberately NOT used: in accounting software, failing to merge Access Control Lists (DACLs) could expose confidential financial exports if the destination file had restrictive permissions. Failing closed on ACL merge failure preserves strict security.
  - Implemented exact Windows error code capture via `std::io::Error::last_os_error()`.

### 2.2 Correction 2: Temporary File Flush, Storage Synchronization & Handle Closure
- **Prior Defect (v2.2.1)**: Conflated NTFS metadata replacement with file payload power-loss durability, omitting explicit storage cache synchronization.
- **Correction Applied**:
  - Mandated a strict two-stage durability sequence prior to destination replacement:
    1. `writer.flush().await?`: Flushes application-level memory buffers (`BufWriter` 64 KB) to operating system kernel page cache.
    2. `file.sync_all().await?`: Issues Win32 `FlushFileBuffers` (or POSIX `fsync`), synchronizing dirty pages to physical drive controller storage. Reference: [`tokio::fs::File::sync_all`](https://docs.rs/tokio/latest/tokio/fs/struct.File.html#method.sync_all).
    3. Treat failure of either `flush()` or `sync_all()` as an immediate export failure (unlinking `.tmp` and failing closed).
    4. Explicitly drop/close file and writer handles (`drop(writer); drop(file);`) to release all Windows sharing leases before invoking `ReplaceFileW`.
  - Documented honest durability boundaries: while `sync_all()` provides the strongest portable durability guarantee on Windows, physical write-through is ultimately bounded by on-disk controller write-cache policies.

### 2.3 Correction 3: Filesystem-Specific Replacement & Race-Safe Creation Behavior
- **Prior Defect (v2.2.1)**: Assumed `tokio::fs::rename` was safe for non-existent destinations without addressing concurrent file creation races, and lacked documented fail-closed behavior for unsupported filesystems.
- **Correction Applied**:
  - **A. Existing Destination on NTFS/ReFS**: Atomic swap via Win32 `ReplaceFileW(target, tmp, backup, 0, NULL, NULL)`.
  - **B. Existing Destination on Unsupported Filesystems (FAT32, exFAT, SMB)**:
    - If `ReplaceFileW` returns `ERROR_NOT_SUPPORTED` (50), `ERROR_INVALID_FUNCTION` (1), or `ERROR_UNABLE_TO_MOVE_REPLACEMENT` (1179), OpenTally **fails closed** with a clear diagnostic (`"Atomic file replacement is unsupported on this filesystem or network share..."`).
    - OpenTally strictly refuses to silently fall back to a non-atomic multi-step sequence (`delete` + `rename`) to prevent destination disappearance.
  - **C. Non-Existent Destination (Race-Safe Creation)**:
    - Replaced standard `rename` (which passes `MOVEFILE_REPLACE_EXISTING`) with Win32 `MoveFileExW(from, to, 0)` without replace.
    - If another process created `target_path` concurrently after our preflight existence check, `MoveFileExW(..., 0)` fails with `ERROR_ALREADY_EXISTS` (183) or `ERROR_FILE_EXISTS` (80), unlinks `.tmp`, and returns `OpenTallyError::Validation`.
    - Explicitly distinguished `MOVEFILE_WRITE_THROUGH` (`0x08` in `MoveFileExW`) from unsupported `REPLACEFILE_WRITE_THROUGH` (`0x01` in `ReplaceFileW`).
  - **D. Network Shares (SMB) & Removable Media**:
    - Handled network disconnection errors (`ERROR_NETNAME_DELETED` 64, `WSAECONNRESET` 10054); operation fails closed and existing destination is left untouched.

### 2.4 Correction 4: Comprehensive 14-Scenario Failure & Recovery Matrix
Added a complete 14-scenario matrix (Section 7.2.5) distinguishing process-crash consistency from physical power-loss durability across all failure events:
1. Temp file creation failure (`create_new` fails)
2. Stream write failure (disk full / I/O error)
3. Writer buffer flush failure (`flush().await` fails)
4. Storage sync failure (`sync_all().await` fails)
5. Temp file close failure (handle closure error)
6. Destination locked (`ERROR_SHARING_VIOLATION` 32)
7. Access denied (`ERROR_ACCESS_DENIED` 5)
8. Unsupported filesystem (`ERROR_NOT_SUPPORTED` 50)
9. Network share disconnection (`ERROR_NETNAME_DELETED` 64)
10. Replacement API failure (`ReplaceFileW` fails)
11. Application termination before replacement
12. Application termination during/after replacement
13. Power loss after sync but before replacement
14. Power loss after replacement committed

### 2.5 Correction 5 & 6: Domain & Governance Preservation
- All six non-modified design domains remain intact.
- Zero database migrations introduced.
- Zero new external dependencies added.
- All governance statuses preserved without modification:
  - `DEC-S2-001` (Rounding): `PENDING USER APPROVAL`
  - `DEC-S2-002` (Sequence-Gap): `PENDING USER APPROVAL`
  - `DEC-S2-003` (Cancellation Storage): `APPROVAL EVIDENCE NOT VERIFIED`
  - `DEC-S2-004` (Correction Workflow): `APPROVAL EVIDENCE NOT VERIFIED`
  - `FIND-6-01` through `FIND-6-06`: Remain tracked.

---

## 3. File Checksums & Artifact Verification

| File Path | Version | SHA-256 Checksum |
|:---|:---:|:---:|
| `02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md` | v2.2.2 | `FCD9413BAA6B2BA29B8D4E36B811F6976C89E9F73581EB94AB52DAF4D9F94267` |
| `C:\Users\sanvi\.gemini\antigravity\brain\6c1fb660-6b74-4677-bba2-c159b4f31c4f\OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md` | v2.2.2 | `FCD9413BAA6B2BA29B8D4E36B811F6976C89E9F73581EB94AB52DAF4D9F94267` |

Both primary and root artifact copies are verified to be byte-identical.

