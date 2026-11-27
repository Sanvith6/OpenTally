use crate::errors::OpenTallyError;
use crate::vouchers::service::execute_read_only_query;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Pool, Sqlite};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// 1. Custom Fields (Clean-Room TDL UDF Replacement)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CustomFieldDefinitionDto {
    pub id: String,
    pub company_id: String,
    pub entity_scope: String,
    pub field_key: String,
    pub label: String,
    pub field_type: String,
    pub is_required: i64,
    pub show_on_pdf: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomFieldInput {
    pub entity_scope: String,
    pub field_key: String,
    pub label: String,
    pub field_type: String,
    pub is_required: bool,
    pub show_on_pdf: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFieldValueInput {
    pub definition_id: String,
    pub value_text: String,
}

pub struct CustomFieldService;

impl CustomFieldService {
    pub async fn create_definition(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreateCustomFieldInput,
    ) -> Result<CustomFieldDefinitionDto, OpenTallyError> {
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO custom_field_definitions (
                id, company_id, entity_scope, field_key, label, field_type, is_required, show_on_pdf
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(input.entity_scope.trim())
        .bind(input.field_key.trim())
        .bind(input.label.trim())
        .bind(input.field_type.trim())
        .bind(if input.is_required { 1i64 } else { 0i64 })
        .bind(if input.show_on_pdf { 1i64 } else { 0i64 })
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query_as(
            "SELECT id, company_id, entity_scope, field_key, label, field_type, is_required, show_on_pdf, created_at
             FROM custom_field_definitions WHERE id = ?",
        )
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(OpenTallyError::Database)
    }

    pub async fn list_definitions(
        pool: &Pool<Sqlite>,
        company_id: &str,
        entity_scope: Option<String>,
    ) -> Result<Vec<CustomFieldDefinitionDto>, OpenTallyError> {
        let cid = company_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                if let Some(scope) = entity_scope {
                    sqlx::query_as(
                        "SELECT id, company_id, entity_scope, field_key, label, field_type, is_required, show_on_pdf, created_at
                         FROM custom_field_definitions WHERE company_id = ? AND entity_scope = ? ORDER BY created_at ASC",
                    )
                    .bind(&cid)
                    .bind(scope.trim())
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)
                } else {
                    sqlx::query_as(
                        "SELECT id, company_id, entity_scope, field_key, label, field_type, is_required, show_on_pdf, created_at
                         FROM custom_field_definitions WHERE company_id = ? ORDER BY created_at ASC",
                    )
                    .bind(&cid)
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)
                }
            })
        })
        .await
    }

    pub async fn set_custom_field_values(
        pool: &Pool<Sqlite>,
        company_id: &str,
        entity_id: &str,
        values: Vec<CustomFieldValueInput>,
    ) -> Result<(), OpenTallyError> {
        let mut tx = pool.begin().await.map_err(OpenTallyError::Database)?;
        for v in values {
            let ftype: String = sqlx::query_scalar(
                "SELECT field_type FROM custom_field_definitions WHERE id = ? AND company_id = ?",
            )
            .bind(&v.definition_id)
            .bind(company_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;

            let val = v.value_text.trim();
            match ftype.as_str() {
                "Decimal" => {
                    Decimal::from_str(val).map_err(|_| {
                        OpenTallyError::Validation(format!("Invalid Decimal value '{}'", val))
                    })?;
                }
                "Date" => {
                    NaiveDate::parse_from_str(val, "%Y-%m-%d").map_err(|_| {
                        OpenTallyError::Validation(format!("Invalid Date value '{}'", val))
                    })?;
                }
                "Boolean" if val != "true" && val != "false" && val != "1" && val != "0" => {
                    return Err(OpenTallyError::Validation(format!(
                        "Invalid Boolean value '{}'",
                        val
                    )));
                }
                _ => {}
            }

            sqlx::query(
                "INSERT INTO custom_field_values (id, company_id, definition_id, entity_id, value_text)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT(company_id, definition_id, entity_id) DO UPDATE SET value_text = excluded.value_text",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(company_id)
            .bind(&v.definition_id)
            .bind(entity_id)
            .bind(val)
            .execute(&mut *tx)
            .await
            .map_err(OpenTallyError::Database)?;
        }
        tx.commit().await.map_err(OpenTallyError::Database)?;
        Ok(())
    }

    pub async fn get_pdf_custom_fields(
        pool: &Pool<Sqlite>,
        company_id: &str,
        entity_id: &str,
    ) -> Result<Vec<(String, String)>, OpenTallyError> {
        let cid = company_id.to_string();
        let eid = entity_id.to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT d.label, v.value_text
                     FROM custom_field_values v
                     JOIN custom_field_definitions d ON d.id = v.definition_id
                     WHERE v.company_id = ? AND v.entity_id = ? AND d.show_on_pdf = 1
                     ORDER BY d.created_at ASC",
                )
                .bind(&cid)
                .bind(&eid)
                .fetch_all(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
            })
        })
        .await
    }
}

