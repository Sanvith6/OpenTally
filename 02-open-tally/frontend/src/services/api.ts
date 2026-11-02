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
