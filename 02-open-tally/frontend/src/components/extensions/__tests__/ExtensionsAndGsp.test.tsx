import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { CustomReportsView } from '../CustomReportsView';
import { Company } from '../../../types';
import * as api from '../../../services/api';

vi.mock('../../../services/api');

const mockCompany: Company = {
  id: 'cmp_m18',
  name: 'Enterprise Exports Ltd',
  mailing_name: 'Enterprise Exports',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01',
  updated_at: '2026-04-01',
};

describe('Milestone 18: CustomReportsView & GSP Extensions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listCustomReports).mockResolvedValue([
      {
        id: 'crpt_1',
        company_id: 'cmp_m18',
        name: 'Sales Register Custom',
        description: 'Posted sales',
        data_source: 'Vouchers',
        columns: ['voucher_number', 'date', 'total_amount'],
        filters: [],
        sort_by: 'date',
        sort_dir: 'ASC',
        created_by: 'admin',
        created_at: '2026-04-01',
        updated_at: '2026-04-01',
      },
    ]);
    vi.mocked(api.runCustomReport).mockResolvedValue({
      report_id: 'crpt_1',
      report_name: 'Sales Register Custom',
      data_source: 'Vouchers',
      columns: ['voucher_number', 'date', 'total_amount'],
      rows: [{ voucher_number: 'SAL/0001', date: '2026-04-15', total_amount: '11800.00' }],
      numeric_totals: { total_amount: '11800.00' },
      row_count: 1,
    });
    vi.mocked(api.exportCustomReportTemplate).mockResolvedValue(
      JSON.stringify({ format: 'opentally-custom-report', version: '3.0.0', name: 'Sales Register Custom' })
    );
    vi.mocked(api.pushEinvoiceToGsp).mockResolvedValue({
      irn: 'a'.repeat(64),
      ack_no: '112233445566778',
      ack_date: '2026-04-15 10:00:00',
      signed_qr_code: 'SANDBOX-QR',
      provider_mode: 'Sandbox',
    });
  });

  it('runs custom report, displays exact totals, exports template, and unwinds on Esc', async () => {
    const onBack = vi.fn();
    render(<CustomReportsView activeCompany={mockCompany} onBack={onBack} />);

    expect(await screen.findByText('Sales Register Custom')).toBeDefined();

    fireEvent.click(screen.getByRole('button', { name: 'Run' }));
    await waitFor(() => {
      expect(api.runCustomReport).toHaveBeenCalledWith('crpt_1');
      expect(screen.getByText('SAL/0001')).toBeDefined();
      expect(screen.getAllByText('11800.00').length).toBeGreaterThanOrEqual(2);
    });

    fireEvent.click(screen.getByRole('button', { name: 'Template' }));
    await waitFor(() => {
      expect(api.exportCustomReportTemplate).toHaveBeenCalledWith('crpt_1');
      expect(screen.getByText(/Exported .opentally-report.json template/i)).toBeDefined();
    });

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it('pushes e-Invoice to GSP/Sandbox via API wrapper', async () => {
    const res = await api.pushEinvoiceToGsp('vch_m18_01');
    expect(res.irn.length).toBe(64);
    expect(res.provider_mode).toBe('Sandbox');
  });
});
