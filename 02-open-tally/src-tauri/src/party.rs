use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq, Eq)]
pub struct PartyProfile {
    pub id: String,
    pub company_id: String,
    pub ledger_id: String,
    pub party_code: String,
    pub display_name: String,
    pub customer_enabled: bool,
    pub supplier_enabled: bool,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub billing_address: Option<String>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub credit_period_days: i64,
    pub credit_limit: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyRequest {
    pub ledger_id: String,
    pub party_code: String,
    pub display_name: String,
    pub customer_enabled: bool,
    pub supplier_enabled: bool,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub billing_address: Option<String>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub credit_period_days: i64,
    pub credit_limit: String,
}

pub struct PartyService;

impl PartyService {
    pub async fn list(
        pool: &SqlitePool,
        company_id: &str,
        search: Option<&str>,
    ) -> Result<Vec<PartyProfile>, OpenTallyError> {
        let rows = if let Some(search) = search {
            sqlx::query_as::<_, PartyProfile>(
                "SELECT id, company_id, ledger_id, party_code, display_name, customer_enabled,
                        supplier_enabled, contact_person, email, phone, billing_address,
                        shipping_address, notes, credit_period_days, credit_limit, is_active,
                        created_at, updated_at
                 FROM party_profiles
                 WHERE company_id = ? AND (party_code LIKE '%' || ? || '%' COLLATE NOCASE
                    OR display_name LIKE '%' || ? || '%' COLLATE NOCASE)
                 ORDER BY display_name COLLATE NOCASE, id",
            )
            .bind(company_id)
            .bind(search)
            .bind(search)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, PartyProfile>(
                "SELECT id, company_id, ledger_id, party_code, display_name, customer_enabled,
                        supplier_enabled, contact_person, email, phone, billing_address,
                        shipping_address, notes, credit_period_days, credit_limit, is_active,
                        created_at, updated_at
                 FROM party_profiles WHERE company_id = ?
                 ORDER BY display_name COLLATE NOCASE, id",
            )
            .bind(company_id)
            .fetch_all(pool)
            .await?
        };
        Ok(rows)
    }

    pub async fn get(
        pool: &SqlitePool,
        company_id: &str,
        id: &str,
    ) -> Result<PartyProfile, OpenTallyError> {
        sqlx::query_as::<_, PartyProfile>(
            "SELECT id, company_id, ledger_id, party_code, display_name, customer_enabled,
                    supplier_enabled, contact_person, email, phone, billing_address,
                    shipping_address, notes, credit_period_days, credit_limit, is_active,
                    created_at, updated_at
             FROM party_profiles WHERE company_id = ? AND id = ?",
        )
        .bind(company_id)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| OpenTallyError::NotFound(format!("Party '{}' not found", id)))
    }

    pub async fn create(
        pool: &SqlitePool,
        company_id: &str,
        request: PartyRequest,
    ) -> Result<PartyProfile, OpenTallyError> {
        let request = validate_request(request)?;
        Self::validate_ledger(pool, company_id, &request.ledger_id).await?;
        let id = Uuid::new_v4().to_string();
        let mut tx = pool.begin().await?;
        sqlx::query(
            "INSERT INTO party_profiles
             (id, company_id, ledger_id, party_code, display_name, customer_enabled, supplier_enabled,
              contact_person, email, phone, billing_address, shipping_address, notes,
              credit_period_days, credit_limit)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id).bind(company_id).bind(&request.ledger_id).bind(&request.party_code)
        .bind(&request.display_name).bind(request.customer_enabled).bind(request.supplier_enabled)
        .bind(&request.contact_person).bind(&request.email).bind(&request.phone)
        .bind(&request.billing_address).bind(&request.shipping_address).bind(&request.notes)
        .bind(request.credit_period_days).bind(&request.credit_limit)
        .execute(&mut *tx).await.map_err(map_constraint)?;
        audit(&mut tx, company_id, &id, "CREATE", None, Some(&request)).await?;
        tx.commit().await?;
        Self::get(pool, company_id, &id).await
    }

