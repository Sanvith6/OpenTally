import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  Company,
  BankLedgerItem,
  BankReconciliationStatement,
  BankStatementMatchPreview,
  ReconcileEntryInput,
} from '../../types';
import {
  listBankLedgers,
  getBankReconciliation,
  reconcileBankEntries,
  previewBankStatementAutoMatch,
} from '../../services/api';
import './BankReconciliationView.css';

interface Props {
  activeCompany: Company | null;
  initialBankLedgerId?: string;
  onBack: () => void;
  onViewVoucher?: (voucherId: string) => void;
}

export const BankReconciliationView: React.FC<Props> = ({
  activeCompany,
  initialBankLedgerId,
  onBack,
  onViewVoucher,
}) => {
  const [bankLedgers, setBankLedgers] = useState<BankLedgerItem[]>([]);
  const [selectedLedgerId, setSelectedLedgerId] = useState<string>(initialBankLedgerId || '');
  const [fromDate, setFromDate] = useState<string>(
    activeCompany?.financial_year_begins || '2026-04-01'
  );
  const [toDate, setToDate] = useState<string>('2026-04-30');
  const [includeReconciled, setIncludeReconciled] = useState<boolean>(false);

  const [statement, setStatement] = useState<BankReconciliationStatement | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [saving, setSaving] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // Edited bank dates: allocation_id -> YYYY-MM-DD or empty
  const [editedBankDates, setEditedBankDates] = useState<Record<string, string>>({});

  // CSV Modal State
  const [isCsvModalOpen, setIsCsvModalOpen] = useState<boolean>(false);
  const [csvContent, setCsvContent] = useState<string>('');
  const [csvPreview, setCsvPreview] = useState<BankStatementMatchPreview | null>(null);
  const [csvLoading, setCsvLoading] = useState<boolean>(false);

  // 1. Fetch available bank ledgers
  const fetchLedgers = useCallback(async () => {
    if (!activeCompany) return;
    try {
      const ledgers = await listBankLedgers(activeCompany.id);
      setBankLedgers(ledgers);
      if (!selectedLedgerId && ledgers.length > 0) {
        setSelectedLedgerId(ledgers[0].id);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [activeCompany, selectedLedgerId]);

  useEffect(() => {
    void fetchLedgers();
  }, [fetchLedgers]);

  // 2. Fetch BRS Statement
  const fetchBrs = useCallback(async () => {
    if (!activeCompany || !selectedLedgerId) return;
    setLoading(true);
    setError(null);
    try {
      const res = await getBankReconciliation(
        selectedLedgerId,
        fromDate,
        toDate,
        includeReconciled,
        activeCompany.id
      );
      setStatement(res);
      // Reset unsaved edits on reload
      setEditedBankDates({});
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, selectedLedgerId, fromDate, toDate, includeReconciled]);

  useEffect(() => {
    if (selectedLedgerId) {
      void fetchBrs();
    }
  }, [fetchBrs, selectedLedgerId]);

  // Handle inline date change
  const handleBankDateChange = (allocId: string, val: string) => {
    setEditedBankDates((prev) => ({
      ...prev,
      [allocId]: val,
    }));
  };

  // Determine dirty rows
  const dirtyCount = useMemo(() => {
    return Object.keys(editedBankDates).length;
  }, [editedBankDates]);

  // 3. Save modified reconciliation dates (Ctrl+A)
  const handleSave = useCallback(async () => {
    if (!activeCompany || dirtyCount === 0) return;
    setSaving(true);
    setError(null);
    setMessage(null);

    const updates: ReconcileEntryInput[] = Object.entries(editedBankDates).map(
      ([allocId, bDate]) => ({
        bank_allocation_id: allocId,
        bank_date: bDate.trim() ? bDate.trim() : null,
      })
    );

    try {
      const res = await reconcileBankEntries(updates, activeCompany.id);
      setMessage(`Successfully updated ${res.updated_count} bank reconciliation entries.`);
      setEditedBankDates({});
      await fetchBrs();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }, [activeCompany, dirtyCount, editedBankDates, fetchBrs]);

  // 4. Handle CSV Statement Parsing
  const handlePreviewCsv = async () => {
    if (!activeCompany || !selectedLedgerId || !csvContent.trim()) return;
    setCsvLoading(true);
    setError(null);
    try {
      const preview = await previewBankStatementAutoMatch(
        selectedLedgerId,
        csvContent,
        activeCompany.id
      );
      setCsvPreview(preview);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCsvLoading(false);
    }
  };

  // 5. Apply CSV matches
  const handleApplyCsvMatches = () => {
    if (!csvPreview) return;
    const newDates: Record<string, string> = { ...editedBankDates };
    let applied = 0;

    for (const row of csvPreview.rows) {
      if (
        (row.match_type === 'exact' || row.match_type === 'suggested') &&
        row.matched_bank_allocation_id &&
        row.suggested_bank_date
      ) {
        newDates[row.matched_bank_allocation_id] = row.suggested_bank_date;
        applied++;
      }
    }

    setEditedBankDates(newDates);
    setIsCsvModalOpen(false);
    setCsvPreview(null);
    setCsvContent('');
    setMessage(`Applied ${applied} bank clearing dates from statement. Press Save (Ctrl+A) to commit.`);
  };

  // Keyboard navigation & Shortcuts (Esc to back, Ctrl+A to save)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (isCsvModalOpen) {
          setIsCsvModalOpen(false);
        } else {
          onBack();
        }
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'a' || e.key === 'A')) {
        // Prevent default select-all in table view
        const target = e.target as HTMLElement;
        if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA') {
          e.preventDefault();
          void handleSave();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onBack, isCsvModalOpen, handleSave]);

  const selectedLedger = bankLedgers.find((l) => l.id === selectedLedgerId);

  return (
    <div className="ot-brs-container" data-testid="bank-reconciliation-view">
      {/* Header & Controls */}
      <div className="ot-brs-header">
        <div className="ot-brs-title-bar">
          <h2>
            <span>🏦</span> Bank Reconciliation Statement (BRS)
          </h2>
          <div className="ot-brs-actions">
            <button
              className="ot-brs-btn"
              onClick={() => setIsCsvModalOpen(true)}
              data-testid="btn-import-csv"
            >
              📥 Import Statement (CSV)
            </button>
            <button
              className="ot-brs-btn ot-brs-btn-primary"
              onClick={() => void handleSave()}
              disabled={saving || dirtyCount === 0}
              data-testid="btn-save-brs"
            >
              {saving ? 'Saving...' : `💾 Save (Ctrl+A)${dirtyCount > 0 ? ` (${dirtyCount})` : ''}`}
            </button>
            <button className="ot-brs-btn" onClick={onBack} data-testid="btn-back">
              Esc - Back
            </button>
          </div>
        </div>

        <div className="ot-brs-controls">
          <div className="ot-brs-control-group">
            <label htmlFor="brs-bank-select">Bank Ledger:</label>
            <select
              id="brs-bank-select"
              className="ot-brs-select"
              value={selectedLedgerId}
              onChange={(e) => setSelectedLedgerId(e.target.value)}
              data-testid="select-bank-ledger"
            >
              {bankLedgers.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.name} ({l.current_balance} {l.current_balance_dr_cr})
                </option>
              ))}
            </select>
          </div>

          <div className="ot-brs-control-group">
            <label htmlFor="brs-from-date">From:</label>
            <input
              id="brs-from-date"
              type="date"
              className="ot-brs-input"
              value={fromDate}
              onChange={(e) => setFromDate(e.target.value)}
            />
          </div>

          <div className="ot-brs-control-group">
            <label htmlFor="brs-to-date">To:</label>
            <input
              id="brs-to-date"
              type="date"
              className="ot-brs-input"
              value={toDate}
              onChange={(e) => setToDate(e.target.value)}
            />
          </div>

          <label className="ot-brs-checkbox-group">
            <input
              type="checkbox"
              checked={includeReconciled}
              onChange={(e) => setIncludeReconciled(e.target.checked)}
              data-testid="toggle-include-reconciled"
            />
            Show Reconciled
          </label>

          <button className="ot-brs-btn" onClick={() => void fetchBrs()} disabled={loading}>
            {loading ? 'Refreshing...' : '🔄 Refresh'}
          </button>
        </div>
      </div>

      {/* Notifications */}
      {error && (
        <div style={{ background: '#7f1d1d', color: '#fecaca', padding: '0.6rem 1.5rem', fontSize: '0.85rem' }}>
          ⚠️ {error}
        </div>
      )}
      {message && (
        <div style={{ background: '#064e3b', color: '#a7f3d0', padding: '0.6rem 1.5rem', fontSize: '0.85rem' }}>
          ✓ {message}
        </div>
      )}

      {/* BRS Table */}
      <div className="ot-brs-table-container">
        <table className="ot-brs-table" data-testid="brs-table">
          <thead>
            <tr>
              <th>Date</th>
              <th>Particulars</th>
              <th>Vch Type</th>
              <th>Vch No</th>
              <th>Trans Type</th>
              <th>Inst No</th>
              <th>Inst Date</th>
              <th style={{ width: '140px' }}>Bank Date</th>
              <th className="ot-brs-text-right">Debit</th>
              <th className="ot-brs-text-right">Credit</th>
            </tr>
          </thead>
          <tbody>
            {statement && statement.rows.length > 0 ? (
              statement.rows.map((row) => {
                const isModified = row.bank_allocation_id in editedBankDates;
                const displayBankDate = isModified
                  ? editedBankDates[row.bank_allocation_id]
                  : row.bank_date || '';

                return (
                  <tr key={row.bank_allocation_id} data-testid={`brs-row-${row.bank_allocation_id}`}>
                    <td>{row.voucher_date}</td>
                    <td>{row.particulars}</td>
                    <td>{row.voucher_type}</td>
                    <td>
                      {onViewVoucher ? (
                        <span
                          className="ot-brs-link"
                          onClick={() => onViewVoucher(row.voucher_id)}
                          data-testid={`vch-link-${row.voucher_number}`}
                        >
                          {row.voucher_number}
                        </span>
                      ) : (
                        row.voucher_number
                      )}
                    </td>
                    <td>{row.transaction_type}</td>
                    <td>{row.instrument_number || '-'}</td>
                    <td>{row.instrument_date || '-'}</td>
                    <td>
                      <input
                        type="date"
                        className={`ot-brs-date-input ${isModified ? 'modified' : ''}`}
                        value={displayBankDate}
                        onChange={(e) =>
                          handleBankDateChange(row.bank_allocation_id, e.target.value)
                        }
                        data-testid={`input-bank-date-${row.bank_allocation_id}`}
                      />
                    </td>
                    <td className="ot-brs-text-right">
                      {row.debit_amount !== '0.00' ? row.debit_amount : ''}
                    </td>
                    <td className="ot-brs-text-right">
                      {row.credit_amount !== '0.00' ? row.credit_amount : ''}
                    </td>
                  </tr>
                );
              })
            ) : (
              <tr>
                <td colSpan={10} style={{ textAlign: 'center', padding: '2rem', color: '#64748b' }}>
                  {loading ? 'Loading bank allocations...' : 'No bank entries for this period.'}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* Summary Box (TallyPrime BRS Standard) */}
      {statement && (
        <div className="ot-brs-summary-box" data-testid="brs-summary-box">
          <div className="ot-brs-summary-card">
            <div className="ot-brs-summary-row bold">
              <span>Balance as per Company Books:</span>
              <span data-testid="summary-books-balance">
                {statement.summary.book_balance} {statement.summary.book_balance_dr_cr}
              </span>
            </div>
            <div className="ot-brs-summary-row">
              <span>Amounts not reflected in Bank (Debit):</span>
              <span data-testid="summary-unrec-dr">{statement.summary.unreconciled_debit}</span>
            </div>
            <div className="ot-brs-summary-row">
              <span>Amounts not reflected in Bank (Credit):</span>
              <span data-testid="summary-unrec-cr">{statement.summary.unreconciled_credit}</span>
            </div>
            <div className="ot-brs-summary-row final">
              <span>Balance as per Bank:</span>
              <span data-testid="summary-bank-balance">
                {statement.summary.bank_balance} {statement.summary.bank_balance_dr_cr}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Status Bar */}
      <div className="ot-brs-status-bar">
        <span>Bank: {selectedLedger ? selectedLedger.name : 'None selected'}</span>
        <span>
          Period: {fromDate} to {toDate} | Total Entries: {statement?.rows.length || 0}
        </span>
      </div>

      {/* CSV Auto-Match Modal */}
      {isCsvModalOpen && (
        <div className="ot-brs-modal-overlay" data-testid="csv-modal">
          <div className="ot-brs-modal">
            <div className="ot-brs-modal-header">
              <h3>📥 Auto-Match Bank Statement CSV</h3>
              <button className="ot-brs-btn" onClick={() => setIsCsvModalOpen(false)}>
                ✕
              </button>
            </div>
            <div className="ot-brs-modal-body">
              <p style={{ margin: 0, fontSize: '0.85rem', color: '#94a3b8' }}>
                Paste bank statement CSV data with standard headers (Date, Description, Reference,
                Withdrawal/Debit, Deposit/Credit).
              </p>
              <textarea
                className="ot-brs-csv-textarea"
                placeholder="Date,Description,Reference,Withdrawal,Deposit&#10;2026-04-10,Cheque Cleared,CHQ1001,15000.00,0.00&#10;2026-04-12,NEFT Inflow,NEFT999,0.00,25000.00"
                value={csvContent}
                onChange={(e) => setCsvContent(e.target.value)}
                data-testid="csv-textarea"
              />
              <button
                className="ot-brs-btn"
                onClick={() => void handlePreviewCsv()}
                disabled={csvLoading || !csvContent.trim()}
                data-testid="btn-preview-csv"
              >
                {csvLoading ? 'Analyzing...' : '🔍 Parse & Match Statement'}
              </button>

              {csvPreview && (
                <div>
                  <div style={{ display: 'flex', gap: '1rem', margin: '0.75rem 0', fontSize: '0.85rem' }}>
                    <span>Total Rows: <strong>{csvPreview.total_statement_rows}</strong></span>
                    <span style={{ color: '#34d399' }}>Exact Matches: <strong>{csvPreview.exact_matches}</strong></span>
                    <span style={{ color: '#fde047' }}>Suggested Matches: <strong>{csvPreview.suggested_matches}</strong></span>
                    <span style={{ color: '#94a3b8' }}>Unmatched: <strong>{csvPreview.unmatched}</strong></span>
                  </div>

                  <table className="ot-brs-table" style={{ fontSize: '0.8rem' }} data-testid="csv-preview-table">
                    <thead>
                      <tr>
                        <th>Date</th>
                        <th>Ref</th>
                        <th>Description</th>
                        <th className="ot-brs-text-right">Amount</th>
                        <th>Type</th>
                        <th>Match Status</th>
                        <th>Matched Voucher</th>
                        <th>Suggested Date</th>
                      </tr>
                    </thead>
                    <tbody>
                      {csvPreview.rows.map((r) => (
                        <tr key={r.csv_row_index}>
                          <td>{r.statement_date}</td>
                          <td>{r.statement_ref}</td>
                          <td>{r.statement_description}</td>
                          <td className="ot-brs-text-right">{r.statement_amount}</td>
                          <td>{r.statement_dr_cr}</td>
                          <td>
                            <span className={`ot-badge-${r.match_type}`}>
                              {r.match_type.toUpperCase()} ({r.confidence})
                            </span>
                          </td>
                          <td>{r.matched_voucher_number || '-'}</td>
                          <td>{r.suggested_bank_date || '-'}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
            <div className="ot-brs-modal-footer">
              <button className="ot-brs-btn" onClick={() => setIsCsvModalOpen(false)}>
                Cancel
              </button>
              <button
                className="ot-brs-btn ot-brs-btn-primary"
                onClick={handleApplyCsvMatches}
                disabled={!csvPreview || (csvPreview.exact_matches === 0 && csvPreview.suggested_matches === 0)}
                data-testid="btn-apply-csv-matches"
              >
                Apply Matches to BRS
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
