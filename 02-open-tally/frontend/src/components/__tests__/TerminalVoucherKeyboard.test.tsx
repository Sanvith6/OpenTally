import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { VoucherEntryView } from '../vouchers/VoucherEntryView';
import * as api from '../../services/api';
import { invoke } from '@tauri-apps/api/core';
import { Company, VoucherWithLines } from '../../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const company = { id: 'C', name: 'Keyboard Company', financial_year_begins: '2026-04-01', books_beginning_from: '2026-04-01' } as Company;
const draft = { voucher: { id: 'V', voucher_number: 'PAY-1', status: 'draft' }, lines: [], debit_total: '0.30', credit_total: '0.30', difference: '0.00', is_balanced: true } as unknown as VoucherWithLines;

beforeEach(() => {
  vi.restoreAllMocks();
  vi.spyOn(api, 'listVoucherTypes').mockResolvedValue([{ id: 'vt_payment', name: 'Payment', category: 'accounting' }] as never);
  vi.spyOn(api, 'listVoucherSeries').mockResolvedValue([{ id: 'SER', is_active: true, series_name: 'Payment', series_code: 'P' }] as never);
  vi.spyOn(api, 'listLedgers').mockResolvedValue([{ id: 'L1', name: 'Cash', ledger_classification: 'cash' }, { id: 'L2', name: 'Office Expense', ledger_classification: 'standard' }] as never);
  vi.spyOn(api, 'saveVoucherDraft').mockResolvedValue(draft);
  vi.mocked(invoke).mockResolvedValue({ ...draft, voucher: { ...draft.voucher, status: 'posted' } });
});

describe('accounting terminal keyboard workflow', () => {
  it('advances from Date using Enter without saving and starts Payment with two legs', async () => {
    render(<VoucherEntryView activeCompany={company} initialTypeId="vt_payment" onBackToGateway={vi.fn()} />);
    const date = await screen.findByLabelText(/Voucher Date/);
    date.focus();
    fireEvent.keyDown(date, { key: 'Enter' });
    expect(document.activeElement).toBe(screen.getByLabelText(/Voucher Type/));
    expect(screen.getAllByPlaceholderText('0.00')).toHaveLength(2);
    expect(api.saveVoucherDraft).not.toHaveBeenCalled();
  });

  it('posts an exactly balanced voucher with Ctrl+A using the current field values', async () => {
    render(<VoucherEntryView activeCompany={company} initialTypeId="vt_payment" onBackToGateway={vi.fn()} />);
    const amounts = await screen.findAllByPlaceholderText('0.00');
    fireEvent.change(document.getElementById('line-ledger-0')!, { target: { value: 'Cash' } });
    fireEvent.keyDown(document.getElementById('line-ledger-0')!, { key: 'ArrowDown' });
    fireEvent.keyDown(document.getElementById('line-ledger-0')!, { key: 'Enter' });
    fireEvent.change(document.getElementById('line-ledger-1')!, { target: { value: 'Office' } });
    fireEvent.keyDown(document.getElementById('line-ledger-1')!, { key: 'ArrowDown' });
    fireEvent.keyDown(document.getElementById('line-ledger-1')!, { key: 'Enter' });
    fireEvent.change(amounts[0], { target: { value: '0.30' } });
    fireEvent.change(amounts[1], { target: { value: '0.30' } });
    amounts[1].focus();
    fireEvent.keyDown(amounts[1], { key: 'a', ctrlKey: true });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('post_voucher', expect.objectContaining({ req: expect.objectContaining({ voucher_id: 'V' }) })));
    expect(api.saveVoucherDraft).toHaveBeenCalledWith(expect.objectContaining({ lines: expect.arrayContaining([expect.objectContaining({ amount: '0.30' })]) }));
    expect(await screen.findByText(/posted successfully/i)).toBeTruthy();
  });

  it('closes the acceptance dialog before navigating back and never posts on Enter in a field', async () => {
    const back = vi.fn();
    render(<VoucherEntryView activeCompany={company} initialTypeId="vt_payment" onBackToGateway={back} />);
    fireEvent.click(await screen.findByRole('button', { name: /Accept.*Y\/N/ }));
    expect(screen.getByRole('dialog', { name: /Accept voucher/ })).toBeTruthy();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: /Accept voucher/ })).toBeNull();
    expect(back).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(back).toHaveBeenCalledTimes(1);
  });
});
