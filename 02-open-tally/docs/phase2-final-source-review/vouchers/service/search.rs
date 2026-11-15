use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    DateBasis, PaginatedResponse, SortDirection, VoucherLineageResponse, VoucherRegisterItem,
    VoucherSearchFilter, VoucherSortField,
};
use crate::vouchers::validator::{
    normalize_canonical_amount_for_query, validate_pagination_params,
};
use sqlx::{Connection, Pool, Row, Sqlite, TransactionManager};

/// Fixed-width 30-character canonical normalization SQL expression from Section 5.3:
/// `(substr('000...0' || substr(COALESCE(v.total_amount, '0'), 1, instr(...) - 1), -25) || '.' || substr(... || '0000', 1, 4))`
pub const CANONICAL_AMOUNT_SQL: &str = "(substr('0000000000000000000000000' || substr(COALESCE(v.total_amount, '0'), 1, instr(COALESCE(v.total_amount, '0') || '.', '.') - 1), -25) || '.' || substr(substr(COALESCE(v.total_amount, '0'), instr(COALESCE(v.total_amount, '0') || '.', '.') + 1) || '0000', 1, 4))";

/// Escapes SQL LIKE special wildcard characters (`%`, `_`, `\`) for safe literal substring searching.
pub fn escape_like_wildcards(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '%' => escaped.push_str("\\%"),
            '_' => escaped.push_str("\\_"),
            '\\' => escaped.push_str("\\\\"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadOnlyLifecyclePoint {
    AfterPragmaOnBeforeBegin,
    AfterBeginBeforeClosure,
    BeforeCommit,
    AfterCommitBeforeReset,
    BeforeRollback,
    AfterSearchCountBeforeSelect,
}

#[cfg(test)]
pub(crate) type TestLifecycleHook = std::sync::Arc<
    dyn Fn(
            ReadOnlyLifecyclePoint,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

#[cfg(test)]
tokio::task_local! {
    static READ_ONLY_TEST_HOOK: TestLifecycleHook;
}

#[cfg(test)]
pub(crate) async fn with_read_only_lifecycle_hook<F, T>(hook: TestLifecycleHook, fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    READ_ONLY_TEST_HOOK.scope(hook, fut).await
}

#[cfg(test)]
async fn maybe_trigger_lifecycle_hook(point: ReadOnlyLifecyclePoint) {
    let hook = READ_ONLY_TEST_HOOK.try_with(|h| h.clone()).ok();
    if let Some(h) = hook {
        h(point).await;
    }
}

/// Cancellation- and panic-safe RAII guard around an acquired pooled SQLite connection.
///
/// Defaults to `clean_for_reuse = false`. Unless `release_clean()` is called after full
/// cleanup and hygiene verification, `Drop` marks the underlying `PoolConnection` with
/// `close_on_drop()` so dropped, cancelled, or panicked futures never return a dirty
/// (`PRAGMA query_only = ON` or open-transaction) connection back to the pool.
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

/// Executes a read-only query closure under dedicated, cancellation-safe SQLite connection lifecycle.
///
/// Implements the lifecycle matrix and cancellation hardening:
/// 1. Wraps the acquired connection immediately in `ReadOnlyConnectionGuard` (`clean_for_reuse = false`).
/// 2. Enforces engine-level read-only mode via `PRAGMA query_only = ON;`.
/// 3. Starts a SQLx-managed transaction (`Connection::begin`) so `transaction_depth` stays accurate.
/// 4. Executes the user query closure against `&mut *tx`.
/// 5. On success: `tx.commit()` -> `PRAGMA query_only = OFF;` -> verifies both `PRAGMA query_only == 0`
///    and `transaction_depth == 0`, only then calling `guard.release_clean()`.
/// 6. On error, cancellation, or unwind panic: discards/closes the connection (`close_on_drop()` /
///    `close_and_discard().await`) so the pool never receives a contaminated connection.
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

    #[cfg(test)]
    maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::AfterPragmaOnBeforeBegin).await;

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
                #[cfg(test)]
                maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::AfterBeginBeforeClosure).await;

                match f(&mut tx).await {
                    Ok(val) => {
                        #[cfg(test)]
                        maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::BeforeCommit).await;

                        match tx.commit().await {
                            Ok(()) => TxOutcome::Committed(val),
                            Err(e) => TxOutcome::TxLifecycleFailed(e),
                        }
                    }
                    Err(err) => {
                        #[cfg(test)]
                        maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::BeforeRollback).await;

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

    #[cfg(test)]
    maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::AfterCommitBeforeReset).await;

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

