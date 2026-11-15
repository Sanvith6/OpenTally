import { useCallback, useEffect, useMemo, useState } from 'react';
import { Company, Godown, StockCategory, StockGroup, StockItem, Unit } from '../../types';
import {
  createGodown, createStockCategory, createStockGroup, createStockItem, createUnit,
  deleteGodown, deleteStockCategory, deleteStockGroup, deleteStockItem, deleteUnit,
  listGodowns, listStockCategories, listStockGroups, listStockItems, listUnits,
  setGodownActive, setStockCategoryActive, setStockGroupActive, setStockItemActive, setUnitActive,
  updateGodown, updateStockCategory, updateStockGroup, updateStockItem, updateUnit,
} from '../../services/api';
import '../reports/Reports.css';

type Tab = 'groups' | 'categories' | 'units' | 'items' | 'godowns';
type Row = StockGroup | StockCategory | Unit | StockItem | Godown;
const TABS: { id: Tab; label: string }[] = [{ id: 'groups', label: 'Stock Groups' }, { id: 'categories', label: 'Categories' }, { id: 'units', label: 'Units' }, { id: 'items', label: 'Stock Items' }, { id: 'godowns', label: 'Godowns' }];
const blank = { name: '', description: '', parent_id: '', symbol: '', formal_name: '', decimal_precision: '2', stock_group_id: '', stock_category_id: '', base_unit_id: '', alternate_unit_id: '', sku: '', opening_quantity: '0', opening_rate: '0', opening_value: '0', reorder_level: '0' };

interface Props { activeCompany: Company | null; onBack: () => void; }
function active(row: Row): boolean { return row.is_active; }
function label(row: Row): string { return 'symbol' in row ? `${row.symbol} — ${row.formal_name}` : row.name; }

