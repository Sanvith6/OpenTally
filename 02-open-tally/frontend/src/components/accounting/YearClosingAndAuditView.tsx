import { useCallback, useEffect, useState } from 'react';
import { AuditCertificationReport, Company, FinancialYearClosingRecord, YearEndClosingPreview } from '../../types';
import {
  executeYearEndClosing,
  exportAuditCertificationCsv,
  listClosedYears,
  previewYearEndClosing,
  verifyAuditTrailIntegrity,
} from '../../services/api';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function YearClosingAndAuditView({ activeCompany, onBack }: Props) {
  const [activeTab, setActiveTab] = useState<'closing' | 'audit'>('closing');

  // Year Closing state
  const [closedYears, setClosedYears] = useState<FinancialYearClosingRecord[]>([]);
  const [periodEnd, setPeriodEnd] = useState<string>('2025-03-31');
  const [preview, setPreview] = useState<YearEndClosingPreview | null>(null);
  const [closingLoading, setClosingLoading] = useState(false);
  const [closingMessage, setClosingMessage] = useState<string | null>(null);

  // Audit Trail state
  const [auditReport, setAuditReport] = useState<AuditCertificationReport | null>(null);
  const [auditLoading, setAuditLoading] = useState(false);

  const [error, setError] = useState<string | null>(null);

  const fetchClosedYears = useCallback(async () => {
    if (!activeCompany) return;
    try {
      const list = await listClosedYears(activeCompany.id);
      setClosedYears(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [activeCompany]);

  const runAuditVerification = useCallback(async () => {
    if (!activeCompany) return;
    setAuditLoading(true);
    setError(null);
    try {
      const rep = await verifyAuditTrailIntegrity(activeCompany.id);
      setAuditReport(rep);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setAuditLoading(false);
    }
  }, [activeCompany]);

  useEffect(() => {
    void fetchClosedYears();
    void runAuditVerification();
  }, [fetchClosedYears, runAuditVerification]);

  // Esc hotkey
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onBack();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onBack]);

  const handlePreview = async () => {
    if (!activeCompany) return;
    setClosingLoading(true);
    setError(null);
    setClosingMessage(null);
    try {
      const p = await previewYearEndClosing(periodEnd, activeCompany.id);
      setPreview(p);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setClosingLoading(false);
    }
  };

  const handleExecuteClosing = async () => {
    if (!activeCompany || !preview) return;
    if (!preview.is_equilibrium) {
      alert(`Cannot close financial year: Trial balance has a difference of ₹${preview.trial_balance_difference}`);
      return;
    }
    const confirmed = confirm(
      `CONFIRM FINANCIAL YEAR CLOSING:\n\n` +
      `This will:\n` +
      `1. Zero nominal P&L accounts via an automated Closing Journal.\n` +
      `2. Roll forward balance sheet opening balances to next FY (${preview.next_fy_start}).\n` +
      `3. Permanently lock the period up to ${preview.period_end}.\n\n` +
      `Proceed?`
    );
    if (!confirmed) return;

    setClosingLoading(true);
    setError(null);
    try {
      const record = await executeYearEndClosing(periodEnd, 'admin', activeCompany.id);
      setClosingMessage(`Successfully closed ${record.financial_year_label}. Period locked up to ${record.period_end}.`);
      setPreview(null);
      await fetchClosedYears();
      await runAuditVerification();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setClosingLoading(false);
    }
  };

  const handleExportAuditCsv = async () => {
    if (!activeCompany) return;
    try {
      const csv = await exportAuditCertificationCsv(activeCompany.id);
      const blob = new Blob([csv], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `MCA_Audit_Certification_${activeCompany.id}.csv`;
      a.click();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div style={{ padding: '16px', background: '#0f172a', color: '#f8fafc', height: '100%', overflowY: 'auto' }}>
      {/* Top Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid #334155', paddingBottom: '12px', marginBottom: '16px' }}>
        <div>
          <h2 style={{ margin: 0, fontSize: '1.25rem', color: '#38bdf8', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>Year-End Closing & MCA Audit Certification</span>
            <span style={{ fontSize: '0.85rem', color: '#94a3b8', fontWeight: 'normal' }}>
              ({activeCompany?.name || 'OpenTally'})
            </span>
          </h2>
          <div style={{ fontSize: '0.8rem', color: '#94a3b8', marginTop: '4px' }}>
            Statutory financial year rollover, period lock enforcement, and forensic cryptographic audit verification
          </div>
        </div>

        <div style={{ display: 'flex', gap: '12px' }}>
          <button
            onClick={onBack}
            style={{ background: '#334155', color: '#cbd5e1', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.85rem' }}
          >
            Esc: Back
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div style={{ display: 'flex', gap: '8px', borderBottom: '1px solid #334155', marginBottom: '16px' }}>
        <button
          onClick={() => setActiveTab('closing')}
          style={{
            background: activeTab === 'closing' ? '#1e293b' : 'transparent',
            color: activeTab === 'closing' ? '#38bdf8' : '#94a3b8',
            border: 'none',
            borderBottom: activeTab === 'closing' ? '2px solid #38bdf8' : 'none',
            padding: '8px 16px',
            cursor: 'pointer',
            fontWeight: 600,
          }}
        >
          Financial Year-End Closing & Period Lock
        </button>
        <button
          onClick={() => setActiveTab('audit')}
          style={{
            background: activeTab === 'audit' ? '#1e293b' : 'transparent',
            color: activeTab === 'audit' ? '#38bdf8' : '#94a3b8',
            border: 'none',
            borderBottom: activeTab === 'audit' ? '2px solid #38bdf8' : 'none',
            padding: '8px 16px',
            cursor: 'pointer',
            fontWeight: 600,
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
          }}
        >
          <span>MCA 2023 Audit Trail Certification</span>
          {auditReport && (
            <span
              style={{
                fontSize: '0.7rem',
                padding: '2px 6px',
                borderRadius: '4px',
                background: auditReport.is_valid ? '#064e3b' : '#7f1d1d',
                color: auditReport.is_valid ? '#a7f3d0' : '#fecaca',
              }}
            >
              {auditReport.certification_status}
            </span>
          )}
        </button>
      </div>

      {error && (
        <div style={{ background: '#7f1d1d', color: '#fecaca', padding: '10px 14px', borderRadius: '4px', marginBottom: '16px' }}>
          {error}
        </div>
      )}

      {closingMessage && (
        <div style={{ background: '#064e3b', color: '#a7f3d0', padding: '10px 14px', borderRadius: '4px', marginBottom: '16px' }}>
          {closingMessage}
        </div>
      )}

      {activeTab === 'closing' ? (
        /* Year Closing Tab */
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px' }}>
          {/* Left: Execution Form & Pre-Flight Preview */}
          <div style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', padding: '16px' }}>
            <h3 style={{ margin: '0 0 12px 0', fontSize: '1rem', color: '#38bdf8' }}>Close Financial Year</h3>
            <div style={{ display: 'flex', gap: '12px', alignItems: 'center', marginBottom: '16px' }}>
              <label style={{ fontSize: '0.85rem', color: '#cbd5e1' }}>
                Period Ending Date:
                <input
                  type="date"
                  value={periodEnd}
                  onChange={e => setPeriodEnd(e.target.value)}
                  style={{ marginLeft: '8px', padding: '6px 10px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                />
              </label>
              <button
                onClick={() => void handlePreview()}
                disabled={closingLoading}
                style={{ background: '#0284c7', color: '#fff', border: 'none', padding: '6px 16px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600 }}
              >
                {closingLoading ? 'Checking...' : 'Pre-Closing Audit Preview'}
              </button>
            </div>

            {preview && (
              <div style={{ background: '#0f172a', padding: '14px', borderRadius: '6px', border: '1px solid #334155' }}>
                <h4 style={{ margin: '0 0 10px 0', color: '#e2e8f0', fontSize: '0.9rem' }}>
                  Pre-Closing Pre-Flight Verification: {preview.financial_year_label}
                </h4>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', fontSize: '0.85rem' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #1e293b' }}>
                    <span style={{ color: '#94a3b8' }}>Trial Balance Equilibrium:</span>
                    <span style={{ fontWeight: 600, color: preview.is_equilibrium ? '#34d399' : '#f87171' }}>
                      {preview.is_equilibrium ? 'PASSED (Difference = 0.00)' : `FAILED (Diff: ₹${preview.trial_balance_difference})`}
                    </span>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #1e293b' }}>
                    <span style={{ color: '#94a3b8' }}>Calculated Net Result:</span>
                    <span style={{ fontWeight: 600, color: preview.is_profit ? '#34d399' : '#f87171' }}>
                      {preview.is_profit ? 'Net Profit' : 'Net Loss'}: ₹{preview.net_profit_or_loss}
                    </span>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #1e293b' }}>
                    <span style={{ color: '#94a3b8' }}>Nominal Accounts To Zero:</span>
                    <span style={{ color: '#e2e8f0' }}>{preview.nominal_transfers.length} Ledgers</span>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0', borderBottom: '1px solid #1e293b' }}>
                    <span style={{ color: '#94a3b8' }}>Balance Sheet Accounts To Roll Forward:</span>
                    <span style={{ color: '#e2e8f0' }}>{preview.balance_sheet_balances.length} Ledgers</span>
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'space-between', padding: '6px 0' }}>
                    <span style={{ color: '#94a3b8' }}>Next Financial Year Opening Date:</span>
                    <span style={{ color: '#38bdf8', fontWeight: 600 }}>{preview.next_fy_start}</span>
                  </div>
                </div>

                <div style={{ marginTop: '16px' }}>
                  <button
                    onClick={() => void handleExecuteClosing()}
                    disabled={!preview.is_equilibrium || closingLoading}
                    style={{
                      width: '100%',
                      background: preview.is_equilibrium ? '#dc2626' : '#475569',
                      color: '#fff',
                      border: 'none',
                      padding: '10px 16px',
                      borderRadius: '4px',
                      cursor: preview.is_equilibrium ? 'pointer' : 'not-allowed',
                      fontWeight: 700,
                    }}
                  >
                    {closingLoading ? 'Executing Closing...' : 'Execute Year-End Closing & Lock Books'}
                  </button>
                </div>
              </div>
            )}
          </div>

          {/* Right: Closed Financial Years History */}
          <div style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', overflow: 'hidden' }}>
            <div style={{ background: '#0f172a', padding: '10px 16px', borderBottom: '1px solid #334155', fontWeight: 600, color: '#e2e8f0' }}>
              Locked & Closed Financial Years
            </div>
            {closedYears.length === 0 ? (
              <div style={{ padding: '30px', textAlign: 'center', color: '#64748b' }}>
                No financial years have been locked or closed yet.
              </div>
            ) : (
              <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
                <thead>
                  <tr style={{ background: '#0f172a', borderBottom: '1px solid #334155', color: '#94a3b8', textAlign: 'left' }}>
                    <th style={{ padding: '8px 12px' }}>Label</th>
                    <th style={{ padding: '8px 12px' }}>Period</th>
                    <th style={{ padding: '8px 12px', textAlign: 'right' }}>Net Profit / Loss</th>
                    <th style={{ padding: '8px 12px', textAlign: 'center' }}>Lock Status</th>
                  </tr>
                </thead>
                <tbody>
                  {closedYears.map(cy => (
                    <tr key={cy.id} style={{ borderBottom: '1px solid #334155' }}>
                      <td style={{ padding: '10px 12px', fontWeight: 600, color: '#38bdf8' }}>{cy.financial_year_label}</td>
                      <td style={{ padding: '10px 12px', color: '#cbd5e1' }}>
                        {cy.period_start} to {cy.period_end}
                      </td>
                      <td style={{ padding: '10px 12px', textAlign: 'right', fontWeight: 600, color: parseFloat(cy.net_profit_or_loss) >= 0 ? '#34d399' : '#f87171' }}>
                        ₹{cy.net_profit_or_loss}
                      </td>
                      <td style={{ padding: '10px 12px', textAlign: 'center' }}>
                        <span style={{ background: '#7f1d1d', color: '#fecaca', fontSize: '0.75rem', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
                          🔒 LOCKED
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      ) : (
        /* MCA Audit Certification Tab */
        <div style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', padding: '20px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '1.1rem', color: '#38bdf8' }}>
                MCA 2023 Statutory Audit Trail Verification
              </h3>
              <p style={{ margin: '4px 0 0 0', color: '#94a3b8', fontSize: '0.85rem' }}>
                Verifies cryptographic SHA-256 hash chaining, detects modifications, deletions, or broken audit sequences
              </p>
            </div>
            <div style={{ display: 'flex', gap: '10px' }}>
              <button
                onClick={() => void runAuditVerification()}
                disabled={auditLoading}
                style={{ background: '#0284c7', color: '#fff', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600 }}
              >
                {auditLoading ? 'Verifying Chain...' : 'Re-verify Audit Integrity'}
              </button>
              <button
                onClick={() => void handleExportAuditCsv()}
                style={{ background: '#059669', color: '#fff', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600 }}
              >
                Export Forensic Report (CSV)
              </button>
            </div>
          </div>

          {auditReport && (
            <div>
              {/* Status Banner */}
              <div
                style={{
                  background: auditReport.is_valid ? '#064e3b' : '#7f1d1d',
                  color: auditReport.is_valid ? '#a7f3d0' : '#fecaca',
                  padding: '16px',
                  borderRadius: '6px',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  marginBottom: '20px',
                }}
              >
                <div>
                  <div style={{ fontSize: '1.1rem', fontWeight: 700 }}>
                    {auditReport.is_valid ? '✓ AUDIT TRAIL INTEGRITY VERIFIED (CLEAN)' : '⚠ AUDIT TRAIL TAMPER DETECTED'}
                  </div>
                  <div style={{ fontSize: '0.85rem', marginTop: '4px' }}>
                    {auditReport.is_valid
                      ? 'All cryptographic SHA-256 hashes match and the log sequence chain is intact.'
                      : `${auditReport.tampered_entries} tampered records and ${auditReport.broken_chain_links} broken chain links detected.`}
                  </div>
                </div>
                <div style={{ fontSize: '0.8rem', textAlign: 'right' }}>
                  Verified At: {new Date(auditReport.generated_at).toLocaleString()}
                </div>
              </div>

              {/* Metrics Grid */}
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '12px', marginBottom: '20px' }}>
                <div style={{ background: '#0f172a', padding: '12px', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ fontSize: '0.75rem', color: '#94a3b8' }}>TOTAL ENTRIES</div>
                  <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#f8fafc', marginTop: '4px' }}>
                    {auditReport.total_entries}
                  </div>
                </div>
                <div style={{ background: '#0f172a', padding: '12px', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ fontSize: '0.75rem', color: '#94a3b8' }}>CHAINED HASH ENTRIES</div>
                  <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#38bdf8', marginTop: '4px' }}>
                    {auditReport.chained_entries}
                  </div>
                </div>
                <div style={{ background: '#0f172a', padding: '12px', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ fontSize: '0.75rem', color: '#94a3b8' }}>LEGACY UNCHAINED</div>
                  <div style={{ fontSize: '1.25rem', fontWeight: 700, color: '#94a3b8', marginTop: '4px' }}>
                    {auditReport.legacy_unchained_entries}
                  </div>
                </div>
                <div style={{ background: '#0f172a', padding: '12px', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ fontSize: '0.75rem', color: '#94a3b8' }}>TAMPERED ENTRIES</div>
                  <div style={{ fontSize: '1.25rem', fontWeight: 700, color: auditReport.tampered_entries > 0 ? '#f87171' : '#34d399', marginTop: '4px' }}>
                    {auditReport.tampered_entries}
                  </div>
                </div>
                <div style={{ background: '#0f172a', padding: '12px', borderRadius: '6px', border: '1px solid #334155' }}>
                  <div style={{ fontSize: '0.75rem', color: '#94a3b8' }}>BROKEN CHAIN LINKS</div>
                  <div style={{ fontSize: '1.25rem', fontWeight: 700, color: auditReport.broken_chain_links > 0 ? '#f87171' : '#34d399', marginTop: '4px' }}>
                    {auditReport.broken_chain_links}
                  </div>
                </div>
              </div>

              {/* Latest Hash Details */}
              <div style={{ background: '#0f172a', padding: '12px 16px', borderRadius: '6px', border: '1px solid #334155', fontSize: '0.85rem' }}>
                <span style={{ color: '#94a3b8' }}>Latest Entry Cryptographic Fingerprint (SHA-256): </span>
                <code style={{ color: '#38bdf8', wordBreak: 'break-all' }}>
                  {auditReport.latest_entry_hash || 'N/A (Chain Genesis)'}
                </code>
              </div>

              {/* Discrepancies Table */}
              {auditReport.issues.length > 0 && (
                <div style={{ marginTop: '20px' }}>
                  <h4 style={{ margin: '0 0 10px 0', color: '#f87171' }}>Discrepancies & Audit Violations</h4>
                  <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.8rem' }}>
                    <thead>
                      <tr style={{ background: '#0f172a', color: '#94a3b8', textAlign: 'left' }}>
                        <th style={{ padding: '6px 10px' }}>Entry ID</th>
                        <th style={{ padding: '6px 10px' }}>Entity</th>
                        <th style={{ padding: '6px 10px' }}>Issue Type</th>
                        <th style={{ padding: '6px 10px' }}>Expected Hash</th>
                        <th style={{ padding: '6px 10px' }}>Actual Hash</th>
                      </tr>
                    </thead>
                    <tbody>
                      {auditReport.issues.map(iss => (
                        <tr key={iss.entry_id} style={{ borderBottom: '1px solid #334155', color: '#fecaca' }}>
                          <td style={{ padding: '8px 10px' }}>{iss.entry_id}</td>
                          <td style={{ padding: '8px 10px' }}>{iss.entity_type} ({iss.entity_id})</td>
                          <td style={{ padding: '8px 10px', fontWeight: 600 }}>{iss.issue_type}</td>
                          <td style={{ padding: '8px 10px' }}><code>{iss.expected_hash.slice(0, 16)}...</code></td>
                          <td style={{ padding: '8px 10px' }}><code>{iss.actual_hash.slice(0, 16)}...</code></td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
