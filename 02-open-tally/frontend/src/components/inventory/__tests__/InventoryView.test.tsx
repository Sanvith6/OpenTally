import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { InventoryView } from '../InventoryView';

const company = { id: 'c1', name: 'Inventory Co', mailing_name: 'Inventory Co', country: 'India', state: 'Delhi', financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01', currency_symbol: '₹', currency_formal_name: 'INR', currency_decimal_places: 2, status: 'active' as const, created_at: '', updated_at: '' };

describe('InventoryView', () => {
  it('renders each master section and empty state in browser preview', async () => {
    render(<InventoryView activeCompany={company} onBack={() => undefined} />);
    expect((await screen.findByText('No stock groups found.')).textContent).toBe('No stock groups found.');
    for (const label of ['Categories', 'Units', 'Stock Items', 'Godowns']) {
      fireEvent.click(screen.getByRole('button', { name: label }));
      expect((await screen.findByText(`No ${label.toLowerCase()} found.`)).textContent).toBe(`No ${label.toLowerCase()} found.`);
    }
  });

  it('supports search and back navigation controls', () => {
    const onBack = () => undefined;
    render(<InventoryView activeCompany={company} onBack={onBack} />);
    const search = screen.getByRole('textbox', { name: 'Search inventory' });
    fireEvent.change(search, { target: { value: 'rice' } });
    expect((search as HTMLInputElement).value).toBe('rice');
    expect(screen.getByRole('button', { name: 'Back to Gateway' }).textContent).toBe('Back to Gateway');
  });
});
