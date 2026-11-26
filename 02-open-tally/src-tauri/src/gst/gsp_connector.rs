//! Milestone 18: BYO-GSP e-Invoice & e-Way Bill Connector
//!
//! Provides a pluggable `GspProvider` trait with:
//! - `SandboxGspProvider`: deterministic offline simulator computing 64-char SHA-256 IRN + Ack No + Ack Date (or 12-digit e-Way Bill No + Valid Upto).
//! - `HttpGspProvider`: configurable BYO-Credentials HTTPS GSP/IRP endpoint (strict 15s timeout, rejects non-localhost `http://`).

use crate::audit::certification::AuditCertificationService;
use crate::errors::OpenTallyError;
use crate::gst::einvoice::EinvoiceService;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Pool, Sqlite};
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GspProviderMode {
    Sandbox,
    HttpGsp,
}

impl GspProviderMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sandbox => "Sandbox",
            Self::HttpGsp => "HttpGsp",
        }
    }

    pub fn parse(s: &str) -> Result<Self, OpenTallyError> {
        match s {
            "Sandbox" => Ok(Self::Sandbox),
            "HttpGsp" => Ok(Self::HttpGsp),
            other => Err(OpenTallyError::Validation(format!(
                "Invalid GSP provider_mode '{}'. Expected 'Sandbox' or 'HttpGsp'.",
                other
            ))),
        }
    }
}

