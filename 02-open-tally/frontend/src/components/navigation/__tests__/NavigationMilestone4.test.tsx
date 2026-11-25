import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BreadcrumbBar } from '../BreadcrumbBar';
import { FunctionKeyBar } from '../FunctionKeyBar';
import { GoToModal } from '../GoToModal';
import * as api from '../../../services/api';
import { NavigationStackEntry, GotoSearchResult } from '../../../types';

vi.mock('../../../services/api', () => ({
  globalGotoSearch: vi.fn(),
}));

describe('Milestone 4: Navigation Components', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('BreadcrumbBar Component', () => {
    const mockStack: NavigationStackEntry[] = [
      { view: 'gateway', title: 'Gateway of OpenTally' },
      { view: 'trial_balance', title: 'Trial Balance' },
      { view: 'ledger_report', title: 'Ledger: Sundry Debtors', params: { ledgerId: 'led_1' } },
    ];

    it('renders hierarchical breadcrumb trail and separators', () => {
      const onNavigateToLevel = vi.fn();
      const onOpenGoTo = vi.fn();

      render(
        <BreadcrumbBar
          stack={mockStack}
          onNavigateToLevel={onNavigateToLevel}
          onOpenGoTo={onOpenGoTo}
        />
      );

      expect(screen.getByText('Gateway of OpenTally')).toBeTruthy();
      expect(screen.getByText('Trial Balance')).toBeTruthy();
      expect(screen.getByText('Ledger: Sundry Debtors')).toBeTruthy();

      // Active (last) item is disabled
      const activeBtn = screen.getByText('Ledger: Sundry Debtors') as HTMLButtonElement;
      expect(activeBtn.disabled).toBe(true);

      // Clicking previous level triggers onNavigateToLevel
      const tbBtn = screen.getByText('Trial Balance');
      fireEvent.click(tbBtn);
      expect(onNavigateToLevel).toHaveBeenCalledWith(1);

      // Clicking Go To pill triggers onOpenGoTo
      const gotoBtn = screen.getByTestId('open-goto-button');
      fireEvent.click(gotoBtn);
      expect(onOpenGoTo).toHaveBeenCalledTimes(1);
    });
  });

  describe('FunctionKeyBar Component', () => {
    it('renders high-contrast TallyPrime function shortcuts and triggers actions', () => {
      const onCompanyClick = vi.fn();
      const onSalesClick = vi.fn();
      const onPurchaseClick = vi.fn();
      const onGoToClick = vi.fn();
      const onBackClick = vi.fn();

      render(
        <FunctionKeyBar
          canBack={true}
          onCompanyClick={onCompanyClick}
          onSalesClick={onSalesClick}
          onPurchaseClick={onPurchaseClick}
          onGoToClick={onGoToClick}
          onBackClick={onBackClick}
        />
      );

      expect(screen.getByText('F2')).toBeTruthy();
      expect(screen.getByText('F3')).toBeTruthy();
      expect(screen.getByText('F8')).toBeTruthy();
      expect(screen.getByText('F9')).toBeTruthy();
      expect(screen.getByText('Alt+G')).toBeTruthy();
      expect(screen.getByText('Esc')).toBeTruthy();

      fireEvent.click(screen.getByText('Company'));
      expect(onCompanyClick).toHaveBeenCalledTimes(1);

      fireEvent.click(screen.getByText('Sales'));
      expect(onSalesClick).toHaveBeenCalledTimes(1);

      fireEvent.click(screen.getByText('Purchase'));
      expect(onPurchaseClick).toHaveBeenCalledTimes(1);

      fireEvent.click(screen.getByText('Go To'));
      expect(onGoToClick).toHaveBeenCalledTimes(1);

      fireEvent.click(screen.getByText('Back'));
      expect(onBackClick).toHaveBeenCalledTimes(1);
    });

    it('disables Back button when canBack is false', () => {
      render(<FunctionKeyBar canBack={false} />);
      const backBtn = screen.getByRole('button', { name: /Back/i }) as HTMLButtonElement;
      expect(backBtn.disabled).toBe(true);
    });
  });

  describe('GoToModal Component', () => {
    it('does not render when closed', () => {
      render(<GoToModal isOpen={false} onClose={vi.fn()} onSelectView={vi.fn()} />);
      expect(screen.queryByPlaceholderText(/Go To: Type a report/i)).toBeNull();
    });

    it('filters screens catalog and allows selection with click and Enter', async () => {
      const onSelectView = vi.fn();
      const onClose = vi.fn();

      render(<GoToModal isOpen={true} onClose={onClose} onSelectView={onSelectView} />);

      const searchInput = screen.getByPlaceholderText(/Go To: Type a report/i);
      expect(searchInput).toBeTruthy();

      // Search for Balance Sheet
      fireEvent.change(searchInput, { target: { value: 'Balance' } });

      await waitFor(() => {
        expect(screen.getByText('Balance Sheet')).toBeTruthy();
      });

      // Click on result
      fireEvent.click(screen.getByText('Balance Sheet'));
      expect(onSelectView).toHaveBeenCalledWith('balance_sheet', 'Balance Sheet', undefined);
    });

    it('renders backend search results for vouchers and ledgers', async () => {
      const mockBackendResults: GotoSearchResult[] = [
        {
          id: 'vch_sales_001',
          category: 'Voucher',
          title: 'Voucher INV-2026-0001',
          subtitle: 'Sales • 2026-05-10 • ₹ 45,000.00',
          target_view: 'voucher_detail',
          target_id: 'vch_sales_001',
        },
      ];

      vi.mocked(api.globalGotoSearch).mockResolvedValue(mockBackendResults);

      const onSelectView = vi.fn();
      const onClose = vi.fn();

      render(<GoToModal isOpen={true} onClose={onClose} onSelectView={onSelectView} />);

      const searchInput = screen.getByPlaceholderText(/Go To: Type a report/i);
      fireEvent.change(searchInput, { target: { value: 'INV-2026' } });

      await waitFor(() => {
        expect(api.globalGotoSearch).toHaveBeenCalledWith('INV-2026', 25);
      });

      await waitFor(() => {
        expect(screen.getByText('Voucher INV-2026-0001')).toBeTruthy();
      });

      fireEvent.click(screen.getByText('Voucher INV-2026-0001'));
      expect(onSelectView).toHaveBeenCalledWith(
        'voucher_detail',
        'Voucher INV-2026-0001',
        { voucherId: 'vch_sales_001' }
      );
    });
  });
});
