import { invoke } from '@tauri-apps/api/core';
import {
  HealthResponse,
  Company,
  CreateCompanyInput,
  UpdateCompanyInput,
  AccountGroup,
  CreateAccountGroupInput,
  UpdateAccountGroupInput,
  Ledger,
  CreateLedgerInput,
  UpdateLedgerInput,
  OpeningBalance,
  SetOpeningBalanceInput,
  ChartOfAccountsTree,
  ChartOfAccountsTreeNode,
  VoucherType,
  VoucherNumberSeries,
  VoucherNumberConfig,
  CreateVoucherSeriesRequest,
  UpdateVoucherSeriesRequest,
  CreateVoucherNumberConfigRequest,
  EffectiveNumberConfig,
  SequenceRecoveryReport,
  InspectSequenceRequest,
  ResyncSequenceRequest,
  Voucher,
  VoucherLine,
  VoucherWithLines,
  VoucherSummary,
  CreateVoucherDraftRequest,
  UpdateVoucherDraftRequest,
  AddVoucherLineRequest,
  UpdateVoucherLineRequest,
  ReorderVoucherLinesRequest,
  SaveVoucherDraftRequest,
  VoucherFilter,
  PaginatedResponse,
} from '../types';

function getWebPreviewMock(): HealthResponse {
  return {
    app_name: 'OpenTally (Web Preview)',
    app_version: '0.1.0-web',
    status: 'web_mode',
    timestamp: new Date().toISOString(),
    environment: 'development_browser',
    uptime_seconds: 0,
    database: {
      is_connected: false,
      engine: 'Browser Virtual Storage',
      journal_mode: 'none',
      foreign_keys_enabled: false,
      schema_version: '0.2.0-sprint1',
      encryption_enabled: false,
      encryption_status: 'unencrypted (browser preview)',
      key_management: 'unavailable_in_browser',
    },
    security: {
      encryption_active: false,
      encryption_engine: 'none (web mode)',
      key_management_status: 'unavailable',
      csp_enforced: false,
    },
    subsystems: [
      {
        name: 'tauri_ipc',
        status: 'disconnected',
        details: 'Running outside native Tauri window container',
      },
      {
        name: 'company_subsystem',
        status: 'operational',
        details: 'Company management and multi-tenant isolation active',
      },
    ],
  };
}

// In-memory mock storage for browser preview mode
let mockCompanies: Company[] = [
  {
    id: 'cmp_demo_01',
    name: 'Bharat Enterprises Ltd',
    mailing_name: 'Bharat Enterprises Head Office',
    country: 'India',
    state: 'Maharashtra',
    financial_year_begins: '2026-04-01',
    books_beginning_from: '2026-04-01',
    currency_symbol: '₹',
    currency_formal_name: 'INR',
    currency_decimal_places: 2,
    status: 'active',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  },
];
let mockActiveCompanyId: string | null =
  typeof localStorage !== 'undefined'
    ? localStorage.getItem('opentally_mock_active_company_id') || 'cmp_demo_01'
    : 'cmp_demo_01';

const isTauriEnv = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export async function fetchHealth(): Promise<HealthResponse> {
  if (!isTauriEnv()) {
    console.info('[OpenTally] Running in browser preview; using mock response.');
    return getWebPreviewMock();
  }

  try {
    return await invoke<HealthResponse>('get_app_health');
  } catch (error) {
    console.error('[OpenTally] Native Tauri IPC get_app_health failed:', error);
    throw error;
  }
}

export async function listCompanies(): Promise<Company[]> {
  if (!isTauriEnv()) {
    return [...mockCompanies];
  }
  return await invoke<Company[]>('list_companies');
}

export async function getCompany(id: string): Promise<Company> {
  if (!isTauriEnv()) {
    const found = mockCompanies.find((c) => c.id === id);
    if (!found) throw new Error(`Company with ID '${id}' not found`);
    return found;
  }
  return await invoke<Company>('get_company', { id });
}

export async function getActiveCompany(): Promise<Company | null> {
  if (!isTauriEnv()) {
    if (!mockActiveCompanyId) return null;
    return mockCompanies.find((c) => c.id === mockActiveCompanyId) || null;
  }
  return await invoke<Company | null>('get_active_company');
}

export async function selectCompany(id: string): Promise<Company> {
  if (!isTauriEnv()) {
    const found = mockCompanies.find((c) => c.id === id);
    if (!found) throw new Error(`Company with ID '${id}' not found`);
    if (found.status !== 'active') {
      throw new Error(`Cannot select company '${found.name}' with status '${found.status}'`);
    }
    mockActiveCompanyId = id;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('opentally_mock_active_company_id', id);
    }
    return found;
  }
  return await invoke<Company>('select_company', { id });
}

