import { useCallback, useEffect, useState } from 'react';
import { Company, RatioAnalysisReport } from '../../types';
import { getRatioAnalysis } from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function RatioAnalysisView({ activeCompany, onBack }: Props) {
  const [fromDate, setFromDate] = useState<string>(activeCompany?.financial_year_begins || '2024-04-01');
  const [toDate, setToDate] = useState<string>(new Date().toISOString().slice(0, 10));
  const [report, setReport] = useState<RatioAnalysisReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const data = await getRatioAnalysis(activeCompany.id, fromDate, toDate);
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

  // Keyboard navigation
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

  const formatAmount = (amt: string | undefined) => {
    if (!amt) return '₹0.00';
    const num = parseFloat(amt);
    if (isNaN(num)) return amt;
    return `₹${num.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  };

  return (
    <div className="report-container ratio-analysis-view" style={{ padding: '16px', background: '#0f172a', color: '#f8fafc', height: '100%', overflowY: 'auto' }}>
      {/* Top Action Bar */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid #334155', paddingBottom: '12px', marginBottom: '16px' }}>
        <div>
          <h2 style={{ margin: 0, fontSize: '1.25rem', color: '#38bdf8', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>Ratio Analysis</span>
            <span style={{ fontSize: '0.85rem', color: '#94a3b8', fontWeight: 'normal' }}>
              ({activeCompany?.name || 'OpenTally'})
            </span>
          </h2>
          <div style={{ fontSize: '0.8rem', color: '#94a3b8', marginTop: '4px' }}>
            Period: {fromDate} to {toDate}
          </div>
        </div>

        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <label style={{ fontSize: '0.8rem', color: '#cbd5e1' }}>
            From:
            <input
              type="date"
              value={fromDate}
              onChange={e => setFromDate(e.target.value)}
              style={{ marginLeft: '4px', background: '#1e293b', border: '1px solid #475569', color: '#f8fafc', padding: '4px 8px', borderRadius: '4px' }}
            />
          </label>
          <label style={{ fontSize: '0.8rem', color: '#cbd5e1' }}>
            To:
            <input
              type="date"
              value={toDate}
              onChange={e => setToDate(e.target.value)}
              style={{ marginLeft: '4px', background: '#1e293b', border: '1px solid #475569', color: '#f8fafc', padding: '4px 8px', borderRadius: '4px' }}
            />
          </label>
          <button
            onClick={() => void loadData()}
            style={{ background: '#0284c7', color: '#fff', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600, fontSize: '0.85rem' }}
          >
            Refresh
          </button>
          <button
            onClick={onBack}
            style={{ background: '#334155', color: '#cbd5e1', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.85rem' }}
          >
            Esc: Back
          </button>
        </div>
      </div>

      {error && (
        <div style={{ background: '#7f1d1d', color: '#fecaca', padding: '10px 14px', borderRadius: '4px', marginBottom: '16px' }}>
          {error}
        </div>
      )}

      {loading ? (
        <div style={{ padding: '40px', textAlign: 'center', color: '#94a3b8' }}>
          Calculating Financial Ratios & Group Positions...
        </div>
      ) : report ? (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '20px' }}>
          {/* Left Pane: Principal Groups & Figures */}
          <div style={{ background: '#1e293b', borderRadius: '6px', border: '1px solid #334155', overflow: 'hidden' }}>
            <div style={{ background: '#0f172a', padding: '10px 16px', borderBottom: '1px solid #334155', fontWeight: 600, color: '#e2e8f0', display: 'flex', justifyContent: 'space-between' }}>
              <span>Principal Groups & Figures</span>
              <span style={{ fontSize: '0.8rem', color: '#94a3b8' }}>Amounts</span>
            </div>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.875rem' }}>
              <tbody>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Working Capital</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#38bdf8' }}>
                    {formatAmount(report.working_capital)}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#94a3b8', paddingLeft: '28px' }}>Cash-in-hand</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.cash_in_hand)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#94a3b8', paddingLeft: '28px' }}>Bank Accounts</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.bank_accounts)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#94a3b8', paddingLeft: '28px' }}>Bank OD A/c</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.bank_od_ac)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Sundry Debtors (Receivables)</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.sundry_debtors)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Sundry Creditors (Payables)</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.sundry_creditors)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Sales Accounts (Turnover)</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#34d399', fontWeight: 600 }}>{formatAmount(report.sales_accounts)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Purchase Accounts</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#f87171' }}>{formatAmount(report.purchase_accounts)}</td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px', color: '#cbd5e1', fontWeight: 500 }}>Closing Stock (Inventory)</td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', color: '#e2e8f0' }}>{formatAmount(report.closing_stock)}</td>
                </tr>
                <tr style={{ background: '#0f172a' }}>
                  <td style={{ padding: '12px 16px', color: '#38bdf8', fontWeight: 700 }}>Net Profit / (Loss)</td>
                  <td style={{ padding: '12px 16px', textAlign: 'right', fontWeight: 700, color: parseFloat(report.net_profit) >= 0 ? '#34d399' : '#f87171', fontSize: '1rem' }}>
                    {formatAmount(report.net_profit)}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          {/* Right Pane: Principal Ratios */}
          <div style={{ background: '#1e293b', borderRadius: '6px', border: '1px solid #334155', overflow: 'hidden' }}>
            <div style={{ background: '#0f172a', padding: '10px 16px', borderBottom: '1px solid #334155', fontWeight: 600, color: '#e2e8f0', display: 'flex', justifyContent: 'space-between' }}>
              <span>Principal Financial Ratios</span>
              <span style={{ fontSize: '0.8rem', color: '#94a3b8' }}>Formula / Ratio</span>
            </div>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.875rem' }}>
              <tbody>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Current Ratio</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Current Assets ÷ Current Liabilities (Norm: 2.0 : 1)</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 700, color: '#38bdf8', fontSize: '1rem' }}>
                    {report.current_ratio}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Quick Ratio (Acid Test)</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Quick Assets ÷ Current Liabilities (Norm: 1.0 : 1)</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 700, color: '#38bdf8', fontSize: '1rem' }}>
                    {report.quick_ratio}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Debt / Equity Ratio</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Total Debt ÷ Net Worth</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#e2e8f0' }}>
                    {report.debt_equity_ratio}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Gross Profit %</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Gross Profit ÷ Total Sales × 100</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#34d399' }}>
                    {report.gross_profit_pct}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Net Profit %</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Net Profit ÷ Total Sales × 100</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#34d399' }}>
                    {report.net_profit_pct}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Operating Cost %</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Operating Costs ÷ Net Sales × 100</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#fbbf24' }}>
                    {report.operating_cost_pct}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Receivables Turnover (Days)</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Average Collection Period</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#e2e8f0' }}>
                    {report.receivables_turnover_days}
                  </td>
                </tr>
                <tr style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '10px 16px' }}>
                    <div style={{ fontWeight: 600, color: '#cbd5e1' }}>Inventory Turnover</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>COGS ÷ Closing Stock</div>
                  </td>
                  <td style={{ padding: '10px 16px', textAlign: 'right', fontWeight: 600, color: '#e2e8f0' }}>
                    {report.inventory_turnover}
                  </td>
                </tr>
                <tr style={{ background: '#0f172a' }}>
                  <td style={{ padding: '12px 16px' }}>
                    <div style={{ fontWeight: 700, color: '#38bdf8' }}>Return on Working Capital %</div>
                    <div style={{ fontSize: '0.75rem', color: '#64748b' }}>Net Profit ÷ Working Capital × 100</div>
                  </td>
                  <td style={{ padding: '12px 16px', textAlign: 'right', fontWeight: 700, color: '#38bdf8', fontSize: '1rem' }}>
                    {report.return_on_working_capital_pct}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      ) : null}
    </div>
  );
}
