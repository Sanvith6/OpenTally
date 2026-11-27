import { useCallback, useEffect, useState } from 'react';
import { Company, Godown, Ledger, StockItem } from '../../types';
import { listGodowns, listLedgers, listStockItems } from '../../services/api';
import { Itc04Report, JobWorkChallan, downloadItc04Json, factoryError, loadItc04Report, saveFactoryJson } from '../../services/factoryApi';
import { JobWorkDispatchForm, JobWorkReceiptForm } from './JobWorkForms';
import '../reports/Reports.css';

interface Props { activeCompany: Company | null; onBack: () => void }
type Tab = 'out' | 'in' | 'itc04';

export function JobWorkView({ activeCompany, onBack }: Props) {
  const [tab, setTab] = useState<Tab>('out');
  const [godowns, setGodowns] = useState<Godown[]>([]);
  const [items, setItems] = useState<StockItem[]>([]);
  const [ledgers, setLedgers] = useState<Ledger[]>([]);
  const [from, setFrom] = useState(activeCompany?.financial_year_begins || '');
  const [to, setTo] = useState(new Date().toISOString().slice(0, 10));
  const [report, setReport] = useState<Itc04Report | null>(null);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    if (!activeCompany || !from || !to) return;
    setBusy(true); setError('');
    try {
      const [warehouseRows, itemRows, ledgerRows, reportRows] = await Promise.all([
        listGodowns(activeCompany.id), listStockItems(activeCompany.id), listLedgers(activeCompany.id), loadItc04Report(activeCompany.id, from, to),
      ]);
      setGodowns(warehouseRows); setItems(itemRows); setLedgers(ledgerRows); setReport(reportRows);
    } catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }, [activeCompany, from, to]);
  useEffect(() => { void load(); }, [load]);
  async function saved(challan: JobWorkChallan) {
    setMessage(`Challan ${challan.challan_number} saved. Statutory due date: ${challan.statutory_due_date}.`);
    await load();
  }
  async function exportJson() {
    if (!activeCompany) return;
    try { saveFactoryJson(`opentally-itc04-${from}-${to}.json`, await downloadItc04Json(activeCompany.id, from, to)); }
    catch (cause) { setError(factoryError(cause)); }
  }
  if (!activeCompany) return <section className="ot-report-screen"><h2>Job-Work &amp; Form GST ITC-04</h2><p>Select a company first.</p></section>;
  const props = { companyId: activeCompany.id, godowns, items, ledgers, onSaved: saved };
  return <section className="ot-report-screen" data-testid="job-work-view">
    <div className="ot-report-header"><div><h2>Job-Work &amp; Form GST ITC-04</h2><p>CGST Section 143 · Input: 365 days · Capital Goods: 1095 days. Due dates and overdue flags come from the accounting engine.</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-toolbar"><label className="ot-report-field">Job-work report from<input type="date" value={from} onChange={(e) => setFrom(e.target.value)} /></label><label className="ot-report-field">Job-work report to<input type="date" value={to} onChange={(e) => setTo(e.target.value)} /></label><button className="ot-report-action" onClick={() => void load()} disabled={busy}>Refresh Job-Work</button></div>
    <div className="ot-report-toolbar" role="tablist" aria-label="Job-work workflow">
      {([['out', 'Material Out (Dispatch)'], ['in', 'Material In (Receipt)'], ['itc04', 'Form GST ITC-04']] as const).map(([value, label]) => <button key={value} className="ot-report-action" aria-pressed={tab === value} onClick={() => setTab(value)}>{label}</button>)}
    </div>
    {error && <p className="ot-report-error" role="alert">{error}</p>}{message && <p role="status">{message}</p>}
    {tab === 'out' && <JobWorkDispatchForm {...props} />}
    {tab === 'in' && <JobWorkReceiptForm {...props} outgoing={report?.table_4 || []} />}
    {tab === 'itc04' && <div className="ot-report-card"><button className="ot-report-action" disabled={busy || !report} onClick={() => void exportJson()}>Export ITC-04 JSON</button>
      <h3>Table 4 · Material Out</h3><table className="ot-report-table"><thead><tr><th>Challan / Date</th><th>Job Worker</th><th>Item / Category</th><th className="money">Dispatched Qty</th><th className="money">Taxable Value</th><th>Section 143 Due / Status</th></tr></thead><tbody>{report?.table_4.map((row, index) => <tr key={`${row.challan_id}-${index}`}><td>{row.challan_number}<br />{row.challan_date}</td><td>{row.job_worker_name}</td><td>{row.stock_item_name}<br />{row.goods_category}</td><td className="money">{row.dispatched_qty}</td><td className="money">{row.taxable_value}</td><td>{row.statutory_due_date}<br /><strong>{row.is_overdue ? 'OVERDUE' : 'Within statutory period'}</strong></td></tr>)}</tbody></table>{!report?.table_4.length && <p className="ot-report-empty">No material dispatches for this period.</p>}
      <h3>Table 5A · Material Returned</h3><table className="ot-report-table"><thead><tr><th>In Challan / Date</th><th>Linked Out Challan</th><th className="money">Received Qty</th><th className="money">Scrap Qty</th><th className="money">Pending Qty</th></tr></thead><tbody>{report?.table_5a.map((row, index) => <tr key={`${row.in_challan_number}-${index}`}><td>{row.in_challan_number}<br />{row.in_challan_date}</td><td>{row.original_out_challan_number}</td><td className="money">{row.received_qty}</td><td className="money">{row.scrap_qty}</td><td className="money">{row.pending_qty}</td></tr>)}</tbody></table>{!report?.table_5a.length && <p className="ot-report-empty">No material returns for this period.</p>}
    </div>}
  </section>;
}
