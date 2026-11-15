import { useCallback, useEffect, useState } from 'react';
import { AccountGroup, CashFlowReport, CashFlowRow, Company, Ledger } from '../../types';
import { getCashFlow, listAccountGroups, listLedgers } from '../../services/api';
import './Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void; }

function csvValue(value: string): string {
  const safe = /^[=+\-@]/.test(value) ? `'${value}` : value;
  return `"${safe.replace(/"/g, '""')}"`;
}

function downloadCsv(report: CashFlowReport): void {
  const rows: string[][] = [['Cash Flow Statement', report.company_id], ['Period', `${report.date_from} to ${report.date_to}`], [], ['Activity', 'Cash/bank ledger', 'Voucher', 'Date', 'Description', 'Amount']];
  const add = (title: string, items: CashFlowRow[], subtotal: string) => {
    rows.push([title, '', '', '', '', '']);
    items.forEach((row) => rows.push([title, row.ledger_name, row.voucher_number, row.date, row.description || '', row.amount]));
    rows.push([`${title} subtotal`, '', '', '', '', subtotal]);
  };
  add('Operating Activities', report.operating_rows, report.operating_subtotal);
  add('Investing Activities', report.investing_rows, report.investing_subtotal);
  add('Financing Activities', report.financing_rows, report.financing_subtotal);
  rows.push(['Opening cash and bank', '', '', '', '', report.opening_cash_balance], ['Net change in cash', '', '', '', '', report.net_change_in_cash], ['Closing cash and bank', '', '', '', '', report.closing_cash_balance], ['Actual closing ledger balance', '', '', '', '', report.actual_closing_cash_balance], ['Reconciliation difference', '', '', '', report.is_reconciled ? 'Reconciled' : 'Not reconciled', report.reconciliation_difference]);
  const blob = new Blob([rows.map((row) => row.map(csvValue).join(',')).join('\r\n')], { type: 'text/csv;charset=utf-8' });
  const url = URL.createObjectURL(blob); const anchor = document.createElement('a'); anchor.href = url; anchor.download = `cash-flow-${report.date_from}-${report.date_to}.csv`; anchor.click(); URL.revokeObjectURL(url);
}

function Section({ title, rows, subtotal }: { title: string; rows: CashFlowRow[]; subtotal: string }) {
  return <><tr className="ot-report-section-row"><th colSpan={3}>{title}</th><th colSpan={3}></th></tr>{rows.map((row) => <tr key={`${row.voucher_id}-${row.ledger_id}`}><td></td><td>{row.ledger_name}</td><td>{row.voucher_number}</td><td>{row.date}</td><td>{row.description || ''}</td><td className="money">{row.amount}</td></tr>)}<tr className="ot-report-subtotal"><td colSpan={5}>{title} subtotal</td><td className="money">{subtotal}</td></tr></>;
}

export function CashFlowView({ activeCompany, onBack }: Props) {
  const [from, setFrom] = useState(activeCompany?.financial_year_begins || ''); const [to, setTo] = useState(activeCompany?.financial_year_begins || '');
  const [cashLedgerId, setCashLedgerId] = useState(''); const [activity, setActivity] = useState(''); const [groups, setGroups] = useState<AccountGroup[]>([]); const [ledgers, setLedgers] = useState<Ledger[]>([]);
  const [report, setReport] = useState<CashFlowReport | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState<string | null>(null);
  const load = useCallback(async () => { if (!activeCompany || !from || !to) return; setLoading(true); setError(null); try { setReport(await getCashFlow({ period: { date_from: from, date_to: to }, cash_ledger_id: cashLedgerId || null, activity_filter: activity || null })); } catch (err) { setError(err instanceof Error ? err.message : String(err)); } finally { setLoading(false); } }, [activeCompany, activity, cashLedgerId, from, to]);
  useEffect(() => { Promise.all([listAccountGroups(), listLedgers()]).then(([nextGroups, nextLedgers]) => { setGroups(nextGroups); setLedgers(nextLedgers); }).catch((err: unknown) => setError(err instanceof Error ? err.message : String(err))); }, []);
  useEffect(() => { void load(); }, [load]);
  const cashLedgers = ledgers.filter((ledger) => ledger.ledger_classification === 'cash' || groups.find((group) => group.id === ledger.group_id && (group.name === 'Cash-in-Hand' || group.name === 'Bank Accounts')));
  return <section className="ot-report-screen"><div className="ot-report-header"><div><h2>Cash Flow Statement</h2><p>Cash and bank movement by operating, investing, and financing activity.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-toolbar"><label className="ot-report-field">From<input type="date" value={from} onChange={(event) => setFrom(event.target.value)} /></label><label className="ot-report-field">To<input type="date" value={to} onChange={(event) => setTo(event.target.value)} /></label><label className="ot-report-field">Cash/bank ledger<select value={cashLedgerId} onChange={(event) => setCashLedgerId(event.target.value)}><option value="">All cash and bank</option>{cashLedgers.map((ledger) => <option key={ledger.id} value={ledger.id}>{ledger.name}</option>)}</select></label><label className="ot-report-field">Activity<select value={activity} onChange={(event) => setActivity(event.target.value)}><option value="">All activities</option><option value="operating">Operating</option><option value="investing">Investing</option><option value="financing">Financing</option></select></label><button className="ot-report-action" onClick={() => void load()} disabled={loading || !activeCompany}>Refresh</button><button className="ot-report-action" onClick={() => report && downloadCsv(report)} disabled={!report}>Export CSV</button></div>
    {report && <div className="ot-report-summary"><div className="ot-report-summary-card"><span>Opening cash</span><strong>{report.opening_cash_balance}</strong></div><div className="ot-report-summary-card"><span>Net change</span><strong>{report.net_change_in_cash}</strong></div><div className="ot-report-summary-card"><span>Closing cash</span><strong>{report.closing_cash_balance}</strong></div><div className="ot-report-summary-card"><span>Reconciliation</span><strong>{report.is_reconciled ? 'Reconciled' : 'Difference'}</strong></div></div>}
    <div className="ot-report-card">{loading ? <div className="ot-report-loading">Loading Cash Flow…</div> : error ? <div className="ot-report-error">{error}</div> : !report || report.ledger_count === 0 ? <div className="ot-report-empty">No cash or bank ledgers for this selection.</div> : <table className="ot-report-table"><thead><tr><th></th><th>Cash/bank ledger</th><th>Voucher</th><th>Date</th><th>Description</th><th className="money">Amount</th></tr></thead><tbody><Section title="Operating Activities" rows={report.operating_rows} subtotal={report.operating_subtotal} /><Section title="Investing Activities" rows={report.investing_rows} subtotal={report.investing_subtotal} /><Section title="Financing Activities" rows={report.financing_rows} subtotal={report.financing_subtotal} /><tr className="ot-report-subtotal"><th colSpan={5}>Opening cash and bank</th><th className="money">{report.opening_cash_balance}</th></tr><tr className="ot-report-result-row"><th colSpan={5}>Net change in cash</th><th className="money">{report.net_change_in_cash}</th></tr><tr className="ot-report-result-row"><th colSpan={5}>Closing cash and bank</th><th className="money">{report.closing_cash_balance}</th></tr><tr className="ot-report-result-row"><th colSpan={5}>Reconciliation ({report.is_reconciled ? 'Reconciled' : 'Not reconciled'})</th><th className="money">{report.reconciliation_difference}</th></tr></tbody></table>}</div>
  </section>;
}
