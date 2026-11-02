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
  // Masters
  { id: 'acc_masters', label: 'Accounts Info', hotkey: 'A', category: 'MASTERS', sprint: 'Sprint 1', description: 'Chart of Accounts, Groups & Ledgers' },
  { id: 'inv_masters', label: 'Inventory Info', hotkey: 'I', category: 'MASTERS', sprint: 'Sprint 3', description: 'Stock Items, Units of Measure & Godowns' },

  // Transactions
  { id: 'vouchers', label: 'Accounting Vouchers', hotkey: 'V', category: 'TRANSACTIONS', sprint: 'Sprint 2', description: 'Sales, Purchase, Payment, Receipt, Journal, Contra' },
  { id: 'day_book', label: 'Day Book', hotkey: 'D', category: 'TRANSACTIONS', sprint: 'Sprint 2', description: 'Chronological transaction register' },

  // Utilities
  { id: 'banking', label: 'Banking & BRS', hotkey: 'K', category: 'UTILITIES', sprint: 'Sprint 4', description: 'Bank Reconciliation & Cheque Advice' },
  { id: 'taxation', label: 'Statutory & GST', hotkey: 'G', category: 'UTILITIES', sprint: 'Sprint 3', description: 'GSTR-1, GSTR-3B & Tax Slabs' },

  // Reports
  { id: 'balance_sheet', label: 'Balance Sheet', hotkey: 'B', category: 'REPORTS', sprint: 'Sprint 4', description: 'Assets, Liabilities & Capital Position' },
  { id: 'pnl', label: 'Profit & Loss A/c', hotkey: 'P', category: 'REPORTS', sprint: 'Sprint 4', description: 'Operating Income & Expense Statements' },
  { id: 'trial_balance', label: 'Trial Balance', hotkey: 'T', category: 'REPORTS', sprint: 'Sprint 4', description: 'Real-time Debit/Credit Ledger Equilibrium' },
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
      if (e.target instanceof HTMLInputElement || e.altKey || e.ctrlKey || e.metaKey) {
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

