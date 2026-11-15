import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, BalanceSheetLedgerRow, BalanceSheetReport, Company } from '../../types';
import { getBalanceSheet, listAccountGroups } from '../../services/api';
import './Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void; }

function csvValue(value: string): string {
  const safe = /^[=+\-@]/.test(value) ? `'${value}` : value;
  return `"${safe.replace(/"/g, '""')}"`;
}

function downloadCsv(report: BalanceSheetReport): void {
  const rows: string[][] = [
    ['Balance Sheet', report.company_id], ['As of', report.as_of_date], [],
    ['Section', 'Account group', 'Ledger', 'Debit', 'Credit', 'Net balance', 'Balance type'],
  ];
  const addSection = (title: string, sectionRows: BalanceSheetLedgerRow[], subtotal: string) => {
    rows.push([title, '', '', '', '', '', '']);
    sectionRows.forEach((row) => rows.push([title, row.account_group_name, row.ledger_name, row.debit, row.credit, row.presentation_amount, row.balance_type]));
    rows.push([`${title} subtotal`, '', '', '', '', subtotal, '']);
  };
  addSection('Assets', report.asset_rows, report.total_assets);
  addSection('Liabilities', report.liability_rows, report.total_liabilities);
  addSection('Equity', report.equity_rows, report.total_equity);
  rows.push(['Current-period result', '', '', '', '', report.current_period_profit_or_loss, report.current_period_result_type]);
  rows.push(['Total liabilities + equity', '', '', '', '', report.total_liabilities_and_equity, '']);
  rows.push(['Balance difference', '', '', '', '', report.balance_difference, report.is_balanced ? 'Balanced' : 'Not balanced']);
  const blob = new Blob([rows.map((row) => row.map(csvValue).join(',')).join('\r\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a'); anchor.href = url; anchor.download = `balance-sheet-${report.as_of_date}.csv`; anchor.click(); URL.revokeObjectURL(url);
}

function Section({ title, rows }: { title: string; rows: BalanceSheetLedgerRow[] }) {
  if (rows.length === 0) return null;
  return <>
    <tr className="ot-report-section-row"><th colSpan={3}>{title}</th><th colSpan={4}></th></tr>
    {rows.map((row) => <tr key={row.ledger_id}><td></td><td>{row.account_group_name}</td><td>{row.ledger_name}</td><td className="money">{row.debit}</td><td className="money">{row.credit}</td><td className="money">{row.presentation_amount}</td><td>{row.balance_type}</td></tr>)}
  </>;
}

function sectionRows(rows: BalanceSheetLedgerRow[], classification: string): BalanceSheetLedgerRow[] {
  return rows.filter((row) => row.classification === classification);
}

export function BalanceSheetView({ activeCompany, onBack }: Props) {
  const [asOfDate, setAsOfDate] = useState(activeCompany?.financial_year_begins || '');
  const [groupId, setGroupId] = useState('');
  const [search, setSearch] = useState('');
  const [includeZero, setIncludeZero] = useState(false);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [report, setReport] = useState<BalanceSheetReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!activeCompany || !asOfDate) return;
    setLoading(true); setError(null);
    try {
      setReport(await getBalanceSheet({ as_of_date: asOfDate, account_group_id: groupId || null, ledger_search: search || null, include_zero_balances: includeZero }));
    } catch (err) { setError(err instanceof Error ? err.message : String(err)); } finally { setLoading(false); }
  }, [activeCompany, asOfDate, groupId, includeZero, search]);

  useEffect(() => { listAccountGroups().then(setGroups).catch((err: unknown) => setError(err instanceof Error ? err.message : String(err))); }, []);
  useEffect(() => { void load(); }, [load]);

  return <section className="ot-report-screen">
    <div className="ot-report-header"><div><h2>Balance Sheet</h2><p>Financial position as of the selected date.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-toolbar">
      <label className="ot-report-field">As of<input type="date" value={asOfDate} onChange={(event) => setAsOfDate(event.target.value)} /></label>
      <label className="ot-report-field">Account group<select value={groupId} onChange={(event) => setGroupId(event.target.value)}><option value="">All groups</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</select></label>
      <label className="ot-report-field">Ledger search<input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Account name" /></label>
      <label className="ot-report-field"><span>Include zero balances</span><input type="checkbox" checked={includeZero} onChange={(event) => setIncludeZero(event.target.checked)} /></label>
      <button className="ot-report-action" onClick={() => void load()} disabled={loading || !activeCompany}>Refresh</button>
      <button className="ot-report-action" onClick={() => report && downloadCsv(report)} disabled={!report}>Export CSV</button>
    </div>
    {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Total assets</span><strong>{report.total_assets}</strong></div><div className="ot-report-summary-card"><span>Liabilities + equity</span><strong>{report.total_liabilities_and_equity}</strong></div><div className="ot-report-summary-card"><span>Difference</span><strong>{report.balance_difference}</strong></div><div className="ot-report-summary-card"><span>Status</span><strong>{report.is_balanced ? 'Balanced' : 'Not balanced'}</strong></div></div>}
    <div className="ot-report-card">
      {loading ? <div className="ot-report-loading">Loading Balance Sheet…</div> : error ? <div className="ot-report-error">{error}</div> : !report || report.ledger_count === 0 ? <div className="ot-report-empty">No balance-sheet accounts for this selection.</div> : <table className="ot-report-table"><thead><tr><th></th><th>Account group</th><th>Ledger</th><th className="money">Debit</th><th className="money">Credit</th><th className="money">Balance</th><th>Type</th></tr></thead><tbody>
        <Section title="Assets — Fixed Assets" rows={sectionRows(report.asset_rows, 'fixed_assets')} />
        <Section title="Assets — Current Assets" rows={sectionRows(report.asset_rows, 'current_assets')} />
        <Section title="Assets — Other Assets" rows={sectionRows(report.asset_rows, 'other_assets')} />
        <tr className="ot-report-result-row"><th colSpan={6}>Total Assets</th><th className="money">{report.total_assets}</th></tr>
        <Section title="Liabilities — Long-term" rows={sectionRows(report.liability_rows, 'long_term_liabilities')} />
        <Section title="Liabilities — Current" rows={sectionRows(report.liability_rows, 'current_liabilities')} />
        <Section title="Liabilities — Other" rows={sectionRows(report.liability_rows, 'other_liabilities')} />
        <tr className="ot-report-subtotal"><th colSpan={6}>Total Liabilities</th><th className="money">{report.total_liabilities}</th></tr>
        <Section title="Equity / Capital" rows={report.equity_rows} />
        <tr className="ot-report-subtotal"><th colSpan={6}>Current-period {report.current_period_result_type}</th><th className="money">{report.current_period_profit_or_loss}</th></tr>
        <tr className="ot-report-result-row"><th colSpan={6}>Total Equity</th><th className="money">{report.total_equity}</th></tr>
        <tr className="ot-report-result-row"><th colSpan={6}>Total Liabilities + Equity</th><th className="money">{report.total_liabilities_and_equity}</th></tr>
        <tr className="ot-report-result-row"><th colSpan={6}>Balance Difference ({report.is_balanced ? 'Balanced' : 'Not balanced'})</th><th className="money">{report.balance_difference}</th></tr>
      </tbody></table>}
    </div>
  </section>;
}