struct FilteredQuery {
    where_sql: String,
    filter_params: Vec<String>,
    order_by_sql: String,
}

fn build_filtered_query(
    company_id: &str,
    filter: &VoucherSearchFilter,
) -> Result<FilteredQuery, OpenTallyError> {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    // Parameter 1: Always company_id
    params.push(company_id.to_string());
    clauses.push("v.company_id = $1".to_string());

    // 1. Status Filter
    match &filter.status {
        None => {
            // Default status: posted, reversed, cancelled (excluding drafts)
            clauses.push("v.status IN ('posted', 'reversed', 'cancelled')".to_string());
        }
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.eq_ignore_ascii_case("all") {
                // Include all statuses (no filter clause)
            } else {
                params.push(trimmed.to_lowercase());
                clauses.push(format!("v.status = ${}", params.len()));
            }
        }
    }

    // 2. Voucher Type Filter
    if let Some(ref vt_id) = filter.voucher_type_id {
        let trimmed = vt_id.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!("v.voucher_type_id = ${}", params.len()));
        }
    }

    // 3. Series Filter
    if let Some(ref s_id) = filter.series_id {
        let trimmed = s_id.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!("v.series_id = ${}", params.len()));
        }
    }

    // 4. Date Basis and Date Range Filter
    let date_col = match filter.date_basis.unwrap_or(DateBasis::TransactionDate) {
        DateBasis::TransactionDate => "v.date",
        DateBasis::EffectiveDate => "v.effective_date",
    };

    if let Some(ref df) = filter.date_from {
        let trimmed = df.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!("{} >= ${}", date_col, params.len()));
        }
    }

    if let Some(ref dt) = filter.date_to {
        let trimmed = dt.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!("{} <= ${}", date_col, params.len()));
        }
    }

    // 5. Ledger Touching Filter
    if let Some(ref l_id) = filter.ledger_id {
        let trimmed = l_id.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!(
                "EXISTS (SELECT 1 FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id AND vl.ledger_id = ${})",
                params.len()
            ));
        }
    }

    // 6. Reversal Ancestry Filter
    if let Some(ref r_id) = filter.reversal_of_voucher_id {
        let trimmed = r_id.trim();
        if !trimmed.is_empty() {
            params.push(trimmed.to_string());
            clauses.push(format!("v.reversal_of_voucher_id = ${}", params.len()));
        }
    }

    // 7. Amount Range Filter (Exact 30-char canonical normalization)
    if let Some(ref min_amt) = filter.min_amount {
        let trimmed = min_amt.trim();
        if !trimmed.is_empty() {
            let norm_min = normalize_canonical_amount_for_query(trimmed, "min_amount")?;
            params.push(norm_min);
            clauses.push(format!("{} >= ${}", CANONICAL_AMOUNT_SQL, params.len()));
        }
    }

    if let Some(ref max_amt) = filter.max_amount {
        let trimmed = max_amt.trim();
        if !trimmed.is_empty() {
            let norm_max = normalize_canonical_amount_for_query(trimmed, "max_amount")?;
            params.push(norm_max);
            clauses.push(format!("{} <= ${}", CANONICAL_AMOUNT_SQL, params.len()));
        }
    }

    // 8. Text Search Filter (escaped LIKE with ESCAPE '\')
    if let Some(ref sq) = filter.search_query {
        let trimmed = sq.trim();
        if !trimmed.is_empty() {
            let escaped = escape_like_wildcards(trimmed);
            let pattern = format!("%{}%", escaped);
            params.push(pattern);
            let idx = params.len();
            clauses.push(format!(
                "(v.voucher_number LIKE ${idx} ESCAPE '\\' OR COALESCE(v.reference_no, '') LIKE ${idx} ESCAPE '\\' OR COALESCE(v.narration, '') LIKE ${idx} ESCAPE '\\')"
            ));
        }
    }

    // Deterministic Sorting allowlist
    let sort_field = filter.sort_by.unwrap_or(VoucherSortField::Date);
    let sort_dir = filter.sort_direction.unwrap_or(SortDirection::Desc);
    let dir_sql = match sort_dir {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };

    let order_by_sql = match sort_field {
        VoucherSortField::Date => match sort_dir {
            SortDirection::Desc => format!("{date_col} DESC, v.sequence_number DESC, v.id ASC"),
            SortDirection::Asc => format!("{date_col} ASC, v.sequence_number ASC, v.id ASC"),
        },
        VoucherSortField::VoucherNumber => {
            format!("v.voucher_number {dir_sql}, {date_col} DESC, v.id ASC")
        }
        VoucherSortField::TotalAmount => {
            format!("{CANONICAL_AMOUNT_SQL} {dir_sql}, {date_col} DESC, v.id ASC")
        }
        VoucherSortField::CreatedAt => {
            format!("v.created_at {dir_sql}, v.id ASC")
        }
    };

    let where_sql = clauses.join(" AND ");

    Ok(FilteredQuery {
        where_sql,
        filter_params: params,
        order_by_sql,
    })
}

