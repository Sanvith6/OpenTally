import { useCallback, useEffect, useState } from 'react';
import { Company, CostCategory, CostCategoryRequest, CostCentre, CostCentreRequest } from '../../types';
import { createCostCategory, createCostCentre, listCostCategories, listCostCentres } from '../../services/api';
import '../reports/Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
}

export function CostCentresView({ activeCompany, onBack }: Props) {
  const [categories, setCategories] = useState<CostCategory[]>([]);
  const [selectedCatId, setSelectedCatId] = useState<string>('');
  const [centres, setCentres] = useState<CostCentre[]>([]);

  // Category form state
  const [showCatModal, setShowCatModal] = useState(false);
  const [catName, setCatName] = useState('');
  const [allocRevenue, setAllocRevenue] = useState(true);
  const [allocNonRevenue, setAllocNonRevenue] = useState(false);

  // Centre form state
  const [showCentreModal, setShowCentreModal] = useState(false);
  const [centreName, setCentreName] = useState('');
  const [parentId, setParentId] = useState<string>('');

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const loadCategories = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const cats = await listCostCategories();
      setCategories(cats);
      if (cats.length > 0 && !selectedCatId) {
        setSelectedCatId(cats[0].id);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, selectedCatId]);

  const loadCentres = useCallback(async () => {
    if (!activeCompany || !selectedCatId) return;
    try {
      const c = await listCostCentres(selectedCatId);
      setCentres(c);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [activeCompany, selectedCatId]);

  useEffect(() => {
    void loadCategories();
  }, [loadCategories]);

  useEffect(() => {
    if (selectedCatId) {
      void loadCentres();
    }
  }, [loadCentres, selectedCatId]);

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

  const handleCreateCategory = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!catName.trim()) return;
    setError(null);
    try {
      const req: CostCategoryRequest = {
        name: catName.trim(),
        allocate_revenue: allocRevenue,
        allocate_non_revenue: allocNonRevenue,
      };
      const created = await createCostCategory(req);
      setMessage(`Cost Category '${created.name}' created.`);
      setCatName('');
      setShowCatModal(false);
      await loadCategories();
      setSelectedCatId(created.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCreateCentre = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!centreName.trim() || !selectedCatId) return;
    setError(null);
    try {
      const req: CostCentreRequest = {
        category_id: selectedCatId,
        name: centreName.trim(),
        parent_id: parentId ? parentId : null,
      };
      const created = await createCostCentre(req);
      setMessage(`Cost Centre '${created.name}' created.`);
      setCentreName('');
      setParentId('');
      setShowCentreModal(false);
      await loadCentres();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <section className="ot-report-screen">
      <div className="ot-report-header">
        <div>
          <h2>Cost Categories & Cost Centres</h2>
          <p>{activeCompany?.name || 'Select an active company'}</p>
        </div>
        <button className="ot-report-back" onClick={onBack}>
          Back (Esc)
        </button>
      </div>

      {message && <div className="ot-report-success">{message}</div>}
      {error && <div className="ot-report-error" role="alert">{error}</div>}

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 2fr', gap: '20px' }}>
        {/* Left Column: Cost Categories */}
        <div className="ot-report-card" style={{ padding: '16px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '14px' }}>
            <h3 style={{ margin: 0, fontSize: '16px' }}>Cost Categories</h3>
            <button
              className="ot-report-action"
              style={{ fontSize: '12px', padding: '4px 10px' }}
              onClick={() => setShowCatModal(true)}
            >
              + Category
            </button>
          </div>

          {loading ? (
            <div className="ot-report-loading">Loading categories...</div>
          ) : categories.length === 0 ? (
            <div className="ot-report-empty">No categories created yet.</div>
          ) : (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
              {categories.map(cat => (
                <li
                  key={cat.id}
                  onClick={() => setSelectedCatId(cat.id)}
                  style={{
                    padding: '10px 14px',
                    borderRadius: '6px',
                    cursor: 'pointer',
                    marginBottom: '6px',
                    background: selectedCatId === cat.id ? '#e0f2fe' : '#f8fafc',
                    border: selectedCatId === cat.id ? '1px solid #0284c7' : '1px solid #e2e8f0',
                    fontWeight: selectedCatId === cat.id ? 700 : 500,
                  }}
                >
                  <div>{cat.name}</div>
                  <div style={{ fontSize: '11px', color: '#64748b', marginTop: '2px' }}>
                    Revenue: {cat.allocate_revenue ? 'Yes' : 'No'} • Non-Revenue: {cat.allocate_non_revenue ? 'Yes' : 'No'}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Right Column: Cost Centres for Selected Category */}
        <div className="ot-report-card" style={{ padding: '16px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '14px' }}>
            <div>
              <h3 style={{ margin: 0, fontSize: '16px' }}>Cost Centres</h3>
              <p style={{ margin: '2px 0 0', fontSize: '12px', color: '#64748b' }}>
                Category: <strong>{categories.find(c => c.id === selectedCatId)?.name || 'None selected'}</strong>
              </p>
            </div>
            <button
              className="ot-report-action"
              style={{ fontSize: '12px', padding: '4px 10px' }}
              onClick={() => setShowCentreModal(true)}
              disabled={!selectedCatId}
            >
              + Cost Centre
            </button>
          </div>

          <table className="ot-report-table">
            <thead>
              <tr>
                <th>Cost Centre Name</th>
                <th>Parent Centre</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {centres.length === 0 ? (
                <tr>
                  <td colSpan={3} className="ot-report-empty">No cost centres under this category.</td>
                </tr>
              ) : (
                centres.map(centre => {
                  const parent = centres.find(c => c.id === centre.parent_id);
                  return (
                    <tr key={centre.id}>
                      <td style={{ fontWeight: 600 }}>{centre.name}</td>
                      <td>{parent ? parent.name : '— (Primary)'}</td>
                      <td>
                        <span style={{ color: centre.is_active ? '#166534' : '#991b1b', fontWeight: 600 }}>
                          {centre.is_active ? 'Active' : 'Inactive'}
                        </span>
                      </td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Category Modal */}
      {showCatModal && (
        <div style={{
          position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
          background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000
        }}>
          <form
            onSubmit={handleCreateCategory}
            style={{ background: 'white', padding: '24px', borderRadius: '8px', width: '380px' }}
          >
            <h3 style={{ marginTop: 0 }}>Add Cost Category</h3>
            <div className="ot-report-field" style={{ marginBottom: '12px' }}>
              <label>Category Name</label>
              <input
                autoFocus
                placeholder="e.g. Sales Regions, Departments"
                value={catName}
                onChange={e => setCatName(e.target.value)}
                required
              />
            </div>
            <div style={{ marginBottom: '12px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px' }}>
                <input
                  type="checkbox"
                  checked={allocRevenue}
                  onChange={e => setAllocRevenue(e.target.checked)}
                />
                Allocate Revenue Items (Sales / Income / Expenses)
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px' }}>
                <input
                  type="checkbox"
                  checked={allocNonRevenue}
                  onChange={e => setAllocNonRevenue(e.target.checked)}
                />
                Allocate Non-Revenue Items (Assets / Liabilities)
              </label>
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button
                type="button"
                className="ot-report-back"
                onClick={() => setShowCatModal(false)}
              >
                Cancel
              </button>
              <button type="submit" className="ot-report-action">
                Create Category
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Centre Modal */}
      {showCentreModal && (
        <div style={{
          position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
          background: 'rgba(0,0,0,0.5)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000
        }}>
          <form
            onSubmit={handleCreateCentre}
            style={{ background: 'white', padding: '24px', borderRadius: '8px', width: '380px' }}
          >
            <h3 style={{ marginTop: 0 }}>Add Cost Centre</h3>
            <div className="ot-report-field" style={{ marginBottom: '12px' }}>
              <label>Cost Centre Name</label>
              <input
                autoFocus
                placeholder="e.g. North Region, Marketing, Project Alpha"
                value={centreName}
                onChange={e => setCentreName(e.target.value)}
                required
              />
            </div>
            <div className="ot-report-field" style={{ marginBottom: '16px' }}>
              <label>Parent Centre (Optional)</label>
              <select value={parentId} onChange={e => setParentId(e.target.value)}>
                <option value="">None (Primary Level)</option>
                {centres.map(c => (
                  <option key={c.id} value={c.id}>{c.name}</option>
                ))}
              </select>
            </div>
            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button
                type="button"
                className="ot-report-back"
                onClick={() => setShowCentreModal(false)}
              >
                Cancel
              </button>
              <button type="submit" className="ot-report-action">
                Create Centre
              </button>
            </div>
          </form>
        </div>
      )}
    </section>
  );
}
