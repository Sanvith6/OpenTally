import { useState, useEffect, useCallback } from 'react';
import { Header } from './components/Header';
import { CompanyPanel } from './components/CompanyPanel';
import { Gateway } from './components/Gateway';
import { StatusBar } from './components/StatusBar';
import { CompanyFormModal } from './components/CompanyFormModal';
import { CompanySelectModal } from './components/CompanySelectModal';
import { ChartOfAccountsView } from './components/ChartOfAccountsView';
import { VoucherMastersView } from './components/VoucherMastersView';
import { VoucherEntryView } from './components/vouchers/VoucherEntryView';
import { DayBookView } from './components/reports/DayBookView';
import { LedgerReportView } from './components/reports/LedgerReportView';
import { TrialBalanceView } from './components/reports/TrialBalanceView';
import { ProfitAndLossView } from './components/reports/ProfitAndLossView';
import { BalanceSheetView } from './components/reports/BalanceSheetView';
import { CashFlowView } from './components/reports/CashFlowView';
import { InventoryView } from './components/inventory/InventoryView';
import { InventoryTransactionsView } from './components/inventory/InventoryTransactionsView';
import { InventoryValuationView } from './components/inventory/InventoryValuationView';
import { InventoryReportsView } from './components/inventory/InventoryReportsView';
import { InventoryAccountingView } from './components/inventory/InventoryAccountingView';
import { PartyManagementView } from './components/party/PartyManagementView';
import { SalesVoucherView } from './components/sales/SalesVoucherView';
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
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'voucher_masters'
      ? 'voucher_masters'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'voucher_entry'
      ? 'voucher_entry'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'day_book'
      ? 'day_book'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'ledger_report'
      ? 'ledger_report'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'trial_balance'
      ? 'trial_balance'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'profit_and_loss'
      ? 'profit_and_loss'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'balance_sheet'
      ? 'balance_sheet'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'cash_flow'
      ? 'cash_flow'
      : typeof window !== 'undefined' &&
        new URLSearchParams(window.location.search).get('view') === 'inventory'
      ? 'inventory'
      : typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('view') === 'inventory_transactions'
      ? 'inventory_transactions'
      : typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('view') === 'party_management'
      ? 'party_management'
      : typeof window !== 'undefined' && new URLSearchParams(window.location.search).get('view') === 'sales_vouchers'
      ? 'sales_vouchers'
      : 'gateway';
  const [currentView, setCurrentView] = useState<
    'gateway' | 'chart_of_accounts' | 'voucher_masters' | 'voucher_entry' | 'day_book' | 'ledger_report' | 'trial_balance' | 'profit_and_loss' | 'balance_sheet' | 'cash_flow' | 'inventory' | 'inventory_transactions' | 'inventory_valuation' | 'inventory_reports' | 'inventory_accounting' | 'party_management' | 'sales_vouchers'
  >(initialView);

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

      <main className="ot-workspace" style={currentView !== 'gateway' ? { padding: 0 } : undefined}>
        {currentView === 'chart_of_accounts' ? (
          <ChartOfAccountsView
            activeCompany={activeCompany}
            onBackToGateway={() => setCurrentView('gateway')}
          />
        ) : currentView === 'voucher_masters' ? (
          <VoucherMastersView
            activeCompany={activeCompany}
            onBackToGateway={() => setCurrentView('gateway')}
            onSwitchToEntry={() => setCurrentView('voucher_entry')}
          />
        ) : currentView === 'voucher_entry' ? (
          <VoucherEntryView
            activeCompany={activeCompany}
            onBackToGateway={() => setCurrentView('gateway')}
            onSwitchToMasters={() => setCurrentView('voucher_masters')}
          />
        ) : currentView === 'day_book' ? (
          <DayBookView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'ledger_report' ? (
          <LedgerReportView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'trial_balance' ? (
          <TrialBalanceView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'profit_and_loss' ? (
          <ProfitAndLossView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'balance_sheet' ? (
          <BalanceSheetView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'cash_flow' ? (
          <CashFlowView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'inventory' ? (
          <InventoryView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'inventory_transactions' ? (
          <InventoryTransactionsView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'inventory_valuation' ? (
          <InventoryValuationView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'inventory_reports' ? (
          <InventoryReportsView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'inventory_accounting' ? (
          <InventoryAccountingView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'party_management' ? (
          <PartyManagementView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
        ) : currentView === 'sales_vouchers' ? (
          <SalesVoucherView activeCompany={activeCompany} onBack={() => setCurrentView('gateway')} />
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
                  } else if (item.id === 'vouchers') {
                    if (!activeCompany) {
                      showNotification('Please select or create an active company first (F3).');
                      setIsSelectModalOpen(true);
                    } else {
                      setCurrentView('voucher_entry');
                    }
                  } else if (item.id === 'day_book' || item.id === 'ledger_report' || item.id === 'trial_balance' || item.id === 'profit_and_loss' || item.id === 'balance_sheet' || item.id === 'cash_flow') {
                    if (!activeCompany) {
                      showNotification('Please select or create an active company first (F3).');
                      setIsSelectModalOpen(true);
                    } else {
                      setCurrentView(item.id as 'day_book' | 'ledger_report' | 'trial_balance' | 'profit_and_loss' | 'balance_sheet' | 'cash_flow');
                    }
                  } else if (item.id === 'inv_masters') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('inventory');
                  } else if (item.id === 'party_masters') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('party_management');
                  } else if (item.id === 'sales_vouchers') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('sales_vouchers');
                  } else if (item.id === 'inv_transactions') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('inventory_transactions');
                  } else if (item.id === 'inv_valuation') {
                    setCurrentView('inventory_valuation');
                  } else if (item.id === 'inv_reports') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('inventory_reports');
                  } else if (item.id === 'inv_accounting') {
                    if (!activeCompany) { showNotification('Please select or create an active company first (F3).'); setIsSelectModalOpen(true); }
                    else setCurrentView('inventory_accounting');
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
