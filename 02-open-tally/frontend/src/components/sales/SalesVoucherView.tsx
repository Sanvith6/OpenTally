import { useCallback, useEffect, useState } from 'react';
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

    const totalTaxable = form.lines.reduce((acc, line) => {
      const q = parseFloat(line.quantity) || 0;
      const r = parseFloat(line.rate) || 0;
      let lineTotal = q * r;
      if (line.discount_kind === 'amount') {
        lineTotal = Math.max(0, lineTotal - (parseFloat(line.discount_value) || 0));
      } else if (line.discount_kind === 'percentage') {
        const pct = parseFloat(line.discount_value) || 0;
        lineTotal = Math.max(0, lineTotal * (1 - pct / 100));
      }
      return acc + lineTotal;
    }, 0);

    if (totalTaxable > 0) {
      previewGstCalculation(
        placeOfSupplyStateCode.trim() || '27',
        'Taxable',
        totalTaxable.toFixed(2),
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
          rate: line.rate,
          discount_kind: line.discount_kind,
          discount_value: line.discount_value,
        })),
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const submit = async () => {
    setError(null);
    if (!form.party_id || !form.series_id || form.lines.some((x) => !x.description.trim())) {
      setError('Customer, series, and every line description are required.');
      return;
    }
    try {
      if (editingId) await updateSalesDraft(editingId, form);
      else await createSalesDraft(form);
      setNotice(editingId ? 'Sales draft updated.' : 'Sales draft created.');
      reset();
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
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

  return (
    <section className="ot-report-screen">
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
            <select aria-label="Customer" value={form.party_id} onChange={(e) => set('party_id', e.target.value)}>
              <option value="">Customer</option>
              {parties.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.display_name} · {x.party_code}
                </option>
              ))}
            </select>
            <input
              aria-label="Series"
              placeholder="Sales series ID"
              value={form.series_id}
              onChange={(e) => set('series_id', e.target.value)}
            />
            <input
              type="date"
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
              placeholder="Narration"
              value={form.narration || ''}
              onChange={(e) => set('narration', e.target.value)}
            />
          </div>

          {form.lines.map((line, index) => (
            <div className="ot-inventory-fields" key={index}>
              <input
                placeholder="Line description"
                value={line.description}
                onChange={(e) => setLine(index, 'description', e.target.value)}
              />
              <select
                aria-label={`Stock item ${index + 1}`}
                value={line.stock_item_id || ''}
                onChange={(e) => setLine(index, 'stock_item_id', e.target.value)}
              >
                <option value="">Accounting-only line</option>
                {items.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.name}
                  </option>
                ))}
              </select>
              <select
                aria-label={`Unit ${index + 1}`}
                value={line.unit_id || ''}
                onChange={(e) => setLine(index, 'unit_id', e.target.value)}
              >
                <option value="">Unit</option>
                {units.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.symbol}
                  </option>
                ))}
              </select>
              <select
                aria-label={`Godown ${index + 1}`}
                value={line.godown_id || ''}
                onChange={(e) => setLine(index, 'godown_id', e.target.value)}
              >
                <option value="">Godown</option>
                {godowns.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.name}
                  </option>
                ))}
              </select>
              <input
                aria-label={`Quantity ${index + 1}`}
                value={line.quantity}
                onChange={(e) => setLine(index, 'quantity', e.target.value)}
              />
              <input
                aria-label={`Rate ${index + 1}`}
                value={line.rate}
                onChange={(e) => setLine(index, 'rate', e.target.value)}
              />
              <select
                aria-label={`Discount ${index + 1}`}
                value={line.discount_kind}
                onChange={(e) => setLine(index, 'discount_kind', e.target.value)}
              >
                <option value="none">No discount</option>
                <option value="amount">Amount</option>
                <option value="percentage">Percentage</option>
              </select>
              <input
                aria-label={`Discount value ${index + 1}`}
                value={line.discount_value}
                onChange={(e) => setLine(index, 'discount_value', e.target.value)}
              />
              {form.lines.length > 1 && (
                <button onClick={() => set('lines', form.lines.filter((_, i) => i !== index))}>
                  Remove line
                </button>
              )}
            </div>
          ))}

          <button onClick={() => set('lines', [...form.lines, newLine()])}>Add line</button>

          {/* Live GST Preview Box */}
          {gstPreview && (
            <div
              data-testid="gst-preview-box"
              style={{
                marginTop: '1.25rem',
                padding: '1rem',
                background: '#f4fbf7',
                border: '1px solid #c2ebd5',
                borderRadius: '6px',
              }}
            >
              <h4 style={{ margin: '0 0 0.5rem 0', color: '#145a32' }}>
                GST Calculation Preview ({gstPreview.supply_type})
              </h4>
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))',
                  gap: '0.75rem',
                  fontSize: '0.9rem',
                }}
              >
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Taxable Value:</span>
                  <strong>₹ {gstPreview.taxable_value}</strong>
                </div>
                {gstPreview.supply_type === 'IntraState' ? (
                  <>
                    <div>
                      <span style={{ color: '#666', display: 'block' }}>CGST ({gstPreview.cgst_rate}%):</span>
                      <strong>₹ {gstPreview.cgst_amount}</strong>
                    </div>
                    <div>
                      <span style={{ color: '#666', display: 'block' }}>SGST ({gstPreview.sgst_rate}%):</span>
                      <strong>₹ {gstPreview.sgst_amount}</strong>
                    </div>
                  </>
                ) : (
                  <div>
                    <span style={{ color: '#666', display: 'block' }}>IGST ({gstPreview.igst_rate}%):</span>
                    <strong>₹ {gstPreview.igst_amount}</strong>
                  </div>
                )}
                {parseFloat(gstPreview.cess_amount) > 0 && (
                  <div>
                    <span style={{ color: '#666', display: 'block' }}>Cess ({gstPreview.cess_rate}%):</span>
                    <strong>₹ {gstPreview.cess_amount}</strong>
                  </div>
                )}
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Total Tax:</span>
                  <strong style={{ color: '#b03a2e' }}>₹ {gstPreview.total_tax}</strong>
                </div>
                <div>
                  <span style={{ color: '#666', display: 'block' }}>Grand Total:</span>
                  <strong style={{ color: '#196f3d', fontSize: '1.05rem' }}>₹ {gstPreview.grand_total}</strong>
                </div>
              </div>
            </div>
          )}

          <div style={{ marginTop: '1rem', display: 'flex', gap: '0.5rem' }}>
            <button onClick={() => void submit()} disabled={!activeCompany}>
              {editingId ? 'Update draft' : 'Save draft'}
            </button>
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
    </section>
  );
}
