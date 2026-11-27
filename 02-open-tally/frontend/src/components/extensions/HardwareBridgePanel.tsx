import { useState } from 'react';
import { WeighbridgeReading, factoryError, lookupStockBarcode, parseFactoryWeighbridge } from '../../services/factoryApi';

export function HardwareBridgePanel({ companyId }: { companyId: string }) {
  const [frame, setFrame] = useState('ST,GS,+001234.567 KG');
  const [tare, setTare] = useState('0');
  const [barcode, setBarcode] = useState('');
  const [reading, setReading] = useState<WeighbridgeReading | null>(null);
  const [lookup, setLookup] = useState<[string, string] | null>(null);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function parse() {
    setBusy(true); setError(''); setReading(null);
    try { setReading(await parseFactoryWeighbridge(frame, tare)); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  async function find() {
    setBusy(true); setError(''); setLookup(null); setSearched(false);
    try { setLookup(await lookupStockBarcode(companyId, barcode)); setSearched(true); }
    catch (cause) { setError(factoryError(cause)); }
    finally { setBusy(false); }
  }
  return <div data-testid="hardware-bridge-panel"><h3>Weighbridge / Barcode Helper</h3><p>Test an independently defined text frame or look up an item by barcode. This helper does not open a serial port or pretend that hardware is connected.</p>
    <div className="ot-report-toolbar"><label className="ot-report-field">Weighbridge frame<input value={frame} onChange={(e) => setFrame(e.target.value)} /></label><label className="ot-report-field">Tare weight<input inputMode="decimal" value={tare} onChange={(e) => setTare(e.target.value)} /></label><button className="ot-report-action" disabled={busy || !frame.trim()} onClick={() => void parse()}>Parse Test Frame</button></div>
    {reading && <dl className="ot-report-summary"><div><dt>Gross</dt><dd>{reading.gross_weight} {reading.unit}</dd></div><div><dt>Tare</dt><dd>{reading.tare_weight} {reading.unit}</dd></div><div><dt>Net Weight</dt><dd>{reading.net_weight} {reading.unit}</dd></div></dl>}
    <div className="ot-report-toolbar"><label className="ot-report-field">Barcode / SKU<input value={barcode} onChange={(e) => setBarcode(e.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); void find(); } }} /></label><button className="ot-report-action" disabled={busy || !barcode.trim()} onClick={() => void find()}>Lookup Barcode</button></div>
    {searched && <p role="status">{lookup ? `Stock item: ${lookup[1]} (${lookup[0]})` : 'No matching stock item for this company.'}</p>}{error && <p className="ot-report-error" role="alert">{error}</p>}
  </div>;
}
