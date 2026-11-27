import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, Company, ProfitAndLossLedgerRow, ProfitAndLossReport } from '../../types';
import { exportProfitAndLossPdf, getProfitAndLoss, listAccountGroups } from '../../services/api';
import { addDecimals } from '../../utils/decimal';
import { PdfExportAction } from './PdfExportAction';
import { TFormatRow, TFormatStatement } from './TFormatStatement';
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

function section(title: string, rows: ProfitAndLossLedgerRow[], subtotal: string): TFormatRow[] {
  return [{ label: title, kind: 'section' },
    ...rows.map((row) => ({ label: row.ledger_name, group: row.account_group_name, amount: row.contribution })),
    { label: `${title} subtotal`, amount: subtotal, kind: 'subtotal' }];
}

function ProfitAndLossStatement({ report }: { report: ProfitAndLossReport }) {
  const expenses = [...section('Direct Expenses', report.direct_expense_rows, report.direct_expense_subtotal),
    ...section('Indirect Expenses', report.indirect_expense_rows, report.indirect_expense_subtotal)];
  const income = [...section('Direct Income', report.direct_income_rows, report.direct_income_subtotal),
    ...section('Indirect Income', report.indirect_income_rows, report.indirect_income_subtotal)];
  let expenseTotal = addDecimals(report.direct_expense_subtotal, report.indirect_expense_subtotal);
  let incomeTotal = addDecimals(report.direct_income_subtotal, report.indirect_income_subtotal);
  if (report.net_result_type === 'Profit') {
    expenses.push({ label: 'Net Profit', amount: report.net_profit_or_loss, kind: 'balance' });
    expenseTotal = addDecimals(expenseTotal, report.net_profit_or_loss);
  } else if (report.net_result_type === 'Loss') {
    income.push({ label: 'Net Loss', amount: report.net_profit_or_loss, kind: 'balance' });
    incomeTotal = addDecimals(incomeTotal, report.net_profit_or_loss);
  }
  return <TFormatStatement name="Profit & Loss T-format" leftTitle="Expenses" rightTitle="Income"
    left={expenses} right={income} leftTotal={expenseTotal} rightTotal={incomeTotal} />;
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
      <PdfExportAction filename={`profit-and-loss-${from}-${to}.pdf`} disabled={!activeCompany || loading || !report}
        loadPdf={() => exportProfitAndLossPdf(from, to, activeCompany?.id)} />
    </div>
    {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Gross {report.gross_result_type}</span><strong>{report.gross_profit_or_loss}</strong></div><div className="ot-report-summary-card"><span>Net {report.net_result_type}</span><strong>{report.net_profit_or_loss}</strong></div><div className="ot-report-summary-card"><span>Ledgers</span><strong>{report.ledger_count}</strong></div></div>}
    <div className="ot-report-card">
      {loading ? <div className="ot-report-loading">Loading Profit &amp; Loss…</div> : error ? <div className="ot-report-error">{error}</div> : !report ? <div className="ot-report-empty">No income or expense activity for this period.</div> : <ProfitAndLossStatement report={report} />}
    </div>
  </section>;
}
