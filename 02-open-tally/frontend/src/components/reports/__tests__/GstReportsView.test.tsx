import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { GstReportsView } from '../GstReportsView';
import * as api from '../../../services/api';
import { Company, Gstr1SummaryReport, Gstr3bSummaryReport, Gstr2bReconciliationReport } from '../../../types';

const company: Company = {
  id: 'company-1',
  name: 'Acme Bharat Trading Ltd',
  mailing_name: 'Acme Bharat Trading Ltd',
  country: 'India',
  state: 'Maharashtra',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '',
  updated_at: '',
};

const mockGstr1: Gstr1SummaryReport = {
  company_id: 'company-1',
  from_date: '2026-06-01',
  to_date: '2026-06-30',
  b2b_invoices: [
    {
      voucher_id: 'v_b2b_1',
      voucher_number: 'INV-001',
      voucher_date: '2026-06-05',
      party_ledger_id: 'led_cust_1',
      party_name: 'Zenith Retailers Ltd',
      party_gstin: '27AABCT3518Q1Z6',
      place_of_supply_state_code: '27',
      supply_type: 'IntraState',
      taxable_value: '10000.00',
      cgst_amount: '900.00',
      sgst_amount: '900.00',
      igst_amount: '0.00',
      cess_amount: '0.00',
      invoice_value: '11800.00',
    },
  ],
  b2c_large_invoices: [
    {
      voucher_id: 'v_b2c_l',
      voucher_number: 'INV-002',
      voucher_date: '2026-06-10',
      party_ledger_id: 'led_cust_2',
      party_name: 'Direct Consumer Large',
      place_of_supply_state_code: '29',
      taxable_value: '300000.00',
      igst_amount: '54000.00',
      cess_amount: '0.00',
      invoice_value: '354000.00',
    },
  ],
  b2c_small_summary: [
    {
      place_of_supply_state_code: '27',
      supply_type: 'IntraState',
      gst_rate: '18.00',
      taxable_value: '5000.00',
      cgst_amount: '450.00',
      sgst_amount: '450.00',
      igst_amount: '0.00',
      cess_amount: '0.00',
      total_tax: '900.00',
    },
  ],
  hsn_summary: [
    {
      hsn_sac_code: '8471',
      description: 'Computing machines',
      total_taxable_value: '315000.00',
      cgst_amount: '1350.00',
      sgst_amount: '1350.00',
      igst_amount: '54000.00',
      cess_amount: '0.00',
      total_tax: '56700.00',
      total_value: '371700.00',
    },
  ],
  total_taxable_value: '315000.00',
  total_cgst: '1350.00',
  total_sgst: '1350.00',
  total_igst: '54000.00',
  total_cess: '0.00',
  total_tax: '56700.00',
  total_invoice_value: '371700.00',
};

const mockGstr3b: Gstr3bSummaryReport = {
  company_id: 'company-1',
  from_date: '2026-06-01',
  to_date: '2026-06-30',
  table_3_1: [
    {
      nature_of_supplies: '(a) Outward Taxable Supplies',
      total_taxable_value: '315000.00',
      integrated_tax: '54000.00',
      central_tax: '1350.00',
      state_ut_tax: '1350.00',
      cess: '0.00',
    },
    {
      nature_of_supplies: '(d) Inward Supplies Liable to Reverse Charge',
      total_taxable_value: '0.00',
      integrated_tax: '0.00',
      central_tax: '0.00',
      state_ut_tax: '0.00',
      cess: '0.00',
    },
  ],
  table_4_itc: [
    {
      itc_details: '(A)(5) All other ITC',
      integrated_tax: '10000.00',
      central_tax: '2000.00',
      state_ut_tax: '2000.00',
      cess: '0.00',
    },
  ],
  total_output_tax: {
    integrated_tax: '54000.00',
    central_tax: '1350.00',
    state_ut_tax: '1350.00',
    cess: '0.00',
    total: '56700.00',
  },
  total_eligible_itc: {
    integrated_tax: '10000.00',
    central_tax: '2000.00',
    state_ut_tax: '2000.00',
    cess: '0.00',
    total: '14000.00',
  },
  net_tax_payable: {
    integrated_tax: '44000.00',
    central_tax: '0.00',
    state_ut_tax: '0.00',
    cess: '0.00',
    total: '44000.00',
  },
  closing_itc_balance: {
    integrated_tax: '0.00',
    central_tax: '650.00',
    state_ut_tax: '650.00',
    cess: '0.00',
    total: '1300.00',
  },
};

