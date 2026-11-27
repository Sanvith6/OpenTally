import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PriceLevelsView } from '../PriceLevelsView';
import * as api from '../../../services/api';
import { Company } from '../../../types';

const company: Company = {
  id: 'cmp_1',
  name: 'Distributor Ltd',
  mailing_name: 'Distributor Ltd',
  country: 'India',
  state: 'Maharashtra',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '',
  updated_at: '',
};

describe('Milestone 20: PriceLevelsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listPriceLevels').mockResolvedValue([
      { id: 'pl_1', company_id: 'cmp_1', name: 'Wholesale', is_active: 1, created_at: '' },
    ]);
    vi.spyOn(api, 'listPriceListEntries').mockResolvedValue([
      {
        id: 'ple_1',
        company_id: 'cmp_1',
        price_level_id: 'pl_1',
        stock_item_id: 'it_1',
        effective_from: '2026-04-01',
        min_qty: '10',
        max_qty: null,
        unit_rate: '85.00',
        discount_pct: '5.00',
      },
    ]);
    vi.spyOn(api, 'listStockItems').mockResolvedValue([
      {
        id: 'it_1',
        company_id: 'cmp_1',
        name: 'FMCG Soap',
        base_unit_id: 'u_nos',
        standard_selling_rate: '100.00',
      } as any,
    ]);
  });

  it('renders price levels and quantity slabs and creates a new price level', async () => {
    const createSpy = vi.spyOn(api, 'createPriceLevel').mockResolvedValue({
      id: 'pl_2',
      company_id: 'cmp_1',
      name: 'Dealer',
      is_active: 1,
      created_at: '',
    });

    render(<PriceLevelsView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByTestId('price-levels-view')).toBeTruthy();
    expect(await screen.findByText('85.00')).toBeTruthy();

    fireEvent.change(screen.getByLabelText('New Price Level Name'), { target: { value: 'Dealer' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create Price Level' }));
    expect(createSpy).toHaveBeenCalledWith('Dealer');
  });
});
