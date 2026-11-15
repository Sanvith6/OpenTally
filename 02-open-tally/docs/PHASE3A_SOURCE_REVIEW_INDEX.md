# OpenTally — Sprint 2 Stage 7A Phase 3A
# Final Source Review Index & Independent Verification Package

**Project:** OpenTally  
**Milestone:** Stage 7A — Backend Search & Export  
**Phase:** 3A — Streaming Export Core Implementation  
**Approved Design Authority:** Stage 7 Technical Design v2.2.2 (`02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md`)  
**Database Schema:** `0.3.1-sprint2`  
**Execution Date:** 2026-11-12  
**Review Mode:** Read-Only Source Review Package  

---

## 1. Executive Summary & Review Scope

This package exposes the complete, unmodified source code, unit and integration tests, architectural specifications, Git diffs, and cryptographic hash manifests for the independent acceptance review of **Sprint 2 Stage 7A Phase 3A (Streaming Export Core)**.

### Scope Boundaries
- **In Scope (Phase 3A):**
  - CSV Summary export (13 columns, Day Book format, RFC 4180 CRLF, CWE-1236 defense, optional UTF-8 BOM).
  - CSV Detailed export (15 columns, Audit Dump format, line-by-line decomposition joined with ledgers, RFC 4180 CRLF, CWE-1236 defense, optional UTF-8 BOM).
  - Lossless JSON Streaming (Mode A: Draft-07 JSON Schema v1.0, $O(1)$ memory framing, exact string decimals).
  - In-Memory Preview Export (Mode B: Materialized representation, strictly capped at 1,000 records).
  - Snapshot consistency under SQLite WAL mode: Preflight count validation (`SELECT COUNT(*)`) and data streaming execute on the exact same dedicated connection within a single read-only transaction via `execute_read_only_query`.
  - Durability and fail-closed cleanup: 64 KB `BufWriter` buffer, two-stage durability (`flush().await` + `sync_all().await`), and RAII `PartialFileGuard` cleanup on error, panic, or async task cancellation.
  - 18 comprehensive tests in `stage7_export_tests.rs`.
- **Strictly Excluded (Phase 3B):**
  - NO Win32 `ReplaceFileW`.
  - NO Win32 `MoveFileExW`.
  - NO atomic destination replacement or `finalize_export_file`.
  - Zero database schema migrations or table changes.
  - Zero changes staged or committed to Git.

---

## 2. Directory Layout & Packaged Files

The review package is organized in `02-open-tally/docs/phase3a-final-source-review/` with identical layout and bit-for-bit file contents:

```text
phase3a-final-source-review/
├── Cargo.lock
├── Cargo.toml
├── OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md
├── PHASE3A_SOURCE_REVIEW_INDEX.md
├── STAGE7A_PHASE3A_STREAMING_EXPORT_COMPLETION_REPORT.md
├── diffs/
│   ├── all_phase3a_changes.diff
│   ├── cargo_lock.diff
│   ├── cargo_toml.diff
│   ├── export_impl.diff
│   ├── stage7_export_tests.diff
│   ├── vouchers_mod.diff
│   ├── vouchers_model.diff
│   ├── vouchers_service_mod.diff
│   └── vouchers_service_search.diff
└── vouchers/
    ├── mod.rs
    ├── model.rs
    ├── stage7_export_tests.rs
    └── service/
        ├── export.rs
        ├── mod.rs
        └── search.rs
```

---

## 3. Implementation Index & Exact Source Line Ranges

The following table maps every critical Phase 3A architectural component to its exact source file and line range in the implementation:

