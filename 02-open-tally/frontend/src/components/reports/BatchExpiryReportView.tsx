import { useCallback, useEffect, useState } from 'react';
import { BatchExpiryReportRow, Company } from '../../types';
import { getBatchExpiryReport } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function BatchExpiryReportView({ activeCompany, onBack }: Props) {
  const [asOfDate, setAsOfDate] = useState<string>(new Date().toISOString().slice(0, 10));
  const [statusFilter, setStatusFilter] = useState<string>('All');
  const [search, setSearch] = useState<string>('');
  const [rows, setRows] = useState<BatchExpiryReportRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const data = await getBatchExpiryReport(asOfDate);
      setRows(data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, asOfDate]);

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

  const filtered = rows.filter(r => {
    const matchesSearch =
      r.stock_item_name.toLowerCase().includes(search.toLowerCase()) ||
      r.batch_number.toLowerCase().includes(search.toLowerCase());
    const matchesStatus =
      statusFilter === 'All' ||
      r.status === statusFilter ||
      (statusFilter === 'Good' && r.status === 'GoodStock');
    return matchesSearch && matchesStatus;
  });

  const totalValue =
    rows.reduce(
      (acc, r) => acc + Math.round((parseFloat(r.closing_value) || 0) * 100),
      0
    ) / 100;
  const expiredCount = rows.filter(r => r.status === 'Expired').length;
  const expiring30Count = rows.filter(r => r.status === 'ExpiringIn30Days').length;

  const getStatusBadge = (status: string) => {
    switch (status) {
      case 'Expired':
        return (
          <span style={{ background: '#fee2e2', color: '#991b1b', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
            Expired
          </span>
        );
      case 'ExpiringIn30Days':
        return (
          <span style={{ background: '#ffedd5', color: '#9a3412', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
            ≤ 30 Days
          </span>
        );
      case 'ExpiringIn60Days':
        return (
          <span style={{ background: '#fef9c3', color: '#854d0e', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
            ≤ 60 Days
          </span>
        );
      case 'Good':
      case 'GoodStock':
        return (
          <span style={{ background: '#dcfce7', color: '#166534', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
            Good
          </span>
        );
      default:
        return <span>{status}</span>;
    }
  };

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div>
          <h2>Batch & Expiry Aging Report</h2>
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
          <label>As of Date</label>
          <input
            type="date"
            value={asOfDate}
            onChange={e => setAsOfDate(e.target.value)}
          />
        </div>
        <div className="ot-report-field">
          <label>Status Filter</label>
          <select
            value={statusFilter}
            onChange={e => setStatusFilter(e.target.value)}
          >
            <option value="All">All Batches</option>
            <option value="Expired">Expired</option>
            <option value="ExpiringIn30Days">Expiring in 30 Days</option>
            <option value="ExpiringIn60Days">Expiring in 60 Days</option>
            <option value="Good">Good / Healthy</option>
          </select>
        </div>
        <div className="ot-report-field" style={{ flex: 1 }}>
          <label>Search</label>
          <input
            placeholder="Search item, batch, or godown..."
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
        </div>
        <button
          className="ot-report-action"
          onClick={() => void loadData()}
          disabled={loading}
        >
          {loading ? 'Refreshing...' : 'Refresh'}
        </button>
      </div>

      {/* Summary Cards */}
      <div className="ot-report-summary">
        <div className="ot-report-summary-card">
          <span>Total Batched Inventory</span>
          <strong>₹{totalValue.toFixed(2)}</strong>
        </div>
        <div className="ot-report-summary-card">
          <span>Expired Batches</span>
          <strong style={{ color: expiredCount > 0 ? '#b91c1c' : '#1e293b' }}>
            {expiredCount}
          </strong>
        </div>
        <div className="ot-report-summary-card">
          <span>Expiring in 30 Days</span>
          <strong style={{ color: expiring30Count > 0 ? '#c2410c' : '#1e293b' }}>
            {expiring30Count}
          </strong>
        </div>
      </div>

      {/* Report Table */}
      <div className="ot-report-card">
        <table className="ot-report-table">
          <thead>
            <tr>
              <th>Stock Item</th>
              <th>Batch No</th>
              <th>Mfg Date</th>
              <th>Expiry Date</th>
              <th className="money">Closing Qty</th>
              <th className="money">Unit Cost</th>
              <th className="money">Closing Value</th>
              <th className="money">Days to Expiry</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={9} className="ot-report-loading">Loading batch expiry data...</td>
              </tr>
            ) : filtered.length === 0 ? (
              <tr>
                <td colSpan={9} className="ot-report-empty">No matching batches found.</td>
              </tr>
            ) : (
              filtered.map((row, idx) => (
                <tr key={`${row.batch_id}-${idx}`}>
                  <td style={{ fontWeight: 600 }}>{row.stock_item_name}</td>
                  <td>
                    <code>{row.batch_number}</code>
                  </td>
                  <td>{row.mfg_date || '—'}</td>
                  <td>{row.expiry_date || '—'}</td>
                  <td className="money" style={{ fontWeight: 600 }}>{row.closing_quantity}</td>
                  <td className="money">₹{row.unit_cost}</td>
                  <td className="money" style={{ fontWeight: 700 }}>₹{row.closing_value}</td>
                  <td className="money">
                    {row.days_until_expiry !== null ? `${row.days_until_expiry} d` : '—'}
                  </td>
                  <td>{getStatusBadge(row.status)}</td>
                </tr>
              ))
            )}
          </tbody>
          {filtered.length > 0 && (
            <tfoot>
              <tr className="ot-report-subtotal">
                <td colSpan={6} style={{ textAlign: 'right', fontWeight: 700 }}>Filtered Batch Total:</td>
                <td className="money" style={{ fontWeight: 700, color: '#166534' }}>
                  ₹{filtered.reduce((acc, r) => acc + (parseFloat(r.closing_value) || 0), 0).toFixed(2)}
                </td>
                <td colSpan={2}></td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
    </section>
  );
}