const mockGstr2b: Gstr2bReconciliationReport = {
  company_id: 'company-1',
  return_period: '062026',
  batch_id: 'batch-1',
  imported_at: '2026-06-30T10:00:00Z',
  summary: {
    total_portal_invoices: 2,
    matched_count: 1,
    mismatch_count: 0,
    missing_in_books_count: 1,
    missing_in_2b_count: 0,
    matched_itc: '1800.00',
    mismatch_itc: '0.00',
    missing_in_books_itc: '500.00',
    at_risk_itc: '0.00',
    total_eligible_itc: '1800.00',
  },
  invoices: [
    {
      id: 'p-1',
      batch_id: 'batch-1',
      supplier_gstin: '27AABCT3518Q1Z6',
      supplier_name: 'Zenith Retailers Ltd',
      invoice_number: 'INV-001',
      invoice_date: '2026-06-05',
      invoice_value: '11800.00',
      taxable_value: '10000.00',
      cgst_amount: '900.00',
      sgst_amount: '900.00',
      igst_amount: '0.00',
      cess_amount: '0.00',
      total_tax: '1800.00',
      itc_availability: 'Y',
      reconciliation_status: 'Matched',
      matched_voucher_id: 'v_pur_1',
      matched_voucher_number: 'PUR-001',
      books_taxable_value: '10000.00',
      books_total_tax: '1800.00',
      tax_difference: '0.00',
    },
    {
      id: 'p-2',
      batch_id: 'batch-1',
      supplier_gstin: '27AABCT3518Q1Z6',
      supplier_name: 'Alpha Traders',
      invoice_number: 'INV-999',
      invoice_date: '2026-06-15',
      invoice_value: '3540.00',
      taxable_value: '3000.00',
      cgst_amount: '250.00',
      sgst_amount: '250.00',
      igst_amount: '0.00',
      cess_amount: '0.00',
      total_tax: '500.00',
      itc_availability: 'Y',
      reconciliation_status: 'MissingInBooks',
      matched_voucher_id: null,
      matched_voucher_number: null,
      books_taxable_value: null,
      books_total_tax: null,
      tax_difference: '500.00',
    },
  ],
};

