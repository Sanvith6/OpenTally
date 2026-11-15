import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { InventoryAccountingView } from '../InventoryAccountingView';

const company = { id: 'c1', name: 'Accounting Co', mailing_name: 'Accounting Co', country: 'India', state: 'Delhi', financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01', currency_symbol: '₹', currency_formal_name: 'INR', currency_decimal_places: 2, status: 'active' as const, created_at: '', updated_at: '' };

describe('InventoryAccountingView', () => {
  it('renders mapping, preview, and integration controls', () => {
    render(<InventoryAccountingView activeCompany={company} onBack={() => undefined} />);
    expect(screen.getByText('Account Mapping')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Save Mapping' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Inspect' })).toBeTruthy();
    expect(screen.getByText('Integration Register')).toBeTruthy();
  });
});
