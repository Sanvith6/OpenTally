import { describe, expect, it, vi, beforeEach } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { DayBookView } from '../DayBookView';
import { LedgerReportView } from '../LedgerReportView';
import { TrialBalanceView } from '../TrialBalanceView';
import { ProfitAndLossView } from '../ProfitAndLossView';
import { BalanceSheetView } from '../BalanceSheetView';
import { CashFlowView } from '../CashFlowView';
import { OutstandingAgingView } from '../OutstandingAgingView';
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

describe('Milestone 2 Outstanding & Aging View', () => {
  it('renders receivables bills and switches tabs to aging analysis', async () => {
    vi.spyOn(api, 'getBillsReceivable').mockResolvedValue([
      {
        party_ledger_id: 'ledger-debtor-1',
        party_name: 'Acme Corp',
        bill_name: 'INV-001',
        bill_date: '2026-04-01',
        due_date: '2026-04-15',
        dr_cr: 'Dr',
        original_amount: '5000.00',
        settled_amount: '2000.00',
        pending_amount: '3000.00',
        overdue_days: 16,
      },
    ]);

    vi.spyOn(api, 'getAgingAnalysis').mockResolvedValue({
      as_of_date: '2026-05-01',
      report_type: 'receivable',
      parties: [
        {
          party_ledger_id: 'ledger-debtor-1',
          party_name: 'Acme Corp',
          buckets: {
            not_due: '0.00',
            days_0_30: '3000.00',
            days_31_60: '0.00',
            days_61_90: '0.00',
            days_90_plus: '0.00',
            total: '3000.00',
          },
          bills: [
            {
              party_ledger_id: 'ledger-debtor-1',
              party_name: 'Acme Corp',
              bill_name: 'INV-001',
              bill_date: '2026-04-01',
              due_date: '2026-04-15',
              dr_cr: 'Dr',
              original_amount: '5000.00',
              settled_amount: '2000.00',
              pending_amount: '3000.00',
              overdue_days: 16,
            },
          ],
        },
      ],
      grand_total: {
        not_due: '0.00',
        days_0_30: '3000.00',
        days_31_60: '0.00',
        days_61_90: '0.00',
        days_90_plus: '0.00',
        total: '3000.00',
      },
    });

    render(<OutstandingAgingView activeCompany={company} onBack={vi.fn()} />);

    expect(await screen.findByText('Acme Corp')).toBeTruthy();
    expect(screen.getByText('INV-001')).toBeTruthy();
    expect(screen.getAllByText('3000.00 Dr').length).toBeGreaterThan(0);
    expect(screen.getByText('16 days')).toBeTruthy();

    // Switch tab to Aging Analysis
    fireEvent.click(screen.getByText('Aging Analysis'));
    await waitFor(() => {
      expect(api.getAgingAnalysis).toHaveBeenCalled();
    });
    expect(screen.getAllByText('Not Due').length).toBeGreaterThan(0);
    expect(screen.getAllByText('90+ Days').length).toBeGreaterThan(0);
  });
});

