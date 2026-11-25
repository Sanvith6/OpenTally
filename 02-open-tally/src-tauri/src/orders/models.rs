use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Open,
    PartiallyFulfilled,
    Fulfilled,
    Cancelled,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::PartiallyFulfilled => "PartiallyFulfilled",
            Self::Fulfilled => "Fulfilled",
            Self::Cancelled => "Cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    PurchaseOrder,
    SalesOrder,
}

impl OrderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PurchaseOrder => "PurchaseOrder",
            Self::SalesOrder => "SalesOrder",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrder {
    pub id: String,
    pub company_id: String,
    pub order_number: String,
    pub order_date: String,
    pub due_date: Option<String>,
    pub party_ledger_id: String,
    pub party_ledger_name: Option<String>,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub status: String,
    pub total_amount: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderLine {
    pub id: String,
    pub company_id: String,
    pub order_id: String,
    pub stock_item_id: String,
    pub stock_item_name: Option<String>,
    pub godown_id: Option<String>,
    pub godown_name: Option<String>,
    pub ordered_qty: String,
    pub fulfilled_qty: String,
    pub pending_qty: String,
    pub unit_rate: String,
    pub discount_kind: Option<String>,
    pub discount_value: Option<String>,
    pub line_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderWithLines {
    pub order: PurchaseOrder,
    pub lines: Vec<PurchaseOrderLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrder {
    pub id: String,
    pub company_id: String,
    pub order_number: String,
    pub order_date: String,
    pub due_date: Option<String>,
    pub party_ledger_id: String,
    pub party_ledger_name: Option<String>,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub status: String,
    pub total_amount: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrderLine {
    pub id: String,
    pub company_id: String,
    pub order_id: String,
    pub stock_item_id: String,
    pub stock_item_name: Option<String>,
    pub godown_id: Option<String>,
    pub godown_name: Option<String>,
    pub ordered_qty: String,
    pub fulfilled_qty: String,
    pub pending_qty: String,
    pub unit_rate: String,
    pub discount_kind: Option<String>,
    pub discount_value: Option<String>,
    pub line_amount: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesOrderWithLines {
    pub order: SalesOrder,
    pub lines: Vec<SalesOrderLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderLineInput {
    pub stock_item_id: String,
    pub godown_id: Option<String>,
    pub ordered_qty: Decimal,
    pub unit_rate: Decimal,
    pub discount_kind: Option<String>,
    pub discount_value: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePurchaseOrderInput {
    pub order_number: String,
    pub order_date: String,
    pub due_date: Option<String>,
    pub party_ledger_id: String,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub lines: Vec<CreateOrderLineInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSalesOrderInput {
    pub order_number: String,
    pub order_date: String,
    pub due_date: Option<String>,
    pub party_ledger_id: String,
    pub reference_no: Option<String>,
    pub narration: Option<String>,
    pub lines: Vec<CreateOrderLineInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillOrderLineInput {
    pub order_line_id: String,
    pub fulfill_qty: Decimal,
    pub unit_rate: Option<Decimal>,
    pub godown_id: Option<String>,
    pub batch_number: Option<String>,
    pub mfg_date: Option<String>,
    pub expiry_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertPurchaseOrderInput {
    pub order_id: String,
    pub invoice_date: String,
    pub series_id: String,
    pub commercial_account_ledger_id: Option<String>,
    pub lines: Vec<FulfillOrderLineInput>,
    pub tax_lines: Option<Vec<crate::commercial::TaxPostingLine>>,
    pub narration: Option<String>,
    pub reference_no: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertSalesOrderInput {
    pub order_id: String,
    pub invoice_date: String,
    pub series_id: String,
    pub commercial_account_ledger_id: Option<String>,
    pub lines: Vec<FulfillOrderLineInput>,
    pub tax_lines: Option<Vec<crate::commercial::TaxPostingLine>>,
    pub narration: Option<String>,
    pub reference_no: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFulfillmentRecord {
    pub id: String,
    pub company_id: String,
    pub order_type: String,
    pub order_id: String,
    pub order_line_id: String,
    pub voucher_id: String,
    pub commercial_doc_id: Option<String>,
    pub fulfilled_qty: String,
    pub fulfillment_date: String,
    pub is_cancelled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertOrderResult {
    pub voucher_id: String,
    pub voucher_number: String,
    pub order_status: String,
    pub fulfillments: Vec<OrderFulfillmentRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOrderItem {
    pub order_id: String,
    pub order_number: String,
    pub order_date: String,
    pub due_date: Option<String>,
    pub party_ledger_id: String,
    pub party_name: String,
    pub stock_item_id: String,
    pub stock_item_name: String,
    pub godown_id: Option<String>,
    pub godown_name: Option<String>,
    pub ordered_qty: String,
    pub fulfilled_qty: String,
    pub pending_qty: String,
    pub unit_rate: String,
    pub pending_amount: String,
    pub is_overdue: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingOrdersReport {
    pub company_id: String,
    pub order_type: String,
    pub as_of_date: String,
    pub items: Vec<PendingOrderItem>,
    pub total_pending_qty: String,
    pub total_pending_amount: String,
}
