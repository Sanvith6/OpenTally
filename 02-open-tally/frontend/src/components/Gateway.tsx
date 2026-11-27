import { useState, useEffect } from 'react';

interface MenuItem {
  id: string;
  label: string;
  hotkey: string;
  category: string;
  sprint: string;
  description: string;
}

const MENU_ITEMS: MenuItem[] = [
  // MASTERS
  { id: 'acc_masters', label: 'Accounts Info', hotkey: 'A', category: 'MASTERS', sprint: 'Sprint 1', description: 'Chart of Accounts, Groups & Ledgers' },
  { id: 'inv_masters', label: 'Inventory Masters', hotkey: 'M', category: 'MASTERS', sprint: 'Sprint 4', description: 'Stock Groups, Categories, Items, Units & Godowns' },
  { id: 'party_masters', label: 'Party Masters', hotkey: '1', category: 'MASTERS', sprint: 'Sprint 6', description: 'Customer and supplier master records linked to ledgers' },
  { id: 'cost_centres', label: 'Cost Categories & Centres', hotkey: 'E', category: 'MASTERS', sprint: 'Milestone 11', description: 'Cost hierarchy setup and multi-dimensional tracking' },
  { id: 'budgets', label: 'Budgets & Scenarios', hotkey: '2', category: 'MASTERS', sprint: 'Milestone 12', description: 'Group, Ledger and Cost Centre budget targets' },
  { id: 'price_levels', label: 'Price Levels & Quantity Slabs', hotkey: '[', category: 'MASTERS', sprint: 'Milestone 20', description: 'Distributor price tiers, quantity slabs and effective dates' },

  // TRANSACTIONS
  { id: 'vouchers', label: 'Accounting Vouchers', hotkey: 'V', category: 'TRANSACTIONS', sprint: 'Sprint 2', description: 'Sales, Purchase, Payment, Receipt, Journal, Contra' },
  { id: 'sales_vouchers', label: 'Sales Vouchers', hotkey: 'S', category: 'TRANSACTIONS', sprint: 'Sprint 6', description: 'Sales document drafts and invoice line management' },
  { id: 'purchase_vouchers', label: 'Purchase Vouchers', hotkey: 'U', category: 'TRANSACTIONS', sprint: 'Milestone 1', description: 'Purchase document drafts, supplier bills and inventory receipts' },
  { id: 'inv_transactions', label: 'Inventory Vouchers', hotkey: 'N', category: 'TRANSACTIONS', sprint: 'Sprint 4', description: 'Receipts, deliveries, transfers, adjustments & stock register' },
  { id: 'stock_journal', label: 'Stock Journal (Alt+F7)', hotkey: 'J', category: 'TRANSACTIONS', sprint: 'Milestone 11', description: 'Inter-godown transfer & consumption/production vouchers' },
  { id: 'bank_reconciliation', label: 'Ban[k]ing (BRS)', hotkey: 'K', category: 'TRANSACTIONS', sprint: 'Milestone 8', description: 'Bank Allocations, Reconciliation Statement (BRS) & CSV Statement Matching' },
  { id: 'day_book', label: 'Day Book', hotkey: 'D', category: 'TRANSACTIONS', sprint: 'Sprint 2', description: 'Chronological transaction register' },
  { id: 'order_processing', label: 'Order Processing (Ctrl+F8/F9)', hotkey: '3', category: 'TRANSACTIONS', sprint: 'Milestone 13', description: 'Sales & Purchase Order processing, pending order tracking and invoice conversion' },
  { id: 'manufacturing_bom', label: 'Manufacturing & BOM', hotkey: '4', category: 'TRANSACTIONS', sprint: 'Milestone 14', description: 'Define multi-component BOM recipes, proportional scaling & manufacturing stock journals' },
  { id: 'job_work', label: 'Job-Work & GST ITC-04', hotkey: ']', category: 'TRANSACTIONS', sprint: 'Milestone 25', description: 'Material dispatch, linked receipts, scrap and statutory due-date review' },
  { id: 'payroll', label: 'Payroll (Ctrl+F4)', hotkey: 'Y', category: 'TRANSACTIONS', sprint: 'Milestone 15', description: 'Employee masters, LOP proration, EPF/ESI/PT statutory computation & A4 Payslips' },

  // UTILITIES
  { id: 'company_features', label: 'Company Features (F11)', hotkey: 'F', category: 'UTILITIES', sprint: 'Milestone 7', description: 'Configure Accounting, Inventory, Indian GST & Auto-Seeding' },
  { id: 'backup_restore', label: 'Backup & Restore', hotkey: 'X', category: 'UTILITIES', sprint: 'Milestone 5', description: 'WAL-safe SQLite backup creation and SHA-256 verified restore' },
  { id: 'tally_xml', label: 'Import / Export (Tally XML)', hotkey: 'I', category: 'UTILITIES', sprint: 'Milestone 6', description: 'File-based Tally XML import with dry-run preview and export' },
  { id: 'year_closing_audit', label: 'Year Closing & MCA Audit', hotkey: 'Z', category: 'UTILITIES', sprint: 'Milestone 12', description: 'Financial year rollover, period locking and forensic audit trail' },

  // REPORTS
  { id: 'balance_sheet', label: 'Balance Sheet', hotkey: 'B', category: 'REPORTS', sprint: 'Sprint 3', description: 'Assets, Liabilities & Capital Position' },
  { id: 'profit_and_loss', label: 'Profit & Loss A/c', hotkey: 'P', category: 'REPORTS', sprint: 'Sprint 3', description: 'Operating Income & Expense Statements' },
  { id: 'trial_balance', label: 'Trial Balance', hotkey: 'T', category: 'REPORTS', sprint: 'Sprint 3', description: 'Real-time Debit/Credit Ledger Equilibrium' },
  { id: 'cash_flow', label: 'Cash Flow', hotkey: 'C', category: 'REPORTS', sprint: 'Sprint 3', description: 'Operating, investing and financing cash movement' },
  { id: 'ledger_report', label: 'Ledger Report', hotkey: 'L', category: 'REPORTS', sprint: 'Sprint 3', description: 'Account-wise transaction and running balance' },
  { id: 'outstanding_aging', label: 'Outstanding & Aging', hotkey: 'O', category: 'REPORTS', sprint: 'Milestone 2', description: 'Bill-wise receivables, payables and aging analysis' },
  { id: 'taxation', label: 'GST Reports (GSTR-1 / 3B)', hotkey: 'G', category: 'REPORTS', sprint: 'Milestone 3', description: 'GSTR-1 outward supplies, GSTR-3B monthly return and GST masters' },
  { id: 'tds_reports', label: 'TDS & Form 26Q Reports', hotkey: '8', category: 'REPORTS', sprint: 'Milestone 9', description: 'TDS Outstanding Remittance, Challan Tagging & Form 26Q Filing' },
  { id: 'inv_reports', label: 'Inventory Reports', hotkey: 'R', category: 'REPORTS', sprint: 'Sprint 4', description: 'Stock summary, register, ledger and reconciliation' },
  { id: 'inv_valuation', label: 'Inventory Valuation', hotkey: '5', category: 'REPORTS', sprint: 'Sprint 4', description: 'FIFO and weighted average stock valuation' },
  { id: 'inv_accounting', label: 'Inventory Accounting', hotkey: '6', category: 'REPORTS', sprint: 'Sprint 5', description: 'Configure and integrate inventory accounting effects' },
  { id: 'godown_summary', label: 'Godown Summary', hotkey: 'W', category: 'REPORTS', sprint: 'Milestone 11', description: 'Godown-wise stock balance, inflow/outflow & valuation' },
  { id: 'batch_expiry', label: 'Batch & Expiry Report', hotkey: '7', category: 'REPORTS', sprint: 'Milestone 11', description: 'Batch tracking, shelf-life and expiry date monitoring' },
  { id: 'cost_centre_breakup', label: 'Cost Centre Breakup', hotkey: 'H', category: 'REPORTS', sprint: 'Milestone 11', description: 'Voucher expense and revenue allocations by cost centre' },
  { id: 'ratio_analysis', label: 'Ratio Analysis', hotkey: 'Q', category: 'REPORTS', sprint: 'Milestone 12', description: 'Principal groups, liquidity, solvency and profitability ratios' },

  // ENTERPRISE
  { id: 'multi_currency', label: 'Multi-Currency & Forex', hotkey: '9', category: 'ENTERPRISE', sprint: 'Milestone 16', description: 'Currencies, daily exchange rates, foreign bill settlement and unrealized forex revaluation' },
  { id: 'security_control', label: 'Security Control (Users & Passwords)', hotkey: '0', category: 'ENTERPRISE', sprint: 'Milestone 14', description: 'Enable company security, manage user roles (Admin, Accountant, DataEntry, Auditor) & permissions' },
  { id: 'lan_server', label: 'LAN Host / Multi-User (Gold)', hotkey: '-', category: 'ENTERPRISE', sprint: 'Milestone 17', description: 'Start/stop opt-in LAN HTTP/JSON-RPC server on port 7900 & manage active client sessions' },
  { id: 'custom_reports', label: 'Custom Report Extensions', hotkey: '=', category: 'ENTERPRISE', sprint: 'Milestone 18', description: 'Declarative custom report builder & .opentally-report.json templates' },
];

