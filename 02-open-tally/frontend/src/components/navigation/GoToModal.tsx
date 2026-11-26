import React, { useState, useEffect, useRef, useCallback } from 'react';
import { GotoSearchResult } from '../../types';
import { globalGotoSearch } from '../../services/api';
import './GoToModal.css';

interface ScreenCatalogItem {
  id: string;
  title: string;
  subtitle: string;
  category: 'Screen' | 'Report' | 'Master';
  view: string;
  hotkey?: string;
  params?: Record<string, any>;
}

const STATIC_SCREENS: ScreenCatalogItem[] = [
  { id: 'scr_gateway', title: 'Gateway of OpenTally', subtitle: 'Main application portal', category: 'Screen', view: 'gateway' },
  { id: 'scr_coa', title: 'Chart of Accounts', subtitle: 'Ledger hierarchy and groups tree', category: 'Master', view: 'chart_of_accounts' },
  { id: 'scr_vch_entry', title: 'Voucher Entry (Accounting Vouchers)', subtitle: 'Create Payment, Receipt, Sales, Purchase vouchers', category: 'Screen', view: 'voucher_entry', hotkey: 'V' },
  { id: 'scr_vch_masters', title: 'Voucher Masters & Numbering Series', subtitle: 'Configure series, numbering tokens, reset frequencies', category: 'Master', view: 'voucher_masters' },
  { id: 'scr_daybook', title: 'Day Book', subtitle: 'Chronological list of all daily transactions', category: 'Report', view: 'day_book', hotkey: 'K' },
  { id: 'scr_brs', title: 'Bank Reconciliation Statement (BRS)', subtitle: 'Reconcile bank ledgers with bank date and CSV import', category: 'Report', view: 'bank_reconciliation' },
  { id: 'scr_banking', title: 'Banking', subtitle: 'Bank ledgers, reconciliation & instrument allocation', category: 'Screen', view: 'bank_reconciliation' },
  { id: 'scr_ledger_report', title: 'Ledger Monthly Summary & Register', subtitle: 'Account transaction history and running balances', category: 'Report', view: 'ledger_report', hotkey: 'L' },
  { id: 'scr_tb', title: 'Trial Balance', subtitle: 'Equilibrium statement of opening, period and closing balances', category: 'Report', view: 'trial_balance', hotkey: 'T' },
  { id: 'scr_pnl', title: 'Profit & Loss Account', subtitle: 'Trading and income statement with gross/net profit', category: 'Report', view: 'profit_and_loss', hotkey: 'P' },
  { id: 'scr_bs', title: 'Balance Sheet', subtitle: 'Liabilities and Assets financial position statement', category: 'Report', view: 'balance_sheet', hotkey: 'B' },
  { id: 'scr_cf', title: 'Cash Flow Statement', subtitle: 'Inflows and outflows across operational periods', category: 'Report', view: 'cash_flow', hotkey: 'C' },
  { id: 'scr_inv_masters', title: 'Inventory Masters (Items, Units, Godowns)', subtitle: 'Stock items, categories, groups and warehouses', category: 'Master', view: 'inventory', hotkey: 'I' },
  { id: 'scr_inv_tx', title: 'Stock Transactions & Physical Stock', subtitle: 'Goods receipts, deliveries, internal godown transfers', category: 'Screen', view: 'inventory_transactions' },
  { id: 'scr_inv_val', title: 'FIFO Valuation & Inventory Cost Layers', subtitle: 'Perpetual cost computation and run-time valuations', category: 'Report', view: 'inventory_valuation' },
  { id: 'scr_inv_rep', title: 'Stock Summary & Inventory Registers', subtitle: 'Movement registers, item ledgers, godown reconciliation', category: 'Report', view: 'inventory_reports' },
  { id: 'scr_inv_acc', title: 'Integrated Inventory Accounting', subtitle: 'Stock-in-hand integration and P&L cost of goods sold', category: 'Master', view: 'inventory_accounting' },
  { id: 'scr_parties', title: 'Sundry Debtors & Creditors (Parties)', subtitle: 'Party profiles, GSTINs, credit limits and state codes', category: 'Master', view: 'party_management' },
  { id: 'scr_sales', title: 'Sales Vouchers (F8 Commercial)', subtitle: 'Invoicing with atomic double-entry GL + FIFO COGS', category: 'Screen', view: 'sales_vouchers', hotkey: 'F8' },
  { id: 'scr_pur', title: 'Purchase Vouchers (F9 Commercial)', subtitle: 'Procurement with atomic inventory layer creation', category: 'Screen', view: 'purchase_vouchers', hotkey: 'F9' },
  { id: 'scr_aging', title: 'Bills Outstanding & Aging Analysis', subtitle: 'Bill-wise receivables, payables and overdue aging intervals', category: 'Report', view: 'outstanding_aging' },
  { id: 'scr_gst', title: 'GST Statutory Reports (GSTR-1 & 3B)', subtitle: 'Outward supplies, B2B invoices, ITC and HSN summaries', category: 'Report', view: 'gst_reports' },
  { id: 'scr_gstr2b', title: 'GSTR-2B ITC Reconciliation', subtitle: 'Reconcile purchase ITC against portal JSON', category: 'Report', view: 'gst_reports', params: { tab: 'gstr2b' } },
  { id: 'scr_tds_reports', title: 'TDS & TCS Statutory Reports', subtitle: 'Tax Deducted at Source, Challan Tagging & Form 26Q', category: 'Report', view: 'tds_reports' },
  { id: 'scr_tds_outstanding', title: 'TDS Outstanding Remittance', subtitle: 'Unpaid TDS obligations grouped by section and due date', category: 'Report', view: 'tds_reports' },
  { id: 'scr_form_26q', title: 'Form 26Q Quarterly Return', subtitle: 'Quarterly TDS return summary, JSON and CSV export', category: 'Report', view: 'tds_reports' },
  { id: 'scr_cn', title: 'Credit Note (Alt+F6)', subtitle: 'Sales Return with GST and original invoice linkage', category: 'Screen', view: 'voucher_entry', hotkey: 'Alt+F6', params: { type: 'vt_credit_note' } },
  { id: 'scr_dn', title: 'Debit Note (Alt+F5)', subtitle: 'Purchase Return with ITC reversal and stock depletion', category: 'Screen', view: 'voucher_entry', hotkey: 'Alt+F5', params: { type: 'vt_debit_note' } },
  { id: 'scr_backup', title: 'Backup & Restore Database', subtitle: 'WAL-safe SQLite backup with SHA-256 verification', category: 'Master', view: 'backup_restore', hotkey: 'B' },
  { id: 'scr_tally_xml', title: 'Tally XML Import / Export', subtitle: 'File-based Tally XML import with dry-run preview and export', category: 'Master', view: 'tally_xml', hotkey: 'Alt+I' },
  { id: 'scr_company_features', title: 'Company Features (F11)', subtitle: 'Accounting, inventory, GST & statutory configuration', category: 'Master', view: 'company_features', hotkey: 'F11' },
  { id: 'scr_multi_currency', title: 'Multi-Currency & Forex Management', subtitle: 'Currencies, daily rates, foreign bill settlement and unrealized forex revaluation', category: 'Master', view: 'multi_currency', hotkey: 'Alt+U' },
  { id: 'scr_stock_journal', title: 'Stock Journal (Alt+F7)', subtitle: 'Inter-godown transfer & consumption/production voucher', category: 'Screen', view: 'stock_journal', hotkey: 'Alt+F7' },
  { id: 'scr_godown_summary', title: 'Godown Summary Report', subtitle: 'Godown-wise stock balance, inflow/outflow & valuation', category: 'Report', view: 'godown_summary' },
  { id: 'scr_batch_expiry', title: 'Batch & Expiry Aging Report', subtitle: 'Batch tracking, shelf-life and expiry date monitoring', category: 'Report', view: 'batch_expiry' },
  { id: 'scr_cost_centres', title: 'Cost Categories & Cost Centres', subtitle: 'Cost hierarchy setup and multi-dimensional tracking', category: 'Master', view: 'cost_centres' },
  { id: 'scr_cost_centre_breakup', title: 'Cost Centre Breakup Report', subtitle: 'Voucher expense and revenue allocations by cost centre', category: 'Report', view: 'cost_centre_breakup' },
  { id: 'scr_ratio_analysis', title: 'Ratio Analysis', subtitle: 'Financial ratios, working capital, liquidity and profitability', category: 'Report', view: 'ratio_analysis', hotkey: 'Q' },
  { id: 'scr_custom_reports', title: 'Custom Report Extensions', subtitle: 'Declarative custom report builder & .opentally-report.json templates', category: 'Report', view: 'custom_reports' },
  { id: 'scr_budgets', title: 'Budgets & Scenarios', subtitle: 'Target definitions and actual vs budget variance tracking', category: 'Master', view: 'budgets' },
  { id: 'scr_year_closing_audit', title: 'Year Closing & MCA Audit Certification', subtitle: 'Financial year rollover, period lock and forensic audit trail', category: 'Master', view: 'year_closing_audit' },
  { id: 'scr_sales_order', title: 'Sales Order Processing (Ctrl+F8)', subtitle: 'Customer orders, partial fulfillment tracking & invoice conversion', category: 'Screen', view: 'order_processing', hotkey: 'Ctrl+F8', params: { orderType: 'SalesOrder' } },
  { id: 'scr_pur_order', title: 'Purchase Order Processing (Ctrl+F9)', subtitle: 'Supplier procurement orders and conversion to F9 purchase invoices', category: 'Screen', view: 'order_processing', hotkey: 'Ctrl+F9', params: { orderType: 'PurchaseOrder' } },
  { id: 'scr_security_control', title: 'Security Control & User Passwords', subtitle: 'Manage company users, roles (RBAC) and passwords', category: 'Master', view: 'security_control' },
  { id: 'scr_lan_server', title: 'LAN Host / Multi-User Server (Gold)', subtitle: 'Start/stop opt-in LAN server on port 7900 & manage client sessions', category: 'Master', view: 'lan_server' },
  { id: 'scr_manufacturing_bom', title: 'Manufacturing & Bill of Materials (BOM)', subtitle: 'Production journal, BOM multi-component recipes & finished goods costing', category: 'Screen', view: 'manufacturing_bom' },
  { id: 'scr_payroll', title: 'Payroll & Statutory Compliance (Ctrl+F4)', subtitle: 'Employee masters, LOP proration, EPF/ESI/PT & Payslip PDF', category: 'Screen', view: 'payroll', hotkey: 'Ctrl+F4' },
];

