import { useEffect, useState } from 'react';
import { Company } from '../types';
import { listCompanies, selectCompany, archiveCompany } from '../services/api';

interface CompanySelectModalProps {
  isOpen: boolean;
  activeCompanyId: string | null;
  onClose: () => void;
  onCompanySelected: (company: Company) => void;
  onCreateNew: () => void;
}

export function CompanySelectModal({
  isOpen,
  activeCompanyId,
  onClose,
  onCompanySelected,
  onCreateNew,
}: CompanySelectModalProps) {
  const [companies, setCompanies] = useState<Company[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionLoadingId, setActionLoadingId] = useState<string | null>(null);

  const loadCompanies = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await listCompanies();
      setCompanies(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      loadCompanies();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSelect = async (company: Company) => {
    if (company.status !== 'active') {
      setError(`Cannot select ${company.status} company '${company.name}'`);
      return;
    }
    setActionLoadingId(company.id);
    try {
      const selected = await selectCompany(company.id);
      onCompanySelected(selected);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoadingId(null);
    }
  };

  const handleArchive = async (company: Company, e: React.MouseEvent) => {
    e.stopPropagation();
    const confirmed = window.confirm(
      `Are you sure you want to archive company '${company.name}'?\nArchived companies cannot be selected for daily voucher entries.`
    );
    if (!confirmed) return;

    setActionLoadingId(company.id);
    try {
      await archiveCompany(company.id);
      await loadCompanies();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setActionLoadingId(null);
    }
  };

  return (
    <div className="ot-modal-backdrop" onClick={onClose}>
      <div className="ot-modal-content ot-modal-lg" onClick={(e) => e.stopPropagation()}>
        <div className="ot-modal-header">
          <h2>Select Company (F3)</h2>
          <button className="ot-btn-close" onClick={onClose}>
            &times;
          </button>
        </div>

        {error && <div className="ot-modal-error">{error}</div>}

        <div className="ot-company-list-container">
          {loading ? (
            <div className="ot-loading-text">Loading companies list...</div>
          ) : companies.length === 0 ? (
            <div className="ot-empty-state">
              <p>No companies found in database.</p>
              <button className="ot-btn ot-btn-primary" onClick={onCreateNew}>
                Create First Company
              </button>
            </div>
          ) : (
            <table className="ot-table">
              <thead>
                <tr>
                  <th>Company Name</th>
                  <th>State / Country</th>
                  <th>Financial Year</th>
                  <th>Currency</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {companies.map((c) => {
                  const isActive = c.id === activeCompanyId;
                  const isArchived = c.status === 'archived';
                  return (
                    <tr
                      key={c.id}
                      className={`ot-clickable-row ${isActive ? 'ot-row-active' : ''}`}
                      onClick={() => handleSelect(c)}
                    >
                      <td className="ot-fw-bold">
                        {c.name}
                        {isActive && <span className="ot-badge-active">ACTIVE</span>}
                      </td>
                      <td>
                        {c.state}, {c.country}
                      </td>
                      <td>{c.financial_year_begins}</td>
                      <td>
                        {c.currency_formal_name} ({c.currency_symbol})
                      </td>
                      <td>
                        <span className={`ot-status-pill ot-status-${c.status}`}>
                          {c.status.toUpperCase()}
                        </span>
                      </td>
                      <td>
                        <div className="ot-action-buttons" onClick={(e) => e.stopPropagation()}>
                          {!isArchived && (
                            <button
                              className="ot-btn ot-btn-sm ot-btn-warning"
                              onClick={(e) => handleArchive(c, e)}
                              disabled={actionLoadingId === c.id}
                              title="Archive company"
                            >
                              Archive
                            </button>
                          )}
                          <button
                            className="ot-btn ot-btn-sm ot-btn-primary"
                            onClick={() => handleSelect(c)}
                            disabled={actionLoadingId === c.id || isArchived}
                          >
                            {isActive ? 'Current' : 'Select'}
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        <div className="ot-modal-footer">
          <button className="ot-btn ot-btn-primary" onClick={onCreateNew}>
            <kbd>Alt+F3</kbd> Create New Company
          </button>
          <button className="ot-btn ot-btn-secondary" onClick={onClose}>
            Close <kbd>Esc</kbd>
          </button>
        </div>
      </div>
    </div>
  );
}