describe('GstReportsView Component', () => {
  beforeEach(() => {
    vi.spyOn(api, 'getGstr1Summary').mockResolvedValue(mockGstr1);
    vi.spyOn(api, 'getGstr3bSummary').mockResolvedValue(mockGstr3b);
    vi.spyOn(api, 'getGstr2bReconciliationReport').mockResolvedValue(mockGstr2b);
    vi.spyOn(api, 'importAndReconcileGstr2b').mockResolvedValue(mockGstr2b);
    vi.spyOn(api, 'exportGstr2bReconciliationCsv').mockResolvedValue('Reconciliation CSV Data');
    vi.spyOn(api, 'getCompanyGstProfile').mockResolvedValue({
      company_id: 'company-1',
      gstin: '27AABCT3518Q1Z6',
      state_code: '27',
      registration_type: 'Regular',
      gst_enabled: 1,
      created_at: '',
      updated_at: '',
    });
    vi.spyOn(api, 'listHsnSacCodes').mockResolvedValue([
      {
        id: 'hsn_1',
        company_id: 'company-1',
        code: '8471',
        description: 'Computers',
        code_type: 'HSN',
        default_gst_rate: '18.00',
        cess_rate: '0.00',
        effective_from: '2026-04-01',
        created_at: '',
      },
    ]);
    vi.spyOn(api, 'getGstLedgerMapping').mockResolvedValue({
      company_id: 'company-1',
      output_cgst_ledger_id: 'led_out_cgst',
      output_sgst_ledger_id: 'led_out_sgst',
      output_igst_ledger_id: 'led_out_igst',
      output_cess_ledger_id: '',
      input_cgst_ledger_id: 'led_in_cgst',
      input_sgst_ledger_id: 'led_in_sgst',
      input_igst_ledger_id: 'led_in_igst',
      input_cess_ledger_id: '',
    });
    vi.spyOn(api, 'saveCompanyGstProfile').mockResolvedValue({
      company_id: 'company-1',
      gstin: '27AABCT3518Q1Z6',
      state_code: '27',
      registration_type: 'Regular',
      gst_enabled: 1,
      created_at: '',
      updated_at: '',
    });
  });

  it('renders GSTR-1 outward supplies with KPI cards and table sections', async () => {
    const onBack = vi.fn();
    render(<GstReportsView activeCompany={company} onBack={onBack} />);

    expect(await screen.findByTestId('gstr1-view')).toBeTruthy();

    // Check KPI cards
    expect(screen.getByText('₹ 315000.00')).toBeTruthy();
    expect(screen.getByText('₹ 371700.00')).toBeTruthy();

    // Check Table 4 B2B
    expect(screen.getByText('Zenith Retailers Ltd')).toBeTruthy();
    expect(screen.getByText('27AABCT3518Q1Z6')).toBeTruthy();
    expect(screen.getByText('11800.00')).toBeTruthy();

    // Check Table 5 B2C Large
    expect(screen.getByText('Direct Consumer Large')).toBeTruthy();
    expect(screen.getByText('354000.00')).toBeTruthy();

    // Check Table 12 HSN
    expect(screen.getByText('8471')).toBeTruthy();
    expect(screen.getByText('56700.00')).toBeTruthy();
  });

  it('switches to GSTR-3B tab and displays outward liability, eligible ITC, and net tax payable', async () => {
    const onBack = vi.fn();
    render(<GstReportsView activeCompany={company} onBack={onBack} />);

    const gstr3bTab = screen.getByRole('tab', { name: /GSTR-3B/i });
    fireEvent.click(gstr3bTab);

    expect(await screen.findByTestId('gstr3b-view')).toBeTruthy();
    expect(api.getGstr3bSummary).toHaveBeenCalled();

    // Net tax payable & eligible ITC
    expect(screen.getByText('₹ 44000.00')).toBeTruthy();
    expect(screen.getByText('₹ 14000.00')).toBeTruthy();
    expect(screen.getByText('₹ 1300.00')).toBeTruthy();
  });

  it('switches to GST Settings & Masters tab and displays profile information', async () => {
    const onBack = vi.fn();
    render(<GstReportsView activeCompany={company} onBack={onBack} />);

    const settingsTab = screen.getByRole('tab', { name: /GST Settings & Masters/i });
    fireEvent.click(settingsTab);

    expect(await screen.findByTestId('gst-settings-view')).toBeTruthy();
    expect(api.getCompanyGstProfile).toHaveBeenCalled();
    expect(api.listHsnSacCodes).toHaveBeenCalled();

    expect(screen.getByDisplayValue('27AABCT3518Q1Z6')).toBeTruthy();
    expect(screen.getByDisplayValue('27')).toBeTruthy();
    expect(screen.getByText('Output CGST:')).toBeTruthy();
  });

  it('triggers onBack when Back to Gateway is clicked', async () => {
    const onBack = vi.fn();
    render(<GstReportsView activeCompany={company} onBack={onBack} />);

    fireEvent.click(screen.getByText('Back to Gateway'));
    expect(onBack).toHaveBeenCalled();
  });

  it('triggers GSTR-1 JSON export when export button is clicked', async () => {
    const mockJson = JSON.stringify({ gstin: '27AABCT3518Q1Z6', fp: '062026' });
    vi.spyOn(api, 'exportGstr1Json').mockResolvedValue(mockJson);

    render(<GstReportsView activeCompany={company} onBack={vi.fn()} />);

    const exportBtn = screen.getByRole('button', { name: /Export GSTR-1 JSON/i });
    expect(exportBtn).toBeTruthy();

    fireEvent.click(exportBtn);

    await waitFor(() => {
      expect(api.exportGstr1Json).toHaveBeenCalled();
    });
  });

  it('triggers GSTR-3B JSON export when export button is clicked on 3B tab', async () => {
    const mockJson = JSON.stringify({ gstin: '27AABCT3518Q1Z6', ret_period: '062026' });
    vi.spyOn(api, 'exportGstr3bJson').mockResolvedValue(mockJson);

    render(<GstReportsView activeCompany={company} onBack={vi.fn()} />);

    const gstr3bTab = screen.getByRole('tab', { name: /GSTR-3B/i });
    fireEvent.click(gstr3bTab);

    const exportBtn = await screen.findByRole('button', { name: /Export GSTR-3B JSON/i });
    expect(exportBtn).toBeTruthy();

    fireEvent.click(exportBtn);

    await waitFor(() => {
      expect(api.exportGstr3bJson).toHaveBeenCalled();
    });
  });

  it('renders GSTR-2B tab with summary cards and table', async () => {
    const onBack = vi.fn();
    render(<GstReportsView activeCompany={company} onBack={onBack} />);

    const gstr2bTab = screen.getByRole('tab', { name: /GSTR-2B/i });
    fireEvent.click(gstr2bTab);

    expect(await screen.findByTestId('gstr2b-view')).toBeTruthy();
    expect(screen.getByTestId('gstr2b-table')).toBeTruthy();
    expect(api.getGstr2bReconciliationReport).toHaveBeenCalled();

    // Check KPI cards
    expect(screen.getByText('Matched ITC')).toBeTruthy();
    expect(screen.getAllByText('₹ 1800.00').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Missing in Books').length).toBeGreaterThan(0);
    expect(screen.getAllByText('₹ 500.00').length).toBeGreaterThan(0);

    // Check table rows
    expect(screen.getByText('PUR-001')).toBeTruthy();
    expect(screen.getByText('Alpha Traders')).toBeTruthy();
    expect(screen.getByText('INV-999')).toBeTruthy();
  });

  it('handles GSTR-2B JSON file upload', async () => {
    render(<GstReportsView activeCompany={company} onBack={vi.fn()} />);

    const gstr2bTab = screen.getByRole('tab', { name: /GSTR-2B/i });
    fireEvent.click(gstr2bTab);

    expect(await screen.findByTestId('gstr2b-view')).toBeTruthy();

    const fileInput = screen.getByLabelText(/Upload GSTR-2B JSON/i);
    const mockFile = new File([JSON.stringify({ data: { docdata: { b2b: [] } } })], 'gstr2b.json', {
      type: 'application/json',
    });

    fireEvent.change(fileInput, { target: { files: [mockFile] } });

    await waitFor(() => {
      expect(api.importAndReconcileGstr2b).toHaveBeenCalled();
    });
  });

  it('exports GSTR-2B reconciliation CSV when export button is clicked', async () => {
    render(<GstReportsView activeCompany={company} onBack={vi.fn()} />);

    const gstr2bTab = screen.getByRole('tab', { name: /GSTR-2B/i });
    fireEvent.click(gstr2bTab);

    expect(await screen.findByTestId('gstr2b-view')).toBeTruthy();

    const exportBtn = screen.getByRole('button', { name: /Export Reconciliation CSV/i });
    fireEvent.click(exportBtn);

    await waitFor(() => {
      expect(api.exportGstr2bReconciliationCsv).toHaveBeenCalled();
    });
  });
});
