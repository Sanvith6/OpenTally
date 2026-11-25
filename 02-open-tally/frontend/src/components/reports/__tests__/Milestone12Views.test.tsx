import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { RatioAnalysisView } from '../RatioAnalysisView';
import { BudgetsView } from '../../accounting/BudgetsView';
import { YearClosingAndAuditView } from '../../accounting/YearClosingAndAuditView';
import * as api from '../../../services/api';
import {
  AuditCertificationReport,
  Budget,
  BudgetVarianceReport,
  Company,
  FinancialYearClosingRecord,
  RatioAnalysisReport,
  YearEndClosingPreview,
} from '../../../types';

const mockCompany: Company = {
  id: 'cmp_test',
  name: 'Bharath Precision Components Ltd',
  mailing_name: 'Bharath Precision Components Ltd',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2024-04-01',
  books_beginning_from: '2024-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2024-04-01T00:00:00Z',
  updated_at: '2024-04-01T00:00:00Z',
};

const mockRatioReport: RatioAnalysisReport = {
  company_id: 'cmp_test',
  from_date: '2024-04-01',
  to_date: '2025-03-31',
  working_capital: '450000.00',
  cash_in_hand: '25000.00',
  bank_accounts: '380000.00',
  bank_od_ac: '0.00',
  sundry_debtors: '185000.00',
  sundry_creditors: '140000.00',
  sales_accounts: '1250000.00',
  purchase_accounts: '720000.00',
  closing_stock: '150000.00',
  net_profit: '235000.00',
  current_ratio: '3.21',
  quick_ratio: '2.14',
  debt_equity_ratio: '0.28',
  gross_profit_pct: '42.40%',
  net_profit_pct: '18.80%',
  operating_cost_pct: '38.80%',
  receivables_turnover_days: '54 days',
  inventory_turnover: '4.80',
  return_on_working_capital_pct: '52.22%',
};

const mockBudgets: Budget[] = [
  {
    id: 'bdg_1',
    company_id: 'cmp_test',
    name: 'FY 2024-25 R&D Budget',
    from_date: '2024-04-01',
    to_date: '2025-03-31',
    created_at: '2024-04-01T00:00:00Z',
    updated_at: '2024-04-01T00:00:00Z',
  },
];

const mockVariance: BudgetVarianceReport = {
  budget_id: 'bdg_1',
  budget_name: 'FY 2024-25 R&D Budget',
  company_id: 'cmp_test',
  as_of_date: '2024-08-31',
  from_date: '2024-04-01',
  to_date: '2025-03-31',
  rows: [
    {
      line_id: 'l1',
      target_type: 'Group',
      target_id: 'grp_indirect_expenses',
      target_name: 'Indirect Expenses',
      budget_type: 'OnNetTransactions',
      dr_cr: 'Dr',
      budget_amount: '50000.00',
      actual_amount: '35000.00',
      variance_amount: '15000.00',
      utilization_pct: '70.00%',
      is_over_budget: false,
    },
  ],
  total_budget: '50000.00',
  total_actual: '35000.00',
  total_variance: '15000.00',
};

const mockClosingPreview: YearEndClosingPreview = {
  company_id: 'cmp_test',
  financial_year_label: 'FY 2024-25',
  period_start: '2024-04-01',
  period_end: '2025-03-31',
  next_fy_start: '2025-04-01',
  trial_balance_difference: '0.00',
  is_equilibrium: true,
  net_profit_or_loss: '235000.00',
  is_profit: true,
  nominal_transfers: [
    {
      ledger_id: 'led_sales',
      ledger_name: 'Sales Accounts',
      account_group_nature: 'Income',
      closing_debit: '0.00',
      closing_credit: '1250000.00',
      transfer_entry_type: 'Dr',
      transfer_amount: '1250000.00',
    },
  ],
  balance_sheet_balances: [
    {
      ledger_id: 'led_bank',
      ledger_name: 'HDFC Bank',
      account_group_nature: 'Assets',
      balance_type: 'Dr',
      amount: '380000.00',
    },
  ],
};

const mockClosedYears: FinancialYearClosingRecord[] = [
  {
    id: 'fyc_prev',
    company_id: 'cmp_test',
    financial_year_label: 'FY 2023-24',
    period_start: '2023-04-01',
    period_end: '2024-03-31',
    next_fy_start: '2024-04-01',
    closing_voucher_id: 'vch_closing_2324',
    net_profit_or_loss: '180000.00',
    is_locked: true,
    closed_at: '2024-03-31T18:00:00Z',
    closed_by: 'system',
  },
];

const mockAuditReport: AuditCertificationReport = {
  company_id: 'cmp_test',
  generated_at: '2026-11-24T18:00:00Z',
  total_entries: 58,
  chained_entries: 58,
  legacy_unchained_entries: 0,
  tampered_entries: 0,
  broken_chain_links: 0,
  is_valid: true,
  certification_status: 'VERIFIED_CLEAN',
  first_entry_timestamp: '2024-04-01T10:00:00Z',
  latest_entry_timestamp: '2025-03-31T18:00:00Z',
  latest_entry_hash: '3a7b9c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b',
  issues: [],
};

