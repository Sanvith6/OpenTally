import { useCallback, useEffect, useState } from 'react';
import { Company } from '../../types';
import {
  cancelPurchaseOrder,
  cancelSalesOrder,
  convertPurchaseOrderToInvoice,
  convertSalesOrderToInvoice,
  createPurchaseOrder,
  createSalesOrder,
  listLedgers,
  getPendingPurchaseOrders,
  getPendingSalesOrders,
  getPurchaseOrder,
  getSalesOrder,
  listPurchaseOrders,
  listSalesOrders,
  listStockItems,
  listGodowns,
  listVoucherSeries,
  PendingOrdersReport,
  PurchaseOrder,
  PurchaseOrderLine,
  SalesOrder,
  SalesOrderLine,
} from '../../services/api';

interface Props {
  activeCompany: Company | null;
  initialOrderType?: 'SalesOrder' | 'PurchaseOrder';
  onBack: () => void;
}

interface NewOrderLineState {
  stock_item_id: string;
  godown_id: string;
  ordered_qty: string;
  unit_rate: string;
  discount_kind: string;
  discount_value: string;
}

export function OrderProcessingView({
  activeCompany,
  initialOrderType = 'SalesOrder',
  onBack,
}: Props) {
  const [orderType, setOrderType] = useState<'SalesOrder' | 'PurchaseOrder'>(initialOrderType);
  const [activeTab, setActiveTab] = useState<'list' | 'create' | 'pending'>('list');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);

  // Orders list state
  const [orders, setOrders] = useState<(PurchaseOrder | SalesOrder)[]>([]);
  const [statusFilter, setStatusFilter] = useState<string>('All');
  const [searchQuery, setSearchQuery] = useState('');

  // Selected Order for Detail / Conversion
  const [selectedOrder, setSelectedOrder] = useState<{
    order: PurchaseOrder | SalesOrder;
    lines: (PurchaseOrderLine | SalesOrderLine)[];
  } | null>(null);

  // Convert to Invoice Modal
  const [isConvertModalOpen, setIsConvertModalOpen] = useState(false);
  const [invoiceDate, setInvoiceDate] = useState(new Date().toISOString().slice(0, 10));
  const [seriesId, setSeriesId] = useState('');
  const [seriesList, setSeriesList] = useState<any[]>([]);
  const [convertQuantities, setConvertQuantities] = useState<Record<string, string>>({});

  // Pending Orders Report state
  const [asOfDate, setAsOfDate] = useState(new Date().toISOString().slice(0, 10));
  const [pendingReport, setPendingReport] = useState<PendingOrdersReport | null>(null);

  // Masters for Create Form
  const [ledgers, setLedgers] = useState<{ id: string; name: string }[]>([]);
  const [stockItems, setStockItems] = useState<{ id: string; name: string }[]>([]);
  const [godowns, setGodowns] = useState<{ id: string; name: string }[]>([]);

  // Create Order Form State
  const [orderNumber, setOrderNumber] = useState('');
  const [orderDate, setOrderDate] = useState(new Date().toISOString().slice(0, 10));
  const [dueDate, setDueDate] = useState('');
  const [partyLedgerId, setPartyLedgerId] = useState('');
  const [referenceNo, setReferenceNo] = useState('');
  const [narration, setNarration] = useState('');
  const [lines, setLines] = useState<NewOrderLineState[]>([
    {
      stock_item_id: '',
      godown_id: '',
      ordered_qty: '1',
      unit_rate: '100.00',
      discount_kind: '',
      discount_value: '',
    },
  ]);

  // Load masters on company change
  useEffect(() => {
    if (!activeCompany) return;
    const fetchMasters = async () => {
      try {
        const ledgersList = await listLedgers();
        setLedgers(ledgersList.map((l: any) => ({ id: l.id, name: l.name })));

        const items = await listStockItems();
        setStockItems(items.map((i: any) => ({ id: i.id, name: i.name })));

        const gList = await listGodowns();
        setGodowns(gList.map((g: any) => ({ id: g.id, name: g.name })));

        const vTypeId = orderType === 'PurchaseOrder' ? 'vt_pur' : 'vt_sales';
        const sList = await listVoucherSeries(vTypeId);
        setSeriesList(sList);
        if (sList.length > 0) {
          setSeriesId(sList[0].id);
        }
      } catch (err) {
        console.error('Failed to load masters:', err);
      }
    };
    void fetchMasters();
  }, [activeCompany, orderType]);

  // Load orders
  const loadOrders = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const filter = statusFilter === 'All' ? undefined : statusFilter;
      if (orderType === 'PurchaseOrder') {
        const data = await listPurchaseOrders(filter, activeCompany.id);
        setOrders(data);
      } else {
        const data = await listSalesOrders(filter, activeCompany.id);
        setOrders(data);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, orderType, statusFilter]);

  // Load pending report
  const loadPendingReport = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      if (orderType === 'PurchaseOrder') {
        const rep = await getPendingPurchaseOrders(asOfDate, activeCompany.id);
        setPendingReport(rep);
      } else {
        const rep = await getPendingSalesOrders(asOfDate, activeCompany.id);
        setPendingReport(rep);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, orderType, asOfDate]);

  useEffect(() => {
    if (activeTab === 'list') {
      void loadOrders();
    } else if (activeTab === 'pending') {
      void loadPendingReport();
    }
  }, [activeTab, loadOrders, loadPendingReport]);

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        if (isConvertModalOpen) {
          setIsConvertModalOpen(false);
        } else if (selectedOrder) {
          setSelectedOrder(null);
        } else {
          onBack();
        }
      } else if (e.key === 'F8' && e.ctrlKey) {
        e.preventDefault();
        setOrderType('SalesOrder');
        setSelectedOrder(null);
        setIsConvertModalOpen(false);
      } else if (e.key === 'F9' && e.ctrlKey) {
        e.preventDefault();
        setOrderType('PurchaseOrder');
        setSelectedOrder(null);
        setIsConvertModalOpen(false);
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onBack, isConvertModalOpen, selectedOrder]);

  const handleSelectOrder = async (orderId: string) => {
    if (!activeCompany) return;
    setLoading(true);
    try {
      if (orderType === 'PurchaseOrder') {
        const res = await getPurchaseOrder(orderId, activeCompany.id);
        setSelectedOrder(res);
      } else {
        const res = await getSalesOrder(orderId, activeCompany.id);
        setSelectedOrder(res);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleOpenConvertModal = () => {
    if (!selectedOrder) return;
    const initialQty: Record<string, string> = {};
    for (const l of selectedOrder.lines) {
      initialQty[l.id] = l.pending_qty;
    }
    setConvertQuantities(initialQty);
    setIsConvertModalOpen(true);
  };

  const handleExecuteConvert = async () => {
    if (!activeCompany || !selectedOrder) return;
    setLoading(true);
    setError(null);
    try {
      const fulfillLines = selectedOrder.lines
        .map((l) => ({
          order_line_id: l.id,
          fulfill_qty: convertQuantities[l.id] || '0',
          godown_id: l.godown_id || undefined,
        }))
        .filter((l) => parseFloat(l.fulfill_qty) > 0);

      if (fulfillLines.length === 0) {
        setError('Please enter at least one quantity greater than zero to fulfill.');
        setLoading(false);
        return;
      }

      if (orderType === 'PurchaseOrder') {
        const res = await convertPurchaseOrderToInvoice(
          {
            order_id: selectedOrder.order.id,
            invoice_date: invoiceDate,
            series_id: seriesId,
            lines: fulfillLines,
            reference_no: selectedOrder.order.order_number,
          },
          activeCompany.id
        );
        setSuccessMsg(
          `Successfully converted to Purchase Invoice #${res.voucher_number}! New order status: ${res.order_status}`
        );
      } else {
        const res = await convertSalesOrderToInvoice(
          {
            order_id: selectedOrder.order.id,
            invoice_date: invoiceDate,
            series_id: seriesId,
            lines: fulfillLines,
            reference_no: selectedOrder.order.order_number,
          },
          activeCompany.id
        );
        setSuccessMsg(
          `Successfully converted to Sales Invoice #${res.voucher_number}! New order status: ${res.order_status}`
        );
      }
      setIsConvertModalOpen(false);
      setSelectedOrder(null);
      void loadOrders();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCancelOrder = async (orderId: string) => {
    if (!activeCompany) return;
    if (!confirm('Are you sure you want to cancel this order?')) return;
    setLoading(true);
    setError(null);
    try {
      if (orderType === 'PurchaseOrder') {
        await cancelPurchaseOrder(orderId, 'User requested cancellation', activeCompany.id);
      } else {
        await cancelSalesOrder(orderId, 'User requested cancellation', activeCompany.id);
      }
      setSuccessMsg('Order successfully cancelled.');
      setSelectedOrder(null);
      void loadOrders();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCreateOrder = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const cleanLines = lines.map((l) => ({
        stock_item_id: l.stock_item_id,
        godown_id: l.godown_id ? l.godown_id : null,
        ordered_qty: l.ordered_qty,
        unit_rate: l.unit_rate,
        discount_kind: l.discount_kind ? l.discount_kind : null,
        discount_value: l.discount_value ? l.discount_value : null,
      }));

      if (orderType === 'PurchaseOrder') {
        await createPurchaseOrder(
          {
            order_number: orderNumber,
            order_date: orderDate,
            due_date: dueDate ? dueDate : null,
            party_ledger_id: partyLedgerId,
            reference_no: referenceNo ? referenceNo : null,
            narration: narration ? narration : null,
            lines: cleanLines,
          },
          activeCompany.id
        );
      } else {
        await createSalesOrder(
          {
            order_number: orderNumber,
            order_date: orderDate,
            due_date: dueDate ? dueDate : null,
            party_ledger_id: partyLedgerId,
            reference_no: referenceNo ? referenceNo : null,
            narration: narration ? narration : null,
            lines: cleanLines,
          },
          activeCompany.id
        );
      }

      setSuccessMsg(`Order ${orderNumber} created successfully! (Pre-accounting invariant preserved: 0 GL / 0 FIFO pollution)`);
      setActiveTab('list');
      setOrderNumber('');
      void loadOrders();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const filteredOrders = orders.filter((o) => {
    const q = searchQuery.toLowerCase();
    return (
      o.order_number.toLowerCase().includes(q) ||
      (o.party_ledger_name && o.party_ledger_name.toLowerCase().includes(q))
    );
  });

  return (
    <div
      className="order-processing-view"
      style={{
        padding: '16px',
        background: '#0f172a',
        color: '#f8fafc',
        height: '100%',
        overflowY: 'auto',
        fontFamily: 'Inter, system-ui, sans-serif',
      }}
    >
      {/* Top Header & Order Type Switcher */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          borderBottom: '1px solid #334155',
          paddingBottom: '12px',
          marginBottom: '16px',
        }}
      >
        <div>
          <h2 style={{ margin: 0, fontSize: '1.25rem', color: '#38bdf8', display: 'flex', alignItems: 'center', gap: '10px' }}>
            <span>Order Processing</span>
            <span style={{ fontSize: '0.85rem', color: '#94a3b8', fontWeight: 'normal' }}>
              ({activeCompany?.name || 'OpenTally'})
            </span>
          </h2>
          <div style={{ fontSize: '0.8rem', color: '#94a3b8', marginTop: '4px' }}>
            Active Mode: <strong style={{ color: orderType === 'SalesOrder' ? '#10b981' : '#f59e0b' }}>{orderType === 'SalesOrder' ? 'Sales Orders (Ctrl+F8)' : 'Purchase Orders (Ctrl+F9)'}</strong> | Strict Non-GL / Non-Stock Pre-Billing Commitment
          </div>
        </div>

        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            onClick={() => {
              setOrderType('SalesOrder');
              setSelectedOrder(null);
            }}
            style={{
              padding: '6px 14px',
              borderRadius: '4px',
              border: 'none',
              background: orderType === 'SalesOrder' ? '#10b981' : '#1e293b',
              color: '#ffffff',
              fontWeight: 'bold',
              cursor: 'pointer',
              fontSize: '0.85rem',
            }}
          >
            Sales Orders [Ctrl+F8]
          </button>
          <button
            onClick={() => {
              setOrderType('PurchaseOrder');
              setSelectedOrder(null);
            }}
            style={{
              padding: '6px 14px',
              borderRadius: '4px',
              border: 'none',
              background: orderType === 'PurchaseOrder' ? '#f59e0b' : '#1e293b',
              color: '#ffffff',
              fontWeight: 'bold',
              cursor: 'pointer',
              fontSize: '0.85rem',
            }}
          >
            Purchase Orders [Ctrl+F9]
          </button>
          <button
            onClick={onBack}
            style={{
              padding: '6px 14px',
              borderRadius: '4px',
              border: '1px solid #475569',
              background: '#1e293b',
              color: '#cbd5e1',
              cursor: 'pointer',
              fontSize: '0.85rem',
            }}
          >
            Back (Esc)
          </button>
        </div>
      </div>

      {/* Messages */}
      {error && (
        <div style={{ background: '#7f1d1d', color: '#fecaca', padding: '10px', borderRadius: '4px', marginBottom: '12px' }}>
          <strong>Error:</strong> {error}
        </div>
      )}
      {successMsg && (
        <div style={{ background: '#064e3b', color: '#a7f3d0', padding: '10px', borderRadius: '4px', marginBottom: '12px' }}>
          {successMsg}
        </div>
      )}

      {/* Navigation Sub-Tabs */}
      <div style={{ display: 'flex', gap: '10px', marginBottom: '16px', borderBottom: '1px solid #334155' }}>
        <button
          onClick={() => {
            setActiveTab('list');
            setSelectedOrder(null);
          }}
          style={{
            padding: '8px 16px',
            background: 'none',
            border: 'none',
            borderBottom: activeTab === 'list' ? '2px solid #38bdf8' : 'none',
            color: activeTab === 'list' ? '#38bdf8' : '#94a3b8',
            fontWeight: 'bold',
            cursor: 'pointer',
          }}
        >
          Orders List
        </button>
        <button
          onClick={() => {
            setActiveTab('create');
            setSelectedOrder(null);
          }}
          style={{
            padding: '8px 16px',
            background: 'none',
            border: 'none',
            borderBottom: activeTab === 'create' ? '2px solid #38bdf8' : 'none',
            color: activeTab === 'create' ? '#38bdf8' : '#94a3b8',
            fontWeight: 'bold',
            cursor: 'pointer',
          }}
        >
          + Create New Order
        </button>
        <button
          onClick={() => {
            setActiveTab('pending');
            setSelectedOrder(null);
          }}
          style={{
            padding: '8px 16px',
            background: 'none',
            border: 'none',
            borderBottom: activeTab === 'pending' ? '2px solid #38bdf8' : 'none',
            color: activeTab === 'pending' ? '#38bdf8' : '#94a3b8',
            fontWeight: 'bold',
            cursor: 'pointer',
          }}
        >
          Pending Orders Report (Outstanding)
        </button>
      </div>

      {loading && <div style={{ color: '#38bdf8', padding: '12px' }}>Processing...</div>}

      {/* TAB 1: ORDERS LIST */}
      {activeTab === 'list' && !selectedOrder && (
        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
            <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
              <input
                type="text"
                placeholder="Search order number or party..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                style={{
                  background: '#1e293b',
                  border: '1px solid #475569',
                  color: '#f8fafc',
                  padding: '6px 12px',
                  borderRadius: '4px',
                  width: '280px',
                }}
              />
              <label style={{ fontSize: '0.85rem', color: '#94a3b8' }}>Status:</label>
              <select
                value={statusFilter}
                onChange={(e) => setStatusFilter(e.target.value)}
                style={{
                  background: '#1e293b',
                  border: '1px solid #475569',
                  color: '#f8fafc',
                  padding: '6px 10px',
                  borderRadius: '4px',
                }}
              >
                <option value="All">All</option>
                <option value="Open">Open</option>
                <option value="PartiallyFulfilled">Partially Fulfilled</option>
                <option value="Fulfilled">Fulfilled</option>
                <option value="Cancelled">Cancelled</option>
              </select>
            </div>
            <div style={{ fontSize: '0.85rem', color: '#94a3b8' }}>
              Showing {filteredOrders.length} order(s)
            </div>
          </div>

          <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '0.85rem' }}>
            <thead>
              <tr style={{ background: '#1e293b', borderBottom: '1px solid #334155', color: '#94a3b8' }}>
                <th style={{ padding: '8px' }}>Order #</th>
                <th style={{ padding: '8px' }}>Date</th>
                <th style={{ padding: '8px' }}>Due Date</th>
                <th style={{ padding: '8px' }}>Party Ledger</th>
                <th style={{ padding: '8px' }}>Reference</th>
                <th style={{ padding: '8px' }}>Status</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Total Amount</th>
                <th style={{ padding: '8px', textAlign: 'center' }}>Action</th>
              </tr>
            </thead>
            <tbody>
              {filteredOrders.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ padding: '24px', textAlign: 'center', color: '#64748b' }}>
                    No orders found. Click "+ Create New Order" to create one.
                  </td>
                </tr>
              ) : (
                filteredOrders.map((o) => (
                  <tr
                    key={o.id}
                    style={{ borderBottom: '1px solid #1e293b', cursor: 'pointer' }}
                    onClick={() => handleSelectOrder(o.id)}
                  >
                    <td style={{ padding: '8px', color: '#38bdf8', fontWeight: 'bold' }}>{o.order_number}</td>
                    <td style={{ padding: '8px' }}>{o.order_date}</td>
                    <td style={{ padding: '8px' }}>{o.due_date || '-'}</td>
                    <td style={{ padding: '8px' }}>{o.party_ledger_name || o.party_ledger_id}</td>
                    <td style={{ padding: '8px' }}>{o.reference_no || '-'}</td>
                    <td style={{ padding: '8px' }}>
                      <span
                        style={{
                          padding: '2px 8px',
                          borderRadius: '4px',
                          fontSize: '0.75rem',
                          background:
                            o.status === 'Fulfilled'
                              ? '#064e3b'
                              : o.status === 'PartiallyFulfilled'
                              ? '#1e3a8a'
                              : o.status === 'Cancelled'
                              ? '#7f1d1d'
                              : '#854d0e',
                          color: '#ffffff',
                        }}
                      >
                        {o.status}
                      </span>
                    </td>
                    <td style={{ padding: '8px', textAlign: 'right', fontWeight: 'bold' }}>
                      ₹{parseFloat(o.total_amount).toFixed(2)}
                    </td>
                    <td style={{ padding: '8px', textAlign: 'center' }}>
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSelectOrder(o.id);
                        }}
                        style={{
                          padding: '3px 8px',
                          background: '#334155',
                          border: 'none',
                          borderRadius: '3px',
                          color: '#f8fafc',
                          fontSize: '0.75rem',
                          cursor: 'pointer',
                        }}
                      >
                        View / Convert
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* SELECTED ORDER DETAIL VIEW */}
      {selectedOrder && (
        <div style={{ background: '#1e293b', padding: '16px', borderRadius: '6px', border: '1px solid #334155' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '16px' }}>
            <div>
              <h3 style={{ margin: 0, color: '#38bdf8', fontSize: '1.15rem' }}>
                Order #{selectedOrder.order.order_number} ({selectedOrder.order.status})
              </h3>
              <div style={{ fontSize: '0.85rem', color: '#94a3b8', marginTop: '4px' }}>
                Party: <strong style={{ color: '#f8fafc' }}>{selectedOrder.order.party_ledger_name}</strong> | Date: {selectedOrder.order.order_date} {selectedOrder.order.due_date ? `| Due: ${selectedOrder.order.due_date}` : ''}
              </div>
              {selectedOrder.order.reference_no && (
                <div style={{ fontSize: '0.8rem', color: '#94a3b8' }}>
                  Ref: {selectedOrder.order.reference_no}
                </div>
              )}
            </div>

            <div style={{ display: 'flex', gap: '8px' }}>
              {selectedOrder.order.status !== 'Fulfilled' && selectedOrder.order.status !== 'Cancelled' && (
                <>
                  <button
                    onClick={handleOpenConvertModal}
                    style={{
                      padding: '6px 14px',
                      background: '#10b981',
                      border: 'none',
                      borderRadius: '4px',
                      color: '#ffffff',
                      fontWeight: 'bold',
                      cursor: 'pointer',
                      fontSize: '0.85rem',
                    }}
                  >
                    {orderType === 'PurchaseOrder' ? '[Convert to Invoice (F9)]' : '[Convert to Invoice (F8)]'}
                  </button>
                  <button
                    onClick={() => handleCancelOrder(selectedOrder.order.id)}
                    style={{
                      padding: '6px 12px',
                      background: '#7f1d1d',
                      border: 'none',
                      borderRadius: '4px',
                      color: '#fecaca',
                      cursor: 'pointer',
                      fontSize: '0.85rem',
                    }}
                  >
                    Cancel Order
                  </button>
                </>
              )}
              <button
                onClick={() => setSelectedOrder(null)}
                style={{
                  padding: '6px 12px',
                  background: '#334155',
                  border: 'none',
                  borderRadius: '4px',
                  color: '#cbd5e1',
                  cursor: 'pointer',
                  fontSize: '0.85rem',
                }}
              >
                Close View (Esc)
              </button>
            </div>
          </div>

          <h4 style={{ margin: '0 0 8px 0', color: '#94a3b8', fontSize: '0.9rem' }}>Line Items & Fulfillment Status</h4>
          <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '0.85rem' }}>
            <thead>
              <tr style={{ background: '#0f172a', borderBottom: '1px solid #334155', color: '#94a3b8' }}>
                <th style={{ padding: '8px' }}>Item</th>
                <th style={{ padding: '8px' }}>Godown</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Ordered</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Fulfilled</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Pending</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Rate</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Line Amount</th>
              </tr>
            </thead>
            <tbody>
              {selectedOrder.lines.map((l) => (
                <tr key={l.id} style={{ borderBottom: '1px solid #334155' }}>
                  <td style={{ padding: '8px', fontWeight: 'bold' }}>{l.stock_item_name || l.stock_item_id}</td>
                  <td style={{ padding: '8px' }}>{l.godown_name || '-'}</td>
                  <td style={{ padding: '8px', textAlign: 'right' }}>{l.ordered_qty}</td>
                  <td style={{ padding: '8px', textAlign: 'right', color: '#10b981' }}>{l.fulfilled_qty}</td>
                  <td style={{ padding: '8px', textAlign: 'right', color: '#f59e0b', fontWeight: 'bold' }}>{l.pending_qty}</td>
                  <td style={{ padding: '8px', textAlign: 'right' }}>₹{parseFloat(l.unit_rate).toFixed(2)}</td>
                  <td style={{ padding: '8px', textAlign: 'right', fontWeight: 'bold' }}>₹{parseFloat(l.line_amount).toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div style={{ marginTop: '12px', textAlign: 'right', fontSize: '1rem', fontWeight: 'bold', color: '#38bdf8' }}>
            Order Total: ₹{parseFloat(selectedOrder.order.total_amount).toFixed(2)}
          </div>
        </div>
      )}

      {/* CONVERT TO INVOICE MODAL */}
      {isConvertModalOpen && selectedOrder && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: 'rgba(0, 0, 0, 0.75)',
            display: 'flex',
            justifyContent: 'center',
            alignItems: 'center',
            zIndex: 1000,
          }}
        >
          <div
            style={{
              background: '#1e293b',
              padding: '24px',
              borderRadius: '8px',
              border: '1px solid #475569',
              width: '650px',
              maxWidth: '90%',
              maxHeight: '90vh',
              overflowY: 'auto',
            }}
          >
            <h3 style={{ margin: '0 0 16px 0', color: '#38bdf8' }}>
              Convert Order #{selectedOrder.order.order_number} to {orderType === 'PurchaseOrder' ? 'Purchase Invoice (F9)' : 'Sales Invoice (F8)'}
            </h3>

            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '12px', marginBottom: '16px' }}>
              <div>
                <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Invoice Date:</label>
                <input
                  type="date"
                  value={invoiceDate}
                  onChange={(e) => setInvoiceDate(e.target.value)}
                  style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
                />
              </div>
              <div>
                <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Voucher Series:</label>
                <select
                  value={seriesId}
                  onChange={(e) => setSeriesId(e.target.value)}
                  style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
                >
                  {seriesList.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.series_name || s.name} ({s.series_code || s.code})
                    </option>
                  ))}
                </select>
              </div>
            </div>

            <h4 style={{ margin: '0 0 8px 0', fontSize: '0.85rem', color: '#cbd5e1' }}>
              Enter Fulfill Quantities (Supports Partial Quantities):
            </h4>
            <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '0.85rem', marginBottom: '16px' }}>
              <thead>
                <tr style={{ background: '#0f172a', borderBottom: '1px solid #334155', color: '#94a3b8' }}>
                  <th style={{ padding: '6px' }}>Item</th>
                  <th style={{ padding: '6px', textAlign: 'right' }}>Pending Qty</th>
                  <th style={{ padding: '6px', textAlign: 'right' }}>Fulfill Qty</th>
                </tr>
              </thead>
              <tbody>
                {selectedOrder.lines.map((l) => (
                  <tr key={l.id} style={{ borderBottom: '1px solid #334155' }}>
                    <td style={{ padding: '6px' }}>{l.stock_item_name || l.stock_item_id}</td>
                    <td style={{ padding: '6px', textAlign: 'right', color: '#f59e0b' }}>{l.pending_qty}</td>
                    <td style={{ padding: '6px', textAlign: 'right' }}>
                      <input
                        type="number"
                        min="0"
                        max={l.pending_qty}
                        step="0.01"
                        value={convertQuantities[l.id] || ''}
                        onChange={(e) =>
                          setConvertQuantities({ ...convertQuantities, [l.id]: e.target.value })
                        }
                        style={{
                          width: '100px',
                          background: '#0f172a',
                          border: '1px solid #475569',
                          color: '#f8fafc',
                          padding: '4px 6px',
                          textAlign: 'right',
                          borderRadius: '4px',
                        }}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
              <button
                onClick={() => setIsConvertModalOpen(false)}
                style={{ padding: '8px 16px', background: '#334155', border: 'none', borderRadius: '4px', color: '#cbd5e1', cursor: 'pointer' }}
              >
                Cancel (Esc)
              </button>
              <button
                onClick={handleExecuteConvert}
                style={{ padding: '8px 16px', background: '#10b981', border: 'none', borderRadius: '4px', color: '#ffffff', fontWeight: 'bold', cursor: 'pointer' }}
              >
                Confirm & Post to Invoice (GL + FIFO + GST)
              </button>
            </div>
          </div>
        </div>
      )}

      {/* TAB 2: CREATE ORDER FORM */}
      {activeTab === 'create' && (
        <form onSubmit={handleCreateOrder} style={{ background: '#1e293b', padding: '16px', borderRadius: '6px', border: '1px solid #334155' }}>
          <h3 style={{ margin: '0 0 16px 0', color: '#38bdf8' }}>
            New {orderType === 'PurchaseOrder' ? 'Purchase Order (Ctrl+F9)' : 'Sales Order (Ctrl+F8)'}
          </h3>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '12px', marginBottom: '16px' }}>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Order Number *</label>
              <input
                type="text"
                required
                placeholder={orderType === 'PurchaseOrder' ? 'PO-001' : 'SO-001'}
                value={orderNumber}
                onChange={(e) => setOrderNumber(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Order Date *</label>
              <input
                type="date"
                required
                value={orderDate}
                onChange={(e) => setOrderDate(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Due Date</label>
              <input
                type="date"
                value={dueDate}
                onChange={(e) => setDueDate(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Party Ledger *</label>
              <select
                required
                value={partyLedgerId}
                onChange={(e) => setPartyLedgerId(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              >
                <option value="">-- Select Party --</option>
                {ledgers.map((l) => (
                  <option key={l.id} value={l.id}>
                    {l.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Reference No</label>
              <input
                type="text"
                placeholder="Optional ref"
                value={referenceNo}
                onChange={(e) => setReferenceNo(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              />
            </div>
            <div>
              <label style={{ display: 'block', fontSize: '0.8rem', color: '#94a3b8', marginBottom: '4px' }}>Narration</label>
              <input
                type="text"
                placeholder="Optional remarks"
                value={narration}
                onChange={(e) => setNarration(e.target.value)}
                style={{ width: '100%', background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px 8px', borderRadius: '4px' }}
              />
            </div>
          </div>

          <h4 style={{ margin: '16px 0 8px 0', color: '#cbd5e1', fontSize: '0.9rem' }}>Line Items</h4>
          {lines.map((line, idx) => (
            <div
              key={idx}
              style={{
                display: 'grid',
                gridTemplateColumns: '2fr 1.5fr 1fr 1fr 1fr 1fr auto',
                gap: '8px',
                marginBottom: '8px',
                alignItems: 'center',
              }}
            >
              <select
                required
                value={line.stock_item_id}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].stock_item_id = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              >
                <option value="">-- Stock Item --</option>
                {stockItems.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                  </option>
                ))}
              </select>

              <select
                value={line.godown_id}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].godown_id = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              >
                <option value="">-- Main Godown --</option>
                {godowns.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name}
                  </option>
                ))}
              </select>

              <input
                type="number"
                min="0.01"
                step="0.01"
                placeholder="Qty"
                required
                value={line.ordered_qty}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].ordered_qty = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              />

              <input
                type="number"
                min="0.01"
                step="0.01"
                placeholder="Rate"
                required
                value={line.unit_rate}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].unit_rate = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              />

              <select
                value={line.discount_kind}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].discount_kind = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              >
                <option value="">No Disc</option>
                <option value="Percentage">%</option>
                <option value="FixedAmount">Fixed</option>
              </select>

              <input
                type="number"
                min="0"
                step="0.01"
                placeholder="Disc Val"
                value={line.discount_value}
                onChange={(e) => {
                  const newLines = [...lines];
                  newLines[idx].discount_value = e.target.value;
                  setLines(newLines);
                }}
                style={{ background: '#0f172a', border: '1px solid #475569', color: '#f8fafc', padding: '6px', borderRadius: '4px' }}
              />

              {lines.length > 1 && (
                <button
                  type="button"
                  onClick={() => setLines(lines.filter((_, i) => i !== idx))}
                  style={{ background: '#7f1d1d', border: 'none', color: '#fecaca', padding: '6px 10px', borderRadius: '4px', cursor: 'pointer' }}
                >
                  ✕
                </button>
              )}
            </div>
          ))}

          <button
            type="button"
            onClick={() =>
              setLines([
                ...lines,
                {
                  stock_item_id: '',
                  godown_id: '',
                  ordered_qty: '1',
                  unit_rate: '100.00',
                  discount_kind: '',
                  discount_value: '',
                },
              ])
            }
            style={{
              padding: '6px 12px',
              background: '#334155',
              border: 'none',
              borderRadius: '4px',
              color: '#cbd5e1',
              cursor: 'pointer',
              fontSize: '0.85rem',
              marginTop: '4px',
            }}
          >
            + Add Line
          </button>

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '20px' }}>
            <button
              type="button"
              onClick={() => setActiveTab('list')}
              style={{ padding: '8px 16px', background: '#334155', border: 'none', borderRadius: '4px', color: '#cbd5e1', cursor: 'pointer' }}
            >
              Cancel
            </button>
            <button
              type="submit"
              style={{
                padding: '8px 18px',
                background: orderType === 'PurchaseOrder' ? '#f59e0b' : '#10b981',
                border: 'none',
                borderRadius: '4px',
                color: '#ffffff',
                fontWeight: 'bold',
                cursor: 'pointer',
              }}
            >
              Create {orderType === 'PurchaseOrder' ? 'Purchase Order' : 'Sales Order'}
            </button>
          </div>
        </form>
      )}

      {/* TAB 3: PENDING ORDERS REPORT */}
      {activeTab === 'pending' && (
        <div>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
            <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
              <label style={{ fontSize: '0.85rem', color: '#94a3b8' }}>As of Date:</label>
              <input
                type="date"
                value={asOfDate}
                onChange={(e) => setAsOfDate(e.target.value)}
                style={{
                  background: '#1e293b',
                  border: '1px solid #475569',
                  color: '#f8fafc',
                  padding: '6px 10px',
                  borderRadius: '4px',
                }}
              />
              <button
                onClick={loadPendingReport}
                style={{
                  padding: '6px 12px',
                  background: '#38bdf8',
                  border: 'none',
                  borderRadius: '4px',
                  color: '#0f172a',
                  fontWeight: 'bold',
                  cursor: 'pointer',
                }}
              >
                Refresh
              </button>
            </div>

            {pendingReport && (
              <div style={{ display: 'flex', gap: '16px', fontSize: '0.9rem' }}>
                <div>
                  Total Pending Qty: <strong style={{ color: '#f59e0b' }}>{pendingReport.total_pending_qty}</strong>
                </div>
                <div>
                  Total Pending Value: <strong style={{ color: '#38bdf8' }}>₹{parseFloat(pendingReport.total_pending_amount).toFixed(2)}</strong>
                </div>
              </div>
            )}
          </div>

          <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '0.85rem' }}>
            <thead>
              <tr style={{ background: '#1e293b', borderBottom: '1px solid #334155', color: '#94a3b8' }}>
                <th style={{ padding: '8px' }}>Item Name</th>
                <th style={{ padding: '8px' }}>Order #</th>
                <th style={{ padding: '8px' }}>Order Date</th>
                <th style={{ padding: '8px' }}>Due Date</th>
                <th style={{ padding: '8px' }}>Party Name</th>
                <th style={{ padding: '8px' }}>Godown</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Ordered</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Fulfilled</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Pending Qty</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Rate</th>
                <th style={{ padding: '8px', textAlign: 'right' }}>Pending Value</th>
                <th style={{ padding: '8px', textAlign: 'center' }}>Overdue</th>
              </tr>
            </thead>
            <tbody>
              {!pendingReport || pendingReport.items.length === 0 ? (
                <tr>
                  <td colSpan={12} style={{ padding: '24px', textAlign: 'center', color: '#64748b' }}>
                    No pending orders outstanding as of {asOfDate}. All orders fulfilled!
                  </td>
                </tr>
              ) : (
                pendingReport.items.map((item, idx) => (
                  <tr key={idx} style={{ borderBottom: '1px solid #1e293b' }}>
                    <td style={{ padding: '8px', fontWeight: 'bold', color: '#f8fafc' }}>{item.stock_item_name}</td>
                    <td style={{ padding: '8px', color: '#38bdf8' }}>{item.order_number}</td>
                    <td style={{ padding: '8px' }}>{item.order_date}</td>
                    <td style={{ padding: '8px' }}>{item.due_date || '-'}</td>
                    <td style={{ padding: '8px' }}>{item.party_name}</td>
                    <td style={{ padding: '8px' }}>{item.godown_name || '-'}</td>
                    <td style={{ padding: '8px', textAlign: 'right' }}>{item.ordered_qty}</td>
                    <td style={{ padding: '8px', textAlign: 'right', color: '#10b981' }}>{item.fulfilled_qty}</td>
                    <td style={{ padding: '8px', textAlign: 'right', color: '#f59e0b', fontWeight: 'bold' }}>
                      {item.pending_qty}
                    </td>
                    <td style={{ padding: '8px', textAlign: 'right' }}>₹{parseFloat(item.unit_rate).toFixed(2)}</td>
                    <td style={{ padding: '8px', textAlign: 'right', fontWeight: 'bold' }}>
                      ₹{parseFloat(item.pending_amount).toFixed(2)}
                    </td>
                    <td style={{ padding: '8px', textAlign: 'center' }}>
                      {item.is_overdue ? (
                        <span style={{ padding: '2px 6px', borderRadius: '4px', background: '#7f1d1d', color: '#fecaca', fontSize: '0.75rem', fontWeight: 'bold' }}>
                          OVERDUE
                        </span>
                      ) : (
                        <span style={{ color: '#64748b' }}>-</span>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
