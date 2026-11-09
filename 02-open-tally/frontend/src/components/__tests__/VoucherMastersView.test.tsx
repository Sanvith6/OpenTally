import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { VoucherMastersView } from '../VoucherMastersView';
import * as api from '../../services/api';
import { Company, VoucherType, VoucherNumberSeries } from '../../types';

const mockCompany: Company = {
  id: 'cmp_test_01',
  name: 'Acme Accounting Pvt Ltd',
  mailing_name: 'Acme HQ Mumbai',
  country: 'India',
  state: 'Maharashtra',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockVoucherTypes: VoucherType[] = [
  {
    id: 'vt_sales',
    company_id: 'cmp_test_01',
    name: 'Sales',
    category: 'accounting',
    parent_type: null,
    affects_ledger_balance: true,
    affects_inventory: false,
    is_predefined: true,
    is_active: true,
    numbering_method: 'automatic',
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'vt_payment',
    company_id: 'cmp_test_01',
    name: 'Payment',
    category: 'accounting',
    parent_type: null,
    affects_ledger_balance: true,
    affects_inventory: false,
    is_predefined: true,
    is_active: true,
    numbering_method: 'automatic',
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'vt_delivery_note',
    company_id: 'cmp_test_01',
    name: 'Delivery Note',
    category: 'inventory',
    parent_type: null,
    affects_ledger_balance: false,
    affects_inventory: true,
    is_predefined: true,
    is_active: true,
    numbering_method: 'automatic',
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockSeriesList: VoucherNumberSeries[] = [
  {
    id: 'ser_sales_tax',
    company_id: 'cmp_test_01',
    voucher_type_id: 'vt_sales',
    series_name: 'Main Tax Invoice',
    series_code: 'TAX',
    numbering_method: 'automatic',
    restart_period: 'yearly',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'ser_sales_exp',
    company_id: 'cmp_test_01',
    voucher_type_id: 'vt_sales',
    series_name: 'Export Sales',
    series_code: 'EXP',
    numbering_method: 'manual',
    restart_period: 'never',
    is_active: false,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

describe('VoucherMastersView Component', () => {
  const onBackToGateway = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listVoucherTypes').mockResolvedValue(mockVoucherTypes);
    vi.spyOn(api, 'listVoucherSeries').mockResolvedValue(mockSeriesList);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders header, breadcrumbs, and voucher types list', async () => {
    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    expect(screen.getByText(/Voucher Type & Numbering-Series Masters/i)).toBeDefined();
    expect(screen.getByText(/Acme Accounting Pvt Ltd/i)).toBeDefined();

    await waitFor(() => {
      expect(screen.getAllByText('Sales')[0]).toBeDefined();
      expect(screen.getByText('Payment')).toBeDefined();
      expect(screen.getByText('Delivery Note')).toBeDefined();
    });
  });

  it('displays series table for the selected voucher type', async () => {
    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getByText('Main Tax Invoice')).toBeDefined();
      expect(screen.getByText('TAX')).toBeDefined();
      expect(screen.getByText('Export Sales')).toBeDefined();
      expect(screen.getByText('EXP')).toBeDefined();
    });
  });

  it('filters voucher types with the search input', async () => {
    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getAllByText('Sales')[0]).toBeDefined();
    });

    const searchInput = screen.getByPlaceholderText(/Search 24 Voucher Types/i);
    fireEvent.change(searchInput, { target: { value: 'Delivery' } });

    expect(screen.getByText('Delivery Note')).toBeDefined();
    expect(screen.queryByText('Payment')).toBeNull();
  });

  it('opens Create Series modal and validates mandatory restart_period', async () => {
    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getByText('Main Tax Invoice')).toBeDefined();
    });

    const createBtn = screen.getByRole('button', { name: /\+ Create Series/i });
    fireEvent.click(createBtn);

    expect(screen.getByRole('heading', { name: /Create Numbering Series/i })).toBeDefined();

    // Fill form without selecting restart_period
    const nameInput = screen.getByLabelText(/Series Name \*/i);
    const codeInput = screen.getByLabelText(/Series Code/i);
    fireEvent.change(nameInput, { target: { value: 'Online Sales' } });
    fireEvent.change(codeInput, { target: { value: 'ONL' } });

    const submitBtn = screen.getByRole('button', { name: /Create Numbering Series/i });
    fireEvent.click(submitBtn);

    // Expect validation message regarding restart period
    await waitFor(() => {
      expect(
        screen.getByText(/Explicit selection of Restart Period is mandatory/i)
      ).toBeDefined();
    });
  });

  it('creates numbering series when form is valid', async () => {
    const createSpy = vi.spyOn(api, 'createVoucherSeries').mockResolvedValue({
      id: 'ser_sales_onl',
      company_id: 'cmp_test_01',
      voucher_type_id: 'vt_sales',
      series_name: 'Online Sales',
      series_code: 'ONL',
      numbering_method: 'automatic',
      restart_period: 'yearly',
      is_active: true,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    });

    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getByText('Main Tax Invoice')).toBeDefined();
    });

    fireEvent.click(screen.getByRole('button', { name: /\+ Create Series/i }));

    fireEvent.change(screen.getByLabelText(/Series Name \*/i), { target: { value: 'Online Sales' } });
    fireEvent.change(screen.getByLabelText(/Series Code/i), { target: { value: 'ONL' } });
    fireEvent.change(screen.getByLabelText(/Restart Period \*/i), { target: { value: 'yearly' } });

    fireEvent.click(screen.getByRole('button', { name: /Create Numbering Series/i }));

    await waitFor(() => {
      expect(createSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          voucher_type_id: 'vt_sales',
          series_name: 'Online Sales',
          series_code: 'ONL',
          restart_period: 'yearly',
        })
      );
    });
  });

  it('opens Edit modal and updates series details', async () => {
    const updateSpy = vi.spyOn(api, 'updateVoucherSeries').mockResolvedValue({
      ...mockSeriesList[0],
      series_name: 'Renamed Tax Invoice',
    });

    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getByText('Main Tax Invoice')).toBeDefined();
    });

    const editBtns = screen.getAllByRole('button', { name: /^Edit$/i });
    fireEvent.click(editBtns[0]);

    expect(screen.getByText(/Edit Series — TAX/i)).toBeDefined();

    const nameInput = screen.getByLabelText(/Series Name \*/i);
    fireEvent.change(nameInput, { target: { value: 'Renamed Tax Invoice' } });

    fireEvent.click(screen.getByRole('button', { name: /Save Changes/i }));

    await waitFor(() => {
      expect(updateSpy).toHaveBeenCalledWith(
        'ser_sales_tax',
        expect.objectContaining({
          series_name: 'Renamed Tax Invoice',
        })
      );
    });
  });

  it('opens Deactivate confirmation modal and confirms deactivation', async () => {
    const deactivateSpy = vi.spyOn(api, 'deactivateVoucherSeries').mockResolvedValue({
      ...mockSeriesList[0],
      is_active: false,
    });

    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getByText('Main Tax Invoice')).toBeDefined();
    });

    const deactBtn = screen.getByRole('button', { name: /^Deactivate$/i });
    fireEvent.click(deactBtn);

    expect(screen.getByText(/Deactivate Numbering Series/i)).toBeDefined();
    expect(
      screen.getByText(/Deactivation prevents newly created vouchers from selecting this series/i)
    ).toBeDefined();

    const confirmBtn = screen.getByRole('button', { name: /Confirm Deactivation/i });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(deactivateSpy).toHaveBeenCalledWith('ser_sales_tax');
    });
  });

  it('calls onBackToGateway when Escape is pressed and no modal is open', async () => {
    render(<VoucherMastersView activeCompany={mockCompany} onBackToGateway={onBackToGateway} />);

    await waitFor(() => {
      expect(screen.getAllByText('Sales')[0]).toBeDefined();
    });

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBackToGateway).toHaveBeenCalled();
  });
});
