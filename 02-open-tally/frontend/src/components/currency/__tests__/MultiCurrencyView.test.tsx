import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MultiCurrencyView } from '../MultiCurrencyView';
import { Company } from '../../../types';
import * as api from '../../../services/api';

vi.mock('../../../services/api');

const mockCompany: Company = {
  id: 'cmp_test_mc',
  name: 'Global Traders Corp',
  mailing_name: 'Global Traders',
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

const mockCurrencies = [
  { id: 'cur_inr', company_id: 'cmp_test_mc', code: 'INR', symbol: '₹', formal_name: 'Indian Rupee', decimal_places: 2, is_base: true, created_at: '2026-04-01' },
  { id: 'cur_usd', company_id: 'cmp_test_mc', code: 'USD', symbol: '$', formal_name: 'US Dollar', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
  { id: 'cur_eur', company_id: 'cmp_test_mc', code: 'EUR', symbol: '€', formal_name: 'Euro', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
];

const mockRates = [
  { id: 'exr_1', company_id: 'cmp_test_mc', currency_code: 'USD', effective_date: '2026-05-01', standard_rate: '83.50', selling_rate: '84.00', buying_rate: '83.00', created_at: '2026-05-01' },
];

const mockBills = [
  {
    bill_name: 'INV-USD-101',
    party_ledger_id: 'led_debtor_us1',
    party_name: 'US Customer Inc',
    voucher_id: 'vch_001',
    voucher_number: 'SAL/0001',
    bill_date: '2026-04-15',
    currency_code: 'USD',
    invoice_rate: '83.50',
    original_foreign: '1000.00',
    settled_foreign: '0.00',
    outstanding_foreign: '1000.00',
    original_inr: '83500.00',
    settled_inr: '0.00',
    outstanding_inr: '83500.00',
    dr_cr: 'Dr',
  },
];

const mockRevaluation = {
  company_id: 'cmp_test_mc',
  as_of_date: '2026-05-31',
  bills: [
    {
      bill_name: 'INV-USD-101',
      party_ledger_id: 'led_debtor_us1',
      party_name: 'US Customer Inc',
      currency_code: 'USD',
      dr_cr: 'Dr',
      open_foreign_amount: '1000.00',
      invoice_rate: '83.50',
      book_value_inr: '83500.00',
      revaluation_rate: '84.20',
      revalued_value_inr: '84200.00',
      unrealized_gain_or_loss: '700.00',
      is_gain: true,
    },
  ],
  total_book_value_inr: '83500.00',
  total_revalued_value_inr: '84200.00',
  total_unrealized_gain: '700.00',
  total_unrealized_loss: '0.00',
  net_unrealized_gain_loss: '700.00',
};

const mockCheques = [
  { id: 'chq_hdfc', company_id: 'cmp_test_mc', bank_name: 'HDFC Bank', width_mm: 202, height_mm: 92, date_x_mm: 148, date_y_mm: 9, payee_x_mm: 22, payee_y_mm: 25, words_x_mm: 26, words_y_mm: 36, amount_x_mm: 152, amount_y_mm: 42, is_account_payee_default: true },
];

describe('MultiCurrencyView Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listCurrencies).mockResolvedValue(mockCurrencies);
    vi.mocked(api.listExchangeRates).mockResolvedValue(mockRates);
    vi.mocked(api.listOpenForeignBills).mockResolvedValue(mockBills);
    vi.mocked(api.getUnrealizedForexReport).mockResolvedValue(mockRevaluation);
    vi.mocked(api.listChequeTemplates).mockResolvedValue(mockCheques);
    vi.mocked(api.seedCompanyCurrencyDefaults).mockResolvedValue({
      base_currency_seeded: true,
      currencies_seeded_count: 5,
      forex_ledger_id: 'led_forex_mc1',
      cheque_templates_seeded_count: 4,
    });
  });

  it('renders header, tabs, and loads currency masters', async () => {
    const onBack = vi.fn();
    render(<MultiCurrencyView activeCompany={mockCompany} onBack={onBack} />);

    expect(screen.getByText('Multi-Currency & Forex Management')).toBeDefined();
    expect(screen.getByText('Global Traders Corp')).toBeDefined();

    await waitFor(() => {
      expect(screen.getByText('USD')).toBeDefined();
      expect(screen.getByText('US Dollar')).toBeDefined();
      expect(screen.getByText('BASE')).toBeDefined();
    });
  });

  it('switches between tabs and displays respective data', async () => {
    const onBack = vi.fn();
    render(<MultiCurrencyView activeCompany={mockCompany} onBack={onBack} />);

    await waitFor(() => {
      expect(screen.getByText('USD')).toBeDefined();
    });

    // Tab 2: Exchange Rates
    const ratesTab = screen.getByRole('button', { name: /Exchange Rates/i });
    fireEvent.click(ratesTab);
    expect(await screen.findByText('Daily Exchange Rates')).toBeDefined();
    expect(screen.getByText('₹83.50')).toBeDefined();

    // Tab 3: Foreign Bills
    const billsTab = screen.getByRole('button', { name: /Foreign Bills/i });
    fireEvent.click(billsTab);
    expect(await screen.findByText('Open Foreign Currency Bills')).toBeDefined();
    expect(screen.getByText('INV-USD-101')).toBeDefined();
    expect(screen.getByText('US Customer Inc')).toBeDefined();

    // Tab 4: Revaluation Report
    const revalTab = screen.getByRole('button', { name: /Unrealized Forex Revaluation/i });
    fireEvent.click(revalTab);
    expect(await screen.findByText('Unrealized Forex Revaluation Report')).toBeDefined();
    expect(screen.getAllByText('+₹700.00')[0]).toBeDefined();

    // Tab 5: Cheques
    const chequesTab = screen.getByRole('button', { name: /CTS-2010 Cheque Templates/i });
    fireEvent.click(chequesTab);
    expect(await screen.findByText('CTS-2010 Bank Cheque Dimensions & Coordinates')).toBeDefined();
    expect(screen.getByText('HDFC Bank')).toBeDefined();
  });

  it('triggers seed defaults when button is clicked', async () => {
    const onBack = vi.fn();
    render(<MultiCurrencyView activeCompany={mockCompany} onBack={onBack} />);

    const seedBtn = screen.getByRole('button', { name: /Seed Defaults/i });
    fireEvent.click(seedBtn);

    await waitFor(() => {
      expect(api.seedCompanyCurrencyDefaults).toHaveBeenCalledWith('cmp_test_mc');
      expect(screen.getByText(/Seeded default currencies/i)).toBeDefined();
    });
  });

  it('unwinds back to Gateway on Escape key press', async () => {
    const onBack = vi.fn();
    render(<MultiCurrencyView activeCompany={mockCompany} onBack={onBack} />);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
