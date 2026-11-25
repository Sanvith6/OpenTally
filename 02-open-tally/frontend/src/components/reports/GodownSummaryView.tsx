import { useCallback, useEffect, useState } from 'react';
import { Company, Godown, GodownSummaryReport } from '../../types';
import { getGodownSummaryReport, listGodowns } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function GodownSummaryView({ activeCompany, onBack }: Props) {
  const [godowns, setGodowns] = useState<Godown[]>([]);
  const [selectedGodownId, setSelectedGodownId] = useState<string>('');
  const [asOfDate, setAsOfDate] = useState<string>(new Date().toISOString().slice(0, 10));
  const [report, setReport] = useState<GodownSummaryReport | null>(null);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load godowns list
  useEffect(() => {
    if (!activeCompany) return;
    async function fetchGodowns() {
      try {
        const gdns = await listGodowns();
        const active = gdns.filter(g => g.is_active);
        setGodowns(active);
        if (active.length > 0 && !selectedGodownId) {
          setSelectedGodownId(active[0].id);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    }
    void fetchGodowns();
  }, [activeCompany]);

  // Load report data
  const loadReport = useCallback(async () => {
    if (!activeCompany || !selectedGodownId) return;
    setLoading(true);
    setError(null);
    try {
      const data = await getGodownSummaryReport(selectedGodownId, asOfDate);
      setReport(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, selectedGodownId, asOfDate]);

  useEffect(() => {
    if (selectedGodownId) {
      void loadReport();
    }
  }, [loadReport, selectedGodownId]);

  // Esc hotkey
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onBack();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onBack]);

  const filteredItems = report?.items.filter(i =>
    i.stock_item_name.toLowerCase().includes(search.toLowerCase())
  ) || [];

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div>
          <h2>Godown Summary Report</h2>
          <p>{activeCompany?.name || 'Select an active company'}</p>
        </div>
        <button className="ot-report-back" onClick={onBack}>
          Back (Esc)
        </button>
      </div>

      {error && <div className="ot-report-error" role="alert">{error}</div>}

      {/* Toolbar */}
      <div className="ot-report-toolbar">
        <div className="ot-report-field">
          <label>Godown</label>
          <select
            value={selectedGodownId}
            onChange={e => setSelectedGodownId(e.target.value)}
          >
            {godowns.map(g => (
              <option key={g.id} value={g.id}>
                {g.name} {g.is_external ? '(External)' : ''}
              </option>
            ))}
          </select>
        </div>
        <div className="ot-report-field">
          <label>As of Date</label>
          <input
            type="date"
            value={asOfDate}
            onChange={e => setAsOfDate(e.target.value)}
          />
        </div>
        <div className="ot-report-field" style={{ flex: 1 }}>
          <label>Filter Item</label>
          <input
            placeholder="Search stock item..."
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
        </div>
        <button
          className="ot-report-action"
          onClick={() => void loadReport()}
          disabled={loading || !selectedGodownId}
        >
          {loading ? 'Refreshing...' : 'Refresh'}
        </button>
      </div>

      {/* Summary Cards */}
      {report && (
        <div className="ot-report-summary">
          <div className="ot-report-summary-card">
            <span>Godown Location</span>
            <strong>{report.godown_name}</strong>
            <small style={{ color: '#64748b' }}>
              {report.address || 'No specific address'} • {report.is_external ? 'Third-Party' : 'Internal'}
            </small>
          </div>
          <div className="ot-report-summary-card">
            <span>Total Valuation</span>
            <strong style={{ color: '#166534' }}>₹{report.total_godown_value}</strong>
          </div>
          <div className="ot-report-summary-card">
            <span>Active Items</span>
            <strong>{report.items.length}</strong>
          </div>
        </div>
      )}

      {/* Items Table */}
      <div className="ot-report-card">
        <table className="ot-report-table">
          <thead>
            <tr>
              <th>Stock Item</th>
              <th>Unit</th>
              <th className="money">Inflow Qty</th>
              <th className="money">Outflow Qty</th>
              <th className="money">Closing Qty</th>
              <th className="money">Avg Rate</th>
              <th className="money">Closing Value</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={7} className="ot-report-loading">Loading godown summary...</td>
              </tr>
            ) : filteredItems.length === 0 ? (
              <tr>
                <td colSpan={7} className="ot-report-empty">No stock activity found for this godown.</td>
              </tr>
            ) : (
              filteredItems.map(item => (
                <tr key={item.stock_item_id}>
                  <td style={{ fontWeight: 600 }}>{item.stock_item_name}</td>
                  <td>{item.base_unit}</td>
                  <td className="money" style={{ color: '#166534' }}>{item.inflow_quantity}</td>
                  <td className="money" style={{ color: '#991b1b' }}>{item.outflow_quantity}</td>
                  <td className="money" style={{ fontWeight: 700 }}>{item.closing_quantity}</td>
                  <td className="money">₹{item.average_rate}</td>
                  <td className="money" style={{ fontWeight: 700 }}>₹{item.closing_value}</td>
                </tr>
              ))
            )}
          </tbody>
          {report && filteredItems.length > 0 && (
            <tfoot>
              <tr className="ot-report-subtotal">
                <td colSpan={6} style={{ textAlign: 'right', fontWeight: 700 }}>Total Godown Valuation:</td>
                <td className="money" style={{ fontWeight: 700, color: '#166534' }}>
                  ₹{report.total_godown_value}
                </td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
    </section>
  );
}