/// Voucher Search Repository interface defining database-backed search operations.
pub trait VoucherSearchRepository: Send + Sync {
    /// Executes a paginated voucher register query.
    fn search_vouchers<'a>(
        &self,
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        filter: &'a VoucherSearchFilter,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<PaginatedResponse<VoucherRegisterItem>, OpenTallyError>,
                > + Send
                + 'a,
        >,
    >;

    /// Executes a bidirectional lineage traversal query up to depth 11 for definite truncation detection.
    fn get_voucher_lineage<'a>(
        &self,
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        voucher_id: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<VoucherLineageResponse, OpenTallyError>>
                + Send
                + 'a,
        >,
    >;
}

/// Concrete SQLite implementation of `VoucherSearchRepository`.
pub struct SqliteVoucherSearchRepository;

/// Computes `ceil(total / page_size)` safely with checked arithmetic.
///
/// Rejects negative `total`, zero `page_size`, and `total_pages > u32::MAX` without wrapping
/// or silently truncating via `as u32`.
pub(crate) fn compute_total_pages(total: i64, page_size: u32) -> Result<u32, OpenTallyError> {
    if page_size == 0 {
        return Err(OpenTallyError::Validation(
            "Page size must be greater than or equal to 1".to_string(),
        ));
    }
    let total_u64 = u64::try_from(total).map_err(|_| {
        OpenTallyError::Internal(format!(
            "Search count returned invalid negative total: {}",
            total
        ))
    })?;
    if total_u64 == 0 {
        return Ok(1);
    }
    let pages_u64 = total_u64.div_ceil(u64::from(page_size));
    u32::try_from(pages_u64).map_err(|_| {
        OpenTallyError::Validation(format!(
            "Total page count {} exceeds maximum supported u32 range",
            pages_u64
        ))
    })
}

impl SqliteVoucherSearchRepository {
    pub fn search_vouchers_impl<'a>(
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        filter: &'a VoucherSearchFilter,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<PaginatedResponse<VoucherRegisterItem>, OpenTallyError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            // Enforce pagination safety directly at repository boundary before querying
            let (page, page_size, offset) =
                validate_pagination_params(filter.page, filter.page_size)?;

