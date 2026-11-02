import React from 'react';
import { Company } from '../types';

interface ChartOfAccountsHeaderProps {
  activeCompany: Company | null;
  onBackToGateway: () => void;
}

export const ChartOfAccountsHeader: React.FC<ChartOfAccountsHeaderProps> = ({
  activeCompany,
  onBackToGateway,
}) => {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '12px 20px',
        backgroundColor: '#1e293b',
        borderBottom: '2px solid #0284c7',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
        <button
          onClick={onBackToGateway}
          style={{
            padding: '6px 12px',
            backgroundColor: '#334155',
            border: 'none',
            borderRadius: '4px',
            color: '#ffffff',
            cursor: 'pointer',
            fontWeight: 'bold',
          }}
          title="Return to Gateway (Esc)"
        >
          ← Gateway (Esc)
        </button>
        <h2 style={{ margin: 0, fontSize: '1.25rem', color: '#38bdf8' }}>
          Chart of Accounts
        </h2>
        {activeCompany && (
          <span
            style={{
              fontSize: '0.85rem',
              backgroundColor: '#0369a1',
              padding: '2px 8px',
              borderRadius: '4px',
              fontWeight: 'bold',
            }}
          >
            {activeCompany.name} ({activeCompany.financial_year_begins})
          </span>
        )}
      </div>

      <div style={{ fontSize: '0.8rem', color: '#94a3b8' }}>
        Hotkeys: <kbd>↑/↓</kbd> Navigate | <kbd>→</kbd> Expand | <kbd>←</kbd> Collapse |{' '}
        <kbd>Enter</kbd> Action | <kbd>Alt+C</kbd> Group | <kbd>Ctrl+L</kbd> Ledger |{' '}
        <kbd>Del</kbd> Delete
      </div>
    </div>
  );
};