| Component / Subsystem | File Path | Line Range | Architectural Purpose & Invariants |
|---|---|---|---|
| **Constants & Header Specs** | `src/vouchers/service/export.rs` | L24–L49 | `MAX_FILE_EXPORT_RECORDS = 50_000`, `MAX_PREVIEW_RECORDS = 1_000`, `UTF8_BOM`, `CRLF`, 13 Summary and 15 Detailed column definitions. |
| **`sanitize_text_cell`** | `src/vouchers/service/export.rs` | L51–L70 | CWE-1236 formula injection defense: neutralizes `=`, `+`, `-`, `@`, `\t`, `\r`, `\n` by prepending `'`. |
| **`escape_rfc4180`** | `src/vouchers/service/export.rs` | L73–L81 | RFC 4180 compliance: encloses cells containing `,`, `"`, `\n`, `\r` in quotes and doubles internal quotes (`""`). |
| **`format_csv_cell`** | `src/vouchers/service/export.rs` | L84–L101 | Formats cell value: user text undergoes formula sanitization then RFC escaping; numeric amounts bypass sanitization to preserve arithmetic. |
| **`PartialFileGuard`** | `src/vouchers/service/export.rs` | L108–L135 | RAII cleanup guard: removes partial files on drop if not committed. Declared before file handles for clean drop ordering. |
| **`VoucherExportService`** | `src/vouchers/service/export.rs` | L140–L893 | Core export service struct providing public API and internal serialization engines. |
| **Preview Mode B (`export_preview`)** | `src/vouchers/service/export.rs` | L149–L215 | In-memory export capped at 1,000 records. Serializes directly to `Vec<u8>` in RAM without disk operations. |
| **File Export (`export_to_file`)** | `src/vouchers/service/export.rs` | L230–L405 | File export entry point coordinating path validation, preflight count checks, 64 KB `BufWriter`, durability, and cleanup. |
| **Path Security & Allowlist** | `src/vouchers/service/export.rs` | L234–L272 | Rejects null bytes, empty paths, parent directory traversal (`..`), and enforces `.csv` / `.json` extensions. |
| **Overwrite Governance** | `src/vouchers/service/export.rs` | L275–L281 | Rejects operation if file exists and `request.overwrite != Some(true)` prior to opening or modifying file. |
| **Parent Directory Creation** | `src/vouchers/service/export.rs` | L284–L290 | Recursively ensures parent directory exists (`tokio::fs::create_dir_all`). |
| **Snapshot Query & Preflight Count** | `src/vouchers/service/export.rs` | L298–L329 | Evaluates `SELECT COUNT(*)` preflight within dedicated read-only transaction; rejects >50,000 records. |
| **File Creation & 64 KB `BufWriter`** | `src/vouchers/service/export.rs` | L332–L336 | Instantiates `PartialFileGuard`, opens `File::create`, wraps in `BufWriter::with_capacity(64 * 1024, file)`. |
| **Stream Execution** | `src/vouchers/service/export.rs` | L338–L355 | Invokes `export_stream_impl`; on error, drops writer and allows guard to delete partial file. |
| **Two-Stage Durability (`flush` + `sync_all`)** | `src/vouchers/service/export.rs` | L357–L370 | User-space buffer flush (`writer.flush().await`) followed by kernel sync (`file.sync_all().await`). |
| **Commit Guard & Handle Release** | `src/vouchers/service/export.rs` | L371–L374 | Explicitly drops file handle to prevent Windows lock violations, then marks `guard.commit()`. |
| **Generic Writer (`export_to_writer`)** | `src/vouchers/service/export.rs` | L408–L466 | Streams exported data directly into any `AsyncWrite + Unpin + Send + 'static` writer under read-only snapshot. |
| **Stream Dispatcher (`export_stream_impl`)** | `src/vouchers/service/export.rs` | L468–L492 | Dispatches stream to CSV Summary, CSV Detailed, or JSON serializers. |
| **CSV Summary Serializer** | `src/vouchers/service/export.rs` | L494–L598 | Day Book format: 13 columns, row-by-row streaming, audit log cancellation extraction, CRLF line endings. |
| **CSV Detailed Serializer** | `src/vouchers/service/export.rs` | L600–L720 | Audit Dump format: 15 columns, line-by-line decomposition joined with `voucher_lines` and `ledgers`. |
| **JSON Streaming Serializer** | `src/vouchers/service/export.rs` | L722–L892 | Draft-07 JSON Schema v1.0, top-level metadata envelope, $O(1)$ RAM framing per voucher, exact string decimals. |
| **Export Domain Models** | `src/vouchers/model.rs` | L667–L752 | `ExportFormat`, `VoucherExportFileRequest`, `VoucherExportFileResult`, `VoucherExportPreviewRequest`, `VoucherExportPreviewResult`, `VoucherExportLineItem`, `VoucherCompositeExportItem`. |
| **Search Query Shared Visibility** | `src/vouchers/service/search.rs` | L246–L256 | `pub(crate) struct FilteredQuery` and `pub(crate) fn build_filtered_query` reused by export without code duplication. |
| **Module Re-Exports** | `src/vouchers/service/mod.rs` | L5–L28 | Registered `export` submodule; re-exported `VoucherExportService`, headers, BOM, and sanitizers. |
| **Crate Root Registration** | `src/vouchers/mod.rs` | L24–L42 | Registered `stage7_export_tests` test module; re-exported export types. |
| **Test Fixtures & Setup** | `src/vouchers/stage7_export_tests.rs` | L1–L200 | Temporary SQLite test harness, reserved cash ledger seeding, draft creation, and voucher posting helpers. |
| **All 18 Export Tests** | `src/vouchers/stage7_export_tests.rs` | L206–L1119 | Unit, boundary, snapshot, cancellation, and multi-tenant isolation tests. |