            let filtered = build_filtered_query(company_id, filter)?;

            // 1. Count query
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

            #[cfg(test)]
            maybe_trigger_lifecycle_hook(ReadOnlyLifecyclePoint::AfterSearchCountBeforeSelect)
                .await;

            let total_pages = compute_total_pages(total, page_size)?;

            if total == 0 {
                return Ok(PaginatedResponse {
                    items: Vec::new(),
                    total: 0,
                    page,
                    page_size,
                    total_pages,
                });
            }

            // 2. Select query
            let limit_param_idx = filtered.filter_params.len() + 1;
            let offset_param_idx = filtered.filter_params.len() + 2;

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
                ORDER BY {}
                LIMIT ${} OFFSET ${}",
                filtered.where_sql,
                filtered.order_by_sql,
                limit_param_idx,
                offset_param_idx
            );

            let mut select_query = sqlx::query_as::<_, VoucherRegisterItem>(&select_sql);
            for p in &filtered.filter_params {
                select_query = select_query.bind(p);
            }
            select_query = select_query.bind(i64::from(page_size)).bind(offset);

            let items = select_query
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

            Ok(PaginatedResponse {
                items,
                total,
                page,
                page_size,
                total_pages,
            })
        })
    }

    pub fn get_voucher_lineage_impl<'a>(
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        voucher_id: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<VoucherLineageResponse, OpenTallyError>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM vouchers WHERE company_id = $1 AND id = $2)",
            )
            .bind(company_id)
            .bind(voucher_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if !exists {
                return Err(OpenTallyError::NotFound(format!(
                    "Voucher '{}' not found for company '{}'",
                    voucher_id, company_id
                )));
            }

            let lineage_sql = "WITH RECURSIVE voucher_lineage(id, company_id, parent_id, depth, path) AS (
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
                (SELECT COUNT(*) FROM voucher_lines vl_sub WHERE vl_sub.company_id = v.company_id AND vl_sub.voucher_id = v.id) AS line_count,
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
                v.updated_at,
                MIN(vl.depth) AS min_depth
            FROM voucher_lineage vl
            JOIN vouchers v ON vl.company_id = v.company_id AND vl.id = v.id
            JOIN voucher_types vt ON vt.company_id = v.company_id AND vt.id = v.voucher_type_id
            JOIN voucher_number_series s ON s.company_id = v.company_id AND s.id = v.series_id
            WHERE vl.depth <= 11
            GROUP BY v.id
            ORDER BY v.date ASC, v.sequence_number ASC, v.id ASC;";

            let rows = sqlx::query(lineage_sql)
                .bind(company_id)
                .bind(voucher_id)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

            let mut lineage_items: Vec<(VoucherRegisterItem, i64)> = Vec::with_capacity(rows.len());
            for row in rows {
                let item = VoucherRegisterItem {
                    id: row.try_get("id").map_err(OpenTallyError::Database)?,
                    company_id: row
                        .try_get("company_id")
                        .map_err(OpenTallyError::Database)?,
                    voucher_type_id: row
                        .try_get("voucher_type_id")
                        .map_err(OpenTallyError::Database)?,
                    voucher_type_name: row
                        .try_get("voucher_type_name")
                        .map_err(OpenTallyError::Database)?,
                    series_id: row.try_get("series_id").map_err(OpenTallyError::Database)?,
                    series_name: row
                        .try_get("series_name")
                        .map_err(OpenTallyError::Database)?,
                    voucher_number: row
                        .try_get("voucher_number")
                        .map_err(OpenTallyError::Database)?,
                    date: row.try_get("date").map_err(OpenTallyError::Database)?,
                    effective_date: row
                        .try_get("effective_date")
                        .map_err(OpenTallyError::Database)?,
                    status: row.try_get("status").map_err(OpenTallyError::Database)?,
                    reference_no: row
                        .try_get("reference_no")
                        .map_err(OpenTallyError::Database)?,
                    reference_date: row
                        .try_get("reference_date")
                        .map_err(OpenTallyError::Database)?,
                    narration: row.try_get("narration").map_err(OpenTallyError::Database)?,
                    total_amount: row
                        .try_get("total_amount")
                        .map_err(OpenTallyError::Database)?,
                    line_count: row
                        .try_get("line_count")
                        .map_err(OpenTallyError::Database)?,
                    reversal_of_voucher_id: row
                        .try_get("reversal_of_voucher_id")
                        .map_err(OpenTallyError::Database)?,
                    reversal_voucher_id: row
                        .try_get("reversal_voucher_id")
                        .map_err(OpenTallyError::Database)?,
                    cancellation_reason: row
                        .try_get("cancellation_reason")
                        .map_err(OpenTallyError::Database)?,
                    created_at: row
                        .try_get("created_at")
                        .map_err(OpenTallyError::Database)?,
                    updated_at: row
                        .try_get("updated_at")
                        .map_err(OpenTallyError::Database)?,
                };
                let min_depth: i64 = row.try_get("min_depth").map_err(OpenTallyError::Database)?;
                lineage_items.push((item, min_depth));
            }

            let has_exceeded_depth_limit = lineage_items.iter().any(|(_, depth)| *depth > 10);
            let max_depth_reached = lineage_items
                .iter()
                .filter_map(|(_, depth)| {
                    if *depth >= 0 && *depth <= 10 {
                        u32::try_from(*depth).ok()
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);

            let items: Vec<VoucherRegisterItem> = lineage_items
                .into_iter()
                .filter(|(_, depth)| *depth <= 10)
                .map(|(item, _)| item)
                .collect();

            Ok(VoucherLineageResponse {
                items,
                has_exceeded_depth_limit,
                max_depth_reached,
            })
        })
    }
}

