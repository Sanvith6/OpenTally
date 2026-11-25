import { useCallback, useEffect, useState } from 'react';
import { Budget, BudgetLineRequest, BudgetRequest, BudgetVarianceReport, Company } from '../../types';
import { createBudget, deleteBudget, getBudgetVarianceReport, listBudgets } from '../../services/api';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function BudgetsView({ activeCompany, onBack }: Props) {
  const [budgets, setBudgets] = useState<Budget[]>([]);
  const [selectedBudgetId, setSelectedBudgetId] = useState<string | null>(null);
  const [varianceReport, setVarianceReport] = useState<BudgetVarianceReport | null>(null);
  const [asOfDate, setAsOfDate] = useState<string>(new Date().toISOString().slice(0, 10));

  const [isCreating, setIsCreating] = useState(false);
  const [newBudgetName, setNewBudgetName] = useState('');
  const [newFromDate, setNewFromDate] = useState(activeCompany?.financial_year_begins || '2024-04-01');
  const [newToDate, setNewToDate] = useState('2025-03-31');
  const [newLines, setNewLines] = useState<BudgetLineRequest[]>([
    {
      target_type: 'Group',
      target_id: 'grp_indirect_expenses',
      budget_type: 'OnNetTransactions',
      amount: '50000.00',
      dr_cr: 'Dr',
    },
  ]);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchBudgets = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const list = await listBudgets(activeCompany.id);
      setBudgets(list);
      if (list.length > 0 && !selectedBudgetId) {
        setSelectedBudgetId(list[0].id);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, selectedBudgetId]);

  useEffect(() => {
    void fetchBudgets();
  }, [fetchBudgets]);

  // Load variance report when selected budget or as_of_date changes
  const loadVariance = useCallback(async (bid: string, dt: string) => {
    if (!activeCompany) return;
    try {
      const rep = await getBudgetVarianceReport(bid, dt, activeCompany.id);
      setVarianceReport(rep);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [activeCompany]);

  useEffect(() => {
    if (selectedBudgetId && !isCreating) {
      void loadVariance(selectedBudgetId, asOfDate);
    }
  }, [selectedBudgetId, asOfDate, isCreating, loadVariance]);

  // Esc hotkey
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        if (isCreating) {
          setIsCreating(false);
        } else {
          onBack();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isCreating, onBack]);

  const handleCreateSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    if (!newBudgetName.trim()) {
      setError('Budget name is required');
      return;
    }
    if (newLines.length === 0) {
      setError('At least one budget line is required');
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const req: BudgetRequest = {
        name: newBudgetName.trim(),
        from_date: newFromDate,
        to_date: newToDate,
        lines: newLines,
      };
      const created = await createBudget(req, activeCompany.id);
      setIsCreating(false);
      setNewBudgetName('');
      await fetchBudgets();
      setSelectedBudgetId(created.budget.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteBudget = async (id: string) => {
    if (!confirm('Are you sure you want to delete this budget?')) return;
    setLoading(true);
    try {
      await deleteBudget(id, activeCompany?.id);
      setSelectedBudgetId(null);
      setVarianceReport(null);
      await fetchBudgets();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleAddLine = () => {
    setNewLines([
      ...newLines,
      {
        target_type: 'Ledger',
        target_id: '',
        budget_type: 'OnNetTransactions',
        amount: '10000.00',
        dr_cr: 'Dr',
      },
    ]);
  };

  const handleRemoveLine = (idx: number) => {
    setNewLines(newLines.filter((_, i) => i !== idx));
  };

  return (
    <div style={{ padding: '16px', background: '#0f172a', color: '#f8fafc', height: '100%', overflowY: 'auto' }}>
      {/* Top Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', borderBottom: '1px solid #334155', paddingBottom: '12px', marginBottom: '16px' }}>
        <div>
          <h2 style={{ margin: 0, fontSize: '1.25rem', color: '#38bdf8', display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span>Budgets & Variance Analysis</span>
            <span style={{ fontSize: '0.85rem', color: '#94a3b8', fontWeight: 'normal' }}>
              ({activeCompany?.name || 'OpenTally'})
            </span>
          </h2>
          <div style={{ fontSize: '0.8rem', color: '#94a3b8', marginTop: '4px' }}>
            Manage departmental, group, and ledger budgets against actual financial transactions
          </div>
        </div>

        <div style={{ display: 'flex', gap: '12px' }}>
          {!isCreating && (
            <button
              onClick={() => setIsCreating(true)}
              style={{ background: '#059669', color: '#fff', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600, fontSize: '0.85rem' }}
            >
              + Create Budget (Alt+C)
            </button>
          )}
          <button
            onClick={() => {
              if (isCreating) setIsCreating(false);
              else onBack();
            }}
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

      {isCreating ? (
        /* Create Budget Form */
        <form onSubmit={handleCreateSubmit} style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', padding: '20px', maxWidth: '800px' }}>
          <h3 style={{ margin: '0 0 16px 0', color: '#38bdf8', fontSize: '1.1rem' }}>Define New Budget</h3>

          <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr 1fr', gap: '16px', marginBottom: '16px' }}>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#cbd5e1', marginBottom: '4px' }}>Budget Name *</label>
              <input
                type="text"
                value={newBudgetName}
                onChange={e => setNewBudgetName(e.target.value)}
                placeholder="e.g. FY 2024-25 Marketing & Operations"
                required
                style={{ width: '100%', padding: '8px 12px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#cbd5e1', marginBottom: '4px' }}>From Date</label>
              <input
                type="date"
                value={newFromDate}
                onChange={e => setNewFromDate(e.target.value)}
                required
                style={{ width: '100%', padding: '8px 12px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#cbd5e1', marginBottom: '4px' }}>To Date</label>
              <input
                type="date"
                value={newToDate}
                onChange={e => setNewToDate(e.target.value)}
                required
                style={{ width: '100%', padding: '8px 12px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
              />
            </div>
          </div>

          <h4 style={{ margin: '16px 0 8px 0', color: '#94a3b8', fontSize: '0.9rem' }}>Budget Line Items</h4>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
            {newLines.map((line, idx) => (
              <div key={idx} style={{ display: 'grid', gridTemplateColumns: '120px 180px 180px 120px 80px 40px', gap: '8px', alignItems: 'center' }}>
                <select
                  value={line.target_type}
                  onChange={e => {
                    const updated = [...newLines];
                    updated[idx].target_type = e.target.value as any;
                    setNewLines(updated);
                  }}
                  style={{ padding: '6px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                >
                  <option value="Group">Group</option>
                  <option value="Ledger">Ledger</option>
                  <option value="CostCenter">Cost Centre</option>
                </select>

                <input
                  type="text"
                  placeholder="Target ID or Name"
                  value={line.target_id}
                  onChange={e => {
                    const updated = [...newLines];
                    updated[idx].target_id = e.target.value;
                    setNewLines(updated);
                  }}
                  required
                  style={{ padding: '6px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                />

                <select
                  value={line.budget_type}
                  onChange={e => {
                    const updated = [...newLines];
                    updated[idx].budget_type = e.target.value as any;
                    setNewLines(updated);
                  }}
                  style={{ padding: '6px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                >
                  <option value="OnNetTransactions">On Net Transactions</option>
                  <option value="OnClosingBalance">On Closing Balance</option>
                </select>

                <input
                  type="text"
                  placeholder="Amount"
                  value={String(line.amount)}
                  onChange={e => {
                    const updated = [...newLines];
                    updated[idx].amount = e.target.value;
                    setNewLines(updated);
                  }}
                  required
                  style={{ padding: '6px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                />

                <select
                  value={line.dr_cr}
                  onChange={e => {
                    const updated = [...newLines];
                    updated[idx].dr_cr = e.target.value as any;
                    setNewLines(updated);
                  }}
                  style={{ padding: '6px', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', borderRadius: '4px' }}
                >
                  <option value="Dr">Dr</option>
                  <option value="Cr">Cr</option>
                </select>

                {newLines.length > 1 && (
                  <button
                    type="button"
                    onClick={() => handleRemoveLine(idx)}
                    style={{ background: '#7f1d1d', color: '#fecaca', border: 'none', borderRadius: '4px', cursor: 'pointer', height: '32px' }}
                  >
                    ×
                  </button>
                )}
              </div>
            ))}
          </div>

          <div style={{ display: 'flex', gap: '12px', marginTop: '16px' }}>
            <button
              type="button"
              onClick={handleAddLine}
              style={{ background: '#334155', color: '#e2e8f0', border: 'none', padding: '6px 14px', borderRadius: '4px', cursor: 'pointer', fontSize: '0.85rem' }}
            >
              + Add Target Line
            </button>
            <div style={{ flex: 1 }} />
            <button
              type="button"
              onClick={() => setIsCreating(false)}
              style={{ background: '#475569', color: '#fff', border: 'none', padding: '6px 16px', borderRadius: '4px', cursor: 'pointer' }}
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading}
              style={{ background: '#0284c7', color: '#fff', border: 'none', padding: '6px 20px', borderRadius: '4px', cursor: 'pointer', fontWeight: 600 }}
            >
              {loading ? 'Saving...' : 'Save Budget'}
            </button>
          </div>
        </form>
      ) : (
        /* Main View: Left List + Right Variance Report */
        <div style={{ display: 'grid', gridTemplateColumns: '280px 1fr', gap: '20px' }}>
          {/* Left: Budgets List */}
          <div style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', overflow: 'hidden' }}>
            <div style={{ background: '#0f172a', padding: '10px 16px', borderBottom: '1px solid #334155', fontWeight: 600, color: '#e2e8f0' }}>
              Budgets Master
            </div>
            {budgets.length === 0 ? (
              <div style={{ padding: '24px 16px', textAlign: 'center', color: '#64748b' }}>
                No budgets configured. Click "+ Create Budget" above to set up financial targets.
              </div>
            ) : (
              <div>
                {budgets.map(b => (
                  <div
                    key={b.id}
                    onClick={() => setSelectedBudgetId(b.id)}
                    style={{
                      padding: '12px 16px',
                      borderBottom: '1px solid #334155',
                      cursor: 'pointer',
                      background: selectedBudgetId === b.id ? '#334155' : 'transparent',
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                    }}
                  >
                    <div>
                      <div style={{ fontWeight: 600, color: selectedBudgetId === b.id ? '#38bdf8' : '#e2e8f0' }}>{b.name}</div>
                      <div style={{ fontSize: '0.75rem', color: '#94a3b8', marginTop: '2px' }}>
                        {b.from_date} to {b.to_date}
                      </div>
                    </div>
                    <button
                      onClick={e => {
                        e.stopPropagation();
                        void handleDeleteBudget(b.id);
                      }}
                      title="Delete Budget"
                      style={{ background: 'transparent', border: 'none', color: '#94a3b8', cursor: 'pointer', fontSize: '0.85rem' }}
                    >
                      🗑
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Right: Variance Analysis Report */}
          <div style={{ background: '#1e293b', border: '1px solid #334155', borderRadius: '6px', overflow: 'hidden' }}>
            <div style={{ background: '#0f172a', padding: '10px 16px', borderBottom: '1px solid #334155', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <span style={{ fontWeight: 600, color: '#e2e8f0' }}>
                Variance Analysis: {varianceReport?.budget_name || 'Select Budget'}
              </span>
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                <span style={{ fontSize: '0.8rem', color: '#94a3b8' }}>As of Date:</span>
                <input
                  type="date"
                  value={asOfDate}
                  onChange={e => setAsOfDate(e.target.value)}
                  style={{ background: '#1e293b', border: '1px solid #475569', color: '#f8fafc', padding: '3px 8px', borderRadius: '4px', fontSize: '0.8rem' }}
                />
              </div>
            </div>

            {varianceReport ? (
              <div>
                <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.875rem' }}>
                  <thead>
                    <tr style={{ background: '#0f172a', borderBottom: '1px solid #334155', color: '#94a3b8', textAlign: 'left' }}>
                      <th style={{ padding: '8px 16px' }}>Target Name</th>
                      <th style={{ padding: '8px 12px' }}>Type</th>
                      <th style={{ padding: '8px 12px', textAlign: 'right' }}>Budget Amount</th>
                      <th style={{ padding: '8px 12px', textAlign: 'right' }}>Actual Amount</th>
                      <th style={{ padding: '8px 12px', textAlign: 'right' }}>Variance</th>
                      <th style={{ padding: '8px 12px', textAlign: 'center' }}>Utilization</th>
                      <th style={{ padding: '8px 16px', textAlign: 'center' }}>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {varianceReport.rows.map(r => (
                      <tr key={r.line_id} style={{ borderBottom: '1px solid #334155' }}>
                        <td style={{ padding: '10px 16px', fontWeight: 500, color: '#e2e8f0' }}>
                          {r.target_name} ({r.target_id})
                        </td>
                        <td style={{ padding: '10px 12px', color: '#94a3b8' }}>{r.target_type}</td>
                        <td style={{ padding: '10px 12px', textAlign: 'right', color: '#e2e8f0' }}>₹{r.budget_amount}</td>
                        <td style={{ padding: '10px 12px', textAlign: 'right', color: '#38bdf8', fontWeight: 600 }}>₹{r.actual_amount}</td>
                        <td style={{ padding: '10px 12px', textAlign: 'right', color: r.is_over_budget ? '#f87171' : '#34d399', fontWeight: 600 }}>
                          ₹{r.variance_amount}
                        </td>
                        <td style={{ padding: '10px 12px', textAlign: 'center' }}>
                          <span style={{ fontSize: '0.8rem', color: '#cbd5e1' }}>{r.utilization_pct}</span>
                        </td>
                        <td style={{ padding: '10px 16px', textAlign: 'center' }}>
                          {r.is_over_budget ? (
                            <span style={{ background: '#7f1d1d', color: '#fecaca', fontSize: '0.75rem', padding: '2px 8px', borderRadius: '4px', fontWeight: 600 }}>
                              OVER BUDGET
                            </span>
                          ) : (
                            <span style={{ background: '#064e3b', color: '#a7f3d0', fontSize: '0.75rem', padding: '2px 8px', borderRadius: '4px' }}>
                              WITHIN
                            </span>
                          )}
                        </td>
                      </tr>
                    ))}
                    {/* Summary row */}
                    <tr style={{ background: '#0f172a', fontWeight: 700 }}>
                      <td colSpan={2} style={{ padding: '12px 16px', color: '#38bdf8' }}>TOTALS</td>
                      <td style={{ padding: '12px 12px', textAlign: 'right', color: '#e2e8f0' }}>₹{varianceReport.total_budget}</td>
                      <td style={{ padding: '12px 12px', textAlign: 'right', color: '#38bdf8' }}>₹{varianceReport.total_actual}</td>
                      <td style={{ padding: '12px 12px', textAlign: 'right', color: '#34d399' }}>₹{varianceReport.total_variance}</td>
                      <td colSpan={2} />
                    </tr>
                  </tbody>
                </table>
              </div>
            ) : (
              <div style={{ padding: '40px', textAlign: 'center', color: '#64748b' }}>
                Select a budget from the left panel to display its variance analysis report.
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
