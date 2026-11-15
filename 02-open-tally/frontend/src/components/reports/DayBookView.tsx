import { Fragment, useCallback, useEffect, useState } from 'react';
import { Company, DayBookEntry, ReportPage, VoucherType } from '../../types';
import { exportReportPreview, getDayBook, listVoucherTypes } from '../../services/api';
import './Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void; }

const today = () => new Date().toISOString().slice(0, 10);

export function DayBookView({ activeCompany, onBack }: Props) {
  const initialDate = activeCompany?.financial_year_begins || today();
  const [from, setFrom] = useState(initialDate);
  const [to, setTo] = useState(today());
  const [voucherType, setVoucherType] = useState('');
  const [search, setSearch] = useState('');
  const [types, setTypes] = useState<VoucherType[]>([]);
  const [result, setResult] = useState<ReportPage<DayBookEntry> | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

  useEffect(() => { listVoucherTypes().then(setTypes).catch(() => setTypes([])); }, []);

  const load = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true); setError(null);
    try {
      const data = await getDayBook({
        filter: { period: { date_from: from, date_to: to }, ledger_id: null },
        voucher_type_id: voucherType || null,
        voucher_number_query: search.trim() || null,
        page: { page, page_size: 25 },
      });
      setResult(data); setExpanded(new Set());
    } catch (err) { setError(err instanceof Error ? err.message : String(err)); }
    finally { setLoading(false); }
  }, [activeCompany, from, to, voucherType, search, page]);

  useEffect(() => { void load(); }, [load]);

  const exportCsv = async () => {
    setExporting(true);
    try {
      const result = await exportReportPreview({ voucher_type_id: voucherType || null, status: 'all', date_from: from, date_to: to, search_query: search.trim() || null });
      const url = URL.createObjectURL(new Blob([result.content], { type: 'text/csv;charset=utf-8' }));
      const link = document.createElement('a'); link.href = url; link.download = `day-book-${from}-${to}.csv`; link.click(); URL.revokeObjectURL(url);
    } catch (err) { setError(err instanceof Error ? err.message : String(err)); }
    finally { setExporting(false); }
  };

  return <section className="ot-report-screen">
    <header className="ot-report-header"><div><h2>Day Book</h2><p>Chronological posted voucher activity for {activeCompany?.name || 'the active company'}.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></header>
    <div className="ot-report-toolbar">
      <label className="ot-report-field">From date<input type="date" value={from} onChange={(e) => { setFrom(e.target.value); setPage(1); }} /></label>
      <label className="ot-report-field">To date<input type="date" value={to} onChange={(e) => { setTo(e.target.value); setPage(1); }} /></label>
      <label className="ot-report-field">Voucher type<select value={voucherType} onChange={(e) => { setVoucherType(e.target.value); setPage(1); }}><option value="">All types</option>{types.map((type) => <option key={type.id} value={type.id}>{type.name}</option>)}</select></label>
      <label className="ot-report-field">Voucher number<input value={search} onChange={(e) => { setSearch(e.target.value); setPage(1); }} placeholder="Search" /></label>
      <button className="ot-report-action" onClick={() => void load()} disabled={loading}>Refresh</button><button className="ot-report-action" onClick={() => void exportCsv()} disabled={exporting}>Export CSV</button>
    </div>
    {loading && <div className="ot-report-loading">Loading day book…</div>}
    {error && <div className="ot-report-error">{error}</div>}
    {!loading && !error && result && result.items.length === 0 && <div className="ot-report-empty">No posted transactions in this period.</div>}
    {!loading && !error && result && result.items.length > 0 && <>
      <div className="ot-report-card"><table className="ot-report-table"><thead><tr><th></th><th>Date</th><th>Voucher</th><th>Type</th><th>Narration</th><th className="money">Debit</th><th className="money">Credit</th><th className="money">Total</th></tr></thead><tbody>{result.items.map((entry) => <Fragment key={entry.voucher_id}>
        <tr key={entry.voucher_id}><td className="expand" onClick={() => setExpanded((old) => { const next = new Set(old); next.has(entry.voucher_id) ? next.delete(entry.voucher_id) : next.add(entry.voucher_id); return next; })}>{expanded.has(entry.voucher_id) ? '▾' : '▸'}</td><td>{entry.date}</td><td>{entry.voucher_number}</td><td>{entry.voucher_type}</td><td>{entry.narration || '—'}</td><td className="money">{entry.debit_total}</td><td className="money">{entry.credit_total}</td><td className="money">{entry.voucher_total}</td></tr>
        {expanded.has(entry.voucher_id) && <tr className="ot-report-detail" key={`${entry.voucher_id}-detail`}><td colSpan={8}><table><tbody>{entry.lines.map((line) => <tr key={`${entry.voucher_id}-${line.ledger_id}`}><td>{line.ledger_name}</td><td>{line.narration || ''}</td><td className="money">{line.debit}</td><td className="money">{line.credit}</td></tr>)}</tbody></table></td></tr>}
      </Fragment>)}</tbody></table></div>
      <div className="ot-report-pagination"><span>{result.total} voucher(s)</span><span><button disabled={page <= 1} onClick={() => setPage((value) => value - 1)}>Previous</button> <button disabled={page >= result.total_pages} onClick={() => setPage((value) => value + 1)}>Next</button></span></div>
    </>}
  </section>;
}
