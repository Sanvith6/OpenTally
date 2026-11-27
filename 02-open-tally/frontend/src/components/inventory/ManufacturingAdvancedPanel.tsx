import { useState } from 'react';
import { computeNetUnitCost, expandMultilevelBom, factoryError } from '../../services/factoryApi';

export function ManufacturingAdvancedPanel({ companyId, bomId }: { companyId: string; bomId: string }) {
  const [quantity, setQuantity] = useState('1');
  const [leaves, setLeaves] = useState<[string, string][] | null>(null);
  const [raw, setRaw] = useState('0');
  const [overheads, setOverheads] = useState('0');
  const [recovery, setRecovery] = useState('0');
  const [output, setOutput] = useState('1');
  const [cost, setCost] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function explode() {
    setBusy(true); setError(''); setLeaves(null);
    try { setLeaves(await expandMultilevelBom(companyId, bomId, quantity)); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  async function calculate() {
    setBusy(true); setError(''); setCost(null);
    try { setCost(await computeNetUnitCost(raw, overheads, recovery, output)); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <section className="ot-report-card" data-testid="manufacturing-advanced-panel"><h3>Multi-Level BOM Explosion Preview</h3><p>Expand the selected BOM to leaf components; cyclic dependencies are rejected by the Rust engine.</p>
    <div className="ot-report-toolbar"><label className="ot-report-field">Explosion target quantity<input inputMode="decimal" value={quantity} onChange={(e) => setQuantity(e.target.value)} /></label><button className="ot-report-action" disabled={busy || !bomId} onClick={() => void explode()}>Preview Multi-Level BOM</button></div>
    {leaves && <table className="ot-report-table"><thead><tr><th>Leaf Stock Item ID</th><th className="money">Required Quantity</th></tr></thead><tbody>{leaves.map(([id, qty]) => <tr key={id}><td>{id}</td><td className="money">{qty}</td></tr>)}</tbody></table>}
    <h3>Scrap / By-Product Recovery</h3><p>Preview net unit cost after recovery. This calculator does not post a stock or accounting voucher.</p>
    <div className="ot-report-toolbar"><label className="ot-report-field">Raw cost<input inputMode="decimal" value={raw} onChange={(e) => setRaw(e.target.value)} /></label><label className="ot-report-field">Overhead cost<input inputMode="decimal" value={overheads} onChange={(e) => setOverheads(e.target.value)} /></label><label className="ot-report-field">By-product recovery value<input inputMode="decimal" value={recovery} onChange={(e) => setRecovery(e.target.value)} /></label><label className="ot-report-field">Produced quantity for recovery<input inputMode="decimal" value={output} onChange={(e) => setOutput(e.target.value)} /></label><button disabled={busy} className="ot-report-action" onClick={() => void calculate()}>Calculate Net Unit Cost</button></div>
    {cost !== null && <p role="status">Net unit cost: <strong>{cost}</strong></p>}{error && <p className="ot-report-error" role="alert">{error}</p>}
  </section>;
}
