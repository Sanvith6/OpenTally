import type { VoucherWithLines } from './voucher';
export type { VoucherWithLines } from './voucher';

export interface DatabaseHealth {
  is_connected: boolean;
  engine: string;
  journal_mode: string;
  foreign_keys_enabled: boolean;
  schema_version: string;
  encryption_enabled: boolean;
  encryption_status: string;
  key_management: string;
}

export interface SecurityStatus {
  encryption_active: boolean;
  encryption_engine: string;
  key_management_status: string;
  csp_enforced: boolean;
}

export interface SubsystemStatus {
  name: string;
  status: string;
  details: string;
}

export interface HealthResponse {
  app_name: string;
  app_version: string;
  status: string;
  timestamp: string;
  environment: string;
  uptime_seconds: number;
  database: DatabaseHealth;
  security: SecurityStatus;
  subsystems: SubsystemStatus[];
}

export type CompanyStatus = 'active' | 'archived' | 'closed';

export interface Company {
  id: string;
  name: string;
  mailing_name: string;
  country: string;
  state: string;
  financial_year_begins: string;
  books_beginning_from: string;
  currency_symbol: string;
  currency_formal_name: string;
  currency_decimal_places: number;
  status: CompanyStatus;
  created_at: string;
  updated_at: string;
}

export interface CompanySettings {
  company_id: string;
  key: string;
  value: string;
  updated_at: string;
}

export interface CreateCompanyInput {
  name: string;
  mailing_name?: string;
  country?: string;
  state?: string;
  financial_year_begins: string;
  books_beginning_from?: string;
  currency_symbol?: string;
  currency_formal_name?: string;
  currency_decimal_places?: number;
}

export interface UpdateCompanyInput {
  mailing_name?: string;
  country?: string;
  state?: string;
  currency_symbol?: string;
  currency_formal_name?: string;
}

// ---------------------------------------------------------------------------
// Chart of Accounts Types (Stage 3)
// ---------------------------------------------------------------------------

export type BalanceType = 'Dr' | 'Cr';

