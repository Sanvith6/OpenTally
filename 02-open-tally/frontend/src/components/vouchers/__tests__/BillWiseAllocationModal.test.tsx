import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BillWiseAllocationModal } from '../BillWiseAllocationModal';
import * as api from '../../../services/api';
import { PendingBill } from '../../../types';

vi.mock('../../../services/api', () => ({
  getPartyPendingBills: vi.fn(),
}));

const mockPendingBills: PendingBill[] = [
  {
    party_ledger_id: 'led_debtor_01',
    party_name: 'Acme Buyers Ltd',
    bill_name: 'INV-2026-001',
    bill_date: '2026-04-10',
    due_date: '2026-05-10',
    dr_cr: 'Dr',
    original_amount: '25000.00',
    settled_amount: '5000.00',
    pending_amount: '20000.00',
    overdue_days: 5,
  },
];

describe('Milestone 4: BillWiseAllocationModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads pending bills and defaults AgstRef matching line amount', async () => {
    vi.mocked(api.getPartyPendingBills).mockResolvedValue(mockPendingBills);
    const onSave = vi.fn();
    const onClose = vi.fn();

    render(
      <BillWiseAllocationModal
        isOpen={true}
        partyLedgerId="led_debtor_01"
        partyLedgerName="Acme Buyers Ltd"
        lineAmount="20000.00"
        lineEntryType="Cr"
        initialAllocations={[]}
        onSaveAllocations={onSave}
        onClose={onClose}
      />
    );

    expect(screen.getByText(/Bill-wise Details for:/i)).toBeTruthy();
    expect(screen.getByText(/Acme Buyers Ltd/i)).toBeTruthy();

    await waitFor(() => {
      expect(api.getPartyPendingBills).toHaveBeenCalledWith('led_debtor_01', expect.any(String));
    });

    // Check that AgstRef is selected and bill_name is INV-2026-001
    await waitFor(() => {
      expect(screen.getByDisplayValue(/INV-2026-001/i)).toBeTruthy();
      expect(screen.getByDisplayValue('20000.00')).toBeTruthy();
    });

    // Balanced sum = 20000.00, difference = 0.00
    const saveBtn = screen.getByRole('button', { name: /Accept Bill Allocations/i }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(false);

    fireEvent.click(saveBtn);
    expect(onSave).toHaveBeenCalledWith([
      expect.objectContaining({
        allocation_type: 'AgstRef',
        bill_name: 'INV-2026-001',
        amount: '20000.00',
        dr_cr: 'Cr',
      }),
    ]);
  });

  it('prevents saving when allocations do not balance line amount', async () => {
    vi.mocked(api.getPartyPendingBills).mockResolvedValue([]);
    const onSave = vi.fn();
    const onClose = vi.fn();

    render(
      <BillWiseAllocationModal
        isOpen={true}
        partyLedgerId="led_debtor_01"
        partyLedgerName="Acme Buyers Ltd"
        lineAmount="50000.00"
        lineEntryType="Dr"
        initialAllocations={[]}
        onSaveAllocations={onSave}
        onClose={onClose}
      />
    );

    await waitFor(() => {
      expect(screen.getByDisplayValue('50000.00')).toBeTruthy();
    });

    // Change amount to 30000.00 so difference is 20000.00
    const amountInput = screen.getByDisplayValue('50000.00');
    fireEvent.change(amountInput, { target: { value: '30000.00' } });

    // Save button should be disabled because allocated sum (30000) != line amount (50000)
    const saveBtn = screen.getByRole('button', { name: /Accept Bill Allocations/i }) as HTMLButtonElement;
    expect(saveBtn.disabled).toBe(true);
    expect(screen.getByText(/Difference: ₹/i)).toBeTruthy();
  });
});
