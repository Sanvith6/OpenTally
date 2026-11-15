import { useCallback, useEffect, useState } from 'react';
import { Company, Ledger, LedgerReport } from '../../types';
import { getLedgerReport, listLedgers } from '../../services/api';
import './Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void; }
const today = () => new Date().toISOString().slice(0, 10);

export function LedgerReportView({ activeCompany, onBack }: Props) {
  const [ledgers, setLedgers] = useState<Ledger[]>([]);
  const [ledgerId, setLedgerId] = useState('');
  const [from, setFrom] = useState(activeCompany?.financial_year_begins || today());
  const [to, setTo] = useState(today());
  const [page, setPage] = useState(1);
  const [report, setReport] = useState<LedgerReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { listLedgers().then((items) => { setLedgers(items); setLedgerId((current) => current || items[0]?.id || ''); }).catch((err) => setError(err instanceof Error ? err.message : String(err))); }, []);
  const load = useCallback(async () => {
    if (!activeCompany || !ledgerId) return;
    setLoading(true); setError(null);
    try { setReport(await getLedgerReport({ filter: { period: { date_from: from, date_to: to }, ledger_id: ledgerId }, page: { page, page_size: 25 } })); }
    catch (err) { setError(err instanceof Error ? err.message : String(err)); }
    finally { setLoading(false); }
  }, [activeCompany, ledgerId, from, to, page]);
  useEffect(() => { void load(); }, [load]);

  return <section className="ot-report-screen"><header className="ot-report-header"><div><h2>Ledger Report</h2><p>Account movement and running balance for {activeCompany?.name || 'the active company'}.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></header>
    <div className="ot-report-toolbar"><label className="ot-report-field">Ledger<select value={ledgerId} onChange={(e) => { setLedgerId(e.target.value); setPage(1); }}>{ledgers.map((ledger) => <option key={ledger.id} value={ledger.id}>{ledger.name}</option>)}</select></label><label className="ot-report-field">From date<input type="date" value={from} onChange={(e) => { setFrom(e.target.value); setPage(1); }} /></label><label className="ot-report-field">To date<input type="date" value={to} onChange={(e) => { setTo(e.target.value); setPage(1); }} /></label><button className="ot-report-action" onClick={() => void load()} disabled={loading}>Refresh</button></div>
    {loading && <div className="ot-report-loading">Loading ledger report…</div>}{error && <div className="ot-report-error">{error}</div>}
    {!loading && !error && report && <><div className="ot-report-summary"><div className="ot-report-summary-card"><span>Opening balance</span><strong>{report.opening_balance} {report.opening_balance_type}</strong></div><div className="ot-report-summary-card"><span>Closing balance</span><strong>{report.closing_balance} {report.closing_balance_type}</strong></div><div className="ot-report-summary-card"><span>Ledger nature</span><strong>{report.ledger_nature}</strong></div></div><div className="ot-report-card"><table className="ot-report-table"><thead><tr><th>Date</th><th>Voucher</th><th>Type</th><th>Particulars</th><th className="money">Debit</th><th className="money">Credit</th><th className="money">Balance</th></tr></thead><tbody>{report.transactions.items.map((row) => <tr key={`${row.voucher_id}-${row.date}`}><td>{row.date}</td><td>{row.voucher_number}</td><td>{row.voucher_type}</td><td>{row.particulars || '—'}</td><td className="money">{row.debit}</td><td className="money">{row.credit}</td><td className="money">{row.running_balance} {row.running_balance_type}</td></tr>)}</tbody></table>{report.transactions.items.length === 0 && <div className="ot-report-empty">No transactions in this period.</div>}</div><div className="ot-report-pagination"><span>{report.transactions.total} transaction(s)</span><span><button disabled={page <= 1} onClick={() => setPage((value) => value - 1)}>Previous</button> <button disabled={page >= report.transactions.total_pages} onClick={() => setPage((value) => value + 1)}>Next</button></span></div></>}
  </section>;
}
