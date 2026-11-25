import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { TdsReportsView } from '../TdsReportsView';
import * as api from '../../../services/api';
import { Company, Form26qSummary, TdsOutstandingReport } from '../../../types';

const company: Company = {
  id: 'company-1',
  name: 'Bharath Precision Components Pvt Ltd',
  mailing_name: 'Bharath Precision Components Pvt Ltd',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '',
  updated_at: '',
};

const mockOutstanding: TdsOutstandingReport = {
  as_of_date: '2026-05-15',
  total_unpaid_tds: '10000.00',
  sections: [
    {
      section_code: '194J',
      section_name: 'Fees for Professional or Technical Services',
      unpaid_tds_amount: '10000.00',
      voucher_count: 1,
      vouchers: [
        {
          voucher_tds_id: 'vtds_001',
          voucher_id: 'v_pur_001',
          voucher_number: 'PUR-001',
          voucher_date: '2026-05-10',
          party_ledger_id: 'led_sup_1',
          party_name: 'Infosys Consultancy Services',
          party_pan: 'AAACI1234F',
          section_code: '194J',
          gross_amount: '100000.00',
          tds_rate: '10.00',
          tds_amount: '10000.00',
          due_date: '2026-06-07',
          status: 'Due',
        },
      ],
    },
  ],
};

const mockForm26q: Form26qSummary = {
  company_id: 'company-1',
  financial_year: '2026-2027',
  quarter: 'Q1',
  deductor_tan: 'BLRP12345A',
  deductor_name: 'Bharath Precision Components Pvt Ltd',
  total_amount_paid: '100000.00',
  total_tds_deducted: '10000.00',
  total_tds_deposited: '10000.00',
  total_tds_unpaid: '0.00',
  challans: [
    {
      challan_bsr_code: '0210001',
      challan_date: '2026-06-05',
      challan_serial_no: '00123',
      payment_voucher_id: null,
      total_tds_deposited: '10000.00',
      voucher_count: 1,
    },
  ],
  deductions: [
    {
      voucher_tds_id: 'vtds_001',
      voucher_number: 'PUR-001',
      deductee_pan: 'AAACI1234F',
      deductee_name: 'Infosys Consultancy Services',
      deductee_type: 'Company_Domestic',
      section_code: '194J',
      payment_date: '2026-05-10',
      amount_paid: '100000.00',
      tds_rate: '10.00',
      tds_deducted: '10000.00',
      is_pan_missing_higher_rate: false,
      challan_serial_no: '00123',
      challan_date: '2026-06-05',
      challan_bsr_code: '0210001',
      status: 'Paid',
    },
    {
      voucher_tds_id: 'vtds_002',
      voucher_number: 'PUR-002',
      deductee_pan: null,
      deductee_name: 'Vendor Without PAN',
      deductee_type: 'Individual_HUF',
      section_code: '194C',
      payment_date: '2026-05-15',
      amount_paid: '50000.00',
      tds_rate: '20.00',
      tds_deducted: '10000.00',
      is_pan_missing_higher_rate: true,
      challan_serial_no: null,
      challan_date: null,
      challan_bsr_code: null,
      status: 'Due',
    },
  ],
};

describe('TdsReportsView Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'getTdsOutstanding').mockResolvedValue(mockOutstanding);
    vi.spyOn(api, 'getForm26qSummary').mockResolvedValue(mockForm26q);
    vi.spyOn(api, 'recordTdsChallanPayment').mockResolvedValue(1);
    vi.spyOn(api, 'exportForm26qJson').mockResolvedValue('{"fy":"2026-2027"}');
    vi.spyOn(api, 'exportForm26qCsv').mockResolvedValue('# Deductor TAN: BLRP12345A\n');
  });

  it('renders TDS Outstanding tab by default with metrics and vouchers', async () => {
    render(<TdsReportsView activeCompany={company} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('total-unpaid-tds').textContent).toContain('10000.00');
    });

    expect(screen.getByText(/Section 194J — Fees for Professional/)).toBeTruthy();
    expect(screen.getByText('PUR-001')).toBeTruthy();
    expect(screen.getByText('Infosys Consultancy Services')).toBeTruthy();
    expect(screen.getByText('AAACI1234F')).toBeTruthy();
  });

  it('allows voucher selection and proceeding to challan tagging', async () => {
    render(<TdsReportsView activeCompany={company} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByTestId('checkbox-voucher-vtds_001')).toBeTruthy();
    });

    // Check the voucher
    fireEvent.click(screen.getByTestId('checkbox-voucher-vtds_001'));

    // "Proceed to Challan Tagging" button should appear
    const proceedBtn = screen.getByTestId('btn-proceed-challan');
    expect(proceedBtn).toBeTruthy();
    fireEvent.click(proceedBtn);

    // Should switch to Challan tab
    expect(screen.getByText(/Record & Tag Challan Remittance/)).toBeTruthy();
    expect(screen.getByTestId('input-bsr-code')).toBeTruthy();

    // Fill BSR, date, serial
    fireEvent.change(screen.getByTestId('input-bsr-code'), { target: { value: '0210001' } });
    fireEvent.change(screen.getByTestId('input-challan-serial'), { target: { value: '00123' } });

    // Submit challan
    fireEvent.click(screen.getByTestId('btn-tag-challan-submit'));

    await waitFor(() => {
      expect(api.recordTdsChallanPayment).toHaveBeenCalledWith({
        company_id: 'company-1',
        voucher_tds_detail_ids: ['vtds_001'],
        challan_bsr_code: '0210001',
        challan_date: expect.any(String),
        challan_serial_no: '00123',
      });
    });
  });

  it('switches to Form 26Q tab, shows metrics, handles quarter pills and exports', async () => {
    // Mock URL.createObjectURL and revokeObjectURL
    const createObjectUrlMock = vi.fn().mockReturnValue('blob:mock-url');
    const revokeObjectUrlMock = vi.fn();
    window.URL.createObjectURL = createObjectUrlMock;
    window.URL.revokeObjectURL = revokeObjectUrlMock;

    render(<TdsReportsView activeCompany={company} onBack={vi.fn()} />);

    // Click Form 26Q tab
    fireEvent.click(screen.getByTestId('tab-tds-form26q'));

    await waitFor(() => {
      expect(screen.getByText('BLRP12345A')).toBeTruthy();
    });

    // Check Sec 206AA badge for vendor without PAN
    expect(screen.getByText('Sec 206AA')).toBeTruthy();

    // Switch quarter pill to Q2
    fireEvent.click(screen.getByTestId('pill-quarter-Q2'));
    await waitFor(() => {
      expect(api.getForm26qSummary).toHaveBeenCalledWith('2026-2027', 'Q2');
    });

    // Export JSON
    fireEvent.click(screen.getByTestId('btn-export-26q-json'));
    await waitFor(() => {
      expect(api.exportForm26qJson).toHaveBeenCalledWith('2026-2027', 'Q2');
    });

    // Export CSV
    fireEvent.click(screen.getByTestId('btn-export-26q-csv'));
    await waitFor(() => {
      expect(api.exportForm26qCsv).toHaveBeenCalledWith('2026-2027', 'Q2');
    });
  });

  it('navigates back when Escape is pressed', async () => {
    const onBack = vi.fn();
    render(<TdsReportsView activeCompany={company} onBack={onBack} />);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
