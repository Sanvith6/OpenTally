import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, Company, ProfitAndLossLedgerRow, ProfitAndLossReport } from '../../types';
import { getProfitAndLoss, listAccountGroups } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

function csvValue(value: string): string {
  const safe = /^[=+\-@]/.test(value) ? `'${value}` : value;
  return `"${safe.replace(/"/g, '""')}"`;
}

function downloadCsv(report: ProfitAndLossReport): void {
  const rows: string[][] = [
    ['Profit & Loss', report.company_id],
    ['Period', `${report.date_from} to ${report.date_to}`],
    [],
    ['Section', 'Account group', 'Ledger', 'Debit', 'Credit', 'Contribution'],
  ];
  const addSection = (name: string, sectionRows: ProfitAndLossLedgerRow[], subtotal: string) => {
    rows.push([name, '', '', '', '', '']);
    sectionRows.forEach((row) => rows.push([name, row.account_group_name, row.ledger_name, row.debit, row.credit, row.contribution]));
    rows.push([`${name} subtotal`, '', '', '', '', subtotal]);
  };
  addSection('Direct Income', report.direct_income_rows, report.direct_income_subtotal);
  addSection('Direct Expenses', report.direct_expense_rows, report.direct_expense_subtotal);
  rows.push([`Gross ${report.gross_result_type}`, '', '', '', '', report.gross_profit_or_loss]);
  addSection('Indirect Income', report.indirect_income_rows, report.indirect_income_subtotal);
  addSection('Indirect Expenses', report.indirect_expense_rows, report.indirect_expense_subtotal);
  rows.push([`Net ${report.net_result_type}`, '', '', '', '', report.net_profit_or_loss]);
  const blob = new Blob([rows.map((row) => row.map(csvValue).join(',')).join('\r\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = `profit-and-loss-${report.date_from}-${report.date_to}.csv`;
  anchor.click();
  URL.revokeObjectURL(url);
}

function Section({ title, rows, subtotal }: { title: string; rows: ProfitAndLossLedgerRow[]; subtotal: string }) {
  return <>
    <tr className="ot-report-section-row"><th colSpan={3}>{title}</th><th colSpan={3}></th></tr>
    {rows.map((row) => <tr key={row.ledger_id}><td></td><td>{row.account_group_name}</td><td>{row.ledger_name}</td><td className="money">{row.debit}</td><td className="money">{row.credit}</td><td className="money">{row.contribution}</td></tr>)}
    <tr className="ot-report-subtotal"><td colSpan={5}>{title} subtotal</td><td className="money">{subtotal}</td></tr>
  </>;
}

export function ProfitAndLossView({ activeCompany, onBack }: Props) {
  const [from, setFrom] = useState(activeCompany?.financial_year_begins || '');
  const [to, setTo] = useState(activeCompany?.financial_year_begins || '');
  const [groupId, setGroupId] = useState('');
  const [search, setSearch] = useState('');
  const [includeZero, setIncludeZero] = useState(false);
  const [groups, setGroups] = useState<AccountGroup[]>([]);
  const [report, setReport] = useState<ProfitAndLossReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!activeCompany || !from || !to) return;
    setLoading(true);
    setError(null);
    try {
      setReport(await getProfitAndLoss({ period: { date_from: from, date_to: to }, account_group_id: groupId || null, ledger_search: search || null, include_zero_balances: includeZero }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, from, groupId, includeZero, search, to]);

  useEffect(() => {
    listAccountGroups().then(setGroups).catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)));
  }, []);
  useEffect(() => { void load(); }, [load]);

  return <section className="ot-report-screen">
    <div className="ot-report-header"><div><h2>Profit &amp; Loss</h2><p>Income and expenses for the selected accounting period.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-toolbar">
      <label className="ot-report-field">From<input type="date" value={from} onChange={(event) => setFrom(event.target.value)} /></label>
      <label className="ot-report-field">To<input type="date" value={to} onChange={(event) => setTo(event.target.value)} /></label>
      <label className="ot-report-field">Account group<select value={groupId} onChange={(event) => setGroupId(event.target.value)}><option value="">All groups</option>{groups.map((group) => <option key={group.id} value={group.id}>{group.name}</option>)}</select></label>
      <label className="ot-report-field">Ledger search<input value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Account name" /></label>
      <label className="ot-report-field"><span>Include zero balances</span><input type="checkbox" checked={includeZero} onChange={(event) => setIncludeZero(event.target.checked)} /></label>
      <button className="ot-report-action" onClick={() => void load()} disabled={loading || !activeCompany}>Refresh</button>
      <button className="ot-report-action" onClick={() => report && downloadCsv(report)} disabled={!report}>Export CSV</button>
    </div>
    {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Gross {report.gross_result_type}</span><strong>{report.gross_profit_or_loss}</strong></div><div className="ot-report-summary-card"><span>Net {report.net_result_type}</span><strong>{report.net_profit_or_loss}</strong></div><div className="ot-report-summary-card"><span>Ledgers</span><strong>{report.ledger_count}</strong></div></div>}
    <div className="ot-report-card">
      {loading ? <div className="ot-report-loading">Loading Profit &amp; Loss…</div> : error ? <div className="ot-report-error">{error}</div> : !report || report.ledger_count === 0 ? <div className="ot-report-empty">No income or expense activity for this period.</div> : <table className="ot-report-table"><thead><tr><th></th><th>Account group</th><th>Ledger</th><th className="money">Debit</th><th className="money">Credit</th><th className="money">Contribution</th></tr></thead><tbody>
        <Section title="Direct Income" rows={report.direct_income_rows} subtotal={report.direct_income_subtotal} />
        <Section title="Direct Expenses" rows={report.direct_expense_rows} subtotal={report.direct_expense_subtotal} />
        <tr className="ot-report-result-row"><th colSpan={5}>Gross {report.gross_result_type}</th><th className="money">{report.gross_profit_or_loss}</th></tr>
        <Section title="Indirect Income" rows={report.indirect_income_rows} subtotal={report.indirect_income_subtotal} />
        <Section title="Indirect Expenses" rows={report.indirect_expense_rows} subtotal={report.indirect_expense_subtotal} />
        <tr className="ot-report-result-row"><th colSpan={5}>Net {report.net_result_type}</th><th className="money">{report.net_profit_or_loss}</th></tr>
      </tbody></table>}
    </div>
  </section>;
}
