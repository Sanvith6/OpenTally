import React from 'react';
import { Company, OpeningBalanceEquilibrium } from '../types';

interface EquilibriumStatusBarProps {
  activeCompany: Company | null;
  equilibrium: OpeningBalanceEquilibrium | null;
}

export const EquilibriumStatusBar: React.FC<EquilibriumStatusBarProps> = ({
  activeCompany,
  equilibrium,
}) => {
  const currencySymbol = activeCompany?.currency_symbol || '₹';

  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: '10px 16px',
        backgroundColor: '#1e293b',
        borderTop: '1px solid #334155',
        fontSize: '13px',
      }}
    >
      <div>
        <span style={{ color: '#94a3b8' }}>Active Company: </span>
        <span style={{ fontWeight: 'bold', color: '#ffffff' }}>
          {activeCompany ? activeCompany.name : 'None'}
        </span>
      </div>

      {equilibrium && (
        <div style={{ display: 'flex', gap: '24px', alignItems: 'center' }}>
          <div>
            <span style={{ color: '#94a3b8' }}>Total Opening Dr: </span>
            <span style={{ fontWeight: 'bold', color: '#60a5fa', fontFamily: 'monospace' }}>
              {currencySymbol} {equilibrium.total_debit}
            </span>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Total Opening Cr: </span>
            <span style={{ fontWeight: 'bold', color: '#f87171', fontFamily: 'monospace' }}>
              {currencySymbol} {equilibrium.total_credit}
            </span>
          </div>
          <div>
            <span style={{ color: '#94a3b8' }}>Difference in Balances: </span>
            {equilibrium.is_balanced ? (
              <span style={{ fontWeight: 'bold', color: '#4ade80' }}>
                Balanced (0.00)
              </span>
            ) : (
              <span
                style={{
                  fontWeight: 'bold',
                  color: '#fbbf24',
                  fontFamily: 'monospace',
                  backgroundColor: 'rgba(245, 158, 11, 0.1)',
                  padding: '2px 8px',
                  borderRadius: '4px',
                  border: '1px solid #d97706',
                }}
              >
                {currencySymbol} {equilibrium.difference} {equilibrium.difference_type || ''}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

