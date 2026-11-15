//! Sprint 2 Stage 7A: Streaming Export Core Engine
//!
//! Implements production-grade, bounded-memory streaming export for voucher registers:
//! - CSV Summary (Day Book format, 13 columns, RFC 4180 CRLF, CWE-1236 defense, UTF-8 BOM)
//! - CSV Detailed (Audit Dump format, 15 columns, RFC 4180 CRLF, CWE-1236 defense, UTF-8 BOM)
//! - JSON Streaming Mode A (Draft-07 JSON Schema v1.0, O(1) memory framing, exact decimals)
//! - JSON Preview Mode B (In-memory materialized representation, capped at 1,000 records)
//! - Hard cap preflight checks (50,000 records for file export, 1,000 records for preview)
//! - Snapshot-consistent execution on a single dedicated read-only connection via `execute_read_only_query`
//! - Fail-closed partial export cleanup and cancellation safety

use sqlx::{Pool, Row, Sqlite};
use std::path::Path;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    ExportFormat, VoucherCompositeExportItem, VoucherExportFileRequest, VoucherExportFileResult,
    VoucherExportLineItem, VoucherExportPreviewRequest, VoucherExportPreviewResult,
    VoucherRegisterItem, VoucherSearchFilter,
};
use crate::vouchers::service::search::{build_filtered_query, execute_read_only_query};

/// UTF-8 Byte Order Mark (BOM) sequence `[0xEF, 0xBB, 0xBF]`.
pub const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// RFC 4180 standard CRLF record delimiter.
pub const CRLF: &str = "\r\n";

/// Hard limit for streaming file export (Mode A).
pub const MAX_FILE_EXPORT_RECORDS: usize = 50_000;

/// Hard limit for in-memory preview export (Mode B).
pub const MAX_PREVIEW_RECORDS: usize = 1_000;

/// CSV Summary header line (Day Book format, 13 columns).
pub const CSV_SUMMARY_HEADERS: &str = "Voucher Number,Date,Effective Date,Voucher Type,Series,Reference No,Reference Date,Status,Total Amount,Line Count,Narration,Cancellation Reason,Reversal Of";

/// CSV Detailed header line (Audit Dump format, 15 columns).
pub const CSV_DETAILED_HEADERS: &str = "Voucher Number,Date,Effective Date,Voucher Type,Series,Line No,Ledger ID,Ledger Name,Entry Type,Amount,Line Narration,Reference No,Status,Cancellation Reason,Reversal Of";

// ============================================================================
// Sanitization & RFC 4180 Escaping Helpers
// ============================================================================

/// Sanitizes untrusted user-controlled text cells against CWE-1236 CSV Formula Injection.
///
/// Strips leading ASCII whitespace and ASCII control characters (0..=31).
/// If the raw string begins with `\t`, `\r`, `\n` or if the trimmed string begins with
/// `=`, `+`, `-`, or `@`, prepends a single quote (`'`) to neutralize spreadsheet execution.
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

/// Escapes a cell value according to RFC 4180 specifications.
///
/// If the value contains a comma (`,`), double quote (`"`), line feed (`\n`), or
/// carriage return (`\r`), double quotes are escaped as `""` and the entire cell
/// is enclosed in double quotes.
pub fn escape_rfc4180(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}

/// Formats a cell value, applying CWE-1236 formula sanitization for user text
/// followed by RFC 4180 escaping.
fn format_csv_cell(value: Option<&str>, is_user_text: bool) -> String {
    match value {
        None => String::new(),
        Some(val) => {
            if is_user_text {
                let sanitized = sanitize_text_cell(val);
                escape_rfc4180(&sanitized)
            } else {
                escape_rfc4180(val)
            }
        }
    }
}

// ============================================================================
// RAII Guards & Utilities
// ============================================================================