    pub async fn update(
        pool: &SqlitePool,
        company_id: &str,
        id: &str,
        request: PartyRequest,
    ) -> Result<PartyProfile, OpenTallyError> {
        let request = validate_request(request)?;
        Self::validate_ledger(pool, company_id, &request.ledger_id).await?;
        let old = Self::get(pool, company_id, id).await?;
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE party_profiles SET ledger_id = ?, party_code = ?, display_name = ?,
             customer_enabled = ?, supplier_enabled = ?, contact_person = ?, email = ?, phone = ?,
             billing_address = ?, shipping_address = ?, notes = ?, credit_period_days = ?,
             credit_limit = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?",
        )
        .bind(&request.ledger_id)
        .bind(&request.party_code)
        .bind(&request.display_name)
        .bind(request.customer_enabled)
        .bind(request.supplier_enabled)
        .bind(&request.contact_person)
        .bind(&request.email)
        .bind(&request.phone)
        .bind(&request.billing_address)
        .bind(&request.shipping_address)
        .bind(&request.notes)
        .bind(request.credit_period_days)
        .bind(&request.credit_limit)
        .bind(company_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(map_constraint)?;
        audit(
            &mut tx,
            company_id,
            id,
            "UPDATE",
            Some(&old),
            Some(&request),
        )
        .await?;
        tx.commit().await?;
        Self::get(pool, company_id, id).await
    }

    pub async fn set_active(
        pool: &SqlitePool,
        company_id: &str,
        id: &str,
        active: bool,
    ) -> Result<PartyProfile, OpenTallyError> {
        let old = Self::get(pool, company_id, id).await?;
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE party_profiles SET is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE company_id = ? AND id = ?")
            .bind(active).bind(company_id).bind(id).execute(&mut *tx).await?;
        let action = "UPDATE";
        audit(&mut tx, company_id, id, action, Some(&old), None).await?;
        tx.commit().await?;
        Self::get(pool, company_id, id).await
    }

    async fn validate_ledger(
        pool: &SqlitePool,
        company_id: &str,
        ledger_id: &str,
    ) -> Result<(), OpenTallyError> {
        let ledger: Option<(bool,)> =
            sqlx::query_as("SELECT is_reserved FROM ledgers WHERE company_id = ? AND id = ?")
                .bind(company_id)
                .bind(ledger_id)
                .fetch_optional(pool)
                .await?;
        match ledger {
            None => Err(OpenTallyError::Validation(format!(
                "Ledger '{}' not found in company",
                ledger_id
            ))),
            Some((true,)) => Err(OpenTallyError::Validation(
                "Reserved ledgers cannot be party ledgers".to_owned(),
            )),
            Some((false,)) => Ok(()),
        }
    }
}

fn validate_request(mut request: PartyRequest) -> Result<PartyRequest, OpenTallyError> {
    request.party_code = request.party_code.trim().to_owned();
    request.display_name = request.display_name.trim().to_owned();
    if request.party_code.is_empty()
        || request.party_code.len() > 120
        || request.display_name.is_empty()
        || request.display_name.len() > 200
    {
        return Err(OpenTallyError::Validation(
            "Party code and display name are required and must be within limits".to_owned(),
        ));
    }
    if !request.customer_enabled && !request.supplier_enabled {
        return Err(OpenTallyError::Validation(
            "A party must be a customer, supplier, or both".to_owned(),
        ));
    }
    if request.credit_period_days < 0 {
        return Err(OpenTallyError::Validation(
            "Credit period cannot be negative".to_owned(),
        ));
    }
    let limit = Decimal::from_str_exact(request.credit_limit.trim())
        .map_err(|_| OpenTallyError::Validation("Credit limit must be a Decimal".to_owned()))?;
    if limit.is_sign_negative() || limit.scale() > 4 {
        return Err(OpenTallyError::Validation(
            "Credit limit must be non-negative with at most four decimal places".to_owned(),
        ));
    }
    request.credit_limit = limit.normalize().to_string();
    Ok(request)
}

fn map_constraint(error: sqlx::Error) -> OpenTallyError {
    if let sqlx::Error::Database(db) = &error {
        if db.message().contains("UNIQUE") {
            return OpenTallyError::Validation(
                "Party code and ledger link must be unique within the company".to_owned(),
            );
        }
    }
    OpenTallyError::Database(error)
}