// ============================================================================
// 2. Signed Event Webhooks (HMAC-SHA256)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookSubscriptionDto {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub event_type: String,
    pub target_url: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWebhookInput {
    pub name: String,
    pub event_type: String,
    pub target_url: String,
    pub hmac_secret: String,
}

pub struct WebhookService;

impl WebhookService {
    pub fn validate_target_url(url: &str) -> Result<(), OpenTallyError> {
        let u = url.trim();
        if u.starts_with("https://")
            || u.starts_with("http://127.0.0.1")
            || u.starts_with("http://localhost")
        {
            Ok(())
        } else {
            Err(OpenTallyError::Validation(
                "Webhook target_url must use https:// (or http://127.0.0.1 / http://localhost)"
                    .into(),
            ))
        }
    }

    /// RFC 2104 HMAC-SHA256 hex digest.
    pub fn hmac_sha256_hex(secret: &str, payload: &str) -> String {
        let mut key_block = [0u8; 64];
        let kb = secret.as_bytes();
        if kb.len() > 64 {
            let mut hasher = Sha256::new();
            hasher.update(kb);
            let digest = hasher.finalize();
            key_block[..32].copy_from_slice(&digest);
        } else {
            key_block[..kb.len()].copy_from_slice(kb);
        }

        let mut ipad = [0x36u8; 64];
        let mut opad = [0x5cu8; 64];
        for i in 0..64 {
            ipad[i] ^= key_block[i];
            opad[i] ^= key_block[i];
        }

        let mut inner = Sha256::new();
        inner.update(ipad);
        inner.update(payload.as_bytes());
        let inner_hash = inner.finalize();

        let mut outer = Sha256::new();
        outer.update(opad);
        outer.update(inner_hash);
        let final_hash = outer.finalize();
        format!("sha256={:x}", final_hash)
    }

    pub async fn create_subscription(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: CreateWebhookInput,
    ) -> Result<WebhookSubscriptionDto, OpenTallyError> {
        Self::validate_target_url(&input.target_url)?;
        if input.hmac_secret.trim().is_empty() {
            return Err(OpenTallyError::Validation(
                "hmac_secret cannot be empty".into(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO webhook_subscriptions (
                id, company_id, name, event_type, target_url, hmac_secret_ref, is_active
             ) VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(company_id)
        .bind(input.name.trim())
        .bind(input.event_type.trim())
        .bind(input.target_url.trim())
        .bind(input.hmac_secret.trim())
        .execute(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        Ok(WebhookSubscriptionDto {
            id,
            company_id: company_id.to_string(),
            name: input.name.trim().to_string(),
            event_type: input.event_type.trim().to_string(),
            target_url: input.target_url.trim().to_string(),
            is_active: true,
            created_at: String::new(),
        })
    }
}

// ============================================================================
// 3. Weighbridge & Barcode Hardware Bridge
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeighbridgeReading {
    pub gross_weight: String,
    pub tare_weight: String,
    pub net_weight: String,
    pub unit: String,
}

pub struct HardwareBridgeService;

impl HardwareBridgeService {
    pub fn parse_weighbridge_reading(
        raw_frame: &str,
        tare_weight: &str,
    ) -> Result<WeighbridgeReading, OpenTallyError> {
        let tare = Decimal::from_str(tare_weight.trim())
            .map_err(|_| OpenTallyError::Validation("Invalid tare_weight".into()))?;
        if tare < Decimal::ZERO {
            return Err(OpenTallyError::Validation(
                "tare_weight cannot be negative".into(),
            ));
        }

        let upper = raw_frame.trim().to_uppercase();
        let unit = if upper.contains("MT") || upper.contains("TON") {
            "MT".to_string()
        } else {
            "KG".to_string()
        };

        let numeric_str: String = raw_frame
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let gross = Decimal::from_str(&numeric_str).map_err(|_| {
            OpenTallyError::Validation(format!(
                "Could not parse numeric weight from frame '{}'",
                raw_frame
            ))
        })?;

        let net = gross - tare;
        if net < Decimal::ZERO {
            return Err(OpenTallyError::Validation(format!(
                "Net weight cannot be negative (gross {}, tare {})",
                gross, tare
            )));
        }

        Ok(WeighbridgeReading {
            gross_weight: gross.round_dp(2).to_string(),
            tare_weight: tare.round_dp(2).to_string(),
            net_weight: net.round_dp(2).to_string(),
            unit,
        })
    }

    pub async fn lookup_stock_item_by_barcode(
        pool: &Pool<Sqlite>,
        company_id: &str,
        barcode: &str,
    ) -> Result<Option<(String, String)>, OpenTallyError> {
        let cid = company_id.to_string();
        let bc = barcode.trim().to_string();
        execute_read_only_query(pool, move |conn| {
            Box::pin(async move {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT id, name FROM stock_items WHERE company_id = ? AND barcode = ? LIMIT 1",
                )
                .bind(&cid)
                .bind(&bc)
                .fetch_optional(&mut *conn)
                .await
                .map_err(OpenTallyError::Database)
            })
        })
        .await
    }
}
