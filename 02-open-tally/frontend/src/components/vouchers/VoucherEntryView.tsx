import { useState, useEffect, useRef, useMemo } from 'react';
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
import { BillWiseAllocationModal } from './BillWiseAllocationModal';
import { formatFromScaledBigInt } from '../../utils/decimal';
import { minorAmount } from './terminalAmounts';
import { useTerminalKeyboard } from './terminalKeyboard';
import { AcceptVoucherDialog } from './AcceptVoucherDialog';
import { postAccountingVoucher } from '../../services/voucherTerminalApi';
import './VoucherEntryView.css';
import { VoucherEntryToolbar } from './VoucherEntryToolbar';

interface VoucherEntryViewProps {
  activeCompany: Company | null;
  onBackToGateway: () => void;
  onSwitchToMasters?: () => void;
  initialTypeId?: string;
}

export function VoucherEntryView({
  activeCompany,
  onBackToGateway,
  onSwitchToMasters,
  initialTypeId,
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
  const [billAllocLineIndex, setBillAllocLineIndex] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);
  const [acceptOpen, setAcceptOpen] = useState(false);
  const [posted, setPosted] = useState(false);
  const accepting = useRef(false);
  const workspaceRef = useRef<HTMLDivElement>(null);

  const dateInputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (/payment|contra|receipt/i.test(initialTypeId || '')) setLines([
      { id: 'payment-dr', ledger_id: '', entry_type: 'Dr', amount: '', narration: '' },
      { id: 'payment-cr', ledger_id: '', entry_type: 'Cr', amount: '', narration: '' },
    ]);
  }, [initialTypeId]);
  useEffect(() => { if (!loading) dateInputRef.current?.focus(); }, [loading]);

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

        // Default to initialTypeId or vt_sales or first available
        const targetType = initialTypeId
          ? types.find(
              (t) =>
                t.id === initialTypeId ||
                t.name.toLowerCase().includes(initialTypeId.toLowerCase())
            )
          : null;
        const defaultType = targetType || types.find((t) => t.id === 'vt_sales') || types[0];
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
  }, [activeCompany, initialTypeId]);

  useEffect(() => {
    if (initialTypeId && voucherTypes.length > 0) {
      const target = voucherTypes.find(
        (t) =>
          t.id === initialTypeId ||
          t.name.toLowerCase().includes(initialTypeId.toLowerCase())
      );
      if (target) {
        setSelectedTypeId(target.id);
      }
    }
  }, [initialTypeId, voucherTypes]);

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
    let dr = 0n;
    let cr = 0n;
    for (const l of lines) {
      const val = minorAmount(l.amount);
      if (l.entry_type === 'Dr') dr += val;
      else if (l.entry_type === 'Cr') cr += val;
    }
    const diff = dr > cr ? dr - cr : cr - dr;
    const balanced = dr === cr && lines.length > 0 && dr > 0n;
    return {
      debitTotal: formatFromScaledBigInt(dr, 2, 2),
      creditTotal: formatFromScaledBigInt(cr, 2, 2),
      difference: formatFromScaledBigInt(diff, 2, 2),
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
    setPosted(false);
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
        lines: lines.filter((l) => l.ledger_id && minorAmount(l.amount) > 0n),
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

  const handleAccept = async () => {
    if (accepting.current || posted) return;
    if (!isBalanced || lines.length < 2 || lines.some((line) => !line.ledger_id || minorAmount(line.amount) <= 0n)) {
      setError('Complete every ledger and positive amount; Debit and Credit must balance exactly before accepting.');
      setAcceptOpen(false); return;
    }
    if (!selectedTypeId || !selectedSeriesId) { setError('Select a voucher type and active numbering series.'); return; }
    accepting.current = true; setSaving(true); setError(null);
    try {
      const draft = await saveVoucherDraft({ voucher_id: activeDraftId, voucher_type_id: selectedTypeId,
        series_id: selectedSeriesId, date, effective_date: effectiveDate || null, reference_no: referenceNo || null,
        reference_date: referenceDate || null, narration: narration || null, lines });
      setActiveDraftId(draft.voucher.id);
      const result = await postAccountingVoucher(draft.voucher.id);
      setProvisionalNumber(result.voucher.voucher_number); setPosted(true); setAcceptOpen(false);
      showNotification(`Voucher ${result.voucher.voucher_number} posted successfully.`);
    } catch (err) { setError(err instanceof Error ? err.message : String(err)); setAcceptOpen(false); }
    finally { accepting.current = false; setSaving(false); }
  };

  useTerminalKeyboard({ container: workspaceRef, onAccept: () => void handleAccept(), onBack: onBackToGateway,
    onSave: () => void handleSaveDraft(), onDate: () => dateInputRef.current?.focus(), onVoucherType: setSelectedTypeId,
    onInsert: () => { handleAddLine(); setTimeout(() => document.getElementById(`line-type-${lines.length}`)?.focus(), 0); } });

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

  return (
    <div className="ot-voucher-entry-workspace" ref={workspaceRef} data-terminal-entry>
      <VoucherEntryToolbar typeId={selectedTypeId} types={voucherTypes} onBack={onBackToGateway} onMasters={onSwitchToMasters}
        onDrafts={() => setIsDraftsModalOpen(true)} notification={notification} onClearNotification={() => setNotification(null)} error={error} />
      {loading ? (
        <div style={{ padding: '48px', textAlign: 'center' }}>Loading voucher workspace...</div>
      ) : (
        <div className="ot-voucher-workspace-body">
          {/* Header Form */}
          <VoucherHeaderForm
            voucherTypes={voucherTypes} selectedTypeId={selectedTypeId} onSelectType={setSelectedTypeId}
            seriesList={seriesList} selectedSeriesId={selectedSeriesId} onSelectSeries={setSelectedSeriesId}
            date={date} onChangeDate={setDate} effectiveDate={effectiveDate} onChangeEffectiveDate={setEffectiveDate}
            referenceNo={referenceNo} onChangeReferenceNo={setReferenceNo}
            referenceDate={referenceDate} onChangeReferenceDate={setReferenceDate}
            narration={narration} onChangeNarration={setNarration}
            provisionalNumber={provisionalNumber} dateInputRef={dateInputRef}
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
            onOpenBillAllocations={(idx) => setBillAllocLineIndex(idx)}
          />

          {/* Sticky Running Totals and Balance Bar */}
          <VoucherTotalBar
            debitTotal={debitTotal} creditTotal={creditTotal} difference={difference} isBalanced={isBalanced}
            onSaveDraft={handleSaveDraft}
            onOpenDraftsModal={() => setIsDraftsModalOpen(true)}
            onNewDraft={handleNewDraft}
            saving={saving} />
          <button type="button" className="ot-btn ot-btn-primary" data-terminal-order={1000} data-terminal-accept
            disabled={saving || posted} onClick={() => setAcceptOpen(true)}>Accept [Y/N] Â· Ctrl+A</button>
        </div>
      )}

      {acceptOpen && <AcceptVoucherDialog busy={saving} onYes={() => void handleAccept()} onNo={() => setAcceptOpen(false)} />}

      {/* Bill-Wise Allocation Modal */}
      {billAllocLineIndex !== null && lines[billAllocLineIndex] && (
        <BillWiseAllocationModal
          isOpen={true}
          partyLedgerId={lines[billAllocLineIndex].ledger_id}
          partyLedgerName={
            ledgers.find((l) => l.id === lines[billAllocLineIndex].ledger_id)?.name || 'Party Ledger'
          }
          lineAmount={lines[billAllocLineIndex].amount}
          lineEntryType={lines[billAllocLineIndex].entry_type}
          initialAllocations={lines[billAllocLineIndex].bill_allocations || []}
          onSaveAllocations={(allocs) => {
            handleUpdateLine(billAllocLineIndex, { bill_allocations: allocs });
            setBillAllocLineIndex(null);
            showNotification(`Bill-wise allocations recorded for line ${billAllocLineIndex + 1}.`);
          }}
          onClose={() => setBillAllocLineIndex(null)}
        />
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