describe('M24 CA PDF exports and accounting T-format', () => {
  beforeEach(() => {
    const pdf = Array.from('%PDF-1.4\n1 0 obj\n<<>>\nendobj\n%%EOF', (char) => char.charCodeAt(0));
    vi.spyOn(api, 'exportTrialBalancePdf').mockResolvedValue(pdf);
    vi.spyOn(api, 'exportProfitAndLossPdf').mockResolvedValue(pdf);
    vi.spyOn(api, 'exportBalanceSheetPdf').mockResolvedValue(pdf);
    vi.spyOn(api, 'exportLedgerStatementPdf').mockResolvedValue(pdf);
    vi.spyOn(api, 'exportOutstandingAgingPdf').mockResolvedValue(pdf);
    vi.spyOn(api, 'getBillsReceivable').mockResolvedValue([]);
    Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: vi.fn(() => 'blob:statement') });
    Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: vi.fn() });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);
  });

  it.each([
    { View: TrialBalanceView, operation: 'exportTrialBalancePdf' as const },
    { View: ProfitAndLossView, operation: 'exportProfitAndLossPdf' as const },
    { View: BalanceSheetView, operation: 'exportBalanceSheetPdf' as const },
    { View: LedgerReportView, operation: 'exportLedgerStatementPdf' as const },
    { View: OutstandingAgingView, operation: 'exportOutstandingAgingPdf' as const },
  ])('wires the PDF button and Ctrl+P in $operation', async ({ View, operation }) => {
    render(<View activeCompany={company} onBack={vi.fn()} />);
    const button = screen.getByRole('button', { name: 'Export PDF (Ctrl+P)' });
    await waitFor(() => expect(button.hasAttribute('disabled')).toBe(false));
    fireEvent.click(button);
    await waitFor(() => expect(api[operation]).toHaveBeenCalledTimes(1));
    const today = new Date().toISOString().slice(0, 10);
    const argumentsForOperation = operation === 'exportLedgerStatementPdf' ? ['l1', '2026-04-01', today, company.id]
      : operation === 'exportOutstandingAgingPdf' ? ['receivable', today, company.id]
      : operation === 'exportProfitAndLossPdf' ? ['2026-04-01', '2026-04-01', company.id]
      : ['2026-04-01', company.id];
    expect(api[operation]).toHaveBeenCalledWith(...argumentsForOperation);
    await waitFor(() => expect(button.hasAttribute('disabled')).toBe(false));
    const event = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true, cancelable: true });
    await act(async () => { window.dispatchEvent(event); });
    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(api[operation]).toHaveBeenCalledTimes(2));
    expect(URL.createObjectURL).toHaveBeenCalledTimes(2);
    expect(URL.revokeObjectURL).toHaveBeenCalledTimes(2);
  });

  it('uses selected dates and the active company for statement exports', async () => {
    render(<ProfitAndLossView activeCompany={company} onBack={vi.fn()} />);
    const button = screen.getByRole('button', { name: 'Export PDF (Ctrl+P)' });
    fireEvent.change(screen.getByLabelText('To'), { target: { value: '2026-05-31' } });
    await waitFor(() => expect(button.hasAttribute('disabled')).toBe(false));
    fireEvent.click(button);
    await waitFor(() => expect(api.exportProfitAndLossPdf).toHaveBeenCalledWith('2026-04-01', '2026-05-31', company.id));
  });

  it('renders expenses to the left of income with a balancing net result', async () => {
    render(<ProfitAndLossView activeCompany={company} onBack={vi.fn()} />);
    const statement = await screen.findByRole('table', { name: 'Profit & Loss T-format' });
    const headings = Array.from(statement.querySelectorAll('thead th')).map((cell) => cell.textContent);
    expect(headings[0]).toBe('Expenses');
    expect(headings[2]).toBe('Income');
    const totals = statement.querySelectorAll('tfoot .money');
    expect(totals[0].textContent).toBe(totals[1].textContent);
  });

  it('renders liabilities and capital left, assets right without concealing differences', async () => {
    render(<BalanceSheetView activeCompany={company} onBack={vi.fn()} />);
    const statement = await screen.findByRole('table', { name: 'Balance Sheet T-format' });
    const headings = Array.from(statement.querySelectorAll('thead th')).map((cell) => cell.textContent);
    expect(headings[0]).toBe('Liabilities & Capital');
    expect(headings[2]).toBe('Assets');
    expect(statement.querySelectorAll('tfoot .money')[0].textContent).toBe('150.00');
  });

  it('rejects invalid PDF bytes and displays a visible export error', async () => {
    vi.mocked(api.exportTrialBalancePdf).mockResolvedValue([65, 66, 67]);
    render(<TrialBalanceView activeCompany={company} onBack={vi.fn()} />);
    const button = screen.getByRole('button', { name: 'Export PDF (Ctrl+P)' });
    await waitFor(() => expect(button.hasAttribute('disabled')).toBe(false));
    fireEvent.click(button);
    expect(await screen.findByRole('alert')).toHaveProperty('textContent', expect.stringContaining('PDF'));
    expect(URL.createObjectURL).not.toHaveBeenCalled();
  });

  it('exports the selected payable aging scope', async () => {
    vi.spyOn(api, 'getAgingAnalysis').mockResolvedValue({ as_of_date: '2026-11-27', report_type: 'payable', parties: [],
      grand_total: { not_due: '0.00', days_0_30: '0.00', days_31_60: '0.00', days_61_90: '0.00', days_90_plus: '0.00', total: '0.00' } });
    render(<OutstandingAgingView activeCompany={company} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Aging Analysis' }));
    fireEvent.change(screen.getByLabelText('Aging Category'), { target: { value: 'payable' } });
    const button = screen.getByRole('button', { name: 'Export PDF (Ctrl+P)' });
    await waitFor(() => expect(button.hasAttribute('disabled')).toBe(false));
    fireEvent.click(button);
    await waitFor(() => expect(api.exportOutstandingAgingPdf).toHaveBeenCalledWith('payable', new Date().toISOString().slice(0, 10), company.id));
  });

  it('keeps a mismatched balance sheet visibly unbalanced', async () => {
    const original = await api.getBalanceSheet({ as_of_date: '2026-04-01', include_zero_balances: false });
    vi.mocked(api.getBalanceSheet).mockResolvedValue({ ...original, total_liabilities_and_equity: '100.00', balance_difference: '50.00', is_balanced: false });
    render(<BalanceSheetView activeCompany={company} onBack={vi.fn()} />);
    const statement = await screen.findByRole('table', { name: 'Balance Sheet T-format' });
    const totals = statement.querySelectorAll('tfoot .money');
    expect(totals[0].textContent).toBe('100.00');
    expect(totals[1].textContent).toBe('150.00');
    expect(screen.getByText('Not balanced')).toBeTruthy();
  });

  it('balances a loss on the income side using exact decimal totals', async () => {
    const original = await api.getProfitAndLoss({ period: { date_from: '2026-04-01', date_to: '2026-04-30' }, include_zero_balances: false });
    vi.mocked(api.getProfitAndLoss).mockResolvedValue({ ...original, direct_expense_subtotal: '125.10', net_profit_or_loss: '25.10', net_result_type: 'Loss' });
    render(<ProfitAndLossView activeCompany={company} onBack={vi.fn()} />);
    const statement = await screen.findByRole('table', { name: 'Profit & Loss T-format' });
    expect(statement.querySelectorAll('tfoot .money')[0].textContent).toBe('125.10');
    expect(statement.querySelectorAll('tfoot .money')[1].textContent).toBe('125.10');
  });

  it('sums outstanding amounts exactly beyond the IEEE754 safe integer range', async () => {
    vi.mocked(api.getBillsReceivable).mockResolvedValue([
      { party_ledger_id: 'p1', party_name: 'Precision Supplier', bill_name: 'LARGE', bill_date: '2026-04-01', due_date: null, dr_cr: 'Dr', original_amount: '90071992547409.91', settled_amount: '0.00', pending_amount: '90071992547409.91', overdue_days: 0 },
      { party_ledger_id: 'p1', party_name: 'Precision Supplier', bill_name: 'SMALL', bill_date: '2026-04-01', due_date: null, dr_cr: 'Dr', original_amount: '0.10', settled_amount: '0.00', pending_amount: '0.10', overdue_days: 0 },
    ]);
    render(<OutstandingAgingView activeCompany={company} onBack={vi.fn()} />);
    await screen.findByText('LARGE');
    expect(screen.getAllByText('90071992547410.01').length).toBeGreaterThan(0);
  });

  it('keeps an empty profit and loss report in T-format', async () => {
    const original = await api.getProfitAndLoss({ period: { date_from: '2026-04-01', date_to: '2026-04-30' }, include_zero_balances: false });
    vi.mocked(api.getProfitAndLoss).mockResolvedValue({ ...original, ledger_count: 0, direct_income_rows: [], direct_expense_rows: [], indirect_income_rows: [], indirect_expense_rows: [] });
    render(<ProfitAndLossView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByRole('table', { name: 'Profit & Loss T-format' })).toBeTruthy();
  });

  it('keeps an empty balance sheet report in T-format', async () => {
    const original = await api.getBalanceSheet({ as_of_date: '2026-04-01', include_zero_balances: false });
    vi.mocked(api.getBalanceSheet).mockResolvedValue({ ...original, ledger_count: 0, asset_rows: [], liability_rows: [], equity_rows: [] });
    render(<BalanceSheetView activeCompany={company} onBack={vi.fn()} />);
    expect(await screen.findByRole('table', { name: 'Balance Sheet T-format' })).toBeTruthy();
  });
});
