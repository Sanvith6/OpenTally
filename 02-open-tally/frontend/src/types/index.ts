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
export interface Godown { id:string; company_id:string; name:string; parent_id:string|null; description:string|null; is_active:boolean; created_at:string; updated_at:string; }
export interface StockGroupRequest { name:string; parent_id?:string|null; description?:string|null; }
export interface StockCategoryRequest { name:string; description?:string|null; }
export interface UnitRequest { symbol:string; formal_name:string; decimal_precision:number; unit_type?:string; base_unit_id?:string|null; secondary_unit_id?:string|null; conversion_factor?:string|null; }
export interface StockItemRequest { name:string; stock_group_id:string; stock_category_id?:string|null; base_unit_id:string; alternate_unit_id?:string|null; sku?:string|null; description?:string|null; opening_quantity:string; opening_rate:string; opening_value:string; reorder_level:string; }
export interface GodownRequest { name:string; parent_id?:string|null; description?:string|null; }
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
export interface SalesLineRequest { description:string; stock_item_id?:string|null; unit_id?:string|null; godown_id?:string|null; quantity:string; rate:string; discount_kind:DiscountKind; discount_value:string; }
export interface SalesDocumentRequest { series_id:string; party_id:string; invoice_date:string; document_reference?:string|null; due_date_override?:string|null; due_date_override_reason?:string|null; narration?:string|null; document_discount_amount:string; additional_charges:string; lines:SalesLineRequest[]; }
export interface SalesDocument { id:string; company_id:string; voucher_id:string; party_id:string; invoice_date:string; document_reference:string|null; due_date:string; due_date_source:string; due_date_override_reason:string|null; narration:string|null; status:'draft'; subtotal:string; discount_total:string; additional_charges:string; net_total:string; tax_status:'inactive'; created_at:string; updated_at:string; }
export interface SalesDocumentLine { id:string; company_id:string; sales_document_id:string; line_number:number; description:string; stock_item_id:string|null; unit_id:string|null; godown_id:string|null; quantity:string; rate:string; discount_kind:DiscountKind; discount_value:string; discount_amount:string; line_amount:string; created_at:string; }
export interface SalesDocumentWithLines { document:SalesDocument; voucher_id:string; voucher_number:string; lines:SalesDocumentLine[]; }
