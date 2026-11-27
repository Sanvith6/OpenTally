import { useCallback, useEffect, useState, useRef } from 'react';
import {
  Company,
  Godown,
  PartyProfile,
  SalesDocument,
  SalesDocumentRequest,
  StockItem,
  Unit,
  GstCalculationResult,
} from '../../types';
import {
  createSalesDraft,
  deleteSalesDraft,
  getSalesDraft,
  listGodowns,
  listParties,
  listSalesDrafts,
  listStockItems,
  listUnits,
  postSalesDocument,
  updateSalesDraft,
  previewGstCalculation,
} from '../../services/api';
import '../reports/Reports.css';
import { CommercialLineFields } from '../vouchers/CommercialLineFields';
import { CommercialGstPreview } from '../vouchers/CommercialTaxPreview';
import { taxableLinesTotal, minorAmount } from '../vouchers/terminalAmounts';
import { useTerminalKeyboard } from '../vouchers/terminalKeyboard';
import { AcceptVoucherDialog } from '../vouchers/AcceptVoucherDialog';
import { SearchableAccountSelect } from '../vouchers/SearchableAccountSelect';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

const newLine = () => ({
  description: '',
  stock_item_id: '',
  unit_id: '',
  godown_id: '',
  quantity: '1',
  rate: '0',
  discount_kind: 'none' as const,
  discount_value: '0',
});

const emptyRequest = (): SalesDocumentRequest => ({
  series_id: '',
  party_id: '',
  invoice_date: new Date().toISOString().slice(0, 10),
  document_reference: '',
  due_date_override: '',
  due_date_override_reason: '',
  narration: '',
  document_discount_amount: '0',
  additional_charges: '0',
  lines: [newLine()],
});

