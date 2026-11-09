use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    CorrectionResult, CreateCorrectionDraftRequest, CreateVoucherDraftRequest,
    ReverseVoucherRequest, Voucher, VoucherLine, VoucherLineInput,
};
use crate::vouchers::service::{VoucherDraftService, VoucherReversalService};
use sqlx::{Pool, Sqlite};

/// Authoritative service handling two-step voucher corrections (Option A under DEC-S2-004).
pub struct VoucherCorrectionService;

impl VoucherCorrectionService {
    /// Executes the two-step correction workflow (Option A):
    /// 1. Reverses the original posted voucher via `VoucherReversalService::reverse_voucher`.
    /// 2. Clones the original voucher lines and metadata into a new draft voucher
    ///    via `VoucherDraftService::create_draft`.
    ///
    /// Returns `CorrectionResult` containing:
    /// - `original_voucher`: The reversed original voucher (`status = 'reversed'`)
    /// - `reversing_voucher`: The generated reversing journal voucher (`status = 'posted'`)
    /// - `corrected_draft`: The newly created draft voucher ready for edits (`status = 'draft'`)
    pub async fn create_correction_draft(
        pool: &Pool<Sqlite>,
        company_id: &str,
        req: CreateCorrectionDraftRequest,
    ) -> Result<CorrectionResult, OpenTallyError> {
        // Step 1: Fetch original voucher to obtain metadata
        let orig_voucher = sqlx::query_as::<_, Voucher>(
            "SELECT id, company_id, voucher_type_id, series_id, numbering_method,
                    period_start, sequence_number, voucher_number, date, effective_date,
                    status, reference_no, reference_date, narration, total_amount,
                    reversal_of_voucher_id, created_at, updated_at
             FROM vouchers
             WHERE company_id = $1 AND id = $2",
        )
        .bind(company_id)
        .bind(&req.original_voucher_id)
        .fetch_optional(pool)
        .await
        .map_err(OpenTallyError::Database)?
        .ok_or_else(|| {
            OpenTallyError::NotFound(format!(
                "Original voucher '{}' not found in active company",
                req.original_voucher_id
            ))
        })?;

        // Determine reversal date: user-supplied or default to original voucher's date
        let reversal_date = req
            .reversal_date
            .unwrap_or_else(|| orig_voucher.date.clone());

        // Step 2: Execute Reversal
        let reversal_result = VoucherReversalService::reverse_voucher(
            pool,
            company_id,
            ReverseVoucherRequest {
                original_voucher_id: orig_voucher.id.clone(),
                reversal_date,
                reason: req.reversal_reason.clone(),
                actor_id: req.actor_id.clone(),
            },
        )
        .await?;

        // Step 3: Fetch original lines to clone into the new draft
        let orig_lines = sqlx::query_as::<_, VoucherLine>(
            "SELECT id, company_id, voucher_id, line_number, ledger_id, entry_type, amount, narration, created_at
             FROM voucher_lines
             WHERE company_id = $1 AND voucher_id = $2
             ORDER BY line_number ASC",
        )
        .bind(company_id)
        .bind(&orig_voucher.id)
        .fetch_all(pool)
        .await
        .map_err(OpenTallyError::Database)?;

        let draft_lines: Vec<VoucherLineInput> = orig_lines
            .into_iter()
            .map(|l| VoucherLineInput {
                id: None,
                ledger_id: l.ledger_id,
                entry_type: l.entry_type,
                amount: l.amount,
                narration: l.narration,
            })
            .collect();

        // Step 4: Create new correction draft
        let correction_narration = Some(format!(
            "Correction draft for {}: {}",
            orig_voucher.voucher_number, req.reversal_reason
        ));
        let correction_ref_no = Some(format!("Correction of {}", orig_voucher.voucher_number));

        let draft_req = CreateVoucherDraftRequest {
            voucher_type_id: orig_voucher.voucher_type_id,
            series_id: orig_voucher.series_id,
            date: orig_voucher.date,
            effective_date: Some(orig_voucher.effective_date),
            reference_no: correction_ref_no,
            reference_date: orig_voucher.reference_date,
            narration: correction_narration,
            lines: Some(draft_lines),
        };

        let corrected_draft =
            VoucherDraftService::create_draft(pool, company_id, draft_req).await?;

        Ok(CorrectionResult {
            original_voucher: reversal_result.original_voucher,
            reversing_voucher: reversal_result.reversing_voucher,
            corrected_draft,
        })
    }
}
