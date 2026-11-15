import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { DayBookView } from '../DayBookView';
import { LedgerReportView } from '../LedgerReportView';
import { TrialBalanceView } from '../TrialBalanceView';
import { ProfitAndLossView } from '../ProfitAndLossView';
import { BalanceSheetView } from '../BalanceSheetView';
import { CashFlowView } from '../CashFlowView';
import * as api from '../../../services/api';
import { Company } from '../../../types';

const company: Company = {
  id: 'company-1', name: 'Test Company', mailing_name: 'Test Company', country: 'India', state: 'Maharashtra',
  financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01', currency_symbol: '₹',
  currency_formal_name: 'INR', currency_decimal_places: 2, status: 'active', created_at: '', updated_at: '',
};

beforeEach(() => {
  vi.spyOn(api, 'listVoucherTypes').mockResolvedValue([{ id: 'vt_sales', company_id: company.id, name: 'Sales', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: '', updated_at: '' }]);
  vi.spyOn(api, 'getDayBook').mockResolvedValue({ items: [{ voucher_id: 'v1', date: '2026-04-01', voucher_number: 'SAL-001', voucher_type: 'Sales', narration: 'Sale', debit_total: '100.00', credit_total: '100.00', voucher_total: '100.00', lines: [{ ledger_id: 'l1', ledger_name: 'Cash', debit: '100.00', credit: '0.00', narration: null }] }], total: 1, page: 1, page_size: 25, total_pages: 1 });
  vi.spyOn(api, 'listLedgers').mockResolvedValue([{ id: 'l1', company_id: company.id, group_id: 'g1', name: 'Cash', ledger_classification: 'cash', is_reserved: true, description: null, created_at: '', updated_at: '' }]);
  vi.spyOn(api, 'getLedgerReport').mockResolvedValue({ ledger_id: 'l1', ledger_name: 'Cash', ledger_nature: 'Assets', opening_balance: '50.00', opening_balance_type: 'Dr', closing_balance: '150.00', closing_balance_type: 'Dr', transactions: { items: [{ voucher_id: 'v1', date: '2026-04-01', voucher_number: 'SAL-001', voucher_type: 'Sales', particulars: 'Sale', debit: '100.00', credit: '0.00', running_balance: '150.00', running_balance_type: 'Dr' }], total: 1, page: 1, page_size: 25, total_pages: 1 } });
  vi.spyOn(api, 'listAccountGroups').mockResolvedValue([{ id: 'g1', company_id: company.id, name: 'Current Assets', parent_id: null, nature: 'Assets', is_primary: true, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' }]);
  vi.spyOn(api, 'getTrialBalance').mockResolvedValue({ as_of_date: '2026-04-01', rows: [{ ledger_id: 'l1', ledger_name: 'Cash', account_group_id: 'g1', account_group_name: 'Current Assets', account_group_nature: 'Assets', opening_debit: '50.00', opening_credit: '0.00', period_debit: '100.00', period_credit: '0.00', closing_debit: '150.00', closing_credit: '0.00' }], total_debit: '150.00', total_credit: '0.00', difference: '150.00', ledger_count: 1 });
  vi.spyOn(api, 'getProfitAndLoss').mockResolvedValue({ company_id: company.id, date_from: '2026-04-01', date_to: '2026-04-30', direct_income_rows: [{ ledger_id: 'sales', ledger_name: 'Sales', account_group_id: 'g1', account_group_name: 'Sales Accounts', account_group_nature: 'Income', is_direct: true, debit: '0.00', credit: '100.00', contribution: '100.00' }], direct_income_subtotal: '100.00', direct_expense_rows: [], direct_expense_subtotal: '0.00', gross_profit_or_loss: '100.00', gross_result_type: 'Profit', indirect_income_rows: [], indirect_income_subtotal: '0.00', indirect_expense_rows: [], indirect_expense_subtotal: '0.00', net_profit_or_loss: '100.00', net_result_type: 'Profit', ledger_count: 1, warnings: [] });
  vi.spyOn(api, 'getBalanceSheet').mockResolvedValue({ company_id: company.id, as_of_date: '2026-04-30', asset_rows: [{ ledger_id: 'cash', ledger_name: 'Cash', account_group_id: 'g1', account_group_name: 'Current Assets', classification: 'current_assets', debit: '150.00', credit: '0.00', net_balance: '150.00', balance_type: 'Dr', presentation_amount: '150.00' }], total_assets: '150.00', liability_rows: [], total_liabilities: '0.00', equity_rows: [], current_period_profit_or_loss: '150.00', current_period_result_type: 'Profit', total_equity: '150.00', total_liabilities_and_equity: '150.00', balance_difference: '0.00', is_balanced: true, ledger_count: 1, warnings: [] });
  vi.spyOn(api, 'getCashFlow').mockResolvedValue({ company_id: company.id, date_from: '2026-04-01', date_to: '2026-04-30', opening_cash_balance: '100.00', operating_rows: [{ activity: 'operating', ledger_id: 'cash', ledger_name: 'Cash', voucher_id: 'v1', voucher_number: 'RCPT-1', date: '2026-04-05', description: 'Receipt', amount: '50.00' }], operating_subtotal: '50.00', investing_rows: [], investing_subtotal: '0.00', financing_rows: [], financing_subtotal: '0.00', net_change_in_cash: '50.00', closing_cash_balance: '150.00', actual_closing_cash_balance: '150.00', reconciliation_difference: '0.00', is_reconciled: true, ledger_count: 1, warnings: [] });
});

describe('Stage 8C report screens', () => {
  it('loads Trial Balance rows and exact decimal totals', async () => {
    render(<TrialBalanceView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('Cash')).toBeTruthy();
    expect(screen.getAllByText('150.00').length).toBeGreaterThan(0);
    expect(api.getTrialBalance).toHaveBeenCalledWith(expect.objectContaining({ as_of_date: '2026-04-01', include_zero_balances: false }));
  });
});

describe('Stage 8B report screens', () => {
  it('loads Day Book rows and expands voucher details', async () => {
    render(<DayBookView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('SAL-001')).toBeTruthy();
    fireEvent.click(screen.getByText('▸'));
    expect(await screen.findByText('Cash')).toBeTruthy();
    expect(screen.getAllByText('100.00').length).toBeGreaterThan(0);
  });

  it('loads Ledger Report and changes the ledger selector', async () => {
    render(<LedgerReportView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('Opening balance')).toBeTruthy();
    expect(screen.getAllByText('150.00 Dr').length).toBeGreaterThan(0);
    fireEvent.change(screen.getByLabelText('Ledger'), { target: { value: 'l1' } });
    await waitFor(() => expect(api.getLedgerReport).toHaveBeenCalled());
  });
});

describe('Stage 8D report screens', () => {
  it('renders P&L sections, totals, and period filters', async () => {
    render(<ProfitAndLossView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('Sales')).toBeTruthy();
    expect(screen.getAllByText('100.00').length).toBeGreaterThan(0);
    expect(api.getProfitAndLoss).toHaveBeenCalledWith(expect.objectContaining({ include_zero_balances: false }));
  });
});

describe('Stage 8E report screens', () => {
  it('renders Balance Sheet totals and balanced status', async () => {
    render(<BalanceSheetView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('Cash')).toBeTruthy();
    expect(screen.getByText('Balanced')).toBeTruthy();
    expect(api.getBalanceSheet).toHaveBeenCalledWith(expect.objectContaining({ as_of_date: '2026-04-01', include_zero_balances: false }));
  });
});

describe('Stage 9 report screens', () => {
  it('renders Cash Flow reconciliation and operating activity', async () => {
    render(<CashFlowView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByText('Receipt')).toBeTruthy();
    expect(screen.getByText('Reconciled')).toBeTruthy();
    expect(api.getCashFlow).toHaveBeenCalledWith(expect.objectContaining({ activity_filter: null }));
  });
});
