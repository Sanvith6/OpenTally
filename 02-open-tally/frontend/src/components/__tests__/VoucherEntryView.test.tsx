import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { VoucherEntryView } from '../vouchers/VoucherEntryView';
import * as api from '../../services/api';
import { Company, VoucherType, VoucherNumberSeries, Ledger, VoucherWithLines } from '../../types';

const mockCompany: Company = {
  id: 'cmp_test_01',
  name: 'Acme Test Corp',
  mailing_name: 'Acme HQ',
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
    id: 'vt_contra',
    company_id: 'cmp_test_01',
    name: 'Contra',
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
];

const mockSeries: VoucherNumberSeries[] = [
  {
    id: 'ser_sales_01',
    company_id: 'cmp_test_01',
    voucher_type_id: 'vt_sales',
    series_name: 'General Sales',
    series_code: 'GEN',
    numbering_method: 'automatic',
    restart_period: 'yearly',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'ser_pay_01',
    company_id: 'cmp_test_01',
    voucher_type_id: 'vt_payment',
    series_name: 'Bank Payments',
    series_code: 'BP',
    numbering_method: 'automatic',
    restart_period: 'yearly',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockLedgers: Ledger[] = [
  {
    id: 'led_cash',
    company_id: 'cmp_test_01',
    group_id: 'grp_01',
    name: 'Cash in Hand',
    ledger_classification: 'cash',
    is_reserved: true,
    created_at: '',
    updated_at: '',
  },
  {
    id: 'led_sales',
    company_id: 'cmp_test_01',
    group_id: 'grp_02',
    name: 'Domestic Sales A/c',
    ledger_classification: 'standard',
    is_reserved: false,
    created_at: '',
    updated_at: '',
  },
];

describe('VoucherEntryView Component (Sprint 2 Stage 4)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(api, 'listVoucherTypes').mockResolvedValue(mockVoucherTypes);
    vi.spyOn(api, 'listVoucherSeries').mockImplementation(async (typeId) =>
      mockSeries.filter((s) => s.voucher_type_id === typeId)
    );
    vi.spyOn(api, 'listLedgers').mockResolvedValue(mockLedgers);
  });

  it('renders voucher entry workspace with breadcrumbs and default header fields', async () => {
    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    expect(screen.getByText('Loading voucher workspace...')).toBeDefined();

    await waitFor(() => {
      expect(screen.getByText('Accounting Vouchers')).toBeDefined();
      expect(screen.getByText('PROVISIONAL:')).toBeDefined();
      expect(screen.getByText('DRAFT-NEW')).toBeDefined();
    });

    expect(screen.getByLabelText(/Voucher Type/i)).toBeDefined();
    expect(screen.getByLabelText(/Numbering Series/i)).toBeDefined();
  });

  it('allows adding and updating line items with running Dr/Cr totals', async () => {
    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('+ Add Line Item (Insert)')).toBeDefined();
    });

    const addBtn = screen.getByText('+ Add Line Item (Insert)');
    fireEvent.click(addBtn);

    // After clicking add line, select ledger and amount
    const ledgerSelect = document.getElementById('line-ledger-0') as HTMLSelectElement;
    expect(ledgerSelect).toBeDefined();
    fireEvent.change(ledgerSelect, { target: { value: 'led_cash' } });

    const amountInput = screen.getByPlaceholderText('0.00');
    fireEvent.change(amountInput, { target: { value: '2500.00' } });

    // Running totals should reflect Debit ₹ 2500.00 and Difference ₹ 2500.00 (Unbalanced)
    expect(screen.getAllByText('₹ 2500.00').length).toBeGreaterThan(0);
    expect(screen.getByText(/DRAFT \(Unbalanced\)/i)).toBeDefined();
  });

  it('calculates balanced status when debit equals credit', async () => {
    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('+ Add Line Item (Insert)')).toBeDefined();
    });

    const addBtn = screen.getByText('+ Add Line Item (Insert)');
    // Add 2 lines
    fireEvent.click(addBtn);
    fireEvent.click(addBtn);

    const amounts = screen.getAllByPlaceholderText('0.00');
    expect(amounts.length).toBe(2);

    // Line 1: Dr 5000.00
    fireEvent.change(amounts[0], { target: { value: '5000.00' } });
    // Line 2: Cr 5000.00
    fireEvent.change(amounts[1], { target: { value: '5000.00' } });

    // Status should update to Balanced
    await waitFor(() => {
      expect(screen.getByText(/✓ DRAFT \(Balanced\)/i)).toBeDefined();
    });
  });

  it('deletes a line item and updates totals', async () => {
    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('+ Add Line Item (Insert)')).toBeDefined();
    });

    const addBtn = screen.getByText('+ Add Line Item (Insert)');
    fireEvent.click(addBtn);

    const amountInput = screen.getByPlaceholderText('0.00');
    fireEvent.change(amountInput, { target: { value: '1000.00' } });
    expect(screen.getAllByText('₹ 1000.00').length).toBeGreaterThan(0);

    const deleteBtn = screen.getByTitle('Delete Line (Ctrl+Delete)');
    fireEvent.click(deleteBtn);

    expect(screen.getAllByText('₹ 0.00').length).toBeGreaterThan(0);
  });

  it('handles hotkeys F5 (Payment) and F8 (Sales) to switch voucher types', async () => {
    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByLabelText(/Voucher Type/i)).toBeDefined();
    });

    // Press F5
    fireEvent.keyDown(window, { key: 'F5' });
    await waitFor(() => {
      const typeSelect = screen.getByLabelText(/Voucher Type/i) as HTMLSelectElement;
      expect(typeSelect.value).toBe('vt_payment');
    });

    // Press F8
    fireEvent.keyDown(window, { key: 'F8' });
    await waitFor(() => {
      const typeSelect = screen.getByLabelText(/Voucher Type/i) as HTMLSelectElement;
      expect(typeSelect.value).toBe('vt_sales');
    });
  });

  it('saves draft voucher when Save Draft button is clicked', async () => {
    const mockSaveResponse: VoucherWithLines = {
      voucher: {
        id: 'vch_saved_01',
        company_id: 'cmp_test_01',
        voucher_type_id: 'vt_sales',
        series_id: 'ser_sales_01',
        numbering_method: 'automatic',
        period_start: null,
        sequence_number: null,
        voucher_number: 'DRAFT-SAVED-01',
        date: '2026-04-01',
        effective_date: '2026-04-01',
        status: 'draft',
        reference_no: null,
        reference_date: null,
        narration: null,
        total_amount: null,
        reversal_of_voucher_id: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: [],
      debit_total: '0.00',
      credit_total: '0.00',
      difference: '0.00',
      is_balanced: false,
    };

    const saveSpy = vi.spyOn(api, 'saveVoucherDraft').mockResolvedValue(mockSaveResponse);

    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Save Draft (Ctrl+S)')).toBeDefined();
    });

    const saveBtn = screen.getByText('Save Draft (Ctrl+S)');
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(saveSpy).toHaveBeenCalled();
      expect(screen.getByText('DRAFT-SAVED-01')).toBeDefined();
    });
  });

  it('opens and closes saved drafts modal', async () => {
    vi.spyOn(api, 'listVoucherDrafts').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      page_size: 10,
      total_pages: 1,
    });

    render(<VoucherEntryView activeCompany={mockCompany} onBackToGateway={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTitle('Open saved drafts modal')).toBeDefined();
    });

    const openDraftsBtn = screen.getByTitle('Open saved drafts modal');
    fireEvent.click(openDraftsBtn);

    await waitFor(() => {
      expect(screen.getByText('Saved Voucher Drafts')).toBeDefined();
    });

    const closeBtn = screen.getByLabelText('Close modal');
    fireEvent.click(closeBtn);

    await waitFor(() => {
      expect(screen.queryByText('Saved Voucher Drafts')).toBeNull();
    });
  });
});