export function InventoryView({ activeCompany, onBack }: Props) {
  const [tab, setTab] = useState<Tab>('groups');
  const [rows, setRows] = useState<Row[]>([]);
  const [search, setSearch] = useState('');
  const [form, setForm] = useState(blank);
  const [editing, setEditing] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [groups, setGroups] = useState<StockGroup[]>([]);
  const [categories, setCategories] = useState<StockCategory[]>([]);
  const [units, setUnits] = useState<Unit[]>([]);
  const [godowns, setGodowns] = useState<Godown[]>([]);

  const load = useCallback(async () => {
    if (!activeCompany) { setRows([]); return; }
    setLoading(true); setError(null);
    try {
      const [g, c, u, i, d] = await Promise.all([listStockGroups(search), listStockCategories(search), listUnits(search), listStockItems(search), listGodowns(search)]);
      setGroups(g); setCategories(c); setUnits(u); setGodowns(d);
      setRows(tab === 'groups' ? g : tab === 'categories' ? c : tab === 'units' ? u : tab === 'items' ? i : d);
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setLoading(false); }
  }, [activeCompany, search, tab]);
  useEffect(() => { void load(); }, [load]);

  const title = TABS.find((x) => x.id === tab)?.label || 'Inventory';
  const set = (key: keyof typeof blank, value: string) => setForm((current) => ({ ...current, [key]: value }));
  const reset = () => { setEditing(null); setForm(blank); };
  const submit = async () => {
    setError(null);
    try {
      if (tab === 'groups') { const req = { name: form.name, parent_id: form.parent_id || null, description: form.description || null }; editing ? await updateStockGroup(editing, req) : await createStockGroup(req); }
      else if (tab === 'categories') { const req = { name: form.name, description: form.description || null }; editing ? await updateStockCategory(editing, req) : await createStockCategory(req); }
      else if (tab === 'units') { const req = { symbol: form.symbol, formal_name: form.formal_name, decimal_precision: Number(form.decimal_precision), unit_type: 'simple' }; editing ? await updateUnit(editing, req) : await createUnit(req); }
      else if (tab === 'godowns') { const req = { name: form.name, parent_id: form.parent_id || null, description: form.description || null }; editing ? await updateGodown(editing, req) : await createGodown(req); }
      else { const req = { name: form.name, stock_group_id: form.stock_group_id, stock_category_id: form.stock_category_id || null, base_unit_id: form.base_unit_id, alternate_unit_id: form.alternate_unit_id || null, sku: form.sku || null, description: form.description || null, opening_quantity: form.opening_quantity, opening_rate: form.opening_rate, opening_value: form.opening_value, reorder_level: form.reorder_level }; editing ? await updateStockItem(editing, req) : await createStockItem(req); }
      setNotice(`${title} ${editing ? 'updated' : 'created'}.`); reset(); await load();
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };
  const edit = (row: Row) => { setEditing(row.id); if ('symbol' in row) setForm({ ...blank, symbol: row.symbol, formal_name: row.formal_name, decimal_precision: String(row.decimal_precision) }); else if ('stock_group_id' in row) setForm({ ...blank, name: row.name, stock_group_id: row.stock_group_id, stock_category_id: row.stock_category_id || '', base_unit_id: row.base_unit_id, alternate_unit_id: row.alternate_unit_id || '', sku: row.sku || '', description: row.description || '', opening_quantity: row.opening_quantity, opening_rate: row.opening_rate, opening_value: row.opening_value, reorder_level: row.reorder_level }); else if ('parent_id' in row) setForm({ ...blank, name: row.name, parent_id: row.parent_id || '', description: row.description || '' }); else setForm({ ...blank, name: row.name, description: row.description || '' }); };
  const toggle = async (row: Row) => { try { const next = !active(row); if ('stock_group_id' in row) await setStockItemActive(row.id, next); else if ('symbol' in row) await setUnitActive(row.id, next); else if ('parent_id' in row && tab === 'godowns') await setGodownActive(row.id, next); else if (tab === 'categories') await setStockCategoryActive(row.id, next); else await setStockGroupActive(row.id, next); await load(); } catch (e) { setError(e instanceof Error ? e.message : String(e)); } };
  const remove = async (row: Row) => { if (!window.confirm(`Delete ${label(row)}? This is allowed only when no dependent records exist.`)) return; try { if ('stock_group_id' in row) await deleteStockItem(row.id); else if ('symbol' in row) await deleteUnit(row.id); else if (tab === 'godowns') await deleteGodown(row.id); else if (tab === 'categories') await deleteStockCategory(row.id); else await deleteStockGroup(row.id); await load(); } catch (e) { setError(e instanceof Error ? e.message : String(e)); } };
  const parentOptions: (StockGroup | Godown)[] = tab === 'groups' ? groups : godowns;
  const activeRelated = useMemo(() => ({ groups: groups.filter((x) => x.is_active), categories: categories.filter((x) => x.is_active), units: units.filter((x) => x.is_active) }), [groups, categories, units]);

  return <section className="ot-report-screen"><div className="ot-report-header"><div><h2>Inventory Masters</h2><p>{activeCompany ? `${activeCompany.name} · ${title}` : 'Select an active company'}</p></div><button className="ot-report-back" onClick={onBack}>Back to Gateway</button></div>
    <div className="ot-report-card"><div className="ot-inventory-tabs">{TABS.map((item) => <button key={item.id} className={tab === item.id ? 'ot-tab-active' : ''} onClick={() => { setTab(item.id); reset(); }}>{item.label}</button>)}</div>
      <div className="ot-inventory-toolbar"><input aria-label="Search inventory" value={search} onChange={(e) => setSearch(e.target.value)} placeholder={`Search ${title.toLowerCase()}…`} /><button onClick={() => void load()}>Refresh</button></div>
      {notice && <div className="ot-report-success" role="status">{notice}</div>}{error && <div className="ot-report-error" role="alert">{error}</div>}
      <div className="ot-inventory-form"><h3>{editing ? `Edit ${title}` : `New ${title}`}</h3><div className="ot-inventory-fields">{tab === 'units' ? <><input placeholder="Symbol" value={form.symbol} onChange={(e) => set('symbol', e.target.value)} /><input placeholder="Formal name" value={form.formal_name} onChange={(e) => set('formal_name', e.target.value)} /><input type="number" min="0" max="4" placeholder="Precision" value={form.decimal_precision} onChange={(e) => set('decimal_precision', e.target.value)} /></> : <><input placeholder="Name" value={form.name} onChange={(e) => set('name', e.target.value)} />{(tab === 'groups' || tab === 'godowns') && <select value={form.parent_id} onChange={(e) => set('parent_id', e.target.value)}><option value="">No parent</option>{parentOptions.filter((x) => x.id !== editing).map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}</select>}{tab !== 'categories' && tab !== 'items' && <input placeholder="Description" value={form.description} onChange={(e) => set('description', e.target.value)} />}{tab === 'items' && <><select value={form.stock_group_id} onChange={(e) => set('stock_group_id', e.target.value)}><option value="">Stock group</option>{activeRelated.groups.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}</select><select value={form.stock_category_id} onChange={(e) => set('stock_category_id', e.target.value)}><option value="">No category</option>{activeRelated.categories.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}</select><select value={form.base_unit_id} onChange={(e) => set('base_unit_id', e.target.value)}><option value="">Base unit</option>{activeRelated.units.map((x) => <option key={x.id} value={x.id}>{x.symbol}</option>)}</select><input placeholder="SKU / item code" value={form.sku} onChange={(e) => set('sku', e.target.value)} /><input placeholder="Description" value={form.description} onChange={(e) => set('description', e.target.value)} /><input placeholder="Opening quantity" value={form.opening_quantity} onChange={(e) => set('opening_quantity', e.target.value)} /><input placeholder="Opening rate" value={form.opening_rate} onChange={(e) => set('opening_rate', e.target.value)} /><input placeholder="Opening value" value={form.opening_value} onChange={(e) => set('opening_value', e.target.value)} /><input placeholder="Reorder level" value={form.reorder_level} onChange={(e) => set('reorder_level', e.target.value)} /></>}</>}</div><button onClick={() => void submit()} disabled={!activeCompany}>{editing ? 'Save changes' : 'Create'}</button>{editing && <button onClick={reset}>Cancel</button>}</div>
      {loading ? <div className="ot-report-loading">Loading {title}…</div> : rows.length === 0 ? <div className="ot-report-empty">No {title.toLowerCase()} found.</div> : <table className="ot-report-table"><thead><tr><th>Name</th><th>Status</th><th>Details</th><th>Actions</th></tr></thead><tbody>{rows.map((row) => <tr key={row.id}><td>{label(row)}</td><td>{active(row) ? 'Active' : 'Inactive'}</td><td>{'stock_group_id' in row ? `${row.sku || '—'} · opening ${row.opening_value}` : 'description' in row ? row.description || '—' : 'unit_type' in row ? `${row.decimal_precision} decimals` : '—'}</td><td><button onClick={() => edit(row)}>Edit</button> <button onClick={() => void toggle(row)}>{active(row) ? 'Deactivate' : 'Activate'}</button> <button onClick={() => void remove(row)}>Delete</button></td></tr>)}</tbody></table>}
    </div></section>;
}
