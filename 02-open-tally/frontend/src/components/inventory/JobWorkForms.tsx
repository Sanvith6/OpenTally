import { FormEvent, useState } from 'react';
import { Godown, Ledger, StockItem } from '../../types';
import {
  DispatchJobWorkRequest, ReceiveJobWorkRequest, Itc04OutRow, JobWorkChallan,
  dispatchJobWorkMaterialOut, receiveJobWorkMaterialIn, factoryError,
} from '../../services/factoryApi';

interface FormProps {
  companyId: string; godowns: Godown[]; items: StockItem[]; ledgers: Ledger[];
  onSaved: (challan: JobWorkChallan) => Promise<void>;
}

export function JobWorkDispatchForm({ companyId, godowns, items, ledgers, onSaved }: FormProps) {
  const [request, setRequest] = useState<DispatchJobWorkRequest>({
    challan_number: '', challan_date: new Date().toISOString().slice(0, 10),
    job_worker_ledger_id: '', source_godown_id: '', external_godown_id: '', goods_category: 'Input', narration: '',
    lines: [{ stock_item_id: '', qty: '1', unit_cost: '0' }],
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const set = <K extends keyof DispatchJobWorkRequest>(key: K, value: DispatchJobWorkRequest[K]) => setRequest((old) => ({ ...old, [key]: value }));
  async function submit(event: FormEvent) {
    event.preventDefault(); setBusy(true); setError('');
    try { await onSaved(await dispatchJobWorkMaterialOut(companyId, request)); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <form onSubmit={submit} data-testid="job-work-dispatch">
    <h3>Material Out · Dispatch</h3>
    <p>One stock item per challan. Record separate dispatch challans for additional materials so each return can be linked unambiguously.</p>
    <div className="ot-report-toolbar">
      <label className="ot-report-field">Out challan number<input required value={request.challan_number} onChange={(e) => set('challan_number', e.target.value)} /></label>
      <label className="ot-report-field">Out challan date<input required type="date" value={request.challan_date} onChange={(e) => set('challan_date', e.target.value)} /></label>
      <label className="ot-report-field">Job-worker party<select required value={request.job_worker_ledger_id} onChange={(e) => set('job_worker_ledger_id', e.target.value)}><option value="">Select party ledger</option>{ledgers.map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
      <label className="ot-report-field">Source godown<select required value={request.source_godown_id} onChange={(e) => set('source_godown_id', e.target.value)}><option value="">Select internal godown</option>{godowns.filter((row) => !row.is_external && row.is_active).map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
      <label className="ot-report-field">External job-worker godown<select required value={request.external_godown_id} onChange={(e) => set('external_godown_id', e.target.value)}><option value="">Select external godown</option>{godowns.filter((row) => Boolean(row.is_external) && row.is_active).map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
      <label className="ot-report-field">Goods category<select value={request.goods_category} onChange={(e) => set('goods_category', e.target.value as 'Input' | 'CapitalGood')}><option value="Input">Input · 365 days</option><option value="CapitalGood">Capital Good · 1095 days</option></select></label>
    </div>
    <table className="ot-report-table"><thead><tr><th>Item</th><th className="money">Quantity</th><th className="money">Unit cost</th><th>Remove</th></tr></thead><tbody>{request.lines.map((line, index) => <tr key={index}>
      <td><select required aria-label={`Dispatch item ${index + 1}`} value={line.stock_item_id} onChange={(e) => set('lines', request.lines.map((old, i) => i === index ? { ...old, stock_item_id: e.target.value } : old))}><option value="">Select item</option>{items.filter((row) => row.is_active).map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></td>
      <td><input required aria-label={`Dispatch quantity ${index + 1}`} inputMode="decimal" value={line.qty} onChange={(e) => set('lines', request.lines.map((old, i) => i === index ? { ...old, qty: e.target.value } : old))} /></td>
      <td><input required aria-label={`Dispatch unit cost ${index + 1}`} inputMode="decimal" value={line.unit_cost} onChange={(e) => set('lines', request.lines.map((old, i) => i === index ? { ...old, unit_cost: e.target.value } : old))} /></td>
      <td><button type="button" disabled={request.lines.length === 1} onClick={() => set('lines', request.lines.filter((_, i) => i !== index))}>Remove</button></td>
    </tr>)}</tbody></table>
    <div className="ot-report-toolbar"><label className="ot-report-field">Dispatch narration<input value={request.narration || ''} onChange={(e) => set('narration', e.target.value)} /></label><button disabled={busy} className="ot-report-action" type="submit">Dispatch Material Out</button></div>
    {error && <p className="ot-report-error" role="alert">{error}</p>}
  </form>;
}

export function JobWorkReceiptForm({ companyId, godowns, items, onSaved, outgoing }: FormProps & { outgoing: Itc04OutRow[] }) {
  const [request, setRequest] = useState<ReceiveJobWorkRequest>({ challan_number: '', challan_date: new Date().toISOString().slice(0, 10), linked_out_challan_id: '', destination_godown_id: '', finished_stock_item_id: '', finished_qty: '1', returned_raw_qty: '0', scrap_qty: '0', job_work_charges: '0', scrap_recovery_value: '0', narration: '' });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const set = (key: keyof ReceiveJobWorkRequest, value: string) => setRequest((old) => ({ ...old, [key]: value }));
  const uniqueOutgoing = outgoing.filter((row, index) => outgoing.findIndex((other) => other.challan_id === row.challan_id) === index);
  async function submit(event: FormEvent) {
    event.preventDefault(); setBusy(true); setError('');
    try { await onSaved(await receiveJobWorkMaterialIn(companyId, request)); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <form onSubmit={submit} data-testid="job-work-receipt"><h3>Material In · Receipt</h3><div className="ot-report-toolbar">
    <label className="ot-report-field">In challan number<input required value={request.challan_number} onChange={(e) => set('challan_number', e.target.value)} /></label>
    <label className="ot-report-field">In challan date<input required type="date" value={request.challan_date} onChange={(e) => set('challan_date', e.target.value)} /></label>
    <label className="ot-report-field">Linked out challan<select required value={request.linked_out_challan_id} onChange={(e) => set('linked_out_challan_id', e.target.value)}><option value="">Select dispatch</option>{uniqueOutgoing.map((row) => <option key={row.challan_id} value={row.challan_id}>{row.challan_number} · {row.job_worker_name}</option>)}</select></label>
    <label className="ot-report-field">Destination godown<select required value={request.destination_godown_id} onChange={(e) => set('destination_godown_id', e.target.value)}><option value="">Select internal godown</option>{godowns.filter((row) => !row.is_external && row.is_active).map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
    <label className="ot-report-field">Finished item<select required value={request.finished_stock_item_id} onChange={(e) => set('finished_stock_item_id', e.target.value)}><option value="">Select finished item</option>{items.filter((row) => row.is_active).map((row) => <option key={row.id} value={row.id}>{row.name}</option>)}</select></label>
    {([['finished_qty', 'Finished quantity'], ['returned_raw_qty', 'Returned raw quantity'], ['scrap_qty', 'Scrap quantity'], ['job_work_charges', 'Job-work charges'], ['scrap_recovery_value', 'Scrap recovery value']] as const).map(([key, label]) => <label key={key} className="ot-report-field">{label}<input required inputMode="decimal" value={request[key]} onChange={(e) => set(key, e.target.value)} /></label>)}
    <label className="ot-report-field">Receipt narration<input value={request.narration || ''} onChange={(e) => set('narration', e.target.value)} /></label>
    <button className="ot-report-action" type="submit" disabled={busy || !uniqueOutgoing.length}>Receive Material In</button>
  </div>{!uniqueOutgoing.length && <p className="ot-report-empty">No dispatch challans in this reporting period. Extend the date range or record Material Out first.</p>}{error && <p className="ot-report-error" role="alert">{error}</p>}</form>;
}
