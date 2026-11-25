import React, { useCallback, useEffect, useState } from 'react';
import { AgingReport, Company, PendingBill } from '../../types';
import { getAgingAnalysis, getBillsPayable, getBillsReceivable } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
  onViewLedger?: (partyLedgerId: string, partyName?: string) => void;
}

const today = () => new Date().toISOString().slice(0, 10);

export function OutstandingAgingView({ activeCompany, onBack, onViewLedger }: Props) {
  const [tab, setTab] = useState<'receivables' | 'payables' | 'aging'>('receivables');
  const [asOfDate, setAsOfDate] = useState(today());
  const [search, setSearch] = useState('');
  const [bills, setBills] = useState<PendingBill[]>([]);
  const [agingReport, setAgingReport] = useState<AgingReport | null>(null);
  const [agingType, setAgingType] = useState<'receivable' | 'payable'>('receivable');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedParties, setExpandedParties] = useState<Set<string>>(new Set());

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      if (tab === 'receivables') {
        const data = await getBillsReceivable(asOfDate);
        setBills(data);
      } else if (tab === 'payables') {
        const data = await getBillsPayable(asOfDate);
        setBills(data);
      } else {
        const report = await getAgingAnalysis(agingType, asOfDate);
        setAgingReport(report);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, tab, asOfDate, agingType]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const togglePartyExpand = (partyId: string) => {
    setExpandedParties((prev) => {
      const next = new Set(prev);
      if (next.has(partyId)) {
        next.delete(partyId);
      } else {
        next.add(partyId);
      }
      return next;
    });
  };

  const filteredBills = bills.filter((b) => {
    if (!search.trim()) return true;
    const q = search.toLowerCase();
    return (
      b.party_name.toLowerCase().includes(q) ||
      b.bill_name.toLowerCase().includes(q) ||
      b.party_ledger_id.toLowerCase().includes(q)
    );
  });

  const totalOriginal = filteredBills.reduce(
    (sum, b) => sum + (parseFloat(b.original_amount) || 0),
    0
  );
  const totalSettled = filteredBills.reduce(
    (sum, b) => sum + (parseFloat(b.settled_amount) || 0),
    0
  );
  const totalPending = filteredBills.reduce(
    (sum, b) => sum + (parseFloat(b.pending_amount) || 0),
    0
  );
  const overdueCount = filteredBills.filter((b) => b.overdue_days > 0).length;

  return (
    <section className="ot-report-screen" data-testid="outstanding-aging-view">
      <header className="ot-report-header">
        <div>
          <h2>Outstanding Bills & Aging Analysis</h2>
          <p>
            Bill-wise receivables, payables, and aging buckets for{' '}
            {activeCompany?.name || 'the active company'}.
          </p>
        </div>
        <button className="ot-report-back" onClick={onBack}>
          Back to Gateway
        </button>
      </header>

      <div className="ot-report-toolbar" style={{ display: 'flex', gap: '1rem', alignItems: 'center', flexWrap: 'wrap' }}>
        <div style={{ display: 'flex', gap: '0.25rem', background: '#1e293b', padding: '0.25rem', borderRadius: '4px' }}>
          <button
            className={`ot-report-action ${tab === 'receivables' ? 'ot-action-primary' : ''}`}
            onClick={() => setTab('receivables')}
            style={{ fontWeight: tab === 'receivables' ? 'bold' : 'normal' }}
          >
            Receivables (Sundry Debtors)
          </button>
          <button
            className={`ot-report-action ${tab === 'payables' ? 'ot-action-primary' : ''}`}
            onClick={() => setTab('payables')}
            style={{ fontWeight: tab === 'payables' ? 'bold' : 'normal' }}
          >
            Payables (Sundry Creditors)
          </button>
          <button
            className={`ot-report-action ${tab === 'aging' ? 'ot-action-primary' : ''}`}
            onClick={() => setTab('aging')}
            style={{ fontWeight: tab === 'aging' ? 'bold' : 'normal' }}
          >
            Aging Analysis
          </button>
        </div>

        <label className="ot-report-field">
          As of Date
          <input
            type="date"
            value={asOfDate}
            onChange={(e) => setAsOfDate(e.target.value)}
          />
        </label>

        {tab === 'aging' && (
          <label className="ot-report-field">
            Aging Category
            <select
              value={agingType}
              onChange={(e) => setAgingType(e.target.value as 'receivable' | 'payable')}
            >
              <option value="receivable">Debtors (Receivables)</option>
              <option value="payable">Creditors (Payables)</option>
            </select>
          </label>
        )}

        {tab !== 'aging' && (
          <label className="ot-report-field">
            Search Party / Bill
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search by party or bill ref..."
            />
          </label>
        )}

        <button className="ot-report-action" onClick={() => void loadData()} disabled={loading}>
          {loading ? 'Refreshing...' : 'Refresh (F5)'}
        </button>
      </div>

      {error && (
        <div style={{ padding: '0.75rem 1rem', background: '#7f1d1d', color: '#fecaca', borderRadius: '4px', margin: '0.5rem 0' }}>
          {error}
        </div>
      )}

      {tab !== 'aging' ? (
        <>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
              gap: '1rem',
              margin: '1rem 0',
            }}
          >
            <div style={{ background: '#1e293b', padding: '1rem', borderRadius: '4px', borderLeft: '4px solid #3b82f6' }}>
              <div style={{ fontSize: '0.75rem', color: '#94a3b8', textTransform: 'uppercase' }}>Total Original</div>
              <div style={{ fontSize: '1.25rem', fontWeight: 'bold', color: '#f8fafc' }}>{totalOriginal.toFixed(2)}</div>
            </div>
            <div style={{ background: '#1e293b', padding: '1rem', borderRadius: '4px', borderLeft: '4px solid #10b981' }}>
              <div style={{ fontSize: '0.75rem', color: '#94a3b8', textTransform: 'uppercase' }}>Total Settled</div>
              <div style={{ fontSize: '1.25rem', fontWeight: 'bold', color: '#f8fafc' }}>{totalSettled.toFixed(2)}</div>
            </div>
            <div style={{ background: '#1e293b', padding: '1rem', borderRadius: '4px', borderLeft: '4px solid #f59e0b' }}>
              <div style={{ fontSize: '0.75rem', color: '#94a3b8', textTransform: 'uppercase' }}>Pending Balance</div>
              <div style={{ fontSize: '1.25rem', fontWeight: 'bold', color: '#f8fafc' }}>
                {totalPending.toFixed(2)} {tab === 'receivables' ? 'Dr' : 'Cr'}
              </div>
            </div>
            <div style={{ background: '#1e293b', padding: '1rem', borderRadius: '4px', borderLeft: '4px solid #ef4444' }}>
              <div style={{ fontSize: '0.75rem', color: '#94a3b8', textTransform: 'uppercase' }}>Overdue Bills</div>
              <div style={{ fontSize: '1.25rem', fontWeight: 'bold', color: '#f8fafc' }}>
                {overdueCount} of {filteredBills.length}
              </div>
            </div>
          </div>

          <div className="ot-table-wrapper" style={{ overflowX: 'auto', margin: '1rem 0' }}>
            <table className="ot-data-table" style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ background: '#0f172a', borderBottom: '2px solid #334155' }}>
                  <th style={{ padding: '0.75rem', textAlign: 'left' }}>Party Name</th>
                  <th style={{ padding: '0.75rem', textAlign: 'left' }}>Bill Reference</th>
                  <th style={{ padding: '0.75rem', textAlign: 'center' }}>Bill Date</th>
                  <th style={{ padding: '0.75rem', textAlign: 'center' }}>Due Date</th>
                  <th style={{ padding: '0.75rem', textAlign: 'center' }}>Overdue</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>Original Amount</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>Settled Amount</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>Pending Amount</th>
                </tr>
              </thead>
              <tbody>
                {filteredBills.length === 0 ? (
                  <tr>
                    <td colSpan={8} style={{ padding: '2rem', textAlign: 'center', color: '#94a3b8' }}>
                      {loading ? 'Loading bills...' : 'No pending bills found for the selected criteria.'}
                    </td>
                  </tr>
                ) : (
                  filteredBills.map((b, idx) => (
                    <tr
                      key={`${b.party_ledger_id}-${b.bill_name}-${idx}`}
                      style={{ borderBottom: '1px solid #1e293b' }}
                    >
                      <td style={{ padding: '0.75rem', fontWeight: 500 }}>
                        {onViewLedger ? (
                          <button
                            type="button"
                            className="ot-drilldown-btn"
                            style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold', textAlign: 'left' }}
                            onClick={() => onViewLedger(b.party_ledger_id, b.party_name)}
                            title="View Party Ledger Report"
                          >
                            {b.party_name}
                          </button>
                        ) : (
                          b.party_name
                        )}
                      </td>
                      <td style={{ padding: '0.75rem', fontFamily: 'monospace' }}>{b.bill_name}</td>
                      <td style={{ padding: '0.75rem', textAlign: 'center' }}>{b.bill_date}</td>
                      <td style={{ padding: '0.75rem', textAlign: 'center' }}>{b.due_date || '—'}</td>
                      <td style={{ padding: '0.75rem', textAlign: 'center' }}>
                        {b.overdue_days > 0 ? (
                          <span
                            style={{
                              background: '#7f1d1d',
                              color: '#fecaca',
                              padding: '0.2rem 0.5rem',
                              borderRadius: '4px',
                              fontSize: '0.8rem',
                              fontWeight: 600,
                            }}
                          >
                            {b.overdue_days} days
                          </span>
                        ) : (
                          <span style={{ color: '#10b981', fontSize: '0.85rem' }}>Current</span>
                        )}
                      </td>
                      <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                        {b.original_amount}
                      </td>
                      <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                        {b.settled_amount}
                      </td>
                      <td
                        style={{
                          padding: '0.75rem',
                          textAlign: 'right',
                          fontFamily: 'monospace',
                          fontWeight: 'bold',
                          color: '#38bdf8',
                        }}
                      >
                        {b.pending_amount} {b.dr_cr}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
              {filteredBills.length > 0 && (
                <tfoot>
                  <tr style={{ background: '#0f172a', borderTop: '2px solid #334155', fontWeight: 'bold' }}>
                    <td colSpan={5} style={{ padding: '0.75rem' }}>Grand Total ({filteredBills.length} bills)</td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {totalOriginal.toFixed(2)}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {totalSettled.toFixed(2)}
                    </td>
                    <td
                      style={{
                        padding: '0.75rem',
                        textAlign: 'right',
                        fontFamily: 'monospace',
                        color: '#38bdf8',
                      }}
                    >
                      {totalPending.toFixed(2)} {tab === 'receivables' ? 'Dr' : 'Cr'}
                    </td>
                  </tr>
                </tfoot>
              )}
            </table>
          </div>
        </>
      ) : (
        <>
          {agingReport && (
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))',
                gap: '0.75rem',
                margin: '1rem 0',
              }}
            >
              <div style={{ background: '#1e293b', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #3b82f6' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>Not Due</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold' }}>{agingReport.grand_total.not_due}</div>
              </div>
              <div style={{ background: '#1e293b', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #10b981' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>0-30 Days</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold' }}>{agingReport.grand_total.days_0_30}</div>
              </div>
              <div style={{ background: '#1e293b', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #f59e0b' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>31-60 Days</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold' }}>{agingReport.grand_total.days_31_60}</div>
              </div>
              <div style={{ background: '#1e293b', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #ea580c' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>61-90 Days</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold' }}>{agingReport.grand_total.days_61_90}</div>
              </div>
              <div style={{ background: '#1e293b', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #ef4444' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>90+ Days</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold' }}>{agingReport.grand_total.days_90_plus}</div>
              </div>
              <div style={{ background: '#0f172a', padding: '0.75rem', borderRadius: '4px', borderLeft: '4px solid #8b5cf6' }}>
                <div style={{ fontSize: '0.7rem', color: '#94a3b8', textTransform: 'uppercase' }}>Total Outstanding</div>
                <div style={{ fontSize: '1.1rem', fontWeight: 'bold', color: '#a78bfa' }}>
                  {agingReport.grand_total.total}
                </div>
              </div>
            </div>
          )}

          <div className="ot-table-wrapper" style={{ overflowX: 'auto', margin: '1rem 0' }}>
            <table className="ot-data-table" style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ background: '#0f172a', borderBottom: '2px solid #334155' }}>
                  <th style={{ padding: '0.75rem', textAlign: 'left' }}>Party Name</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>Not Due</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>0-30 Days</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>31-60 Days</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>61-90 Days</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>90+ Days</th>
                  <th style={{ padding: '0.75rem', textAlign: 'right' }}>Total</th>
                  <th style={{ padding: '0.75rem', textAlign: 'center' }}>Action</th>
                </tr>
              </thead>
              <tbody>
                {!agingReport || agingReport.parties.length === 0 ? (
                  <tr>
                    <td colSpan={8} style={{ padding: '2rem', textAlign: 'center', color: '#94a3b8' }}>
                      {loading ? 'Computing aging analysis...' : 'No aging records found.'}
                    </td>
                  </tr>
                ) : (
                  agingReport.parties.map((p) => {
                    const isExpanded = expandedParties.has(p.party_ledger_id);
                    return (
                      <React.Fragment key={p.party_ledger_id}>
                        <tr style={{ borderBottom: '1px solid #1e293b' }}>
                          <td style={{ padding: '0.75rem', fontWeight: 600 }}>{p.party_name}</td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                            {p.buckets.not_due}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                            {p.buckets.days_0_30}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                            {p.buckets.days_31_60}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                            {p.buckets.days_61_90}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace', color: p.buckets.days_90_plus !== '0.00' ? '#f87171' : 'inherit' }}>
                            {p.buckets.days_90_plus}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace', fontWeight: 'bold', color: '#38bdf8' }}>
                            {p.buckets.total}
                          </td>
                          <td style={{ padding: '0.75rem', textAlign: 'center' }}>
                            <button
                              className="ot-report-action"
                              style={{ padding: '0.2rem 0.5rem', fontSize: '0.75rem' }}
                              onClick={() => togglePartyExpand(p.party_ledger_id)}
                            >
                              {isExpanded ? 'Hide Bills' : `Show (${p.bills.length})`}
                            </button>
                          </td>
                        </tr>
                        {isExpanded && (
                          <tr style={{ background: '#0b1120' }}>
                            <td colSpan={8} style={{ padding: '0.5rem 1rem 1rem 1rem' }}>
                              <table style={{ width: '100%', fontSize: '0.85rem', borderCollapse: 'collapse' }}>
                                <thead>
                                  <tr style={{ color: '#94a3b8', borderBottom: '1px solid #334155' }}>
                                    <th style={{ textAlign: 'left', padding: '0.4rem' }}>Bill Reference</th>
                                    <th style={{ textAlign: 'center', padding: '0.4rem' }}>Bill Date</th>
                                    <th style={{ textAlign: 'center', padding: '0.4rem' }}>Due Date</th>
                                    <th style={{ textAlign: 'center', padding: '0.4rem' }}>Overdue Days</th>
                                    <th style={{ textAlign: 'right', padding: '0.4rem' }}>Original Amount</th>
                                    <th style={{ textAlign: 'right', padding: '0.4rem' }}>Settled Amount</th>
                                    <th style={{ textAlign: 'right', padding: '0.4rem' }}>Pending Balance</th>
                                  </tr>
                                </thead>
                                <tbody>
                                  {p.bills.map((b) => (
                                    <tr key={`${p.party_ledger_id}-${b.bill_name}`} style={{ borderBottom: '1px solid #1e293b' }}>
                                      <td style={{ padding: '0.4rem', fontFamily: 'monospace' }}>{b.bill_name}</td>
                                      <td style={{ padding: '0.4rem', textAlign: 'center' }}>{b.bill_date}</td>
                                      <td style={{ padding: '0.4rem', textAlign: 'center' }}>{b.due_date || '—'}</td>
                                      <td style={{ padding: '0.4rem', textAlign: 'center' }}>
                                        {b.overdue_days > 0 ? `${b.overdue_days} days` : 'Not Due'}
                                      </td>
                                      <td style={{ padding: '0.4rem', textAlign: 'right', fontFamily: 'monospace' }}>
                                        {b.original_amount}
                                      </td>
                                      <td style={{ padding: '0.4rem', textAlign: 'right', fontFamily: 'monospace' }}>
                                        {b.settled_amount}
                                      </td>
                                      <td style={{ padding: '0.4rem', textAlign: 'right', fontFamily: 'monospace', fontWeight: 600, color: '#38bdf8' }}>
                                        {b.pending_amount} {b.dr_cr}
                                      </td>
                                    </tr>
                                  ))}
                                </tbody>
                              </table>
                            </td>
                          </tr>
                        )}
                      </React.Fragment>
                    );
                  })
                )}
              </tbody>
              {agingReport && agingReport.parties.length > 0 && (
                <tfoot>
                  <tr style={{ background: '#0f172a', borderTop: '2px solid #334155', fontWeight: 'bold' }}>
                    <td style={{ padding: '0.75rem' }}>Grand Total</td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {agingReport.grand_total.not_due}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {agingReport.grand_total.days_0_30}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {agingReport.grand_total.days_31_60}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace' }}>
                      {agingReport.grand_total.days_61_90}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace', color: '#f87171' }}>
                      {agingReport.grand_total.days_90_plus}
                    </td>
                    <td style={{ padding: '0.75rem', textAlign: 'right', fontFamily: 'monospace', color: '#38bdf8' }}>
                      {agingReport.grand_total.total}
                    </td>
                    <td />
                  </tr>
                </tfoot>
              )}
            </table>
          </div>
        </>
      )}
    </section>
  );
}