interface GoToModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectView: (view: string, title: string, params?: Record<string, any>) => void;
}

export const GoToModal: React.FC<GoToModalProps> = ({
  isOpen,
  onClose,
  onSelectView,
}) => {
  const [query, setQuery] = useState('');
  const [backendResults, setBackendResults] = useState<GotoSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Focus on open
  useEffect(() => {
    if (isOpen) {
      setQuery('');
      setBackendResults([]);
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  // Backend live search debounce
  useEffect(() => {
    if (!isOpen) return;
    const cleanQuery = query.trim();
    if (!cleanQuery) {
      setBackendResults([]);
      setLoading(false);
      return;
    }

    setLoading(true);
    const handler = setTimeout(async () => {
      try {
        const res = await globalGotoSearch(cleanQuery, 25);
        setBackendResults(res);
      } catch {
        setBackendResults([]);
      } finally {
        setLoading(false);
      }
    }, 150);

    return () => clearTimeout(handler);
  }, [query, isOpen]);

  // Combined Results
  const matchingScreens = STATIC_SCREENS.filter((s) => {
    if (!query.trim()) return true;
    const q = query.toLowerCase();
    return (
      s.title.toLowerCase().includes(q) ||
      s.subtitle.toLowerCase().includes(q) ||
      (s.hotkey && s.hotkey.toLowerCase() === q)
    );
  });

  interface UnifiedResultItem {
    id: string;
    category: string;
    title: string;
    subtitle: string;
    view: string;
    params?: Record<string, any>;
  }

  const allItems: UnifiedResultItem[] = [
    ...matchingScreens.map((s) => ({
      id: s.id,
      category: s.category,
      title: s.title,
      subtitle: s.subtitle,
      view: s.view,
      params: s.params,
    })),
    ...backendResults.map((b) => ({
      id: b.id,
      category: b.category,
      title: b.title,
      subtitle: b.subtitle,
      view: b.target_view,
      params: b.target_id
        ? b.target_view === 'voucher_detail'
          ? { voucherId: b.target_id }
          : b.target_view === 'ledger_report'
          ? { ledgerId: b.target_id }
          : { id: b.target_id }
        : undefined,
    })),
  ];

  // Adjust selectedIndex bounds
  useEffect(() => {
    if (selectedIndex >= allItems.length) {
      setSelectedIndex(Math.max(0, allItems.length - 1));
    }
  }, [allItems.length, selectedIndex]);

  const handleSelect = useCallback(
    (item: UnifiedResultItem) => {
      onSelectView(item.view, item.title, item.params);
      onClose();
    },
    [onSelectView, onClose]
  );

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelectedIndex((prev) => (prev < allItems.length - 1 ? prev + 1 : 0));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelectedIndex((prev) => (prev > 0 ? prev - 1 : Math.max(0, allItems.length - 1)));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const current = allItems[selectedIndex];
      if (current) {
        handleSelect(current);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };

  // Scroll active item into view
  useEffect(() => {
    const listEl = listRef.current;
    if (!listEl) return;
    const activeEl = listEl.querySelector<HTMLElement>(`[data-index="${selectedIndex}"]`);
    if (activeEl && typeof activeEl.scrollIntoView === 'function') {
      activeEl.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  if (!isOpen) return null;

  return (
    <div className="ot-goto-backdrop" onClick={onClose} data-testid="goto-modal">
      <div className="ot-goto-container" onClick={(e) => e.stopPropagation()}>
        <div className="ot-goto-search-bar">
          <span className="ot-goto-search-icon">&#128269;</span>
          <input
            ref={inputRef}
            type="text"
            className="ot-goto-input"
            placeholder="Go To: Type a report, screen, ledger, group, stock item or voucher number..."
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
            data-testid="goto-search-input"
          />
          {loading && <span className="ot-goto-spinner">&#9696;</span>}
          <button type="button" className="ot-goto-close-btn" onClick={onClose}>
            &times;
          </button>
        </div>

        <div className="ot-goto-results-list" ref={listRef} data-testid="goto-results-list">
          {allItems.length === 0 ? (
            <div className="ot-goto-empty">
              No matching screens, ledgers, items or vouchers found for &quot;{query}&quot;.
            </div>
          ) : (
            allItems.map((item, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={`${item.category}-${item.id}-${idx}`}
                  data-index={idx}
                  className={`ot-goto-row ${isSelected ? 'ot-goto-row-selected' : ''}`}
                  onClick={() => handleSelect(item)}
                  onMouseEnter={() => setSelectedIndex(idx)}
                >
                  <span className={`ot-goto-cat-badge ot-cat-${item.category.toLowerCase().replace(/\s+/g, '-')}`}>
                    {item.category}
                  </span>
                  <div className="ot-goto-row-text">
                    <span className="ot-goto-row-title">{item.title}</span>
                    <span className="ot-goto-row-subtitle">{item.subtitle}</span>
                  </div>
                  {isSelected && <span className="ot-goto-row-enter">&#9166; Enter</span>}
                </div>
              );
            })
          )}
        </div>

        <div className="ot-goto-footer">
          <span>
            <kbd className="ot-kbd">&uarr;</kbd> <kbd className="ot-kbd">&darr;</kbd> Navigate
          </span>
          <span>
            <kbd className="ot-kbd">&#9166; Enter</kbd> Select
          </span>
          <span>
            <kbd className="ot-kbd">Esc</kbd> Dismiss
          </span>
        </div>
      </div>
    </div>
  );
};
