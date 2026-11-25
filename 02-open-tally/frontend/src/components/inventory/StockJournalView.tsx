import { useCallback, useEffect, useState } from 'react';
import { Company, Godown, StockItem, StockJournalRequest, StockJournalResponse } from '../../types';
import { listGodowns, listStockItems, postStockJournalVoucher, listStockJournalVouchers } from '../../services/api';
import '../reports/Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

interface UiLine {
  id: string;
  stock_item_id: string;
  godown_id: string;
  batch_number: string;
  quantity: string;
  rate: string;
  amount: string;
}

const emptyLine = (): UiLine => ({
  id: Math.random().toString(36).substring(2, 9),
  stock_item_id: '',
  godown_id: '',
  batch_number: '',
  quantity: '1',
  rate: '0.00',
  amount: '0.00',
});

export function StockJournalView({ activeCompany, onBack }: Props) {
  const [activeTab, setActiveTab] = useState<'entry' | 'register'>('entry');
  const [voucherNumber, setVoucherNumber] = useState('');
  const [voucherDate, setVoucherDate] = useState(new Date().toISOString().slice(0, 10));
  const [narration, setNarration] = useState('');

  const [sourceLines, setSourceLines] = useState<UiLine[]>([emptyLine()]);
  const [destLines, setDestLines] = useState<UiLine[]>([emptyLine()]);

  const [items, setItems] = useState<StockItem[]>([]);
  const [godowns, setGodowns] = useState<Godown[]>([]);
  const [pastVouchers, setPastVouchers] = useState<StockJournalResponse[]>([]);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const [itms, gdns, past] = await Promise.all([
        listStockItems(),
        listGodowns(),
        listStockJournalVouchers(),
      ]);
      setItems(itms.filter(i => i.is_active));
      setGodowns(gdns.filter(g => g.is_active));
      setPastVouchers(past);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  // Keyboard shortcut listener
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onBack();
      } else if (e.ctrlKey && (e.key === 'a' || e.key === 'A')) {
        e.preventDefault();
        void handleSubmit();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [sourceLines, destLines, voucherDate, voucherNumber, narration, activeCompany]);

  const updateLine = (
    isSource: boolean,
    index: number,
    field: keyof UiLine,
    value: string
  ) => {
    const setter = isSource ? setSourceLines : setDestLines;
    setter(prev => {
      const updated = [...prev];
      const cur = { ...updated[index], [field]: value };
      if (field === 'quantity' || field === 'rate') {
        const qPaise = Math.round((parseFloat(field === 'quantity' ? value : cur.quantity) || 0) * 100);
        const rPaise = Math.round((parseFloat(field === 'rate' ? value : cur.rate) || 0) * 100);
        const amountPaise = Math.round((qPaise * rPaise) / 10000);
        cur.amount = (amountPaise / 100).toFixed(2);
      }
      updated[index] = cur;
      return updated;
    });
  };

  const addLine = (isSource: boolean) => {
    const setter = isSource ? setSourceLines : setDestLines;
    setter(prev => [...prev, emptyLine()]);
  };

  const removeLine = (isSource: boolean, index: number) => {
    const setter = isSource ? setSourceLines : setDestLines;
    setter(prev => {
      if (prev.length <= 1) return prev;
      return prev.filter((_, i) => i !== index);
    });
  };

  const calcTotal = (lines: UiLine[]) => {
    const totalPaise = lines.reduce(
      (acc, l) => acc + Math.round((parseFloat(l.amount) || 0) * 100),
      0
    );
    return totalPaise / 100;
  };

  const totalSource = calcTotal(sourceLines);
  const totalDest = calcTotal(destLines);

  const handleSubmit = async () => {
    if (!activeCompany) return;
    setError(null);
    setMessage(null);

    // Validation
    const validSource = sourceLines.filter(
      l => l.stock_item_id && l.godown_id && parseFloat(l.quantity) > 0
    );
    const validDest = destLines.filter(
      l => l.stock_item_id && l.godown_id && parseFloat(l.quantity) > 0
    );

    if (validSource.length === 0 && validDest.length === 0) {
      setError('Please provide at least one valid source or destination line.');
      return;
    }

    try {
      setLoading(true);
      const req: StockJournalRequest = {
        voucher_number: voucherNumber.trim() ? voucherNumber.trim() : null,
        voucher_date: voucherDate,
        narration: narration.trim() ? narration.trim() : null,
        source_lines: validSource.map(l => ({
          stock_item_id: l.stock_item_id,
          godown_id: l.godown_id,
          batch_number: l.batch_number.trim() ? l.batch_number.trim() : null,
          quantity: l.quantity,
          rate: l.rate,
          amount: l.amount,
        })),
        destination_lines: validDest.map(l => ({
          stock_item_id: l.stock_item_id,
          godown_id: l.godown_id,
          batch_number: l.batch_number.trim() ? l.batch_number.trim() : null,
          quantity: l.quantity,
          rate: l.rate,
          amount: l.amount,
        })),
      };

      const res = await postStockJournalVoucher(req);
      setMessage(`Stock Journal ${res.voucher_number} posted successfully.`);
      setVoucherNumber('');
      setNarration('');
      setSourceLines([emptyLine()]);
      setDestLines([emptyLine()]);
      await loadData();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div>
          <h2>Stock Journal (Alt+F7)</h2>
          <p>{activeCompany?.name || 'Select an active company'}</p>
        </div>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            className={`ot-report-back ${activeTab === 'entry' ? 'ot-tab-active' : ''}`}
            onClick={() => setActiveTab('entry')}
          >
            Voucher Entry
          </button>
          <button
            className={`ot-report-back ${activeTab === 'register' ? 'ot-tab-active' : ''}`}
            onClick={() => setActiveTab('register')}
          >
            Journal Register
          </button>
          <button className="ot-report-back" onClick={onBack}>
            Back (Esc)
          </button>
        </div>
      </div>

      {message && <div className="ot-report-success">{message}</div>}
      {error && <div className="ot-report-error" role="alert">{error}</div>}

      {activeTab === 'entry' ? (
        <div className="ot-report-card" style={{ padding: '20px' }}>
          {/* Header fields */}
          <div style={{ display: 'flex', gap: '16px', marginBottom: '20px', flexWrap: 'wrap' }}>
            <div className="ot-report-field">
              <label>Voucher Date</label>
              <input
                type="date"
                value={voucherDate}
                onChange={e => setVoucherDate(e.target.value)}
              />
            </div>
            <div className="ot-report-field">
              <label>Voucher Number (Optional)</label>
              <input
                placeholder="Auto-generated if blank"
                value={voucherNumber}
                onChange={e => setVoucherNumber(e.target.value)}
              />
            </div>
            <div className="ot-report-field" style={{ flex: 1, minWidth: '240px' }}>
              <label>Narration</label>
              <input
                placeholder="Remarks or transfer reason"
                value={narration}
                onChange={e => setNarration(e.target.value)}
              />
            </div>
          </div>

          {/* Dual-column: Source vs Destination */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px' }}>
            {/* Source / Consumption */}
            <div style={{ border: '1px solid #e2e8f0', borderRadius: '6px', padding: '12px', background: '#fafbfc' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
                <h4 style={{ margin: 0, color: '#991b1b' }}>Source (Consumption / Transfer Out)</h4>
                <button
                  type="button"
                  style={{ fontSize: '12px', padding: '4px 8px', cursor: 'pointer' }}
                  onClick={() => addLine(true)}
                >
                  + Add Line
                </button>
              </div>
              <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
                <thead>
                  <tr style={{ background: '#fee2e2', color: '#991b1b', textAlign: 'left' }}>
                    <th style={{ padding: '6px' }}>Item</th>
                    <th style={{ padding: '6px' }}>Godown</th>
                    <th style={{ padding: '6px' }}>Batch</th>
                    <th style={{ padding: '6px' }}>Qty</th>
                    <th style={{ padding: '6px' }}>Rate</th>
                    <th style={{ padding: '6px' }}>Amount</th>
                    <th style={{ padding: '6px' }}></th>
                  </tr>
                </thead>
                <tbody>
                  {sourceLines.map((line, idx) => (
                    <tr key={line.id} style={{ borderBottom: '1px solid #f1f5f9' }}>
                      <td style={{ padding: '4px' }}>
                        <select
                          style={{ width: '100%', fontSize: '11px', padding: '4px' }}
                          value={line.stock_item_id}
                          onChange={e => updateLine(true, idx, 'stock_item_id', e.target.value)}
                        >
                          <option value="">Select Item</option>
                          {items.map(i => (
                            <option key={i.id} value={i.id}>{i.name}</option>
                          ))}
                        </select>
                      </td>
                      <td style={{ padding: '4px' }}>
                        <select
                          style={{ width: '100%', fontSize: '11px', padding: '4px' }}
                          value={line.godown_id}
                          onChange={e => updateLine(true, idx, 'godown_id', e.target.value)}
                        >
                          <option value="">Select Godown</option>
                          {godowns.map(g => (
                            <option key={g.id} value={g.id}>{g.name}</option>
                          ))}
                        </select>
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          placeholder="Batch (opt)"
                          style={{ width: '70px', fontSize: '11px', padding: '4px' }}
                          value={line.batch_number}
                          onChange={e => updateLine(true, idx, 'batch_number', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          type="number"
                          style={{ width: '50px', fontSize: '11px', padding: '4px' }}
                          value={line.quantity}
                          onChange={e => updateLine(true, idx, 'quantity', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          type="number"
                          style={{ width: '60px', fontSize: '11px', padding: '4px' }}
                          value={line.rate}
                          onChange={e => updateLine(true, idx, 'rate', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px', textAlign: 'right', fontWeight: 600 }}>
                        {line.amount}
                      </td>
                      <td style={{ padding: '4px', textAlign: 'center' }}>
                        <button
                          type="button"
                          style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer' }}
                          onClick={() => removeLine(true, idx)}
                          title="Remove line"
                        >
                          ✕
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div style={{ textAlign: 'right', marginTop: '10px', fontWeight: 700, color: '#991b1b' }}>
                Total Source Value: ₹{totalSource.toFixed(2)}
              </div>
            </div>

            {/* Destination / Production */}
            <div style={{ border: '1px solid #e2e8f0', borderRadius: '6px', padding: '12px', background: '#fafbfc' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
                <h4 style={{ margin: 0, color: '#166534' }}>Destination (Production / Transfer In)</h4>
                <button
                  type="button"
                  style={{ fontSize: '12px', padding: '4px 8px', cursor: 'pointer' }}
                  onClick={() => addLine(false)}
                >
                  + Add Line
                </button>
              </div>
              <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
                <thead>
                  <tr style={{ background: '#dcfce7', color: '#166534', textAlign: 'left' }}>
                    <th style={{ padding: '6px' }}>Item</th>
                    <th style={{ padding: '6px' }}>Godown</th>
                    <th style={{ padding: '6px' }}>Batch</th>
                    <th style={{ padding: '6px' }}>Qty</th>
                    <th style={{ padding: '6px' }}>Rate</th>
                    <th style={{ padding: '6px' }}>Amount</th>
                    <th style={{ padding: '6px' }}></th>
                  </tr>
                </thead>
                <tbody>
                  {destLines.map((line, idx) => (
                    <tr key={line.id} style={{ borderBottom: '1px solid #f1f5f9' }}>
                      <td style={{ padding: '4px' }}>
                        <select
                          style={{ width: '100%', fontSize: '11px', padding: '4px' }}
                          value={line.stock_item_id}
                          onChange={e => updateLine(false, idx, 'stock_item_id', e.target.value)}
                        >
                          <option value="">Select Item</option>
                          {items.map(i => (
                            <option key={i.id} value={i.id}>{i.name}</option>
                          ))}
                        </select>
                      </td>
                      <td style={{ padding: '4px' }}>
                        <select
                          style={{ width: '100%', fontSize: '11px', padding: '4px' }}
                          value={line.godown_id}
                          onChange={e => updateLine(false, idx, 'godown_id', e.target.value)}
                        >
                          <option value="">Select Godown</option>
                          {godowns.map(g => (
                            <option key={g.id} value={g.id}>{g.name}</option>
                          ))}
                        </select>
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          placeholder="Batch (opt)"
                          style={{ width: '70px', fontSize: '11px', padding: '4px' }}
                          value={line.batch_number}
                          onChange={e => updateLine(false, idx, 'batch_number', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          type="number"
                          style={{ width: '50px', fontSize: '11px', padding: '4px' }}
                          value={line.quantity}
                          onChange={e => updateLine(false, idx, 'quantity', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px' }}>
                        <input
                          type="number"
                          style={{ width: '60px', fontSize: '11px', padding: '4px' }}
                          value={line.rate}
                          onChange={e => updateLine(false, idx, 'rate', e.target.value)}
                        />
                      </td>
                      <td style={{ padding: '4px', textAlign: 'right', fontWeight: 600 }}>
                        {line.amount}
                      </td>
                      <td style={{ padding: '4px', textAlign: 'center' }}>
                        <button
                          type="button"
                          style={{ background: 'transparent', border: 0, color: '#ef4444', cursor: 'pointer' }}
                          onClick={() => removeLine(false, idx)}
                          title="Remove line"
                        >
                          ✕
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div style={{ textAlign: 'right', marginTop: '10px', fontWeight: 700, color: '#166534' }}>
                Total Destination Value: ₹{totalDest.toFixed(2)}
              </div>
            </div>
          </div>

          {/* Action bar */}
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '24px', paddingTop: '16px', borderTop: '1px solid #e2e8f0' }}>
            <div style={{ fontSize: '13px', color: '#64748b' }}>
              Shortcut: <strong>Ctrl+A</strong> to Save & Post, <strong>Esc</strong> to cancel.
            </div>
            <div style={{ display: 'flex', gap: '12px' }}>
              <button
                type="button"
                className="ot-report-back"
                onClick={onBack}
              >
                Cancel (Esc)
              </button>
              <button
                type="button"
                className="ot-report-action"
                onClick={() => void handleSubmit()}
                disabled={loading || !activeCompany}
              >
                {loading ? 'Posting...' : 'Save & Post (Ctrl+A)'}
              </button>
            </div>
          </div>
        </div>
      ) : (
        /* Register Tab */
        <div className="ot-report-card">
          <table className="ot-report-table">
            <thead>
              <tr>
                <th>Voucher Number</th>
                <th>Voucher Date</th>
                <th>Lines</th>
                <th>Source Value</th>
                <th>Dest Value</th>
              </tr>
            </thead>
            <tbody>
              {pastVouchers.length === 0 ? (
                <tr>
                  <td colSpan={5} className="ot-report-empty">
                    No Stock Journal vouchers recorded yet.
                  </td>
                </tr>
              ) : (
                pastVouchers.map(v => (
                  <tr key={v.voucher_id}>
                    <td style={{ fontWeight: 600 }}>{v.voucher_number}</td>
                    <td>{v.voucher_date}</td>
                    <td>{v.lines_count} lines</td>
                    <td className="money">{v.total_source_value}</td>
                    <td className="money">{v.total_destination_value}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
