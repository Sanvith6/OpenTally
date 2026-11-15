import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PartyManagementView } from '../PartyManagementView';

const company = { id: 'c1', name: 'Party Co', mailing_name: 'Party Co', country: 'India', state: 'Delhi', financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01', currency_symbol: '₹', currency_formal_name: 'INR', currency_decimal_places: 2, status: 'active' as const, created_at: '', updated_at: '' };

describe('PartyManagementView', () => {
  it('renders isolated master form, unavailable utilization notice, and empty state', async () => {
    render(<PartyManagementView activeCompany={company} onBack={() => undefined} />);
    expect(await screen.findByText('No parties found.')).toBeTruthy();
    expect(screen.getByText(/Credit-limit utilization is unavailable/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Back to Gateway' })).toBeTruthy();
  });

  it('validates required ledger, identity, and role fields before create', () => {
    render(<PartyManagementView activeCompany={company} onBack={() => undefined} />);
    fireEvent.click(screen.getByRole('button', { name: 'Create party' }));
    expect(screen.getByRole('alert').textContent).toContain('Ledger, code, name');
  });

  it('supports search input and role toggles', () => {
    render(<PartyManagementView activeCompany={company} onBack={() => undefined} />);
    const search = screen.getByRole('textbox', { name: 'Search parties' });
    fireEvent.change(search, { target: { value: 'Acme' } });
    expect((search as HTMLInputElement).value).toBe('Acme');
    const supplier = screen.getByRole('checkbox', { name: 'Supplier' }) as HTMLInputElement;
    expect(supplier.checked).toBe(false);
    fireEvent.click(supplier);
    expect(supplier.checked).toBe(true);
  });
});
