use crate::errors::OpenTallyError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Pool, Row, Sqlite};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditIntegrityIssue {
    pub entry_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub timestamp: String,
    pub expected_hash: String,
    pub actual_hash: String,
    pub issue_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditCertificationReport {
    pub company_id: String,
    pub generated_at: String,
    pub total_entries: usize,
    pub chained_entries: usize,
    pub legacy_unchained_entries: usize,
    pub tampered_entries: usize,
    pub broken_chain_links: usize,
    pub is_valid: bool,
    pub certification_status: String,
    pub first_entry_timestamp: Option<String>,
    pub latest_entry_timestamp: Option<String>,
    pub latest_entry_hash: Option<String>,
    pub issues: Vec<AuditIntegrityIssue>,
}

pub struct AuditCertificationService;

impl AuditCertificationService {
    /// Computes the cryptographic SHA-256 hash for an audit log entry.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_entry_hash(
        prev_hash: &str,
        id: &str,
        company_id: &str,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        timestamp: &str,
        new_values: Option<&str>,
    ) -> String {
        let payload = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            prev_hash,
            id,
            company_id,
            entity_type,
            entity_id,
            action,
            timestamp,
            new_values.unwrap_or("")
        );
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verifies the cryptographic hash-chain integrity of all audit_log records for a company.
    pub async fn verify_audit_trail_integrity(
        pool: &Pool<Sqlite>,
        company_id: &str,
    ) -> Result<AuditCertificationReport, OpenTallyError> {
        let rows = sqlx::query(
            "SELECT id, company_id, entity_type, entity_id, action, new_values, timestamp, prev_hash, entry_hash
             FROM audit_log
             WHERE company_id = ?
             ORDER BY timestamp ASC, id ASC",
        )
        .bind(company_id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let total_entries = rows.len();
        let mut chained_entries = 0;
        let mut legacy_unchained_entries = 0;
        let mut tampered_entries = 0;
        let mut broken_chain_links = 0;
        let mut issues = Vec::new();

        let mut expected_prev_hash = "GENESIS".to_string();
        let mut first_entry_timestamp = None;
        let mut latest_entry_timestamp = None;
        let mut latest_entry_hash = None;

        for r in rows {
            let id: String = r.try_get("id").map_err(OpenTallyError::Database)?;
            let entity_type: String = r.try_get("entity_type").map_err(OpenTallyError::Database)?;
            let entity_id: String = r.try_get("entity_id").map_err(OpenTallyError::Database)?;
            let action: String = r.try_get("action").map_err(OpenTallyError::Database)?;
            let new_values: Option<String> =
                r.try_get("new_values").map_err(OpenTallyError::Database)?;
            let timestamp: String = r.try_get("timestamp").map_err(OpenTallyError::Database)?;
            let prev_hash: Option<String> =
                r.try_get("prev_hash").map_err(OpenTallyError::Database)?;
            let entry_hash: Option<String> =
                r.try_get("entry_hash").map_err(OpenTallyError::Database)?;

            if first_entry_timestamp.is_none() {
                first_entry_timestamp = Some(timestamp.clone());
            }
            latest_entry_timestamp = Some(timestamp.clone());

            match (prev_hash, entry_hash) {
                (Some(actual_prev), Some(actual_entry)) => {
                    chained_entries += 1;
                    latest_entry_hash = Some(actual_entry.clone());

                    // 1. Verify prev_hash link
                    if actual_prev != expected_prev_hash {
                        broken_chain_links += 1;
                        issues.push(AuditIntegrityIssue {
                            entry_id: id.clone(),
                            entity_type: entity_type.clone(),
                            entity_id: entity_id.clone(),
                            timestamp: timestamp.clone(),
                            expected_hash: expected_prev_hash.clone(),
                            actual_hash: actual_prev.clone(),
                            issue_type: "BrokenPrevHashLink".to_string(),
                        });
                    }

                    // 2. Recompute and verify entry_hash
                    let expected_entry = Self::compute_entry_hash(
                        &actual_prev,
                        &id,
                        company_id,
                        &entity_type,
                        &entity_id,
                        &action,
                        &timestamp,
                        new_values.as_deref(),
                    );

                    if actual_entry != expected_entry {
                        tampered_entries += 1;
                        issues.push(AuditIntegrityIssue {
                            entry_id: id.clone(),
                            entity_type: entity_type.clone(),
                            entity_id: entity_id.clone(),
                            timestamp: timestamp.clone(),
                            expected_hash: expected_entry,
                            actual_hash: actual_entry.clone(),
                            issue_type: "TamperedContent".to_string(),
                        });
                    }

                    // Chain forward using recorded actual_entry
                    expected_prev_hash = actual_entry;
                }
                _ => {
                    legacy_unchained_entries += 1;
                }
            }
        }

        let is_valid = tampered_entries == 0 && broken_chain_links == 0;
        let certification_status = if is_valid {
            "VERIFIED_CLEAN".to_string()
        } else {
            "TAMPER_DETECTED".to_string()
        };

        Ok(AuditCertificationReport {
            company_id: company_id.to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_entries,
            chained_entries,
            legacy_unchained_entries,
            tampered_entries,
            broken_chain_links,
            is_valid,
            certification_status,
            first_entry_timestamp,
            latest_entry_timestamp,
            latest_entry_hash,
            issues,
        })
    }

    /// Exports the audit certification report as pretty JSON.
    pub fn export_certification_json(
        report: &AuditCertificationReport,
    ) -> Result<String, OpenTallyError> {
        serde_json::to_string_pretty(report).map_err(|e| {
            OpenTallyError::Validation(format!(
                "Failed to serialize audit certification report: {}",
                e
            ))
        })
    }

    /// Exports the audit certification report as CSV.
    pub fn export_certification_csv(report: &AuditCertificationReport) -> String {
        let mut csv = String::new();
        csv.push_str("Summary Parameter,Value\n");
        csv.push_str(&format!("Company ID,\"{}\"\n", report.company_id));
        csv.push_str(&format!("Generated At,\"{}\"\n", report.generated_at));
        csv.push_str(&format!("Status,\"{}\"\n", report.certification_status));
        csv.push_str(&format!("Total Entries,{}\n", report.total_entries));
        csv.push_str(&format!("Chained Entries,{}\n", report.chained_entries));
        csv.push_str(&format!(
            "Legacy Unchained Entries,{}\n",
            report.legacy_unchained_entries
        ));
        csv.push_str(&format!("Tampered Entries,{}\n", report.tampered_entries));
        csv.push_str(&format!(
            "Broken Chain Links,{}\n",
            report.broken_chain_links
        ));
        csv.push_str(&format!(
            "First Entry Timestamp,\"{}\"\n",
            report.first_entry_timestamp.as_deref().unwrap_or("N/A")
        ));
        csv.push_str(&format!(
            "Latest Entry Timestamp,\"{}\"\n",
            report.latest_entry_timestamp.as_deref().unwrap_or("N/A")
        ));
        csv.push_str(&format!(
            "Latest Entry Hash,\"{}\"\n\n",
            report.latest_entry_hash.as_deref().unwrap_or("N/A")
        ));

        csv.push_str("Discrepancies and Issues\n");
        csv.push_str(
            "Entry ID,Entity Type,Entity ID,Timestamp,Issue Type,Expected Hash,Actual Hash\n",
        );
        for issue in &report.issues {
            csv.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                issue.entry_id,
                issue.entity_type,
                issue.entity_id,
                issue.timestamp,
                issue.issue_type,
                issue.expected_hash,
                issue.actual_hash
            ));
        }

        csv
    }
}
