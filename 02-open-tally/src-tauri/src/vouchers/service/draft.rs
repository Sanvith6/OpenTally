use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    AddVoucherLineRequest, CreateVoucherDraftRequest, PaginatedResponse,
    ReorderVoucherLinesRequest, SaveVoucherDraftRequest, UpdateVoucherDraftRequest,
    UpdateVoucherLineRequest, Voucher, VoucherFilter, VoucherLine, VoucherSummary,
    VoucherWithLines,
};
use crate::vouchers::validator::{
    validate_canonical_amount, validate_entry_type, validate_line_number, validate_narration,
    validate_voucher_date, validate_voucher_line_count, LINE_RENUMBER_OFFSET, MAX_LINE_NUMBER,
    MAX_VOUCHER_LINES,
};
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use uuid::Uuid;

/// Service managing draft voucher headers and multi-leg line items (Sprint 2 Stage 4).
pub struct VoucherDraftService;

impl VoucherDraftService {
    /// Retrieve company metadata (books_beginning_from, currency_decimal_places).
    async fn get_company_meta(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
    ) -> Result<(String, u32), OpenTallyError> {
        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT books_beginning_from, currency_decimal_places FROM companies WHERE id = $1",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match row {
            Some((books_beginning, precision)) => Ok((books_beginning, precision as u32)),
            None => Err(OpenTallyError::NotFound(format!(
                "Company '{}' not found",
                company_id
            ))),
        }
    }

    /// Verify that a voucher type exists in the company and is active.
    async fn verify_voucher_type(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_type_id: &str,
    ) -> Result<(), OpenTallyError> {
        let is_active = sqlx::query_scalar::<_, i64>(
            "SELECT is_active FROM voucher_types WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_type_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match is_active {
            Some(1) => Ok(()),
            Some(_) => Err(OpenTallyError::Validation(format!(
                "Voucher type '{}' is inactive",
                voucher_type_id
            ))),
            None => Err(OpenTallyError::NotFound(format!(
                "Voucher type '{}' not found for company '{}'",
                voucher_type_id, company_id
            ))),
        }
    }

    /// Verify that a numbering series exists for the company and voucher type, is active,
    /// and return its numbering_method.
    async fn verify_series(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        voucher_type_id: &str,
        series_id: &str,
    ) -> Result<String, OpenTallyError> {
        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT numbering_method, is_active FROM voucher_number_series 
             WHERE company_id = $1 AND voucher_type_id = $2 AND id = $3",
        )
        .bind(company_id)
        .bind(voucher_type_id)
        .bind(series_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match row {
            Some((method, 1)) => Ok(method),
            Some((_, _)) => Err(OpenTallyError::Validation(format!(
                "Series '{}' is inactive",
                series_id
            ))),
            None => Err(OpenTallyError::NotFound(format!(
                "Series '{}' not found for voucher type '{}'",
                series_id, voucher_type_id
            ))),
        }
    }

    /// Verify that a ledger exists in the company.
    async fn verify_ledger(
        conn: &mut sqlx::SqliteConnection,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<(), OpenTallyError> {
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT 1 FROM ledgers WHERE company_id = $1 AND id = $2")
                .bind(company_id)
                .bind(ledger_id)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)?;

        if exists.is_none() {
            return Err(OpenTallyError::NotFound(format!(
                "Ledger '{}' not found in company '{}'",
                ledger_id, company_id
            )));
        }
        Ok(())
    }

    /// Create a new draft voucher header and optional lines atomically in a transaction.
    pub async fn create_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CreateVoucherDraftRequest,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let (books_beginning, precision) = Self::get_company_meta(&mut tx, company_id).await?;

        // 1. Date validation
        validate_voucher_date(&req.date, &books_beginning)?;
        let effective_date = match req.effective_date.as_deref() {
            Some(ed) if !ed.trim().is_empty() => {
                validate_voucher_date(ed, &books_beginning)?;
                ed.to_string()
            }
            _ => req.date.clone(),
        };

        if let Some(ref_date) = req.reference_date.as_deref() {
            if !ref_date.trim().is_empty() {
                validate_voucher_date(ref_date, &books_beginning)?;
            }
        }

        validate_narration(req.narration.as_deref())?;

        // 2. Validate voucher type and series
        Self::verify_voucher_type(&mut tx, company_id, &req.voucher_type_id).await?;
        let numbering_method =
            Self::verify_series(&mut tx, company_id, &req.voucher_type_id, &req.series_id).await?;

        // 3. Generate provisional DRAFT identifier
        let voucher_id = Uuid::new_v4().to_string();
        let provisional_voucher_number = format!("DRAFT-{}", Uuid::new_v4());

        // 4. Insert draft header
        sqlx::query(
            "INSERT INTO vouchers (
                id, company_id, voucher_type_id, series_id, numbering_method,
                period_start, sequence_number, voucher_number,
                date, effective_date, status, reference_no, reference_date,
                narration, total_amount, reversal_of_voucher_id
            ) VALUES (
                $1, $2, $3, $4, $5,
                NULL, NULL, $6,
                $7, $8, 'draft', $9, $10,
                $11, NULL, NULL
            )",
        )
        .bind(&voucher_id)
        .bind(company_id)
        .bind(&req.voucher_type_id)
        .bind(&req.series_id)
        .bind(&numbering_method)
        .bind(&provisional_voucher_number)
        .bind(&req.date)
        .bind(&effective_date)
        .bind(&req.reference_no)
        .bind(&req.reference_date)
        .bind(&req.narration)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // 5. Insert lines if provided
        let mut inserted_lines = Vec::new();
        if let Some(lines_input) = req.lines {
            validate_voucher_line_count(lines_input.len())?;
            for (idx, line) in lines_input.into_iter().enumerate() {
                validate_entry_type(&line.entry_type)?;
                let dec = validate_canonical_amount(&line.amount, precision)?;
                validate_narration(line.narration.as_deref())?;
                Self::verify_ledger(&mut tx, company_id, &line.ledger_id).await?;

                let line_id = line.id.unwrap_or_else(|| Uuid::new_v4().to_string());
                let line_number = (idx + 1) as i64;
                let canonical_amount = dec.to_string();

                sqlx::query(
                    "INSERT INTO voucher_lines (
                        id, company_id, voucher_id, line_number,
                        ledger_id, entry_type, amount, narration
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(&line_id)
                .bind(company_id)
                .bind(&voucher_id)
                .bind(line_number)
                .bind(&line.ledger_id)
                .bind(&line.entry_type)
                .bind(&canonical_amount)
                .bind(&line.narration)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                inserted_lines.push(VoucherLine {
                    id: line_id,
                    company_id: company_id.to_string(),
                    voucher_id: voucher_id.clone(),
                    line_number,
                    ledger_id: line.ledger_id,
                    entry_type: line.entry_type,
                    amount: canonical_amount,
                    narration: line.narration,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        let header = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&voucher_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(Self::build_composite(header, inserted_lines))
    }

    /// Retrieve an individual draft voucher with its ordered line items and running balance.
    pub async fn get_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        let header = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if header.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Voucher '{}' is not in draft status (current: '{}')",
                voucher_id, header.status
            )));
        }

        let lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type,
                    amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(Self::build_composite(header, lines))
    }

    /// List draft vouchers with filtering and pagination.
    pub async fn list_drafts(
        pool: &Pool<Sqlite>,
        company_id: &str,
        filter: VoucherFilter,
    ) -> Result<PaginatedResponse<VoucherSummary>, OpenTallyError> {
        let page = filter.page.unwrap_or(1).max(1);
        let page_size = filter.page_size.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * page_size;
        let status = filter.status.unwrap_or_else(|| "draft".to_string());

        let mut count_query = String::from(
            "SELECT COUNT(*) FROM vouchers v WHERE v.company_id = $1 AND v.status = $2",
        );
        let mut select_query = String::from(
            "SELECT v.id, v.company_id, v.voucher_type_id, vt.name AS voucher_type_name,
                    v.series_id, s.series_name, v.voucher_number, v.date, v.status,
                    v.total_amount,
                    (SELECT COUNT(*) FROM voucher_lines vl WHERE vl.company_id = v.company_id AND vl.voucher_id = v.id) AS line_count,
                    v.updated_at
             FROM vouchers v
             JOIN voucher_types vt ON v.company_id = vt.company_id AND v.voucher_type_id = vt.id
             JOIN voucher_number_series s ON v.company_id = s.company_id AND v.series_id = s.id
             WHERE v.company_id = $1 AND v.status = $2",
        );

        let mut binds: Vec<String> = vec![company_id.to_string(), status];

        if let Some(vt_id) = filter.voucher_type_id {
            if !vt_id.trim().is_empty() {
                binds.push(vt_id);
                let param_idx = binds.len();
                count_query.push_str(&format!(" AND v.voucher_type_id = ${}", param_idx));
                select_query.push_str(&format!(" AND v.voucher_type_id = ${}", param_idx));
            }
        }

        if let Some(s_id) = filter.series_id {
            if !s_id.trim().is_empty() {
                binds.push(s_id);
                let param_idx = binds.len();
                count_query.push_str(&format!(" AND v.series_id = ${}", param_idx));
                select_query.push_str(&format!(" AND v.series_id = ${}", param_idx));
            }
        }

        if let Some(from_date) = filter.date_from {
            if !from_date.trim().is_empty() {
                binds.push(from_date);
                let param_idx = binds.len();
                count_query.push_str(&format!(" AND v.date >= ${}", param_idx));
                select_query.push_str(&format!(" AND v.date >= ${}", param_idx));
            }
        }

        if let Some(to_date) = filter.date_to {
            if !to_date.trim().is_empty() {
                binds.push(to_date);
                let param_idx = binds.len();
                count_query.push_str(&format!(" AND v.date <= ${}", param_idx));
                select_query.push_str(&format!(" AND v.date <= ${}", param_idx));
            }
        }

        // Execute count query
        let mut count_cmd = sqlx::query_scalar::<_, i64>(&count_query);
        for bind_val in &binds {
            count_cmd = count_cmd.bind(bind_val);
        }
        let total = count_cmd
            .fetch_one(pool)
            .await
            .map_err(OpenTallyError::Database)?;

        // Execute select query
        select_query.push_str(" ORDER BY v.date DESC, v.created_at DESC LIMIT $");
        let limit_idx = binds.len() + 1;
        let offset_idx = binds.len() + 2;
        select_query.push_str(&format!("{} OFFSET ${}", limit_idx, offset_idx));

        let mut select_cmd = sqlx::query_as::<_, VoucherSummary>(&select_query);
        for bind_val in binds {
            select_cmd = select_cmd.bind(bind_val);
        }
        select_cmd = select_cmd.bind(page_size as i64).bind(offset as i64);

        let items = select_cmd
            .fetch_all(pool)
            .await
            .map_err(OpenTallyError::Database)?;

        let total_pages = if total == 0 {
            1
        } else {
            (total as u32).div_ceil(page_size)
        };

        Ok(PaginatedResponse {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }

    /// Update draft voucher header details.
    pub async fn update_draft_header(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        req: UpdateVoucherDraftRequest,
    ) -> Result<Voucher, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let current = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if current.status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Voucher '{}' is not in draft status (current: '{}')",
                voucher_id, current.status
            )));
        }

        let (books_beginning, _) = Self::get_company_meta(&mut tx, company_id).await?;

        let new_date = match req.date {
            Some(d) => {
                validate_voucher_date(&d, &books_beginning)?;
                d
            }
            None => current.date,
        };

        let new_effective_date = match req.effective_date {
            Some(ed) => {
                validate_voucher_date(&ed, &books_beginning)?;
                ed
            }
            None => current.effective_date,
        };

        if let Some(ref_date) = req.reference_date.as_deref() {
            if !ref_date.trim().is_empty() {
                validate_voucher_date(ref_date, &books_beginning)?;
            }
        }

        validate_narration(req.narration.as_deref())?;

        let new_type_id = req.voucher_type_id.unwrap_or(current.voucher_type_id);
        let new_series_id = req.series_id.unwrap_or(current.series_id);

        Self::verify_voucher_type(&mut tx, company_id, &new_type_id).await?;
        let new_numbering_method =
            Self::verify_series(&mut tx, company_id, &new_type_id, &new_series_id).await?;

        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE vouchers 
             SET voucher_type_id = $1, series_id = $2, numbering_method = $3,
                 date = $4, effective_date = $5, reference_no = $6, reference_date = $7,
                 narration = $8, updated_at = $9
             WHERE company_id = $10 AND id = $11 AND status = 'draft'",
        )
        .bind(&new_type_id)
        .bind(&new_series_id)
        .bind(&new_numbering_method)
        .bind(&new_date)
        .bind(&new_effective_date)
        .bind(req.reference_no.or(current.reference_no))
        .bind(req.reference_date.or(current.reference_date))
        .bind(req.narration.or(current.narration))
        .bind(&now)
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let updated = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(updated)
    }

    /// Hard delete a draft voucher and its child lines (cascade).
    pub async fn delete_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
    ) -> Result<(), OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete voucher '{}' with status '{}'. Only draft vouchers can be deleted.",
                voucher_id, status
            )));
        }

        let res = sqlx::query(
            "DELETE FROM vouchers WHERE company_id = $1 AND id = $2 AND status = 'draft'",
        )
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if res.rows_affected() == 0 {
            return Err(OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found for deletion",
                voucher_id
            )));
        }

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    /// Add a line to a draft voucher.
    pub async fn add_line(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        req: AddVoucherLineRequest,
    ) -> Result<VoucherLine, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot add lines to voucher '{}' with status '{}'",
                voucher_id, status
            )));
        }

        let (_, precision) = Self::get_company_meta(&mut tx, company_id).await?;
        validate_entry_type(&req.entry_type)?;
        let dec = validate_canonical_amount(&req.amount, precision)?;
        validate_narration(req.narration.as_deref())?;
        Self::verify_ledger(&mut tx, company_id, &req.ledger_id).await?;

        // 1. Verify line count limit (TD-S2-002)
        let current_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if current_count >= MAX_VOUCHER_LINES as i64 {
            return Err(OpenTallyError::Validation(format!(
                "Cannot add line: voucher already contains maximum allowed lines ({})",
                MAX_VOUCHER_LINES
            )));
        }

        // 2. Validate target line number if supplied
        if let Some(ln) = req.line_number {
            validate_line_number(ln)?;
        }

        // 3. Verify existing lines are not corrupted (< 1 or > MAX_LINE_NUMBER)
        let corrupted = sqlx::query_scalar::<_, i64>(
            "SELECT line_number FROM voucher_lines 
             WHERE company_id = $1 AND voucher_id = $2 AND (line_number < 1 OR line_number > $3)
             LIMIT 1",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(MAX_LINE_NUMBER)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(bad_num) = corrupted {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Cannot insert line: corrupted line number {} detected (must be between 1 and {})",
                bad_num, MAX_LINE_NUMBER
            )));
        }

        // Determine line number
        let max_line = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(line_number) FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .unwrap_or(0);

        let target_line_number = match req.line_number {
            Some(ln) if ln > 0 && ln <= max_line => {
                // Shift lines >= ln upwards safely avoiding unique constraint collision
                sqlx::query(
                    "UPDATE voucher_lines SET line_number = line_number + $1 
                     WHERE company_id = $2 AND voucher_id = $3 AND line_number >= $4",
                )
                .bind(LINE_RENUMBER_OFFSET)
                .bind(company_id)
                .bind(voucher_id)
                .bind(ln)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                sqlx::query(
                    "UPDATE voucher_lines SET line_number = line_number - $1 + 1 
                     WHERE company_id = $2 AND voucher_id = $3 AND line_number >= $1",
                )
                .bind(LINE_RENUMBER_OFFSET)
                .bind(company_id)
                .bind(voucher_id)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                ln
            }
            _ => max_line + 1,
        };

        let line_id = Uuid::new_v4().to_string();
        let canonical_amount = dec.to_string();

        sqlx::query(
            "INSERT INTO voucher_lines (
                id, company_id, voucher_id, line_number,
                ledger_id, entry_type, amount, narration
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&line_id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(target_line_number)
        .bind(&req.ledger_id)
        .bind(&req.entry_type)
        .bind(&canonical_amount)
        .bind(&req.narration)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let created_line = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type,
                    amount, narration, created_at
             FROM voucher_lines WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&line_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(created_line)
    }

    /// Update an existing voucher line.
    pub async fn update_line(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        line_id: &str,
        req: UpdateVoucherLineRequest,
    ) -> Result<VoucherLine, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot modify lines of non-draft voucher '{}' (status: '{}')",
                voucher_id, status
            )));
        }

        let current_line = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type,
                    amount, narration, created_at
             FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2 AND id = $3",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(line_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Voucher line '{}' not found in voucher '{}'",
                line_id, voucher_id
            ))
        })?;

        let (_, precision) = Self::get_company_meta(&mut tx, company_id).await?;

        let new_ledger_id = match req.ledger_id {
            Some(lid) => {
                Self::verify_ledger(&mut tx, company_id, &lid).await?;
                lid
            }
            None => current_line.ledger_id,
        };

        let new_entry_type = match req.entry_type {
            Some(et) => {
                validate_entry_type(&et)?;
                et
            }
            None => current_line.entry_type,
        };

        let new_amount = match req.amount {
            Some(amt) => {
                let dec = validate_canonical_amount(&amt, precision)?;
                dec.to_string()
            }
            None => current_line.amount,
        };

        validate_narration(req.narration.as_deref())?;
        let new_narration = req.narration.or(current_line.narration);

        sqlx::query(
            "UPDATE voucher_lines 
             SET ledger_id = $1, entry_type = $2, amount = $3, narration = $4
             WHERE company_id = $5 AND voucher_id = $6 AND id = $7",
        )
        .bind(&new_ledger_id)
        .bind(&new_entry_type)
        .bind(&new_amount)
        .bind(&new_narration)
        .bind(company_id)
        .bind(voucher_id)
        .bind(line_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let updated_line = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type,
                    amount, narration, created_at
             FROM voucher_lines WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(line_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(updated_line)
    }

    /// Delete a line item and renumber the remaining lines to maintain contiguous 1-based indices.
    pub async fn delete_line(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        line_id: &str,
    ) -> Result<(), OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot delete lines from non-draft voucher '{}' (status: '{}')",
                voucher_id, status
            )));
        }

        let res = sqlx::query(
            "DELETE FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2 AND id = $3",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(line_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if res.rows_affected() == 0 {
            return Err(OpenTallyError::NotFound(format!(
                "Voucher line '{}' not found for deletion in voucher '{}'",
                line_id, voucher_id
            )));
        }

        // Check for corrupted line numbers among existing lines before renumbering
        let corrupted = sqlx::query_scalar::<_, i64>(
            "SELECT line_number FROM voucher_lines 
             WHERE company_id = $1 AND voucher_id = $2 AND (line_number < 1 OR line_number > $3)
             LIMIT 1",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(MAX_LINE_NUMBER)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(bad_num) = corrupted {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Cannot delete line: corrupted line number {} detected (must be between 1 and {})",
                bad_num, MAX_LINE_NUMBER
            )));
        }

        // Renumber remaining lines to contiguous 1..N
        let remaining_lines = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM voucher_lines 
             WHERE company_id = $1 AND voucher_id = $2 
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if !remaining_lines.is_empty() {
            // Phase 1: shift lines to high range
            sqlx::query(
                "UPDATE voucher_lines SET line_number = line_number + $1 
                 WHERE company_id = $2 AND voucher_id = $3",
            )
            .bind(LINE_RENUMBER_OFFSET)
            .bind(company_id)
            .bind(voucher_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            // Phase 2: assign clean 1..N
            for (idx, (lid,)) in remaining_lines.iter().enumerate() {
                let new_num = (idx + 1) as i64;
                sqlx::query(
                    "UPDATE voucher_lines SET line_number = $1 
                     WHERE company_id = $2 AND voucher_id = $3 AND id = $4",
                )
                .bind(new_num)
                .bind(company_id)
                .bind(voucher_id)
                .bind(lid)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;
            }
        }

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    /// Reorder lines according to caller-supplied ordered line ID list.
    pub async fn reorder_lines(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        req: ReorderVoucherLinesRequest,
    ) -> Result<Vec<VoucherLine>, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Draft voucher '{}' not found in company '{}'",
                voucher_id, company_id
            ))
        })?;

        if status != "draft" {
            return Err(OpenTallyError::Validation(format!(
                "Cannot reorder lines on non-draft voucher '{}' (status: '{}')",
                voucher_id, status
            )));
        }

        validate_voucher_line_count(req.line_ids.len())?;

        let existing_lines = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let existing_set: HashSet<String> = existing_lines.into_iter().map(|(id,)| id).collect();

        if req.line_ids.len() != existing_set.len() {
            return Err(OpenTallyError::Validation(
                "Invalid line IDs for reordering: count does not match existing lines".to_string(),
            ));
        }

        let req_set: HashSet<String> = req.line_ids.iter().cloned().collect();
        if req_set != existing_set {
            return Err(OpenTallyError::Validation(
                "Invalid line IDs for reordering: supplied IDs do not match existing line set"
                    .to_string(),
            ));
        }

        // Check for corrupted line numbers before reordering
        let corrupted = sqlx::query_scalar::<_, i64>(
            "SELECT line_number FROM voucher_lines 
             WHERE company_id = $1 AND voucher_id = $2 AND (line_number < 1 OR line_number > $3)
             LIMIT 1",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(MAX_LINE_NUMBER)
        .fetch_optional(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some(bad_num) = corrupted {
            return Err(OpenTallyError::IntegrityViolation(format!(
                "Cannot reorder lines: corrupted line number {} detected (must be between 1 and {})",
                bad_num, MAX_LINE_NUMBER
            )));
        }

        // Two-phase renumbering
        sqlx::query(
            "UPDATE voucher_lines SET line_number = line_number + $1 
             WHERE company_id = $2 AND voucher_id = $3",
        )
        .bind(LINE_RENUMBER_OFFSET)
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        for (idx, line_id) in req.line_ids.iter().enumerate() {
            let new_line_num = (idx + 1) as i64;
            sqlx::query(
                "UPDATE voucher_lines SET line_number = $1 
                 WHERE company_id = $2 AND voucher_id = $3 AND id = $4",
            )
            .bind(new_line_num)
            .bind(company_id)
            .bind(voucher_id)
            .bind(line_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        let reordered = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type,
                    amount, narration, created_at
             FROM voucher_lines 
             WHERE company_id = $1 AND voucher_id = $2 
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(reordered)
    }

    /// Save a draft voucher atomically (create new or update existing with full lines replacement).
    pub async fn save_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: SaveVoucherDraftRequest,
    ) -> Result<VoucherWithLines, OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let (books_beginning, precision) = Self::get_company_meta(&mut tx, company_id).await?;

        validate_voucher_date(&req.date, &books_beginning)?;
        let effective_date = match req.effective_date.as_deref() {
            Some(ed) if !ed.trim().is_empty() => {
                validate_voucher_date(ed, &books_beginning)?;
                ed.to_string()
            }
            _ => req.date.clone(),
        };

        if let Some(ref_date) = req.reference_date.as_deref() {
            if !ref_date.trim().is_empty() {
                validate_voucher_date(ref_date, &books_beginning)?;
            }
        }

        validate_narration(req.narration.as_deref())?;

        Self::verify_voucher_type(&mut tx, company_id, &req.voucher_type_id).await?;
        let numbering_method =
            Self::verify_series(&mut tx, company_id, &req.voucher_type_id, &req.series_id).await?;

        validate_voucher_line_count(req.lines.len())?;

        // Pre-validate all line items before mutating database
        let mut validated_lines = Vec::new();
        for (idx, line) in req.lines.into_iter().enumerate() {
            validate_entry_type(&line.entry_type)?;
            let dec = validate_canonical_amount(&line.amount, precision)?;
            validate_narration(line.narration.as_deref())?;
            Self::verify_ledger(&mut tx, company_id, &line.ledger_id).await?;

            let line_id = line.id.unwrap_or_else(|| Uuid::new_v4().to_string());
            let line_number = (idx + 1) as i64;
            validated_lines.push((
                line_id,
                line_number,
                line.ledger_id,
                line.entry_type,
                dec.to_string(),
                line.narration,
            ));
        }

        let voucher_id = match req.voucher_id {
            Some(vid) => {
                // Verify existing draft
                let status = sqlx::query_scalar::<_, String>(
                    "SELECT status FROM vouchers WHERE company_id = $1 AND id = $2",
                )
                .bind(company_id)
                .bind(&vid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| {
                    OpenTallyError::NotFound(format!(
                        "Draft voucher '{}' not found in company '{}'",
                        vid, company_id
                    ))
                })?;

                if status != "draft" {
                    return Err(OpenTallyError::Validation(format!(
                        "Cannot save non-draft voucher '{}' (current status: '{}')",
                        vid, status
                    )));
                }

                let now = chrono::Utc::now().to_rfc3339();

                sqlx::query(
                    "UPDATE vouchers 
                     SET voucher_type_id = $1, series_id = $2, numbering_method = $3,
                         date = $4, effective_date = $5, reference_no = $6, reference_date = $7,
                         narration = $8, updated_at = $9
                     WHERE company_id = $10 AND id = $11 AND status = 'draft'",
                )
                .bind(&req.voucher_type_id)
                .bind(&req.series_id)
                .bind(&numbering_method)
                .bind(&req.date)
                .bind(&effective_date)
                .bind(&req.reference_no)
                .bind(&req.reference_date)
                .bind(&req.narration)
                .bind(&now)
                .bind(company_id)
                .bind(&vid)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                // Delete existing lines
                sqlx::query("DELETE FROM voucher_lines WHERE company_id = $1 AND voucher_id = $2")
                    .bind(company_id)
                    .bind(&vid)
                    .execute(&mut *tx)
                    .await
                    .map_err(OpenTallyError::Database)?;

                vid
            }
            None => {
                // Create new draft
                let vid = Uuid::new_v4().to_string();
                let provisional_voucher_number = format!("DRAFT-{}", Uuid::new_v4());

                sqlx::query(
                    "INSERT INTO vouchers (
                        id, company_id, voucher_type_id, series_id, numbering_method,
                        period_start, sequence_number, voucher_number,
                        date, effective_date, status, reference_no, reference_date,
                        narration, total_amount, reversal_of_voucher_id
                    ) VALUES (
                        $1, $2, $3, $4, $5,
                        NULL, NULL, $6,
                        $7, $8, 'draft', $9, $10,
                        $11, NULL, NULL
                    )",
                )
                .bind(&vid)
                .bind(company_id)
                .bind(&req.voucher_type_id)
                .bind(&req.series_id)
                .bind(&numbering_method)
                .bind(&provisional_voucher_number)
                .bind(&req.date)
                .bind(&effective_date)
                .bind(&req.reference_no)
                .bind(&req.reference_date)
                .bind(&req.narration)
                .execute(&mut *tx)
                .await
                .map_err(OpenTallyError::Database)?;

                vid
            }
        };

        // Insert validated lines
        let mut final_lines = Vec::new();
        for (line_id, line_number, ledger_id, entry_type, canonical_amount, line_narration) in
            validated_lines
        {
            sqlx::query(
                "INSERT INTO voucher_lines (
                    id, company_id, voucher_id, line_number,
                    ledger_id, entry_type, amount, narration
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&line_id)
            .bind(company_id)
            .bind(&voucher_id)
            .bind(line_number)
            .bind(&ledger_id)
            .bind(&entry_type)
            .bind(&canonical_amount)
            .bind(&line_narration)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            final_lines.push(VoucherLine {
                id: line_id,
                company_id: company_id.to_string(),
                voucher_id: voucher_id.clone(),
                line_number,
                ledger_id,
                entry_type,
                amount: canonical_amount,
                narration: line_narration,
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        let header = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&voucher_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(Self::build_composite(header, final_lines))
    }

    /// Internal helper to construct VoucherWithLines and calculate running debit/credit totals and balance.
    /// Note: Stored database lines are guaranteed pre-validated against company precision at insertion time;
    /// passing database ceiling (4) avoids an extra redundant company metadata query during read operations.
    fn build_composite(header: Voucher, lines: Vec<VoucherLine>) -> VoucherWithLines {
        let val_res = crate::vouchers::service::DoubleEntryValidator::validate(
            &lines,
            4,
            crate::vouchers::model::ValidationMode::Draft,
        );

        VoucherWithLines {
            voucher: header,
            lines,
            debit_total: val_res.debit_total.to_string(),
            credit_total: val_res.credit_total.to_string(),
            difference: val_res.difference.to_string(),
            is_balanced: val_res.is_balanced,
        }
    }
}
