use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    CreateVoucherNumberConfigRequest, CreateVoucherSeriesRequest, SeriesNamespaceInfo,
    UpdateVoucherSeriesRequest, VoucherNumberConfig, VoucherNumberSeries,
};
use crate::vouchers::service::voucher_type::VoucherTypeService;
use crate::vouchers::validator::{
    validate_configuration_tokens, validate_numbering_method, validate_restart_period,
    validate_series_code, validate_series_name, validate_series_namespace_isolation,
};
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

/// Service handling numbering series CRUD, effective configurations, and namespace management.
pub struct VoucherSeriesService;

impl VoucherSeriesService {
    /// Create a new numbering series and initial configuration within an explicit ACID transaction.
    pub async fn create_series(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CreateVoucherSeriesRequest,
    ) -> Result<VoucherNumberSeries, OpenTallyError> {
        // 1. Validate inputs
        validate_series_name(&req.series_name)?;
        validate_series_code(&req.series_code)?;
        validate_numbering_method(&req.numbering_method)?;
        validate_restart_period(&req.restart_period)?;

        // 2. Verify voucher type belongs to this company
        VoucherTypeService::get_voucher_type(pool, company_id, &req.voucher_type_id).await?;

        // 3. Retrieve company financial year start date
        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = $1")
                .bind(company_id)
                .fetch_optional(pool)
                .await
                .map_err(OpenTallyError::Database)?
                .ok_or_else(|| {
                    OpenTallyError::NotFound(format!("Company with ID '{}' not found", company_id))
                })?;

        let prefix = req.prefix.clone().unwrap_or_default();
        let suffix = req.suffix.clone().unwrap_or_default();
        let applicable_from = req
            .applicable_from
            .clone()
            .unwrap_or_else(|| fy_begins.clone());
        crate::company::validate_calendar_date(&applicable_from)?;
        let start_number = req.start_number.unwrap_or(1);
        let width = req.width.unwrap_or(4);

        if start_number <= 0 {
            return Err(OpenTallyError::Validation(
                "start_number must be strictly positive".into(),
            ));
        }
        if !(1..=10).contains(&width) {
            return Err(OpenTallyError::Validation(
                "width must be between 1 and 10".into(),
            ));
        }

        // Validate configuration tokens
        validate_configuration_tokens(&req.restart_period, &fy_begins, &prefix, &suffix)?;

        // 4. Fetch existing series and configs to enforce namespace isolation
        let existing_rows = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>)>(
            "SELECT s.series_name, s.series_code, s.numbering_method, c.prefix, c.suffix
             FROM voucher_number_series s
             LEFT JOIN voucher_number_configs c ON s.company_id = c.company_id AND s.id = c.series_id
             WHERE s.company_id = $1 AND s.voucher_type_id = $2",
        )
        .bind(company_id)
        .bind(&req.voucher_type_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let existing_namespaces: Vec<SeriesNamespaceInfo> = existing_rows
            .into_iter()
            .map(|(s_name, s_code, s_method, p, s)| SeriesNamespaceInfo {
                series_name: s_name,
                series_code: s_code,
                numbering_method: s_method,
                prefix: p.unwrap_or_default(),
                suffix: s.unwrap_or_default(),
            })
            .collect();

        let new_namespace = SeriesNamespaceInfo {
            series_name: req.series_name.clone(),
            series_code: req.series_code.clone(),
            numbering_method: req.numbering_method.clone(),
            prefix: prefix.clone(),
            suffix: suffix.clone(),
        };

        validate_series_namespace_isolation(&existing_namespaces, &new_namespace)?;

        // 5. Begin transaction to insert series and initial configuration
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        let series_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
        )
        .bind(&series_id)
        .bind(company_id)
        .bind(&req.voucher_type_id)
        .bind(&req.series_name)
        .bind(&req.series_code)
        .bind(&req.numbering_method)
        .bind(&req.restart_period)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        let config_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&config_id)
        .bind(company_id)
        .bind(&req.voucher_type_id)
        .bind(&series_id)
        .bind(&applicable_from)
        .bind(&prefix)
        .bind(&suffix)
        .bind(start_number)
        .bind(width)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Self::get_series(pool, company_id, &series_id).await
    }

    /// List all numbering series for a given voucher type under a company.
    pub async fn list_series_by_voucher_type(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_type_id: &str,
    ) -> Result<Vec<VoucherNumberSeries>, OpenTallyError> {
        let rows = sqlx::query_as::<_, VoucherNumberSeries>(
            "SELECT id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active, created_at, updated_at
             FROM voucher_number_series
             WHERE company_id = $1 AND voucher_type_id = $2
             ORDER BY series_name ASC",
        )
        .bind(company_id)
        .bind(voucher_type_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }

    /// Retrieve a single series by id and company_id.
    pub async fn get_series(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
    ) -> Result<VoucherNumberSeries, OpenTallyError> {
        let row = sqlx::query_as::<_, VoucherNumberSeries>(
            "SELECT id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period, is_active, created_at, updated_at
             FROM voucher_number_series
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        row.ok_or_else(|| {
            OpenTallyError::NotFound(format!("Voucher series with ID '{}' not found", series_id))
        })
    }

    /// Update mutable attributes (series_name, is_active) of a series.
    pub async fn update_series(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
        req: UpdateVoucherSeriesRequest,
    ) -> Result<VoucherNumberSeries, OpenTallyError> {
        let existing = Self::get_series(pool, company_id, series_id).await?;

        let new_name = match req.series_name {
            Some(ref name) => {
                validate_series_name(name)?;
                name.clone()
            }
            None => existing.series_name.clone(),
        };
        let new_active = req.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            "UPDATE voucher_number_series
             SET series_name = $1, is_active = $2, updated_at = CURRENT_TIMESTAMP
             WHERE company_id = $3 AND id = $4",
        )
        .bind(&new_name)
        .bind(if new_active { 1 } else { 0 })
        .bind(company_id)
        .bind(series_id)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Self::get_series(pool, company_id, &existing.id).await
    }

    /// Deactivate a numbering series.
    pub async fn deactivate_series(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
    ) -> Result<VoucherNumberSeries, OpenTallyError> {
        Self::update_series(
            pool,
            company_id,
            series_id,
            UpdateVoucherSeriesRequest {
                series_name: None,
                is_active: Some(false),
            },
        )
        .await
    }

    /// Delete a series if no dependent vouchers exist.
    pub async fn delete_series(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
    ) -> Result<(), OpenTallyError> {
        let _ = Self::get_series(pool, company_id, series_id).await?;

        let voucher_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM vouchers WHERE company_id = $1 AND series_id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if voucher_count > 0 {
            return Err(OpenTallyError::Validation(
                "Cannot delete numbering series with existing vouchers".to_string(),
            ));
        }

        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        sqlx::query("DELETE FROM voucher_number_configs WHERE company_id = $1 AND series_id = $2")
            .bind(company_id)
            .bind(series_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        sqlx::query(
            "DELETE FROM voucher_number_sequences WHERE company_id = $1 AND series_id = $2",
        )
        .bind(company_id)
        .bind(series_id)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("DELETE FROM voucher_number_series WHERE company_id = $1 AND id = $2")
            .bind(company_id)
            .bind(series_id)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

        tx.commit().await.map_err(OpenTallyError::Database)?;

        Ok(())
    }

    /// Create a new effective-dated configuration version for a series.
    pub async fn create_configuration(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
        req: CreateVoucherNumberConfigRequest,
    ) -> Result<VoucherNumberConfig, OpenTallyError> {
        let series = Self::get_series(pool, company_id, series_id).await?;

        let fy_begins: String =
            sqlx::query_scalar("SELECT financial_year_begins FROM companies WHERE id = $1")
                .bind(company_id)
                .fetch_one(pool)
                .await
                .map_err(OpenTallyError::Database)?;

        let prefix = req.prefix.unwrap_or_default();
        let suffix = req.suffix.unwrap_or_default();
        let start_number = req.start_number.unwrap_or(1);
        let width = req.width.unwrap_or(4);

        if start_number <= 0 {
            return Err(OpenTallyError::Validation(
                "start_number must be strictly positive".into(),
            ));
        }
        if !(1..=10).contains(&width) {
            return Err(OpenTallyError::Validation(
                "width must be between 1 and 10".into(),
            ));
        }

        crate::company::validate_calendar_date(&req.applicable_from)?;
        validate_configuration_tokens(&series.restart_period, &fy_begins, &prefix, &suffix)?;

        let config_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&config_id)
        .bind(company_id)
        .bind(&series.voucher_type_id)
        .bind(series_id)
        .bind(&req.applicable_from)
        .bind(&prefix)
        .bind(&suffix)
        .bind(start_number)
        .bind(width)
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let config = sqlx::query_as::<_, VoucherNumberConfig>(
            "SELECT id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width, created_at, updated_at
             FROM voucher_number_configs
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&config_id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(config)
    }

    /// List all effective-dated configurations for a series.
    pub async fn list_configurations(
        pool: &Pool<Sqlite>,
        company_id: &str,
        series_id: &str,
    ) -> Result<Vec<VoucherNumberConfig>, OpenTallyError> {
        let _ = Self::get_series(pool, company_id, series_id).await?;

        let rows = sqlx::query_as::<_, VoucherNumberConfig>(
            "SELECT id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width, created_at, updated_at
             FROM voucher_number_configs
             WHERE company_id = $1 AND series_id = $2
             ORDER BY applicable_from ASC",
        )
        .bind(company_id)
        .bind(series_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(rows)
    }
}
