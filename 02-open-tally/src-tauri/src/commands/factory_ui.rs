//! Typed UI adapters for independently implemented factory services.
use super::require_active_company;
use crate::app::AppState;
use crate::errors::OpenTallyError;
use crate::extensions::{
    CreateCustomFieldInput, CreateWebhookInput, CustomFieldDefinitionDto, CustomFieldService,
    WebhookService, WebhookSubscriptionDto,
};
use crate::inventory::bom::BomService;
use crate::vouchers::service::execute_read_only_query;
use rust_decimal::Decimal;
use std::str::FromStr;

async fn company(state: &AppState, company_id: Option<String>) -> Result<String, OpenTallyError> {
    match company_id {
        Some(id) => Ok(id),
        None => require_active_company(state).await,
    }
}

#[tauri::command]
pub async fn create_custom_field_definition(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: CreateCustomFieldInput,
) -> Result<CustomFieldDefinitionDto, OpenTallyError> {
    let cid = company(&state, company_id).await?;
    CustomFieldService::create_definition(state.db.pool(), &cid, input).await
}

#[tauri::command]
pub async fn list_custom_field_definitions(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    entity_scope: Option<String>,
) -> Result<Vec<CustomFieldDefinitionDto>, OpenTallyError> {
    let cid = company(&state, company_id).await?;
    CustomFieldService::list_definitions(state.db.pool(), &cid, entity_scope).await
}

#[tauri::command]
pub async fn create_webhook_subscription(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    input: CreateWebhookInput,
) -> Result<WebhookSubscriptionDto, OpenTallyError> {
    let cid = company(&state, company_id).await?;
    WebhookService::create_subscription(state.db.pool(), &cid, input).await
}

#[tauri::command]
pub async fn list_webhook_subscriptions(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
) -> Result<Vec<WebhookSubscriptionDto>, OpenTallyError> {
    let cid = company(&state, company_id).await?;
    execute_read_only_query(state.db.pool(),move |conn| Box::pin(async move {
        // Never select, serialize or return the stored HMAC secret.
        let rows=sqlx::query_as::<_,(String,String,String,String,String,i64,String)>(
            "SELECT id,company_id,name,event_type,target_url,is_active,created_at FROM webhook_subscriptions WHERE company_id=? ORDER BY created_at,id")
            .bind(cid).fetch_all(&mut *conn).await.map_err(OpenTallyError::Database)?;
        Ok(rows.into_iter().map(|row| WebhookSubscriptionDto {id:row.0,company_id:row.1,name:row.2,event_type:row.3,target_url:row.4,is_active:row.5!=0,created_at:row.6}).collect())
    })).await
}

#[tauri::command]
pub async fn expand_multilevel_bom(
    state: tauri::State<'_, AppState>,
    company_id: Option<String>,
    bom_id: String,
    target_qty: String,
) -> Result<Vec<(String, String)>, OpenTallyError> {
    let cid = company(&state, company_id).await?;
    let qty = decimal(&target_qty)?;
    Ok(
        BomService::expand_multilevel_bom(state.db.pool(), &cid, &bom_id, qty)
            .await?
            .into_iter()
            .map(|(id, qty)| (id, qty.normalize().to_string()))
            .collect(),
    )
}

fn decimal(value: &str) -> Result<Decimal, OpenTallyError> {
    Decimal::from_str(value.trim())
        .map_err(|_| OpenTallyError::Validation("A valid exact decimal is required".into()))
}

#[tauri::command]
pub fn compute_net_unit_cost_with_byproducts(
    raw_cost: String,
    overhead_cost: String,
    byproduct_recovery_value: String,
    produced_qty: String,
) -> Result<String, OpenTallyError> {
    Ok(BomService::compute_net_unit_cost_with_byproducts(
        decimal(&raw_cost)?,
        decimal(&overhead_cost)?,
        decimal(&byproduct_recovery_value)?,
        decimal(&produced_qty)?,
    )?
    .to_string())
}
