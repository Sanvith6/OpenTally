import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Company } from '../../types';
import {
  listCompanies,
  selectCompany,
  createSampleCompany,
  testLanHealth,
  loginLanClient,
} from '../../services/api';
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

  // TallyPrime Gold LAN Client Connection State
  const [showLanPanel, setShowLanPanel] = useState(false);
  const [lanHost, setLanHost] = useState('127.0.0.1');
  const [lanPort, setLanPort] = useState('7900');
  const [lanHealthOk, setLanHealthOk] = useState(false);
  const [lanCompanies, setLanCompanies] = useState<Company[]>([]);
  const [lanCompanyId, setLanCompanyId] = useState('');
  const [lanUsername, setLanUsername] = useState('');
  const [lanPassword, setLanPassword] = useState('');
  const [lanLoading, setLanLoading] = useState(false);
  const [lanError, setLanError] = useState<string | null>(null);

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

  const handleTestLanHealth = async () => {
    const portNum = parseInt(lanPort, 10) || 7900;
    setLanLoading(true);
    setLanError(null);
    setLanHealthOk(false);
    try {
      const health = await testLanHealth(lanHost.trim(), portNum);
      if (health.status !== 'ok') {
        throw new Error(`Unexpected LAN health status: ${health.status}`);
      }
      setLanHealthOk(true);
      const res = await fetch(`http://${lanHost.trim()}:${portNum}/api/v1/companies`);
      if (res.ok) {
        const list: Company[] = await res.json();
        setLanCompanies(list);
        if (list.length > 0 && !lanCompanyId) {
          setLanCompanyId(list[0].id);
        }
      }
    } catch (err) {
      setLanError(err instanceof Error ? err.message : String(err));
    } finally {
      setLanLoading(false);
    }
  };

  const handleConnectLan = async (e: React.FormEvent) => {
    e.preventDefault();
    const portNum = parseInt(lanPort, 10) || 7900;
    if (!lanCompanyId.trim()) {
      setLanError('Please select or enter a Company ID.');
      return;
    }
    setLanLoading(true);
    setLanError(null);
    try {
      await loginLanClient(
        lanHost.trim(),
        portNum,
        lanCompanyId.trim(),
        lanUsername.trim() || 'lan_user',
        lanPassword || undefined
      );
      const matched = lanCompanies.find((c) => c.id === lanCompanyId.trim()) || {
        id: lanCompanyId.trim(),
        name: `LAN: ${lanCompanyId.trim()}`,
        mailing_name: lanCompanyId.trim(),
        country: 'India',
        state: 'Karnataka',
        financial_year_begins: '2026-04-01',
        books_beginning_from: '2026-04-01',
        currency_symbol: '₹',
        currency_formal_name: 'INR',
        currency_decimal_places: 2,
        status: 'active',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      setShowLanPanel(false);
      onCompanySelected(matched);
    } catch (err) {
      setLanError(err instanceof Error ? err.message : String(err));
    } finally {
      setLanLoading(false);
    }
  };

  // Global Keyboard Navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput =
        target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT');

      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        if (showLanPanel) {
          setShowLanPanel(false);
        } else if (onClose) {
          onClose();
        }
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
        } else if (key === 'L') {
          e.preventDefault();
          setShowLanPanel((prev) => !prev);
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
  }, [filtered, selectedIndex, handleSelect, onCreateCompany, handleLoadSample, onImportXml, onRestoreBackup, onClose, showLanPanel]);

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

        <button
          type="button"
          className={`ot-action-btn ${showLanPanel ? 'ot-action-btn-highlight' : ''}`}
          onClick={() => setShowLanPanel((prev) => !prev)}
          title="Connect to LAN Server (TallyPrime Gold) [L]"
          data-testid="btn-connect-lan"
        >
          <span className="ot-action-key">L</span>
          <span className="ot-action-label">Connect to LAN Server</span>
        </button>
      </div>

      {showLanPanel && (
        <form className="ot-hub-lan-panel" onSubmit={handleConnectLan} data-testid="lan-connect-panel">
          <div className="ot-hub-lan-header">
            <strong>Connect to OpenTally LAN Host (TallyPrime Gold)</strong>
            <button
              type="button"
              className="ot-btn-close"
              onClick={() => setShowLanPanel(false)}
              aria-label="Close LAN Connect Panel"
            >
              &times;
            </button>
          </div>
          {lanError && (
            <div className="ot-hub-error" role="alert">
              {lanError}
            </div>
          )}
          <div className="ot-hub-lan-grid">
            <div>
              <label htmlFor="lan-client-host">Host IP / Hostname</label>
              <input
                id="lan-client-host"
                type="text"
                value={lanHost}
                onChange={(e) => setLanHost(e.target.value)}
                placeholder="192.168.1.10"
              />
            </div>
            <div>
              <label htmlFor="lan-client-port">Port (Default 7900)</label>
              <input
                id="lan-client-port"
                type="number"
                value={lanPort}
                onChange={(e) => setLanPort(e.target.value)}
              />
            </div>
            <div style={{ display: 'flex', alignItems: 'flex-end' }}>
              <button
                type="button"
                className="ot-btn ot-btn-secondary"
                onClick={handleTestLanHealth}
                disabled={lanLoading}
                data-testid="btn-test-lan-health"
              >
                {lanLoading ? 'Testing...' : 'Test Health'}
              </button>
            </div>
          </div>

          {lanHealthOk && (
            <div style={{ color: '#10b981', fontSize: '0.85rem', marginTop: '6px' }}>
              LAN Host reachable at http://{lanHost}:{lanPort}
            </div>
          )}

          <div className="ot-hub-lan-grid" style={{ marginTop: '10px' }}>
            <div>
              <label htmlFor="lan-client-company">Company</label>
              {lanCompanies.length > 0 ? (
                <select
                  id="lan-client-company"
                  value={lanCompanyId}
                  onChange={(e) => setLanCompanyId(e.target.value)}
                >
                  {lanCompanies.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name} ({c.id})
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  id="lan-client-company"
                  type="text"
                  value={lanCompanyId}
                  onChange={(e) => setLanCompanyId(e.target.value)}
                  placeholder="Enter Company ID"
                />
              )}
            </div>
            <div>
              <label htmlFor="lan-client-user">Username</label>
              <input
                id="lan-client-user"
                type="text"
                value={lanUsername}
                onChange={(e) => setLanUsername(e.target.value)}
                placeholder="admin"
              />
            </div>
            <div>
              <label htmlFor="lan-client-pass">Password</label>
              <input
                id="lan-client-pass"
                type="password"
                value={lanPassword}
                onChange={(e) => setLanPassword(e.target.value)}
                placeholder="Optional if security off"
              />
            </div>
          </div>

          <div style={{ marginTop: '12px', display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
            <button
              type="button"
              className="ot-btn ot-btn-secondary"
              onClick={() => setShowLanPanel(false)}
            >
              Cancel (Esc)
            </button>
            <button
              type="submit"
              className="ot-btn ot-btn-primary"
              disabled={lanLoading}
              data-testid="btn-submit-lan-login"
            >
              {lanLoading ? 'Connecting...' : 'Connect & Login'}
            </button>
          </div>
        </form>
      )}

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
