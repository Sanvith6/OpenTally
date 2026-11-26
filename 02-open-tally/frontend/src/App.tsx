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
import { VoucherDetailInspectorView } from './components/vouchers/VoucherDetailInspectorView';
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
import { PurchaseVoucherView } from './components/purchase/PurchaseVoucherView';
import { OutstandingAgingView } from './components/reports/OutstandingAgingView';
import { GstReportsView } from './components/reports/GstReportsView';
import { BreadcrumbBar } from './components/navigation/BreadcrumbBar';
import { FunctionKeyBar } from './components/navigation/FunctionKeyBar';
import { GoToModal } from './components/navigation/GoToModal';
import { BackupRestoreModal } from './components/backup/BackupRestoreModal';
import { TallyXmlModal } from './components/interoperability/TallyXmlModal';
import { CompanyFeaturesModal } from './components/company/CompanyFeaturesModal';
import { BankReconciliationView } from './components/banking/BankReconciliationView';
import { TdsReportsView } from './components/reports/TdsReportsView';
import { StockJournalView } from './components/inventory/StockJournalView';
import { GodownSummaryView } from './components/reports/GodownSummaryView';
import { BatchExpiryReportView } from './components/reports/BatchExpiryReportView';
import { CostCentresView } from './components/accounting/CostCentresView';
import { CostCentreBreakupReportView } from './components/reports/CostCentreBreakupReportView';
import { RatioAnalysisView } from './components/reports/RatioAnalysisView';
import { BudgetsView } from './components/accounting/BudgetsView';
import { YearClosingAndAuditView } from './components/accounting/YearClosingAndAuditView';
import { OrderProcessingView } from './components/orders/OrderProcessingView';
import { SecurityControlView } from './components/security/SecurityControlView';
import { ManufacturingBomView } from './components/inventory/ManufacturingBomView';
import { PayrollView } from './components/payroll/PayrollView';
import { MultiCurrencyView } from './components/currency/MultiCurrencyView';
import { CustomReportsView } from './components/extensions/CustomReportsView';
import { CompanyLoginModal } from './components/security/CompanyLoginModal';
import { LanServerModal } from './components/lan/LanServerModal';
import {
  fetchHealth,
  getActiveCompany,
  getCompanySecurityStatus,
  getLanClientConfig,
  getLanServerStatus,
  logoutLanClient,
} from './services/api';
import {
  HealthResponse,
  Company,
  NavigationStackEntry,
  CompanyFeaturesDto,
  AuthSessionInfo,
  LanServerStatusDto,
} from './types';
import './App.css';

function getInitialTitle(view: string): string {
  switch (view) {
    case 'chart_of_accounts':
      return 'Chart of Accounts';
    case 'voucher_masters':
      return 'Voucher Masters & Numbering';
    case 'voucher_entry':
      return 'Accounting Vouchers';
    case 'bank_reconciliation':
      return 'Bank Reconciliation (BRS)';
    case 'voucher_detail':
      return 'Voucher Details';
    case 'day_book':
      return 'Day Book';
    case 'ledger_report':
      return 'Ledger Report';
    case 'trial_balance':
      return 'Trial Balance';
    case 'profit_and_loss':
      return 'Profit & Loss A/c';
    case 'balance_sheet':
      return 'Balance Sheet';
    case 'cash_flow':
      return 'Cash Flow Statement';
    case 'inventory':
      return 'Inventory Masters';
    case 'inventory_transactions':
      return 'Stock Transactions';
    case 'inventory_valuation':
      return 'FIFO Valuation';
    case 'inventory_reports':
      return 'Stock Summary & Registers';
    case 'inventory_accounting':
      return 'Integrated Inventory Accounting';
    case 'party_management':
      return 'Sundry Debtors & Creditors';
    case 'sales_vouchers':
      return 'Sales Vouchers (F8)';
    case 'purchase_vouchers':
      return 'Purchase Vouchers (F9)';
    case 'outstanding_aging':
      return 'Bills Outstanding & Aging';
    case 'gst_reports':
      return 'GST Statutory Reports';
    case 'tds_reports':
      return 'TDS & TCS Statutory Reports';
    case 'stock_journal':
      return 'Stock Journal (Alt+F7)';
    case 'godown_summary':
      return 'Godown Summary Report';
    case 'batch_expiry':
      return 'Batch & Expiry Aging Report';
    case 'cost_centres':
      return 'Cost Categories & Cost Centres';
    case 'cost_centre_breakup':
      return 'Cost Centre Breakup Report';
    case 'ratio_analysis':
      return 'Ratio Analysis';
    case 'budgets':
      return 'Budgets & Scenarios';
    case 'year_closing_audit':
      return 'Year Closing & MCA Audit Certification';
    case 'order_processing':
      return 'Order Processing';
    case 'security_control':
      return 'Users & Passwords (Security Control)';
    case 'manufacturing_bom':
      return 'Manufacturing & Bill of Materials';
    case 'payroll':
      return 'Payroll & Statutory Compliance';
    case 'multi_currency':
      return 'Multi-Currency & Forex';
    case 'custom_reports':
      return 'Custom Report Extensions';
    default:
      return 'Gateway of OpenTally';
  }
}

