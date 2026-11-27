import React, { useEffect, useState } from 'react';
import { Company, PriceLevelDto, PriceListEntryDto, StockItem } from '../../types';
import {
  listPriceLevels,
  createPriceLevel,
  listPriceListEntries,
  upsertPriceListEntry,
  listStockItems,
} from '../../services/api';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function PriceLevelsView({ activeCompany, onBack }: Props) {
  const [levels, setLevels] = useState<PriceLevelDto[]>([]);
  const [entries, setEntries] = useState<PriceListEntryDto[]>([]);
  const [items, setItems] = useState<StockItem[]>([]);
  const [newLevelName, setNewLevelName] = useState('');
  const [selectedLevelId, setSelectedLevelId] = useState('');
  const [itemId, setItemId] = useState('');
  const [effectiveFrom, setEffectiveFrom] = useState('2026-04-01');
  const [minQty, setMinQty] = useState('1');
  const [maxQty, setMaxQty] = useState('');
  const [unitRate, setUnitRate] = useState('100.00');
  const [discountPct, setDiscountPct] = useState('0.00');
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = async () => {
    if (!activeCompany) return;
    try {
      const [lv, en, it] = await Promise.all([
        listPriceLevels(),
        listPriceListEntries(),
        listStockItems(),
      ]);
      setLevels(lv);
      setEntries(en);
      setItems(it);
      if (lv.length > 0 && !selectedLevelId) setSelectedLevelId(lv[0].id);
      if (it.length > 0 && !itemId) setItemId(it[0].id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  useEffect(() => {
    void load();
  }, [activeCompany]);

  const handleCreateLevel = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const created = await createPriceLevel(newLevelName);
      setLevels((prev) => [...prev, created]);
      setSelectedLevelId(created.id);
      setNewLevelName('');
      setNotice(`Price level '${created.name}' created.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSaveEntry = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const saved = await upsertPriceListEntry(
        selectedLevelId,
        itemId,
        effectiveFrom,
        minQty,
        maxQty.trim() || null,
        unitRate,
        discountPct
      );
      setEntries((prev) => [saved, ...prev]);
      setNotice('Price slab saved.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <section className="ot-report-screen" data-testid="price-levels-view">
      <header className="ot-report-header">
        <div>
          <h2>Tiered Price Levels &amp; Quantity Slabs</h2>
          <p>{activeCompany ? `${activeCompany.name} · Wholesale, Dealer & Retail Price Lists` : 'Select an active company'}</p>
        </div>
        <button className="ot-report-back" onClick={onBack}>Back to Gateway (Esc)</button>
      </header>
      <div className="ot-report-card">
        {notice && <div className="ot-report-success" role="status">{notice}</div>}
        {error && <div className="ot-report-error" role="alert">{error}</div>}
        <form onSubmit={handleCreateLevel} className="ot-inventory-toolbar" style={{ marginBottom: '1rem' }}>
          <input
            aria-label="New Price Level Name"
            placeholder="e.g. Wholesale, Dealer, Retail"
            value={newLevelName}
            onChange={(e) => setNewLevelName(e.target.value)}
            required
          />
          <button type="submit">Create Price Level</button>
        </form>

        <form onSubmit={handleSaveEntry} className="ot-inventory-fields" style={{ marginBottom: '1.5rem' }}>
          <select aria-label="Price Level" value={selectedLevelId} onChange={(e) => setSelectedLevelId(e.target.value)}>
            <option value="">Select Price Level</option>
            {levels.map((l) => <option key={l.id} value={l.id}>{l.name}</option>)}
          </select>
          <select aria-label="Stock Item" value={itemId} onChange={(e) => setItemId(e.target.value)}>
            <option value="">Select Stock Item</option>
            {items.map((i) => <option key={i.id} value={i.id}>{i.name}</option>)}
          </select>
          <input type="date" aria-label="Effective From" value={effectiveFrom} onChange={(e) => setEffectiveFrom(e.target.value)} />
          <input aria-label="Min Qty" placeholder="Min Qty" value={minQty} onChange={(e) => setMinQty(e.target.value)} />
          <input aria-label="Max Qty" placeholder="Max Qty (Optional)" value={maxQty} onChange={(e) => setMaxQty(e.target.value)} />
          <input aria-label="Unit Rate" placeholder="Unit Rate" value={unitRate} onChange={(e) => setUnitRate(e.target.value)} />
          <input aria-label="Discount %" placeholder="Discount %" value={discountPct} onChange={(e) => setDiscountPct(e.target.value)} />
          <button type="submit" disabled={!selectedLevelId || !itemId}>Save Price Slab</button>
        </form>

        <table className="ot-report-table">
          <thead>
            <tr>
              <th>Price Level</th>
              <th>Item</th>
              <th>Effective From</th>
              <th data-numeric="true">Min Qty</th>
              <th data-numeric="true">Max Qty</th>
              <th data-numeric="true">Rate (₹)</th>
              <th data-numeric="true">Disc %</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((en) => (
              <tr key={en.id}>
                <td>{levels.find((l) => l.id === en.price_level_id)?.name || en.price_level_id}</td>
                <td>{items.find((i) => i.id === en.stock_item_id)?.name || en.stock_item_id}</td>
                <td>{en.effective_from}</td>
                <td data-numeric="true">{en.min_qty}</td>
                <td data-numeric="true">{en.max_qty || '∞'}</td>
                <td data-numeric="true">{en.unit_rate}</td>
                <td data-numeric="true">{en.discount_pct}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
