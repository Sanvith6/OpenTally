import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import {
  Company,
  VoucherType,
  VoucherNumberSeries,
  Ledger,
  VoucherLineInput,
  SaveVoucherDraftRequest,
} from '../../types';
import {
  listVoucherTypes,
  listVoucherSeries,
  listLedgers,
  saveVoucherDraft,
  getVoucherDraft,
} from '../../services/api';
import { VoucherHeaderForm } from './VoucherHeaderForm';
import { VoucherLineGrid } from './VoucherLineGrid';
import { VoucherTotalBar } from './VoucherTotalBar';
import { DraftsModal } from './DraftsModal';
import './VoucherEntryView.css';

interface VoucherEntryViewProps {
  activeCompany: Company | null;
  onBackToGateway: () => void;
  onSwitchToMasters?: () => void;
}

export function VoucherEntryView({
  activeCompany,
  onBackToGateway,
  onSwitchToMasters,
}: VoucherEntryViewProps) {
  const [voucherTypes, setVoucherTypes] = useState<VoucherType[]>([]);
  const [selectedTypeId, setSelectedTypeId] = useState<string>('');
  const [seriesList, setSeriesList] = useState<VoucherNumberSeries[]>([]);
  const [selectedSeriesId, setSelectedSeriesId] = useState<string>('');
  const [ledgers, setLedgers] = useState<Ledger[]>([]);

  // Draft Header State
  const [activeDraftId, setActiveDraftId] = useState<string | null>(null);
  const [provisionalNumber, setProvisionalNumber] = useState<string>('DRAFT-NEW');
  const [date, setDate] = useState<string>(
    activeCompany?.books_beginning_from || new Date().toISOString().substring(0, 10)
  );
  const [effectiveDate, setEffectiveDate] = useState<string>(
    activeCompany?.books_beginning_from || new Date().toISOString().substring(0, 10)
  );
  const [referenceNo, setReferenceNo] = useState<string>('');
  const [referenceDate, setReferenceDate] = useState<string>('');
  const [narration, setNarration] = useState<string>('');

  // Line items state
  const [lines, setLines] = useState<VoucherLineInput[]>([]);

  // Modals & Async state
  const [isDraftsModalOpen, setIsDraftsModalOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);

  const dateInputRef = useRef<HTMLInputElement>(null);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => {
      setNotification((curr) => (curr === msg ? null : curr));
    }, 4000);
  };

  // 1. Initial Load: Voucher Types & Ledgers
  useEffect(() => {
    async function initWorkspace() {
      if (!activeCompany) return;
      setLoading(true);
      setError(null);
      try {
        const [types, ledgersList] = await Promise.all([
          listVoucherTypes(),
          listLedgers(),
        ]);
        setVoucherTypes(types);
        setLedgers(ledgersList);

        // Default to Sales or first available
        const defaultType = types.find((t) => t.id === 'vt_sales') || types[0];
        if (defaultType) {
          setSelectedTypeId(defaultType.id);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    }
    initWorkspace();
  }, [activeCompany]);

  // 2. Load Series when selectedTypeId changes
  useEffect(() => {
    async function loadSeries() {
      if (!selectedTypeId || !activeCompany) return;
      try {
        const series = await listVoucherSeries(selectedTypeId);
        setSeriesList(series);
        const activeSeries = series.filter((s) => s.is_active);
        if (activeSeries.length > 0) {
          setSelectedSeriesId(activeSeries[0].id);
        } else {
          setSelectedSeriesId('');
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
    loadSeries();
  }, [selectedTypeId, activeCompany]);

  // Compute Running Dr / Cr totals and difference
  const { debitTotal, creditTotal, difference, isBalanced } = useMemo(() => {
    let dr = 0;
    let cr = 0;
    for (const l of lines) {
      const val = parseFloat(l.amount) || 0;
      if (l.entry_type === 'Dr') dr += val;
      else if (l.entry_type === 'Cr') cr += val;
    }
    const diff = Math.abs(dr - cr);
    const balanced = dr === cr && lines.length > 0 && dr > 0;
    return {
      debitTotal: dr.toFixed(2),
      creditTotal: cr.toFixed(2),
      difference: diff.toFixed(2),
      isBalanced: balanced,
    };
  }, [lines]);

  // Line item modifiers
  const handleAddLine = () => {
    setLines((prev) => [
      ...prev,
      {
        id: `draft_line_${Date.now()}_${prev.length}`,
        ledger_id: '',
        entry_type: prev.length % 2 === 0 ? 'Dr' : 'Cr',
        amount: '',
        narration: '',
      },
    ]);
  };

  const handleUpdateLine = (index: number, updated: Partial<VoucherLineInput>) => {
    setLines((prev) => {
      const next = [...prev];
      next[index] = { ...next[index], ...updated };
      return next;
    });
  };

  const handleDeleteLine = (index: number) => {
    setLines((prev) => prev.filter((_, idx) => idx !== index));
  };

  const handleMoveLineUp = (index: number) => {
    if (index <= 0) return;
    setLines((prev) => {
      const next = [...prev];
      const temp = next[index - 1];
      next[index - 1] = next[index];
      next[index] = temp;
      return next;
    });
  };

  const handleMoveLineDown = (index: number) => {
    setLines((prev) => {
      if (index >= prev.length - 1) return prev;
      const next = [...prev];
      const temp = next[index + 1];
      next[index + 1] = next[index];
      next[index] = temp;
      return next;
    });
  };

  // Reset to clean draft
  const handleNewDraft = () => {
    setActiveDraftId(null);
    setProvisionalNumber('DRAFT-NEW');
    setReferenceNo('');
    setReferenceDate('');
    setNarration('');
    setLines([]);
    showNotification('Workspace reset to fresh draft voucher.');
  };

  // Save Draft (Atomic persistence)
  const handleSaveDraft = async () => {
    if (!selectedTypeId || !selectedSeriesId) {
      alert('Please select both a Voucher Type and Numbering Series.');
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const payload: SaveVoucherDraftRequest = {
        voucher_id: activeDraftId,
        voucher_type_id: selectedTypeId,
        series_id: selectedSeriesId,
        date,
        effective_date: effectiveDate || null,
        reference_no: referenceNo || null,
        reference_date: referenceDate || null,
        narration: narration || null,
        lines: lines.filter((l) => l.ledger_id && parseFloat(l.amount) > 0),
      };

      const result = await saveVoucherDraft(payload);
      setActiveDraftId(result.voucher.id);
      setProvisionalNumber(result.voucher.voucher_number);
      showNotification(`Draft voucher saved successfully (${result.voucher.voucher_number}).`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  // Load an existing draft
  const handleSelectDraft = async (voucherId: string) => {
    setLoading(true);
    setError(null);
    try {
      const loaded = await getVoucherDraft(voucherId);
      setActiveDraftId(loaded.voucher.id);
      setProvisionalNumber(loaded.voucher.voucher_number);
      setSelectedTypeId(loaded.voucher.voucher_type_id);
      setSelectedSeriesId(loaded.voucher.series_id);
      setDate(loaded.voucher.date);
      setEffectiveDate(loaded.voucher.effective_date);
      setReferenceNo(loaded.voucher.reference_no || '');
      setReferenceDate(loaded.voucher.reference_date || '');
      setNarration(loaded.voucher.narration || '');
      setLines(
        loaded.lines.map((l) => ({
          id: l.id,
          ledger_id: l.ledger_id,
          entry_type: l.entry_type,
          amount: l.amount,
          narration: l.narration || '',
        }))
      );
      showNotification(`Draft voucher ${loaded.voucher.voucher_number} loaded.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  // Keyboard navigation shortcuts
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Don't intercept if drafts modal is open
      if (isDraftsModalOpen) {
        if (e.key === 'Escape') {
          setIsDraftsModalOpen(false);
        }
        return;
      }

      if (e.key === 'F2') {
        e.preventDefault();
        dateInputRef.current?.focus();
      } else if (e.key === 'F4') {
        e.preventDefault();
        setSelectedTypeId('vt_contra');
      } else if (e.key === 'F5') {
        e.preventDefault();
        setSelectedTypeId('vt_payment');
      } else if (e.key === 'F6') {
        e.preventDefault();
        setSelectedTypeId('vt_receipt');
      } else if (e.key === 'F7') {
        e.preventDefault();
        setSelectedTypeId('vt_jnl');
      } else if (e.key === 'F8') {
        e.preventDefault();
        setSelectedTypeId('vt_sales');
      } else if (e.key === 'F9') {
        e.preventDefault();
        setSelectedTypeId('vt_pur');
      } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleSaveDraft();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        onBackToGateway();
      }
    },
    [isDraftsModalOpen, onBackToGateway]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div className="ot-voucher-entry-workspace">
      {/* Top Breadcrumb & Navigation */}
      <div className="ot-voucher-top-bar">
        <div className="ot-breadcrumb">
          <span className="ot-crumb-link" onClick={onBackToGateway}>
            Gateway
          </span>
          <span className="ot-crumb-sep">&gt;</span>
          <span className="ot-crumb-active">Accounting Vouchers</span>
          <span className="ot-badge-clean" style={{ marginLeft: '12px' }}>
            Sprint 2 Stage 4
          </span>
        </div>

        <div className="ot-top-actions">
          {onSwitchToMasters && (
            <button
              type="button"
              className="ot-btn ot-btn-secondary"
              onClick={onSwitchToMasters}
              title="Configure Voucher Types & Numbering Series"
            >
              Voucher Masters Config
            </button>
          )}
          <button
            type="button"
            className="ot-btn ot-btn-secondary"
            onClick={() => setIsDraftsModalOpen(true)}
            title="Open saved drafts modal"
          >
            Saved Drafts
          </button>
          <button
            type="button"
            className="ot-btn"
            onClick={onBackToGateway}
            title="Return to Gateway (Esc)"
          >
            Gateway (Esc)
          </button>
        </div>
      </div>

      {notification && (
        <div className="ot-notification-banner">
          <span className="ot-notification-icon">&#9432;</span>
          <span className="ot-notification-text">{notification}</span>
          <button className="ot-notification-close" onClick={() => setNotification(null)}>
            &times;
          </button>
        </div>
      )}

      {error && (
        <div className="ot-alert ot-alert-danger" style={{ margin: '12px 24px' }}>
          {error}
        </div>
      )}

      {loading ? (
        <div style={{ padding: '48px', textAlign: 'center' }}>Loading voucher workspace...</div>
      ) : (
        <div className="ot-voucher-workspace-body">
          {/* Header Form */}
          <VoucherHeaderForm
            voucherTypes={voucherTypes}
            selectedTypeId={selectedTypeId}
            onSelectType={setSelectedTypeId}
            seriesList={seriesList}
            selectedSeriesId={selectedSeriesId}
            onSelectSeries={setSelectedSeriesId}
            date={date}
            onChangeDate={setDate}
            effectiveDate={effectiveDate}
            onChangeEffectiveDate={setEffectiveDate}
            referenceNo={referenceNo}
            onChangeReferenceNo={setReferenceNo}
            referenceDate={referenceDate}
            onChangeReferenceDate={setReferenceDate}
            narration={narration}
            onChangeNarration={setNarration}
            provisionalNumber={provisionalNumber}
            dateInputRef={dateInputRef}
          />

          {/* Line Items Table */}
          <VoucherLineGrid
            lines={lines}
            ledgers={ledgers}
            onAddLine={handleAddLine}
            onUpdateLine={handleUpdateLine}
            onDeleteLine={handleDeleteLine}
            onMoveLineUp={handleMoveLineUp}
            onMoveLineDown={handleMoveLineDown}
          />

          {/* Sticky Running Totals and Balance Bar */}
          <VoucherTotalBar
            debitTotal={debitTotal}
            creditTotal={creditTotal}
            difference={difference}
            isBalanced={isBalanced}
            onSaveDraft={handleSaveDraft}
            onOpenDraftsModal={() => setIsDraftsModalOpen(true)}
            onNewDraft={handleNewDraft}
            saving={saving}
          />
        </div>
      )}

      {/* Drafts List Modal */}
      <DraftsModal
        isOpen={isDraftsModalOpen}
        onClose={() => setIsDraftsModalOpen(false)}
        onSelectDraft={handleSelectDraft}
        voucherTypes={voucherTypes}
      />
    </div>
  );
}