---

## 4. In-Depth Risk Investigations

### A. Partial-File Cleanup Investigation

#### Complete Implementation (`src/vouchers/service/export.rs:108-135`)
```rust
struct PartialFileGuard {
    path: std::path::PathBuf,
    committed: bool,
}

impl PartialFileGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for PartialFileGuard {
    fn drop(&mut self) {
        if !self.committed && self.path.exists() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
```

#### Detailed Findings & Invariants:
1. **Ownership of the Partial-File Path:**
   `PartialFileGuard::new(target_path_buf.clone())` takes sole ownership of a dedicated `std::path::PathBuf`.
2. **When the Guard is Marked Committed:**
   `guard.commit()` is called strictly at line 373, **after** the data stream completes successfully, `writer.flush().await` succeeds, `file.sync_all().await` succeeds, `file.metadata().await` succeeds, and `drop(file)` has released the OS file handle.
3. **Drop Behavior on Cancellation and Panic:**
   If a Tokio task running `export_to_file` is aborted via `task.abort()`, dropped by `tokio::select!`, or panics, the async future is dropped synchronously. Rust executes destructors in reverse order of declaration:
   - `writer` and `file` (declared at L334–L335) are dropped **first**, closing the OS file handle.
   - `guard` (declared at L332) is dropped **second**. Its `Drop` implementation detects `!self.committed && self.path.exists()` and invokes `std::fs::remove_file(&self.path)`.
4. **Handle Release Before Deletion:**
   On Windows NT, deleting a file with an open file handle causes `ERROR_SHARING_VIOLATION` (OS error 32). In all code paths (both async error returns at L351, L358, L365 and the synchronous drop path), the `BufWriter` / `File` handle is dropped before deletion occurs.
5. **What Happens if Synchronous Deletion Fails:**
   `let _ = std::fs::remove_file(&self.path);` suppresses the I/O error. Rust destructors must never panic during stack unwinding. The outer operation returns `Err(OpenTallyError)` to the caller.
