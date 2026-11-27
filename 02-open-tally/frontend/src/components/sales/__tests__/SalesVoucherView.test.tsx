import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { SalesVoucherView } from '../SalesVoucherView';

const company = { id: 'c1', name: 'Sales Co', mailing_name: 'Sales Co', country: 'India', state: 'Delhi', financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01', currency_symbol: '₹', currency_formal_name: 'INR', currency_decimal_places: 2, status: 'active' as const, created_at: '', updated_at: '' };

describe('SalesVoucherView', () => {
  it('renders the draft workflow and posting notice', async () => {
    render(<SalesVoucherView activeCompany={company} onBack={() => undefined} />);
    expect(await screen.findByText('No sales drafts found.')).toBeTruthy();
    expect(screen.getByText(/Atomic GL \+ FIFO Inventory posting is active/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Save draft' })).toBeTruthy();
  });

  it('validates customer, series, and line description before save', () => {
    render(<SalesVoucherView activeCompany={company} onBack={() => undefined} />);
    fireEvent.click(screen.getByRole('button', { name: 'Save draft' }));
    expect(screen.getByRole('alert').textContent).toContain('Customer, series');
  });

  it('supports accounting-only and inventory line controls', () => {
    render(<SalesVoucherView activeCompany={company} onBack={() => undefined} />);
    expect(screen.getByRole('combobox', { name: 'Stock item 1' })).toBeTruthy();
    expect(screen.getByLabelText('Billed quantity 1')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Add line' }));
    expect(screen.getAllByPlaceholderText('Line description')).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Back to Gateway' })).toBeTruthy();
  });
});