export interface AccountGroup {
  id: string;
  company_id: string;
  name: string;
  parent_id: string | null;
  nature: string;
  is_primary: boolean;
  is_predefined: boolean;
  is_hidden: boolean;
  affects_gross_profit: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateAccountGroupInput {
  name: string;
  parent_id: string;
}

export interface UpdateAccountGroupInput {
  name: string;
  parent_id?: string;
}

export interface Ledger {
  id: string;
  company_id: string;
  group_id: string;
  name: string;
  ledger_classification: string;
  is_reserved: boolean;
  description?: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateLedgerInput {
  name: string;
  group_id: string;
  description?: string;
  opening_balance?: SetOpeningBalanceInput;
}

export interface UpdateLedgerInput {
  name: string;
  group_id?: string;
  description?: string;
}

export interface OpeningBalance {
  id: string;
  company_id: string;
  ledger_id: string;
  financial_year_begins: string;
  balance_type: BalanceType;
  amount: string;
  created_at: string;
  updated_at: string;
}

export interface SetOpeningBalanceInput {
  ledger_id: string;
  financial_year_begins?: string;
  balance_type: BalanceType;
  amount: string;
}

export interface OpeningBalanceEquilibrium {
  total_debit: string;
  total_credit: string;
  difference: string;
  difference_type?: BalanceType | null;
  is_balanced: boolean;
}

export interface OpeningBalanceSummary {
  balance_type: BalanceType;
  amount: string;
}

export interface ChartOfAccountsTreeNode {
  id: string;
  name: string;
  node_type: 'group' | 'ledger';
  nature: string;
  is_primary: boolean;
  is_predefined: boolean;
  is_reserved: boolean;
  parent_id?: string | null;
  opening_balance?: OpeningBalanceSummary | null;
  debit_total: string;
  credit_total: string;
  children: ChartOfAccountsTreeNode[];
}

export interface ChartOfAccountsTree {
  nodes: ChartOfAccountsTreeNode[];
  equilibrium: OpeningBalanceEquilibrium;
}

export * from './voucher';

export interface ReportDateRange {
  date_from: string;
  date_to: string;
}

export interface ReportFilter {
  period: ReportDateRange;
  ledger_id?: string | null;
}

export interface ReportPageRequest {
  page: number;
  page_size: number;
}

export interface DayBookRequest {
  filter: ReportFilter;
  voucher_type_id?: string | null;
  voucher_number_query?: string | null;
  page: ReportPageRequest;
}

export interface DayBookLine {
  ledger_id: string;
  ledger_name: string;
  debit: string;
  credit: string;
  narration?: string | null;
}

export interface DayBookEntry {
  voucher_id: string;
  date: string;
  voucher_number: string;
  voucher_type: string;
  narration?: string | null;
  debit_total: string;
  credit_total: string;
  voucher_total: string;
  lines: DayBookLine[];
}

export interface ReportPage<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}

export interface LedgerReportRequest {
  filter: ReportFilter;
  page: ReportPageRequest;
}

export interface LedgerReportTransaction {
  voucher_id: string;
  date: string;
  voucher_number: string;
  voucher_type: string;
  particulars?: string | null;
  debit: string;
  credit: string;
  running_balance: string;
  running_balance_type: string;
}

export interface LedgerReport {
  ledger_id: string;
  ledger_name: string;
  ledger_nature: string;
  opening_balance: string;
  opening_balance_type: string;
  transactions: ReportPage<LedgerReportTransaction>;
  closing_balance: string;
  closing_balance_type: string;
}

export interface TrialBalanceRequest {
  as_of_date: string;
  account_group_id?: string | null;
  ledger_id?: string | null;
  account_search?: string | null;
  include_zero_balances: boolean;
}

export interface TrialBalanceRow {
  ledger_id: string;
  ledger_name: string;
  account_group_id: string;
  account_group_name: string;
  account_group_nature: string;
  opening_debit: string;
  opening_credit: string;
  period_debit: string;
  period_credit: string;
  closing_debit: string;
  closing_credit: string;
}

export interface TrialBalanceReport {
  as_of_date: string;
  rows: TrialBalanceRow[];
  total_debit: string;
  total_credit: string;
  difference: string;
  ledger_count: number;
}

export interface ProfitAndLossRequest {
  period: ReportDateRange;
  account_group_id?: string | null;
  ledger_search?: string | null;
  include_zero_balances: boolean;
}

export interface ProfitAndLossLedgerRow {
  ledger_id: string;
  ledger_name: string;
  account_group_id: string;
  account_group_name: string;
  account_group_nature: string;
  is_direct: boolean;
  debit: string;
  credit: string;
  contribution: string;
}

export interface ProfitAndLossReport {
  company_id: string;
  date_from: string;
  date_to: string;
  direct_income_rows: ProfitAndLossLedgerRow[];
  direct_income_subtotal: string;
  direct_expense_rows: ProfitAndLossLedgerRow[];
  direct_expense_subtotal: string;
  gross_profit_or_loss: string;
  gross_result_type: string;
  indirect_income_rows: ProfitAndLossLedgerRow[];
  indirect_income_subtotal: string;
  indirect_expense_rows: ProfitAndLossLedgerRow[];
  indirect_expense_subtotal: string;
  net_profit_or_loss: string;
  net_result_type: string;
  ledger_count: number;
  warnings: string[];
}

export interface BalanceSheetRequest {
  as_of_date: string;
  account_group_id?: string | null;
  ledger_search?: string | null;
  include_zero_balances: boolean;
}

export interface BalanceSheetLedgerRow {
  ledger_id: string;
  ledger_name: string;
  account_group_id: string;
  account_group_name: string;
  classification: string;
  debit: string;
  credit: string;
  net_balance: string;
  balance_type: string;
  presentation_amount: string;
}

export interface BalanceSheetReport {
  company_id: string;
  as_of_date: string;
  asset_rows: BalanceSheetLedgerRow[];
  total_assets: string;
  liability_rows: BalanceSheetLedgerRow[];
  total_liabilities: string;
  equity_rows: BalanceSheetLedgerRow[];
  current_period_profit_or_loss: string;
  current_period_result_type: string;
  total_equity: string;
  total_liabilities_and_equity: string;
  balance_difference: string;
  is_balanced: boolean;
  ledger_count: number;
  warnings: string[];
}

export interface CashFlowRequest {
  period: ReportDateRange;
  cash_ledger_id?: string | null;
  activity_filter?: string | null;
}

export interface CashFlowRow {
  activity: string;
  ledger_id: string;
  ledger_name: string;
  voucher_id: string;
  voucher_number: string;
  date: string;
  description?: string | null;
  amount: string;
}

export interface CashFlowReport {
  company_id: string;
  date_from: string;
  date_to: string;
  opening_cash_balance: string;
  operating_rows: CashFlowRow[];
  operating_subtotal: string;
  investing_rows: CashFlowRow[];
  investing_subtotal: string;
  financing_rows: CashFlowRow[];
  financing_subtotal: string;
  net_change_in_cash: string;
  closing_cash_balance: string;
  actual_closing_cash_balance: string;
  reconciliation_difference: string;
  is_reconciled: boolean;
  ledger_count: number;
  warnings: string[];
}

export type ExportFormat = 'csv_summary' | 'csv_detailed' | 'json';
export interface VoucherSearchFilter {
  voucher_type_id?: string | null;
  ledger_id?: string | null;
  status?: string | null;
  date_from?: string | null;
  date_to?: string | null;
  search_query?: string | null;
  page?: number | null;
  page_size?: number | null;
}
export interface VoucherExportPreviewResult {
  format: ExportFormat;
  content: string;
  record_count: number;
  exported_at: string;
}

export interface StockGroup { id:string; company_id:string; name:string; parent_id:string|null; description:string|null; is_active:boolean; created_at:string; updated_at:string; }
export interface StockCategory { id:string; company_id:string; name:string; description:string|null; is_active:boolean; created_at:string; updated_at:string; }
export interface Unit { id:string; company_id:string; symbol:string; formal_name:string; unit_type:string; base_unit_id:string|null; secondary_unit_id:string|null; conversion_factor:string|null; decimal_precision:number; is_active:boolean; created_at:string; updated_at:string; }
export interface StockItem { id:string; company_id:string; name:string; stock_group_id:string; stock_category_id:string|null; base_unit_id:string; alternate_unit_id:string|null; sku:string|null; description:string|null; opening_quantity:string; opening_rate:string; opening_value:string; reorder_level:string; is_active:boolean; created_at:string; updated_at:string; }
export interface Godown { id:string; company_id:string; name:string; parent_id:string|null; description:string|null; address:string|null; is_external:boolean; is_active:boolean; created_at:string; updated_at:string; }
export interface StockGroupRequest { name:string; parent_id?:string|null; description?:string|null; }
export interface StockCategoryRequest { name:string; description?:string|null; }
export interface UnitRequest { symbol:string; formal_name:string; decimal_precision:number; unit_type?:string; base_unit_id?:string|null; secondary_unit_id?:string|null; conversion_factor?:string|null; }
export interface StockItemRequest { name:string; stock_group_id:string; stock_category_id?:string|null; base_unit_id:string; alternate_unit_id?:string|null; sku?:string|null; description?:string|null; opening_quantity:string; opening_rate:string; opening_value:string; reorder_level:string; }
export interface GodownRequest { name:string; parent_id?:string|null; description?:string|null; address?:string|null; is_external?:boolean; }
export interface StockVoucher { id:string; company_id:string; voucher_number:string; voucher_type:string; voucher_date:string; reference_no:string|null; narration:string|null; status:string; reversal_of_voucher_id:string|null; created_at:string; updated_at:string; }
export interface StockVoucherLine { id:string; company_id:string; voucher_id:string; line_number:number; stock_item_id:string; unit_id:string; godown_id:string|null; source_godown_id:string|null; destination_godown_id:string|null; quantity:string; system_quantity:string|null; physical_quantity:string|null; difference_quantity:string|null; description:string|null; created_at:string; }
export interface StockVoucherLineRequest { stock_item_id:string; unit_id:string; godown_id?:string|null; source_godown_id?:string|null; destination_godown_id?:string|null; quantity:string; physical_quantity?:string|null; description?:string|null; }
export interface StockVoucherRequest { voucher_type:string; voucher_date:string; reference_no?:string|null; narration?:string|null; lines:StockVoucherLineRequest[]; }
export interface StockVoucherListFilter { search?:string|null; voucher_type?:string|null; status?:string|null; date_from?:string|null; date_to?:string|null; }
export interface StockBalanceRequest { as_of_date:string; stock_item_id?:string|null; stock_group_id?:string|null; stock_category_id?:string|null; godown_id?:string|null; search?:string|null; include_zero:boolean; }
export interface StockBalanceRow { stock_item_id:string; item_name:string; stock_group_name:string; base_unit_symbol:string; godown_id:string|null; godown_name:string|null; opening_quantity:string; received_quantity:string; delivered_quantity:string; transfer_in_quantity:string; transfer_out_quantity:string; adjustment_quantity:string; closing_quantity:string; }
export interface ValuationConfig { company_id:string; valuation_method:'fifo'|'weighted_average'; }
export interface ValuationRequest { as_of_date:string; stock_item_id?:string|null; stock_group_id?:string|null; stock_category_id?:string|null; godown_id?:string|null; search?:string|null; include_zero:boolean; }
export interface ValuationRow { stock_item_id:string; item_name:string; godown_id:string|null; godown_name:string|null; quantity:string; valuation_method:string; unit_cost:string; total_value:string; }
export interface FifoLayer { id:string; stock_item_id:string; godown_id:string; source_voucher_id:string|null; source_line_id:string|null; receipt_date:string; original_quantity:string; remaining_quantity:string; unit_cost:string; original_value:string; }
export interface ValuationHistoryRow { movement_id:string; movement_date:string; movement_type:string; voucher_id:string; voucher_number:string; quantity:string; unit_cost:string; value:string; }
export interface InventoryReportFilter { date_from?:string|null; date_to?:string|null; as_of_date?:string|null; stock_item_id?:string|null; stock_group_id?:string|null; stock_category_id?:string|null; godown_id?:string|null; voucher_type?:string|null; search?:string|null; include_zero:boolean; page?:number|null; page_size?:number|null; }
export interface StockSummaryReportRow { stock_item_id:string; item_name:string; stock_group_name:string; base_unit_symbol:string; opening_quantity:string; received_quantity:string; delivered_quantity:string; transfer_in_quantity:string; transfer_out_quantity:string; adjustment_quantity:string; closing_quantity:string; valuation_method:string; unit_cost:string; total_stock_value:string; warning:string|null; }
export interface StockSummaryReport { as_of_date:string; rows:StockSummaryReportRow[]; }
export interface StockRegisterRow { movement_id:string; movement_date:string; voucher_number:string; voucher_type:string; stock_item_id:string; item_name:string; godown_id:string; godown_name:string; direction:string; quantity:string; base_unit_symbol:string; source_reference:string|null; voucher_status:string; reversal_of_voucher_id:string|null; signed_quantity:string; }
export interface StockRegisterReport { total:number; page:number; page_size:number; rows:StockRegisterRow[]; }
export interface StockLedgerRow { movement_id:string; movement_date:string; voucher_number:string; movement_type:string; signed_quantity:string; running_balance:string; }
export interface StockLedgerReport { stock_item_id:string; godown_id:string|null; opening_quantity:string; closing_quantity:string; rows:StockLedgerRow[]; }
export interface ReconciliationFinding { check_id:string; company_id:string; stock_item_id:string|null; godown_id:string|null; expected_value:string; actual_value:string; difference:string; severity:string; explanation:string; }
export interface InventoryAccountMapping { company_id:string; inventory_asset_ledger_id:string; adjustment_expense_ledger_id:string; adjustment_gain_ledger_id:string|null; cogs_ledger_id:string; receipt_offset_ledger_id:string; journal_series_id:string; enabled:boolean; }
export interface InventoryAccountMappingRequest { inventory_asset_ledger_id:string; adjustment_expense_ledger_id:string; adjustment_gain_ledger_id?:string|null; cogs_ledger_id:string; receipt_offset_ledger_id:string; journal_series_id:string; enabled:boolean; }
export interface AccountingImpactLine { ledger_id:string; entry_type:string; amount:string; narration:string; }
export interface InventoryAccountingPreview { stock_voucher:StockVoucher; valuation_method:string; amount:string; lines:AccountingImpactLine[]; total_debit:string; total_credit:string; warning:string|null; }
export interface InventoryAccountingIntegration { id:string; company_id:string; stock_voucher_id:string; stock_voucher_type:string; accounting_voucher_id:string|null; status:string; valuation_method:string|null; integrated_amount:string; attempt_count:number; error_details:string|null; created_at:string; updated_at:string; }
export interface InventoryValuationAttribution { id:string; company_id:string; stock_voucher_id:string; voucher_type:string; voucher_line_id:string; stock_item_id:string; stock_movement_id:string; valuation_method:string; quantity:string; unit_cost:string; total_value:string; context:string; posting_timestamp:string; reversal_of_attribution_id:string|null; }
export interface InventoryValuationLayerUse { id:string; attribution_id:string; source_movement_id:string|null; source_voucher_id:string|null; quantity:string; unit_cost:string; total_value:string; }
export interface InventoryValuationAttributionDetails { attribution:InventoryValuationAttribution; layer_uses:InventoryValuationLayerUse[]; }
export interface PartyProfile { id:string; company_id:string; ledger_id:string; party_code:string; display_name:string; customer_enabled:boolean; supplier_enabled:boolean; contact_person:string|null; email:string|null; phone:string|null; billing_address:string|null; shipping_address:string|null; notes:string|null; credit_period_days:number; credit_limit:string; is_active:boolean; created_at:string; updated_at:string; }
export interface PartyRequest { ledger_id:string; party_code:string; display_name:string; customer_enabled:boolean; supplier_enabled:boolean; contact_person?:string|null; email?:string|null; phone?:string|null; billing_address?:string|null; shipping_address?:string|null; notes?:string|null; credit_period_days:number; credit_limit:string; }
export type DiscountKind = 'none' | 'amount' | 'percentage';
export interface SalesLineRequest { description:string; stock_item_id?:string|null; unit_id?:string|null; godown_id?:string|null; quantity:string; billed_quantity?:string|null; rate:string; discount_kind:DiscountKind; discount_value:string; }
export interface SalesDocumentRequest { series_id:string; party_id:string; invoice_date:string; document_reference?:string|null; due_date_override?:string|null; due_date_override_reason?:string|null; narration?:string|null; document_discount_amount:string; additional_charges:string; lines:SalesLineRequest[]; }
export interface SalesDocument { id:string; company_id:string; voucher_id:string; party_id:string; invoice_date:string; document_reference:string|null; due_date:string; due_date_source:string; due_date_override_reason:string|null; narration:string|null; status:'draft'|'posted'|'cancelled'|'reversed'; subtotal:string; discount_total:string; additional_charges:string; net_total:string; tax_status:'inactive'; created_at:string; updated_at:string; }
export interface SalesDocumentLine { id:string; company_id:string; sales_document_id:string; line_number:number; description:string; stock_item_id:string|null; unit_id:string|null; godown_id:string|null; quantity:string; billed_quantity:string|null; rate:string; discount_kind:DiscountKind; discount_value:string; discount_amount:string; line_amount:string; created_at:string; }
export interface SalesDocumentWithLines { document:SalesDocument; voucher_id:string; voucher_number:string; lines:SalesDocumentLine[]; }

export type PurchaseDiscountKind = 'none' | 'amount' | 'percentage';
export interface PurchaseLineRequest { description:string; stock_item_id?:string|null; unit_id?:string|null; godown_id?:string|null; quantity:string; billed_quantity?:string|null; rate:string; discount_kind:PurchaseDiscountKind; discount_value:string; }
export interface PurchaseDocumentRequest { series_id:string; party_id:string; bill_date:string; supplier_invoice_no?:string|null; due_date_override?:string|null; due_date_override_reason?:string|null; narration?:string|null; document_discount_amount:string; additional_charges:string; lines:PurchaseLineRequest[]; }
export interface PurchaseDocument { id:string; company_id:string; voucher_id:string; party_id:string; bill_date:string; supplier_invoice_no:string|null; due_date:string; due_date_source:string; due_date_override_reason:string|null; narration:string|null; status:'draft'|'posted'|'cancelled'|'reversed'; subtotal:string; discount_total:string; additional_charges:string; net_total:string; tax_status:'inactive'; created_at:string; updated_at:string; }
export interface PurchaseDocumentLine { id:string; company_id:string; purchase_document_id:string; line_number:number; description:string; stock_item_id:string|null; unit_id:string|null; godown_id:string|null; quantity:string; billed_quantity:string|null; rate:string; discount_kind:PurchaseDiscountKind; discount_value:string; discount_amount:string; line_amount:string; created_at:string; }
export interface PurchaseDocumentWithLines { document:PurchaseDocument; voucher_id:string; voucher_number:string; lines:PurchaseDocumentLine[]; }
export interface CommercialPostingResult { voucher:VoucherWithLines; stock_voucher_id?:string|null; cogs_amount?:string|null; cost_layer_ids:string[]; consumed_layer_ids:string[]; }

export type AllocationType = 'NewRef' | 'AgstRef' | 'Advance' | 'OnAccount';

export interface BillAllocation {
  id: string;
  company_id: string;
  voucher_id: string;
  voucher_line_id: string;
  party_ledger_id: string;
  bill_name: string;
  allocation_type: AllocationType;
  amount: string;
  dr_cr: 'Dr' | 'Cr';
  bill_date: string;
  due_date: string | null;
  is_cancelled: number;
  created_at: string;
}

export interface BillAllocationInput {
  bill_name: string;
  allocation_type: AllocationType;
  amount: string;
  dr_cr?: string | null;
  due_date?: string | null;
}

export interface PendingBill {
  party_ledger_id: string;
  party_name: string;
  bill_name: string;
  bill_date: string;
  due_date: string | null;
  dr_cr: string;
  original_amount: string;
  settled_amount: string;
  pending_amount: string;
  overdue_days: number;
}

export interface AgingBucketSummary {
  not_due: string;
  days_0_30: string;
  days_31_60: string;
  days_61_90: string;
  days_90_plus: string;
  total: string;
}

export interface PartyAgingRow {
  party_ledger_id: string;
  party_name: string;
  buckets: AgingBucketSummary;
  bills: PendingBill[];
}

export interface AgingReport {
  as_of_date: string;
  report_type: string;
  parties: PartyAgingRow[];
  grand_total: AgingBucketSummary;
}

// -----------------------------------------------------------------------------
// Indian Statutory GST Models (Milestone 3)
// -----------------------------------------------------------------------------

export interface CompanyGstProfile {
  company_id: string;
  gstin: string | null;
  state_code: string;
  registration_type: string;
  gst_enabled: number | boolean;
  legal_name?: string | null;
  trade_name?: string | null;
  created_at?: string;
  updated_at?: string;
}

export interface LedgerGstProfile {
  id?: string;
  company_id: string;
  ledger_id: string;
  gstin: string | null;
  state_code: string;
  registration_type: string;
  is_composition?: boolean;
  created_at?: string;
  updated_at?: string;
}

export interface HsnSacCode {
  id: string;
  company_id: string;
  code: string;
  description: string | null;
  code_type: string;
  type?: string;
  default_gst_rate: string;
  cess_rate: string;
  default_cess_rate?: string;
  effective_from: string;
  created_at?: string;
}

export interface ItemGstMapping {
  id?: string;
  company_id: string;
  stock_item_id: string;
  hsn_sac_code_id: string;
  gst_rate: string;
  cess_rate: string;
  taxability: string;
}

export interface GstLedgerMapping {
  company_id: string;
  output_cgst_ledger_id: string;
  output_sgst_ledger_id: string;
  output_igst_ledger_id: string;
  output_cess_ledger_id: string;
  input_cgst_ledger_id: string;
  input_sgst_ledger_id: string;
  input_igst_ledger_id: string;
  input_cess_ledger_id: string;
}

export interface GstCalculationInput {
  company_state_code: string;
  place_of_supply_state_code: string;
  taxability: 'Taxable' | 'Exempt' | 'NilRated' | 'NonGST';
  taxable_value: string;
  gst_rate: string;
  cess_rate: string;
  is_reverse_charge: boolean;
}

export interface GstCalculationResult {
  supply_type: 'IntraState' | 'InterState';
  taxable_value: string;
  cgst_rate: string;
  cgst_amount: string;
  sgst_rate: string;
  sgst_amount: string;
  igst_rate: string;
  igst_amount: string;
  cess_rate: string;
  cess_amount: string;
  total_tax: string;
  grand_total: string;
}

export interface Gstr1B2bInvoiceRow {
  voucher_id: string;
  voucher_number: string;
  voucher_date: string;
  party_ledger_id: string;
  party_name: string;
  party_gstin: string;
  place_of_supply_state_code: string;
  supply_type: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  invoice_value: string;
}

export interface Gstr1B2cLargeRow {
  voucher_id: string;
  voucher_number: string;
  voucher_date: string;
  party_ledger_id: string;
  party_name: string;
  place_of_supply_state_code: string;
  taxable_value: string;
  igst_amount: string;
  cess_amount: string;
  invoice_value: string;
}

export interface Gstr1B2cSmallRow {
  place_of_supply_state_code: string;
  supply_type: string;
  gst_rate: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  total_tax: string;
}

export interface Gstr1HsnSummaryRow {
  hsn_sac_code: string;
  description: string | null;
  total_taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  total_tax: string;
  total_value: string;
}

export interface Gstr1CdnrRow {
  note_voucher_id: string;
  note_voucher_number: string;
  note_date: string;
  note_type: string;
  original_invoice_number: string;
  original_invoice_date: string;
  party_name: string;
  party_gstin: string;
  place_of_supply_state_code: string;
  supply_type: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  note_value: string;
}

export interface Gstr1CdnurRow {
  note_voucher_id: string;
  note_voucher_number: string;
  note_date: string;
  note_type: string;
  original_invoice_number: string;
  original_invoice_date: string;
  party_name: string;
  place_of_supply_state_code: string;
  supply_type: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  note_value: string;
}

export interface Gstr1SummaryReport {
  company_id: string;
  from_date: string;
  to_date: string;
  total_taxable_value: string;
  total_cgst: string;
  total_sgst: string;
  total_igst: string;
  total_cess: string;
  total_tax: string;
  total_invoice_value: string;
  b2b_invoices: Gstr1B2bInvoiceRow[];
  b2c_large_invoices: Gstr1B2cLargeRow[];
  b2c_small_summary: Gstr1B2cSmallRow[];
  hsn_summary: Gstr1HsnSummaryRow[];
  cdnr_table_9b?: Gstr1CdnrRow[];
  cdnur_table_9b?: Gstr1CdnurRow[];
}

export interface Gstr3bSection31Row {
  nature_of_supplies: string;
  total_taxable_value: string;
  integrated_tax: string;
  central_tax: string;
  state_ut_tax: string;
  cess: string;
}

export interface Gstr3bSection4ItcRow {
  itc_details: string;
  integrated_tax: string;
  central_tax: string;
  state_ut_tax: string;
  cess: string;
}

export interface Gstr3bNetPayableSummary {
  integrated_tax: string;
  central_tax: string;
  state_ut_tax: string;
  cess: string;
  total: string;
}

export interface Gstr3bSummaryReport {
  company_id: string;
  from_date: string;
  to_date: string;
  table_3_1: Gstr3bSection31Row[];
  table_4_itc: Gstr3bSection4ItcRow[];
  total_output_tax: Gstr3bNetPayableSummary;
  total_eligible_itc: Gstr3bNetPayableSummary;
  net_tax_payable: Gstr3bNetPayableSummary;
  closing_itc_balance: Gstr3bNetPayableSummary;
}

// -----------------------------------------------------------------------------
// Keyboard-First Navigation, Command Palette & Voucher Inspector (Milestone 4)
// -----------------------------------------------------------------------------

export interface GotoSearchResult {
  category: string; // "Screen" | "Ledger" | "Account Group" | "Stock Item" | "Voucher"
  id: string;
  title: string;
  subtitle: string;
  target_view: string;
  target_id?: string | null;
}

export interface VoucherFullLineDetail {
  id: string;
  line_number: number;
  ledger_id: string;
  ledger_name: string;
  group_name: string;
  classification: string;
  entry_type: 'Dr' | 'Cr';
  amount: string;
  narration?: string | null;
}

export interface VoucherGstDetailItem {
  id: string;
  party_ledger_id: string;
  party_name: string;
  party_gstin?: string | null;
  hsn_sac_code: string;
  place_of_supply_state_code: string;
  supply_type: string;
  taxability: string;
  taxable_value: string;
  gst_rate: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  is_reverse_charge: boolean;
  is_cancelled: boolean;
}

export interface VoucherBillAllocationItem {
  id: string;
  voucher_line_id: string;
  party_ledger_id: string;
  party_name: string;
  bill_name: string;
  allocation_type: string;
  amount: string;
  dr_cr: string;
  bill_date: string;
  due_date?: string | null;
  is_cancelled: boolean;
}

export interface VoucherStockMovementItem {
  id: string;
  stock_item_id: string;
  stock_item_name: string;
  godown_name?: string | null;
  movement_type: string;
  quantity: string;
  rate?: string | null;
  amount?: string | null;
}

export interface VoucherRegisterItem {
  id: string;
  company_id: string;
  voucher_type_id: string;
  voucher_type_name: string;
  series_id: string;
  series_name: string;
  voucher_number: string;
  date: string;
  effective_date: string;
  status: string;
  reference_no?: string | null;
  reference_date?: string | null;
  narration?: string | null;
  total_amount?: string | null;
  line_count: number;
  reversal_of_voucher_id?: string | null;
  reversal_voucher_id?: string | null;
  cancellation_reason?: string | null;
  created_at: string;
  updated_at: string;
}

export interface VoucherLineageResponse {
  items: VoucherRegisterItem[];
  has_exceeded_depth_limit: boolean;
  max_depth_reached: number;
}

export interface VoucherFullDetail {
  voucher: VoucherRegisterItem;
  lines: VoucherFullLineDetail[];
  gst_details: VoucherGstDetailItem[];
  bill_allocations: VoucherBillAllocationItem[];
  stock_movements: VoucherStockMovementItem[];
  lineage?: VoucherLineageResponse | null;
}

export interface NavigationStackEntry {
  view: string;
  title: string;
  params?: Record<string, any>;
}

// -----------------------------------------------------------------------------
// Milestone 5: Notes, Invoice PDF & WAL-Safe Backup/Restore
// -----------------------------------------------------------------------------

export interface BackupManifest {
  backup_file: string;
  destination_path: string;
  sha256_checksum: string;
  schema_version: string;
  timestamp: string;
  size_bytes: number;
}

export interface RestoreResult {
  restored_from: string;
  schema_version: string;
  safety_backup_path?: string | null;
  success: boolean;
  message: string;
}

export interface BackupStatus {
  database_engine: string;
  is_wal_active: boolean;
  current_schema_version: string;
  last_backup_timestamp?: string | null;
  last_backup_file?: string | null;
  last_backup_sha256?: string | null;
}

export interface CommercialLinePosting {
  line_number: number;
  description: string;
  stock_item_id?: string | null;
  unit_id?: string | null;
  godown_id?: string | null;
  quantity: string;
  rate: string;
  discount_amount: string;
  line_amount: string;
}

export interface TaxPostingLine {
  tax_type: string;
  tax_ledger_id: string;
  rate: string;
  amount: string;
}

export interface BillAllocationPostingLine {
  bill_type: string;
  bill_name: string;
  amount: string;
  due_date?: string | null;
}

export interface CommercialPostingInput {
  company_id: string;
  document_type: 'sales' | 'purchase' | 'credit_note' | 'debit_note';
  source_document_id: string;
  source_voucher_id?: string | null;
  series_id: string;
  date: string;
  due_date?: string | null;
  party_id: string;
  party_ledger_id: string;
  commercial_account_ledger_id?: string | null;
  total_amount: string;
  document_reference?: string | null;
  narration?: string | null;
  lines: CommercialLinePosting[];
  tax_lines: TaxPostingLine[];
  bill_allocations: BillAllocationPostingLine[];
  place_of_supply_state_code?: string | null;
  is_reverse_charge?: boolean | null;
  allow_tax_override?: boolean | null;
  original_voucher_id?: string | null;
  return_reason?: string | null;
  tds_section_code?: string | null;
}

export interface TallyXmlValidationError {
  item_type: string;
  identifier: string;
  message: string;
  severity: 'FATAL' | 'WARNING';
}

export interface TallyXmlImportPreview {
  groups_to_create: number;
  groups_skipped: number;
  ledgers_to_create: number;
  ledgers_skipped: number;
  stock_items_to_create: number;
  stock_items_skipped: number;
  vouchers_to_create: number;
  vouchers_skipped: number;
  can_import: boolean;
  errors: TallyXmlValidationError[];
}

export interface XmlMigrationReconciliationReport {
  source_voucher_count: number;
  imported_voucher_count: number;
  source_total_debit: string;
  imported_total_debit: string;
  source_total_credit: string;
  imported_total_credit: string;
  imported_trial_balance_difference: string;
  bill_allocations_imported: number;
  inventory_lines_imported: number;
  bank_allocations_imported: number;
  cost_centre_allocations_imported: number;
}

export interface TallyXmlImportResult {
  success: boolean;
  groups_created: number;
  ledgers_created: number;
  stock_items_created: number;
  vouchers_created: number;
  message: string;
  reconciliation?: XmlMigrationReconciliationReport | null;
}

export interface CompanyFeaturesDto {
  company_id: string;
  maintain_inventory: boolean;
  billwise_tracking: boolean;
  enable_perpetual_cogs: boolean;
  gst_enabled: boolean;
  gstin?: string | null;
  state_code?: string | null;
  registration_type: string;
  has_gst_ledgers_mapped: boolean;
  has_inventory_ledgers_mapped: boolean;
  tds_enabled: boolean;
  tan?: string | null;
  deductor_type?: string | null;
  has_tds_sections_seeded: boolean;
  enable_payroll?: boolean;
  has_payroll_ledgers_mapped?: boolean;
}

export interface SaveCompanyFeaturesInput {
  maintain_inventory: boolean;
  billwise_tracking: boolean;
  enable_perpetual_cogs: boolean;
  gst_enabled: boolean;
  gstin?: string | null;
  state_code?: string | null;
  registration_type?: string | null;
  auto_seed_gst_ledgers?: boolean;
  auto_seed_inventory_ledgers?: boolean;
  tds_enabled?: boolean;
  tan?: string | null;
  deductor_type?: string | null;
  auto_seed_tds_defaults?: boolean;
  enable_payroll?: boolean;
  auto_seed_payroll_ledgers?: boolean;
}



// Milestone 8 Banking & Bank Reconciliation Statement (BRS) Types
export interface BankAllocationInput {
  voucher_line_id?: string | null;
  bank_ledger_id?: string | null;
  transaction_type?: string | null;
  instrument_number?: string | null;
  instrument_date?: string | null;
  favouring_name?: string | null;
  bank_date?: string | null;
}

export interface BankAllocation {
  id: string;
  company_id: string;
  voucher_id: string;
  voucher_line_id: string;
  bank_ledger_id: string;
  transaction_type: string;
  instrument_number?: string | null;
  instrument_date?: string | null;
  favouring_name?: string | null;
  bank_date?: string | null;
  amount: string;
  dr_cr: string;
  is_cancelled: number;
  created_at: string;
  updated_at: string;
}

export interface BankLedgerItem {
  id: string;
  name: string;
  group_id: string;
  group_name: string;
  current_balance: string;
  current_balance_dr_cr: string;
}

export interface ReconcileEntryInput {
  bank_allocation_id: string;
  bank_date?: string | null;
}

export interface ReconcileResult {
  success: boolean;
  updated_count: number;
  updated_ids: string[];
}

export interface BankReconciliationRow {
  bank_allocation_id: string;
  voucher_id: string;
  voucher_line_id: string;
  voucher_date: string;
  voucher_number: string;
  voucher_type: string;
  particulars: string;
  transaction_type: string;
  instrument_number?: string | null;
  instrument_date?: string | null;
  favouring_name?: string | null;
  bank_date?: string | null;
  debit_amount: string;
  credit_amount: string;
  is_reconciled: boolean;
}

export interface BankReconciliationSummary {
  book_balance: string;
  book_balance_dr_cr: string;
  unreconciled_debit: string;
  unreconciled_credit: string;
  bank_balance: string;
  bank_balance_dr_cr: string;
}

export interface BankReconciliationStatement {
  bank_ledger_id: string;
  bank_ledger_name: string;
  from_date: string;
  to_date: string;
  summary: BankReconciliationSummary;
  rows: BankReconciliationRow[];
}

export interface BankStatementMatchRow {
  csv_row_index: number;
  statement_date: string;
  statement_ref: string;
  statement_description: string;
  statement_amount: string;
  statement_dr_cr: string;
  matched_bank_allocation_id?: string | null;
  matched_voucher_number?: string | null;
  matched_particulars?: string | null;
  suggested_bank_date?: string | null;
  match_type: 'exact' | 'suggested' | 'unmatched';
  confidence: 'HIGH' | 'MEDIUM' | 'NONE';
}

export interface BankStatementMatchPreview {
  total_statement_rows: number;
  exact_matches: number;
  suggested_matches: number;
  unmatched: number;
  rows: BankStatementMatchRow[];
}

// Milestone 9: Indian TDS & TCS Models
export type DeducteeType =
  | 'Individual_HUF'
  | 'Company_Domestic'
  | 'Company_Foreign'
  | 'Firm'
  | 'Other';

export interface CompanyTdsProfile {
  company_id: string;
  tan: string | null;
  deductor_type: string;
  tds_enabled: number;
  created_at: string;
  updated_at: string;
}

export interface SaveCompanyTdsProfileInput {
  company_id: string;
  tan?: string | null;
  deductor_type: string;
  tds_enabled: boolean;
}

export interface LedgerTdsProfile {
  id: string;
  company_id: string;
  ledger_id: string;
  pan: string | null;
  deductee_type: string;
  lower_deduction_cert_no: string | null;
  lower_deduction_rate: string | null;
  created_at: string;
  updated_at: string;
}

export interface SaveLedgerTdsProfileInput {
  company_id: string;
  ledger_id: string;
  pan?: string | null;
  deductee_type: string;
  lower_deduction_cert_no?: string | null;
  lower_deduction_rate?: string | null;
}

export interface TdsNatureOfPayment {
  id: string;
  company_id: string;
  section_code: string;
  name: string;
  rate_individual_huf: string;
  rate_others: string;
  rate_without_pan: string;
  single_threshold_limit: string;
  aggregate_threshold_limit: string;
  tds_payable_ledger_id: string | null;
  is_tcs: number;
  created_at: string;
  updated_at: string;
}

export interface CreateTdsNatureOfPaymentInput {
  company_id: string;
  section_code: string;
  name: string;
  rate_individual_huf: string;
  rate_others: string;
  rate_without_pan?: string | null;
  single_threshold_limit?: string | null;
  aggregate_threshold_limit?: string | null;
  tds_payable_ledger_id?: string | null;
  is_tcs?: boolean | null;
}

export interface VoucherTdsDetail {
  id: string;
  company_id: string;
  voucher_id: string;
  party_ledger_id: string;
  party_pan: string | null;
  section_code: string;
  deductee_type: string;
  gross_amount: string;
  tds_rate: string;
  tds_amount: string;
  net_payable_amount: string;
  is_pan_missing_higher_rate: number;
  challan_bsr_code: string | null;
  challan_date: string | null;
  challan_serial_no: string | null;
  payment_voucher_id: string | null;
  is_paid: number;
  is_cancelled: number;
  created_at: string;
}

export interface TdsCalculationPreviewInput {
  company_id: string;
  party_ledger_id: string;
  section_code: string;
  taxable_amount: string;
  voucher_date: string;
}

export interface TdsCalculationPreviewResult {
  section_code: string;
  section_name: string;
  deductee_type: string;
  party_pan: string | null;
  is_pan_missing: boolean;
  is_pan_missing_higher_rate: boolean;
  taxable_base: string;
  applicable_rate: string;
  threshold_exceeded: boolean;
  single_threshold: string;
  aggregate_threshold: string;
  prior_aggregate_amount: string;
  current_aggregate_amount: string;
  tds_amount: string;
  net_payable_amount: string;
  tds_payable_ledger_id: string | null;
}

export interface RecordTdsChallanPaymentInput {
  company_id: string;
  voucher_tds_detail_ids: string[];
  challan_bsr_code: string;
  challan_date: string;
  challan_serial_no: string;
  payment_voucher_id?: string | null;
  actor_id?: string | null;
}

export interface TdsOutstandingVoucherItem {
  voucher_tds_id: string;
  voucher_id: string;
  voucher_number: string;
  voucher_date: string;
  party_ledger_id: string;
  party_name: string;
  party_pan: string | null;
  section_code: string;
  gross_amount: string;
  tds_rate: string;
  tds_amount: string;
  due_date: string;
  status: string;
}

export interface TdsOutstandingSectionSummary {
  section_code: string;
  section_name: string;
  unpaid_tds_amount: string;
  voucher_count: number;
  vouchers: TdsOutstandingVoucherItem[];
}

export interface TdsOutstandingReport {
  as_of_date: string;
  total_unpaid_tds: string;
  sections: TdsOutstandingSectionSummary[];
}

export interface Form26qChallanItem {
  challan_bsr_code: string;
  challan_date: string;
  challan_serial_no: string;
  payment_voucher_id: string | null;
  total_tds_deposited: string;
  voucher_count: number;
}

export interface Form26qDeductionItem {
  voucher_tds_id: string;
  voucher_number: string;
  deductee_pan: string | null;
  deductee_name: string;
  deductee_type: string;
  section_code: string;
  payment_date: string;
  amount_paid: string;
  tds_rate: string;
  tds_deducted: string;
  is_pan_missing_higher_rate: boolean;
  challan_serial_no: string | null;
  challan_date: string | null;
  challan_bsr_code: string | null;
  status: string;
}

export interface Form26qSummary {
  company_id: string;
  financial_year: string;
  quarter: string;
  deductor_tan: string | null;
  deductor_name: string;
  total_amount_paid: string;
  total_tds_deducted: string;
  total_tds_deposited: string;
  total_tds_unpaid: string;
  challans: Form26qChallanItem[];
  deductions: Form26qDeductionItem[];
}

// -----------------------------------------------------------------------------
// Milestone 10: GST Statutory Compliance (GSTR-2B, e-Invoice, e-Way Bill)
// -----------------------------------------------------------------------------

export interface Gstr2bReconciledInvoiceRow {
  id: string;
  batch_id: string;
  supplier_gstin: string;
  supplier_name: string | null;
  invoice_number: string;
  invoice_date: string;
  invoice_value: string;
  taxable_value: string;
  cgst_amount: string;
  sgst_amount: string;
  igst_amount: string;
  cess_amount: string;
  total_tax: string;
  itc_availability: string;
  reconciliation_status: 'Matched' | 'Mismatch' | 'MissingInBooks' | 'MissingIn2B';
  matched_voucher_id: string | null;
  matched_voucher_number: string | null;
  books_taxable_value: string | null;
  books_total_tax: string | null;
  tax_difference: string;
}

export interface Gstr2bReconciliationSummary {
  total_portal_invoices: number;
  matched_count: number;
  mismatch_count: number;
  missing_in_books_count: number;
  missing_in_2b_count: number;
  matched_itc: string;
  mismatch_itc: string;
  missing_in_books_itc: string;
  at_risk_itc: string;
  total_eligible_itc: string;
}

export interface Gstr2bReconciliationReport {
  company_id: string;
  return_period: string;
  batch_id: string | null;
  imported_at: string | null;
  summary: Gstr2bReconciliationSummary;
  invoices: Gstr2bReconciledInvoiceRow[];
}

export interface TransportDetailsInput {
  transport_mode: string;
  distance_km: number;
  transporter_id?: string | null;
  transporter_name?: string | null;
  trans_doc_no?: string | null;
  trans_doc_date?: string | null;
  vehicle_no?: string | null;
  vehicle_type?: string | null;
}

export interface VoucherEinvoiceMetadataInput {
  irn?: string | null;
  ack_no?: string | null;
  ack_date?: string | null;
  signed_qr_code?: string | null;
  einvoice_status?: string | null;
  eway_bill_no?: string | null;
  eway_bill_date?: string | null;
  eway_valid_upto?: string | null;
  transporter_id?: string | null;
  transporter_name?: string | null;
  transport_mode?: string | null;
  vehicle_no?: string | null;
  distance_km?: number | null;
}

export interface VoucherEinvoiceMetadataDto {
  id: string;
  company_id: string;
  voucher_id: string;
  irn: string | null;
  ack_no: string | null;
  ack_date: string | null;
  signed_qr_code: string | null;
  einvoice_status: 'Pending' | 'Generated' | 'Cancelled';
  eway_bill_no: string | null;
  eway_bill_date: string | null;
  eway_valid_upto: string | null;
  transporter_id: string | null;
  transporter_name: string | null;
  transport_mode: string | null;
  vehicle_no: string | null;
  distance_km: number | null;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Milestone 11: Multi-Location Godowns, Stock Journals, Batches & Cost Centres
// ---------------------------------------------------------------------------

export interface StockItemBatch {
  id: string;
  company_id: string;
  stock_item_id: string;
  batch_number: string;
  mfg_date: string | null;
  expiry_date: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface StockItemBatchRequest {
  stock_item_id: string;
  batch_number: string;
  mfg_date?: string | null;
  expiry_date?: string | null;
}

export interface BatchExpiryReportRow {
  batch_id: string;
  stock_item_id: string;
  stock_item_name: string;
  batch_number: string;
  mfg_date: string | null;
  expiry_date: string | null;
  closing_quantity: string;
  unit_cost: string;
  closing_value: string;
  days_until_expiry: number | null;
  status: string;
}

export interface StockJournalLine {
  stock_item_id: string;
  godown_id: string;
  batch_number?: string | null;
  quantity: string;
  rate: string;
  amount: string;
}

export interface StockJournalRequest {
  voucher_number?: string | null;
  series_id?: string | null;
  voucher_date: string;
  narration?: string | null;
  source_lines: StockJournalLine[];
  destination_lines: StockJournalLine[];
}

export interface StockJournalResponse {
  voucher_id: string;
  voucher_number: string;
  voucher_date: string;
  total_source_value: string;
  total_destination_value: string;
  lines_count: number;
}

export interface GodownSummaryItemRow {
  stock_item_id: string;
  stock_item_name: string;
  base_unit: string;
  inflow_quantity: string;
  outflow_quantity: string;
  closing_quantity: string;
  average_rate: string;
  closing_value: string;
}

export interface GodownSummaryReport {
  godown_id: string;
  godown_name: string;
  address: string | null;
  is_external: boolean;
  items: GodownSummaryItemRow[];
  total_godown_value: string;
}

export interface CostCategory {
  id: string;
  company_id: string;
  name: string;
  allocate_revenue: boolean;
  allocate_non_revenue: boolean;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CostCategoryRequest {
  name: string;
  allocate_revenue: boolean;
  allocate_non_revenue: boolean;
}

export interface CostCentre {
  id: string;
  company_id: string;
  category_id: string;
  name: string;
  parent_id: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CostCentreRequest {
  category_id: string;
  name: string;
  parent_id?: string | null;
}

export interface VoucherCostAllocationItem {
  id: string;
  company_id: string;
  voucher_id: string;
  voucher_line_id: string;
  cost_centre_id: string;
  cost_centre_name: string;
  category_name: string;
  amount: string;
}

export interface CostCentreBreakupItem {
  cost_centre_id: string;
  cost_centre_name: string;
  category_id: string;
  category_name: string;
  debit_total: string;
  credit_total: string;
  net_amount: string;
  allocations_count: number;
}

export interface CostCentreBreakupReport {
  from_date: string | null;
  to_date: string | null;
  centres: CostCentreBreakupItem[];
  total_debit: string;
  total_credit: string;
  net_total: string;
}

// -----------------------------------------------------------------------------
// Milestone 12: Ratio Analysis, Budgets, Year Closing & MCA Audit Trail
// -----------------------------------------------------------------------------

export interface RatioAnalysisReport {
  company_id: string;
  from_date: string;
  to_date: string;
  // Left Pane: Principal Groups & Figures
  working_capital: string;
  cash_in_hand: string;
  bank_accounts: string;
  bank_od_ac: string;
  sundry_debtors: string;
  sundry_creditors: string;
  sales_accounts: string;
  purchase_accounts: string;
  closing_stock: string;
  net_profit: string;
  // Right Pane: Principal Ratios
  current_ratio: string;
  quick_ratio: string;
  debt_equity_ratio: string;
  gross_profit_pct: string;
  net_profit_pct: string;
  operating_cost_pct: string;
  receivables_turnover_days: string;
  inventory_turnover: string;
  return_on_working_capital_pct: string;
}

export interface Budget {
  id: string;
  company_id: string;
  name: string;
  from_date: string;
  to_date: string;
  created_at: string;
  updated_at: string;
}

export interface BudgetLine {
  id: string;
  company_id: string;
  budget_id: string;
  target_type: 'Group' | 'Ledger' | 'CostCenter';
  target_id: string;
  budget_type: 'OnNetTransactions' | 'OnClosingBalance';
  amount: string;
  dr_cr: 'Dr' | 'Cr';
  created_at: string;
}

export interface BudgetLineRequest {
  target_type: 'Group' | 'Ledger' | 'CostCenter';
  target_id: string;
  budget_type: 'OnNetTransactions' | 'OnClosingBalance';
  amount: string | number;
  dr_cr: 'Dr' | 'Cr';
}

export interface BudgetRequest {
  name: string;
  from_date: string;
  to_date: string;
  lines: BudgetLineRequest[];
}

export interface BudgetDetails {
  budget: Budget;
  lines: BudgetLine[];
}

export interface BudgetVarianceRow {
  line_id: string;
  target_type: string;
  target_id: string;
  target_name: string;
  budget_type: string;
  dr_cr: string;
  budget_amount: string;
  actual_amount: string;
  variance_amount: string;
  utilization_pct: string;
  is_over_budget: boolean;
}

export interface BudgetVarianceReport {
  budget_id: string;
  budget_name: string;
  company_id: string;
  as_of_date: string;
  from_date: string;
  to_date: string;
  rows: BudgetVarianceRow[];
  total_budget: string;
  total_actual: string;
  total_variance: string;
}

export interface FinancialYearClosingRecord {
  id: string;
  company_id: string;
  financial_year_label: string;
  period_start: string;
  period_end: string;
  next_fy_start: string;
  closing_voucher_id: string;
  net_profit_or_loss: string;
  is_locked: boolean;
  closed_at: string;
  closed_by: string;
}

export interface NominalTransferPreview {
  ledger_id: string;
  ledger_name: string;
  account_group_nature: string;
  closing_debit: string;
  closing_credit: string;
  transfer_entry_type: 'Dr' | 'Cr';
  transfer_amount: string;
}

export interface BalanceSheetCarryForwardPreview {
  ledger_id: string;
  ledger_name: string;
  account_group_nature: string;
  balance_type: 'Dr' | 'Cr';
  amount: string;
}

export interface YearEndClosingPreview {
  company_id: string;
  financial_year_label: string;
  period_start: string;
  period_end: string;
  next_fy_start: string;
  trial_balance_difference: string;
  is_equilibrium: boolean;
  net_profit_or_loss: string;
  is_profit: boolean;
  nominal_transfers: NominalTransferPreview[];
  balance_sheet_balances: BalanceSheetCarryForwardPreview[];
}

export interface AuditIntegrityIssue {
  entry_id: string;
  entity_type: string;
  entity_id: string;
  timestamp: string;
  expected_hash: string;
  actual_hash: string;
  issue_type: string;
}

export interface AuditCertificationReport {
  company_id: string;
  generated_at: string;
  total_entries: number;
  chained_entries: number;
  legacy_unchained_entries: number;
  tampered_entries: number;
  broken_chain_links: number;
  is_valid: boolean;
  certification_status: 'VERIFIED_CLEAN' | 'TAMPER_DETECTED' | string;
  first_entry_timestamp?: string | null;
  latest_entry_timestamp?: string | null;
  latest_entry_hash?: string | null;
  issues: AuditIntegrityIssue[];
}

export type {
  PurchaseOrder,
  PurchaseOrderLine,
  PurchaseOrderWithLines,
  SalesOrder,
  SalesOrderLine,
  SalesOrderWithLines,
  CreateOrderLineInput,
  CreatePurchaseOrderInput,
  CreateSalesOrderInput,
  FulfillOrderLineInput,
  ConvertPurchaseOrderInput,
  ConvertSalesOrderInput,
  OrderFulfillmentRecord,
  ConvertOrderResult,
  PendingOrderItem,
  PendingOrdersReport,
  UserRole,
  CompanyUser,
  AuthSessionInfo,
  SecurityStatusDto,
  EnableSecurityInput,
  LoginInput,
  CreateUserInput,
  UpdateUserRoleInput,
  UpdateUserPasswordInput,
  DeactivateUserInput,
  BomComponentDetail,
  BomDetail,
  CreateBomComponentInput,
  CreateBomInput,
  AdditionalOverheadCostInput,
  ExecuteManufacturingJournalInput,
  ConsumedComponentResult,
  ManufacturingJournalResult,
  Employee,
  CreateEmployeeInput,
  PayrollAttendanceInput,
  PayrollRunInput,
  ComputedPayslipPreview,
  PayrollPreviewResult,
  PayrollRun,
  PayrollPayslip,
  PayrollRunResult,
  PayrollStatutoryRow,
  PayrollStatutorySummary,
} from '../services/api';

// ---------------------------------------------------------------------------
// Milestone 17: Multi-User LAN Host / Client Mode Types
// ---------------------------------------------------------------------------

export interface LanSessionDto {
  session_id: string;
  company_id: string;
  user_id: string;
  username: string;
  role: string;
  client_ip?: string;
  created_at: string;
  expires_at: string;
  last_active_at: string;
}

export interface LanServerStatusDto {
  is_running: boolean;
  bind_address: string;
  port: number;
  local_ip_addresses: string[];
  active_sessions: LanSessionDto[];
}

export interface LanClientConfig {
  host: string;
  port: number;
  company_id: string;
  username: string;
  token: string;
  role: string;
}

export interface LanLoginResponse {
  session_id: string;
  token: string;
  company_id: string;
  user_id: string;
  username: string;
  role: string;
  expires_at: string;
}

// ---------------------------------------------------------------------------
// Milestone 16: Multi-Currency & Bank Cheque Printing Types
// ---------------------------------------------------------------------------

export interface Currency {
  id: string;
  company_id: string;
  code: string;
  symbol: string;
  formal_name: string;
  decimal_places: number;
  is_base: boolean;
  created_at: string;
}

export interface CreateCurrencyInput {
  code: string;
  symbol: string;
  formal_name: string;
  decimal_places?: number;
  is_base?: boolean;
}

export interface ExchangeRate {
  id: string;
  company_id: string;
  currency_code: string;
  effective_date: string;
  standard_rate: string;
  selling_rate: string;
  buying_rate: string;
  created_at: string;
}

export interface SetExchangeRateInput {
  currency_code: string;
  effective_date: string;
  standard_rate: string;
  selling_rate?: string;
  buying_rate?: string;
}

export interface EffectiveRateDto {
  currency_code: string;
  effective_date: string;
  standard_rate: string;
  selling_rate: string;
  buying_rate: string;
  is_base: boolean;
}

export interface OpenForeignBill {
  bill_name: string;
  party_ledger_id: string;
  party_name: string;
  voucher_id: string;
  voucher_number: string;
  bill_date: string;
  due_date?: string | null;
  currency_code: string;
  invoice_rate: string;
  original_foreign: string;
  settled_foreign: string;
  outstanding_foreign: string;
  original_inr: string;
  settled_inr: string;
  outstanding_inr: string;
  dr_cr: string;
}

export interface SettleForeignBillInput {
  bill_name: string;
  party_ledger_id: string;
  foreign_settled_amount: string;
  settlement_date: string;
  settlement_exchange_rate?: string;
  payment_mode: string;
  bank_or_cash_ledger_id: string;
  narration?: string;
  cheque_instrument_number?: string;
}

export interface RealizedForexResult {
  settlement_voucher_id: string;
  forex_journal_voucher_id?: string | null;
  foreign_settled: string;
  settlement_rate: string;
  invoice_rate: string;
  forex_difference: string;
  is_gain: boolean;
  settled_inr: string;
  book_inr: string;
}

export interface UnrealizedForexBillRow {
  bill_name: string;
  party_ledger_id: string;
  party_name: string;
  currency_code: string;
  dr_cr: string;
  open_foreign_amount: string;
  invoice_rate: string;
  book_value_inr: string;
  revaluation_rate: string;
  revalued_value_inr: string;
  unrealized_gain_or_loss: string;
  is_gain: boolean;
}

export interface UnrealizedForexReport {
  company_id: string;
  as_of_date: string;
  bills: UnrealizedForexBillRow[];
  total_book_value_inr: string;
  total_revalued_value_inr: string;
  total_unrealized_gain: string;
  total_unrealized_loss: string;
  net_unrealized_gain_loss: string;
}

export interface ChequeTemplateDto {
  id: string;
  company_id: string;
  bank_name: string;
  width_mm: number;
  height_mm: number;
  date_x_mm: number;
  date_y_mm: number;
  payee_x_mm: number;
  payee_y_mm: number;
  words_x_mm: number;
  words_y_mm: number;
  amount_x_mm: number;
  amount_y_mm: number;
  is_account_payee_default: boolean;
}

export interface UpsertChequeTemplateInput {
  template_id?: string;
  bank_name: string;
  width_mm: number;
  height_mm: number;
  date_x_mm: number;
  date_y_mm: number;
  payee_x_mm: number;
  payee_y_mm: number;
  words_x_mm: number;
  words_y_mm: number;
  amount_x_mm: number;
  amount_y_mm: number;
  is_account_payee_default?: boolean;
}

export interface CurrencySeedSummary {
  base_currency_seeded: boolean;
  currencies_seeded_count: number;
  forex_ledger_id: string;
  cheque_templates_seeded_count: number;
}




// ==========================================
// Milestone 18: BYO-GSP & Custom Report Extensions
// ==========================================

export interface CompanyGspConfigDto {
  company_id: string;
  provider_mode: 'Sandbox' | 'HttpGsp';
  provider_name: string;
  base_url?: string | null;
  client_id?: string | null;
  has_api_secret: boolean;
  is_enabled: boolean;
  updated_at: string;
}

export interface SaveCompanyGspConfigInput {
  provider_mode: 'Sandbox' | 'HttpGsp';
  provider_name: string;
  base_url?: string;
  client_id?: string;
  api_secret_ref?: string;
  is_enabled: boolean;
}

export interface GspEinvoiceResponse {
  irn: string;
  ack_no: string;
  ack_date: string;
  signed_qr_code?: string | null;
  provider_mode: string;
}

export interface GspEwayBillResponse {
  ewb_no: string;
  ewb_date: string;
  ewb_valid_upto: string;
  provider_mode: string;
}

export type CustomReportDataSource =
  | 'Vouchers'
  | 'Ledgers'
  | 'StockItems'
  | 'OutstandingBills'
  | 'GstInvoices';

export interface CustomReportFilter {
  field: string;
  op: 'eq' | 'contains' | 'gte' | 'lte' | 'between';
  value: string;
  value_end?: string;
}

export interface CustomReportDefinitionDto {
  id: string;
  company_id: string;
  name: string;
  description: string;
  data_source: CustomReportDataSource;
  columns: string[];
  filters: CustomReportFilter[];
  sort_by?: string | null;
  sort_dir: 'ASC' | 'DESC';
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface SaveCustomReportInput {
  id?: string;
  name: string;
  description?: string;
  data_source: CustomReportDataSource;
  columns: string[];
  filters?: CustomReportFilter[];
  sort_by?: string;
  sort_dir?: 'ASC' | 'DESC';
}

export interface CustomReportExecutionResult {
  report_id: string;
  report_name: string;
  data_source: string;
  columns: string[];
  rows: Record<string, string>[];
  numeric_totals: Record<string, string>;
  row_count: number;
}


// ==========================================
// Milestone 19: GSTN Strictness & Complex GST
// ==========================================

export interface Gstr1ExportRow {
  voucher_id: string;
  voucher_number: string;
  voucher_date: string;
  exp_typ: string;
  supply_category: string;
  port_code?: string | null;
  shipping_bill_no?: string | null;
  shipping_bill_date?: string | null;
  taxable_value: string;
  igst_amount: string;
  cess_amount: string;
  invoice_value: string;
}

export interface Gstr1DocIssueRow {
  doc_num: number;
  doc_type: string;
  series_name: string;
  from_num: string;
  to_num: string;
  totnum: number;
  cancel: number;
  net_issue: number;
}

export interface Cmp08QuarterlySummary {
  company_id: string;
  financial_year: number;
  quarter: number;
  from_date: string;
  to_date: string;
  composition_rate: string;
  outward_turnover: string;
  composition_tax_payable: string;
  rcm_taxable_value: string;
  rcm_tax_payable: string;
  total_cmp08_liability: string;
}

export interface GstnValidationIssue {
  section: string;
  field: string;
  message: string;
  severity: string;
}

export interface GstnValidationReport {
  is_valid: boolean;
  error_count: number;
  warning_count: number;
  errors: GstnValidationIssue[];
  warnings: GstnValidationIssue[];
}


// ==========================================
// Milestone 20: Distributor Inventory & Billing Depth
// ==========================================

export interface CompoundUnitDto {
  id: string;
  company_id: string;
  name: string;
  symbol: string;
  unit_type: string;
  first_unit_id?: string | null;
  second_unit_id?: string | null;
  conversion_factor?: string | null;
}

export interface PriceLevelDto {
  id: string;
  company_id: string;
  name: string;
  is_active: number;
  created_at: string;
}

export interface PriceListEntryDto {
  id: string;
  company_id: string;
  price_level_id: string;
  stock_item_id: string;
  effective_from: string;
  min_qty: string;
  max_qty?: string | null;
  unit_rate: string;
  discount_pct: string;
}

export interface ResolvedItemPriceDto {
  unit_rate: string;
  discount_pct: string;
  matched_price_level?: string | null;
}