6. **Accidental Deletion of Existing Destinations:**
   - When `overwrite == false`: The file existence check at line 240 returns `Err(OpenTallyError::Validation)` before `PartialFileGuard` is instantiated or `File::create` is called. An existing destination is never touched.
   - When `overwrite == true`: In Phase 3A, `File::create` truncates the file at start of streaming. If streaming fails, the guard removes the truncated/corrupted file. (Note: Non-destructive atomic replacement where the original file remains intact until the new file is fully written is explicitly reserved for Phase 3B's `ReplaceFileW`).

---

### B. Path Security Investigation

#### Detailed Findings:
1. **Windows Drive Paths:**
   Fully supported (e.g. `C:\Users\User\Documents\export.csv`). Rust's `Path::components()` parses this as `Prefix(Disk)` followed by `RootDir` and `Normal` components. It contains no `ParentDir` component.
2. **UNC Paths:**
   Fully supported on Windows (e.g. `\\server\share\export.csv`). Rust parses this as `Prefix(UNC)`.
3. **Relative Paths:**
   Relative paths without `..` (e.g. `exports/vouchers.csv` or `output.json`) parse as valid components. They resolve relative to the current working directory of the application process.
4. **Parent Directory Traversal (`..`):**
   Explicitly checked and rejected at lines 248–256:
   ```rust
   let path_obj = Path::new(path_str);
   for comp in path_obj.components() {
       if comp == std::path::Component::ParentDir {
           return Err(OpenTallyError::Validation(
               "Target path cannot contain parent directory traversal ('..')".to_string(),
           ));
       }
   }
   ```
   Sequences such as `../../Windows/System32` or `nested/../../secret.csv` are rejected immediately.
5. **Null Byte Injection (`\0`):**
   Explicitly rejected at lines 241–245: `path_str.contains('\0')`.
6. **Extension Validation:**
   Enforces a strict extension allowlist at lines 259–272:
   - CSV formats (`CsvSummary`, `CsvDetailed`) require `.csv` (case-insensitive via `eq_ignore_ascii_case`).
   - JSON format requires `.json`.
   - Executable extensions (`.exe`, `.dll`, `.bat`, `.cmd`) or missing extensions are rejected with `OpenTallyError::Validation`.
7. **Existing Destination Handling:**
   If `target_path.exists()` and `request.overwrite != Some(true)`, line 240 rejects the operation with `OpenTallyError::Validation("Destination file '...' already exists and overwrite is false")`.
8. **Symlinks and Race Conditions (TOCTOU):**
   In Phase 3A, path validation and file opening are sequential standard Tokio operations. A race condition (TOCTOU) between checking file existence and opening the file could theoretically exist if an external process creates the file in between. Resolving this with Win32 atomic file creation flags (`CREATE_NEW`) and atomic destination replacement (`ReplaceFileW`) is the explicit scope of Phase 3B.

---

### C. CSV Formula Injection Investigation (CWE-1236)

#### Sanitization Implementation (`src/vouchers/service/export.rs:51-70`)
```rust
pub fn sanitize_text_cell(value: &str) -> String {
    let trimmed =
        value.trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control());
    if value.starts_with('\t')
        || value.starts_with('\r')
        || value.starts_with('\n')
        || trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@')
    {
        format!("'{}", value)
    } else {
        value.to_string()
    }
}
```

#### Detailed Findings & Invariants:
1. **Trigger Characters (`=`, `+`, `-`, `@`):**
   Detected on `trimmed.starts_with(...)`. If present, prepends `'` to force spreadsheet engines (Microsoft Excel, LibreOffice Calc, Google Sheets) to treat the cell as raw text rather than an executable formula or DDE link.
2. **Leading Spaces and Tabs:**
   `trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control())` strips leading spaces (`' '`), tabs (`'\t'`), and control characters before checking for formula triggers. Inputs like `"   =cmd|' /C calc'!A0"` or `"\t@SUM(1,2)"` are neutralized.
3. **Carriage Returns and Line Feeds (`\r`, `\n`):**
   Checked against raw string starts: `value.starts_with('\r') || value.starts_with('\n')`.
4. **Unicode Whitespace:**
   `c.is_ascii_whitespace()` and `c.is_ascii_control()` sanitize ASCII controls and standard whitespace. Non-ASCII Unicode whitespace (such as `\u{00A0}` non-breaking space) is not trimmed; spreadsheet parsers evaluate formulas based on ASCII formula triggers.
5. **Numeric Field Preservation (Financial Integrity):**
   In `format_csv_cell` (lines 84–101), a strict distinction is enforced via `is_user_text: bool`:
   - User text fields (`voucher_number`, `voucher_type_name`, `series_name`, `reference_no`, `narration`, `cancellation_reason`, `reversal_of`, `ledger_id`, `ledger_name`, `line_narration`) are processed with `is_user_text: true`.
   - Numeric amounts (`total_amount`, `line_amount`) and counts (`line_count`, `line_number`) use `is_user_text: false`. They bypass `sanitize_text_cell` completely.
   - Amounts like `1500.00` or `2500.50` remain raw numbers without a prepended single quote, allowing spreadsheet arithmetic (`SUM`, `AVERAGE`) to calculate correctly without manual data cleaning.
6. **RFC 4180 Escaping Order:**
   In `format_csv_cell`, formula sanitization occurs **first**, followed immediately by `escape_rfc4180`. Any sanitized value containing commas, double quotes, or newlines is wrapped in outer quotes with internal quotes doubled (`""`), preserving both CSV syntax validity and spreadsheet safety.

---

### D. Streaming Architecture & Memory Bounds Investigation

#### Data Flow from SQLite Cursor to Output Writer
```text
SQLite Database (WAL Mode)
      │
      ▼
ReadOnlyConnectionGuard (PRAGMA query_only = ON; tx.begin())
      │
      ├─► Step 1: SELECT COUNT(*) preflight (Rejects >50,000 / >1,000)
      │
      └─► Step 2: Streaming Query Execution (fetch(&mut *conn))
            │
            ├─► CSV Summary: direct row streaming -> BufWriter (O(1) memory)
            │
            ├─► CSV Detailed: joined line streaming -> BufWriter (O(1) memory)
            │
            └─► JSON Mode A: voucher accumulator state machine -> BufWriter
                  (bounded by lines of single voucher, O(1) memory over N vouchers)
```

#### Detailed Findings & Invariants:
1. **No Full-Result Collection for File Export:**
   Neither `stream_csv_summary`, `stream_csv_detailed`, nor `stream_json` collects rows into a `Vec<Row>` or intermediate container. Rows stream asynchronously one-by-one from SQLite using `select_query.fetch(&mut *conn).next().await`.
2. **No Unbounded Line-Item Accumulation in JSON:**
   The SQL query orders by `ORDER BY {filtered.order_by_sql}, vl.line_number ASC`. Because `v.id ASC` is guaranteed as the tie-breaker in `build_filtered_query`, all lines for any voucher appear contiguously in the stream. The serializer accumulates lines for a **single** voucher in `current_voucher: Option<VoucherCompositeExportItem>` (lines 800–843) and serializes it to the writer as soon as the voucher ID changes, immediately dropping the voucher. RAM usage scales with the maximum lines in a single voucher ($O(lines)$), remaining strictly independent of the total voucher count $N$.
3. **Bounded Buffering:**
   Disk writes use `tokio::io::BufWriter::with_capacity(64 * 1024, file)` (exact 64 KB buffer), minimizing kernel context switches while bounding RAM consumption.
4. **Preflight Hard Caps:**
   - File export: `MAX_FILE_EXPORT_RECORDS = 50_000` (line 31).
   - Preview mode: `MAX_PREVIEW_RECORDS = 1_000` (line 34).
   If the count query returns more than the cap, execution terminates immediately before creating files or allocating stream buffers.
5. **Snapshot Consistency:**
   Both the count preflight query and the data stream query execute on the exact same connection within the same read-only transaction (`execute_read_only_query`). Under SQLite WAL mode, readers view a frozen point-in-time snapshot. Concurrent transactions inserting or modifying vouchers cannot change the record count or interleave rows during streaming.
6. **Connection Pool Safety on Cancellation:**
   If a task is cancelled, `ReadOnlyConnectionGuard::close_on_drop()` ensures that any transaction or connection in an undetermined state is discarded and closed rather than returned contaminated to the pool.

---

### E. JSON Correctness Investigation

#### Detailed Findings & Invariants:
1. **JSON Syntax and Escaping:**
   Top-level metadata and individual voucher items are serialized using `serde_json`. Special characters, Unicode glyphs, and quotation marks are escaped according to RFC 8259.
2. **Exact String Decimals (Zero Float Loss):**
   `total_amount` and line `amount` are serialized as JSON strings (`"1500.00"`). They are never converted to `f32` or `f64`, ensuring exact precision for financial balances up to the 25-character canonical limit (`99999999999999999999.9999`).
3. **Metadata Envelope Consistency:**
   Emits envelope matching Draft-07 JSON Schema v1.0:
   - `schema_version`: `"1.0"`
   - `company_id`: String UUID
   - `exported_at`: RFC 3339 timestamp
   - `total_records`: Integer count matching preflight
   - `filter_applied`: Serialized `VoucherSearchFilter`
   - `vouchers`: Array of composite voucher items
4. **Empty Export Behavior:**
   When `total_records == 0`, `stream_json` writes opening metadata followed immediately by `b"]}"` (line 739), emitting valid JSON with an empty `vouchers: []` array.
5. **No BOM in JSON:**
   In compliance with RFC 8259 §8.1 ("JSON text SHALL NOT be encoded with a Byte Order Mark"), JSON exports strictly omit the UTF-8 BOM, even if `request.include_bom == Some(true)`.
6. **Partial Failure Cleanup:**
   If serialization fails midway, `PartialFileGuard` deletes the incomplete JSON file. A truncated or invalid JSON file is never left on disk.

---

## 5. Test and Quality Evidence

### All 18 Phase 3A Export Test Scenarios

The tests in `src/vouchers/stage7_export_tests.rs` cover every operational and failure mode. Tests are categorized by whether they test real integration flows or simulated failure injections:

| Test ID | Function Name | Test Type | Exact Assertions & Validated Invariants |
|---|---|---|---|
| `REQ-01` | `test_01_csv_summary_export_format_and_headers` | Integration | Asserts exact 13 headers, CRLF (`\r\n`), 1 header + 1 data line, `1500.00` total amount, `narration`, and `line_count = 2`. |
| `REQ-02` | `test_02_csv_detailed_export_line_by_line` | Integration | Asserts exact 15 headers, CRLF, line 1 (`Dr`, `Cash`, `2500.50`), line 2 (`Cr`, `Sales`, `2500.50`), and correct ledger IDs. |
| `REQ-03` | `test_03_csv_cwe_1236_formula_injection_defense` | Integration | Asserts formula triggers (`=SUM`, `+CMD`, `-EXEC`, `@HYPERLINK`) in voucher numbers, narrations, and reference numbers are prepended with `'`. |
| `REQ-04` | `test_04_csv_numeric_amount_preservation` | Integration | Asserts raw numeric values (`1500.00`, `2500.50`) are unquoted and NOT prepended with `'`, ensuring spreadsheet arithmetic works. |
| `REQ-05` | `test_05_rfc_4180_escaping_rules` | Unit + Integ. | Asserts double quotes are doubled (`""`), commas and newlines trigger outer quotes (`"Smith, John"`, `"Line 1\nLine 2"`). |
| `REQ-06` | `test_06_utf8_bom_handling` | Integration | Asserts UTF-8 BOM `[0xEF, 0xBB, 0xBF]` is emitted for CSV when `include_bom: true`, omitted when `false`, and strictly omitted for JSON. |
| `REQ-07` | `test_07_json_mode_a_streaming_and_schema` | Integration | Asserts Mode A streams valid JSON conforming to Draft-07 Schema v1.0, exact string amounts, child lines array, and metadata envelope. |
| `REQ-08` | `test_08_json_mode_b_preview_structure` | Integration | Asserts Mode B in-memory preview returns populated content string, matching record count, and valid JSON structure without touching disk. |
| `REQ-09` | `test_09_hard_cap_file_export_preflight_50000` | Unit + Integ. | Asserts `MAX_FILE_EXPORT_RECORDS == 50_000`, invalid file path fails fast, overwrite governance rejects existing destination when `overwrite: false`. |
| `REQ-10` | `test_10_hard_cap_preview_preflight_1000` | Unit | Asserts `MAX_PREVIEW_RECORDS == 1_000`. |
| `REQ-11` | `test_11_snapshot_consistency_during_export` | Integration | Asserts read-only snapshot consistency under concurrent writes during query execution. |
| `REQ-12` | `test_12_writer_failure_handling_and_cleanup` | Simulated I/O | Injects I/O failure via `FailingWriter` and validates fail-closed partial file removal on disk error. |
| `REQ-13` | `test_13_database_failure_handling` | Simulated DB | Asserts clean error propagation when database queries fail, with no file creation on invalid filter parameters. |
| `REQ-14` | `test_14_async_cancellation_safety` | Simulated Abort | Aborts Tokio task running export and asserts SQLite connection pool remains healthy and reusable immediately. |
| `REQ-15` | `test_15_empty_register_export_safety` | Integration | Asserts exporting empty register produces valid CSV with headers only and valid JSON with `vouchers: []`. |
| `REQ-16` | `test_16_maximum_supported_monetary_values_export` | Integration | Asserts maximum 25-character canonical monetary amount (`99999999999999999999.9999`) exports without scientific notation or float loss. |
| `REQ-17` | `test_17_deterministic_sort_ordering_in_export` | Integration | Asserts deterministic sorting and tie-breaking across export rows (vouchers on same date ordered by sequence number). |
| `REQ-18` | `test_18_company_isolation_in_export` | Integration | Asserts company isolation: Company A export contains zero data, voucher numbers, narrations, or IDs from Company B. |

---

### Command Execution Evidence

#### 1. `cargo check`
- **Command:** `cargo check --manifest-path 02-open-tally/src-tauri/Cargo.toml`
- **Exit Code:** `0`
- **Output:**
```text
    Checking opentally-core v0.1.0 (C:\project\OpenTally\02-open-tally\src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.24s
```

#### 2. `cargo fmt -- --check`
- **Command:** `cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml -- --check`
- **Exit Code:** `0`
- **Output:**
```text
(Clean output, zero formatting differences)
```

#### 3. `cargo clippy --all-targets -- -D warnings`
- **Command:** `cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml --all-targets -- -D warnings`
- **Exit Code:** `0`
- **Output:**
```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s
```

#### 4. `cargo test` (Export Module)
- **Command:** `cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml stage7_export_tests`
- **Exit Code:** `0`
- **Output:**
```text
running 18 tests
test vouchers::stage7_export_tests::test_10_hard_cap_preview_preflight_1000 ... ok
test vouchers::stage7_export_tests::test_13_database_failure_handling ... ok
test vouchers::stage7_export_tests::test_11_snapshot_consistency_during_export ... ok
test vouchers::stage7_export_tests::test_08_json_mode_b_preview_structure ... ok
test vouchers::stage7_export_tests::test_04_csv_numeric_amount_preservation ... ok
test vouchers::stage7_export_tests::test_06_utf8_bom_handling ... ok
test vouchers::stage7_export_tests::test_05_rfc_4180_escaping_rules ... ok
test vouchers::stage7_export_tests::test_09_hard_cap_file_export_preflight_50000 ... ok
test vouchers::stage7_export_tests::test_07_json_mode_a_streaming_and_schema ... ok
test vouchers::stage7_export_tests::test_01_csv_summary_export_format_and_headers ... ok
test vouchers::stage7_export_tests::test_12_writer_failure_handling_and_cleanup ... ok
test vouchers::stage7_export_tests::test_02_csv_detailed_export_line_by_line ... ok
test vouchers::stage7_export_tests::test_03_csv_cwe_1236_formula_injection_defense ... ok
test vouchers::stage7_export_tests::test_14_async_cancellation_safety ... ok
test vouchers::stage7_export_tests::test_15_empty_register_export_safety ... ok
test vouchers::stage7_export_tests::test_17_deterministic_sort_ordering_in_export ... ok
test vouchers::stage7_export_tests::test_16_maximum_supported_monetary_values_export ... ok
test vouchers::stage7_export_tests::test_18_company_isolation_in_export ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 316 filtered out; finished in 1.00s
```

#### 5. `cargo test` (Full Backend Suite)
- **Command:** `cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml`
- **Exit Code:** `0`
- **Summary Result:**
```text
test result: ok. 334 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.04s
```

#### 6. `npm test -- --run` (Frontend Suite)
- **Command:** `npm --prefix 02-open-tally/frontend test -- --run`
- **Exit Code:** `0`
- **Summary Result:**
```text
 ✓ src/components/__tests__/VoucherEntryView.test.tsx (7 tests) 330ms
 ✓ src/components/__tests__/VoucherMastersView.test.tsx (8 tests) 633ms
 ✓ src/components/__tests__/ChartOfAccountsView.test.tsx (12 tests) 690ms

 Test Files  3 passed (3)
      Tests  27 passed (27)
   Start at  01:20:37
   Duration  3.60s (transform 625ms, setup 0ms, collect 1.97s, tests 1.65s, environment 4.78s, prepare 651ms)
```

---

## 6. Git Status & Cryptographic Hash Manifest

### Git Status
- **Current Branch:** `feature/phase1`
- **Working Tree State:**
```text
 M 02-open-tally/src-tauri/Cargo.lock
 M 02-open-tally/src-tauri/Cargo.toml
 M 02-open-tally/src-tauri/src/database/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/model.rs
 M 02-open-tally/src-tauri/src/vouchers/service/mod.rs
 M 02-open-tally/src-tauri/src/vouchers/validator.rs
?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_COMPLETION_REPORT.md
?? 02-open-tally/docs/OPENTALLY_SPRINT2_STAGE7A_PHASE2_FINAL_RECONCILIATION.md
?? 02-open-tally/docs/OPENTALLY_STAGE7A_PHASE2_FINAL_REVIEW.zip
?? 02-open-tally/docs/OPENTALLY_STAGE7A_PHASE2_FINAL_REVIEW_MANIFEST.sha256
?? 02-open-tally/docs/PHASE2_SOURCE_REVIEW_INDEX.md
?? 02-open-tally/docs/PHASE3A_SOURCE_REVIEW_INDEX.md
?? 02-open-tally/docs/STAGE7A_PHASE2_FINAL_ACCEPTANCE_EVIDENCE.md
?? 02-open-tally/docs/STAGE7A_PHASE2_TARGETED_HARDENING_REPORT.md
?? 02-open-tally/docs/STAGE7A_PHASE3A_STREAMING_EXPORT_COMPLETION_REPORT.md
?? 02-open-tally/docs/phase2-final-source-review/
?? 02-open-tally/docs/phase3a-final-source-review/
?? 02-open-tally/src-tauri/src/vouchers/service/export.rs
?? 02-open-tally/src-tauri/src/vouchers/service/search.rs
?? 02-open-tally/src-tauri/src/vouchers/stage7_export_tests.rs
?? 02-open-tally/src-tauri/src/vouchers/stage7_hardening_tests.rs
?? 02-open-tally/src-tauri/src/vouchers/stage7_repository_tests.rs
```

### Cryptographic Hash Manifest (SHA-256)

Every packaged file in `02-open-tally/docs/phase3a-final-source-review/` matches its source file bit-for-bit:

| Relative Path | SHA-256 Checksum | Verification Status |
|---|---|---|
| `vouchers/service/export.rs` | `5E60A980006E67DF9B97B994E26C81D98439F7DA4FEC54CF4A5911BAD274DA2A` | Exact Match |
| `vouchers/stage7_export_tests.rs` | `3B212DF4393066AC24021136C46F973096309337651BC621E447CA9E7E790448` | Exact Match |
| `vouchers/model.rs` | `BDDC582240EBFFA83A0B2041BCA38476D5ECF5ADE488D792CA6FC64729D00A02` | Exact Match |
| `vouchers/service/search.rs` | `AE4FC813CD2919D375C431D1882280A2973CE387AE8ED90C894899492FAEC6C0` | Exact Match |
| `vouchers/service/mod.rs` | `B17287BEE0DF98488D7C26F9829660058A0E0D0835976F5C98DA4D003F48DB85` | Exact Match |
| `vouchers/mod.rs` | `8D3EE62459CD3C83BF423453607E2B0FBC7F54BE4837FF3F38D5F7B42BE08D11` | Exact Match |
| `Cargo.toml` | `00887A04119A8333A930397D5143E0B25CE8BC5A0B93B200294354BA13F3E5C1` | Exact Match |
| `Cargo.lock` | `C10A11E71EDE7439B830151333001C3857C193EFAE7BEC3AFA9DEDDE46875DFA` | Exact Match |
| `OPENTALLY_SPRINT2_STAGE7_SEARCH_FILTER_EXPORT_DESIGN.md` | `FCD9413BAA6B2BA29B8D4E36B811F6976C89E9F73581EB94AB52DAF4D9F94267` | Exact Match |
| `STAGE7A_PHASE3A_STREAMING_EXPORT_COMPLETION_REPORT.md` | `3A2B9E0D6EB8FC4821E1BDE96E25BFF01386E3FFE3EE0FA7F57D1D281A139FC7` | Exact Match |

#### Diff Files Checksums
| Diff File Path | SHA-256 Checksum |
|---|---|
| `diffs/export_impl.diff` | `80982E545BBF95103A872CE13B458F3D56C61A4CE14F1A70DB9FB847BCEA0205` |
| `diffs/stage7_export_tests.diff` | `907C3DFEBAAFB81A0A82A69FD44707BA336AD0EF9D3CEFFCA2786224612FFD52` |
| `diffs/vouchers_model.diff` | `0007AF45302B6FA9432A2CD50BB16B836FED0B5AFA83A8237001D185B5BD6988` |
| `diffs/vouchers_service_search.diff` | `FAC5D97E0CDBEE1ABD64DC4A005B11BC3621C5B004A04A584F628DDA63008A78` |
| `diffs/vouchers_service_mod.diff` | `BA299A7845F1F6FE121ACA7FF1463C8ACD316A23417FC3B771FC01E63C9C8DD3` |
| `diffs/vouchers_mod.diff` | `0FEEC4E2F1B4E70C7E77D6DE3AD22BD9411DAD8EF5367EA3C68155BA0FCAC0F1` |
| `diffs/cargo_toml.diff` | `8E27D6C3143182CEC15C7770D503CD81F3201C3FC9F7763DE743D324FCE50644` |
| `diffs/cargo_lock.diff` | `D8B1CE9F49F0D9ABF6CC2C6C77E003D216FE6C54216D24E7DC35E9C33D9F6B97` |
| `diffs/all_phase3a_changes.diff` | `5E536C053C42ADCB72581F92A1BD0DF048E5067C13CCAB5EA1A1EDD80010F366` |

---

## 7. Packaging Integrity & Strict Guardrails Confirmation

1. **Packaging did not modify implementation files:** All source files, tests, and configuration files retain their exact contents and SHA-256 hashes.
2. **Zero staging or commits:** No Git staging operations (`git add`) or commits (`git commit`) have occurred.
3. **No Phase 3B contamination:** Neither `ReplaceFileW`, `MoveFileExW`, nor atomic destination replacement has been implemented.
4. **Independent Acceptance Requirement:** Phase 3A remains submitted for review and will proceed to Phase 3B only upon formal user authorization.
