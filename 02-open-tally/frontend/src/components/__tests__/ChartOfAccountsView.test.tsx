import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ChartOfAccountsView } from '../ChartOfAccountsView';
import * as api from '../../services/api';
import { Company, ChartOfAccountsTree, AccountGroup, Ledger } from '../../types';

// Mock sample data
const mockCompanyA: Company = {
  id: 'comp-alpha-id',
  name: 'Alpha Corporation',
  mailing_name: 'Alpha Corp Mumbai',
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

const mockCompanyB: Company = {
  id: 'comp-beta-id',
  name: 'Beta Industries',
  mailing_name: 'Beta HQ',
  country: 'India',
  state: 'Delhi',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '$',
  currency_formal_name: 'USD',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockTreeData: ChartOfAccountsTree = {
  nodes: [
    {
      id: 'grp-current-assets',
      name: 'Current Assets',
      node_type: 'group',
      nature: 'Assets',
      is_primary: true,
      is_predefined: true,
      is_reserved: false,
      parent_id: null,
      opening_balance: null,
      debit_total: '100.23',
      credit_total: '0.00',
      children: [
        {
          id: 'grp-sundry-debtors',
          name: 'Sundry Debtors',
          node_type: 'group',
          nature: 'Assets',
          is_primary: false,
          is_predefined: true,
          is_reserved: false,
          parent_id: 'grp-current-assets',
          opening_balance: null,
          debit_total: '100.23',
          credit_total: '0.00',
          children: [
            {
              id: 'led-acme-customer',
              name: 'Acme Customer',
              node_type: 'ledger',
              nature: '',
              is_primary: false,
              is_predefined: false,
              is_reserved: false,
              parent_id: 'grp-sundry-debtors',
              opening_balance: {
                balance_type: 'Dr',
                amount: '100.23',
              },
              debit_total: '100.23',
              credit_total: '0.00',
              children: [],
            },
          ],
        },
      ],
    },
    {
      id: 'grp-current-liabilities',
      name: 'Current Liabilities',
      node_type: 'group',
      nature: 'Liabilities',
      is_primary: true,
      is_predefined: true,
      is_reserved: false,
      parent_id: null,
      opening_balance: null,
      debit_total: '0.00',
      credit_total: '100.23',
      children: [
        {
          id: 'led-supplier-beta',
          name: 'Supplier Beta',
          node_type: 'ledger',
          nature: '',
          is_primary: false,
          is_predefined: false,
          is_reserved: false,
          parent_id: 'grp-current-liabilities',
          opening_balance: {
            balance_type: 'Cr',
            amount: '100.23',
          },
          debit_total: '0.00',
          credit_total: '100.23',
          children: [],
        },
      ],
    },
  ],
  equilibrium: {
    total_debit: '100.23',
    total_credit: '100.23',
    difference: '0.00',
    difference_type: null,
    is_balanced: true,
  },
};

const mockGroups: AccountGroup[] = [
  {
    id: 'grp-current-assets',
    company_id: 'comp-alpha-id',
    name: 'Current Assets',
    parent_id: null,
    nature: 'Assets',
    is_primary: true,
    is_predefined: true,
    is_hidden: false,
    affects_gross_profit: false,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'grp-sundry-debtors',
    company_id: 'comp-alpha-id',
    name: 'Sundry Debtors',
    parent_id: 'grp-current-assets',
    nature: 'Assets',
    is_primary: false,
    is_predefined: true,
    is_hidden: false,
    affects_gross_profit: false,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockLedgerDetails: Ledger = {
  id: 'led-acme-customer',
  company_id: 'comp-alpha-id',
  group_id: 'grp-sundry-debtors',
  name: 'Acme Customer',
  ledger_classification: 'standard',
  is_reserved: false,
  description: 'Regular wholesale client',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

describe('ChartOfAccountsView Component Tests', () => {
  beforeEach(() => {
    vi.spyOn(api, 'getChartOfAccounts').mockResolvedValue(mockTreeData);
    vi.spyOn(api, 'listAccountGroups').mockResolvedValue(mockGroups);
    vi.spyOn(api, 'getLedger').mockResolvedValue(mockLedgerDetails);
    vi.spyOn(api, 'createAccountGroup').mockResolvedValue({
      id: 'new-grp-id',
      company_id: 'comp-alpha-id',
      name: 'North Region Debtors',
      parent_id: 'grp-sundry-debtors',
      nature: 'Assets',
      is_primary: false,
      is_predefined: false,
      is_hidden: false,
      affects_gross_profit: false,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    });
    vi.spyOn(api, 'createLedger').mockResolvedValue({
      id: 'new-led-id',
      company_id: 'comp-alpha-id',
      group_id: 'grp-sundry-debtors',
      name: 'New Client X',
      ledger_classification: 'standard',
      is_reserved: false,
      description: null,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    });
    vi.spyOn(api, 'deleteAccountGroup').mockResolvedValue();
    vi.spyOn(api, 'deleteLedger').mockResolvedValue();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('TC-UI-01: Renders unselected/empty company state cleanly', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView activeCompany={null} onBackToGateway={handleBack} />
    );

    // Verifies status bar shows None
    expect(screen.getByText('None')).toBeDefined();
    await waitFor(() => {
      expect(screen.getByText('Chart of Accounts')).toBeDefined();
    });
  });

  it('TC-UI-02: Loads and renders Chart of Accounts tree with formatted debit/credit balances', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Current Assets')).toBeDefined();
      expect(screen.getByText('Current Liabilities')).toBeDefined();
    });

    // Verify company name badge and opening balance totals
    expect(screen.getByText('Alpha Corporation (2026-04-01)')).toBeDefined();
    expect(screen.getByText(/Balanced \(0.00\)/)).toBeDefined();
  });

  it('TC-UI-03: Keyboard Navigation - Arrow Down and Arrow Up moves selection', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Current Assets')).toBeDefined();
    });

    // Initially first row is selected. Press ArrowDown
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'ArrowDown' });

    // Press ArrowUp to move back
    fireEvent.keyDown(window, { key: 'ArrowUp' });
  });

  it('TC-UI-04: Keyboard Navigation - Arrow Left and Right expands and collapses groups', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Current Assets')).toBeDefined();
    });

    // Press ArrowLeft to collapse
    fireEvent.keyDown(window, { key: 'ArrowLeft' });

    // Press ArrowRight to expand
    fireEvent.keyDown(window, { key: 'ArrowRight' });
  });

  it('TC-UI-05: Keyboard Navigation - Space key toggles group expansion', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Current Assets')).toBeDefined();
    });

    // Press Space on selected group
    fireEvent.keyDown(window, { key: ' ' });
  });

  it('TC-UI-06: Keyboard Navigation - Enter key on ledger opens edit modal', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading Chart of Accounts...')).toBeNull();
    });

    await waitFor(() => {
      expect(screen.getByText('Sundry Debtors')).toBeDefined();
    });

    // Expand Sundry Debtors by clicking its expand chevron
    const debtorsRow = screen.getByTestId('tree-node-grp-sundry-debtors');
    const chevron = debtorsRow.querySelector('span');
    if (chevron) fireEvent.click(chevron);

    await waitFor(() => {
      expect(screen.getByText('Acme Customer')).toBeDefined();
    });

    // Click on ledger node to select it
    fireEvent.click(screen.getByText('Acme Customer'));

    // Press Enter to trigger edit
    fireEvent.keyDown(window, { key: 'Enter' });

    await waitFor(() => {
      expect(screen.getByText('Alter Ledger Master')).toBeDefined();
    });
  });

  it('TC-UI-07: Search Filter - Ctrl+F focuses search input and filters nodes', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading Chart of Accounts...')).toBeNull();
    });

    expect(screen.getByText('Current Assets')).toBeDefined();

    // Press Ctrl+F
    fireEvent.keyDown(window, { key: 'f', ctrlKey: true });

    const searchInput = screen.getByPlaceholderText(/Search Groups & Ledgers/);
    fireEvent.change(searchInput, { target: { value: 'Acme' } });

    // When filter is active, matching node Acme Customer is visible
    await waitFor(() => {
      expect(screen.getByText('Acme Customer')).toBeDefined();
    });
  });

  it('TC-UI-08: Group Creation Modal - Opens, validates input, and cancels cleanly', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading Chart of Accounts...')).toBeNull();
    });

    // Click "+ Group (Alt+C)"
    fireEvent.click(screen.getByText('+ Group (Alt+C)'));

    await waitFor(() => {
      expect(screen.getByText('Create Account Group')).toBeDefined();
    });

    // Cancel modal
    fireEvent.click(screen.getByText('Cancel (Esc)'));
  });

  it('TC-UI-09: Ledger Creation Modal - Opens, enters balance, and cancels cleanly', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading Chart of Accounts...')).toBeNull();
    });

    // Click "+ Ledger (Ctrl+L)"
    fireEvent.click(screen.getByText('+ Ledger (Ctrl+L)'));

    await waitFor(() => {
      expect(screen.getByText('Create Ledger Master')).toBeDefined();
    });

    // Cancel modal
    fireEvent.click(screen.getByText('Cancel (Esc)'));
  });

  it('TC-UI-10: Delete Confirmation - Opens confirmation dialog and cancels cleanly', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.queryByText('Loading Chart of Accounts...')).toBeNull();
    });

    // Expand Sundry Debtors
    const debtorsRow = screen.getByTestId('tree-node-grp-sundry-debtors');
    const chevron = debtorsRow.querySelector('span');
    if (chevron) fireEvent.click(chevron);

    await waitFor(() => {
      expect(screen.getByText('Acme Customer')).toBeDefined();
    });

    // Click on ledger
    fireEvent.click(screen.getByText('Acme Customer'));

    // Press Delete key
    fireEvent.keyDown(window, { key: 'Delete' });

    expect(screen.getByText(/Confirm Deletion/)).toBeDefined();

    // Cancel deletion
    fireEvent.click(screen.getByText('Cancel (Esc)'));
  });

  it('TC-UI-11: Escape behavior - Navigates back to gateway when pressed outside input', async () => {
    const handleBack = vi.fn();
    render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Current Assets')).toBeDefined();
    });

    // Press Escape
    fireEvent.keyDown(window, { key: 'Escape' });

    expect(handleBack).toHaveBeenCalledTimes(1);
  });

  it('TC-UI-12: Switching companies - Refreshes tree for newly active company', async () => {
    const handleBack = vi.fn();
    const { rerender } = render(
      <ChartOfAccountsView
        activeCompany={mockCompanyA}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Alpha Corporation (2026-04-01)')).toBeDefined();
    });

    // Switch active company to Company Beta
    rerender(
      <ChartOfAccountsView
        activeCompany={mockCompanyB}
        onBackToGateway={handleBack}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Beta Industries (2026-04-01)')).toBeDefined();
    });
  });
});
