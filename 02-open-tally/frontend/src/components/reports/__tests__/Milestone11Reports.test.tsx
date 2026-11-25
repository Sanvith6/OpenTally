import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { GodownSummaryView } from '../GodownSummaryView';
import { BatchExpiryReportView } from '../BatchExpiryReportView';
import { CostCentresView } from '../../accounting/CostCentresView';
import { CostCentreBreakupReportView } from '../CostCentreBreakupReportView';
import * as api from '../../../services/api';
import {
  BatchExpiryReportRow,
  Company,
  CostCategory,
  CostCentre,
  CostCentreBreakupReport,
  Godown,
  GodownSummaryReport,
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

const mockGodowns: Godown[] = [
  {
    id: 'gdn_1',
    company_id: 'cmp_test',
    name: 'Central Godown',
    parent_id: null,
    description: null,
    address: 'Peenya, Bangalore',
    is_external: false,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockGodownSummary: GodownSummaryReport = {
  godown_id: 'gdn_1',
  godown_name: 'Central Godown',
  address: 'Peenya, Bangalore',
  is_external: false,
  items: [
    {
      stock_item_id: 'item_1',
      stock_item_name: 'Copper Wire 2.5mm',
      base_unit: 'Mtr',
      inflow_quantity: '100',
      outflow_quantity: '20',
      closing_quantity: '80',
      average_rate: '150.00',
      closing_value: '12000.00',
    },
  ],
  total_godown_value: '12000.00',
};

const mockBatchRows: BatchExpiryReportRow[] = [
  {
    stock_item_id: 'item_1',
    stock_item_name: 'Amoxicillin 500mg',
    batch_id: 'batch_1',
    batch_number: 'AMX-2025-A',
    mfg_date: '2025-01-01',
    expiry_date: '2025-12-31',
    closing_quantity: '500',
    unit_cost: '12.50',
    closing_value: '6250.00',
    days_until_expiry: 15,
    status: 'ExpiringIn30Days',
  },
  {
    stock_item_id: 'item_2',
    stock_item_name: 'Paracetamol 650mg',
    batch_id: 'batch_2',
    batch_number: 'PCM-2024-Z',
    mfg_date: '2024-01-01',
    expiry_date: '2024-12-31',
    closing_quantity: '100',
    unit_cost: '5.00',
    closing_value: '500.00',
    days_until_expiry: -50,
    status: 'Expired',
  },
];

const mockCategories: CostCategory[] = [
  {
    id: 'cat_1',
    company_id: 'cmp_test',
    name: 'Primary Cost Category',
    allocate_revenue: true,
    allocate_non_revenue: false,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockCentres: CostCentre[] = [
  {
    id: 'cc_north',
    company_id: 'cmp_test',
    category_id: 'cat_1',
    name: 'North Region Sales',
    parent_id: null,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockBreakup: CostCentreBreakupReport = {
  from_date: null,
  to_date: null,
  centres: [
    {
      cost_centre_id: 'cc_north',
      cost_centre_name: 'North Region Sales',
      category_id: 'cat_1',
      category_name: 'Primary Cost Category',
      debit_total: '25000.00',
      credit_total: '0.00',
      net_amount: '25000.00',
      allocations_count: 2,
    },
  ],
  total_debit: '25000.00',
  total_credit: '0.00',
  net_total: '25000.00',
};

describe('Milestone 11 Views & Reports', () => {
  const onBack = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('GodownSummaryView', () => {
    it('renders godown selection and stock items table with valuation', async () => {
      vi.spyOn(api, 'listGodowns').mockResolvedValue(mockGodowns);
      vi.spyOn(api, 'getGodownSummaryReport').mockResolvedValue(mockGodownSummary);

      render(<GodownSummaryView activeCompany={mockCompany} onBack={onBack} />);

      expect(screen.getByText('Godown Summary Report')).toBeTruthy();

      await waitFor(() => {
        expect(screen.getByText('Copper Wire 2.5mm')).toBeTruthy();
        expect(screen.getAllByText(/12000\.00/).length).toBeGreaterThanOrEqual(1);
      });

      fireEvent.click(screen.getByText('Back (Esc)'));
      expect(onBack).toHaveBeenCalled();
    });
  });

  describe('BatchExpiryReportView', () => {
    it('renders batch aging report with status badges and filters', async () => {
      vi.spyOn(api, 'getBatchExpiryReport').mockResolvedValue(mockBatchRows);

      render(<BatchExpiryReportView activeCompany={mockCompany} onBack={onBack} />);

      expect(screen.getByText('Batch & Expiry Aging Report')).toBeTruthy();

      await waitFor(() => {
        expect(screen.getByText('Amoxicillin 500mg')).toBeTruthy();
        expect(screen.getByText('AMX-2025-A')).toBeTruthy();
        expect(screen.getAllByText('Expired').length).toBeGreaterThanOrEqual(1);
        expect(screen.getByText('≤ 30 Days')).toBeTruthy();
      });
    });
  });

  describe('CostCentresView', () => {
    it('renders categories list and associated cost centres', async () => {
      vi.spyOn(api, 'listCostCategories').mockResolvedValue(mockCategories);
      vi.spyOn(api, 'listCostCentres').mockResolvedValue(mockCentres);

      render(<CostCentresView activeCompany={mockCompany} onBack={onBack} />);

      expect(screen.getByText('Cost Categories & Cost Centres')).toBeTruthy();

      await waitFor(() => {
        expect(screen.getAllByText('Primary Cost Category').length).toBeGreaterThanOrEqual(1);
        expect(screen.getByText('North Region Sales')).toBeTruthy();
      });
    });
  });

  describe('CostCentreBreakupReportView', () => {
    it('renders cost centre expense allocation totals', async () => {
      vi.spyOn(api, 'getCostCentreBreakupReport').mockResolvedValue(mockBreakup);

      render(<CostCentreBreakupReportView activeCompany={mockCompany} onBack={onBack} />);

      expect(screen.getByText('Cost Centre Breakup Report')).toBeTruthy();

      await waitFor(() => {
        expect(screen.getByText('North Region Sales')).toBeTruthy();
        expect(screen.getAllByText(/25000\.00/).length).toBeGreaterThanOrEqual(1);
      });
    });
  });
});