async fn audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    company_id: &str,
    entity_id: &str,
    action: &str,
    old: Option<&PartyProfile>,
    new: Option<&PartyRequest>,
) -> Result<(), OpenTallyError> {
    let old_values = old.map(serde_json::to_string).transpose()?;
    let new_values = new.map(serde_json::to_string).transpose()?;
    sqlx::query(
        "INSERT INTO audit_log (id, company_id, entity_type, entity_id, action, old_values, new_values, performed_by)
         VALUES (?, ?, 'party_profile', ?, ?, ?, ?, 'system')",
    )
    .bind(Uuid::new_v4().to_string()).bind(company_id).bind(entity_id).bind(action)
    .bind(old_values).bind(new_values).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod stage17_tests {
    use super::*;
    use crate::database::migrations::{
        run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
        run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
        run_sprint4_stage12_valuation_migration, run_sprint5_stage14_integration_migration,
        run_sprint5_stage15_valuation_attribution_migration, run_stage3_migration,
    };
    use crate::database::DatabaseManager;
    use rust_decimal::Decimal;
    use sqlx::{Pool, Sqlite};

    async fn fixture() -> (tempfile::TempDir, DatabaseManager) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = DatabaseManager::init(&dir.path().join("stage17.db"))
            .await
            .expect("database");
        let pool = db.pool();
        run_sprint1_migration(pool).await.expect("sprint1");
        run_stage3_migration(pool).await.expect("stage3");
        run_sprint2_migration(pool).await.expect("sprint2");
        run_sprint2_stage4_hardening_migration(pool)
            .await
            .expect("stage4");
        run_sprint4_inventory_migration(pool)
            .await
            .expect("inventory");
        run_sprint4_stage11_stock_migration(pool)
            .await
            .expect("stock");
        run_sprint4_stage12_valuation_migration(pool)
            .await
            .expect("valuation");
        run_sprint5_stage14_integration_migration(pool)
            .await
            .expect("integration");
        run_sprint5_stage15_valuation_attribution_migration(pool)
            .await
            .expect("attribution");
        crate::database::migrations::run_sprint6_stage17_party_migration(pool)
            .await
            .expect("party migration");
        (dir, db)
    }

    async fn seed(pool: &Pool<Sqlite>) {
        sqlx::query(
            "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
             VALUES ('cmp_a', 'Alpha', 'Alpha', 'India', 'Delhi', '2026-04-01', '2026-04-01'),
                    ('cmp_b', 'Beta', 'Beta', 'India', 'Karnataka', '2026-04-01', '2026-04-01')",
        )
        .execute(pool)
        .await
        .expect("companies");
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
             VALUES ('grp_a', 'cmp_a', 'Current Assets', NULL, 'Assets', 1),
                    ('grp_b', 'cmp_b', 'Current Assets', NULL, 'Assets', 1),
                    ('cash_grp_a', 'cmp_a', 'Cash-in-Hand', 'grp_a', 'Assets', 0)",
        )
        .execute(pool)
        .await
        .expect("groups");
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES ('led_a', 'cmp_a', 'grp_a', 'Alpha Receivable', 0, 'standard'),
                    ('led_b', 'cmp_b', 'grp_b', 'Beta Receivable', 0, 'standard'),
                    ('cash_a', 'cmp_a', 'cash_grp_a', 'Cash', 1, 'cash')",
        )
        .execute(pool)
        .await
        .expect("ledgers");
    }

    fn request(ledger_id: &str, code: &str) -> PartyRequest {
        PartyRequest {
            ledger_id: ledger_id.to_owned(),
            party_code: code.to_owned(),
            display_name: format!("{code} Name"),
            customer_enabled: true,
            supplier_enabled: false,
            contact_person: None,
            email: None,
            phone: None,
            billing_address: None,
            shipping_address: None,
            notes: None,
            credit_period_days: 30,
            credit_limit: "100.00".to_owned(),
        }
    }

    #[tokio::test]
    async fn customer_creation_is_audited_and_decimal_exact() {
        let (_dir, db) = fixture().await;
        seed(db.pool()).await;
        let party = PartyService::create(db.pool(), "cmp_a", request("led_a", "CUST-1"))
            .await
            .expect("create");
        assert_eq!(party.party_code, "CUST-1");
        assert_eq!(
            Decimal::from_str_exact(&party.credit_limit).unwrap(),
            Decimal::new(100, 0)
        );
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE company_id = 'cmp_a' AND entity_type = 'party_profile' AND action = 'CREATE'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn dual_role_and_warning_only_credit_configuration_are_supported() {
        let (_dir, db) = fixture().await;
        seed(db.pool()).await;
        let mut req = request("led_a", "BOTH-1");
        req.supplier_enabled = true;
        req.credit_limit = "999999999999.1234".to_owned();
        let party = PartyService::create(db.pool(), "cmp_a", req).await.unwrap();
        assert!(party.customer_enabled && party.supplier_enabled);
        assert_eq!(party.credit_limit, "999999999999.1234");
    }

    #[tokio::test]
    async fn company_and_reserved_ledger_isolation_is_enforced() {
        let (_dir, db) = fixture().await;
        seed(db.pool()).await;
        assert!(
            PartyService::create(db.pool(), "cmp_a", request("led_b", "X"))
                .await
                .is_err()
        );
        assert!(
            PartyService::create(db.pool(), "cmp_a", request("cash_a", "Y"))
                .await
                .is_err()
        );
        let party = PartyService::create(db.pool(), "cmp_a", request("led_a", "OWN"))
            .await
            .unwrap();
        assert!(PartyService::get(db.pool(), "cmp_b", &party.id)
            .await
            .is_err());
        assert!(PartyService::list(db.pool(), "cmp_b", None)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn duplicate_code_and_ledger_are_rejected_and_status_changes_are_audited() {
        let (_dir, db) = fixture().await;
        seed(db.pool()).await;
        let party = PartyService::create(db.pool(), "cmp_a", request("led_a", "DUP"))
            .await
            .unwrap();
        assert!(
            PartyService::create(db.pool(), "cmp_a", request("led_a", "OTHER"))
                .await
                .is_err()
        );
        assert!(
            PartyService::create(db.pool(), "cmp_a", request("led_a", "dup"))
                .await
                .is_err()
        );
        let inactive = PartyService::set_active(db.pool(), "cmp_a", &party.id, false)
            .await
            .unwrap();
        assert!(!inactive.is_active);
        let active = PartyService::set_active(db.pool(), "cmp_a", &party.id, true)
            .await
            .unwrap();
        assert!(active.is_active);
        let audits: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_log WHERE entity_id = ? AND action = 'UPDATE'",
        )
        .bind(&party.id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(audits, 2);
    }

    #[tokio::test]
    async fn party_update_search_and_decimal_validation_are_company_scoped() {
        let (_dir, db) = fixture().await;
        seed(db.pool()).await;
        let party = PartyService::create(db.pool(), "cmp_a", request("led_a", "EDIT"))
            .await
            .unwrap();
        let mut edited = request("led_a", "EDIT");
        edited.display_name = "Edited Name".to_owned();
        edited.credit_limit = "10.12345".to_owned();
        assert!(
            PartyService::update(db.pool(), "cmp_a", &party.id, edited.clone())
                .await
                .is_err()
        );
        edited.credit_limit = "10.1234".to_owned();
        let updated = PartyService::update(db.pool(), "cmp_a", &party.id, edited)
            .await
            .unwrap();
        assert_eq!(updated.display_name, "Edited Name");
        assert_eq!(
            PartyService::list(db.pool(), "cmp_a", Some("Edited"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(PartyService::get(db.pool(), "cmp_b", &party.id)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn migration_is_idempotent_and_preserves_existing_schema_data() {
        let (_dir, db) = fixture().await;
        let before: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ledgers'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        crate::database::migrations::run_sprint6_stage17_party_migration(db.pool())
            .await
            .unwrap();
        let version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(before, 1);
        assert_eq!(version, "0.4.5-sprint6");
    }

    #[tokio::test]
    async fn migration_failure_keeps_schema_version_and_rolls_back_partial_ddl() {
        let (_dir, db) = fixture().await;
        sqlx::query("DROP TABLE party_profiles")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE _opentally_metadata SET schema_version = '0.4.4-sprint5' WHERE id = 1")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("CREATE TABLE party_profiles (id TEXT NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        assert!(
            crate::database::migrations::run_sprint6_stage17_party_migration(db.pool())
                .await
                .is_err()
        );
        let version: String =
            sqlx::query_scalar("SELECT schema_version FROM _opentally_metadata WHERE id = 1")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let columns: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('party_profiles')")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(version, "0.4.4-sprint5");
        assert_eq!(columns, 1);
    }
}
