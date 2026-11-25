import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Company } from '../../types';
import { listCompanies, selectCompany, createSampleCompany } from '../../services/api';
import './SelectCompanyView.css';

export interface SelectCompanyViewProps {
  activeCompanyId: string | null;
  onCompanySelected: (company: Company) => void;
  onCreateCompany: () => void;
  onImportXml: () => void;
  onRestoreBackup: () => void;
  onClose?: () => void;
  isStandalone?: boolean;
}

export const SelectCompanyView: React.FC<SelectCompanyViewProps> = ({
  activeCompanyId,
  onCompanySelected,
  onCreateCompany,
  onImportXml,
  onRestoreBackup,
  onClose,
  isStandalone = false,
}) => {
  const [companies, setCompanies] = useState<Company[]>([]);
  const [loading, setLoading] = useState(false);
  const [sampleLoading, setSampleLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filterText, setFilterText] = useState('');
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filterInputRef = useRef<HTMLInputElement>(null);
  const tableContainerRef = useRef<HTMLDivElement>(null);

  const fetchCompanies = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listCompanies();
      setCompanies(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCompanies();
    filterInputRef.current?.focus();
  }, [fetchCompanies]);

  const filtered = companies.filter((c) => {
    if (!filterText.trim()) return true;
    const q = filterText.toLowerCase();
    return (
      c.name.toLowerCase().includes(q) ||
      (c.state && c.state.toLowerCase().includes(q)) ||
      (c.financial_year_begins && c.financial_year_begins.includes(q))
    );
  });

  // Keep selected index within bounds
  useEffect(() => {
    if (selectedIndex >= filtered.length) {
      setSelectedIndex(Math.max(0, filtered.length - 1));
    }
  }, [filtered.length, selectedIndex]);

  const handleSelect = useCallback(
    async (comp: Company) => {
      try {
        const selected = await selectCompany(comp.id);
        onCompanySelected(selected);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [onCompanySelected]
  );

  const handleLoadSample = useCallback(async () => {
    setSampleLoading(true);
    setError(null);
    try {
      const sample = await createSampleCompany();
      await fetchCompanies();
      onCompanySelected(sample);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSampleLoading(false);
    }
  }, [fetchCompanies, onCompanySelected]);

  // Global Keyboard Navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA');

      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        if (onClose) onClose();
        return;
      }

      if (!isInput) {
        const key = e.key.toUpperCase();
        if (key === 'C') {
          e.preventDefault();
          onCreateCompany();
          return;
        } else if (key === 'S') {
          e.preventDefault();
          handleLoadSample();
          return;
        } else if (key === 'I') {
          e.preventDefault();
          onImportXml();
          return;
        } else if (key === 'R') {
          e.preventDefault();
          onRestoreBackup();
          return;
        }
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev < filtered.length - 1 ? prev + 1 : prev));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : 0));
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (filtered.length > 0 && selectedIndex < filtered.length) {
          handleSelect(filtered[selectedIndex]);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [filtered, selectedIndex, handleSelect, onCreateCompany, handleLoadSample, onImportXml, onRestoreBackup, onClose]);

  // Scroll selected item into view
  useEffect(() => {
    if (tableContainerRef.current) {
      const activeEl = tableContainerRef.current.querySelector('.ot-company-row-selected');
      if (activeEl && typeof activeEl.scrollIntoView === 'function') {
        activeEl.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [selectedIndex]);

  return (
    <div
      className={`ot-select-company-hub ${isStandalone ? 'ot-hub-standalone' : ''}`}
      data-testid="select-company-view"
    >
      {/* Top Title Bar */}
      <div className="ot-hub-title-bar">
        <div className="ot-hub-title-left">
          <h2>Select Company</h2>
          <span className="ot-hub-subtitle">TallyPrime Startup Hub & Company Selector</span>
        </div>
        {onClose && (
          <button className="ot-btn-close" onClick={onClose} title="Back / Close (Esc)">
            &times;
          </button>
        )}
      </div>

      {/* Action Keys Bar */}
      <div className="ot-hub-actions-bar" role="toolbar" aria-label="Company startup actions">
        <button
          type="button"
          className="ot-action-btn"
          onClick={onCreateCompany}
          title="Create New Company [C]"
          data-testid="btn-create-company"
        >
          <span className="ot-action-key">C</span>
          <span className="ot-action-label">Create Company</span>
        </button>

        <button
          type="button"
          className="ot-action-btn ot-action-btn-highlight"
          onClick={handleLoadSample}
          disabled={sampleLoading}
          title="Load Sample Indian SME Company [S]"
          data-testid="btn-load-sample"
        >
          <span className="ot-action-key">S</span>
          <span className="ot-action-label">
            {sampleLoading ? 'Loading Sample...' : 'Load Sample Indian SME Company'}
          </span>
        </button>

        <button
          type="button"
          className="ot-action-btn"
          onClick={onImportXml}
          title="Import from Tally XML [I]"
          data-testid="btn-import-xml"
        >
          <span className="ot-action-key">I</span>
          <span className="ot-action-label">Import from Tally XML</span>
        </button>

        <button
          type="button"
          className="ot-action-btn"
          onClick={onRestoreBackup}
          title="Restore Backup [R]"
          data-testid="btn-restore-backup"
        >
          <span className="ot-action-key">R</span>
          <span className="ot-action-label">Restore Backup</span>
        </button>
      </div>

      {error && <div className="ot-hub-error" role="alert">{error}</div>}

      {/* Filter Input */}
      <div className="ot-hub-filter-box">
        <label htmlFor="company-filter-input">Search / Filter Company:</label>
        <input
          id="company-filter-input"
          ref={filterInputRef}
          type="text"
          placeholder="Type to filter companies... (Arrow keys to navigate, Enter to select)"
          value={filterText}
          onChange={(e) => {
            setFilterText(e.target.value);
            setSelectedIndex(0);
          }}
          autoFocus
        />
      </div>

      {/* Companies List Table */}
      <div className="ot-hub-table-wrapper" ref={tableContainerRef}>
        {loading ? (
          <div className="ot-loading-text">Loading companies from database...</div>
        ) : filtered.length === 0 ? (
          <div className="ot-hub-empty">
            <p>No companies found matching &apos;{filterText}&apos;.</p>
            <div className="ot-hub-empty-actions">
              <button className="ot-btn ot-btn-primary" onClick={onCreateCompany}>
                Create Company (C)
              </button>
              <button className="ot-btn ot-btn-accent" onClick={handleLoadSample} disabled={sampleLoading}>
                Load Sample Indian SME (S)
              </button>
            </div>
          </div>
        ) : (
          <table className="ot-hub-table">
            <thead>
              <tr>
                <th style={{ width: '45%' }}>Company Name</th>
                <th style={{ width: '25%' }}>Financial Year</th>
                <th style={{ width: '20%' }}>State</th>
                <th style={{ width: '10%' }}>Status</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((c, idx) => {
                const isSelected = idx === selectedIndex;
                const isActive = c.id === activeCompanyId;
                return (
                  <tr
                    key={c.id}
                    className={`ot-hub-row ${isSelected ? 'ot-company-row-selected' : ''} ${
                      isActive ? 'ot-company-row-active' : ''
                    }`}
                    onClick={() => {
                      setSelectedIndex(idx);
                      handleSelect(c);
                    }}
                    onMouseEnter={() => setSelectedIndex(idx)}
                  >
                    <td className="ot-col-name">
                      <div className="ot-name-flex">
                        {isSelected && <span className="ot-row-caret">▶</span>}
                        <span className="ot-comp-name">{c.name}</span>
                        {isActive && <span className="ot-active-pill">ACTIVE</span>}
                      </div>
                    </td>
                    <td>{c.financial_year_begins || '2025-04-01'}</td>
                    <td>{c.state || 'Karnataka'}</td>
                    <td>
                      <span className={`ot-status-badge ot-status-${c.status || 'active'}`}>
                        {c.status || 'active'}
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {/* Footer Navigation Bar */}
      <div className="ot-hub-footer">
        <div className="ot-hub-footer-shortcuts">
          <span><code>↑ / ↓</code>: Navigate</span>
          <span><code>Enter</code>: Select</span>
          <span><code>C</code>: Create</span>
          <span><code>S</code>: Sample Demo</span>
          <span><code>I</code>: Import XML</span>
          <span><code>R</code>: Restore</span>
          <span><code>Esc</code>: Exit</span>
        </div>
        <div className="ot-hub-footer-count">
          Showing {filtered.length} of {companies.length} companies
        </div>
      </div>
    </div>
  );
};
