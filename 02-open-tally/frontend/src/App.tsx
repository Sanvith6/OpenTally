import { useState, useEffect, useCallback } from 'react';
import { Header } from './components/Header';
import { CompanyPanel } from './components/CompanyPanel';
import { Gateway } from './components/Gateway';
import { StatusBar } from './components/StatusBar';
import { CompanyFormModal } from './components/CompanyFormModal';
import { CompanySelectModal } from './components/CompanySelectModal';
import { ChartOfAccountsView } from './components/ChartOfAccountsView';
import { fetchHealth, getActiveCompany } from './services/api';
import { HealthResponse, Company } from './types';
import './App.css';

export function App() {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);
  const initialView =
    typeof window !== 'undefined' &&
    new URLSearchParams(window.location.search).get('view') === 'chart_of_accounts'
      ? 'chart_of_accounts'
      : 'gateway';
  const [currentView, setCurrentView] = useState<'gateway' | 'chart_of_accounts'>(
    initialView
  );

  // Company management state
  const initialModal =
    typeof window !== 'undefined'
      ? new URLSearchParams(window.location.search).get('modal')
      : null;
  const [activeCompany, setActiveCompany] = useState<Company | null>(null);
  const [isSelectModalOpen, setIsSelectModalOpen] = useState(
    initialModal === 'select'
  );
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(
    initialModal === 'create'
  );

  const loadHealth = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchHealth();
      setHealth(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const loadCompanyContext = useCallback(async () => {
    try {
      const active = await getActiveCompany();
      setActiveCompany(active);
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : 'Failed to retrieve active company context'
      );
    }
  }, []);

  useEffect(() => {
    loadHealth();
    loadCompanyContext();
  }, [loadCompanyContext]);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => {
      setNotification((curr) => (curr === msg ? null : curr));
    }, 4000);
  };

  // Keyboard shortcut listener for accounting shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ignore if typing in an input or textarea
      const target = e.target as HTMLElement | null;
      const isInput =
        target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA');

      if (e.key === 'F3' && e.altKey) {
        e.preventDefault();
        setIsSelectModalOpen(false);
        setIsCreateModalOpen(true);
      } else if (e.key === 'F3' && !e.altKey && !isInput) {
        e.preventDefault();
        setIsCreateModalOpen(false);
        setIsSelectModalOpen(true);
      } else if (e.key === 'Escape') {
        if (isCreateModalOpen) setIsCreateModalOpen(false);
        else if (isSelectModalOpen) setIsSelectModalOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isCreateModalOpen, isSelectModalOpen]);

  const handleCompanyCreated = (company: Company) => {
    setActiveCompany(company);
    showNotification(`Company '${company.name}' created and loaded successfully.`);
  };

  const handleCompanySelected = (company: Company) => {
    setActiveCompany(company);
    showNotification(`Active company switched to '${company.name}'.`);
  };

  return (
    <div className="ot-app">
      <Header
        appName={health?.app_name || 'OpenTally'}
        version={health?.app_version || '0.1.0'}
        environment={health?.environment || 'development'}
      />

      {notification && (
        <div className="ot-notification-banner">
          <span className="ot-notification-icon">&#9432;</span>
          <span className="ot-notification-text">{notification}</span>
          <button className="ot-notification-close" onClick={() => setNotification(null)}>
            &times;
          </button>
        </div>
      )}

      <main className="ot-workspace" style={currentView === 'chart_of_accounts' ? { padding: 0 } : undefined}>
        {currentView === 'chart_of_accounts' ? (
          <ChartOfAccountsView
            activeCompany={activeCompany}
            onBackToGateway={() => setCurrentView('gateway')}
          />
        ) : (
          <div className="ot-workspace-grid">
            <section className="ot-col-company">
              <CompanyPanel
                activeCompany={activeCompany}
                onOpenSelectModal={() => setIsSelectModalOpen(true)}
                onOpenCreateModal={() => setIsCreateModalOpen(true)}
              />

              <div className="ot-card ot-subsystems-card">
                <div className="ot-panel-header">
                  <h3>Subsystems Architecture</h3>
                  <span className="ot-badge-clean">Sprint 1 Stage 3</span>
                </div>
                <div className="ot-subsystems-list">
                  {health?.subsystems?.map((sub) => (
                    <div key={sub.name} className="ot-subsystem-row">
                      <div className="ot-subsystem-name">
                        <code>{sub.name}</code>
                        <span className={`ot-status-pill ot-status-${sub.status}`}>
                          {sub.status}
                        </span>
                      </div>
                      <div className="ot-subsystem-details">{sub.details}</div>
                    </div>
                  ))}
                </div>
              </div>
            </section>

            <section className="ot-col-gateway">
              <Gateway
                onSelectItem={(item) => {
                  if (item.id === 'acc_masters') {
                    if (!activeCompany) {
                      showNotification('Please select or create an active company first (F3).');
                      setIsSelectModalOpen(true);
                    } else {
                      setCurrentView('chart_of_accounts');
                    }
                  } else {
                    showNotification(
                      `Selected [${item.hotkey}] ${item.label} (${item.sprint}): ${item.description}`
                    );
                  }
                }}
              />
            </section>
          </div>
        )}
      </main>

      <StatusBar
        health={health}
        loading={loading}
        error={error}
        onRefresh={loadHealth}
      />

      {/* Modals for Company Management */}
      <CompanyFormModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onCompanyCreated={handleCompanyCreated}
      />

      <CompanySelectModal
        isOpen={isSelectModalOpen}
        activeCompanyId={activeCompany?.id || null}
        onClose={() => setIsSelectModalOpen(false)}
        onCompanySelected={handleCompanySelected}
        onCreateNew={() => {
          setIsSelectModalOpen(false);
          setIsCreateModalOpen(true);
        }}
      />
    </div>
  );
}

export default App;