/// Public DTO for company GSP configuration (never exposes `api_secret_ref` plaintext!).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyGspConfigDto {
    pub company_id: String,
    pub provider_mode: String,
    pub provider_name: String,
    pub base_url: Option<String>,
    pub client_id: Option<String>,
    pub has_api_secret: bool,
    pub is_enabled: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveCompanyGspConfigInput {
    pub provider_mode: String,
    pub provider_name: String,
    pub base_url: Option<String>,
    pub client_id: Option<String>,
    pub api_secret_ref: Option<String>,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GspEinvoiceResponse {
    pub irn: String,
    pub ack_no: String,
    pub ack_date: String,
    pub signed_qr_code: Option<String>,
    pub provider_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GspEwayBillResponse {
    pub ewb_no: String,
    pub ewb_date: String,
    pub ewb_valid_upto: String,
    pub provider_mode: String,
}

pub type GspFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, OpenTallyError>> + Send + 'a>>;

pub trait GspProvider: Send + Sync {
    fn push_einvoice<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEinvoiceResponse>;
    fn push_eway_bill<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEwayBillResponse>;
}

/// Deterministic offline Sandbox GSP Simulator.
pub struct SandboxGspProvider;

impl GspProvider for SandboxGspProvider {
    fn push_einvoice<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEinvoiceResponse> {
        Box::pin(async move {
            let serialized = serde_json::to_string(payload)
                .map_err(|e| OpenTallyError::Internal(format!("JSON serialize error: {}", e)))?;
            let mut hasher = Sha256::new();
            hasher.update(b"OPENTALLY_SANDBOX_IRN_V1:");
            hasher.update(serialized.as_bytes());
            let hash = hasher.finalize();
            let irn = format!("{:x}", hash);

            // Deterministic 15-digit numeric Ack No derived from first 8 bytes
            let ack_num = u64::from_be_bytes([
                hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
            ]) % 900_000_000_000_000
                + 100_000_000_000_000;
            let ack_date = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

            Ok(GspEinvoiceResponse {
                irn: irn.clone(),
                ack_no: ack_num.to_string(),
                ack_date,
                signed_qr_code: Some(format!("SANDBOX-QR:{}", &irn[..16])),
                provider_mode: "Sandbox".to_string(),
            })
        })
    }

    fn push_eway_bill<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEwayBillResponse> {
        Box::pin(async move {
            let serialized = serde_json::to_string(payload)
                .map_err(|e| OpenTallyError::Internal(format!("JSON serialize error: {}", e)))?;
            let mut hasher = Sha256::new();
            hasher.update(b"OPENTALLY_SANDBOX_EWB_V1:");
            hasher.update(serialized.as_bytes());
            let hash = hasher.finalize();

            let ewb_num = u64::from_be_bytes([
                hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
            ]) % 900_000_000_000
                + 100_000_000_000;
            let now = Utc::now();
            let valid_upto = now + Duration::days(3);

            Ok(GspEwayBillResponse {
                ewb_no: ewb_num.to_string(),
                ewb_date: now.format("%Y-%m-%d %H:%M:%S").to_string(),
                ewb_valid_upto: valid_upto.format("%Y-%m-%d %H:%M:%S").to_string(),
                provider_mode: "Sandbox".to_string(),
            })
        })
    }
}

/// Configurable BYO-Credentials HTTPS GSP/IRP Provider.
pub struct HttpGspProvider {
    pub base_url: String,
    pub client_id: Option<String>,
    pub api_secret: Option<String>,
}

impl HttpGspProvider {
    pub fn validate_url(url: &str) -> Result<(), OpenTallyError> {
        let trimmed = url.trim();
        if trimmed.starts_with("https://")
            || trimmed.starts_with("http://127.0.0.1")
            || trimmed.starts_with("http://localhost")
        {
            Ok(())
        } else {
            Err(OpenTallyError::Validation(
                "Insecure GSP endpoint rejected: base_url must use https:// (or http://localhost / http://127.0.0.1 for local testing).".to_string(),
            ))
        }
    }

    fn build_client(&self) -> Result<reqwest::Client, OpenTallyError> {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| OpenTallyError::Internal(format!("HTTP client init failed: {}", e)))
    }
}

impl GspProvider for HttpGspProvider {
    fn push_einvoice<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEinvoiceResponse> {
        Box::pin(async move {
            Self::validate_url(&self.base_url)?;
            let client = self.build_client()?;
            let endpoint = format!("{}/einvoice", self.base_url.trim_end_matches('/'));
            let mut req = client.post(&endpoint).json(payload);
            if let Some(cid) = &self.client_id {
                req = req.header("X-Client-Id", cid);
            }
            if let Some(sec) = &self.api_secret {
                req = req.bearer_auth(sec);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| OpenTallyError::Internal(format!("GSP HTTP request failed: {}", e)))?;
            if !resp.status().is_success() {
                return Err(OpenTallyError::Validation(format!(
                    "GSP endpoint returned HTTP {}",
                    resp.status()
                )));
            }
            let body: serde_json::Value = resp.json().await.map_err(|e| {
                OpenTallyError::Internal(format!("Invalid GSP JSON response: {}", e))
            })?;
            Ok(GspEinvoiceResponse {
                irn: body["Irn"]
                    .as_str()
                    .or_else(|| body["irn"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                ack_no: body["AckNo"]
                    .as_str()
                    .or_else(|| body["ack_no"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                ack_date: body["AckDt"]
                    .as_str()
                    .or_else(|| body["ack_date"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                signed_qr_code: body["SignedQRCode"].as_str().map(|s| s.to_string()),
                provider_mode: "HttpGsp".to_string(),
            })
        })
    }

    fn push_eway_bill<'a>(
        &'a self,
        payload: &'a serde_json::Value,
    ) -> GspFuture<'a, GspEwayBillResponse> {
        Box::pin(async move {
            Self::validate_url(&self.base_url)?;
            let client = self.build_client()?;
            let endpoint = format!("{}/ewaybill", self.base_url.trim_end_matches('/'));
            let mut req = client.post(&endpoint).json(payload);
            if let Some(cid) = &self.client_id {
                req = req.header("X-Client-Id", cid);
            }
            if let Some(sec) = &self.api_secret {
                req = req.bearer_auth(sec);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| OpenTallyError::Internal(format!("GSP HTTP request failed: {}", e)))?;
            if !resp.status().is_success() {
                return Err(OpenTallyError::Validation(format!(
                    "GSP endpoint returned HTTP {}",
                    resp.status()
                )));
            }
            let body: serde_json::Value = resp.json().await.map_err(|e| {
                OpenTallyError::Internal(format!("Invalid GSP JSON response: {}", e))
            })?;
            Ok(GspEwayBillResponse {
                ewb_no: body["EwbNo"]
                    .as_str()
                    .or_else(|| body["ewb_no"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                ewb_date: body["EwbDt"]
                    .as_str()
                    .or_else(|| body["ewb_date"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                ewb_valid_upto: body["EwbValidTill"]
                    .as_str()
                    .or_else(|| body["ewb_valid_upto"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                provider_mode: "HttpGsp".to_string(),
            })
        })
    }
}

pub struct GspConnectorService;

impl GspConnectorService {
    pub async fn get_config(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<CompanyGspConfigDto, OpenTallyError> {
        #[allow(clippy::type_complexity)]
        let row: Option<(String, String, String, Option<String>, Option<String>, Option<String>, i64, String)> = sqlx::query_as(
            "SELECT company_id, provider_mode, provider_name, base_url, client_id, api_secret_ref, is_enabled, updated_at
             FROM company_gsp_configs WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((cid, mode, name, base_url, client_id, secret_ref, enabled, updated_at)) = row {
            Ok(CompanyGspConfigDto {
                company_id: cid,
                provider_mode: mode,
                provider_name: name,
                base_url,
                client_id,
                has_api_secret: secret_ref.as_deref().is_some_and(|s| !s.trim().is_empty()),
                is_enabled: enabled == 1,
                updated_at,
            })
        } else {
            Ok(CompanyGspConfigDto {
                company_id: company_id.to_string(),
                provider_mode: "Sandbox".to_string(),
                provider_name: "OpenTally Sandbox".to_string(),
                base_url: None,
                client_id: None,
                has_api_secret: false,
                is_enabled: true,
                updated_at: Utc::now().to_rfc3339(),
            })
        }
    }

    pub async fn save_config(
        pool: &Pool<Sqlite>,
        company_id: &str,
        input: SaveCompanyGspConfigInput,
        actor: &str,
    ) -> Result<CompanyGspConfigDto, OpenTallyError> {
        let mode = GspProviderMode::parse(&input.provider_mode)?;
        if mode == GspProviderMode::HttpGsp {
            let url = input.base_url.as_deref().ok_or_else(|| {
                OpenTallyError::Validation(
                    "base_url is required when provider_mode is HttpGsp".into(),
                )
            })?;
            HttpGspProvider::validate_url(url)?;
        }

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let existing_secret: Option<String> = sqlx::query_scalar(
            "SELECT api_secret_ref FROM company_gsp_configs WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .flatten();

        let final_secret = match input.api_secret_ref {
            Some(ref s) if !s.trim().is_empty() => Some(s.clone()),
            _ => existing_secret,
        };

        sqlx::query(
            r#"
            INSERT INTO company_gsp_configs (company_id, provider_mode, provider_name, base_url, client_id, api_secret_ref, is_enabled, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id) DO UPDATE SET
                provider_mode = excluded.provider_mode,
                provider_name = excluded.provider_name,
                base_url = excluded.base_url,
                client_id = excluded.client_id,
                api_secret_ref = excluded.api_secret_ref,
                is_enabled = excluded.is_enabled,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(company_id)
        .bind(mode.as_str())
        .bind(input.provider_name.trim())
        .bind(input.base_url.as_deref().map(|s| s.trim()))
        .bind(input.client_id.as_deref().map(|s| s.trim()))
        .bind(&final_secret)
        .bind(if input.is_enabled { 1_i64 } else { 0_i64 })
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        // Never log api_secret_ref in audit_log!
        let details = format!(
            "Updated GSP config: mode={}, provider='{}', enabled={}",
            mode.as_str(),
            input.provider_name.trim(),
            input.is_enabled
        );
        let _ = AuditCertificationService::record_chained_event(
            pool,
            company_id,
            "GSP_CONFIG",
            company_id,
            "UPDATE",
            actor,
            Some(&details),
        )
        .await;

        Self::get_config(pool, company_id).await
    }

    async fn resolve_provider(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<Box<dyn GspProvider>, OpenTallyError> {
        #[allow(clippy::type_complexity)]
        let row: Option<(String, Option<String>, Option<String>, Option<String>, i64)> = sqlx::query_as(
            "SELECT provider_mode, base_url, client_id, api_secret_ref, is_enabled FROM company_gsp_configs WHERE company_id = ?",
        )
        .bind(company_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        if let Some((mode, base_url, client_id, api_secret, is_enabled)) = row {
            if is_enabled != 1 {
                return Err(OpenTallyError::Validation(
                    "GSP integration is currently disabled for this company.".into(),
                ));
            }
            if mode == "HttpGsp" {
                let url = base_url.ok_or_else(|| {
                    OpenTallyError::Validation("GSP base_url is not configured.".into())
                })?;
                HttpGspProvider::validate_url(&url)?;
                return Ok(Box::new(HttpGspProvider {
                    base_url: url,
                    client_id,
                    api_secret,
                }));
            }
        }
        Ok(Box::new(SandboxGspProvider))
    }

    pub async fn push_einvoice_to_gsp(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        actor: &str,
    ) -> Result<GspEinvoiceResponse, OpenTallyError> {
        let json_str = EinvoiceService::export_einvoice_json(pool, company_id, voucher_id).await?;
        let payload: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| OpenTallyError::Internal(format!("Invalid e-Invoice JSON: {}", e)))?;
        let provider = Self::resolve_provider(pool, company_id).await?;
        let resp = provider.push_einvoice(&payload).await?;

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO voucher_einvoice_details (id, company_id, voucher_id, irn, ack_no, ack_date, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id, voucher_id) DO UPDATE SET
                irn = excluded.irn,
                ack_no = excluded.ack_no,
                ack_date = excluded.ack_date,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(&resp.irn)
        .bind(&resp.ack_no)
        .bind(&resp.ack_date)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let details = format!(
            "Pushed e-Invoice via {} GSP: IRN={}, AckNo={}",
            resp.provider_mode, resp.irn, resp.ack_no
        );
        let _ = AuditCertificationService::record_chained_event(
            pool,
            company_id,
            "VOUCHER_EINVOICE",
            voucher_id,
            "GSP_PUSH",
            actor,
            Some(&details),
        )
        .await;

        Ok(resp)
    }

    pub async fn push_eway_bill_to_gsp(
        pool: &Pool<Sqlite>,
        company_id: &str,
        voucher_id: &str,
        actor: &str,
    ) -> Result<GspEwayBillResponse, OpenTallyError> {
        let json_str = EinvoiceService::export_eway_bill_json(
            pool,
            company_id,
            voucher_id,
            crate::gst::einvoice::TransportDetailsInput {
                transporter_id: None,
                transporter_name: None,
                transport_mode: "1".to_string(),
                distance_km: 100,
                trans_doc_no: None,
                trans_doc_date: None,
                vehicle_no: Some("KA01AB1234".to_string()),
                vehicle_type: Some("R".to_string()),
            },
        )
        .await?;
        let payload: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| OpenTallyError::Internal(format!("Invalid e-Way Bill JSON: {}", e)))?;
        let provider = Self::resolve_provider(pool, company_id).await?;
        let resp = provider.push_eway_bill(&payload).await?;

        let mut conn = pool.acquire().await.map_err(OpenTallyError::Database)?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r#"
            INSERT INTO voucher_einvoice_details (id, company_id, voucher_id, eway_bill_no, eway_bill_date, updated_at)
            VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(company_id, voucher_id) DO UPDATE SET
                eway_bill_no = excluded.eway_bill_no,
                eway_bill_date = excluded.eway_bill_date,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&id)
        .bind(company_id)
        .bind(voucher_id)
        .bind(&resp.ewb_no)
        .bind(&resp.ewb_date)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        sqlx::query("COMMIT")
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

        let details = format!(
            "Pushed e-Way Bill via {} GSP: EWB={}, ValidUpto={}",
            resp.provider_mode, resp.ewb_no, resp.ewb_valid_upto
        );
        let _ = AuditCertificationService::record_chained_event(
            pool,
            company_id,
            "VOUCHER_EWB",
            voucher_id,
            "GSP_PUSH",
            actor,
            Some(&details),
        )
        .await;

        Ok(resp)
    }
}
