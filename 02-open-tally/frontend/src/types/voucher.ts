export type VoucherTypeCategory =
  | 'accounting'
  | 'trading'
  | 'order'
  | 'inventory'
  | 'payroll'
  | 'scenario'
  | 'non_financial';

export type NumberingMethod = 'automatic' | 'manual' | 'multi_series';

export type SeriesRestartPeriod = 'yearly' | 'monthly' | 'daily' | 'never';

export interface VoucherType {
  id: string;
  company_id: string;
  name: string;
  category: VoucherTypeCategory;
  parent_type: string | null;
  affects_ledger_balance: boolean;
  affects_inventory: boolean;
  is_predefined: boolean;
  is_active: boolean;
  numbering_method: NumberingMethod;
  created_at: string;
  updated_at: string;
}

export interface VoucherNumberSeries {
  id: string;
  company_id: string;
  voucher_type_id: string;
  series_name: string;
  series_code: string;
  numbering_method: 'automatic' | 'manual';
  restart_period: SeriesRestartPeriod;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface VoucherNumberConfig {
  id: string;
  company_id: string;
  voucher_type_id: string;
  series_id: string;
  applicable_from: string;
  prefix: string;
  suffix: string;
  start_number: number;
  width: number;
  created_at: string;
  updated_at: string;
}

export interface CreateVoucherSeriesRequest {
  voucher_type_id: string;
  series_name: string;
  series_code: string;
  numbering_method: 'automatic' | 'manual';
  restart_period: SeriesRestartPeriod;
  applicable_from?: string;
  prefix?: string;
  suffix?: string;
  start_number?: number;
  width?: number;
}

export interface UpdateVoucherSeriesRequest {
  series_name?: string;
  is_active?: boolean;
}

export interface CreateVoucherNumberConfigRequest {
  applicable_from: string;
  prefix?: string;
  suffix?: string;
  start_number?: number;
  width?: number;
}

export interface EffectiveNumberConfig {
  config_id: string;
  voucher_type_id: string;
  series_id: string;
  prefix: string;
  suffix: string;
  width: number;
  start_number: number;
  restart_period: string;
  applicable_from: string;
  period_start: string;
}

export interface SequenceRecoveryReport {
  company_id: string;
  voucher_type_id: string;
  series_id: string;
  period_start: string;
  start_number_baseline: number;
  previous_next_number: number;
  new_next_number: number;
  max_allocated: number;
  total_permanent_allocations: number;
  historical_gaps: number[];
  counter_tail_gaps: number[];
  counter_updated: boolean;
  operator_id: string;
  resolution_mode: string;
  resolution_reason?: string | null;
}

export interface InspectSequenceRequest {
  series_id: string;
  period_start: string;
}

export interface ResyncSequenceRequest {
  series_id: string;
  period_start: string;
  resolution_mode: 'strict' | 'allow_historical_gaps';
  resolution_reason?: string | null;
}

// ============================================================================
// Sprint 2 Stage 4: Voucher Header & Line Items / Draft Management Types
// ============================================================================

export type VoucherEntryType = 'Dr' | 'Cr';

export interface Voucher {
  id: string;
  company_id: string;
  voucher_type_id: string;
  series_id: string;
  numbering_method: string;
  period_start?: string | null;
  sequence_number?: number | null;
  voucher_number: string;
  date: string;
  effective_date: string;
  status: 'draft' | 'posted' | 'cancelled' | 'reversed';
  reference_no?: string | null;
  reference_date?: string | null;
  narration?: string | null;
  total_amount?: string | null;
  reversal_of_voucher_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface VoucherLine {
  id: string;
  company_id: string;
  voucher_id: string;
  line_number: number;
  ledger_id: string;
  entry_type: VoucherEntryType;
  amount: string;
  narration?: string | null;
  created_at: string;
}

export interface VoucherWithLines {
  voucher: Voucher;
  lines: VoucherLine[];
  debit_total: string;
  credit_total: string;
  difference: string;
  is_balanced: boolean;
}

export interface VoucherSummary {
  id: string;
  company_id: string;
  voucher_type_id: string;
  voucher_type_name: string;
  series_id: string;
  series_name: string;
  voucher_number: string;
  date: string;
  status: string;
  total_amount?: string | null;
  line_count: number;
  updated_at: string;
}

export interface VoucherLineInput {
  id?: string;
  ledger_id: string;
  entry_type: VoucherEntryType;
  amount: string;
  narration?: string | null;
}

export interface CreateVoucherDraftRequest {
  voucher_type_id: string;
  series_id: string;
  date: string;
  effective_date?: string | null;
  reference_no?: string | null;
  reference_date?: string | null;
  narration?: string | null;
  lines?: VoucherLineInput[] | null;
}

export interface UpdateVoucherDraftRequest {
  voucher_type_id?: string | null;
  series_id?: string | null;
  date?: string | null;
  effective_date?: string | null;
  reference_no?: string | null;
  reference_date?: string | null;
  narration?: string | null;
}

export interface AddVoucherLineRequest {
  ledger_id: string;
  entry_type: VoucherEntryType;
  amount: string;
  narration?: string | null;
  line_number?: number | null;
}

export interface UpdateVoucherLineRequest {
  ledger_id?: string | null;
  entry_type?: VoucherEntryType | null;
  amount?: string | null;
  narration?: string | null;
}

export interface ReorderVoucherLinesRequest {
  line_ids: string[];
}

export interface SaveVoucherDraftRequest {
  voucher_id?: string | null;
  voucher_type_id: string;
  series_id: string;
  date: string;
  effective_date?: string | null;
  reference_no?: string | null;
  reference_date?: string | null;
  narration?: string | null;
  lines: VoucherLineInput[];
}

export interface VoucherFilter {
  voucher_type_id?: string | null;
  series_id?: string | null;
  status?: string | null;
  date_from?: string | null;
  date_to?: string | null;
  page?: number | null;
  page_size?: number | null;
}

export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
}