describe('Milestone 12 Frontend Views', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'getRatioAnalysis').mockResolvedValue(mockRatioReport);
    vi.spyOn(api, 'listBudgets').mockResolvedValue(mockBudgets);
    vi.spyOn(api, 'getBudgetVarianceReport').mockResolvedValue(mockVariance);
    vi.spyOn(api, 'listClosedYears').mockResolvedValue(mockClosedYears);
    vi.spyOn(api, 'previewYearEndClosing').mockResolvedValue(mockClosingPreview);
    vi.spyOn(api, 'verifyAuditTrailIntegrity').mockResolvedValue(mockAuditReport);
  });

  describe('RatioAnalysisView', () => {
    it('renders two-pane Ratio Analysis layout with figures and financial ratios', async () => {
      render(<RatioAnalysisView activeCompany={mockCompany} onBack={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText('Principal Groups & Figures')).toBeTruthy();
      });

      expect(screen.getByText('Working Capital')).toBeTruthy();
      expect(screen.getByText('Sundry Debtors (Receivables)')).toBeTruthy();

      // Right Pane Ratios
      expect(screen.getByText('Principal Financial Ratios')).toBeTruthy();
      expect(screen.getByText('Current Ratio')).toBeTruthy();
      expect(screen.getByText('3.21')).toBeTruthy();
      expect(screen.getByText('Quick Ratio (Acid Test)')).toBeTruthy();
      expect(screen.getByText('2.14')).toBeTruthy();
      expect(screen.getByText('42.40%')).toBeTruthy();
      expect(screen.getByText('18.80%')).toBeTruthy();
    });

    it('handles Esc key to go back', async () => {
      const onBack = vi.fn();
      render(<RatioAnalysisView activeCompany={mockCompany} onBack={onBack} />);

      fireEvent.keyDown(window, { key: 'Escape' });
      expect(onBack).toHaveBeenCalled();
    });
  });

  describe('BudgetsView', () => {
    it('renders budgets list and variance report', async () => {
      render(<BudgetsView activeCompany={mockCompany} onBack={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText('FY 2024-25 R&D Budget')).toBeTruthy();
        expect(screen.getByText(/Variance Analysis: FY 2024-25 R&D Budget/)).toBeTruthy();
      });

      expect(screen.getByText('Indirect Expenses (grp_indirect_expenses)')).toBeTruthy();
      expect(screen.getAllByText('₹50000.00').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('₹35000.00').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('₹15000.00').length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText('70.00%')).toBeTruthy();
      expect(screen.getByText('WITHIN')).toBeTruthy();
    });

    it('toggles budget creation form', async () => {
      render(<BudgetsView activeCompany={mockCompany} onBack={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText('+ Create Budget (Alt+C)')).toBeTruthy();
      });

      const createBtn = screen.getByText('+ Create Budget (Alt+C)');
      fireEvent.click(createBtn);

      expect(screen.getByText('Define New Budget')).toBeTruthy();
      expect(screen.getByPlaceholderText('e.g. FY 2024-25 Marketing & Operations')).toBeTruthy();
    });
  });

  describe('YearClosingAndAuditView', () => {
    it('renders year-end closing tab and triggers pre-closing audit preview', async () => {
      render(<YearClosingAndAuditView activeCompany={mockCompany} onBack={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText('Locked & Closed Financial Years')).toBeTruthy();
        expect(screen.getByText('FY 2023-24')).toBeTruthy();
        expect(screen.getByText('🔒 LOCKED')).toBeTruthy();
      });

      const previewBtn = screen.getByText('Pre-Closing Audit Preview');
      fireEvent.click(previewBtn);

      await waitFor(() => {
        expect(screen.getByText(/Pre-Closing Pre-Flight Verification/)).toBeTruthy();
        expect(screen.getByText(/PASSED \(Difference = 0\.00\)/)).toBeTruthy();
        expect(screen.getByText(/Net Profit: ₹235000\.00/)).toBeTruthy();
      });
    });

    it('switches to MCA Audit Trail tab and displays cryptographic verification results', async () => {
      render(<YearClosingAndAuditView activeCompany={mockCompany} onBack={vi.fn()} />);

      await waitFor(() => {
        expect(screen.getByText(/MCA 2023 Audit Trail Certification/)).toBeTruthy();
      });

      const auditTabBtn = screen.getByRole('button', { name: /MCA 2023 Audit Trail Certification/ });
      fireEvent.click(auditTabBtn);

      await waitFor(() => {
        expect(screen.getByText('MCA 2023 Statutory Audit Trail Verification')).toBeTruthy();
        expect(screen.getByText('✓ AUDIT TRAIL INTEGRITY VERIFIED (CLEAN)')).toBeTruthy();
        expect(screen.getAllByText('58').length).toBeGreaterThanOrEqual(1);
      });
    });
  });
});
