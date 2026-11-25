use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AllocationType {
    NewRef,
    AgstRef,
    Advance,
    OnAccount,
}

impl AllocationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NewRef => "NewRef",
            Self::AgstRef => "AgstRef",
            Self::Advance => "Advance",
            Self::OnAccount => "OnAccount",
        }
    }
}

impl fmt::Display for AllocationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AllocationType {
    type Err = OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NewRef" | "new_ref" | "New Ref" => Ok(Self::NewRef),
            "AgstRef" | "agst_ref" | "Agst Ref" => Ok(Self::AgstRef),
            "Advance" | "advance" => Ok(Self::Advance),
            "OnAccount" | "on_account" | "On Account" => Ok(Self::OnAccount),
            other => Err(OpenTallyError::Validation(format!(
                "Invalid bill allocation type '{}'",
                other
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BillAllocation {
    pub id: String,
    pub company_id: String,
    pub voucher_id: String,
    pub voucher_line_id: String,
    pub party_ledger_id: String,
    pub bill_name: String,
    pub allocation_type: String,
    pub amount: String,
    pub dr_cr: String,
    pub bill_date: String,
    pub due_date: Option<String>,
    pub is_cancelled: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillAllocationInput {
    pub bill_name: String,
    pub allocation_type: AllocationType,
    pub amount: String,
    pub dr_cr: Option<String>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBill {
    pub party_ledger_id: String,
    pub party_name: String,
    pub bill_name: String,
    pub bill_date: String,
    pub due_date: Option<String>,
    pub dr_cr: String,
    pub original_amount: String,
    pub settled_amount: String,
    pub pending_amount: String,
    pub overdue_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgingBucketSummary {
    pub not_due: String,
    pub days_0_30: String,
    pub days_31_60: String,
    pub days_61_90: String,
    pub days_90_plus: String,
    pub total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyAgingRow {
    pub party_ledger_id: String,
    pub party_name: String,
    pub buckets: AgingBucketSummary,
    pub bills: Vec<PendingBill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgingReport {
    pub as_of_date: String,
    pub report_type: String,
    pub parties: Vec<PartyAgingRow>,
    pub grand_total: AgingBucketSummary,
}

pub struct BillWiseService;

impl BillWiseService {
    /// Determines whether bill-wise tracking is enabled for this company.
    /// Defaults to enabled unless explicitly set to 'disabled' or 'false' in company_settings.
    pub async fn is_billwise_enabled(
        conn: &mut SqliteConnection,
        company_id: &str,
    ) -> Result<bool, OpenTallyError> {
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bill_allocations'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(false);
        }

        let val: Option<String> = sqlx::query_scalar(
            "SELECT value FROM company_settings WHERE company_id = ? AND key = 'billwise_tracking'",
        )
        .bind(company_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        match val.as_deref() {
            Some("disabled") | Some("false") | Some("0") => Ok(false),
            _ => Ok(true),
        }
    }

    /// Validates and records bill allocations for a specific voucher line within an active transaction.
    /// Enforces:
    /// 1. Sum equality: sum(allocations.amount) == line_amount.
    /// 2. Unique NewRef: No duplicate active NewRef for the same (company_id, party_ledger_id, bill_name).
    /// 3. No over-settlement: AgstRef cannot exceed remaining unpaid balance of the referenced bill.
    #[allow(clippy::too_many_arguments)]
    pub async fn validate_and_post_allocations(
        conn: &mut SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_line_id: &str,
        party_ledger_id: &str,
        line_entry_type: &str,
        line_amount: Decimal,
        bill_date: &str,
        default_due_date: Option<&str>,
        allocations: &[BillAllocationInput],
    ) -> Result<Vec<BillAllocation>, OpenTallyError> {
        if allocations.is_empty() {
            return Ok(Vec::new());
        }

        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bill_allocations'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(Vec::new());
        }

        // Validate that voucher line belongs to voucher in 'posted' status
        let valid_line: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM voucher_lines vl
             JOIN vouchers v ON v.company_id = vl.company_id AND v.id = vl.voucher_id
             WHERE vl.company_id = ? AND vl.voucher_id = ? AND vl.id = ? AND vl.ledger_id = ?
               AND v.status = 'posted' LIMIT 1",
        )
        .bind(company_id)
        .bind(voucher_id)
        .bind(voucher_line_id)
        .bind(party_ledger_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if valid_line.is_none() {
            return Err(OpenTallyError::Validation(format!(
                "Voucher line '{}' not found, does not match party '{}', or voucher '{}' is not posted",
                voucher_line_id, party_ledger_id, voucher_id
            )));
        }

        let mut sum_allocations = Decimal::ZERO;
        let mut new_ref_names = std::collections::HashSet::new();
        let mut agst_ref_accumulated: std::collections::HashMap<String, Decimal> =
            std::collections::HashMap::new();

        // Pass 1: Syntax, Positivity, Invariants, and Sum check
        for (idx, alloc) in allocations.iter().enumerate() {
            let amt = Decimal::from_str_exact(alloc.amount.trim()).map_err(|_| {
                OpenTallyError::Validation(format!(
                    "Allocation #{}: invalid amount '{}'",
                    idx + 1,
                    alloc.amount
                ))
            })?;

            if amt <= Decimal::ZERO {
                return Err(OpenTallyError::Validation(format!(
                    "Allocation #{}: amount must be strictly positive",
                    idx + 1
                )));
            }

            sum_allocations += amt;

            let bill_name = alloc.bill_name.trim();
            if bill_name.is_empty() {
                return Err(OpenTallyError::Validation(format!(
                    "Allocation #{}: bill name cannot be empty",
                    idx + 1
                )));
            }

            match alloc.allocation_type {
                AllocationType::NewRef => {
                    if !new_ref_names.insert(bill_name.to_lowercase()) {
                        return Err(OpenTallyError::Validation(format!(
                            "Duplicate NewRef '{}' in allocation list for party '{}'",
                            bill_name, party_ledger_id
                        )));
                    }

                    // Check database uniqueness for active NewRef
                    let existing: Option<i64> = sqlx::query_scalar(
                        "SELECT 1 FROM bill_allocations
                         WHERE company_id = ? AND party_ledger_id = ? AND lower(bill_name) = ?
                           AND allocation_type = 'NewRef' AND is_cancelled = 0 LIMIT 1",
                    )
                    .bind(company_id)
                    .bind(party_ledger_id)
                    .bind(bill_name.to_lowercase())
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    if existing.is_some() {
                        return Err(OpenTallyError::Validation(format!(
                            "NewRef bill '{}' already exists for party '{}'",
                            bill_name, party_ledger_id
                        )));
                    }
                }
                AllocationType::AgstRef => {
                    // Find active original bill (NewRef or Advance)
                    let orig: Option<(String, String)> = sqlx::query_as(
                        "SELECT amount, dr_cr FROM bill_allocations
                         WHERE company_id = ? AND party_ledger_id = ? AND lower(bill_name) = ?
                           AND allocation_type IN ('NewRef', 'Advance') AND is_cancelled = 0
                         ORDER BY created_at ASC LIMIT 1",
                    )
                    .bind(company_id)
                    .bind(party_ledger_id)
                    .bind(bill_name.to_lowercase())
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    let (orig_amt_str, orig_dr_cr) = orig.ok_or_else(|| {
                        OpenTallyError::Validation(format!(
                            "Cannot settle bill '{}': no active NewRef or Advance exists for party '{}'",
                            bill_name, party_ledger_id
                        ))
                    })?;

                    // Opposing entry validation: Dr bill must be settled by Cr; Cr bill must be settled by Dr
                    let alloc_dr_cr = alloc.dr_cr.as_deref().unwrap_or(line_entry_type);
                    let expected_settlement_dr_cr = match orig_dr_cr.as_str() {
                        "Dr" => "Cr",
                        "Cr" => "Dr",
                        other => {
                            return Err(OpenTallyError::IntegrityViolation(format!(
                                "Corrupted dr_cr '{}' on original bill '{}'",
                                other, bill_name
                            )))
                        }
                    };
                    if alloc_dr_cr != expected_settlement_dr_cr {
                        return Err(OpenTallyError::Validation(format!(
                            "Cannot settle {} bill '{}' with a {} allocation. Expected {} settlement.",
                            orig_dr_cr, bill_name, alloc_dr_cr, expected_settlement_dr_cr
                        )));
                    }

                    let orig_amt = Decimal::from_str_exact(&orig_amt_str).map_err(|_| {
                        OpenTallyError::IntegrityViolation(format!(
                            "Corrupted bill amount '{}' for bill '{}'",
                            orig_amt_str, bill_name
                        ))
                    })?;

                    // Calculate total settled so far by active AgstRef allocations
                    let settled_str_list: Vec<String> = sqlx::query_scalar(
                        "SELECT amount FROM bill_allocations
                         WHERE company_id = ? AND party_ledger_id = ? AND lower(bill_name) = ?
                           AND allocation_type = 'AgstRef' AND is_cancelled = 0",
                    )
                    .bind(company_id)
                    .bind(party_ledger_id)
                    .bind(bill_name.to_lowercase())
                    .fetch_all(&mut *conn)
                    .await
                    .map_err(OpenTallyError::Database)?;

                    let mut total_settled = Decimal::ZERO;
                    for s in settled_str_list {
                        let parsed = Decimal::from_str_exact(&s).unwrap_or(Decimal::ZERO);
                        total_settled += parsed;
                    }

                    let in_batch_settled = agst_ref_accumulated
                        .get(&bill_name.to_lowercase())
                        .copied()
                        .unwrap_or(Decimal::ZERO);
                    let remaining = orig_amt - (total_settled + in_batch_settled);
                    if amt > remaining {
                        return Err(OpenTallyError::Validation(format!(
                            "Over-settlement of bill '{}': attempting to allocate {}, but only {} remains unpaid",
                            bill_name, amt, remaining
                        )));
                    }
                    *agst_ref_accumulated
                        .entry(bill_name.to_lowercase())
                        .or_default() += amt;
                }
                AllocationType::Advance | AllocationType::OnAccount => {
                    // Valid standalone allocation types
                }
            }
        }

        // Invariant 1: Sum of allocations must equal voucher line amount
        if sum_allocations != line_amount {
            return Err(OpenTallyError::Validation(format!(
                "Bill allocation sum ({}) does not match voucher line amount ({})",
                sum_allocations, line_amount
            )));
        }

        // Pass 2: Insert validated bill allocations
        let mut inserted = Vec::with_capacity(allocations.len());
        for alloc in allocations {
            let id = Uuid::new_v4().to_string();
            let amt = Decimal::from_str_exact(alloc.amount.trim())
                .map_err(|_| {
                    OpenTallyError::Validation(format!(
                        "Invalid allocation amount '{}'",
                        alloc.amount
                    ))
                })?
                .normalize()
                .to_string();
            let dr_cr = alloc
                .dr_cr
                .as_deref()
                .unwrap_or(line_entry_type)
                .to_string();
            let due_date = alloc.due_date.as_deref().or(default_due_date);

            sqlx::query(
                "INSERT INTO bill_allocations (
                    id, company_id, voucher_id, voucher_line_id, party_ledger_id,
                    bill_name, allocation_type, amount, dr_cr, bill_date, due_date, is_cancelled
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
            )
            .bind(&id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(voucher_line_id)
            .bind(party_ledger_id)
            .bind(alloc.bill_name.trim())
            .bind(alloc.allocation_type.as_str())
            .bind(&amt)
            .bind(&dr_cr)
            .bind(bill_date)
            .bind(due_date)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            inserted.push(BillAllocation {
                id,
                company_id: company_id.to_string(),
                voucher_id: voucher_id.to_string(),
                voucher_line_id: voucher_line_id.to_string(),
                party_ledger_id: party_ledger_id.to_string(),
                bill_name: alloc.bill_name.trim().to_string(),
                allocation_type: alloc.allocation_type.as_str().to_string(),
                amount: amt,
                dr_cr,
                bill_date: bill_date.to_string(),
                due_date: due_date.map(ToString::to_string),
                is_cancelled: 0,
                created_at: String::new(),
            });
        }

        Ok(inserted)
    }

    /// Verifies whether a voucher can be cancelled or reversed with respect to bill allocations.
    /// Rejects if any NewRef created by this voucher has been settled by subsequent AgstRef vouchers.
    /// If unconsumed or if this voucher holds settlements, marks allocations is_cancelled = 1 and emits audit log.
    pub async fn verify_and_cancel_billwise(
        conn: &mut SqliteConnection,
        company_id: &str,
        voucher_id: &str,
        voucher_number: &str,
    ) -> Result<(), OpenTallyError> {
        // Defensive check: if table does not exist in this database schema, skip cleanly
        let table_exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'bill_allocations'",
        )
        .fetch_optional(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        if table_exists.is_none() {
            return Ok(());
        }

        // 1. Check if any NewRef or Advance created by this voucher has active AgstRef settlements
        let active_new_refs: Vec<(String, String)> = sqlx::query_as(
            "SELECT bill_name, party_ledger_id FROM bill_allocations
             WHERE company_id = ? AND voucher_id = ?
               AND allocation_type IN ('NewRef', 'Advance') AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        for (bill_name, party_id) in &active_new_refs {
            let settlement_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM bill_allocations
                 WHERE company_id = ? AND party_ledger_id = ? AND lower(bill_name) = ?
                   AND allocation_type = 'AgstRef' AND is_cancelled = 0 AND voucher_id != ?",
            )
            .bind(company_id)
            .bind(party_id)
            .bind(bill_name.to_lowercase())
            .bind(voucher_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;

            if settlement_count > 0 {
                return Err(OpenTallyError::Validation(format!(
                    "Cannot cancel/reverse voucher '{}': bill reference '{}' has active settlements. Cancel settlement vouchers first.",
                    voucher_number, bill_name
                )));
            }
        }

        // 2. Mark all bill allocations of this voucher as cancelled
        let cancelled_rows = sqlx::query(
            "UPDATE bill_allocations
             SET is_cancelled = 1
             WHERE company_id = ? AND voucher_id = ? AND is_cancelled = 0",
        )
        .bind(company_id)
        .bind(voucher_id)
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?
        .rows_affected();

        // 3. Emit MCA Audit Log
        if cancelled_rows > 0 {
            let audit_id = Uuid::new_v4().to_string();
            let payload = serde_json::json!({
                "action": "BILL_ALLOCATIONS_CANCEL",
                "voucher_id": voucher_id,
                "voucher_number": voucher_number,
                "cancelled_allocations_count": cancelled_rows,
            })
            .to_string();

            sqlx::query(
                "INSERT INTO audit_log (
                    id, company_id, entity_type, entity_id, action, new_values, performed_by
                 ) VALUES (?, ?, 'bill_allocation', ?, 'CANCEL', ?, 'system')",
            )
            .bind(&audit_id)
            .bind(company_id)
            .bind(voucher_id)
            .bind(&payload)
            .execute(&mut *conn)
            .await
            .map_err(OpenTallyError::Database)?;
        }

        Ok(())
    }
}
