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
