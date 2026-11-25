import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { VoucherDetailInspectorView } from '../VoucherDetailInspectorView';
import * as api from '../../../services/api';
import { Company, VoucherFullDetail } from '../../../types';

vi.mock('../../../services/api', () => ({
  getVoucherFullDetail: vi.fn(),
  cancelVoucher: vi.fn(),
  reverseVoucher: vi.fn(),
  exportInvoicePdf: vi.fn(),
  getVoucherEinvoiceMetadata: vi.fn().mockResolvedValue(null),
  saveVoucherEinvoiceMetadata: vi.fn(),
  exportEinvoiceJson: vi.fn(),
  exportEwayBillJson: vi.fn(),
}));

const mockCompany: Company = {
  id: 'cmp_demo_01',
  name: 'OpenTally Demo Corp',
  mailing_name: 'OpenTally HQ',
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

const mockVoucherDetail: VoucherFullDetail = {
  voucher: {
    id: 'vch_sales_101',
    company_id: 'cmp_demo_01',
    voucher_type_id: 'vt_sales',
    voucher_type_name: 'Sales',
    series_id: 'ser_sales_01',
    series_name: 'Main Sales Series',
    voucher_number: 'SALES-2026-0042',
    date: '2026-05-15',
    effective_date: '2026-05-15',
    status: 'posted',
    reference_no: 'PO-9988',
    reference_date: '2026-05-14',
    narration: 'Interstate commercial sale of inventory',
    total_amount: '118000.00',
    line_count: 3,
    reversal_of_voucher_id: null,
    reversal_voucher_id: null,
    cancellation_reason: null,
    created_at: '2026-05-15T10:00:00Z',
    updated_at: '2026-05-15T10:00:00Z',
  },
  lines: [
    {
      id: 'line_01',
      line_number: 1,
      ledger_id: 'led_debtor_01',
      ledger_name: 'National Distributors',
      group_name: 'Sundry Debtors',
      classification: 'Current Assets',
      entry_type: 'Dr',
      amount: '118000.00',
      narration: 'Gross invoice receivable',
    },
    {
      id: 'line_02',
      line_number: 2,
      ledger_id: 'led_sales_01',
      ledger_name: 'Sales Account',
      group_name: 'Sales Accounts',
      classification: 'Direct Incomes',
      entry_type: 'Cr',
      amount: '100000.00',
      narration: 'Net taxable sale',
    },
    {
      id: 'line_03',
      line_number: 3,
      ledger_id: 'led_igst_01',
      ledger_name: 'Output IGST',
      group_name: 'Duties & Taxes',
      classification: 'Current Liabilities',
      entry_type: 'Cr',
      amount: '18000.00',
      narration: '18% IGST',
    },
  ],
  gst_details: [
    {
      id: 'gst_01',
      party_ledger_id: 'led_debtor_01',
      party_name: 'National Distributors',
      party_gstin: '27AABCT3518Q1Z6',
      hsn_sac_code: '84713010',
      place_of_supply_state_code: '27',
      supply_type: 'IntraState',
      taxability: 'Taxable',
      taxable_value: '100000.00',
      gst_rate: '18.00',
      cgst_amount: '9000.00',
      sgst_amount: '9000.00',
      igst_amount: '0.00',
      cess_amount: '0.00',
      is_reverse_charge: false,
      is_cancelled: false,
    },
  ],
  bill_allocations: [
    {
      id: 'alloc_01',
      voucher_line_id: 'line_01',
      party_ledger_id: 'led_debtor_01',
      party_name: 'National Distributors',
      bill_name: 'SALES-2026-0042',
      allocation_type: 'NewRef',
      amount: '118000.00',
      dr_cr: 'Dr',
      bill_date: '2026-05-15',
      due_date: '2026-06-15',
      is_cancelled: false,
    },
  ],
  stock_movements: [
    {
      id: 'mov_01',
      stock_item_id: 'item_laptop_01',
      stock_item_name: 'ThinkPad T14',
      godown_name: 'Main Warehouse',
      movement_type: 'Outward',
      quantity: '2',
      rate: '50000.00',
      amount: '100000.00',
    },
  ],
  lineage: null,
};

describe('Milestone 4: VoucherDetailInspectorView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.URL.createObjectURL = vi.fn(() => 'blob:mock-url');
    window.URL.revokeObjectURL = vi.fn();
  });

  it('loads and renders complete voucher inspector with all sub-ledgers, GST, bill-wise, and stock movements', async () => {
    vi.mocked(api.getVoucherFullDetail).mockResolvedValue(mockVoucherDetail);
    const onBack = vi.fn();

    render(
      <VoucherDetailInspectorView
        activeCompany={mockCompany}
        voucherId="vch_sales_101"
        onBack={onBack}
      />
    );

    expect(screen.getByText(/Loading voucher details.../i)).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByRole('heading', { level: 2, name: /SALES-2026-0042/i })).toBeTruthy();
    });

    // Check header metadata
    expect(screen.getByText(/PO-9988/i)).toBeTruthy();
    expect(screen.getByText(/Interstate commercial sale of inventory/i)).toBeTruthy();

    // Check GL lines
    expect(screen.getAllByText('National Distributors').length).toBeGreaterThan(0);
    expect(screen.getByText('Sales Account')).toBeTruthy();
    expect(screen.getByText('Output IGST')).toBeTruthy();

    // Check GST breakdown
    expect(screen.getByText('84713010')).toBeTruthy();

    // Check Bill allocations
    expect(screen.getByText('NewRef')).toBeTruthy();

    // Check Stock movements
    expect(screen.getByText('ThinkPad T14')).toBeTruthy();
    expect(screen.getByText('Main Warehouse')).toBeTruthy();
  });

  it('allows cancelling a posted voucher with reason prompt', async () => {
    vi.mocked(api.getVoucherFullDetail).mockResolvedValue(mockVoucherDetail);
    vi.mocked(api.cancelVoucher).mockResolvedValue({
      id: 'vch_sales_101',
      status: 'cancelled',
      voucher_number: 'SALES-2026-0042',
      cancellation_reason: 'Entered with wrong party GSTIN',
    } as any);

    render(
      <VoucherDetailInspectorView
        activeCompany={mockCompany}
        voucherId="vch_sales_101"
        onBack={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole('heading', { level: 2, name: /SALES-2026-0042/i })).toBeTruthy();
    });

    const cancelBtn = screen.getByRole('button', { name: /Cancel Voucher/i });
    fireEvent.click(cancelBtn);

    expect(screen.getByRole('heading', { level: 3, name: /Cancel Voucher #SALES-2026-0042/i })).toBeTruthy();

    const reasonInput = screen.getByPlaceholderText(/e\.g\. Invoiced in error/i);
    fireEvent.change(reasonInput, { target: { value: 'Entered with wrong party GSTIN' } });

    const confirmCancelBtn = screen.getByRole('button', { name: /Confirm Cancellation/i });
    fireEvent.click(confirmCancelBtn);

    await waitFor(() => {
      expect(api.cancelVoucher).toHaveBeenCalledWith(
        'vch_sales_101',
        'Entered with wrong party GSTIN'
      );
    });
  });

  it('allows reversing a posted voucher with reversal date and explanation', async () => {
    vi.mocked(api.getVoucherFullDetail).mockResolvedValue(mockVoucherDetail);
    vi.mocked(api.reverseVoucher).mockResolvedValue({
      id: 'vch_rev_202',
      status: 'posted',
      voucher_number: 'JNL-REV-001',
      reversal_of_voucher_id: 'vch_sales_101',
    } as any);

    render(
      <VoucherDetailInspectorView
        activeCompany={mockCompany}
        voucherId="vch_sales_101"
        onBack={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByRole('heading', { level: 2, name: /SALES-2026-0042/i })).toBeTruthy();
    });

    const reverseBtn = screen.getByRole('button', { name: /Reverse Voucher/i });
    fireEvent.click(reverseBtn);

    expect(screen.getByRole('heading', { level: 3, name: /Reverse Voucher #SALES-2026-0042/i })).toBeTruthy();

    const reasonInput = screen.getByPlaceholderText(/e\.g\. Cheque bounced/i);
    fireEvent.change(reasonInput, { target: { value: 'Customer returned all items' } });

    const confirmReverseBtn = screen.getByRole('button', { name: /Post Reversal/i });
    fireEvent.click(confirmReverseBtn);

    await waitFor(() => {
      expect(api.reverseVoucher).toHaveBeenCalled();
    });
  });

  it('renders e-Invoice & e-Way Bill section and handles JSON export and metadata save', async () => {
    vi.mocked(api.getVoucherFullDetail).mockResolvedValue(mockVoucherDetail);
    vi.mocked(api.getVoucherEinvoiceMetadata).mockResolvedValue({
      id: 'meta_01',
      company_id: 'cmp_demo_01',
      voucher_id: 'vch_sales_101',
      irn: '1a2b3c4d5e6f78901234567890abcdef1234567890abcdef1234567890abcdef',
      ack_no: '122010045678',
      ack_date: '2026-05-15 11:00:00',
      signed_qr_code: null,
      einvoice_status: 'Generated',
      eway_bill_no: '251000123456',
      eway_bill_date: '2026-05-15',
      eway_valid_upto: '2026-05-17',
      transporter_id: null,
      transporter_name: null,
      transport_mode: 'Road',
      vehicle_no: 'KA01AB1234',
      distance_km: 100,
      created_at: '',
      updated_at: '',
    });
    vi.mocked(api.exportEinvoiceJson).mockResolvedValue(JSON.stringify({ Version: '1.1' }));
    vi.mocked(api.exportEwayBillJson).mockResolvedValue(JSON.stringify({ version: '1.0.0621' }));
    vi.mocked(api.saveVoucherEinvoiceMetadata).mockResolvedValue({
      id: 'meta_01',
      company_id: 'cmp_demo_01',
      voucher_id: 'vch_sales_101',
      irn: '1a2b3c4d5e6f78901234567890abcdef1234567890abcdef1234567890abcdef',
      ack_no: '122010045678',
      ack_date: '2026-05-15 11:00:00',
      signed_qr_code: null,
      einvoice_status: 'Generated',
      eway_bill_no: '251000123456',
      eway_bill_date: '2026-05-15',
      eway_valid_upto: '2026-05-17',
      transporter_id: null,
      transporter_name: null,
      transport_mode: 'Road',
      vehicle_no: 'KA01AB1234',
      distance_km: 100,
      created_at: '',
      updated_at: '',
    });

    render(
      <VoucherDetailInspectorView
        activeCompany={mockCompany}
        voucherId="vch_sales_101"
        onBack={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('einvoice-section')).toBeTruthy();
    });

    expect(screen.getByText(/1a2b3c4d5e6f78901234567890abcdef/i)).toBeTruthy();
    expect(screen.getByText(/251000123456/i)).toBeTruthy();

    // Export e-Invoice JSON
    const einvoiceBtn = screen.getByRole('button', { name: /Generate e-Invoice JSON/i });
    fireEvent.click(einvoiceBtn);
    await waitFor(() => {
      expect(api.exportEinvoiceJson).toHaveBeenCalledWith('vch_sales_101');
    });

    // Open e-Way Bill Modal
    const ewayModalBtn = screen.getByRole('button', { name: /Generate e-Way Bill JSON/i });
    fireEvent.click(ewayModalBtn);

    expect(screen.getByRole('heading', { level: 3, name: /Generate NIC e-Way Bill JSON/i })).toBeTruthy();
    const downloadEwayBtn = screen.getByRole('button', { name: /Download e-Way Bill JSON/i });
    fireEvent.click(downloadEwayBtn);

    await waitFor(() => {
      expect(api.exportEwayBillJson).toHaveBeenCalled();
    });
  });
});
