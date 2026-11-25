import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, Company, TrialBalanceReport } from '../../types';
import { getTrialBalance, listAccountGroups } from '../../services/api';
import '../reports/Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
  onViewLedger?: (ledgerId: string, ledgerName?: string) => void;
}

function downloadCsv(report: TrialBalanceReport): void {
  const quote = (value: string): string => `"${value.replace(/"/g, '""')}"`;
  const lines = [
    ['Trial Balance', report.as_of_date],
    [],
    ['Account Group', 'Ledger', 'Nature', 'Opening Dr', 'Opening Cr', 'Period Dr', 'Period Cr', 'Closing Dr', 'Closing Cr'],
    ...report.rows.map((row) => [row.account_group_name, row.ledger_name, row.account_group_nature, row.opening_debit, row.opening_credit, row.period_debit, row.period_credit, row.closing_debit, row.closing_credit]),
    [],
    ['Totals', '', '', '', '', '', '', report.total_debit, report.total_credit],
    ['Difference', report.difference],
  ].map((row) => row.map((value) => quote(String(value))).join(','));
  const blob = new Blob([lines.join('\r\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = `trial-balance-${report.as_of_date}.csv`;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function TrialBalanceView({ activeCompany, onBack, onViewLedger }: Props) {
  const [asOfDate, setAsOfDate] = useState(activeCompany?.financial_year_begins || '');
  const [groupId, setGroupId] = useState('');
  const [search, setSearch] = useState('');
  const [includeZero, setIncludeZero] = useState(false);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [report, setReport] = useState<TrialBalanceReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!activeCompany || !asOfDate) return;
    setLoading(true);
    setError(null);
    try {
      setReport(await getTrialBalance({ as_of_date: asOfDate, account_group_id: groupId || null, account_search: search || null, include_zero_balances: includeZero }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, asOfDate, groupId, includeZero, search]);

  useEffect(() => {
    listAccountGroups().then(setGroups).catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }, []);
  useEffect(() => { void load(); }, [load]);

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div><h2>Trial Balance</h2><p>Debit and credit equilibrium as of a selected date.</p></div>
        <button className="ot-report-back" onClick={onBack}>Back to Gateway</button>
      </div>
      <div className="ot-report-toolbar">
        <label className="ot-report-field">As of<input type="date" value={asOfDate} onChange={(event) => setAsOfDate(event.target.value)} /></label>
        <label className="ot-report-field">Account group<select value={groupId} onChange={(event) => setGroupId(event.target.value)}><option value="">All groups</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</select></label>
        <label className="ot-report-field">Ledger search<input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Account name" /></label>
        <label className="ot-report-field"><span>Include zero balances</span><input type="checkbox" checked={includeZero} onChange={(event) => setIncludeZero(event.target.checked)} /></label>
        <button className="ot-report-action" onClick={() => void load()} disabled={loading || !activeCompany}>Refresh</button>
        <button className="ot-report-action" onClick={() => report && downloadCsv(report)} disabled={!report}>Export CSV</button>
      </div>
      {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Closing debit</span><strong>{report.total_debit}</strong></div><div className="ot-report-summary-card"><span>Closing credit</span><strong>{report.total_credit}</strong></div><div className="ot-report-summary-card"><span>Difference</span><strong>{report.difference}</strong></div><div className="ot-report-summary-card"><span>Ledgers</span><strong>{report.ledger_count}</strong></div></div>}
      <div className="ot-report-card">
        {loading ? <div className="ot-report-loading">Loading trial balance…</div> : error ? <div className="ot-report-error">{error}</div> : !report || report.rows.length === 0 ? <div className="ot-report-empty">No ledger balances for this selection.</div> : <table className="ot-report-table"><thead><tr><th>Account group</th><th>Ledger</th><th>Nature</th><th className="money">Opening Dr</th><th className="money">Opening Cr</th><th className="money">Period Dr</th><th className="money">Period Cr</th><th className="money">Closing Dr</th><th className="money">Closing Cr</th></tr></thead><tbody>{report.rows.map((row) => <tr key={row.ledger_id}><td>{row.account_group_name}</td><td>{onViewLedger ? <button type="button" className="ot-drilldown-btn" style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold', textAlign: 'left' }} onClick={() => onViewLedger(row.ledger_id, row.ledger_name)} title="View Ledger Report">{row.ledger_name}</button> : row.ledger_name}</td><td>{row.account_group_nature}</td><td className="money">{row.opening_debit}</td><td className="money">{row.opening_credit}</td><td className="money">{row.period_debit}</td><td className="money">{row.period_credit}</td><td className="money">{row.closing_debit}</td><td className="money">{row.closing_credit}</td></tr>)}</tbody></table>}
      </div>
    </section>
  );
}
