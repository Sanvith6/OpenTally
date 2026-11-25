import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { StockJournalView } from '../StockJournalView';
import * as api from '../../../services/api';
import { Company, Godown, StockItem, StockJournalResponse } from '../../../types';

const mockCompany: Company = {
  id: 'cmp_test',
  name: 'Test Enterprise Ltd',
  mailing_name: 'Test Enterprise Ltd',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockItems: StockItem[] = [
  {
    id: 'item_1',
    company_id: 'cmp_test',
    name: 'Copper Wire 2.5mm',
    stock_group_id: 'grp_1',
    stock_category_id: null,
    base_unit_id: 'unit_mtr',
    alternate_unit_id: null,
    sku: 'CW-25',
    description: null,
    opening_quantity: '100',
    opening_rate: '50.00',
    opening_value: '5000.00',
    reorder_level: '20',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockGodowns: Godown[] = [
  {
    id: 'gdn_central',
    company_id: 'cmp_test',
    name: 'Central Godown',
    parent_id: null,
    description: 'Main warehouse',
    address: 'Peenya, Bangalore',
    is_external: false,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'gdn_suburban',
    company_id: 'cmp_test',
    name: 'Suburban Depot',
    parent_id: null,
    description: 'Branch warehouse',
    address: 'Whitefield, Bangalore',
    is_external: true,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockPastVouchers: StockJournalResponse[] = [
  {
    voucher_id: 'sj_vch_1',
    voucher_number: 'SJ-2026-001',
    voucher_date: '2026-04-10',
    total_source_value: '5000.00',
    total_destination_value: '5000.00',
    lines_count: 2,
  },
];

describe('StockJournalView (Alt+F7)', () => {
  const onBack = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listStockItems').mockResolvedValue(mockItems);
    vi.spyOn(api, 'listGodowns').mockResolvedValue(mockGodowns);
    vi.spyOn(api, 'listStockJournalVouchers').mockResolvedValue(mockPastVouchers);
    vi.spyOn(api, 'postStockJournalVoucher').mockResolvedValue({
      voucher_id: 'sj_new_1',
      voucher_number: 'SJ-2026-002',
      voucher_date: '2026-04-15',
      total_source_value: '250.00',
      total_destination_value: '250.00',
      lines_count: 2,
    });
  });

  it('renders dual-column layout with Source and Destination tables', async () => {
    render(<StockJournalView activeCompany={mockCompany} onBack={onBack} />);

    expect(screen.getByText('Stock Journal (Alt+F7)')).toBeTruthy();
    expect(screen.getByText('Source (Consumption / Transfer Out)')).toBeTruthy();
    expect(screen.getByText('Destination (Production / Transfer In)')).toBeTruthy();

    await waitFor(() => {
      expect(api.listStockItems).toHaveBeenCalled();
      expect(api.listGodowns).toHaveBeenCalled();
    });
  });

  it('triggers onBack when Esc button or key is pressed', () => {
    render(<StockJournalView activeCompany={mockCompany} onBack={onBack} />);

    fireEvent.click(screen.getByText('Back (Esc)'));
    expect(onBack).toHaveBeenCalled();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(2);
  });

  it('switches between Voucher Entry and Journal Register tabs', async () => {
    render(<StockJournalView activeCompany={mockCompany} onBack={onBack} />);

    const registerTab = screen.getByText('Journal Register');
    fireEvent.click(registerTab);

    await waitFor(() => {
      expect(screen.getByText('SJ-2026-001')).toBeTruthy();
      expect(screen.getByText(/2\s*lines/i)).toBeTruthy();
    });
  });
});
