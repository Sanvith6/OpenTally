import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { OrderProcessingView } from '../OrderProcessingView';
import * as api from '../../../services/api';
import {
  Company,
  PendingOrdersReport,
  PurchaseOrder,
  PurchaseOrderWithLines,
  SalesOrder,
  SalesOrderWithLines,
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

const mockSalesOrders: SalesOrder[] = [
  {
    id: 'so_1',
    company_id: 'cmp_test',
    order_number: 'SO-2024-001',
    order_date: '2024-05-10',
    due_date: '2024-05-25',
    party_ledger_id: 'led_cust_1',
    party_ledger_name: 'Acme Enterprises',
    reference_no: 'PO-REF-99',
    narration: 'Monthly batch delivery',
    status: 'Open',
    total_amount: '25000.00',
    created_at: '2024-05-10T09:00:00Z',
    updated_at: '2024-05-10T09:00:00Z',
  },
];

const mockSalesOrderDetail: SalesOrderWithLines = {
  order: mockSalesOrders[0],
  lines: [
    {
      id: 'sol_1',
      company_id: 'cmp_test',
      order_id: 'so_1',
      stock_item_id: 'item_1',
      stock_item_name: 'Precision Shaft 10mm',
      godown_id: 'godown_main',
      godown_name: 'Main Godown',
      ordered_qty: '50',
      fulfilled_qty: '0',
      pending_qty: '50',
      unit_rate: '500.00',
      discount_kind: undefined,
      discount_value: undefined,
      line_amount: '25000.00',
      created_at: '2024-05-10T09:00:00Z',
    },
  ],
};

const mockPurchaseOrders: PurchaseOrder[] = [
  {
    id: 'po_1',
    company_id: 'cmp_test',
    order_number: 'PO-2024-001',
    order_date: '2024-05-02',
    due_date: '2024-05-12',
    party_ledger_id: 'led_supp_1',
    party_ledger_name: 'Apex Raw Materials Ltd',
    reference_no: 'QUOT-442',
    narration: 'Raw steel supply',
    status: 'PartiallyFulfilled',
    total_amount: '40000.00',
    created_at: '2024-05-02T10:00:00Z',
    updated_at: '2024-05-02T10:00:00Z',
  },
];

const mockPurchaseOrderDetail: PurchaseOrderWithLines = {
  order: mockPurchaseOrders[0],
  lines: [
    {
      id: 'pol_1',
      company_id: 'cmp_test',
      order_id: 'po_1',
      stock_item_id: 'item_raw',
      stock_item_name: 'Stainless Steel Rod 20mm',
      godown_id: 'godown_main',
      godown_name: 'Main Godown',
      ordered_qty: '100',
      fulfilled_qty: '40',
      pending_qty: '60',
      unit_rate: '400.00',
      discount_kind: undefined,
      discount_value: undefined,
      line_amount: '40000.00',
      created_at: '2024-05-02T10:00:00Z',
    },
  ],
};

const mockPendingReport: PendingOrdersReport = {
  company_id: 'cmp_test',
  order_type: 'SalesOrder',
  as_of_date: '2024-05-31',
  items: [
    {
      order_id: 'so_1',
      order_number: 'SO-2024-001',
      order_date: '2024-05-10',
      due_date: '2024-05-25',
      party_ledger_id: 'led_cust_1',
      party_name: 'Acme Enterprises',
      stock_item_id: 'item_1',
      stock_item_name: 'Precision Shaft 10mm',
      godown_id: 'godown_main',
      godown_name: 'Main Godown',
      ordered_qty: '50',
      fulfilled_qty: '0',
      pending_qty: '50',
      unit_rate: '500.00',
      pending_amount: '25000.00',
      is_overdue: true,
    },
  ],
  total_pending_qty: '50',
  total_pending_amount: '25000.00',
};

describe('OrderProcessingView Component', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(api, 'listLedgers').mockResolvedValue([
      {
        id: 'led_cust_1',
        company_id: 'cmp_test',
        group_id: 'grp_ca',
        name: 'Acme Enterprises',
        is_reserved: false,
        ledger_classification: 'standard',
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
    ]);
    vi.spyOn(api, 'listStockItems').mockResolvedValue([
      {
        id: 'item_1',
        company_id: 'cmp_test',
        name: 'Precision Shaft 10mm',
        stock_group_id: 'sg_1',
        stock_category_id: null,
        base_unit_id: 'unit_pcs',
        alternate_unit_id: null,
        sku: 'SKU-001',
        description: 'Precision Shaft',
        opening_quantity: '100',
        opening_rate: '450.00',
        opening_value: '45000.00',
        reorder_level: '20',
        is_active: true,
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
    ]);
    vi.spyOn(api, 'listGodowns').mockResolvedValue([
      {
        id: 'godown_main',
        company_id: 'cmp_test',
        parent_id: null,
        name: 'Main Godown',
        description: null,
        address: null,
        is_external: false,
        is_active: true,
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
    ]);
    vi.spyOn(api, 'listVoucherSeries').mockResolvedValue([
      {
        id: 'series_sal',
        company_id: 'cmp_test',
        voucher_type_id: 'vt_sales',
        series_name: 'Default Sales',
        series_code: 'SAL',
        numbering_method: 'automatic',
        restart_period: 'yearly',
        is_active: true,
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
    ]);
    vi.spyOn(api, 'listSalesOrders').mockResolvedValue(mockSalesOrders);
    vi.spyOn(api, 'getSalesOrder').mockResolvedValue(mockSalesOrderDetail);
    vi.spyOn(api, 'listPurchaseOrders').mockResolvedValue(mockPurchaseOrders);
    vi.spyOn(api, 'getPurchaseOrder').mockResolvedValue(mockPurchaseOrderDetail);
    vi.spyOn(api, 'getPendingSalesOrders').mockResolvedValue(mockPendingReport);
    vi.spyOn(api, 'getPendingPurchaseOrders').mockResolvedValue(mockPendingReport);
  });

  it('renders with fallback when active company is null', () => {
    render(
      <OrderProcessingView
        activeCompany={null}
        onBack={() => {}}
      />
    );
    expect(screen.getByText('Order Processing')).toBeTruthy();
    expect(screen.getByText('(OpenTally)')).toBeTruthy();
  });

  it('renders sales order list by default with order details and actions', async () => {
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={() => {}}
      />
    );

    expect(screen.getByRole('button', { name: /Sales Orders \[Ctrl\+F8\]/i })).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByText('SO-2024-001')).toBeTruthy();
      expect(screen.getByText('Acme Enterprises')).toBeTruthy();
    });
  });

  it('switches between Sales Orders and Purchase Orders tabs', async () => {
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={() => {}}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('SO-2024-001')).toBeTruthy();
    });

    // Click Purchase Orders switcher button
    const poBtn = screen.getByRole('button', { name: /Purchase Orders \[Ctrl\+F9\]/i });
    fireEvent.click(poBtn);

    await waitFor(() => {
      expect(api.listPurchaseOrders).toHaveBeenCalledWith(undefined, 'cmp_test');
      expect(screen.getByText('PO-2024-001')).toBeTruthy();
      expect(screen.getByText('Apex Raw Materials Ltd')).toBeTruthy();
    });
  });

  it('switches to New Order Entry tab and renders form', async () => {
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={() => {}}
      />
    );

    // Click "+ Create New Order" tab
    const newOrderTab = screen.getByRole('button', { name: /\+ Create New Order/i });
    fireEvent.click(newOrderTab);

    await waitFor(() => {
      expect(screen.getByPlaceholderText('SO-001')).toBeTruthy();
      expect(screen.getByRole('button', { name: /Create Sales Order/i })).toBeTruthy();
    });
  });

  it('switches to Outstanding / Pending Report tab and displays pending items', async () => {
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={() => {}}
      />
    );

    // Click "Pending Orders Report (Outstanding)" tab
    const pendingTab = screen.getByRole('button', { name: /Pending Orders Report/i });
    fireEvent.click(pendingTab);

    await waitFor(() => {
      expect(api.getPendingSalesOrders).toHaveBeenCalled();
      expect(screen.getByText(/OVERDUE/i)).toBeTruthy();
    });
  });

  it('opens Convert to Invoice modal upon clicking Convert button and handles escape', async () => {
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={() => {}}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('SO-2024-001')).toBeTruthy();
    });

    // Click View / Convert action button
    const viewBtn = screen.getByRole('button', { name: /View \/ Convert/i });
    fireEvent.click(viewBtn);

    await waitFor(() => {
      expect(api.getSalesOrder).toHaveBeenCalledWith('so_1', 'cmp_test');
      expect(screen.getByText(/Party:/i)).toBeTruthy();
      expect(screen.getByRole('button', { name: /\[Convert to Invoice \(F8\)\]/i })).toBeTruthy();
    });

    // Click Convert to Sales Invoice button
    const convertBtn = screen.getByRole('button', { name: /\[Convert to Invoice \(F8\)\]/i });
    fireEvent.click(convertBtn);

    await waitFor(() => {
      expect(screen.getByText(/Enter Fulfill Quantities \(Supports Partial Quantities\):/i)).toBeTruthy();
      expect(screen.getByRole('button', { name: /Confirm & Post to Invoice/i })).toBeTruthy();
    });

    // Press Escape to dismiss modal
    fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => {
      expect(screen.queryByText(/Confirm & Post to Invoice/i)).toBeNull();
    });
  });

  it('calls onBack when Back button is clicked', () => {
    const handleBack = vi.fn();
    render(
      <OrderProcessingView
        activeCompany={mockCompany}
        initialOrderType="SalesOrder"
        onBack={handleBack}
      />
    );

    const backBtn = screen.getByRole('button', { name: /Back \(Esc\)/i });
    fireEvent.click(backBtn);
    expect(handleBack).toHaveBeenCalledTimes(1);
  });
});
