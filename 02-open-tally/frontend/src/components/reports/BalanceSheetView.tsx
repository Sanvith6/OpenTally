import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, BalanceSheetLedgerRow, BalanceSheetReport, Company } from '../../types';
import { exportBalanceSheetPdf, getBalanceSheet, listAccountGroups } from '../../services/api';
import { subtractDecimals } from '../../utils/decimal';
import { PdfExportAction } from './PdfExportAction';
import { TFormatRow, TFormatStatement } from './TFormatStatement';
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

function section(title: string, rows: BalanceSheetLedgerRow[]): TFormatRow[] {
  if (rows.length === 0) return [];
  return [{ label: title, kind: 'section' }, ...rows.map((row) => ({
    label: row.ledger_name, group: row.account_group_name, amount: row.presentation_amount,
  }))];
}

function BalanceSheetStatement({ report }: { report: BalanceSheetReport }) {
  const liabilities = [...section('Liabilities', report.liability_rows),
    { label: 'Total Liabilities', amount: report.total_liabilities, kind: 'subtotal' as const },
    ...section('Equity / Capital', report.equity_rows),
    { label: `Current-period ${report.current_period_result_type}`, amount: report.current_period_result_type === 'Loss'
      ? subtractDecimals('0.00', report.current_period_profit_or_loss) : report.current_period_profit_or_loss, kind: 'balance' as const },
    { label: 'Total Equity', amount: report.total_equity, kind: 'subtotal' as const }];
  return <TFormatStatement name="Balance Sheet T-format" leftTitle="Liabilities & Capital" rightTitle="Assets"
    left={liabilities} right={section('Assets', report.asset_rows)} leftTotal={report.total_liabilities_and_equity} rightTotal={report.total_assets} />;
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
      <PdfExportAction filename={`balance-sheet-${asOfDate}.pdf`} disabled={!activeCompany || loading || !report}
        loadPdf={() => exportBalanceSheetPdf(asOfDate, activeCompany?.id)} />
    </div>
    {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Total assets</span><strong>{report.total_assets}</strong></div><div className="ot-report-summary-card"><span>Liabilities + equity</span><strong>{report.total_liabilities_and_equity}</strong></div><div className="ot-report-summary-card"><span>Difference</span><strong>{report.balance_difference}</strong></div><div className="ot-report-summary-card"><span>Status</span><strong>{report.is_balanced ? 'Balanced' : 'Not balanced'}</strong></div></div>}
    <div className="ot-report-card">
      {loading ? <div className="ot-report-loading">Loading Balance Sheet…</div> : error ? <div className="ot-report-error">{error}</div> : !report ? <div className="ot-report-empty">No balance-sheet accounts for this selection.</div> : <BalanceSheetStatement report={report} />}
    </div>
  </section>;
}