export function SalesVoucherView({ activeCompany, onBack }: Props) {
  const [rows, setRows] = useState<SalesDocument[]>([]);
  const [parties, setParties] = useState<PartyProfile[]>([]);
  const [items, setItems] = useState<StockItem[]>([]);
  const [units, setUnits] = useState<Unit[]>([]);
  const [godowns, setGodowns] = useState<Godown[]>([]);
  const [form, setForm] = useState<SalesDocumentRequest>(emptyRequest);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [acceptOpen, setAcceptOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const submitLock = useRef(false);
  const terminalRef = useRef<HTMLElement>(null);

  // GST State
  const [placeOfSupplyStateCode, setPlaceOfSupplyStateCode] = useState('');
  const [isReverseCharge, setIsReverseCharge] = useState(false);
  const [gstPreview, setGstPreview] = useState<GstCalculationResult | null>(null);

  const load = useCallback(async () => {
    if (!activeCompany) {
      setRows([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const [drafts, partyRows, stockRows, unitRows, godownRows] = await Promise.all([
        listSalesDrafts(search),
        listParties(),
        listStockItems(),
        listUnits(),
        listGodowns(),
      ]);
      setRows(drafts);
      setParties(partyRows.filter((x) => x.customer_enabled && x.is_active));
      setItems(stockRows.filter((x) => x.is_active));
      setUnits(unitRows.filter((x) => x.is_active));
      setGodowns(godownRows.filter((x) => x.is_active));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, search]);

  useEffect(() => {
    void load();
  }, [load]);

  // Live GST preview calculation when lines or POS change
  useEffect(() => {
    if (!activeCompany) {
      setGstPreview(null);
      return;
    }

    const totalTaxable = taxableLinesTotal(form.lines);

    if (minorAmount(totalTaxable) > 0n) {
      previewGstCalculation(
        placeOfSupplyStateCode.trim() || '27',
        'Taxable',
        totalTaxable,
        '18.00',
        '0.00',
        isReverseCharge
      )
        .then(setGstPreview)
        .catch(() => setGstPreview(null));
    } else {
      setGstPreview(null);
    }
  }, [activeCompany, form.lines, placeOfSupplyStateCode, isReverseCharge]);

  const set = (key: keyof SalesDocumentRequest, value: unknown) =>
    setForm((current) => ({ ...current, [key]: value }));

  const setLine = (index: number, key: string, value: string) =>
    setForm((current) => ({
      ...current,
      lines: current.lines.map((line, i) => (i === index ? { ...line, [key]: value } : line)),
    }));

  const reset = () => {
    setEditingId(null);
    setForm(emptyRequest());
    setPlaceOfSupplyStateCode('');
    setIsReverseCharge(false);
    setGstPreview(null);
  };

  const edit = async (id: string) => {
    try {
      const draft = await getSalesDraft(id);
      setEditingId(id);
      setForm({
        series_id: '',
        party_id: draft.document.party_id,
        invoice_date: draft.document.invoice_date,
        document_reference: draft.document.document_reference || '',
        due_date_override:
          draft.document.due_date_source === 'explicit_override' ? draft.document.due_date : '',
        due_date_override_reason: draft.document.due_date_override_reason || '',
        narration: draft.document.narration || '',
        document_discount_amount: '0',
        additional_charges: draft.document.additional_charges,
        lines: draft.lines.map((line) => ({
          description: line.description,
          stock_item_id: line.stock_item_id || '',
          unit_id: line.unit_id || '',
          godown_id: line.godown_id || '',
          quantity: line.quantity,
          billed_quantity: line.billed_quantity ?? null,
          rate: line.rate,
          discount_kind: line.discount_kind,
          discount_value: line.discount_value,
        })),
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const submit = async (accept = false) => {
    if (submitLock.current) return;
    setError(null);
    if (!form.party_id || !form.series_id || form.lines.some((x) => !x.description.trim())) {
      setError('Customer, series, and every line description are required.');
      return;
    }
    submitLock.current = true; setSubmitting(true);
    try {
      const saved = editingId ? await updateSalesDraft(editingId, form) : await createSalesDraft(form);
      if (accept) {
        setEditingId(saved.document.id);
        const result = await postSalesDocument(saved.document.id);
        setNotice(`Sales document posted. Voucher: ${result.voucher.voucher.voucher_number}`);
      } else setNotice(editingId ? 'Sales draft updated.' : 'Sales draft created.');
      setAcceptOpen(false);
      reset();
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e)); setAcceptOpen(false);
    } finally { submitLock.current = false; setSubmitting(false); }
  };

  const post = async (id: string) => {
    setError(null);
    try {
      const res = await postSalesDocument(id);
      setNotice(`Sales document posted. Voucher: ${res.voucher.voucher.voucher_number}`);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  useTerminalKeyboard({ container: terminalRef, onAccept: () => void submit(true), onBack });
  useEffect(() => { terminalRef.current?.querySelector<HTMLElement>('[data-terminal-order="10"]')?.focus(); }, []);

  return (
    <section className="ot-report-screen" ref={terminalRef} data-terminal-entry>
      <div className="ot-voucher-ribbon ot-ribbon-sales" data-testid="voucher-ribbon-sales">
        <span>Sales Voucher (F8)</span>
        <span>Outward Taxable Supply &amp; Inventory Delivery</span>
      </div>
      <div className="ot-report-header">
        <div>
          <h2>Sales Vouchers</h2>
          <p>{activeCompany ? `${activeCompany.name} · Draft document management` : 'Select an active company'}</p>
        </div>
        <button className="ot-report-back" onClick={onBack}>
          Back to Gateway
        </button>
      </div>
      <div className="ot-report-card">
        <div className="ot-inventory-toolbar">
          <input
            aria-label="Search sales drafts"
            placeholder="Search reference or narration"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <button onClick={() => void load()}>Refresh</button>
        </div>
        {notice && <div className="ot-report-success" role="status">{notice}</div>}
        {error && <div className="ot-report-error" role="alert">{error}</div>}
        <div className="ot-inventory-form">
          <h3>{editingId ? 'Edit sales draft' : 'New sales draft'}</h3>
          <p className="ot-report-muted">
            Atomic GL + FIFO Inventory posting is active with Indian GST calculation.
          </p>
          <div className="ot-inventory-fields">
            <SearchableAccountSelect label="Customer" order={20} value={form.party_id} onChange={(id) => set('party_id', id)}
              options={parties.map((row) => ({ id: row.id, name: `${row.display_name} · ${row.party_code}` }))} />
            <input
              data-terminal-order={30}
              aria-label="Series"
              placeholder="Sales series ID"
              value={form.series_id}
              onChange={(e) => set('series_id', e.target.value)}
            />
            <input
              type="date"
              data-terminal-order={10}
              aria-label="Invoice date"
              value={form.invoice_date}
              onChange={(e) => set('invoice_date', e.target.value)}
            />
            <input
              placeholder="Document reference"
              value={form.document_reference || ''}
              onChange={(e) => set('document_reference', e.target.value)}
            />
            <input
              placeholder="Due-date override"
              value={form.due_date_override || ''}
              onChange={(e) => set('due_date_override', e.target.value)}
            />
            <input
              placeholder="Override reason"
              value={form.due_date_override_reason || ''}
              onChange={(e) => set('due_date_override_reason', e.target.value)}
            />
            <input
              aria-label="Place of supply"
              placeholder="Place of Supply (State Code, e.g. 27)"
              value={placeOfSupplyStateCode}
              onChange={(e) => setPlaceOfSupplyStateCode(e.target.value)}
              maxLength={2}
            />
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
              <input
                type="checkbox"
                aria-label="Reverse charge"
                checked={isReverseCharge}
                onChange={(e) => setIsReverseCharge(e.target.checked)}
              />
              <span>Reverse Charge</span>
            </label>
            <textarea
              data-terminal-order={900}
              placeholder="Narration"
              value={form.narration || ''}
              onChange={(e) => set('narration', e.target.value)}
            />
          </div>

          {form.lines.map((line, index) => <CommercialLineFields key={index} line={line} index={index}
            items={items} units={units} godowns={godowns} onChange={(field, value) => setLine(index, field, value)}
            onRemove={form.lines.length > 1 ? () => set('lines', form.lines.filter((_, i) => i !== index)) : undefined} />)}

          <button onClick={() => set('lines', [...form.lines, newLine()])}>Add line</button>

          {/* Live GST Preview Box */}
          <CommercialGstPreview preview={gstPreview} />

          <div style={{ marginTop: '1rem', display: 'flex', gap: '0.5rem' }}>
            <button onClick={() => void submit()} disabled={!activeCompany || submitting}>
              {editingId ? 'Update draft' : 'Save draft'}
            </button>
            <button data-terminal-order={1000} data-terminal-accept disabled={!activeCompany || submitting} onClick={() => setAcceptOpen(true)}>Accept [Y/N] · Ctrl+A</button>
            {editingId && <button onClick={reset}>Cancel edit</button>}
          </div>
        </div>

        <h3>Sales draft register</h3>
        {loading ? (
          <div className="ot-report-loading">Loading drafts…</div>
        ) : rows.length === 0 ? (
          <div className="ot-report-empty">No sales drafts found.</div>
        ) : (
          <table className="ot-report-table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Reference</th>
                <th>Status</th>
                <th>Net</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={row.id}>
                  <td>{row.invoice_date}</td>
                  <td>{row.document_reference || '—'}</td>
                  <td>{row.status}</td>
                  <td>{row.net_total}</td>
                  <td>
                    {row.status === 'draft' ? (
                      <>
                        <button onClick={() => void post(row.id)}>Post to GL & Inventory</button>{' '}
                        <button onClick={() => void edit(row.id)}>Edit draft</button>{' '}
                        <button
                          onClick={() =>
                            void deleteSalesDraft(row.id)
                              .then(load)
                              .catch((e) => setError(e instanceof Error ? e.message : String(e)))
                          }
                        >
                          Delete draft
                        </button>
                      </>
                    ) : (
                      <span className="ot-badge-clean">Posted</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      {acceptOpen && <AcceptVoucherDialog busy={submitting} onYes={() => void submit(true)} onNo={() => setAcceptOpen(false)} />}
    </section>
  );
}