export async function createCompany(input: CreateCompanyInput): Promise<Company> {
  if (!isTauriEnv()) {
    const newCompany: Company = {
      id: `cmp_${Date.now()}`,
      name: input.name,
      mailing_name: input.mailing_name || input.name,
      country: input.country || 'India',
      state: input.state || 'Karnataka',
      financial_year_begins: input.financial_year_begins,
      books_beginning_from: input.books_beginning_from || input.financial_year_begins,
      currency_symbol: input.currency_symbol || '₹',
      currency_formal_name: input.currency_formal_name || 'INR',
      currency_decimal_places: input.currency_decimal_places ?? 2,
      status: 'active',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockCompanies.push(newCompany);
    mockActiveCompanyId = newCompany.id;
    return newCompany;
  }
  return await invoke<Company>('create_company', { req: input });
}

export async function updateCompany(id: string, input: UpdateCompanyInput): Promise<Company> {
  if (!isTauriEnv()) {
    const idx = mockCompanies.findIndex((c) => c.id === id);
    if (idx === -1) throw new Error(`Company with ID '${id}' not found`);
    mockCompanies[idx] = {
      ...mockCompanies[idx],
      mailing_name: input.mailing_name ?? mockCompanies[idx].mailing_name,
      country: input.country ?? mockCompanies[idx].country,
      state: input.state ?? mockCompanies[idx].state,
      currency_symbol: input.currency_symbol ?? mockCompanies[idx].currency_symbol,
      currency_formal_name: input.currency_formal_name ?? mockCompanies[idx].currency_formal_name,
      updated_at: new Date().toISOString(),
    };
    return mockCompanies[idx];
  }
  return await invoke<Company>('update_company', { id, req: input });
}

export async function archiveCompany(id: string): Promise<Company> {
  if (!isTauriEnv()) {
    const idx = mockCompanies.findIndex((c) => c.id === id);
    if (idx === -1) throw new Error(`Company with ID '${id}' not found`);
    mockCompanies[idx] = {
      ...mockCompanies[idx],
      status: 'archived',
      updated_at: new Date().toISOString(),
    };
    if (mockActiveCompanyId === id) {
      mockActiveCompanyId = null;
      if (typeof localStorage !== 'undefined') {
        localStorage.removeItem('opentally_mock_active_company_id');
      }
    }
    return mockCompanies[idx];
  }
  return await invoke<Company>('archive_company', { id });
}

// ---------------------------------------------------------------------------
// Chart of Accounts API (Stage 3)
// ---------------------------------------------------------------------------

let mockGroups: AccountGroup[] = [
  { id: 'grp_ca', company_id: 'cmp_demo_01', name: 'Current Assets', parent_id: null, nature: 'Assets', is_primary: true, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
  { id: 'grp_sd', company_id: 'cmp_demo_01', name: 'Sundry Debtors', parent_id: 'grp_ca', nature: 'Assets', is_primary: false, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
  { id: 'grp_ba', company_id: 'cmp_demo_01', name: 'Bank Accounts', parent_id: 'grp_ca', nature: 'Assets', is_primary: false, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
  { id: 'grp_ch', company_id: 'cmp_demo_01', name: 'Cash-in-Hand', parent_id: 'grp_ca', nature: 'Assets', is_primary: false, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
  { id: 'grp_cl', company_id: 'cmp_demo_01', name: 'Current Liabilities', parent_id: null, nature: 'Liabilities', is_primary: true, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
  { id: 'grp_sc', company_id: 'cmp_demo_01', name: 'Sundry Creditors', parent_id: 'grp_cl', nature: 'Liabilities', is_primary: false, is_predefined: true, is_hidden: false, affects_gross_profit: false, created_at: '', updated_at: '' },
];

let mockLedgers: Ledger[] = [
  { id: 'led_cash', company_id: 'cmp_demo_01', group_id: 'grp_ch', name: 'Cash', ledger_classification: 'cash', is_reserved: true, description: 'Default Cash Ledger', created_at: '', updated_at: '' },
  { id: 'led_demo1', company_id: 'cmp_demo_01', group_id: 'grp_sd', name: 'National Distributors', ledger_classification: 'standard', is_reserved: false, description: 'Wholesale client', created_at: '', updated_at: '' },
  { id: 'led_demo2', company_id: 'cmp_demo_01', group_id: 'grp_ba', name: 'State Bank Current A/c', ledger_classification: 'standard', is_reserved: false, description: 'Operating Account', created_at: '', updated_at: '' },
];

let mockOpeningBalances: Record<string, { balance_type: 'Dr' | 'Cr'; amount: string }> = {
  led_cash: { balance_type: 'Dr', amount: '15000.00' },
  led_demo1: { balance_type: 'Dr', amount: '85000.00' },
  led_demo2: { balance_type: 'Dr', amount: '100000.00' },
};

export async function getChartOfAccounts(): Promise<ChartOfAccountsTree> {
  if (!isTauriEnv()) {
    const buildNode = (g: AccountGroup): ChartOfAccountsTreeNode => {
      const childGroups = mockGroups.filter((cg) => cg.parent_id === g.id).map(buildNode);
      const childLedgers: ChartOfAccountsTreeNode[] = mockLedgers
        .filter((l) => l.group_id === g.id)
        .map((l) => {
          const opb = mockOpeningBalances[l.id];
          return {
            id: l.id,
            name: l.name,
            node_type: 'ledger',
            nature: g.nature,
            is_primary: false,
            is_predefined: false,
            is_reserved: l.is_reserved,
            parent_id: g.id,
            opening_balance: opb ? { balance_type: opb.balance_type, amount: opb.amount } : null,
            debit_total: opb && opb.balance_type === 'Dr' ? opb.amount : '0.00',
            credit_total: opb && opb.balance_type === 'Cr' ? opb.amount : '0.00',
            children: [],
          };
        });

      const allChildren = [...childGroups, ...childLedgers];
      let drTotal = 0;
      let crTotal = 0;
      for (const c of allChildren) {
        drTotal += parseFloat(c.debit_total || '0');
        crTotal += parseFloat(c.credit_total || '0');
      }

      return {
        id: g.id,
        name: g.name,
        node_type: 'group',
        nature: g.nature,
        is_primary: g.is_primary,
        is_predefined: g.is_predefined,
        is_reserved: false,
        parent_id: g.parent_id,
        opening_balance: null,
        debit_total: drTotal.toFixed(2),
        credit_total: crTotal.toFixed(2),
        children: allChildren,
      };
    };

    const rootNodes = mockGroups.filter((g) => g.is_primary).map(buildNode);
    return {
      nodes: rootNodes,
      equilibrium: {
        total_debit: '200000.00',
        total_credit: '0.00',
        difference: '200000.00',
        difference_type: 'Dr',
        is_balanced: false,
      },
    };
  }

  return await invoke<ChartOfAccountsTree>('get_chart_of_accounts');
}

export async function listAccountGroups(): Promise<AccountGroup[]> {
  if (!isTauriEnv()) {
    return [...mockGroups];
  }
  return await invoke<AccountGroup[]>('list_account_groups');
}

export async function createAccountGroup(req: CreateAccountGroupInput): Promise<AccountGroup> {
  if (!isTauriEnv()) {
    const parent = mockGroups.find((g) => g.id === req.parent_id);
    if (!parent) throw new Error('Parent group not found');
    const newGroup: AccountGroup = {
      id: `grp_${Date.now()}`,
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      name: req.name,
      parent_id: req.parent_id,
      nature: parent.nature,
      is_primary: false,
      is_predefined: false,
      is_hidden: false,
      affects_gross_profit: parent.affects_gross_profit,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockGroups.push(newGroup);
    return newGroup;
  }
  return await invoke<AccountGroup>('create_account_group', { req });
}

export async function updateAccountGroup(id: string, req: UpdateAccountGroupInput): Promise<AccountGroup> {
  if (!isTauriEnv()) {
    const idx = mockGroups.findIndex((g) => g.id === id);
    if (idx === -1) throw new Error('Group not found');
    mockGroups[idx].name = req.name;
    if (req.parent_id !== undefined) mockGroups[idx].parent_id = req.parent_id;
    return mockGroups[idx];
  }
  return await invoke<AccountGroup>('update_account_group', { id, req });
}

export async function deleteAccountGroup(id: string): Promise<void> {
  if (!isTauriEnv()) {
    const idx = mockGroups.findIndex((g) => g.id === id);
    if (idx !== -1) mockGroups.splice(idx, 1);
    return;
  }
  return await invoke<void>('delete_account_group', { id });
}

export async function listLedgers(groupId?: string, searchQuery?: string): Promise<Ledger[]> {
  if (!isTauriEnv()) {
    let result = [...mockLedgers];
    if (groupId) result = result.filter((l) => l.group_id === groupId);
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter((l) => l.name.toLowerCase().includes(q));
    }
    return result;
  }
  return await invoke<Ledger[]>('list_ledgers', { groupId, searchQuery });
}

export async function getLedger(id: string): Promise<Ledger> {
  if (!isTauriEnv()) {
    const found = mockLedgers.find((l) => l.id === id);
    if (!found) throw new Error(`Ledger with ID '${id}' not found`);
    return found;
  }
  return await invoke<Ledger>('get_ledger', { id });
}

export async function createLedger(req: CreateLedgerInput): Promise<Ledger> {
  if (!isTauriEnv()) {
    const newLedger: Ledger = {
      id: `led_${Date.now()}`,
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      group_id: req.group_id,
      name: req.name,
      ledger_classification: 'standard',
      is_reserved: false,
      description: req.description || null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockLedgers.push(newLedger);
    if (req.opening_balance) {
      mockOpeningBalances[newLedger.id] = {
        balance_type: req.opening_balance.balance_type,
        amount: req.opening_balance.amount,
      };
    }
    return newLedger;
  }
  return await invoke<Ledger>('create_ledger', { req });
}

export async function updateLedger(id: string, req: UpdateLedgerInput): Promise<Ledger> {
  if (!isTauriEnv()) {
    const idx = mockLedgers.findIndex((l) => l.id === id);
    if (idx === -1) throw new Error('Ledger not found');
    mockLedgers[idx].name = req.name;
    if (req.group_id) mockLedgers[idx].group_id = req.group_id;
    if (req.description !== undefined) mockLedgers[idx].description = req.description;
    return mockLedgers[idx];
  }
  return await invoke<Ledger>('update_ledger', { id, req });
}

export async function deleteLedger(id: string): Promise<void> {
  if (!isTauriEnv()) {
    const idx = mockLedgers.findIndex((l) => l.id === id);
    if (idx !== -1) mockLedgers.splice(idx, 1);
    delete mockOpeningBalances[id];
    return;
  }
  return await invoke<void>('delete_ledger', { id });
}

export async function setOpeningBalance(req: SetOpeningBalanceInput): Promise<OpeningBalance | null> {
  if (!isTauriEnv()) {
    const amt = parseFloat(req.amount);
    if (amt === 0) {
      delete mockOpeningBalances[req.ledger_id];
      return null;
    }
    mockOpeningBalances[req.ledger_id] = {
      balance_type: req.balance_type,
      amount: req.amount,
    };
    return {
      id: `opb_${Date.now()}`,
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      ledger_id: req.ledger_id,
      financial_year_begins: '2026-04-01',
      balance_type: req.balance_type,
      amount: req.amount,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<OpeningBalance | null>('set_opening_balance', { req });
}

// ---------------------------------------------------------------------------
// Voucher Types & Series Masters (Sprint 2 Stage 2)
// ---------------------------------------------------------------------------

const MOCK_PREDEFINED_VOUCHER_TYPES: VoucherType[] = [
  { id: 'vt_contra', company_id: 'cmp_demo_01', name: 'Contra', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_payment', company_id: 'cmp_demo_01', name: 'Payment', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_receipt', company_id: 'cmp_demo_01', name: 'Receipt', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_jnl', company_id: 'cmp_demo_01', name: 'Journal', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_sales', company_id: 'cmp_demo_01', name: 'Sales', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_pur', company_id: 'cmp_demo_01', name: 'Purchase', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_credit_note', company_id: 'cmp_demo_01', name: 'Credit Note', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_debit_note', company_id: 'cmp_demo_01', name: 'Debit Note', category: 'accounting', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_reversing_journal', company_id: 'cmp_demo_01', name: 'Reversing Journal', category: 'scenario', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_memorandum', company_id: 'cmp_demo_01', name: 'Memorandum', category: 'non_financial', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_sales_order', company_id: 'cmp_demo_01', name: 'Sales Order', category: 'order', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_purchase_order', company_id: 'cmp_demo_01', name: 'Purchase Order', category: 'order', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_delivery_note', company_id: 'cmp_demo_01', name: 'Delivery Note', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_receipt_note', company_id: 'cmp_demo_01', name: 'Receipt Note', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_rejections_in', company_id: 'cmp_demo_01', name: 'Rejections In', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_rejections_out', company_id: 'cmp_demo_01', name: 'Rejections Out', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_stock_journal', company_id: 'cmp_demo_01', name: 'Stock Journal', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_physical_stock', company_id: 'cmp_demo_01', name: 'Physical Stock', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_material_in', company_id: 'cmp_demo_01', name: 'Material In', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_material_out', company_id: 'cmp_demo_01', name: 'Material Out', category: 'inventory', parent_type: null, affects_ledger_balance: false, affects_inventory: true, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_job_work_in_order', company_id: 'cmp_demo_01', name: 'Job Work In Order', category: 'order', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_job_work_out_order', company_id: 'cmp_demo_01', name: 'Job Work Out Order', category: 'order', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_attendance', company_id: 'cmp_demo_01', name: 'Attendance', category: 'payroll', parent_type: null, affects_ledger_balance: false, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
  { id: 'vt_payroll', company_id: 'cmp_demo_01', name: 'Payroll', category: 'payroll', parent_type: null, affects_ledger_balance: true, affects_inventory: false, is_predefined: true, is_active: true, numbering_method: 'automatic', created_at: new Date().toISOString(), updated_at: new Date().toISOString() },
];

let mockSeries: VoucherNumberSeries[] = [
  {
    id: 'ser_demo_sales_01',
    company_id: 'cmp_demo_01',
    voucher_type_id: 'vt_sales',
    series_name: 'Main Tax Invoice',
    series_code: 'TAX',
    numbering_method: 'automatic',
    restart_period: 'yearly',
    is_active: true,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  },
];

let mockConfigs: VoucherNumberConfig[] = [
  {
    id: 'cfg_demo_01',
    company_id: 'cmp_demo_01',
    voucher_type_id: 'vt_sales',
    series_id: 'ser_demo_sales_01',
    applicable_from: '2026-04-01',
    prefix: 'INV/{YY-YY}/',
    suffix: '',
    start_number: 1,
    width: 4,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  },
];

export async function listVoucherTypes(): Promise<VoucherType[]> {
  if (!isTauriEnv()) {
    return [...MOCK_PREDEFINED_VOUCHER_TYPES];
  }
  return await invoke<VoucherType[]>('list_voucher_types');
}

export async function getVoucherType(id: string): Promise<VoucherType> {
  if (!isTauriEnv()) {
    const found = MOCK_PREDEFINED_VOUCHER_TYPES.find((vt) => vt.id === id);
    if (!found) throw new Error(`Voucher type with id '${id}' not found`);
    return found;
  }
  return await invoke<VoucherType>('get_voucher_type', { id });
}

export async function listVoucherSeries(voucherTypeId: string): Promise<VoucherNumberSeries[]> {
  if (!isTauriEnv()) {
    return mockSeries.filter((s) => s.voucher_type_id === voucherTypeId);
  }
  return await invoke<VoucherNumberSeries[]>('list_voucher_series', { voucherTypeId });
}

export async function getVoucherSeries(id: string): Promise<VoucherNumberSeries> {
  if (!isTauriEnv()) {
    const found = mockSeries.find((s) => s.id === id);
    if (!found) throw new Error(`Voucher series '${id}' not found`);
    return found;
  }
  return await invoke<VoucherNumberSeries>('get_voucher_series', { id });
}

export async function createVoucherSeries(req: CreateVoucherSeriesRequest): Promise<VoucherNumberSeries> {
  if (!isTauriEnv()) {
    const existing = mockSeries.find(
      (s) => s.voucher_type_id === req.voucher_type_id && s.series_code.toUpperCase() === req.series_code.toUpperCase()
    );
    if (existing) {
      throw new Error(`Series code '${req.series_code}' is already used in this voucher type`);
    }
    const newSeries: VoucherNumberSeries = {
      id: `ser_${Date.now()}`,
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      voucher_type_id: req.voucher_type_id,
      series_name: req.series_name,
      series_code: req.series_code.toUpperCase(),
      numbering_method: req.numbering_method,
      restart_period: req.restart_period,
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockSeries.push(newSeries);

    if (req.applicable_from) {
      mockConfigs.push({
        id: `cfg_${Date.now()}`,
        company_id: mockActiveCompanyId || 'cmp_demo_01',
        voucher_type_id: req.voucher_type_id,
        series_id: newSeries.id,
        applicable_from: req.applicable_from,
        prefix: req.prefix || '',
        suffix: req.suffix || '',
        start_number: req.start_number || 1,
        width: req.width || 4,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      });
    }

    return newSeries;
  }
  return await invoke<VoucherNumberSeries>('create_voucher_series', { req });
}

export async function updateVoucherSeries(id: string, req: UpdateVoucherSeriesRequest): Promise<VoucherNumberSeries> {
  if (!isTauriEnv()) {
    const idx = mockSeries.findIndex((s) => s.id === id);
    if (idx === -1) throw new Error('Series not found');
    if (req.series_name !== undefined) mockSeries[idx].series_name = req.series_name;
    if (req.is_active !== undefined) mockSeries[idx].is_active = req.is_active;
    mockSeries[idx].updated_at = new Date().toISOString();
    return mockSeries[idx];
  }
  return await invoke<VoucherNumberSeries>('update_voucher_series', { id, req });
}

export async function deactivateVoucherSeries(id: string): Promise<VoucherNumberSeries> {
  if (!isTauriEnv()) {
    const idx = mockSeries.findIndex((s) => s.id === id);
    if (idx === -1) throw new Error('Series not found');
    mockSeries[idx].is_active = false;
    mockSeries[idx].updated_at = new Date().toISOString();
    return mockSeries[idx];
  }
  return await invoke<VoucherNumberSeries>('deactivate_voucher_series', { id });
}

export async function deleteVoucherSeries(id: string): Promise<void> {
  if (!isTauriEnv()) {
    const idx = mockSeries.findIndex((s) => s.id === id);
    if (idx !== -1) mockSeries.splice(idx, 1);
    mockConfigs = mockConfigs.filter((c) => c.series_id !== id);
    return;
  }
  return await invoke<void>('delete_voucher_series', { id });
}

export async function createVoucherNumberConfig(
  seriesId: string,
  req: CreateVoucherNumberConfigRequest
): Promise<VoucherNumberConfig> {
  if (!isTauriEnv()) {
    const series = mockSeries.find((s) => s.id === seriesId);
    if (!series) throw new Error('Series not found');
    const newCfg: VoucherNumberConfig = {
      id: `cfg_${Date.now()}`,
      company_id: series.company_id,
      voucher_type_id: series.voucher_type_id,
      series_id: seriesId,
      applicable_from: req.applicable_from,
      prefix: req.prefix || '',
      suffix: req.suffix || '',
      start_number: req.start_number || 1,
      width: req.width || 4,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockConfigs.push(newCfg);
    return newCfg;
  }
  return await invoke<VoucherNumberConfig>('create_voucher_number_config', { seriesId, req });
}

export async function listVoucherNumberConfigs(seriesId: string): Promise<VoucherNumberConfig[]> {
  if (!isTauriEnv()) {
    return mockConfigs.filter((c) => c.series_id === seriesId);
  }
  return await invoke<VoucherNumberConfig[]>('list_voucher_number_configs', { seriesId });
}

export async function resolveEffectiveVoucherConfig(
  seriesId: string,
  voucherDate: string
): Promise<EffectiveNumberConfig> {
  if (!isTauriEnv()) {
    const series = mockSeries.find((s) => s.id === seriesId);
    if (!series) throw new Error(`Series '${seriesId}' not found`);
    const applicable = mockConfigs
      .filter((c) => c.series_id === series.id && c.applicable_from <= voucherDate)
      .sort((a, b) => b.applicable_from.localeCompare(a.applicable_from));
    if (applicable.length === 0) {
      throw new Error(`No effective configuration found for series '${seriesId}' on date '${voucherDate}'`);
    }
    const cfg = applicable[0];
    return {
      config_id: cfg.id,
      voucher_type_id: series.voucher_type_id,
      series_id: series.id,
      prefix: cfg.prefix,
      suffix: cfg.suffix,
      width: cfg.width,
      start_number: cfg.start_number,
      restart_period: series.restart_period,
      applicable_from: cfg.applicable_from,
      period_start: '2026-04-01',
    };
  }
  return await invoke<EffectiveNumberConfig>('resolve_effective_voucher_config', {
    seriesId,
    voucherDate,
  });
}

export async function inspectVoucherSequence(req: InspectSequenceRequest): Promise<SequenceRecoveryReport> {
  if (!isTauriEnv()) {
    const series = mockSeries.find((s) => s.id === req.series_id);
    return {
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      voucher_type_id: series?.voucher_type_id || 'vt_sales',
      series_id: req.series_id,
      period_start: req.period_start,
      start_number_baseline: 1,
      previous_next_number: 1,
      new_next_number: 1,
      max_allocated: 0,
      total_permanent_allocations: 0,
      historical_gaps: [],
      counter_tail_gaps: [],
      counter_updated: false,
      operator_id: 'browser_user',
      resolution_mode: 'strict',
      resolution_reason: null,
    };
  }
  return await invoke<SequenceRecoveryReport>('inspect_voucher_sequence', { req });
}

export async function resyncVoucherSequence(req: ResyncSequenceRequest): Promise<SequenceRecoveryReport> {
  if (!isTauriEnv()) {
    const series = mockSeries.find((s) => s.id === req.series_id);
    return {
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      voucher_type_id: series?.voucher_type_id || 'vt_sales',
      series_id: req.series_id,
      period_start: req.period_start,
      start_number_baseline: 1,
      previous_next_number: 1,
      new_next_number: 1,
      max_allocated: 0,
      total_permanent_allocations: 0,
      historical_gaps: [],
      counter_tail_gaps: [],
      counter_updated: false,
      operator_id: 'browser_user',
      resolution_mode: req.resolution_mode,
      resolution_reason: req.resolution_reason || null,
    };
  }
  return await invoke<SequenceRecoveryReport>('resync_voucher_sequence', { req });
}

// ---------------------------------------------------------------------------
// Voucher Creation & Draft Management API (Sprint 2 Stage 4)
// ---------------------------------------------------------------------------

let mockVouchers: Voucher[] = [];
let mockVoucherLines: VoucherLine[] = [];

function buildMockComposite(voucher: Voucher, lines: VoucherLine[]): VoucherWithLines {
  let drTotal = 0;
  let crTotal = 0;
  for (const l of lines) {
    const val = parseFloat(l.amount) || 0;
    if (l.entry_type === 'Dr') drTotal += val;
    else if (l.entry_type === 'Cr') crTotal += val;
  }
  const diff = Math.abs(drTotal - crTotal);
  const isBalanced = drTotal === crTotal && lines.length > 0 && drTotal > 0;

  return {
    voucher,
    lines,
    debit_total: drTotal.toFixed(2),
    credit_total: crTotal.toFixed(2),
    difference: diff.toFixed(2),
    is_balanced: isBalanced,
  };
}

export async function createVoucherDraft(req: CreateVoucherDraftRequest): Promise<VoucherWithLines> {
  if (!isTauriEnv()) {
    const vid = `vch_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
    const series = mockSeries.find((s) => s.id === req.series_id);
    const voucher: Voucher = {
      id: vid,
      company_id: mockActiveCompanyId || 'cmp_demo_01',
      voucher_type_id: req.voucher_type_id,
      series_id: req.series_id,
      numbering_method: series?.numbering_method || 'automatic',
      period_start: null,
      sequence_number: null,
      voucher_number: `DRAFT-${vid.substring(4)}`,
      date: req.date,
      effective_date: req.effective_date || req.date,
      status: 'draft',
      reference_no: req.reference_no || null,
      reference_date: req.reference_date || null,
      narration: req.narration || null,
      total_amount: null,
      reversal_of_voucher_id: null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockVouchers.push(voucher);

    const lines: VoucherLine[] = [];
    if (req.lines) {
      req.lines.forEach((l, idx) => {
        const lid = l.id || `line_${Date.now()}_${idx}`;
        const line: VoucherLine = {
          id: lid,
          company_id: voucher.company_id,
          voucher_id: vid,
          line_number: idx + 1,
          ledger_id: l.ledger_id,
          entry_type: l.entry_type,
          amount: parseFloat(l.amount).toFixed(2),
          narration: l.narration || null,
          created_at: new Date().toISOString(),
        };
        mockVoucherLines.push(line);
        lines.push(line);
      });
    }

    return buildMockComposite(voucher, lines);
  }
  return await invoke<VoucherWithLines>('create_voucher_draft', { req });
}

export async function getVoucherDraft(voucherId: string): Promise<VoucherWithLines> {
  if (!isTauriEnv()) {
    const v = mockVouchers.find((item) => item.id === voucherId);
    if (!v) throw new Error(`Draft voucher '${voucherId}' not found`);
    const lines = mockVoucherLines
      .filter((l) => l.voucher_id === voucherId)
      .sort((a, b) => a.line_number - b.line_number);
    return buildMockComposite(v, lines);
  }
  return await invoke<VoucherWithLines>('get_voucher_draft', { voucherId });
}

export async function listVoucherDrafts(
  filter: VoucherFilter = {}
): Promise<PaginatedResponse<VoucherSummary>> {
  if (!isTauriEnv()) {
    let filtered = mockVouchers.filter((v) => v.status === (filter.status || 'draft'));
    if (filter.voucher_type_id) {
      filtered = filtered.filter((v) => v.voucher_type_id === filter.voucher_type_id);
    }
    if (filter.series_id) {
      filtered = filtered.filter((v) => v.series_id === filter.series_id);
    }
    if (filter.date_from) {
      filtered = filtered.filter((v) => v.date >= filter.date_from!);
    }
    if (filter.date_to) {
      filtered = filtered.filter((v) => v.date <= filter.date_to!);
    }

    const page = filter.page || 1;
    const pageSize = filter.page_size || 20;
    const total = filtered.length;
    const totalPages = total === 0 ? 1 : Math.ceil(total / pageSize);
    const offset = (page - 1) * pageSize;
    const pageItems = filtered.slice(offset, offset + pageSize);

    const items: VoucherSummary[] = pageItems.map((v) => {
      const vt = MOCK_PREDEFINED_VOUCHER_TYPES.find((t) => t.id === v.voucher_type_id);
      const s = mockSeries.find((ser) => ser.id === v.series_id);
      const lineCount = mockVoucherLines.filter((l) => l.voucher_id === v.id).length;
      return {
        id: v.id,
        company_id: v.company_id,
        voucher_type_id: v.voucher_type_id,
        voucher_type_name: vt?.name || v.voucher_type_id,
        series_id: v.series_id,
        series_name: s?.series_name || v.series_id,
        voucher_number: v.voucher_number,
        date: v.date,
        status: v.status,
        total_amount: v.total_amount || null,
        line_count: lineCount,
        updated_at: v.updated_at,
      };
    });

    return {
      items,
      total,
      page,
      page_size: pageSize,
      total_pages: totalPages,
    };
  }
  return await invoke<PaginatedResponse<VoucherSummary>>('list_voucher_drafts', { filter });
}

export async function updateVoucherDraft(
  voucherId: string,
  req: UpdateVoucherDraftRequest
): Promise<Voucher> {
  if (!isTauriEnv()) {
    const idx = mockVouchers.findIndex((v) => v.id === voucherId);
    if (idx === -1) throw new Error(`Draft voucher '${voucherId}' not found`);
    const curr = mockVouchers[idx];
    if (curr.status !== 'draft') throw new Error(`Voucher '${voucherId}' is not in draft status`);

    mockVouchers[idx] = {
      ...curr,
      voucher_type_id: req.voucher_type_id || curr.voucher_type_id,
      series_id: req.series_id || curr.series_id,
      date: req.date || curr.date,
      effective_date: req.effective_date || curr.effective_date,
      reference_no: req.reference_no !== undefined ? req.reference_no : curr.reference_no,
      reference_date: req.reference_date !== undefined ? req.reference_date : curr.reference_date,
      narration: req.narration !== undefined ? req.narration : curr.narration,
      updated_at: new Date().toISOString(),
    };
    return mockVouchers[idx];
  }
  return await invoke<Voucher>('update_voucher_draft', { voucherId, req });
}

export async function deleteVoucherDraft(voucherId: string): Promise<void> {
  if (!isTauriEnv()) {
    const idx = mockVouchers.findIndex((v) => v.id === voucherId);
    if (idx === -1) throw new Error(`Draft voucher '${voucherId}' not found`);
    if (mockVouchers[idx].status !== 'draft') {
      throw new Error(`Cannot delete voucher with status '${mockVouchers[idx].status}'`);
    }
    mockVouchers.splice(idx, 1);
    mockVoucherLines = mockVoucherLines.filter((l) => l.voucher_id !== voucherId);
    return;
  }
  return await invoke<void>('delete_voucher_draft', { voucherId });
}

export async function addVoucherLine(
  voucherId: string,
  req: AddVoucherLineRequest
): Promise<VoucherLine> {
  if (!isTauriEnv()) {
    const v = mockVouchers.find((item) => item.id === voucherId);
    if (!v) throw new Error(`Draft voucher '${voucherId}' not found`);
    if (v.status !== 'draft') throw new Error(`Cannot add lines to non-draft voucher`);

    const existingLines = mockVoucherLines
      .filter((l) => l.voucher_id === voucherId)
      .sort((a, b) => a.line_number - b.line_number);
    const maxLine = existingLines.length > 0 ? existingLines[existingLines.length - 1].line_number : 0;
    const targetLineNum = req.line_number && req.line_number > 0 && req.line_number <= maxLine
      ? req.line_number
      : maxLine + 1;

    if (req.line_number && req.line_number <= maxLine) {
      existingLines.forEach((l) => {
        if (l.line_number >= targetLineNum) {
          l.line_number += 1;
        }
      });
    }

    const newLine: VoucherLine = {
      id: `line_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`,
      company_id: v.company_id,
      voucher_id: voucherId,
      line_number: targetLineNum,
      ledger_id: req.ledger_id,
      entry_type: req.entry_type,
      amount: parseFloat(req.amount).toFixed(2),
      narration: req.narration || null,
      created_at: new Date().toISOString(),
    };
    mockVoucherLines.push(newLine);
    return newLine;
  }
  return await invoke<VoucherLine>('add_voucher_line', { voucherId, req });
}

export async function updateVoucherLine(
  voucherId: string,
  lineId: string,
  req: UpdateVoucherLineRequest
): Promise<VoucherLine> {
  if (!isTauriEnv()) {
    const idx = mockVoucherLines.findIndex((l) => l.voucher_id === voucherId && l.id === lineId);
    if (idx === -1) throw new Error(`Voucher line '${lineId}' not found`);
    const curr = mockVoucherLines[idx];
    mockVoucherLines[idx] = {
      ...curr,
      ledger_id: req.ledger_id || curr.ledger_id,
      entry_type: req.entry_type || curr.entry_type,
      amount: req.amount ? parseFloat(req.amount).toFixed(2) : curr.amount,
      narration: req.narration !== undefined ? req.narration : curr.narration,
    };
    return mockVoucherLines[idx];
  }
  return await invoke<VoucherLine>('update_voucher_line', { voucherId, lineId, req });
}

export async function deleteVoucherLine(voucherId: string, lineId: string): Promise<void> {
  if (!isTauriEnv()) {
    const idx = mockVoucherLines.findIndex((l) => l.voucher_id === voucherId && l.id === lineId);
    if (idx === -1) throw new Error(`Voucher line '${lineId}' not found`);
    mockVoucherLines.splice(idx, 1);

    const remaining = mockVoucherLines
      .filter((l) => l.voucher_id === voucherId)
      .sort((a, b) => a.line_number - b.line_number);
    remaining.forEach((l, i) => {
      l.line_number = i + 1;
    });
    return;
  }
  return await invoke<void>('delete_voucher_line', { voucherId, lineId });
}

export async function reorderVoucherLines(
  voucherId: string,
  req: ReorderVoucherLinesRequest
): Promise<VoucherLine[]> {
  if (!isTauriEnv()) {
    const lines = mockVoucherLines.filter((l) => l.voucher_id === voucherId);
    req.line_ids.forEach((lid, i) => {
      const found = lines.find((l) => l.id === lid);
      if (found) found.line_number = i + 1;
    });
    return mockVoucherLines
      .filter((l) => l.voucher_id === voucherId)
      .sort((a, b) => a.line_number - b.line_number);
  }
  return await invoke<VoucherLine[]>('reorder_voucher_lines', { voucherId, req });
}

export async function saveVoucherDraft(
  req: SaveVoucherDraftRequest
): Promise<VoucherWithLines> {
  if (!isTauriEnv()) {
    let voucher: Voucher;
    if (req.voucher_id) {
      const idx = mockVouchers.findIndex((v) => v.id === req.voucher_id);
      if (idx === -1) throw new Error(`Draft voucher '${req.voucher_id}' not found`);
      mockVouchers[idx] = {
        ...mockVouchers[idx],
        voucher_type_id: req.voucher_type_id,
        series_id: req.series_id,
        date: req.date,
        effective_date: req.effective_date || req.date,
        reference_no: req.reference_no || null,
        reference_date: req.reference_date || null,
        narration: req.narration || null,
        updated_at: new Date().toISOString(),
      };
      voucher = mockVouchers[idx];
      mockVoucherLines = mockVoucherLines.filter((l) => l.voucher_id !== req.voucher_id);
    } else {
      const vid = `vch_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
      const series = mockSeries.find((s) => s.id === req.series_id);
      voucher = {
        id: vid,
        company_id: mockActiveCompanyId || 'cmp_demo_01',
        voucher_type_id: req.voucher_type_id,
        series_id: req.series_id,
        numbering_method: series?.numbering_method || 'automatic',
        period_start: null,
        sequence_number: null,
        voucher_number: `DRAFT-${vid.substring(4)}`,
        date: req.date,
        effective_date: req.effective_date || req.date,
        status: 'draft',
        reference_no: req.reference_no || null,
        reference_date: req.reference_date || null,
        narration: req.narration || null,
        total_amount: null,
        reversal_of_voucher_id: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      mockVouchers.push(voucher);
    }

    const lines: VoucherLine[] = req.lines.map((l, idx) => {
      const lid = l.id || `line_${Date.now()}_${idx}`;
      const line: VoucherLine = {
        id: lid,
        company_id: voucher.company_id,
        voucher_id: voucher.id,
        line_number: idx + 1,
        ledger_id: l.ledger_id,
        entry_type: l.entry_type,
        amount: parseFloat(l.amount).toFixed(2),
        narration: l.narration || null,
        created_at: new Date().toISOString(),
      };
      mockVoucherLines.push(line);
      return line;
    });

    return buildMockComposite(voucher, lines);
  }
  return await invoke<VoucherWithLines>('save_voucher_draft', { req });
}



