import { VoucherType } from '../../types';

interface Props {
  typeId: string;
  types: VoucherType[];
  onBack: () => void;
  onMasters?: () => void;
  onDrafts: () => void;
  notification: string | null;
  onClearNotification: () => void;
  error: string | null;
}

const ribbonTypes: Record<string, { cls: string; label: string; sub: string }> = {
  vt_contra: { cls: 'ot-ribbon-contra', label: 'Contra Voucher (F4)', sub: 'Cash / Bank Fund Transfer' },
  vt_payment: { cls: 'ot-ribbon-payment', label: 'Payment Voucher (F5)', sub: 'Cash / Bank Disbursement' },
  vt_receipt: { cls: 'ot-ribbon-receipt', label: 'Receipt Voucher (F6)', sub: 'Cash / Bank Collection' },
  vt_jnl: { cls: 'ot-ribbon-journal', label: 'Journal Voucher (F7)', sub: 'Non-Cash Adjustment & Accrual' },
  vt_sales: { cls: 'ot-ribbon-sales', label: 'Sales Voucher (F8)', sub: 'Outward Commercial Entry' },
  vt_pur: { cls: 'ot-ribbon-purchase', label: 'Purchase Voucher (F9)', sub: 'Inward Commercial Entry' },
};

export function VoucherEntryToolbar({ typeId, types, onBack, onMasters, onDrafts, notification, onClearNotification, error }: Props) {
  const meta = ribbonTypes[typeId] || { cls: 'ot-ribbon-journal', label: types.find((type) => type.id === typeId)?.name || 'Accounting Voucher', sub: 'Double-Entry Voucher Posting' };
  return <>
    <div className={`ot-voucher-ribbon ${meta.cls}`} data-testid="voucher-entry-ribbon"><span>{meta.label}</span><span>{meta.sub}</span></div>
    <div className="ot-voucher-top-bar">
      <div className="ot-breadcrumb"><span className="ot-crumb-link" onClick={onBack}>Gateway</span><span className="ot-crumb-sep">&gt;</span><span className="ot-crumb-active">Accounting Vouchers</span></div>
      <div className="ot-top-actions">
        {onMasters && <button type="button" className="ot-btn ot-btn-secondary" onClick={onMasters} title="Configure Voucher Types & Numbering Series">Voucher Masters Config</button>}
        <button type="button" className="ot-btn ot-btn-secondary" onClick={onDrafts} title="Open saved drafts modal">Saved Drafts</button>
        <button type="button" className="ot-btn" onClick={onBack} title="Return to Gateway (Esc)">Gateway (Esc)</button>
      </div>
    </div>
    {notification && <div className="ot-notification-banner" role="status"><span className="ot-notification-text">{notification}</span><button className="ot-notification-close" onClick={onClearNotification}>&times;</button></div>}
    {error && <div className="ot-alert ot-alert-danger" role="alert" style={{ margin: '12px 24px' }}>{error}</div>}
  </>;
}
