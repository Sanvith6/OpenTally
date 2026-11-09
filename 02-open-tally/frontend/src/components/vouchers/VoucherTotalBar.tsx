interface VoucherTotalBarProps {
  debitTotal: string;
  creditTotal: string;
  difference: string;
  isBalanced: boolean;
  onSaveDraft: () => void;
  onOpenDraftsModal: () => void;
  onNewDraft: () => void;
  saving?: boolean;
}

export function VoucherTotalBar({
  debitTotal,
  creditTotal,
  difference,
  isBalanced,
  onSaveDraft,
  onOpenDraftsModal,
  onNewDraft,
  saving = false,
}: VoucherTotalBarProps) {
  const diffVal = parseFloat(difference) || 0;

  return (
    <div className="ot-voucher-total-bar">
      <div className="ot-total-column">
        <span className="ot-total-label">Total Debit</span>
        <span className="ot-total-value ot-dr-val">₹ {debitTotal}</span>
      </div>

      <div className="ot-total-column">
        <span className="ot-total-label">Total Credit</span>
        <span className="ot-total-value ot-cr-val">₹ {creditTotal}</span>
      </div>

      <div className="ot-total-column">
        <span className="ot-total-label">Difference</span>
        <span className={`ot-total-value ${diffVal > 0 ? 'ot-diff-unbalanced' : 'ot-diff-zero'}`}>
          ₹ {difference}
        </span>
      </div>

      <div className="ot-status-indicator-box">
        {isBalanced ? (
          <span className="ot-badge ot-badge-success" title="Debit and Credit totals match exactly">
            ✓ DRAFT (Balanced)
          </span>
        ) : (
          <span
            className="ot-badge ot-badge-warning"
            title="Temporary imbalance permitted in draft vouchers. Balances must match before posting."
          >
            ⚠ DRAFT (Unbalanced)
          </span>
        )}
      </div>

      <div className="ot-total-bar-actions">
        <button
          type="button"
          className="ot-btn ot-btn-secondary"
          onClick={onNewDraft}
          title="Reset to a fresh draft voucher"
        >
          New Draft
        </button>
        <button
          type="button"
          className="ot-btn ot-btn-secondary"
          onClick={onOpenDraftsModal}
          title="View and reopen saved drafts"
        >
          View Drafts
        </button>
        <button
          type="button"
          className="ot-btn ot-btn-primary"
          onClick={onSaveDraft}
          disabled={saving}
          title="Save draft voucher to database (Ctrl+S)"
        >
          {saving ? 'Saving...' : 'Save Draft (Ctrl+S)'}
        </button>
      </div>
    </div>
  );
}

