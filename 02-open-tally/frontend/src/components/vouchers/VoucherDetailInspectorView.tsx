import React, { useState, useEffect, useCallback } from 'react';
import {
  Company,
  VoucherFullDetail,
  VoucherRegisterItem,
  VoucherEinvoiceMetadataDto,
  TransportDetailsInput,
  VoucherEinvoiceMetadataInput,
} from '../../types';
import {
  getVoucherFullDetail,
  cancelVoucher,
  reverseVoucher,
  exportInvoicePdf,
  getVoucherEinvoiceMetadata,
  saveVoucherEinvoiceMetadata,
  exportEinvoiceJson,
  exportEwayBillJson,
} from '../../services/api';
import './VoucherDetailInspectorView.css';

interface VoucherDetailInspectorViewProps {
  activeCompany: Company | null;
  voucherId: string;
  onBack: () => void;
  onNavigateToVoucher?: (voucherId: string) => void;
  onNavigateToLedger?: (ledgerId: string) => void;
}

export const VoucherDetailInspectorView: React.FC<VoucherDetailInspectorViewProps> = ({
  activeCompany,
  voucherId,
  onBack,
  onNavigateToVoucher,
  onNavigateToLedger,
}) => {
  const [detail, setDetail] = useState<VoucherFullDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);

  // Cancellation Modal state
  const [isCancelModalOpen, setIsCancelModalOpen] = useState(false);
  const [cancelReason, setCancelReason] = useState('');
  const [cancelling, setCancelling] = useState(false);

  // Reversal Modal state
  const [isReverseModalOpen, setIsReverseModalOpen] = useState(false);
  const [reversalDate, setReversalDate] = useState(
    new Date().toISOString().substring(0, 10)
  );
  const [reverseReason, setReverseReason] = useState('');
  const [reversing, setReversing] = useState(false);
  const [exportingPdf, setExportingPdf] = useState(false);

  // e-Invoice & e-Way Bill state
  const [einvoiceMeta, setEinvoiceMeta] = useState<VoucherEinvoiceMetadataDto | null>(null);
  const [isEwayModalOpen, setIsEwayModalOpen] = useState(false);
  const [isEditMetaOpen, setIsEditMetaOpen] = useState(false);
  const [exportingEinvoice, setExportingEinvoice] = useState(false);
  const [exportingEway, setExportingEway] = useState(false);
  const [savingMeta, setSavingMeta] = useState(false);

  // e-Way transport form
  const [transMode, setTransMode] = useState('Road');
  const [distanceKm, setDistanceKm] = useState(100);
  const [vehicleNo, setVehicleNo] = useState('KA01AB1234');
  const [transporterId, setTransporterId] = useState('');
  const [transporterName, setTransporterName] = useState('');
  const [transDocNo, setTransDocNo] = useState('');
  const [transDocDate, setTransDocDate] = useState('');

  // Edit metadata form
  const [metaIrn, setMetaIrn] = useState('');
  const [metaAckNo, setMetaAckNo] = useState('');
  const [metaAckDate, setMetaAckDate] = useState('');
  const [metaEwbNo, setMetaEwbNo] = useState('');
  const [metaEwbValid, setMetaEwbValid] = useState('');
  const [metaStatus, setMetaStatus] = useState<'Pending' | 'Generated' | 'Cancelled'>('Pending');

  const handleExportPdf = useCallback(async () => {
    if (!detail?.voucher) return;
    setExportingPdf(true);
    setError(null);
    try {
      const cleanNum = detail.voucher.voucher_number.replace(/[\/\\]/g, '_');
      const defaultPath = `invoices/${cleanNum}.pdf`;
      const savedPath = await exportInvoicePdf(detail.voucher.id, defaultPath);
      setNotification(`Tax Invoice PDF successfully exported to: ${savedPath}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setExportingPdf(false);
    }
  }, [detail]);

  // Ctrl+P shortcut for PDF export
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === 'p' || e.key === 'P')) {
        e.preventDefault();
        void handleExportPdf();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleExportPdf]);

  // Intercept Escape key to dismiss child dialogs without popping navigation stack
  useEffect(() => {
    const handleModalKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isCancelModalOpen) {
          e.stopPropagation();
          setIsCancelModalOpen(false);
        } else if (isReverseModalOpen) {
          e.stopPropagation();
          setIsReverseModalOpen(false);
        }
      }
    };

    if (isCancelModalOpen || isReverseModalOpen || isEwayModalOpen) {
      window.addEventListener('keydown', handleModalKeyDown, true);
      return () => window.removeEventListener('keydown', handleModalKeyDown, true);
    }
  }, [isCancelModalOpen, isReverseModalOpen, isEwayModalOpen]);

  const loadDetail = useCallback(async () => {
    if (!activeCompany || !voucherId) {
      setLoading(false);
      setError(!activeCompany ? 'No active company selected' : 'No voucher ID provided');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [data, meta] = await Promise.all([
        getVoucherFullDetail(voucherId),
        getVoucherEinvoiceMetadata(voucherId).catch(() => null),
      ]);
      setDetail(data);
      setEinvoiceMeta(meta);
      if (meta) {
        setMetaIrn(meta.irn || '');
        setMetaAckNo(meta.ack_no || '');
        setMetaAckDate(meta.ack_date || '');
        setMetaEwbNo(meta.eway_bill_no || '');
        setMetaEwbValid(meta.eway_valid_upto || '');
        setMetaStatus(meta.einvoice_status);
        if (meta.transport_mode) setTransMode(meta.transport_mode);
        if (meta.vehicle_no) setVehicleNo(meta.vehicle_no);
        if (meta.distance_km) setDistanceKm(meta.distance_km);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, voucherId]);

  useEffect(() => {
    void loadDetail();
  }, [loadDetail]);

  const handleExportEinvoice = async () => {
    if (!detail?.voucher) return;
    setExportingEinvoice(true);
    setError(null);
    try {
      const jsonStr = await exportEinvoiceJson(detail.voucher.id);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const cleanNum = detail.voucher.voucher_number.replace(/[\/\\]/g, '_');
      a.href = url;
      a.download = `EINVOICE_${cleanNum}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setNotification(`NIC e-Invoice v1.1 JSON exported successfully for #${detail.voucher.voucher_number}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setExportingEinvoice(false);
    }
  };

  const handleExportEwayBill = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!detail?.voucher) return;
    setExportingEway(true);
    setError(null);
    try {
      const transportInput: TransportDetailsInput = {
        transport_mode: transMode,
        distance_km: distanceKm,
        transporter_id: transporterId.trim() || null,
        transporter_name: transporterName.trim() || null,
        trans_doc_no: transDocNo.trim() || null,
        trans_doc_date: transDocDate.trim() || null,
        vehicle_no: vehicleNo.trim() || null,
      };
      const jsonStr = await exportEwayBillJson(detail.voucher.id, transportInput);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const cleanNum = detail.voucher.voucher_number.replace(/[\/\\]/g, '_');
      a.href = url;
      a.download = `EWAYBILL_${cleanNum}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setIsEwayModalOpen(false);
      setNotification(`NIC e-Way Bill JSON exported successfully for #${detail.voucher.voucher_number}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setExportingEway(false);
    }
  };

  const handleSaveMetadata = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!detail?.voucher) return;
    setSavingMeta(true);
    setError(null);
    try {
      const metaInput: VoucherEinvoiceMetadataInput = {
        irn: metaIrn.trim() || null,
        ack_no: metaAckNo.trim() || null,
        ack_date: metaAckDate.trim() || null,
        einvoice_status: metaStatus,
        eway_bill_no: metaEwbNo.trim() || null,
        eway_valid_upto: metaEwbValid.trim() || null,
        transport_mode: transMode || null,
        vehicle_no: vehicleNo.trim() || null,
        distance_km: distanceKm || null,
      };
      const saved = await saveVoucherEinvoiceMetadata(detail.voucher.id, metaInput);
      setEinvoiceMeta(saved);
      setIsEditMetaOpen(false);
      setNotification('e-Invoice & e-Way Bill metadata saved successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingMeta(false);
    }
  };

  const handleCancelSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!cancelReason.trim()) {
      alert('Please enter a cancellation reason.');
      return;
    }
    setCancelling(true);
    setError(null);
    try {
      await cancelVoucher(voucherId, cancelReason.trim());
      setIsCancelModalOpen(false);
      setCancelReason('');
      setNotification('Voucher cancelled successfully.');
      await loadDetail();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCancelling(false);
    }
  };

  const handleReverseSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!reverseReason.trim()) {
      alert('Please enter a reversal reason.');
      return;
    }
    setReversing(true);
    setError(null);
    try {
      const reversed = await reverseVoucher(
        voucherId,
        reversalDate,
        reverseReason.trim()
      );
      setIsReverseModalOpen(false);
      setReverseReason('');
      setNotification(`Voucher reversed successfully. Reversal voucher: ${reversed.voucher.voucher_number}`);
      await loadDetail();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setReversing(false);
    }
  };

  if (loading) {
    return (
      <section className="ot-inspector-view" data-testid="voucher-detail-inspector">
        <div className="ot-inspector-loading">Loading voucher details...</div>
      </section>
    );
  }

  if (error || !detail) {
    return (
      <section className="ot-inspector-view" data-testid="voucher-detail-inspector">
        <div className="ot-inspector-header">
          <h2>Voucher Detail Inspector</h2>
          <button className="ot-btn" onClick={onBack}>
            Back (Esc)
          </button>
        </div>
        <div className="ot-alert ot-alert-danger" style={{ margin: '16px' }}>
          {error || 'Voucher not found'}
        </div>
      </section>
    );
  }

  const { voucher, lines, gst_details, bill_allocations, stock_movements, lineage } = detail;

  // Debit and Credit line totals
  let drTotal = 0;
  let crTotal = 0;
  for (const l of lines) {
    const val = parseFloat(l.amount) || 0;
    if (l.entry_type === 'Dr') drTotal += val;
    else if (l.entry_type === 'Cr') crTotal += val;
  }

  return (
    <section className="ot-inspector-view" data-testid="voucher-detail-inspector">
      {/* Top Header Bar */}
      <header className="ot-inspector-header">
        <div>
          <div className="ot-inspector-title-row">
            <h2>{voucher.voucher_type_name}: {voucher.voucher_number}</h2>
            <span className={`ot-status-badge ot-status-${voucher.status.toLowerCase()}`}>
              {voucher.status.toUpperCase()}
            </span>
          </div>
          <p className="ot-inspector-subtitle">
            Company: {activeCompany?.name} | Date: {voucher.date}
            {voucher.effective_date && voucher.effective_date !== voucher.date && ` (Effective: ${voucher.effective_date})`}
          </p>
        </div>

        <div className="ot-inspector-actions">
          {voucher.status === 'posted' && (
            <>
              <button
                type="button"
                className="ot-btn ot-btn-primary"
                onClick={handleExportPdf}
                disabled={exportingPdf}
                title="Export Indian GST Tax Invoice / Note PDF (Ctrl+P)"
              >
                {exportingPdf ? 'Exporting PDF...' : 'Export Tax Invoice PDF (Ctrl+P)'}
              </button>
              <button
                type="button"
                className="ot-btn ot-btn-danger"
                onClick={() => setIsCancelModalOpen(true)}
              >
                Cancel Voucher
              </button>
              <button
                type="button"
                className="ot-btn ot-btn-warning"
                onClick={() => setIsReverseModalOpen(true)}
              >
                Reverse Voucher
              </button>
            </>
          )}
          <button type="button" className="ot-btn" onClick={onBack}>
            Back (Esc)
          </button>
        </div>
      </header>

      {notification && (
        <div className="ot-notification-banner" style={{ margin: '0 24px 12px 24px' }}>
          <span className="ot-notification-icon">&#9432;</span>
          <span className="ot-notification-text">{notification}</span>
          <button className="ot-notification-close" onClick={() => setNotification(null)}>
            &times;
          </button>
        </div>
      )}

      <div className="ot-inspector-content">
        {/* Voucher Meta Card */}
        <div className="ot-inspector-card ot-meta-card">
          <div className="ot-meta-grid">
            <div className="ot-meta-field">
              <span className="ot-meta-label">Voucher Type</span>
              <span className="ot-meta-val">{voucher.voucher_type_name}</span>
            </div>
            <div className="ot-meta-field">
              <span className="ot-meta-label">Series</span>
              <span className="ot-meta-val">{voucher.series_name}</span>
            </div>
            <div className="ot-meta-field">
              <span className="ot-meta-label">Voucher Date</span>
              <span className="ot-meta-val">{voucher.date}</span>
            </div>
            <div className="ot-meta-field">
              <span className="ot-meta-label">Reference No</span>
              <span className="ot-meta-val">{voucher.reference_no || '—'}</span>
            </div>
            <div className="ot-meta-field">
              <span className="ot-meta-label">Total Amount</span>
              <span className="ot-meta-val ot-money-highlight">
                ₹ {voucher.total_amount || drTotal.toFixed(2)}
              </span>
            </div>
            {voucher.narration && (
              <div className="ot-meta-field ot-meta-field-full">
                <span className="ot-meta-label">Narration</span>
                <span className="ot-meta-val ot-narration-val">{voucher.narration}</span>
              </div>
            )}
            {voucher.cancellation_reason && (
              <div className="ot-meta-field ot-meta-field-full ot-meta-cancellation">
                <span className="ot-meta-label">Cancellation Reason</span>
                <span className="ot-meta-val ot-danger-text">{voucher.cancellation_reason}</span>
              </div>
            )}
            {voucher.reversal_of_voucher_id && (
              <div className="ot-meta-field ot-meta-field-full">
                <span className="ot-meta-label">Reversal Of Voucher</span>
                <span
                  className="ot-meta-val ot-link-val"
                  onClick={() => onNavigateToVoucher && onNavigateToVoucher(voucher.reversal_of_voucher_id!)}
                >
                  {voucher.reversal_of_voucher_id} &#8594;
                </span>
              </div>
            )}
            {voucher.reversal_voucher_id && (
              <div className="ot-meta-field ot-meta-field-full">
                <span className="ot-meta-label">Reversed By Voucher</span>
                <span
                  className="ot-meta-val ot-link-val"
                  onClick={() => onNavigateToVoucher && onNavigateToVoucher(voucher.reversal_voucher_id!)}
                >
                  {voucher.reversal_voucher_id} &#8594;
                </span>
              </div>
            )}
          </div>
        </div>

        {/* 1. General Ledger Lines */}
        <div className="ot-inspector-card">
          <div className="ot-card-header">
            <h3>Double-Entry General Ledger Postings</h3>
            <span className="ot-badge">{lines.length} Line(s)</span>
          </div>

          <table className="ot-inspector-table" data-testid="gl-lines-table">
            <thead>
              <tr>
                <th style={{ width: '40px' }}>#</th>
                <th>Particulars (Ledger)</th>
                <th>Account Group</th>
                <th>Classification</th>
                <th style={{ width: '60px' }}>Type</th>
                <th style={{ textAlign: 'right' }}>Debit (₹)</th>
                <th style={{ textAlign: 'right' }}>Credit (₹)</th>
              </tr>
            </thead>
            <tbody>
              {lines.map((l) => (
                <tr key={l.id}>
                  <td>{l.line_number}</td>
                  <td>
                    <button
                      type="button"
                      className="ot-inline-link-btn"
                      onClick={() => onNavigateToLedger && onNavigateToLedger(l.ledger_id)}
                      title="View Ledger Monthly Summary"
                    >
                      {l.ledger_name}
                    </button>
                    {l.narration && <div className="ot-line-narration">{l.narration}</div>}
                  </td>
                  <td>{l.group_name}</td>
                  <td>{l.classification}</td>
                  <td>
                    <span className={`ot-entry-badge ot-entry-${l.entry_type.toLowerCase()}`}>
                      {l.entry_type}
                    </span>
                  </td>
                  <td style={{ textAlign: 'right', fontWeight: l.entry_type === 'Dr' ? 600 : 'normal' }}>
                    {l.entry_type === 'Dr' ? l.amount : '—'}
                  </td>
                  <td style={{ textAlign: 'right', fontWeight: l.entry_type === 'Cr' ? 600 : 'normal' }}>
                    {l.entry_type === 'Cr' ? l.amount : '—'}
                  </td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr className="ot-table-total-row">
                <td colSpan={5} style={{ textAlign: 'right', fontWeight: 700 }}>
                  Total:
                </td>
                <td style={{ textAlign: 'right', fontWeight: 700 }}>₹ {drTotal.toFixed(2)}</td>
                <td style={{ textAlign: 'right', fontWeight: 700 }}>₹ {crTotal.toFixed(2)}</td>
              </tr>
            </tfoot>
          </table>
        </div>

        {/* 2. Indian Statutory GST Breakdown */}
        {gst_details.length > 0 && (
          <div className="ot-inspector-card" data-testid="gst-details-section">
            <div className="ot-card-header">
              <h3>Indian Statutory GST Breakdown</h3>
              <span className="ot-badge">{gst_details.length} GST Line(s)</span>
            </div>

            <table className="ot-inspector-table">
              <thead>
                <tr>
                  <th>Party</th>
                  <th>GSTIN</th>
                  <th>HSN/SAC</th>
                  <th>POS</th>
                  <th>Supply Type</th>
                  <th style={{ textAlign: 'right' }}>Taxable Val</th>
                  <th style={{ textAlign: 'right' }}>Rate</th>
                  <th style={{ textAlign: 'right' }}>CGST</th>
                  <th style={{ textAlign: 'right' }}>SGST</th>
                  <th style={{ textAlign: 'right' }}>IGST</th>
                  <th style={{ textAlign: 'right' }}>Cess</th>
                </tr>
              </thead>
              <tbody>
                {gst_details.map((g) => (
                  <tr key={g.id}>
                    <td>{g.party_name}</td>
                    <td><code>{g.party_gstin || 'Unregistered'}</code></td>
                    <td>{g.hsn_sac_code}</td>
                    <td>State {g.place_of_supply_state_code}</td>
                    <td>{g.supply_type}</td>
                    <td style={{ textAlign: 'right' }}>₹ {g.taxable_value}</td>
                    <td style={{ textAlign: 'right' }}>{g.gst_rate}%</td>
                    <td style={{ textAlign: 'right' }}>₹ {g.cgst_amount}</td>
                    <td style={{ textAlign: 'right' }}>₹ {g.sgst_amount}</td>
                    <td style={{ textAlign: 'right' }}>₹ {g.igst_amount}</td>
                    <td style={{ textAlign: 'right' }}>₹ {g.cess_amount}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* 2b. Indian Statutory e-Invoice & e-Way Bill Compliance (NIC v1.1) */}
        {(voucher.voucher_type_id === 'vt_sales' ||
          voucher.voucher_type_id === 'vt_credit_note' ||
          voucher.voucher_type_id === 'vt_debit_note' ||
          gst_details.length > 0) && (
          <div className="ot-inspector-card" data-testid="einvoice-section">
            <div className="ot-card-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '0.5rem' }}>
              <div>
                <h3 style={{ margin: 0 }}>e-Invoice & e-Way Bill (NIC Compliance)</h3>
                <span className="ot-report-muted" style={{ fontSize: '0.8rem' }}>
                  Standard JSON payloads for GST e-Invoice v1.1 and e-Way Bill integration
                </span>
              </div>
              <div style={{ display: 'flex', gap: '0.5rem', alignItems: 'center', flexWrap: 'wrap' }}>
                <span
                  style={{
                    display: 'inline-block',
                    padding: '0.2rem 0.6rem',
                    borderRadius: '4px',
                    fontSize: '0.8rem',
                    fontWeight: 600,
                    color: '#fff',
                    backgroundColor:
                      einvoiceMeta?.einvoice_status === 'Generated'
                        ? '#097969'
                        : einvoiceMeta?.einvoice_status === 'Cancelled'
                        ? '#dc2626'
                        : '#6b7280',
                  }}
                >
                  Status: {einvoiceMeta?.einvoice_status || 'Pending'}
                </span>
                <button
                  type="button"
                  className="ot-primary-btn"
                  onClick={handleExportEinvoice}
                  disabled={exportingEinvoice}
                  style={{ fontSize: '0.8rem', padding: '0.35rem 0.75rem' }}
                >
                  {exportingEinvoice ? 'Generating…' : 'Generate e-Invoice JSON'}
                </button>
                <button
                  type="button"
                  className="ot-btn"
                  onClick={() => setIsEwayModalOpen(true)}
                  style={{ fontSize: '0.8rem', padding: '0.35rem 0.75rem' }}
                >
                  Generate e-Way Bill JSON
                </button>
                <button
                  type="button"
                  className="ot-btn"
                  onClick={() => setIsEditMetaOpen(!isEditMetaOpen)}
                  style={{ fontSize: '0.8rem', padding: '0.35rem 0.75rem' }}
                >
                  {isEditMetaOpen ? 'Hide Edit' : 'Edit IRN / e-Way Details'}
                </button>
              </div>
            </div>

            {/* Display Current Metadata */}
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
                gap: '1rem',
                marginTop: '1rem',
                padding: '1rem',
                backgroundColor: 'rgba(0,0,0,0.02)',
                borderRadius: '6px',
                fontSize: '0.85rem',
              }}
            >
              <div>
                <strong>IRN (Invoice Ref Number):</strong>
                <div style={{ wordBreak: 'break-all', fontFamily: 'monospace', color: einvoiceMeta?.irn ? '#097969' : '#888' }}>
                  {einvoiceMeta?.irn || 'Not Recorded'}
                </div>
              </div>
              <div>
                <strong>Ack Number & Date:</strong>
                <div>
                  {einvoiceMeta?.ack_no ? `${einvoiceMeta.ack_no} (${einvoiceMeta.ack_date || '—'})` : '—'}
                </div>
              </div>
              <div>
                <strong>e-Way Bill Number:</strong>
                <div style={{ fontWeight: 600, color: einvoiceMeta?.eway_bill_no ? '#097969' : '#888' }}>
                  {einvoiceMeta?.eway_bill_no || 'Not Generated'}
                </div>
              </div>
              <div>
                <strong>e-Way Bill Valid Upto:</strong>
                <div>{einvoiceMeta?.eway_valid_upto || '—'}</div>
              </div>
            </div>

            {/* Edit Metadata Inline Form */}
            {isEditMetaOpen && (
              <form onSubmit={handleSaveMetadata} style={{ marginTop: '1rem', padding: '1rem', border: '1px solid #ddd', borderRadius: '6px' }}>
                <h4 style={{ margin: '0 0 1rem 0' }}>Record IRN / e-Way Bill Acknowledgement</h4>
                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '1rem' }}>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem', gridColumn: 'span 2' }}>
                    <span>IRN (64-character hexadecimal hash):</span>
                    <input
                      type="text"
                      maxLength={64}
                      value={metaIrn}
                      onChange={(e) => setMetaIrn(e.target.value)}
                      placeholder="e.g. 1a2b3c4d5e6f... (64 hex characters)"
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Ack Number:</span>
                    <input
                      type="text"
                      value={metaAckNo}
                      onChange={(e) => setMetaAckNo(e.target.value)}
                      placeholder="e.g. 122010000000"
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Ack Date:</span>
                    <input
                      type="text"
                      value={metaAckDate}
                      onChange={(e) => setMetaAckDate(e.target.value)}
                      placeholder="YYYY-MM-DD or DD/MM/YYYY"
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>e-Way Bill Number (12 digits):</span>
                    <input
                      type="text"
                      maxLength={12}
                      value={metaEwbNo}
                      onChange={(e) => setMetaEwbNo(e.target.value)}
                      placeholder="e.g. 251000000000"
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Valid Upto:</span>
                    <input
                      type="text"
                      value={metaEwbValid}
                      onChange={(e) => setMetaEwbValid(e.target.value)}
                      placeholder="YYYY-MM-DD"
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Compliance Status:</span>
                    <select value={metaStatus} onChange={(e) => setMetaStatus(e.target.value as any)}>
                      <option value="Pending">Pending</option>
                      <option value="Generated">Generated</option>
                      <option value="Cancelled">Cancelled</option>
                    </select>
                  </label>
                </div>
                <div style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end', marginTop: '1rem' }}>
                  <button type="button" className="ot-btn" onClick={() => setIsEditMetaOpen(false)}>
                    Cancel
                  </button>
                  <button type="submit" className="ot-primary-btn" disabled={savingMeta}>
                    {savingMeta ? 'Saving…' : 'Save Details'}
                  </button>
                </div>
              </form>
            )}
          </div>
        )}

        {/* 3. Bill-wise Allocations */}
        {bill_allocations.length > 0 && (
          <div className="ot-inspector-card" data-testid="bill-allocations-section">
            <div className="ot-card-header">
              <h3>Bill-wise (AR/AP) Allocations</h3>
              <span className="ot-badge">{bill_allocations.length} Allocation(s)</span>
            </div>

            <table className="ot-inspector-table">
              <thead>
                <tr>
                  <th>Party Ledger</th>
                  <th>Type of Ref</th>
                  <th>Bill Name / Ref</th>
                  <th>Dr/Cr</th>
                  <th>Bill Date</th>
                  <th>Due Date</th>
                  <th style={{ textAlign: 'right' }}>Allocated Amount</th>
                </tr>
              </thead>
              <tbody>
                {bill_allocations.map((b) => (
                  <tr key={b.id}>
                    <td>{b.party_name}</td>
                    <td>
                      <span className="ot-ref-badge">{b.allocation_type}</span>
                    </td>
                    <td><strong>{b.bill_name}</strong></td>
                    <td>{b.dr_cr}</td>
                    <td>{b.bill_date}</td>
                    <td>{b.due_date || '—'}</td>
                    <td style={{ textAlign: 'right', fontWeight: 600 }}>₹ {b.amount}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* 4. Inventory Movements */}
        {stock_movements.length > 0 && (
          <div className="ot-inspector-card" data-testid="stock-movements-section">
            <div className="ot-card-header">
              <h3>FIFO Stock Movements & Inventory Attribution</h3>
              <span className="ot-badge">{stock_movements.length} Movement(s)</span>
            </div>

            <table className="ot-inspector-table">
              <thead>
                <tr>
                  <th>Stock Item</th>
                  <th>Warehouse / Godown</th>
                  <th>Movement Type</th>
                  <th style={{ textAlign: 'right' }}>Quantity</th>
                </tr>
              </thead>
              <tbody>
                {stock_movements.map((sm) => (
                  <tr key={sm.id}>
                    <td>{sm.stock_item_name}</td>
                    <td>{sm.godown_name || 'Main Location'}</td>
                    <td>
                      <span className="ot-movement-badge">{sm.movement_type}</span>
                    </td>
                    <td style={{ textAlign: 'right', fontWeight: 600 }}>{sm.quantity}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        {/* 5. Lineage Graph */}
        {lineage && lineage.items.length > 1 && (
          <div className="ot-inspector-card" data-testid="lineage-section">
            <div className="ot-card-header">
              <h3>Voucher Lineage & Relationship Graph</h3>
              <span className="ot-badge">{lineage.items.length} Linked Vouchers</span>
            </div>

            <div className="ot-lineage-list">
              {lineage.items.map((item: VoucherRegisterItem) => (
                <div
                  key={item.id}
                  className={`ot-lineage-row ${item.id === voucher.id ? 'ot-lineage-current' : ''}`}
                  onClick={() => item.id !== voucher.id && onNavigateToVoucher && onNavigateToVoucher(item.id)}
                >
                  <span className="ot-lineage-type">{item.voucher_type_name}</span>
                  <span className="ot-lineage-num">{item.voucher_number}</span>
                  <span className="ot-lineage-date">{item.date}</span>
                  <span className={`ot-status-badge ot-status-${item.status.toLowerCase()}`}>
                    {item.status}
                  </span>
                  <span className="ot-lineage-amt">₹ {item.total_amount || '—'}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Cancellation Modal */}
      {isCancelModalOpen && (
        <div className="ot-modal-backdrop" onClick={() => setIsCancelModalOpen(false)}>
          <div className="ot-modal-container" onClick={(e) => e.stopPropagation()}>
            <div className="ot-modal-header">
              <h3>Cancel Voucher #{voucher.voucher_number}</h3>
              <button className="ot-modal-close" onClick={() => setIsCancelModalOpen(false)}>
                &times;
              </button>
            </div>
            <form onSubmit={handleCancelSubmit} className="ot-modal-form">
              <p className="ot-modal-desc">
                Cancelling marks the voucher as cancelled, preserves MCA audit trail records, and unlinks settled bill balances.
              </p>
              <label className="ot-form-field">
                Reason for Cancellation:
                <textarea
                  required
                  rows={3}
                  value={cancelReason}
                  onChange={(e) => setCancelReason(e.target.value)}
                  placeholder="e.g. Invoiced in error, duplicate entry, client request..."
                />
              </label>
              <div className="ot-modal-actions">
                <button
                  type="button"
                  className="ot-btn"
                  onClick={() => setIsCancelModalOpen(false)}
                >
                  Abort
                </button>
                <button
                  type="submit"
                  className="ot-btn ot-btn-danger"
                  disabled={cancelling}
                >
                  {cancelling ? 'Cancelling...' : 'Confirm Cancellation'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Reversal Modal */}
      {isReverseModalOpen && (
        <div className="ot-modal-backdrop" onClick={() => setIsReverseModalOpen(false)}>
          <div className="ot-modal-container" onClick={(e) => e.stopPropagation()}>
            <div className="ot-modal-header">
              <h3>Reverse Voucher #{voucher.voucher_number}</h3>
              <button className="ot-modal-close" onClick={() => setIsReverseModalOpen(false)}>
                &times;
              </button>
            </div>
            <form onSubmit={handleReverseSubmit} className="ot-modal-form">
              <p className="ot-modal-desc">
                Creates a symmetrical opposing double-entry voucher restoring debit/credit balances and bill positions.
              </p>
              <label className="ot-form-field">
                Reversal Transaction Date:
                <input
                  type="date"
                  required
                  value={reversalDate}
                  onChange={(e) => setReversalDate(e.target.value)}
                />
              </label>
              <label className="ot-form-field">
                Reason for Reversal:
                <textarea
                  required
                  rows={3}
                  value={reverseReason}
                  onChange={(e) => setReverseReason(e.target.value)}
                  placeholder="e.g. Cheque bounced, goods returned, mutual agreement..."
                />
              </label>
              <div className="ot-modal-actions">
                <button
                  type="button"
                  className="ot-btn"
                  onClick={() => setIsReverseModalOpen(false)}
                >
                  Abort
                </button>
                <button
                  type="submit"
                  className="ot-btn ot-btn-warning"
                  disabled={reversing}
                >
                  {reversing ? 'Reversing...' : 'Post Reversal'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* e-Way Bill Consignment Details Modal */}
      {isEwayModalOpen && (
        <div className="ot-modal-backdrop" onClick={() => setIsEwayModalOpen(false)}>
          <div className="ot-modal-container" onClick={(e) => e.stopPropagation()}>
            <div className="ot-modal-header">
              <h3>Generate NIC e-Way Bill JSON (#{voucher.voucher_number})</h3>
              <button className="ot-modal-close" onClick={() => setIsEwayModalOpen(false)}>
                &times;
              </button>
            </div>
            <form onSubmit={handleExportEwayBill} className="ot-modal-form">
              <p className="ot-modal-desc">
                Provide consignment transport details to generate standard NIC e-Way Bill schema JSON.
              </p>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.75rem' }}>
                <label className="ot-form-field">
                  Transport Mode:
                  <select value={transMode} onChange={(e) => setTransMode(e.target.value)}>
                    <option value="Road">Road (1)</option>
                    <option value="Rail">Rail (2)</option>
                    <option value="Air">Air (3)</option>
                    <option value="Ship">Ship (4)</option>
                  </select>
                </label>
                <label className="ot-form-field">
                  Distance (KM):
                  <input
                    type="number"
                    min={1}
                    required
                    value={distanceKm}
                    onChange={(e) => setDistanceKm(Number(e.target.value))}
                  />
                </label>
                {transMode === 'Road' && (
                  <label className="ot-form-field" style={{ gridColumn: 'span 2' }}>
                    Vehicle Number (Required for Road):
                    <input
                      type="text"
                      required
                      placeholder="e.g. KA01AB1234"
                      value={vehicleNo}
                      onChange={(e) => setVehicleNo(e.target.value)}
                    />
                  </label>
                )}
                <label className="ot-form-field">
                  Transporter ID (15-char GSTIN/TRANSIN):
                  <input
                    type="text"
                    maxLength={15}
                    placeholder="e.g. 29ABCDE1234F1Z5"
                    value={transporterId}
                    onChange={(e) => setTransporterId(e.target.value)}
                  />
                </label>
                <label className="ot-form-field">
                  Transporter Name:
                  <input
                    type="text"
                    placeholder="e.g. VRL Logistics"
                    value={transporterName}
                    onChange={(e) => setTransporterName(e.target.value)}
                  />
                </label>
                <label className="ot-form-field">
                  Transport Doc / LR No:
                  <input
                    type="text"
                    placeholder="e.g. LR/2026/889"
                    value={transDocNo}
                    onChange={(e) => setTransDocNo(e.target.value)}
                  />
                </label>
                <label className="ot-form-field">
                  Transport Doc Date:
                  <input
                    type="date"
                    value={transDocDate}
                    onChange={(e) => setTransDocDate(e.target.value)}
                  />
                </label>
              </div>
              <div className="ot-modal-actions" style={{ marginTop: '1rem' }}>
                <button
                  type="button"
                  className="ot-btn"
                  onClick={() => setIsEwayModalOpen(false)}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="ot-primary-btn"
                  disabled={exportingEway}
                >
                  {exportingEway ? 'Generating…' : 'Download e-Way Bill JSON'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </section>
  );
};
