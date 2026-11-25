import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BankReconciliationView } from '../BankReconciliationView';
import * as api from '../../../services/api';
import {
  BankLedgerItem,
  BankReconciliationStatement,
  BankStatementMatchPreview,
  Company,
} from '../../../types';

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

const mockLedgers: BankLedgerItem[] = [
  {
    id: 'led_hdfc_1',
    name: 'HDFC Bank Regular A/c',
    group_id: 'grp_bank',
    group_name: 'Bank Accounts',
    current_balance: '125000.00',
    current_balance_dr_cr: 'Dr',
  },
  {
    id: 'led_sbi_od',
    name: 'SBI Overdraft A/c',
    group_id: 'grp_od',
    group_name: 'Bank OD A/c',
    current_balance: '45000.00',
    current_balance_dr_cr: 'Cr',
  },
];

const mockStatement: BankReconciliationStatement = {
  bank_ledger_id: 'led_hdfc_1',
  bank_ledger_name: 'HDFC Bank Regular A/c',
  from_date: '2026-04-01',
  to_date: '2026-04-30',
  summary: {
    book_balance: '125000.00',
    book_balance_dr_cr: 'Dr',
    unreconciled_debit: '30000.00',
    unreconciled_credit: '12000.00',
    bank_balance: '107000.00',
    bank_balance_dr_cr: 'Dr',
  },
  rows: [
    {
      bank_allocation_id: 'alloc_1',
      voucher_id: 'vch_1',
      voucher_line_id: 'line_1',
      voucher_date: '2026-04-10',
      voucher_number: 'PMT-001',
      voucher_type: 'Payment',
      particulars: 'Precision Tools Ltd',
      transaction_type: 'Cheque',
      instrument_number: 'CHQ-100200',
      instrument_date: '2026-04-09',
      favouring_name: 'Precision Tools',
      bank_date: null,
      debit_amount: '0.00',
      credit_amount: '12000.00',
      is_reconciled: false,
    },
    {
      bank_allocation_id: 'alloc_2',
      voucher_id: 'vch_2',
      voucher_line_id: 'line_2',
      voucher_date: '2026-04-12',
      voucher_number: 'RCT-001',
      voucher_type: 'Receipt',
      particulars: 'Apex Retailers',
      transaction_type: 'NEFT',
      instrument_number: 'NEFT-554433',
      instrument_date: '2026-04-12',
      favouring_name: 'Apex Retailers',
      bank_date: null,
      debit_amount: '30000.00',
      credit_amount: '0.00',
      is_reconciled: false,
    },
  ],
};

const mockCsvPreview: BankStatementMatchPreview = {
  total_statement_rows: 2,
  exact_matches: 1,
  suggested_matches: 1,
  unmatched: 0,
  rows: [
    {
      csv_row_index: 0,
      statement_date: '2026-04-11',
      statement_ref: 'CHQ-100200',
      statement_description: 'Cheque Clearing Precision Tools',
      statement_amount: '12000.00',
      statement_dr_cr: 'Dr',
      matched_bank_allocation_id: 'alloc_1',
      matched_voucher_number: 'PMT-001',
      matched_particulars: 'Precision Tools Ltd',
      suggested_bank_date: '2026-04-11',
      match_type: 'exact',
      confidence: 'HIGH',
    },
    {
      csv_row_index: 1,
      statement_date: '2026-04-13',
      statement_ref: 'NEFT-554433',
      statement_description: 'NEFT INWARD APEX',
      statement_amount: '30000.00',
      statement_dr_cr: 'Cr',
      matched_bank_allocation_id: 'alloc_2',
      matched_voucher_number: 'RCT-001',
      matched_particulars: 'Apex Retailers',
      suggested_bank_date: '2026-04-13',
      match_type: 'suggested',
      confidence: 'MEDIUM',
    },
  ],
};

describe('BankReconciliationView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listBankLedgers').mockResolvedValue(mockLedgers);
    vi.spyOn(api, 'getBankReconciliation').mockResolvedValue(mockStatement);
    vi.spyOn(api, 'reconcileBankEntries').mockResolvedValue({
      success: true,
      updated_count: 1,
      updated_ids: ['alloc_1'],
    });
    vi.spyOn(api, 'previewBankStatementAutoMatch').mockResolvedValue(mockCsvPreview);
  });

  it('renders BRS view and loads bank ledgers and summary', async () => {
    render(
      <BankReconciliationView
        activeCompany={mockCompany}
        onBack={vi.fn()}
      />
    );

    // Header renders
    expect(screen.getByText(/Bank Reconciliation Statement \(BRS\)/i)).toBeTruthy();

    // Bank ledger selector populated and statement loaded
    await waitFor(() => {
      expect(api.listBankLedgers).toHaveBeenCalledWith(mockCompany.id);
      expect(screen.getByTestId('summary-books-balance')).toBeTruthy();
    });

    // Summary box equilibrium numbers
    expect(screen.getByTestId('summary-books-balance').textContent).toContain('125000.00 Dr');
    expect(screen.getByTestId('summary-unrec-dr').textContent).toContain('30000.00');
    expect(screen.getByTestId('summary-unrec-cr').textContent).toContain('12000.00');
    expect(screen.getByTestId('summary-bank-balance').textContent).toContain('107000.00 Dr');

    // Table rows rendered
    expect(screen.getByText('PMT-001')).toBeTruthy();
    expect(screen.getByText('RCT-001')).toBeTruthy();
  });

  it('allows editing bank date and saving via Save button', async () => {
    render(
      <BankReconciliationView
        activeCompany={mockCompany}
        onBack={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('input-bank-date-alloc_1')).toBeTruthy();
    });

    const dateInput = screen.getByTestId('input-bank-date-alloc_1');
    fireEvent.change(dateInput, { target: { value: '2026-04-11' } });

    // Save button should be enabled
    const saveBtn = screen.getByTestId('btn-save-brs') as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(false);

    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(api.reconcileBankEntries).toHaveBeenCalledWith(
        [
          {
            bank_allocation_id: 'alloc_1',
            bank_date: '2026-04-11',
          },
        ],
        mockCompany.id
      );
    });
  });

  it('opens CSV statement modal, previews matches, and applies suggested dates', async () => {
    render(
      <BankReconciliationView
        activeCompany={mockCompany}
        onBack={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('btn-import-csv')).toBeTruthy();
    });

    // Open Modal
    fireEvent.click(screen.getByTestId('btn-import-csv'));
    expect(screen.getByTestId('csv-modal')).toBeTruthy();

    // Type CSV
    const textarea = screen.getByTestId('csv-textarea');
    fireEvent.change(textarea, {
      target: {
        value: 'Date,Description,Reference,Withdrawal,Deposit\n2026-04-11,Cheque Clearing,CHQ-100200,12000.00,0.00',
      },
    });

    // Preview
    fireEvent.click(screen.getByTestId('btn-preview-csv'));

    await waitFor(() => {
      expect(api.previewBankStatementAutoMatch).toHaveBeenCalled();
      expect(screen.getByTestId('csv-preview-table')).toBeTruthy();
    });

    // Apply matches
    fireEvent.click(screen.getByTestId('btn-apply-csv-matches'));

    // Modal closes and input receives date
    expect(screen.queryByTestId('csv-modal')).toBeNull();
    const dateInput1 = screen.getByTestId('input-bank-date-alloc_1') as HTMLInputElement;
    expect(dateInput1.value).toBe('2026-04-11');
  });

  it('triggers onBack when Esc is pressed', async () => {
    const onBack = vi.fn();
    render(
      <BankReconciliationView
        activeCompany={mockCompany}
        onBack={onBack}
      />
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