/// RAII guard to guarantee partial file cleanup on error, panic, or async task cancellation.
///
/// On Windows, open file handles prevent file deletion (`ERROR_SHARING_VIOLATION`).
/// Therefore, Rust's reverse-order drop semantics must be relied upon:
/// declare `guard` before `writer`/`file`, so that `writer`/`file` drops and closes first,
/// followed by `guard` which safely removes the incomplete file if not committed.
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

// ============================================================================
// Streaming Export Core Engine
// ============================================================================

/// Voucher Export Service coordinating streaming serialization, read-only
/// snapshot consistency, preflight limits, and fail-closed error handling.
pub struct VoucherExportService;

impl VoucherExportService {
    /// Generates an in-memory preview of exported voucher data (Mode B).
    ///
    /// Preflights the query under a dedicated read-only transaction:
    /// - Strictly capped at 1,000 records (`MAX_PREVIEW_RECORDS`).
    /// - Serializes directly into an in-memory buffer.
    /// - Returns `VoucherExportPreviewResult`.
    pub async fn export_preview(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: VoucherExportPreviewRequest,
    ) -> Result<VoucherExportPreviewResult, OpenTallyError> {
        let company_id_owned = company_id.to_string();
        let format = request.format;
        let include_bom = request.include_bom.unwrap_or(true);
        let filter = request.filter;

        execute_read_only_query(pool, |conn| {
            let filter = filter.clone();
            let company_id = company_id_owned.clone();
            Box::pin(async move {
                let filtered = build_filtered_query(&company_id, &filter)?;

                // Preflight count query
                let count_sql = format!(
                    "SELECT COUNT(*) FROM vouchers v WHERE {}",
                    filtered.where_sql
                );
                let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
                for p in &filtered.filter_params {
                    count_query = count_query.bind(p);
                }

                let total: i64 = count_query
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                let record_count = usize::try_from(total)
                    .map_err(|_| OpenTallyError::Validation("Record count overflow".to_string()))?;

                if record_count > MAX_PREVIEW_RECORDS {
                    return Err(OpenTallyError::Validation(format!(
                        "Preview record count ({}) exceeds maximum limit of {} records",
                        record_count, MAX_PREVIEW_RECORDS
                    )));
                }

                let mut buffer = Vec::new();
                Self::export_stream_impl(
                    conn,
                    &company_id,
                    &filter,
                    format,
                    include_bom,
                    record_count,
                    &mut buffer,
                )
                .await?;

                let content = String::from_utf8(buffer).map_err(|e| {
                    OpenTallyError::Internal(format!(
                        "Export preview UTF-8 conversion error: {}",
                        e
                    ))
                })?;

                let exported_at = chrono::Utc::now().to_rfc3339();

                Ok(VoucherExportPreviewResult {
                    format,
                    content,
                    record_count,
                    exported_at,
                })
            })
        })
        .await
    }