interface GatewayProps {
  onSelectItem: (item: MenuItem) => void;
}

function renderLabelWithHotkey(label: string, hotkey: string) {
  const idx = label.toUpperCase().indexOf(hotkey.toUpperCase());
  if (idx === -1) {
    return (
      <>
        <span className="ot-hotkey">[{hotkey}]</span> {label}
      </>
    );
  }
  return (
    <>
      {label.substring(0, idx)}
      <span className="ot-hotkey">{label.charAt(idx)}</span>
      {label.substring(idx + 1)}
    </>
  );
}

export function Gateway({ onSelectItem }: GatewayProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't intercept if an input is focused or modifier keys pressed
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement ||
        e.altKey ||
        e.ctrlKey ||
        e.metaKey ||
        document.querySelector('.ot-modal-overlay, .ot-modal-backdrop, .ot-lan-modal-backdrop, [role="dialog"]')
      ) {
        return;
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % MENU_ITEMS.length);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + MENU_ITEMS.length) % MENU_ITEMS.length);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        onSelectItem(MENU_ITEMS[selectedIndex]);
      } else {
        // Direct single key hotkey navigation
        const key = e.key.toUpperCase();
        const matchedIndex = MENU_ITEMS.findIndex((item) => item.hotkey === key);
        if (matchedIndex !== -1) {
          e.preventDefault();
          setSelectedIndex(matchedIndex);
          onSelectItem(MENU_ITEMS[matchedIndex]);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedIndex, onSelectItem]);

  // Group items by category
  const categories = Array.from(new Set(MENU_ITEMS.map((item) => item.category)));

  return (
    <div className="ot-gateway-container">
      <div className="ot-gateway-header">
        <h2>Gateway of OpenTally</h2>
        <span className="ot-keyboard-guide">Use &uarr; &darr; arrows, hotkey letters, or Enter</span>
      </div>

      <div className="ot-gateway-menu">
        {categories.map((cat) => {
          const items = MENU_ITEMS.filter((item) => item.category === cat);
          return (
            <div key={cat} className="ot-menu-section">
              <div className="ot-section-title">{cat}</div>
              <div className="ot-section-items">
                {items.map((item) => {
                  const itemIndex = MENU_ITEMS.indexOf(item);
                  const isSelected = itemIndex === selectedIndex;
                  return (
                    <div
                      key={item.id}
                      data-item-id={item.id}
                      data-hotkey={item.hotkey}
                      className={`ot-menu-row ${isSelected ? 'ot-selected' : ''}`}
                      onClick={() => {
                        setSelectedIndex(itemIndex);
                        onSelectItem(item);
                      }}
                    >
                      <span className="ot-item-label">
                        {renderLabelWithHotkey(item.label, item.hotkey)}
                      </span>
                      <span className="ot-item-badge">{item.sprint}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>

      <div className="ot-gateway-footer">
        <div className="ot-selected-description">
          <strong>{MENU_ITEMS[selectedIndex].label}</strong>: {MENU_ITEMS[selectedIndex].description}
        </div>
      </div>
    </div>
  );
};

