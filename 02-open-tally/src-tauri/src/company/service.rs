use crate::company::model::{
    validate_currency_decimals, validate_financial_year, Company, CompanyStatus,
    CreateCompanyRequest, UpdateCompanyRequest,
};
use crate::company::seeder::seed_company_defaults;
use crate::errors::OpenTallyError;
use sqlx::{Pool, Sqlite};
use uuid::Uuid;

/// Company service handling multi-company persistence, transactional seeding,
/// and lifecycle operations.
pub struct CompanyService;

impl CompanyService {
    /// Create a new company within an explicit ACID transaction.
    ///
    /// Validates date consistency, currency decimal limits, and seeds default
    /// settings, exactly 29 predefined account groups, and exactly 2 reserved ledgers.
    pub async fn create_company(
        pool: &Pool<Sqlite>,
        req: CreateCompanyRequest,
    ) -> Result<Company, OpenTallyError> {
        // 1. Validation
        let name = req.name.trim().to_string();
        if name.is_empty() {
            return Err(OpenTallyError::Validation(
                "Company name cannot be empty".to_string(),
            ));
        }

        let fy_begins = req.financial_year_begins.trim().to_string();
        let books_begins = req
            .books_beginning_from
            .as_deref()
            .unwrap_or(&fy_begins)
            .trim()
            .to_string();
        validate_financial_year(&fy_begins, &books_begins)?;

        let decimals = req.currency_decimal_places.unwrap_or(2);
        validate_currency_decimals(decimals)?;

        let mailing_name = req
            .mailing_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| name.clone());
        let country = req
            .country
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "India".to_string());
        let state = req
            .state
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Karnataka".to_string());
        let currency_symbol = req
            .currency_symbol
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "₹".to_string());
        let currency_formal_name = req
            .currency_formal_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "INR".to_string());

        let company_id = Uuid::new_v4().to_string();

        // 2. Begin explicit scoped transaction on connection
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;

        // 3. Insert company with parameterized query
        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_symbol, currency_formal_name, currency_decimal_places, status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'active', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&company_id)
        .bind(&name)
        .bind(&mailing_name)
        .bind(&country)
        .bind(&state)
        .bind(&fy_begins)
        .bind(&books_begins)
        .bind(&currency_symbol)
        .bind(&currency_formal_name)
        .bind(decimals)
        .execute(&mut *tx)
        .await
        .map_err(OpenTallyError::Database)?;

        // 4. Seed default settings, groups, and reserved ledgers within transaction
        seed_company_defaults(&mut tx, &company_id, decimals, &fy_begins, &books_begins).await?;

        // 5. Commit transaction
        tx.commit().await.map_err(OpenTallyError::Database)?;

        // 4. Verify Invariants
        let total_groups: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account_groups WHERE company_id = $1")
                .bind(&company_id)
                .fetch_one(pool)
                .await?;
        if total_groups != 29 {
            return Err(OpenTallyError::Internal(format!(
                "Seeder invariant violation: expected 29 groups, found {}",
                total_groups
            )));
        }

        let total_reserved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ledgers WHERE company_id = $1 AND is_reserved = 1",
        )
        .bind(&company_id)
        .fetch_one(pool)
        .await?;
        if total_reserved != 2 {
            return Err(OpenTallyError::Internal(format!(
                "Seeder invariant violation: expected 2 reserved ledgers, found {}",
                total_reserved
            )));
        }

        // 5. Fetch Created Company Record
        let company = sqlx::query_as::<_, Company>("SELECT * FROM companies WHERE id = $1")
            .bind(&company_id)
            .fetch_one(pool)
            .await?;

        Ok(company)
    }

    /// List all companies ordered by name.
    pub async fn list_companies(pool: &Pool<Sqlite>) -> Result<Vec<Company>, OpenTallyError> {
        let companies = sqlx::query_as::<_, Company>("SELECT * FROM companies ORDER BY name ASC")
            .fetch_all(pool)
            .await?;
        Ok(companies)
    }

    /// Fetch a single company by ID.
    pub async fn get_company(pool: &Pool<Sqlite>, id: &str) -> Result<Company, OpenTallyError> {
        let company = sqlx::query_as::<_, Company>("SELECT * FROM companies WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                OpenTallyError::Validation(format!("Company with ID '{}' not found", id))
            })?;
        Ok(company)
    }

    /// Update permitted company fields.
    ///
    /// Preserves invariant fields (id, name, financial_year_begins, books_beginning_from, currency_decimal_places).
    pub async fn update_company(
        pool: &Pool<Sqlite>,
        id: &str,
        req: UpdateCompanyRequest,
    ) -> Result<Company, OpenTallyError> {
        let current = Self::get_company(pool, id).await?;

        let mailing_name = req
            .mailing_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(current.mailing_name);
        let country = req
            .country
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(current.country);
        let state = req
            .state
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(current.state);
        let currency_symbol = req
            .currency_symbol
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(current.currency_symbol);
        let currency_formal_name = req
            .currency_formal_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or(current.currency_formal_name);

        sqlx::query(
            "UPDATE companies \
             SET mailing_name = $1, country = $2, state = $3, currency_symbol = $4, currency_formal_name = $5, updated_at = CURRENT_TIMESTAMP \
             WHERE id = $6",
        )
        .bind(&mailing_name)
        .bind(&country)
        .bind(&state)
        .bind(&currency_symbol)
        .bind(&currency_formal_name)
        .bind(id)
        .execute(pool)
        .await?;

        Self::get_company(pool, id).await
    }

    /// Soft-archive a company (transitions status to 'archived').
    pub async fn archive_company(pool: &Pool<Sqlite>, id: &str) -> Result<Company, OpenTallyError> {
        let _ = Self::get_company(pool, id).await?;

        sqlx::query(
            "UPDATE companies \
             SET status = 'archived', updated_at = CURRENT_TIMESTAMP \
             WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;

        Self::get_company(pool, id).await
    }

    /// Validate and select an active company.
    ///
    /// Archived or closed companies cannot be selected without reactivation.
    pub async fn select_active_company(
        pool: &Pool<Sqlite>,
        id: &str,
    ) -> Result<Company, OpenTallyError> {
        let company = Self::get_company(pool, id).await?;

        if company.status != CompanyStatus::Active {
            return Err(OpenTallyError::Validation(format!(
                "Cannot select company '{}' with status '{}'. Only active companies can be selected.",
                company.name, company.status
            )));
        }

        Ok(company)
    }

    /// Persist the selected active company ID to database storage.
    pub async fn set_persisted_active_company_id(
        pool: &Pool<Sqlite>,
        company_id: Option<&str>,
    ) -> Result<(), OpenTallyError> {
        match company_id {
            Some(id) => {
                sqlx::query(
                    "INSERT INTO _opentally_app_state (key, value) VALUES ('active_company_id', $1) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                )
                .bind(id)
                .execute(pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM _opentally_app_state WHERE key = 'active_company_id'")
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Retrieve the persisted active company ID from database storage, if any.
    pub async fn get_persisted_active_company_id(
        pool: &Pool<Sqlite>,
    ) -> Result<Option<String>, OpenTallyError> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM _opentally_app_state WHERE key = 'active_company_id'",
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| r.0))
    }
}