impl VoucherSearchRepository for SqliteVoucherSearchRepository {
    fn search_vouchers<'a>(
        &self,
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        filter: &'a VoucherSearchFilter,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<PaginatedResponse<VoucherRegisterItem>, OpenTallyError>,
                > + Send
                + 'a,
        >,
    > {
        Self::search_vouchers_impl(conn, company_id, filter)
    }

    fn get_voucher_lineage<'a>(
        &self,
        conn: &'a mut sqlx::SqliteConnection,
        company_id: &'a str,
        voucher_id: &'a str,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<VoucherLineageResponse, OpenTallyError>>
                + Send
                + 'a,
        >,
    > {
        Self::get_voucher_lineage_impl(conn, company_id, voucher_id)
    }
}

/// Authoritative service handling voucher search and lineage queries under read-only snapshot consistency.
pub struct VoucherSearchService;

impl VoucherSearchService {
    /// Executes a search across the voucher register for the specified active company.
    pub async fn search_vouchers(
        pool: &Pool<Sqlite>,
        company_id: &str,
        filter: VoucherSearchFilter,
    ) -> Result<PaginatedResponse<VoucherRegisterItem>, OpenTallyError> {
        // Step 1: Pure domain validation
        crate::vouchers::validator::validate_search_filter(&filter)?;

        let company_id = company_id.to_string();
        // Step 2: Read-only connection lifecycle
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let repo = SqliteVoucherSearchRepository;
                repo.search_vouchers(conn, &company_id, &filter).await
            })
        })
        .await
    }

    /// Fetches the bidirectional lineage graph of related vouchers for the specified voucher ID.
    pub async fn get_voucher_lineage(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<VoucherLineageResponse, OpenTallyError> {
        if voucher_id.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "voucher_id cannot be empty".to_string(),
            ));
        }

        let company_id = company_id.to_string();
        let voucher_id = voucher_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                let repo = SqliteVoucherSearchRepository;
                repo.get_voucher_lineage(conn, &company_id, &voucher_id)
                    .await
            })
        })
        .await
    }
}
