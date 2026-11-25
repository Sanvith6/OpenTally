import { useCallback, useEffect, useState } from 'react';
import { Company, CostCentreBreakupReport } from '../../types';
import { getCostCentreBreakupReport } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function CostCentreBreakupReportView({ activeCompany, onBack }: Props) {
  const [fromDate, setFromDate] = useState<string>('');
  const [toDate, setToDate] = useState<string>('');
  const [report, setReport] = useState<CostCentreBreakupReport | null>(null);
  const [search, setSearch] = useState<string>('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const data = await getCostCentreBreakupReport(
        undefined,
        fromDate.trim() ? fromDate.trim() : undefined,
        toDate.trim() ? toDate.trim() : undefined
      );
      setReport(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, fromDate, toDate]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

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

  const filteredCentres = report?.centres.filter(c =>
    c.cost_centre_name.toLowerCase().includes(search.toLowerCase()) ||
    c.category_name.toLowerCase().includes(search.toLowerCase())
  ) || [];

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div>
          <h2>Cost Centre Breakup Report</h2>
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
          <label>From Date</label>
          <input
            type="date"
            value={fromDate}
            onChange={e => setFromDate(e.target.value)}
          />
        </div>
        <div className="ot-report-field">
          <label>To Date</label>
          <input
            type="date"
            value={toDate}
            onChange={e => setToDate(e.target.value)}
          />
        </div>
        <div className="ot-report-field" style={{ flex: 1 }}>
          <label>Filter Centre / Category</label>
          <input
            placeholder="Search category or centre..."
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
        </div>
        <button
          className="ot-report-action"
          onClick={() => void loadData()}
          disabled={loading}
        >
          {loading ? 'Refreshing...' : 'Apply Filters'}
        </button>
      </div>

      {/* Summary Cards */}
      {report && (
        <div className="ot-report-summary">
          <div className="ot-report-summary-card">
            <span>Total Debited</span>
            <strong style={{ color: '#166534' }}>₹{report.total_debit}</strong>
          </div>
          <div className="ot-report-summary-card">
            <span>Total Credited</span>
            <strong style={{ color: '#991b1b' }}>₹{report.total_credit}</strong>
          </div>
          <div className="ot-report-summary-card">
            <span>Net Cost Incurred</span>
            <strong style={{ color: '#1e3a5f' }}>₹{report.net_total}</strong>
          </div>
        </div>
      )}

      {/* Table */}
      <div className="ot-report-card">
        <table className="ot-report-table">
          <thead>
            <tr>
              <th>Cost Category</th>
              <th>Cost Centre</th>
              <th>Allocations</th>
              <th className="money">Debit (₹)</th>
              <th className="money">Credit (₹)</th>
              <th className="money">Net (₹)</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={6} className="ot-report-loading">Loading cost centre breakup...</td>
              </tr>
            ) : filteredCentres.length === 0 ? (
              <tr>
                <td colSpan={6} className="ot-report-empty">No voucher cost allocations found for the selected period.</td>
              </tr>
            ) : (
              filteredCentres.map(c => (
                <tr key={`${c.category_id}-${c.cost_centre_id}`}>
                  <td>{c.category_name}</td>
                  <td style={{ fontWeight: 600 }}>{c.cost_centre_name}</td>
                  <td>{c.allocations_count} items</td>
                  <td className="money" style={{ color: '#166534' }}>{c.debit_total}</td>
                  <td className="money" style={{ color: '#991b1b' }}>{c.credit_total}</td>
                  <td className="money" style={{ fontWeight: 700 }}>{c.net_amount}</td>
                </tr>
              ))
            )}
          </tbody>
          {report && filteredCentres.length > 0 && (
            <tfoot>
              <tr className="ot-report-subtotal">
                <td colSpan={3} style={{ textAlign: 'right', fontWeight: 700 }}>Total:</td>
                <td className="money" style={{ fontWeight: 700, color: '#166534' }}>₹{report.total_debit}</td>
                <td className="money" style={{ fontWeight: 700, color: '#991b1b' }}>₹{report.total_credit}</td>
                <td className="money" style={{ fontWeight: 700, color: '#1e3a5f' }}>₹{report.net_total}</td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
    </section>
  );
}