export function App() {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);
  const [isBackupModalOpen, setIsBackupModalOpen] = useState(false);
  const [isXmlModalOpen, setIsXmlModalOpen] = useState(false);
  const [isFeaturesModalOpen, setIsFeaturesModalOpen] = useState(false);
  const [featuresModalCompany, setFeaturesModalCompany] = useState<Company | null>(null);
  const [isInitialSetup, setIsInitialSetup] = useState(false);
  const [pendingLoginCompany, setPendingLoginCompany] = useState<Company | null>(null);
  const [activeUserSession, setActiveUserSession] = useState<AuthSessionInfo | null>(null);
  const [isLanServerModalOpen, setIsLanServerModalOpen] = useState(false);
  const [lanServerStatus, setLanServerStatus] = useState<LanServerStatusDto | null>(null);

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
                        : typeof window !== 'undefined' &&
                          new URLSearchParams(window.location.search).get('view') === 'inventory_transactions'
                          ? 'inventory_transactions'
                          : typeof window !== 'undefined' &&
                            new URLSearchParams(window.location.search).get('view') === 'party_management'
                            ? 'party_management'
                            : typeof window !== 'undefined' &&
                              new URLSearchParams(window.location.search).get('view') === 'sales_vouchers'
                              ? 'sales_vouchers'
                              : typeof window !== 'undefined' &&
                                new URLSearchParams(window.location.search).get('view') === 'purchase_vouchers'
                                ? 'purchase_vouchers'
                                : typeof window !== 'undefined' &&
                                  new URLSearchParams(window.location.search).get('view') === 'outstanding_aging'
                                  ? 'outstanding_aging'
                                  : typeof window !== 'undefined' &&
                                    new URLSearchParams(window.location.search).get('view') === 'gst_reports'
                                    ? 'gst_reports'
                                    : 'gateway';

  // Navigation Stack State
  const [navStack, setNavStack] = useState<NavigationStackEntry[]>([
    { view: initialView, title: getInitialTitle(initialView) },
  ]);

  const currentEntry =
    navStack[navStack.length - 1] || { view: 'gateway', title: 'Gateway of OpenTally' };
  const currentView = currentEntry.view;

  const pushView = useCallback(
    (view: string, title?: string, params?: Record<string, any>) => {
      const finalTitle = title || getInitialTitle(view);
      setNavStack((prev) => {
        const top = prev[prev.length - 1];
        if (top && top.view === 'voucher_entry' && view === 'voucher_entry') {
          return [...prev.slice(0, prev.length - 1), { view, title: finalTitle, params }];
        }
        return [...prev, { view, title: finalTitle, params }];
      });
    },
    []
  );

  const popView = useCallback(() => {
    setNavStack((prev) => (prev.length > 1 ? prev.slice(0, prev.length - 1) : prev));
  }, []);

  const navigateToLevel = useCallback((index: number) => {
    setNavStack((prev) => (index >= 0 && index < prev.length ? prev.slice(0, index + 1) : prev));
  }, []);

  // Go To Command Palette State
  const [isGoToOpen, setIsGoToOpen] = useState(false);

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

  const refreshLanStatus = useCallback(async () => {
    try {
      const status = await getLanServerStatus();
      setLanServerStatus(status);
    } catch {
      setLanServerStatus(null);
    }
  }, []);

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
      if (!active && !initialModal) {
        setIsSelectModalOpen(true);
      }
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : 'Failed to retrieve active company context'
      );
    }
  }, [initialModal]);

  useEffect(() => {
    loadHealth();
    loadCompanyContext();
    refreshLanStatus();
  }, [loadCompanyContext, refreshLanStatus]);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => {
      setNotification((curr) => (curr === msg ? null : curr));
    }, 4000);
  };

  const openFeaturesModal = useCallback(
    (company?: Company | null, initialSetup = false) => {
      const targetComp = company || activeCompany;
      if (!targetComp) {
        showNotification('Please select or create an active company first (F3).');
        setIsSelectModalOpen(true);
        return;
      }
      setFeaturesModalCompany(targetComp);
      setIsInitialSetup(initialSetup);
      setIsFeaturesModalOpen(true);
    },
    [activeCompany]
  );

  // Centralized Global Keyboard Interceptor (TallyPrime Function Keys & Esc Hierarchy)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // 1. Function Keys
      if (e.key === 'F1') {
        e.preventDefault();
      } else if (e.key === 'F2') {
        e.preventDefault();
      } else if (e.key === 'F3') {
        e.preventDefault();
        if (e.altKey) {
          setIsSelectModalOpen(false);
          setIsCreateModalOpen(true);
        } else {
          setIsCreateModalOpen(false);
          setIsSelectModalOpen(true);
        }
      } else if (['F4', 'F5', 'F6', 'F7', 'F8', 'F9'].includes(e.key) && !activeCompany) {
        e.preventDefault();
        showNotification('Please select or create an active company first (F3).');
        setIsSelectModalOpen(true);
      } else if (e.key === 'F4') {
        e.preventDefault();
        if (e.ctrlKey) {
          pushView('payroll', 'Payroll (Ctrl+F4)');
        } else {
          pushView('voucher_entry', 'Accounting Vouchers (Contra)', { type: 'vt_contra' });
        }
      } else if (e.key === 'F5') {
        e.preventDefault(); // Prevent browser refresh!
        if (e.altKey) {
          pushView('voucher_entry', 'Accounting Vouchers (Debit Note)', { type: 'vt_debit_note' });
        } else {
          pushView('voucher_entry', 'Accounting Vouchers (Payment)', { type: 'vt_payment' });
        }
      } else if (e.key === 'F6') {
        e.preventDefault();
        if (e.altKey) {
          pushView('voucher_entry', 'Accounting Vouchers (Credit Note)', { type: 'vt_credit_note' });
        } else {
          pushView('voucher_entry', 'Accounting Vouchers (Receipt)', { type: 'vt_receipt' });
        }
      } else if (e.key === 'F7') {
        e.preventDefault();
        if (e.altKey) {
          pushView('stock_journal', 'Stock Journal (Alt+F7)');
        } else {
          pushView('voucher_entry', 'Accounting Vouchers (Journal)', { type: 'vt_jnl' });
        }
      } else if (e.key === 'F8') {
        e.preventDefault();
        if (e.ctrlKey) {
          pushView('order_processing', 'Sales Orders (Ctrl+F8)', { orderType: 'SalesOrder' });
        } else {
          pushView('sales_vouchers', 'Sales Vouchers (F8)');
        }
      } else if (e.key === 'F9') {
        e.preventDefault();
        if (e.ctrlKey) {
          pushView('order_processing', 'Purchase Orders (Ctrl+F9)', { orderType: 'PurchaseOrder' });
        } else {
          pushView('purchase_vouchers', 'Purchase Vouchers (F9)');
        }
      } else if (e.key === 'F11') {
        e.preventDefault();
        openFeaturesModal();
      } else if (e.key === 'F12') {
        e.preventDefault();
      } else if ((e.key === 'b' || e.key === 'B') && e.altKey) {
        e.preventDefault();
        setIsBackupModalOpen(true);
      } else if ((e.key === 'k' || e.key === 'K') && e.altKey) {
        e.preventDefault();
        setIsSelectModalOpen(true);
      } else if ((e.key === 'i' || e.key === 'I') && e.altKey) {
        e.preventDefault();
        setIsXmlModalOpen(true);
      } else if ((e.key === 'g' || e.key === 'G') && (e.altKey || e.ctrlKey)) {
        e.preventDefault();
        setIsGoToOpen(true);
      } else if (e.key === 'Escape') {
        if (pendingLoginCompany) {
          e.preventDefault();
          setPendingLoginCompany(null);
        } else if (isLanServerModalOpen) {
          e.preventDefault();
          setIsLanServerModalOpen(false);
          refreshLanStatus();
        } else if (isFeaturesModalOpen) {
          e.preventDefault();
          setIsFeaturesModalOpen(false);
        } else if (isXmlModalOpen) {
          e.preventDefault();
          setIsXmlModalOpen(false);
        } else if (isBackupModalOpen) {
          e.preventDefault();
          setIsBackupModalOpen(false);
        } else if (isGoToOpen) {
          e.preventDefault();
          setIsGoToOpen(false);
        } else if (isCreateModalOpen) {
          e.preventDefault();
          setIsCreateModalOpen(false);
        } else if (isSelectModalOpen) {
          e.preventDefault();
          setIsSelectModalOpen(false);
        } else if (navStack.length > 1) {
          e.preventDefault();
          popView();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    pendingLoginCompany,
    isLanServerModalOpen,
    isFeaturesModalOpen,
    isXmlModalOpen,
    isBackupModalOpen,
    isGoToOpen,
    isCreateModalOpen,
    isSelectModalOpen,
    navStack.length,
    popView,
    pushView,
    currentView,
    openFeaturesModal,
    refreshLanStatus,
    activeCompany,
  ]);

  const handleCompanyCreated = (company: Company) => {
    setActiveCompany(company);
    setIsCreateModalOpen(false);
    openFeaturesModal(company, true);
  };

  const handleCompanySelected = async (company: Company) => {
    try {
      const sec = await getCompanySecurityStatus(company.id);
      if (sec.security_enabled && !sec.active_user) {
        setPendingLoginCompany(company);
        return;
      }
      setActiveUserSession(sec.active_user || null);
    } catch {
      setActiveUserSession(null);
    }
    setActiveCompany(company);
    showNotification(`Active company switched to '${company.name}'.`);
  };

  const handleFeaturesSaved = (_features: CompanyFeaturesDto) => {
    showNotification('Company features updated successfully. Standard ledgers & mappings active.');
    loadCompanyContext();
  };

  const lanClientConfig = getLanClientConfig();
  const lanBadge = lanClientConfig
    ? 'LAN CLIENT'
    : lanServerStatus?.is_running
    ? `LAN HOST :${lanServerStatus.port}`
    : null;

  return (
    <div className="ot-app">
      <Header
        appName={health?.app_name || 'OpenTally'}
        version={health?.app_version || '3.0.0'}
        environment={health?.environment || 'development'}
        companyName={activeCompany?.name || null}
        periodLabel={
          activeCompany ? `FY from ${activeCompany.financial_year_begins}` : null
        }
        username={activeUserSession?.username || null}
        userRole={activeUserSession?.role || null}
        lanBadge={lanBadge}
        onCompanyClick={() => setIsSelectModalOpen(true)}
        onBackupClick={() => setIsBackupModalOpen(true)}
        onXmlClick={() => setIsXmlModalOpen(true)}
        onGoToClick={() => setIsGoToOpen(true)}
        onFeaturesClick={() => openFeaturesModal()}
      />

      <BreadcrumbBar
        stack={navStack}
        onNavigateToLevel={navigateToLevel}
        onOpenGoTo={() => setIsGoToOpen(true)}
        lanClientConfig={lanClientConfig}
        onDisconnectLan={async () => {
          await logoutLanClient();
          setActiveCompany(null);
          setActiveUserSession(null);
          setIsSelectModalOpen(true);
          showNotification('Disconnected from LAN Host.');
        }}
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

      <div className="ot-main-layout" style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <main
          className="ot-workspace"
          style={currentView !== 'gateway' ? { padding: 0 } : undefined}
        >
          {currentView === 'chart_of_accounts' ? (
            <ChartOfAccountsView
              activeCompany={activeCompany}
              onBackToGateway={popView}
            />
          ) : currentView === 'voucher_masters' ? (
            <VoucherMastersView
              activeCompany={activeCompany}
              onBackToGateway={popView}
              onSwitchToEntry={() => pushView('voucher_entry', 'Accounting Vouchers')}
            />
          ) : currentView === 'voucher_entry' ? (
            <VoucherEntryView
              activeCompany={activeCompany}
              onBackToGateway={popView}
              initialTypeId={currentEntry.params?.type}
              onSwitchToMasters={() =>
                pushView('voucher_masters', 'Voucher Masters & Numbering')
              }
            />
          ) : currentView === 'voucher_detail' ? (
            <VoucherDetailInspectorView
              activeCompany={activeCompany}
              voucherId={currentEntry.params?.voucherId || ''}
              onBack={popView}
              onNavigateToVoucher={(vId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId: vId })
              }
              onNavigateToLedger={(lId) =>
                pushView('ledger_report', 'Ledger Report', { ledgerId: lId })
              }
            />
          ) : currentView === 'day_book' ? (
            <DayBookView
              activeCompany={activeCompany}
              onBack={popView}
              onViewVoucher={(voucherId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId })
              }
            />
          ) : currentView === 'ledger_report' ? (
            <LedgerReportView
              activeCompany={activeCompany}
              initialLedgerId={currentEntry.params?.ledgerId}
              onBack={popView}
              onViewVoucher={(voucherId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId })
              }
            />
          ) : currentView === 'trial_balance' ? (
            <TrialBalanceView
              activeCompany={activeCompany}
              onBack={popView}
              onViewLedger={(ledgerId, ledgerName) =>
                pushView('ledger_report', `Ledger: ${ledgerName || ledgerId}`, {
                  ledgerId,
                })
              }
            />
          ) : currentView === 'profit_and_loss' ? (
            <ProfitAndLossView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'balance_sheet' ? (
            <BalanceSheetView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'cash_flow' ? (
            <CashFlowView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'inventory' ? (
            <InventoryView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'inventory_transactions' ? (
            <InventoryTransactionsView
              activeCompany={activeCompany}
              onBack={popView}
            />
          ) : currentView === 'inventory_valuation' ? (
            <InventoryValuationView
              activeCompany={activeCompany}
              onBack={popView}
            />
          ) : currentView === 'inventory_reports' ? (
            <InventoryReportsView
              activeCompany={activeCompany}
              onBack={popView}
            />
          ) : currentView === 'inventory_accounting' ? (
            <InventoryAccountingView
              activeCompany={activeCompany}
              onBack={popView}
            />
          ) : currentView === 'party_management' ? (
            <PartyManagementView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'sales_vouchers' ? (
            <SalesVoucherView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'purchase_vouchers' ? (
            <PurchaseVoucherView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'outstanding_aging' ? (
            <OutstandingAgingView
              activeCompany={activeCompany}
              onBack={popView}
              onViewLedger={(partyLedgerId, partyName) =>
                pushView('ledger_report', `Party: ${partyName || partyLedgerId}`, {
                  ledgerId: partyLedgerId,
                })
              }
            />
          ) : currentView === 'gst_reports' ? (
            <GstReportsView
              activeCompany={activeCompany}
              initialTab={currentEntry.params?.tab}
              onBack={popView}
              onViewVoucher={(voucherId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId })
              }
            />
          ) : currentView === 'bank_reconciliation' ? (
            <BankReconciliationView
              activeCompany={activeCompany}
              initialBankLedgerId={currentEntry.params?.bankLedgerId}
              onBack={popView}
              onViewVoucher={(voucherId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId })
              }
            />
          ) : currentView === 'tds_reports' ? (
            <TdsReportsView
              activeCompany={activeCompany}
              onBack={popView}
              onViewVoucher={(voucherId) =>
                pushView('voucher_detail', 'Voucher Details', { voucherId })
              }
            />
          ) : currentView === 'stock_journal' ? (
            <StockJournalView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'godown_summary' ? (
            <GodownSummaryView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'batch_expiry' ? (
            <BatchExpiryReportView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'cost_centres' ? (
            <CostCentresView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'cost_centre_breakup' ? (
            <CostCentreBreakupReportView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'ratio_analysis' ? (
            <RatioAnalysisView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'budgets' ? (
            <BudgetsView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'year_closing_audit' ? (
            <YearClosingAndAuditView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'order_processing' ? (
            <OrderProcessingView
              activeCompany={activeCompany}
              initialOrderType={currentEntry.params?.orderType || 'SalesOrder'}
              onBack={popView}
            />
          ) : currentView === 'security_control' ? (
            <SecurityControlView
              activeCompany={activeCompany}
              currentUser={activeUserSession}
              onBack={popView}
            />
          ) : currentView === 'manufacturing_bom' ? (
            <ManufacturingBomView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'payroll' && activeCompany ? (
            <PayrollView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'multi_currency' && activeCompany ? (
            <MultiCurrencyView activeCompany={activeCompany} onBack={popView} />
          ) : currentView === 'custom_reports' && activeCompany ? (
            <CustomReportsView activeCompany={activeCompany} onBack={popView} />
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
                        pushView('chart_of_accounts', 'Chart of Accounts');
                      }
                    } else if (item.id === 'vouchers') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else {
                        pushView('voucher_entry', 'Accounting Vouchers');
                      }
                    } else if (
                      item.id === 'day_book' ||
                      item.id === 'ledger_report' ||
                      item.id === 'trial_balance' ||
                      item.id === 'profit_and_loss' ||
                      item.id === 'balance_sheet' ||
                      item.id === 'cash_flow'
                    ) {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else {
                        pushView(item.id, getInitialTitle(item.id));
                      }
                    } else if (item.id === 'inv_masters') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('inventory', 'Inventory Masters');
                    } else if (item.id === 'party_masters') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('party_management', 'Sundry Debtors & Creditors');
                    } else if (item.id === 'sales_vouchers') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('sales_vouchers', 'Sales Vouchers (F8)');
                    } else if (item.id === 'purchase_vouchers') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('purchase_vouchers', 'Purchase Vouchers (F9)');
                    } else if (item.id === 'outstanding_aging') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('outstanding_aging', 'Bills Outstanding & Aging');
                    } else if (item.id === 'bank_reconciliation') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('bank_reconciliation', 'Bank Reconciliation (BRS)');
                    } else if (item.id === 'taxation' || item.id === 'gst_reports') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('gst_reports', 'GST Statutory Reports');
                    } else if (item.id === 'tds_reports') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('tds_reports', 'TDS & Form 26Q Reports');
                    } else if (item.id === 'stock_journal') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('stock_journal', 'Stock Journal (Alt+F7)');
                    } else if (item.id === 'godown_summary') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('godown_summary', 'Godown Summary Report');
                    } else if (item.id === 'payroll') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('payroll', 'Payroll (Ctrl+F4)');
                    } else if (item.id === 'batch_expiry') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('batch_expiry', 'Batch & Expiry Aging Report');
                    } else if (item.id === 'cost_centres') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('cost_centres', 'Cost Categories & Cost Centres');
                    } else if (item.id === 'cost_centre_breakup') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('cost_centre_breakup', 'Cost Centre Breakup Report');
                    } else if (item.id === 'ratio_analysis') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('ratio_analysis', 'Ratio Analysis');
                    } else if (item.id === 'budgets') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('budgets', 'Budgets & Scenarios');
                    } else if (item.id === 'year_closing_audit') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('year_closing_audit', 'Year Closing & MCA Audit Certification');
                    } else if (item.id === 'order_processing') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('order_processing', 'Sales Orders (Ctrl+F8)', { orderType: 'SalesOrder' });
                    } else if (item.id === 'security_control') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('security_control', 'Users & Passwords (Security Control)');
                    } else if (item.id === 'manufacturing_bom') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('manufacturing_bom', 'Manufacturing & Bill of Materials');
                    } else if (item.id === 'inv_transactions') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('inventory_transactions', 'Stock Transactions');
                    } else if (item.id === 'inv_valuation') {
                      pushView('inventory_valuation', 'FIFO Valuation');
                    } else if (item.id === 'inv_reports') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('inventory_reports', 'Stock Summary & Registers');
                    } else if (item.id === 'inv_accounting') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('inventory_accounting', 'Integrated Inventory Accounting');
                    } else if (item.id === 'backup_restore') {
                      setIsBackupModalOpen(true);
                    } else if (item.id === 'tally_xml') {
                      setIsXmlModalOpen(true);
                    } else if (item.id === 'company_features') {
                      openFeaturesModal();
                    } else if (item.id === 'lan_server') {
                      setIsLanServerModalOpen(true);
                    } else if (item.id === 'multi_currency') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('multi_currency', 'Multi-Currency & Forex');
                    } else if (item.id === 'custom_reports') {
                      if (!activeCompany) {
                        showNotification('Please select or create an active company first (F3).');
                        setIsSelectModalOpen(true);
                      } else pushView('custom_reports', 'Custom Report Extensions');
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

        <FunctionKeyBar
          canBack={navStack.length > 1}
          onBackClick={popView}
          onCompanyClick={() => setIsSelectModalOpen(true)}
          onFeaturesClick={() => openFeaturesModal()}
          onGoToClick={() => setIsGoToOpen(true)}
          onContraClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Contra)', { type: 'vt_contra' })
          }
          onPayrollClick={() => pushView('payroll', 'Payroll (Ctrl+F4)')}
          onPaymentClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Payment)', { type: 'vt_payment' })
          }
          onDebitNoteClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Debit Note)', { type: 'vt_debit_note' })
          }
          onReceiptClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Receipt)', { type: 'vt_receipt' })
          }
          onCreditNoteClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Credit Note)', { type: 'vt_credit_note' })
          }
          onJournalClick={() =>
            pushView('voucher_entry', 'Accounting Vouchers (Journal)', { type: 'vt_jnl' })
          }
          onStockJournalClick={() =>
            pushView('stock_journal', 'Stock Journal (Alt+F7)')
          }
          onSalesClick={() => pushView('sales_vouchers', 'Sales Vouchers (F8)')}
          onSalesOrderClick={() =>
            pushView('order_processing', 'Sales Orders (Ctrl+F8)', { orderType: 'SalesOrder' })
          }
          onPurchaseClick={() => pushView('purchase_vouchers', 'Purchase Vouchers (F9)')}
          onPurchaseOrderClick={() =>
            pushView('order_processing', 'Purchase Orders (Ctrl+F9)', { orderType: 'PurchaseOrder' })
          }
        />
      </div>

      <StatusBar
        health={health}
        loading={loading}
        error={error}
        onRefresh={loadHealth}
      />

      {/* Modals for Company Management & Go To Palette */}
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
        onImportXml={() => setIsXmlModalOpen(true)}
        onRestoreBackup={() => setIsBackupModalOpen(true)}
      />

      <GoToModal
        isOpen={isGoToOpen}
        onClose={() => setIsGoToOpen(false)}
        onSelectView={(view, title, params) => {
          if (view === 'company_features') {
            openFeaturesModal();
          } else if (view === 'backup_restore') {
            setIsBackupModalOpen(true);
          } else if (view === 'tally_xml') {
            setIsXmlModalOpen(true);
          } else if (view === 'lan_server') {
            setIsLanServerModalOpen(true);
          } else {
            pushView(view, title, params);
          }
          setIsGoToOpen(false);
        }}
      />

      <BackupRestoreModal
        isOpen={isBackupModalOpen}
        onClose={() => setIsBackupModalOpen(false)}
        onShowNotification={showNotification}
      />

      <TallyXmlModal
        isOpen={isXmlModalOpen}
        onClose={() => setIsXmlModalOpen(false)}
        activeCompany={activeCompany}
        onImportSuccess={() => {
          showNotification('Tally XML imported successfully. Refreshing company context...');
          loadCompanyContext();
        }}
      />

      <CompanyFeaturesModal
        isOpen={isFeaturesModalOpen}
        companyId={featuresModalCompany?.id || activeCompany?.id || null}
        companyName={featuresModalCompany?.name || activeCompany?.name}
        isInitialSetup={isInitialSetup}
        onClose={() => setIsFeaturesModalOpen(false)}
        onSaved={handleFeaturesSaved}
      />

      <LanServerModal
        isOpen={isLanServerModalOpen}
        onClose={() => {
          setIsLanServerModalOpen(false);
          refreshLanStatus();
        }}
        onShowNotification={showNotification}
      />

      {pendingLoginCompany && (
        <CompanyLoginModal
          company={pendingLoginCompany}
          onCancel={() => setPendingLoginCompany(null)}
          onSuccess={(session: AuthSessionInfo) => {
            setActiveUserSession(session);
            setActiveCompany(pendingLoginCompany);
            setPendingLoginCompany(null);
            showNotification(`Logged in as ${session.username} (${session.role}).`);
          }}
        />
      )}
    </div>
  );
}

export default App;