    /// Exports voucher register data to a local file destination (Mode A).
    ///
    /// Enforces:
    /// - Path traversal checks and file existence governance (`overwrite: bool`).
    /// - Hard cap of 50,000 records (`MAX_FILE_EXPORT_RECORDS`) in preflight.
    /// - Streaming directly to disk via 64 KB `tokio::io::BufWriter` with $O(1)$ memory.
    /// - Mandatory two-stage flush (`flush().await`) and sync (`sync_all().await`).
    /// - Automatic deletion of incomplete partial files upon any writer or database error.
    pub async fn export_to_file(
        pool: &Pool<Sqlite>,
        company_id: &str,
        request: VoucherExportFileRequest,
    ) -> Result<VoucherExportFileResult, OpenTallyError> {
        let target_path_str = match &request.target_path {
            Some(p) => p.trim().to_string(),
            None => {
                return Err(OpenTallyError::Validation(
                    "target_path is required for file export".to_string(),
                ));
            }
        };

        if target_path_str.is_empty() {
            return Err(OpenTallyError::Validation(
                "target_path cannot be empty".to_string(),
            ));
        }

        // Basic path sanitization: reject path traversal
        if target_path_str.contains("..") || target_path_str.contains('\0') {
            return Err(OpenTallyError::Validation(
                "Path traversal sequences are not allowed in export path".to_string(),
            ));
        }

        let target_path = Path::new(&target_path_str);

        // Validate extension matches requested format
        let expected_ext = match request.format {
            ExportFormat::CsvSummary | ExportFormat::CsvDetailed => "csv",
            ExportFormat::Json => "json",
        };
        match target_path.extension().and_then(|e| e.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case(expected_ext) => {}
            _ => {
                return Err(OpenTallyError::Validation(format!(
                    "Invalid file extension: expected '.{}'",
                    expected_ext
                )));
            }
        }

        // Overwrite governance
        let overwrite = request.overwrite.unwrap_or(false);
        if target_path.exists() && !overwrite {
            return Err(OpenTallyError::Validation(format!(
                "Destination file '{}' already exists and overwrite is false",
                target_path.display()
            )));
        }

        // Ensure parent directory exists
        if let Some(parent) = target_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(OpenTallyError::Io)?;
            }
        }

        let company_id_owned = company_id.to_string();
        let format = request.format;
        let include_bom = request.include_bom.unwrap_or(true);
        let filter = request.filter;

        let export_result: Result<(usize, u64), OpenTallyError> =
            execute_read_only_query(pool, |conn| {
                let filter = filter.clone();
                let company_id = company_id_owned.clone();
                let target_path_buf = target_path.to_path_buf();
                Box::pin(async move {
                    let filtered = build_filtered_query(&company_id, &filter)?;

                    // Step 1: Count preflight
                    let count_sql = format!(
                        "SELECT COUNT(*) FROM vouchers v WHERE {}",
                        filtered.where_sql
                    );
                    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
                    for p in &filtered.filter_params {
                        count_query = count_query.bind(p);
                    }

                    let total: i64 = count_query
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(OpenTallyError::Database)?;

                    let record_count = usize::try_from(total).map_err(|_| {
                        OpenTallyError::Validation("Record count overflow".to_string())
                    })?;

                    if record_count > MAX_FILE_EXPORT_RECORDS {
                        return Err(OpenTallyError::Validation(format!(
                            "Export record count ({}) exceeds maximum limit of {} records",
                            record_count, MAX_FILE_EXPORT_RECORDS
                        )));
                    }

                    // Step 2: Open output file with 64 KB BufWriter and RAII partial cleanup guard
                    let mut guard = PartialFileGuard::new(target_path_buf.clone());
                    let file = tokio::fs::File::create(&target_path_buf)
                        .await
                        .map_err(OpenTallyError::Io)?;
                    let mut writer = BufWriter::with_capacity(64 * 1024, file);

                    // Step 3: Stream data into writer
                    let stream_res = Self::export_stream_impl(
                        conn,
                        &company_id,
                        &filter,
                        format,
                        include_bom,
                        record_count,
                        &mut writer,
                    )
                    .await;

                    if let Err(e) = stream_res {
                        drop(writer);
                        return Err(e);
                    }

                    // Step 4: Two-stage durability flush and sync
                    if let Err(e) = writer.flush().await {
                        drop(writer);
                        return Err(OpenTallyError::Io(e));
                    }

                    let file = writer.into_inner();
                    if let Err(e) = file.sync_all().await {
                        drop(file);
                        return Err(OpenTallyError::Io(e));
                    }

                    let metadata = file.metadata().await.map_err(OpenTallyError::Io)?;
                    let file_size = metadata.len();
                    drop(file);
                    guard.commit();

                    Ok((record_count, file_size))
                })
            })
            .await;

        match export_result {
            Ok((record_count, file_size_bytes)) => {
                let filename = target_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let exported_at = chrono::Utc::now().to_rfc3339();

                Ok(VoucherExportFileResult {
                    format,
                    file_path: target_path.to_string_lossy().to_string(),
                    filename,
                    file_size_bytes,
                    record_count,
                    exported_at,
                })
            }
            Err(e) => {
                // Ensure partial file is removed on outer error if left on disk
                if target_path.exists() {
                    let _ = tokio::fs::remove_file(target_path).await;
                }
                Err(e)
            }
        }
    }

    /// Generic streaming entry point that serializes directly into any asynchronous writer.
    ///
    /// Used by both `export_preview` and `export_to_file`.
    pub async fn export_to_writer<W: AsyncWrite + Unpin + Send + 'static>(
        pool: &Pool<Sqlite>,
        company_id: &str,
        filter: &VoucherSearchFilter,
        format: ExportFormat,
        include_bom: bool,
        mut writer: W,
    ) -> Result<usize, OpenTallyError> {
        let company_id_owned = company_id.to_string();
        let filter_clone = filter.clone();

        execute_read_only_query(pool, |conn| {
            let company_id = company_id_owned.clone();
            let filter = filter_clone.clone();
            Box::pin(async move {
                let filtered = build_filtered_query(&company_id, &filter)?;

                let count_sql = format!(
                    "SELECT COUNT(*) FROM vouchers v WHERE {}",
                    filtered.where_sql
                );
                let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
                for p in &filtered.filter_params {
                    count_query = count_query.bind(p);
                }

                let total: i64 = count_query
                    .fetch_one(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                let record_count = usize::try_from(total)
                    .map_err(|_| OpenTallyError::Validation("Record count overflow".to_string()))?;

                if record_count > MAX_FILE_EXPORT_RECORDS {
                    return Err(OpenTallyError::Validation(format!(
                        "Export record count ({}) exceeds maximum limit of {} records",
                        record_count, MAX_FILE_EXPORT_RECORDS
                    )));
                }

                Self::export_stream_impl(
                    conn,
                    &company_id,
                    &filter,
                    format,
                    include_bom,
                    record_count,
                    &mut writer,
                )
                .await?;

                writer.flush().await.map_err(OpenTallyError::Io)?;
                Ok(record_count)
            })
        })
        .await
    }

    /// Internal streaming dispatcher for active read-only connection and writer.
    async fn export_stream_impl<W: AsyncWrite + Unpin + Send>(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        filter: &VoucherSearchFilter,
        format: ExportFormat,
        include_bom: bool,
        record_count: usize,
        writer: &mut W,
    ) -> Result<(), OpenTallyError> {
        match format {
            ExportFormat::CsvSummary => {
                Self::stream_csv_summary(conn, company_id, filter, include_bom, writer).await
            }
            ExportFormat::CsvDetailed => {
                Self::stream_csv_detailed(conn, company_id, filter, include_bom, writer).await
            }
            ExportFormat::Json => {
                Self::stream_json(conn, company_id, filter, record_count, writer).await
            }
        }
    }

    // ========================================================================
    // CSV Summary Serializer
    // ========================================================================

    async fn stream_csv_summary<W: AsyncWrite + Unpin + Send>(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        filter: &VoucherSearchFilter,
        include_bom: bool,
        writer: &mut W,
    ) -> Result<(), OpenTallyError> {
        if include_bom {
            writer
                .write_all(UTF8_BOM)
                .await
                .map_err(OpenTallyError::Io)?;
        }

        // Write Header
        writer
            .write_all(format!("{}{}", CSV_SUMMARY_HEADERS, CRLF).as_bytes())
            .await
            .map_err(OpenTallyError::Io)?;

        let filtered = build_filtered_query(company_id, filter)?;

        let select_sql = format!(
            "SELECT 
                v.id,
                v.company_id,
                v.voucher_type_id,
                vt.name AS voucher_type_name,
                v.series_id,
                s.series_name,
                v.voucher_number,
                v.date,
                v.effective_date,
                v.status,
                v.reference_no,
                v.reference_date,
                v.narration,
                v.total_amount,
                (SELECT COUNT(*) FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id) AS line_count,
                v.reversal_of_voucher_id,
                (SELECT r.id FROM vouchers r WHERE r.company_id = v.company_id AND r.reversal_of_voucher_id = v.id) AS reversal_voucher_id,
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
                END AS cancellation_reason,
                v.created_at,
                v.updated_at
            FROM vouchers v
            JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
            JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id
            WHERE {}
            ORDER BY {}",
            filtered.where_sql,
            filtered.order_by_sql
        );

        let mut select_query = sqlx::query_as::<_, VoucherRegisterItem>(&select_sql);
        for p in &filtered.filter_params {
            select_query = select_query.bind(p);
        }

        use futures_util::StreamExt;
        let mut row_stream = select_query.fetch(&mut *conn);

        while let Some(row_res) = row_stream.next().await {
            let item = row_res.map_err(OpenTallyError::Database)?;

            // 13 Columns matching Section 8.2A
            let cells = [
                format_csv_cell(Some(&item.voucher_number), true),
                format_csv_cell(Some(&item.date), false),
                format_csv_cell(Some(&item.effective_date), false),
                format_csv_cell(Some(&item.voucher_type_name), true),
                format_csv_cell(Some(&item.series_name), true),
                format_csv_cell(item.reference_no.as_deref(), true),
                format_csv_cell(item.reference_date.as_deref(), false),
                format_csv_cell(Some(&item.status), false),
                format_csv_cell(item.total_amount.as_deref(), false), // Raw numeric preserved!
                format_csv_cell(Some(&item.line_count.to_string()), false),
                format_csv_cell(item.narration.as_deref(), true),
                format_csv_cell(item.cancellation_reason.as_deref(), true),
                format_csv_cell(item.reversal_of_voucher_id.as_deref(), true),
            ];

            let row_str = format!("{}{}", cells.join(","), CRLF);
            writer
                .write_all(row_str.as_bytes())
                .await
                .map_err(OpenTallyError::Io)?;
        }

        Ok(())
    }

    // ========================================================================
    // CSV Detailed Serializer
    // ========================================================================

    async fn stream_csv_detailed<W: AsyncWrite + Unpin + Send>(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        filter: &VoucherSearchFilter,
        include_bom: bool,
        writer: &mut W,
    ) -> Result<(), OpenTallyError> {
        if include_bom {
            writer
                .write_all(UTF8_BOM)
                .await
                .map_err(OpenTallyError::Io)?;
        }

        // Write Header
        writer
            .write_all(format!("{}{}", CSV_DETAILED_HEADERS, CRLF).as_bytes())
            .await
            .map_err(OpenTallyError::Io)?;

        let filtered = build_filtered_query(company_id, filter)?;

        let detailed_sql = format!(
            "SELECT 
                v.voucher_number,
                v.date,
                v.effective_date,
                vt.name AS voucher_type_name,
                s.series_name,
                vl.line_number,
                vl.ledger_id,
                l.name AS ledger_name,
                vl.entry_type,
                vl.amount AS line_amount,
                vl.narration AS line_narration,
                v.reference_no,
                v.status,
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
                END AS cancellation_reason,
                v.reversal_of_voucher_id AS reversal_of
            FROM vouchers v
            JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
            JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id
            LEFT JOIN voucher_lines vl ON vl.company_id = v.company_id AND vl.voucher_id = v.id
            LEFT JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
            WHERE {}
            ORDER BY {}, vl.line_number ASC",
            filtered.where_sql, filtered.order_by_sql
        );

        let mut select_query = sqlx::query(&detailed_sql);
        for p in &filtered.filter_params {
            select_query = select_query.bind(p);
        }

        use futures_util::StreamExt;
        let mut row_stream = select_query.fetch(&mut *conn);

        while let Some(row_res) = row_stream.next().await {
            let row = row_res.map_err(OpenTallyError::Database)?;

            let voucher_number: String = row.get("voucher_number");
            let date: String = row.get("date");
            let effective_date: String = row.get("effective_date");
            let voucher_type_name: String = row.get("voucher_type_name");
            let series_name: String = row.get("series_name");
            let line_number: Option<i64> = row.get("line_number");
            let ledger_id: Option<String> = row.get("ledger_id");
            let ledger_name: Option<String> = row.get("ledger_name");
            let entry_type: Option<String> = row.get("entry_type");
            let line_amount: Option<String> = row.get("line_amount");
            let line_narration: Option<String> = row.get("line_narration");
            let reference_no: Option<String> = row.get("reference_no");
            let status: String = row.get("status");
            let cancellation_reason: Option<String> = row.get("cancellation_reason");
            let reversal_of: Option<String> = row.get("reversal_of");

            let line_num_str = line_number.map(|n| n.to_string());

            // 15 Columns matching Section 8.2B
            let cells = [
                format_csv_cell(Some(&voucher_number), true),
                format_csv_cell(Some(&date), false),
                format_csv_cell(Some(&effective_date), false),
                format_csv_cell(Some(&voucher_type_name), true),
                format_csv_cell(Some(&series_name), true),
                format_csv_cell(line_num_str.as_deref(), false),
                format_csv_cell(ledger_id.as_deref(), true),
                format_csv_cell(ledger_name.as_deref(), true),
                format_csv_cell(entry_type.as_deref(), false),
                format_csv_cell(line_amount.as_deref(), false), // Raw numeric preserved!
                format_csv_cell(line_narration.as_deref(), true),
                format_csv_cell(reference_no.as_deref(), true),
                format_csv_cell(Some(&status), false),
                format_csv_cell(cancellation_reason.as_deref(), true),
                format_csv_cell(reversal_of.as_deref(), true),
            ];

            let row_str = format!("{}{}", cells.join(","), CRLF);
            writer
                .write_all(row_str.as_bytes())
                .await
                .map_err(OpenTallyError::Io)?;
        }

        Ok(())
    }

    // ========================================================================
    // JSON Stream Serializer (Mode A)
    // ========================================================================

    async fn stream_json<W: AsyncWrite + Unpin + Send>(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        filter: &VoucherSearchFilter,
        total_records: usize,
        writer: &mut W,
    ) -> Result<(), OpenTallyError> {
        let exported_at = chrono::Utc::now().to_rfc3339();
        let filter_json = serde_json::to_string(filter).map_err(OpenTallyError::Serialization)?;

        // Write opening framing
        let opening = format!(
            "{{\"schema_version\":\"1.0\",\"company_id\":\"{}\",\"exported_at\":\"{}\",\"total_records\":{},\"filter_applied\":{},\"vouchers\":[",
            company_id, exported_at, total_records, filter_json
        );
        writer
            .write_all(opening.as_bytes())
            .await
            .map_err(OpenTallyError::Io)?;

        if total_records == 0 {
            writer.write_all(b"]}").await.map_err(OpenTallyError::Io)?;
            return Ok(());
        }

        let filtered = build_filtered_query(company_id, filter)?;

        let json_query_sql = format!(
            "SELECT 
                v.id AS voucher_id,
                v.voucher_number,
                v.voucher_type_id,
                vt.name AS voucher_type_name,
                v.series_id,
                s.series_name,
                v.date,
                v.effective_date,
                v.status,
                v.reference_no,
                v.reference_date,
                v.narration AS voucher_narration,
                v.total_amount,
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
                END AS cancellation_reason,
                v.reversal_of_voucher_id,
                (SELECT r.id FROM vouchers r WHERE r.company_id = v.company_id AND r.reversal_of_voucher_id = v.id) AS reversal_voucher_id,
                v.created_at,
                vl.line_number,
                vl.ledger_id,
                l.name AS ledger_name,
                vl.entry_type,
                vl.amount AS line_amount,
                vl.narration AS line_narration
            FROM vouchers v
            JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
            JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id
            LEFT JOIN voucher_lines vl ON vl.company_id = v.company_id AND vl.voucher_id = v.id
            LEFT JOIN ledgers l ON l.company_id = vl.company_id AND l.id = vl.ledger_id
            WHERE {}
            ORDER BY {}, vl.line_number ASC",
            filtered.where_sql,
            filtered.order_by_sql
        );

        let mut select_query = sqlx::query(&json_query_sql);
        for p in &filtered.filter_params {
            select_query = select_query.bind(p);
        }

        use futures_util::StreamExt;
        let mut row_stream = select_query.fetch(&mut *conn);

        let mut current_voucher: Option<VoucherCompositeExportItem> = None;
        let mut first_voucher_emitted = false;

        while let Some(row_res) = row_stream.next().await {
            let row = row_res.map_err(OpenTallyError::Database)?;

            let v_id: String = row.get("voucher_id");
            let line_num: Option<i64> = row.get("line_number");

            let line_item = line_num.map(|ln| VoucherExportLineItem {
                line_number: ln,
                ledger_id: row
                    .get::<Option<String>, _>("ledger_id")
                    .unwrap_or_default(),
                ledger_name: row
                    .get::<Option<String>, _>("ledger_name")
                    .unwrap_or_default(),
                entry_type: row
                    .get::<Option<String>, _>("entry_type")
                    .unwrap_or_default(),
                amount: row
                    .get::<Option<String>, _>("line_amount")
                    .unwrap_or_default(),
                narration: row.get("line_narration"),
            });

            match &mut current_voucher {
                Some(curr) if curr.id == v_id => {
                    // Same voucher, append line if line exists
                    if let Some(l) = line_item {
                        curr.lines.push(l);
                    }
                }
                _ => {
                    // New voucher encountered; flush previous voucher
                    if let Some(finished) = current_voucher.take() {
                        if first_voucher_emitted {
                            writer.write_all(b",").await.map_err(OpenTallyError::Io)?;
                        }
                        let bytes =
                            serde_json::to_vec(&finished).map_err(OpenTallyError::Serialization)?;
                        writer.write_all(&bytes).await.map_err(OpenTallyError::Io)?;
                        first_voucher_emitted = true;
                    }

                    // Start accumulating new voucher
                    let mut lines = Vec::new();
                    if let Some(l) = line_item {
                        lines.push(l);
                    }

                    current_voucher = Some(VoucherCompositeExportItem {
                        id: v_id,
                        voucher_number: row.get("voucher_number"),
                        voucher_type_id: row.get("voucher_type_id"),
                        voucher_type_name: row.get("voucher_type_name"),
                        series_id: row.get("series_id"),
                        series_name: row.get("series_name"),
                        date: row.get("date"),
                        effective_date: row.get("effective_date"),
                        status: row.get("status"),
                        reference_no: row.get("reference_no"),
                        reference_date: row.get("reference_date"),
                        narration: row.get("voucher_narration"),
                        total_amount: row.get("total_amount"),
                        cancellation_reason: row.get("cancellation_reason"),
                        reversal_of_voucher_id: row.get("reversal_of_voucher_id"),
                        reversal_voucher_id: row.get("reversal_voucher_id"),
                        created_at: row.get("created_at"),
                        lines,
                    });
                }
            }
        }

        // Flush final voucher if present
        if let Some(finished) = current_voucher {
            if first_voucher_emitted {
                writer.write_all(b",").await.map_err(OpenTallyError::Io)?;
            }
            let bytes = serde_json::to_vec(&finished).map_err(OpenTallyError::Serialization)?;
            writer.write_all(&bytes).await.map_err(OpenTallyError::Io)?;
        }

        // Write closing framing
        writer.write_all(b"]}").await.map_err(OpenTallyError::Io)?;

        Ok(())
    }
}
