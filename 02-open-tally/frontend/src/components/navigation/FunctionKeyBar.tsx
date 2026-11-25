import React from 'react';
import './FunctionKeyBar.css';

export interface FunctionKeyItem {
  keyLabel: string;
  actionLabel: string;
  onClick: () => void;
  disabled?: boolean;
  highlight?: boolean;
}

interface FunctionKeyBarProps {
  onDateClick?: () => void;
  onPeriodClick?: () => void;
  onCompanyClick?: () => void;
  onContraClick?: () => void;
  onPayrollClick?: () => void;
  onPaymentClick?: () => void;
  onReceiptClick?: () => void;
  onDebitNoteClick?: () => void;
  onCreditNoteClick?: () => void;
  onJournalClick?: () => void;
  onStockJournalClick?: () => void;
  onSalesClick?: () => void;
  onSalesOrderClick?: () => void;
  onPurchaseClick?: () => void;
  onPurchaseOrderClick?: () => void;
  onGoToClick?: () => void;
  onFeaturesClick?: () => void;
  onAcceptClick?: () => void;
  onBackClick?: () => void;
  canBack?: boolean;
}

export const FunctionKeyBar: React.FC<FunctionKeyBarProps> = ({
  onDateClick,
  onPeriodClick,
  onCompanyClick,
  onContraClick,
  onPayrollClick,
  onPaymentClick,
  onReceiptClick,
  onDebitNoteClick,
  onCreditNoteClick,
  onJournalClick,
  onStockJournalClick,
  onSalesClick,
  onSalesOrderClick,
  onPurchaseClick,
  onPurchaseOrderClick,
  onGoToClick,
  onFeaturesClick,
  onAcceptClick,
  onBackClick,
  canBack = true,
}) => {
  const keys: FunctionKeyItem[] = [
    { keyLabel: 'F2', actionLabel: 'Date', onClick: onDateClick || (() => {}) },
    { keyLabel: 'Alt+F2', actionLabel: 'Period', onClick: onPeriodClick || (() => {}) },
    { keyLabel: 'F3', actionLabel: 'Company', onClick: onCompanyClick || (() => {}) },
    { keyLabel: 'F4', actionLabel: 'Contra', onClick: onContraClick || (() => {}) },
    { keyLabel: 'Ctrl+F4', actionLabel: 'Payroll', onClick: onPayrollClick || (() => {}) },
    { keyLabel: 'F5', actionLabel: 'Payment', onClick: onPaymentClick || (() => {}) },
    { keyLabel: 'Alt+F5', actionLabel: 'Debit Note', onClick: onDebitNoteClick || (() => {}) },
    { keyLabel: 'F6', actionLabel: 'Receipt', onClick: onReceiptClick || (() => {}) },
    { keyLabel: 'Alt+F6', actionLabel: 'Credit Note', onClick: onCreditNoteClick || (() => {}) },
    { keyLabel: 'F7', actionLabel: 'Journal', onClick: onJournalClick || (() => {}) },
    { keyLabel: 'Alt+F7', actionLabel: 'Stock Jnl', onClick: onStockJournalClick || (() => {}) },
    { keyLabel: 'F8', actionLabel: 'Sales', onClick: onSalesClick || (() => {}) },
    { keyLabel: 'Ctrl+F8', actionLabel: 'Sales Order', onClick: onSalesOrderClick || (() => {}) },
    { keyLabel: 'F9', actionLabel: 'Purchase', onClick: onPurchaseClick || (() => {}) },
    { keyLabel: 'Ctrl+F9', actionLabel: 'Pur Order', onClick: onPurchaseOrderClick || (() => {}) },
    { keyLabel: 'F11', actionLabel: 'Features', onClick: onFeaturesClick || (() => {}) },
    { keyLabel: 'Alt+G', actionLabel: 'Go To', onClick: onGoToClick || (() => {}), highlight: true },
    { keyLabel: 'Ctrl+A', actionLabel: 'Accept', onClick: onAcceptClick || (() => {}) },
    { keyLabel: 'Esc', actionLabel: 'Back', onClick: onBackClick || (() => {}), disabled: !canBack },
  ];

  return (
    <aside className="ot-function-bar" aria-label="Tally function shortcuts" data-testid="function-key-bar">
      <div className="ot-function-bar-header">
        <span className="ot-function-bar-title">SHORTCUTS</span>
      </div>
      <div className="ot-function-bar-buttons">
        {keys.map((k) => (
          <button
            key={k.keyLabel}
            type="button"
            className={`ot-function-btn ${k.highlight ? 'ot-function-btn-highlight' : ''}`}
            onClick={k.onClick}
            disabled={k.disabled}
            title={`${k.keyLabel}: ${k.actionLabel}`}
          >
            <span className="ot-fn-key">{k.keyLabel}</span>
            <span className="ot-fn-label">{k.actionLabel}</span>
          </button>
        ))}
      </div>
    </aside>
  );
};
