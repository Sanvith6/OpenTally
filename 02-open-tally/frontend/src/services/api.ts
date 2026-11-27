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
  DayBookRequest,
  DayBookEntry,
  LedgerReportRequest,
  LedgerReport,
  TrialBalanceRequest,
  TrialBalanceReport,
  ProfitAndLossRequest,
  ProfitAndLossReport,
  BalanceSheetRequest,
  BalanceSheetReport,
  CashFlowRequest,
  CashFlowReport,
  ReportPage,
  ExportFormat,
  VoucherSearchFilter,
  VoucherExportPreviewResult,
  StockGroup, StockCategory, Unit, StockItem, Godown,
  StockGroupRequest, StockCategoryRequest, UnitRequest, StockItemRequest, GodownRequest,
  StockVoucher, StockVoucherLine, StockVoucherRequest, StockVoucherListFilter, StockBalanceRequest, StockBalanceRow,
  ValuationConfig, ValuationRequest, ValuationRow, FifoLayer, ValuationHistoryRow,
  InventoryReportFilter, StockSummaryReport, StockRegisterReport, StockLedgerReport, ReconciliationFinding, InventoryValuationAttribution, InventoryValuationAttributionDetails,
  InventoryAccountMapping, InventoryAccountMappingRequest, InventoryAccountingPreview, InventoryAccountingIntegration,
  PartyProfile, PartyRequest,
  SalesDocument, SalesDocumentRequest, SalesDocumentWithLines,
  PurchaseDocument, PurchaseDocumentRequest, PurchaseDocumentWithLines, CommercialPostingResult,
  BillAllocation, BillAllocationInput, PendingBill, AgingReport,
  CompanyGstProfile, LedgerGstProfile, HsnSacCode, ItemGstMapping, GstLedgerMapping, GstCalculationResult, Gstr1SummaryReport, Gstr3bSummaryReport,
  GotoSearchResult, VoucherFullDetail,
  BackupManifest, RestoreResult, BackupStatus, CommercialPostingInput,
  TallyXmlImportPreview, TallyXmlImportResult,
  CompanyFeaturesDto, SaveCompanyFeaturesInput,
  BankLedgerItem, BankReconciliationStatement, ReconcileEntryInput, ReconcileResult, BankStatementMatchPreview,
  CompanyTdsProfile, SaveCompanyTdsProfileInput, LedgerTdsProfile, SaveLedgerTdsProfileInput,
  TdsNatureOfPayment, CreateTdsNatureOfPaymentInput, TdsCalculationPreviewInput, TdsCalculationPreviewResult,
  RecordTdsChallanPaymentInput, TdsOutstandingReport, Form26qSummary,
  Gstr2bReconciliationReport, TransportDetailsInput, VoucherEinvoiceMetadataInput, VoucherEinvoiceMetadataDto,
  GodownSummaryReport,
  StockItemBatch,
  StockItemBatchRequest,
  BatchExpiryReportRow,
  StockJournalRequest,
  StockJournalResponse,
  CostCategory,
  CostCategoryRequest,
  CostCentre,
  CostCentreRequest,
  VoucherCostAllocationItem,
  CostCentreBreakupReport,
  RatioAnalysisReport,
  Budget,
  BudgetRequest,
  BudgetDetails,
  BudgetVarianceReport,
  FinancialYearClosingRecord,
  YearEndClosingPreview,
  AuditCertificationReport,
  Currency,
  CreateCurrencyInput,
  ExchangeRate,
  SetExchangeRateInput,
  EffectiveRateDto,
  OpenForeignBill,
  SettleForeignBillInput,
  RealizedForexResult,
  UnrealizedForexReport,
  ChequeTemplateDto,
  UpsertChequeTemplateInput,
  CurrencySeedSummary,
  CompanyGspConfigDto,
  SaveCompanyGspConfigInput,
  GspEinvoiceResponse,
  GspEwayBillResponse,
  CustomReportDefinitionDto,
  SaveCustomReportInput,
  CustomReportExecutionResult,
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
  if (isLanClientMode()) {
    return await callLanRpc<Company[]>('list_companies');
  }
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

export async function listParties(search?: string): Promise<PartyProfile[]> {
  if (!isTauriEnv()) return [];
  return invoke<PartyProfile[]>('list_parties', { search: search || null });
}
export async function createParty(request: PartyRequest): Promise<PartyProfile> {
  if (!isTauriEnv()) throw new Error('Party management requires the native OpenTally application');
  return invoke<PartyProfile>('create_party', { request });
}
export async function updateParty(id: string, request: PartyRequest): Promise<PartyProfile> {
  if (!isTauriEnv()) throw new Error('Party management requires the native OpenTally application');
  return invoke<PartyProfile>('update_party', { id, request });
}
export async function setPartyActive(id: string, active: boolean): Promise<PartyProfile> {
  if (!isTauriEnv()) throw new Error('Party management requires the native OpenTally application');
  return invoke<PartyProfile>('set_party_active', { id, active });
}

export async function listSalesDrafts(search?: string): Promise<SalesDocument[]> {
  if (!isTauriEnv()) return [];
  return invoke<SalesDocument[]>('list_sales_drafts', { search: search || null });
}
export async function getSalesDraft(id: string): Promise<SalesDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Sales drafts require the native OpenTally application');
  return invoke<SalesDocumentWithLines>('get_sales_draft', { id });
}
export async function createSalesDraft(request: SalesDocumentRequest): Promise<SalesDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Sales drafts require the native OpenTally application');
  return invoke<SalesDocumentWithLines>('create_sales_draft', { request });
}
export async function updateSalesDraft(id: string, request: SalesDocumentRequest): Promise<SalesDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Sales drafts require the native OpenTally application');
  return invoke<SalesDocumentWithLines>('update_sales_draft', { id, request });
}
export async function deleteSalesDraft(id: string): Promise<void> {
  if (!isTauriEnv()) throw new Error('Sales drafts require the native OpenTally application');
  return invoke<void>('delete_sales_draft', { id });
}
export async function reorderSalesLines(id: string, lineIds: string[]): Promise<SalesDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Sales drafts require the native OpenTally application');
  return invoke<SalesDocumentWithLines>('reorder_sales_lines', { id, lineIds });
}
export async function postSalesDocument(id: string, salesLedgerId?: string): Promise<CommercialPostingResult> {
  if (!isTauriEnv()) throw new Error('Posting sales documents requires the native OpenTally application');
  return invoke<CommercialPostingResult>('post_sales_document', { id, salesLedgerId: salesLedgerId || null });
}

export async function listPurchaseDrafts(search?: string): Promise<PurchaseDocument[]> {
  if (!isTauriEnv()) return [];
  return invoke<PurchaseDocument[]>('list_purchase_drafts', { search: search || null });
}
export async function getPurchaseDraft(id: string): Promise<PurchaseDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Purchase drafts require the native OpenTally application');
  return invoke<PurchaseDocumentWithLines>('get_purchase_draft', { id });
}
export async function createPurchaseDraft(request: PurchaseDocumentRequest): Promise<PurchaseDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Purchase drafts require the native OpenTally application');
  return invoke<PurchaseDocumentWithLines>('create_purchase_draft', { request });
}
export async function updatePurchaseDraft(id: string, request: PurchaseDocumentRequest): Promise<PurchaseDocumentWithLines> {
  if (!isTauriEnv()) throw new Error('Purchase drafts require the native OpenTally application');
  return invoke<PurchaseDocumentWithLines>('update_purchase_draft', { id, request });
}
export async function deletePurchaseDraft(id: string): Promise<void> {
  if (!isTauriEnv()) throw new Error('Purchase drafts require the native OpenTally application');
  return invoke<void>('delete_purchase_draft', { id });
}
export async function postPurchaseDocument(
  id: string,
  purchaseLedgerId?: string,
  tdsSectionCode?: string
): Promise<CommercialPostingResult> {
  if (isLanClientMode()) {
    return await callLanRpc<CommercialPostingResult>('post_purchase_document', {
      id,
      purchase_ledger_id: purchaseLedgerId || null,
      tds_section_code: tdsSectionCode || null,
    });
  }
  if (!isTauriEnv()) throw new Error('Posting purchase documents requires the native OpenTally application');
  return invoke<CommercialPostingResult>('post_purchase_document', {
    id,
    purchaseLedgerId: purchaseLedgerId || null,
    tdsSectionCode: tdsSectionCode || null,
  });
}

export async function getDayBook(request: DayBookRequest): Promise<ReportPage<DayBookEntry>> {
  if (isLanClientMode()) {
    return await callLanRpc<ReportPage<DayBookEntry>>('get_daybook', request as unknown as Record<string, unknown>);
  }
  if (!isTauriEnv()) {
    return { items: [], total: 0, page: request.page.page, page_size: request.page.page_size, total_pages: 0 };
  }
  return invoke<ReportPage<DayBookEntry>>('day_book', { request });
}

export async function getLedgerReport(request: LedgerReportRequest): Promise<LedgerReport> {
  if (isLanClientMode()) {
    return await callLanRpc<LedgerReport>('get_ledger_report', request as unknown as Record<string, unknown>);
  }
  if (!isTauriEnv()) {
    throw new Error('Ledger reports require the native OpenTally application');
  }
  return invoke<LedgerReport>('ledger_report', { request });
}

export async function getTrialBalance(request: TrialBalanceRequest): Promise<TrialBalanceReport> {
  if (isLanClientMode()) {
    return await callLanRpc<TrialBalanceReport>('get_trial_balance', request as unknown as Record<string, unknown>);
  }
  if (!isTauriEnv()) {
    return { as_of_date: request.as_of_date, rows: [], total_debit: '0', total_credit: '0', difference: '0', ledger_count: 0 };
  }
  return invoke<TrialBalanceReport>('trial_balance', { request });
}

export async function getProfitAndLoss(request: ProfitAndLossRequest): Promise<ProfitAndLossReport> {
  if (isLanClientMode()) {
    return await callLanRpc<ProfitAndLossReport>('get_profit_and_loss', request as unknown as Record<string, unknown>);
  }
  if (!isTauriEnv()) {
    return {
      company_id: mockActiveCompanyId || 'cmp_demo_01', date_from: request.period.date_from, date_to: request.period.date_to,
      direct_income_rows: [], direct_income_subtotal: '0', direct_expense_rows: [], direct_expense_subtotal: '0',
      gross_profit_or_loss: '0', gross_result_type: 'Profit', indirect_income_rows: [], indirect_income_subtotal: '0',
      indirect_expense_rows: [], indirect_expense_subtotal: '0', net_profit_or_loss: '0', net_result_type: 'Profit',
      ledger_count: 0, warnings: [],
    };
  }
  return invoke<ProfitAndLossReport>('profit_and_loss', { request });
}

export async function getBalanceSheet(request: BalanceSheetRequest): Promise<BalanceSheetReport> {
  if (isLanClientMode()) {
    return await callLanRpc<BalanceSheetReport>('get_balance_sheet', request as unknown as Record<string, unknown>);
  }
  if (!isTauriEnv()) {
    return {
      company_id: mockActiveCompanyId || 'cmp_demo_01', as_of_date: request.as_of_date,
      asset_rows: [], total_assets: '0', liability_rows: [], total_liabilities: '0', equity_rows: [],
      current_period_profit_or_loss: '0', current_period_result_type: 'Profit', total_equity: '0',
      total_liabilities_and_equity: '0', balance_difference: '0', is_balanced: true, ledger_count: 0, warnings: [],
    };
  }
  return invoke<BalanceSheetReport>('balance_sheet', { request });
}

export async function getCashFlow(request: CashFlowRequest): Promise<CashFlowReport> {
  if (!isTauriEnv()) {
    return {
      company_id: mockActiveCompanyId || 'cmp_demo_01', date_from: request.period.date_from, date_to: request.period.date_to,
      opening_cash_balance: '0', operating_rows: [], operating_subtotal: '0', investing_rows: [], investing_subtotal: '0',
      financing_rows: [], financing_subtotal: '0', net_change_in_cash: '0', closing_cash_balance: '0', actual_closing_cash_balance: '0',
      reconciliation_difference: '0', is_reconciled: true, ledger_count: 0, warnings: [],
    };
  }
  return invoke<CashFlowReport>('cash_flow', { request });
}

export async function exportReportPreview(
  filter: VoucherSearchFilter,
  format: ExportFormat = 'csv_summary',
): Promise<VoucherExportPreviewResult> {
  if (!isTauriEnv()) throw new Error('Report export requires the native OpenTally application');
  return invoke<VoucherExportPreviewResult>('export_report_preview', {
    request: { filter, format, include_bom: false },
  });
}

export async function getBillsReceivable(asOfDate: string): Promise<PendingBill[]> {
  if (isLanClientMode()) {
    return await callLanRpc<PendingBill[]>('get_bills_receivable', { as_of_date: asOfDate });
  }
  if (!isTauriEnv()) return [];
  return invoke<PendingBill[]>('get_bills_receivable', { asOfDate });
}

export async function getBillsPayable(asOfDate: string): Promise<PendingBill[]> {
  if (isLanClientMode()) {
    return await callLanRpc<PendingBill[]>('get_bills_payable', { as_of_date: asOfDate });
  }
  if (!isTauriEnv()) return [];
  return invoke<PendingBill[]>('get_bills_payable', { asOfDate });
}

export async function getPartyPendingBills(partyLedgerId: string, asOfDate: string): Promise<PendingBill[]> {
  if (!isTauriEnv()) return [];
  return invoke<PendingBill[]>('get_party_pending_bills', { partyLedgerId, asOfDate });
}

export async function getAgingAnalysis(reportType: 'receivable' | 'payable', asOfDate: string): Promise<AgingReport> {
  if (!isTauriEnv()) {
    return {
      as_of_date: asOfDate,
      report_type: reportType,
      parties: [],
      grand_total: { not_due: '0.00', days_0_30: '0.00', days_31_60: '0.00', days_61_90: '0.00', days_90_plus: '0.00', total: '0.00' },
    };
  }
  return invoke<AgingReport>('get_aging_analysis', { reportType, asOfDate });
}

export async function recordBillAllocations(
  voucherId: string,
  voucherLineId: string,
  partyLedgerId: string,
  lineEntryType: string,
  lineAmount: string,
  billDate: string,
  dueDate: string | null,
  allocations: BillAllocationInput[],
): Promise<BillAllocation[]> {
  if (!isTauriEnv()) throw new Error('Bill allocations require the native OpenTally application');
  return invoke<BillAllocation[]>('record_bill_allocations', {
    voucherId,
    voucherLineId,
    partyLedgerId,
    lineEntryType,
    lineAmount,
    billDate,
    dueDate,
    allocations,
  });
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

// Inventory master-data API. Browser preview intentionally returns empty collections because
// inventory persistence belongs to the native SQLite/Tauri runtime.
const inventoryList = async <T>(command: string, search?: string): Promise<T[]> => isTauriEnv() ? invoke<T[]>(command, { search: search || null }) : [];
const inventoryMutate = async <T>(command: string, args: Record<string, unknown>): Promise<T> => { if (!isTauriEnv()) throw new Error('Inventory editing requires the native OpenTally application'); return invoke<T>(command, args); };
export const listStockGroups = (search?: string) => inventoryList<StockGroup>('list_stock_groups', search);
export const createStockGroup = (req: StockGroupRequest) => inventoryMutate<StockGroup>('create_stock_group', { req });
export const updateStockGroup = (id: string, req: StockGroupRequest) => inventoryMutate<StockGroup>('update_stock_group', { id, req });
export const setStockGroupActive = (id: string, active: boolean) => inventoryMutate<StockGroup>('set_stock_group_active', { id, active });
export const deleteStockGroup = (id: string) => inventoryMutate<void>('delete_stock_group', { id });
export const listStockCategories = (search?: string) => inventoryList<StockCategory>('list_stock_categories', search);
export const createStockCategory = (req: StockCategoryRequest) => inventoryMutate<StockCategory>('create_stock_category', { req });
export const updateStockCategory = (id: string, req: StockCategoryRequest) => inventoryMutate<StockCategory>('update_stock_category', { id, req });
export const setStockCategoryActive = (id: string, active: boolean) => inventoryMutate<StockCategory>('set_stock_category_active', { id, active });
export const deleteStockCategory = (id: string) => inventoryMutate<void>('delete_stock_category', { id });
export const listUnits = (search?: string) => inventoryList<Unit>('list_units', search);
export const createUnit = (req: UnitRequest) => inventoryMutate<Unit>('create_unit', { req });
export const updateUnit = (id: string, req: UnitRequest) => inventoryMutate<Unit>('update_unit', { id, req });
export const setUnitActive = (id: string, active: boolean) => inventoryMutate<Unit>('set_unit_active', { id, active });
export const deleteUnit = (id: string) => inventoryMutate<void>('delete_unit', { id });
export const listStockItems = (search?: string) => inventoryList<StockItem>('list_stock_items', search);
export const createStockItem = (req: StockItemRequest) => inventoryMutate<StockItem>('create_stock_item', { req });
export const updateStockItem = (id: string, req: StockItemRequest) => inventoryMutate<StockItem>('update_stock_item', { id, req });
export const setStockItemActive = (id: string, active: boolean) => inventoryMutate<StockItem>('set_stock_item_active', { id, active });
export const deleteStockItem = (id: string) => inventoryMutate<void>('delete_stock_item', { id });
export const listGodowns = (search?: string) => inventoryList<Godown>('list_godowns', search);
export const createGodown = (req: GodownRequest) => inventoryMutate<Godown>('create_godown', { req });
export const updateGodown = (id: string, req: GodownRequest) => inventoryMutate<Godown>('update_godown', { id, req });
export const setGodownActive = (id: string, active: boolean) => inventoryMutate<Godown>('set_godown_active', { id, active });
export const deleteGodown = (id: string) => inventoryMutate<void>('delete_godown', { id });
export async function createStockVoucher(request: StockVoucherRequest): Promise<StockVoucher> { return inventoryMutate<StockVoucher>('create_stock_voucher', { request }); }
export async function updateStockVoucher(id: string, request: StockVoucherRequest): Promise<StockVoucher> { return inventoryMutate<StockVoucher>('update_stock_voucher', { id, request }); }
export async function getStockVoucher(id: string): Promise<{ 0: StockVoucher; 1: StockVoucherLine[] }> { if (!isTauriEnv()) throw new Error('Stock vouchers require the native application'); return invoke<{ 0: StockVoucher; 1: StockVoucherLine[] }>('get_stock_voucher', { id }); }
export async function listStockVouchers(filter: StockVoucherListFilter): Promise<StockVoucher[]> { if (!isTauriEnv()) return []; return invoke<StockVoucher[]>('list_stock_vouchers', { filter }); }
export async function postStockVoucher(id: string): Promise<StockVoucher> { return inventoryMutate<StockVoucher>('post_stock_voucher', { id }); }
export async function cancelStockVoucher(id: string): Promise<StockVoucher> { return inventoryMutate<StockVoucher>('cancel_stock_voucher', { id }); }
export async function reverseStockVoucher(id: string): Promise<StockVoucher> { return inventoryMutate<StockVoucher>('reverse_stock_voucher', { id }); }
export async function getStockBalance(request: StockBalanceRequest, overall: boolean): Promise<StockBalanceRow[]> { if (!isTauriEnv()) return []; return invoke<StockBalanceRow[]>('get_stock_balance', { request, overall }); }
export async function getValuationConfig(): Promise<ValuationConfig> { if (!isTauriEnv()) return { company_id: '', valuation_method: 'fifo' }; return invoke<ValuationConfig>('get_inventory_valuation_config'); }
export async function setValuationMethod(method: 'fifo' | 'weighted_average'): Promise<ValuationConfig> { return inventoryMutate<ValuationConfig>('set_inventory_valuation_method', { method }); }
export async function getStockValuation(request: ValuationRequest): Promise<ValuationRow[]> { if (!isTauriEnv()) return []; return invoke<ValuationRow[]>('get_stock_valuation', { request }); }
export async function getFifoLayers(request: ValuationRequest): Promise<FifoLayer[]> { if (!isTauriEnv()) return []; return invoke<FifoLayer[]>('get_fifo_layers', { request }); }
export async function getValuationHistory(request: ValuationRequest): Promise<ValuationHistoryRow[]> { if (!isTauriEnv()) return []; return invoke<ValuationHistoryRow[]>('get_valuation_history', { request }); }
export async function getStockSummaryReport(filter: InventoryReportFilter): Promise<StockSummaryReport> { if (!isTauriEnv()) return { as_of_date: filter.as_of_date || filter.date_to || '', rows: [] }; return invoke<StockSummaryReport>('get_stock_summary_report', { filter }); }
export async function getStockMovementRegister(filter: InventoryReportFilter): Promise<StockRegisterReport> { if (!isTauriEnv()) return { total: 0, page: 1, page_size: filter.page_size || 100, rows: [] }; return invoke<StockRegisterReport>('get_stock_movement_register', { filter }); }
export async function getStockLedgerReport(filter: InventoryReportFilter): Promise<StockLedgerReport> { if (!isTauriEnv()) return { stock_item_id: filter.stock_item_id || '', godown_id: filter.godown_id || null, opening_quantity: '0', closing_quantity: '0', rows: [] }; return invoke<StockLedgerReport>('get_stock_ledger_report', { filter }); }
export async function getInventoryReconciliation(filter: InventoryReportFilter): Promise<ReconciliationFinding[]> { if (!isTauriEnv()) return []; return invoke<ReconciliationFinding[]>('get_inventory_reconciliation', { filter }); }
export async function getInventoryAccountMapping(): Promise<InventoryAccountMapping | null> { if (!isTauriEnv()) return null; return invoke<InventoryAccountMapping | null>('get_inventory_account_mapping'); }
export async function configureInventoryAccountMapping(request: InventoryAccountMappingRequest): Promise<InventoryAccountMapping> { return inventoryMutate<InventoryAccountMapping>('configure_inventory_account_mapping', { request }); }
export async function previewInventoryAccounting(stockVoucherId: string): Promise<InventoryAccountingPreview> { if (!isTauriEnv()) throw new Error('Inventory accounting requires the native application'); return invoke<InventoryAccountingPreview>('preview_inventory_accounting', { stockVoucherId }); }
export async function integrateInventoryAccounting(stockVoucherId: string): Promise<InventoryAccountingIntegration> { return inventoryMutate<InventoryAccountingIntegration>('integrate_inventory_accounting', { stockVoucherId }); }
export async function getInventoryAccountingStatus(stockVoucherId: string): Promise<InventoryAccountingIntegration | null> { if (!isTauriEnv()) return null; return invoke<InventoryAccountingIntegration | null>('get_inventory_accounting_status', { stockVoucherId }); }
export async function listInventoryAccountingHistory(): Promise<InventoryAccountingIntegration[]> { if (!isTauriEnv()) return []; return invoke<InventoryAccountingIntegration[]>('list_inventory_accounting_history'); }
export async function getInventoryValuationAttribution(stockVoucherId: string): Promise<InventoryValuationAttribution[]> { if (!isTauriEnv()) return []; return invoke<InventoryValuationAttribution[]>('get_inventory_valuation_attribution', { stockVoucherId }); }
export async function getInventoryValuationAttributionDetails(stockVoucherId: string): Promise<InventoryValuationAttributionDetails[]> { if (!isTauriEnv()) return []; return invoke<InventoryValuationAttributionDetails[]>('get_inventory_valuation_attribution_details', { stockVoucherId }); }

// -----------------------------------------------------------------------------
// Indian Statutory GST Services (Milestone 3)
// -----------------------------------------------------------------------------

export async function getCompanyGstProfile(): Promise<CompanyGstProfile | null> {
  if (!isTauriEnv()) return null;
  return invoke<CompanyGstProfile | null>('get_company_gst_profile');
}

export async function saveCompanyGstProfile(
  gstin: string | null,
  stateCode: string,
  registrationType: string,
  gstEnabled: boolean
): Promise<CompanyGstProfile> {
  if (!isTauriEnv()) throw new Error('Native app required');
  return invoke<CompanyGstProfile>('save_company_gst_profile', {
    gstin,
    stateCode,
    registrationType,
    gstEnabled,
  });
}

export async function getLedgerGstProfile(ledgerId: string): Promise<LedgerGstProfile | null> {
  if (!isTauriEnv()) return null;
  return invoke<LedgerGstProfile | null>('get_ledger_gst_profile', { ledgerId });
}

export async function saveLedgerGstProfile(
  ledgerId: string,
  gstin: string | null,
  stateCode: string,
  registrationType: string
): Promise<LedgerGstProfile> {
  if (!isTauriEnv()) throw new Error('Native app required');
  return invoke<LedgerGstProfile>('save_ledger_gst_profile', {
    ledgerId,
    gstin,
    stateCode,
    registrationType,
  });
}

export async function listHsnSacCodes(): Promise<HsnSacCode[]> {
  if (!isTauriEnv()) return [];
  return invoke<HsnSacCode[]>('list_hsn_sac_codes');
}

export async function createHsnSacCode(
  code: string,
  description: string | null,
  codeType: string,
  defaultGstRate: string,
  cessRate: string,
  effectiveFrom: string
): Promise<HsnSacCode> {
  if (!isTauriEnv()) throw new Error('Native app required');
  return invoke<HsnSacCode>('create_hsn_sac_code', {
    code,
    description,
    codeType,
    defaultGstRate,
    cessRate,
    effectiveFrom,
  });
}

export async function getItemGstMapping(stockItemId: string): Promise<ItemGstMapping | null> {
  if (!isTauriEnv()) return null;
  return invoke<ItemGstMapping | null>('get_item_gst_mapping', { stockItemId });
}

export async function saveItemGstMapping(
  stockItemId: string,
  hsnSacCodeId: string,
  gstRate: string,
  cessRate: string,
  taxability: string
): Promise<ItemGstMapping> {
  if (!isTauriEnv()) throw new Error('Native app required');
  return invoke<ItemGstMapping>('save_item_gst_mapping', {
    stockItemId,
    hsnSacCodeId,
    gstRate,
    cessRate,
    taxability,
  });
}

export async function getGstLedgerMapping(): Promise<GstLedgerMapping | null> {
  if (!isTauriEnv()) return null;
  return invoke<GstLedgerMapping | null>('get_gst_ledger_mapping');
}

export async function saveGstLedgerMapping(
  outputCgstLedgerId: string,
  outputSgstLedgerId: string,
  outputIgstLedgerId: string,
  outputCessLedgerId: string | null,
  inputCgstLedgerId: string,
  inputSgstLedgerId: string,
  inputIgstLedgerId: string,
  inputCessLedgerId: string | null
): Promise<GstLedgerMapping> {
  if (!isTauriEnv()) throw new Error('Native app required');
  return invoke<GstLedgerMapping>('save_gst_ledger_mapping', {
    outputCgstLedgerId,
    outputSgstLedgerId,
    outputIgstLedgerId,
    outputCessLedgerId,
    inputCgstLedgerId,
    inputSgstLedgerId,
    inputIgstLedgerId,
    inputCessLedgerId,
  });
}

export async function previewGstCalculation(
  placeOfSupplyStateCode: string,
  taxability: string,
  taxableValue: string,
  gstRate: string,
  cessRate: string,
  isReverseCharge: boolean
): Promise<GstCalculationResult> {
  if (!isTauriEnv()) {
    const val = parseFloat(taxableValue) || 0;
    const rate = parseFloat(gstRate) || 0;
    const cessR = parseFloat(cessRate) || 0;
    const tax = (val * rate) / 100;
    const cess = (val * cessR) / 100;
    return {
      supply_type: 'IntraState',
      taxable_value: val.toFixed(2),
      cgst_rate: (rate / 2).toFixed(2),
      cgst_amount: (tax / 2).toFixed(2),
      sgst_rate: (rate / 2).toFixed(2),
      sgst_amount: (tax / 2).toFixed(2),
      igst_rate: '0.00',
      igst_amount: '0.00',
      cess_rate: cessR.toFixed(2),
      cess_amount: cess.toFixed(2),
      total_tax: (tax + cess).toFixed(2),
      grand_total: (val + tax + cess).toFixed(2),
    };
  }
  return invoke<GstCalculationResult>('preview_gst_calculation', {
    placeOfSupplyStateCode,
    taxability,
    taxableValue,
    gstRate,
    cessRate,
    isReverseCharge,
  });
}

export async function getGstr1Summary(
  fromDate: string,
  toDate: string
): Promise<Gstr1SummaryReport> {
  if (isLanClientMode()) {
    return await callLanRpc<Gstr1SummaryReport>('get_gstr1_summary', { from_date: fromDate, to_date: toDate });
  }
  if (!isTauriEnv()) {
    return {
      company_id: '',
      from_date: fromDate,
      to_date: toDate,
      total_taxable_value: '0.00',
      total_cgst: '0.00',
      total_sgst: '0.00',
      total_igst: '0.00',
      total_cess: '0.00',
      total_tax: '0.00',
      total_invoice_value: '0.00',
      b2b_invoices: [],
      b2c_large_invoices: [],
      b2c_small_summary: [],
      hsn_summary: [],
    };
  }
  return invoke<Gstr1SummaryReport>('get_gstr1_summary', { fromDate, toDate });
}

export async function getGstr3bSummary(
  fromDate: string,
  toDate: string
): Promise<Gstr3bSummaryReport> {
  if (isLanClientMode()) {
    return await callLanRpc<Gstr3bSummaryReport>('get_gstr3b_summary', { from_date: fromDate, to_date: toDate });
  }
  if (!isTauriEnv()) {
    const zeroSummary = {
      integrated_tax: '0.00',
      central_tax: '0.00',
      state_ut_tax: '0.00',
      cess: '0.00',
      total: '0.00',
    };
    return {
      company_id: '',
      from_date: fromDate,
      to_date: toDate,
      table_3_1: [],
      table_4_itc: [],
      total_output_tax: zeroSummary,
      total_eligible_itc: zeroSummary,
      net_tax_payable: zeroSummary,
      closing_itc_balance: zeroSummary,
    };
  }
  return invoke<Gstr3bSummaryReport>('get_gstr3b_summary', { fromDate, toDate });
}

export async function globalGotoSearch(
  query: string,
  limit?: number
): Promise<GotoSearchResult[]> {
  if (isLanClientMode()) {
    return await callLanRpc<GotoSearchResult[]>('global_goto_search', { query, limit: limit || 20 });
  }
  if (!isTauriEnv()) {
    return [];
  }
  return invoke<GotoSearchResult[]>('global_goto_search', { query, limit: limit || 20 });
}

export async function getVoucherFullDetail(
  voucherId: string
): Promise<VoucherFullDetail> {
  if (isLanClientMode()) {
    return await callLanRpc<VoucherFullDetail>('get_voucher_full_detail', { voucher_id: voucherId });
  }
  if (!isTauriEnv()) {
    throw new Error('Voucher full detail requires the native OpenTally application');
  }
  return invoke<VoucherFullDetail>('get_voucher_full_detail', { voucherId });
}

export async function cancelVoucher(
  voucherId: string,
  reason: string
): Promise<VoucherWithLines> {
  if (isLanClientMode()) {
    return await callLanRpc<VoucherWithLines>('cancel_voucher', {
      voucher_id: voucherId,
      reason,
      actor_id: 'operator',
    });
  }
  if (!isTauriEnv()) {
    throw new Error('Voucher cancellation requires the native OpenTally application');
  }
  return invoke<VoucherWithLines>('cancel_voucher', {
    req: {
      voucher_id: voucherId,
      reason,
      actor_id: 'operator',
    },
  });
}

export async function reverseVoucher(
  voucherId: string,
  reversalDate: string,
  reason: string
): Promise<VoucherWithLines> {
  if (isLanClientMode()) {
    return await callLanRpc<VoucherWithLines>('reverse_voucher', {
      voucher_id: voucherId,
      reversal_date: reversalDate,
      reason,
      actor_id: 'operator',
    });
  }
  if (!isTauriEnv()) {
    throw new Error('Voucher reversal requires the native OpenTally application');
  }
  return invoke<VoucherWithLines>('reverse_voucher', {
    req: {
      voucher_id: voucherId,
      reversal_date: reversalDate,
      reason,
      actor_id: 'operator',
    },
  });
}

// -----------------------------------------------------------------------------
// Milestone 5: Notes, Invoice PDF & WAL-Safe Backup/Restore
// -----------------------------------------------------------------------------

export async function exportInvoicePdf(
  voucherId: string,
  outputPath: string
): Promise<string> {
  if (!isTauriEnv()) {
    throw new Error('Invoice PDF export requires the native OpenTally application');
  }
  return invoke<string>('export_invoice_pdf', { voucherId, outputPath });
}

export async function createDatabaseBackup(
  destinationPath: string
): Promise<BackupManifest> {
  if (!isTauriEnv()) {
    throw new Error('Database backup requires the native OpenTally application');
  }
  return invoke<BackupManifest>('create_database_backup', { destinationPath });
}

export async function restoreDatabaseBackup(
  backupPath: string
): Promise<RestoreResult> {
  if (!isTauriEnv()) {
    throw new Error('Database restore requires the native OpenTally application');
  }
  return invoke<RestoreResult>('restore_database_backup', { backupPath });
}

export async function getBackupStatus(): Promise<BackupStatus> {
  if (!isTauriEnv()) {
    return {
      database_engine: 'Browser Virtual Storage',
      is_wal_active: false,
      current_schema_version: 'v0.7.0-notes (browser preview)',
      last_backup_timestamp: null,
      last_backup_file: null,
      last_backup_sha256: null,
    };
  }
  return invoke<BackupStatus>('get_backup_status');
}

export async function postCommercialVoucher(
  input: CommercialPostingInput
): Promise<CommercialPostingResult> {
  if (!isTauriEnv()) {
    throw new Error('Commercial voucher posting requires the native OpenTally application');
  }
  return invoke<CommercialPostingResult>('post_commercial_voucher', { input });
}

export async function exportCompanyTallyXml(
  fromDate?: string,
  toDate?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return `<ENVELOPE><HEADER><TALLYREQUEST>Import Data</TALLYREQUEST></HEADER><BODY><IMPORTDATA><REQUESTDATA><TALLYMESSAGE><COMPANY>Demo Co</COMPANY></TALLYMESSAGE></REQUESTDATA></IMPORTDATA></BODY></ENVELOPE>`;
  }
  return invoke<string>('export_company_tally_xml', {
    fromDate: fromDate || null,
    toDate: toDate || null,
  });
}

export async function previewTallyXmlImport(
  xmlContent: string
): Promise<TallyXmlImportPreview> {
  if (!isTauriEnv()) {
    return {
      groups_to_create: 1,
      groups_skipped: 0,
      ledgers_to_create: 2,
      ledgers_skipped: 0,
      stock_items_to_create: 1,
      stock_items_skipped: 0,
      vouchers_to_create: 1,
      vouchers_skipped: 0,
      can_import: true,
      errors: [],
    };
  }
  return invoke<TallyXmlImportPreview>('preview_tally_xml_import', { xmlContent });
}

export async function executeTallyXmlImport(
  xmlContent: string
): Promise<TallyXmlImportResult> {
  if (!isTauriEnv()) {
    return {
      success: true,
      groups_created: 1,
      ledgers_created: 2,
      stock_items_created: 1,
      vouchers_created: 1,
      message: 'Demo import completed',
    };
  }
  return invoke<TallyXmlImportResult>('execute_tally_xml_import', { xmlContent });
}

export async function exportGstr1Json(
  fromDate: string,
  toDate: string
): Promise<string> {
  if (!isTauriEnv()) {
    return JSON.stringify({ gstin: 'DEMO', fp: '052026', b2b: [] }, null, 2);
  }
  return invoke<string>('export_gstr1_json', { fromDate, toDate });
}

export async function exportGstr3bJson(
  fromDate: string,
  toDate: string
): Promise<string> {
  if (!isTauriEnv()) {
    return JSON.stringify({ gstin: 'DEMO', ret_period: '052026', sup_details: [] }, null, 2);
  }
  return invoke<string>('export_gstr3b_json', { fromDate, toDate });
}

export async function getCompanyFeatures(companyId?: string): Promise<CompanyFeaturesDto> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || mockActiveCompanyId || 'cmp_demo_01',
      maintain_inventory: true,
      billwise_tracking: true,
      enable_perpetual_cogs: true,
      gst_enabled: true,
      gstin: '29ABCDE1234F1ZW',
      state_code: '29',
      registration_type: 'Regular',
      has_gst_ledgers_mapped: true,
      has_inventory_ledgers_mapped: true,
      tds_enabled: false,
      has_tds_sections_seeded: false,
    };
  }
  return await invoke<CompanyFeaturesDto>('get_company_features', { companyId });
}

export async function saveCompanyFeatures(
  input: SaveCompanyFeaturesInput,
  companyId?: string
): Promise<CompanyFeaturesDto> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || mockActiveCompanyId || 'cmp_demo_01',
      maintain_inventory: input.maintain_inventory,
      billwise_tracking: input.billwise_tracking,
      enable_perpetual_cogs: input.enable_perpetual_cogs,
      gst_enabled: input.gst_enabled,
      gstin: input.gstin || null,
      state_code: input.state_code || null,
      registration_type: input.registration_type || 'Regular',
      has_gst_ledgers_mapped: input.gst_enabled,
      has_inventory_ledgers_mapped: input.maintain_inventory,
      tds_enabled: input.tds_enabled ?? false,
      has_tds_sections_seeded: input.auto_seed_tds_defaults ?? false,
    };
  }
  return await invoke<CompanyFeaturesDto>('save_company_features', { companyId, input });
}

export async function createSampleCompany(): Promise<Company> {
  if (!isTauriEnv()) {
    const sampleCompany: Company = {
      id: `cmp_sample_${Date.now()}`,
      name: 'Bharath Precision Components Pvt Ltd',
      mailing_name: 'Bharath Precision Components Pvt Ltd',
      country: 'India',
      state: 'Karnataka',
      financial_year_begins: '2025-04-01',
      books_beginning_from: '2025-04-01',
      currency_symbol: '₹',
      currency_formal_name: 'INR',
      currency_decimal_places: 2,
      status: 'active',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockCompanies.push(sampleCompany);
    mockActiveCompanyId = sampleCompany.id;
    return sampleCompany;
  }
  return await invoke<Company>('create_sample_company');
}







// Milestone 8 Banking & Bank Reconciliation Statement (BRS) APIs

export async function listBankLedgers(companyId?: string): Promise<BankLedgerItem[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'led_bank_default',
        name: 'HDFC Bank Regular A/c',
        group_id: 'grp_bank',
        group_name: 'Bank Accounts',
        current_balance: '125000.00',
        current_balance_dr_cr: 'Dr',
      },
      {
        id: 'led_bank_od',
        name: 'SBI Overdraft A/c',
        group_id: 'grp_od',
        group_name: 'Bank OD A/c',
        current_balance: '45000.00',
        current_balance_dr_cr: 'Cr',
      },
    ];
  }
  return await invoke<BankLedgerItem[]>('list_bank_ledgers', { companyId: companyId || null });
}

export async function getBankReconciliation(
  bankLedgerId: string,
  fromDate: string,
  toDate: string,
  includeReconciled: boolean = false,
  companyId?: string
): Promise<BankReconciliationStatement> {
  if (!isTauriEnv()) {
    return {
      bank_ledger_id: bankLedgerId,
      bank_ledger_name: 'HDFC Bank Regular A/c',
      from_date: fromDate,
      to_date: toDate,
      summary: {
        book_balance: '125000.00',
        book_balance_dr_cr: 'Dr',
        unreconciled_debit: '25000.00',
        unreconciled_credit: '15000.00',
        bank_balance: '115000.00',
        bank_balance_dr_cr: 'Dr',
      },
      rows: [
        {
          bank_allocation_id: 'alloc_mock_1',
          voucher_id: 'vch_mock_1',
          voucher_line_id: 'line_mock_1',
          voucher_date: '2026-04-10',
          voucher_number: 'PMT-001',
          voucher_type: 'Payment',
          particulars: 'Precision Tools Ltd',
          transaction_type: 'Cheque',
          instrument_number: 'CHQ-100200',
          instrument_date: '2026-04-09',
          favouring_name: 'Precision Tools',
          bank_date: null,
          debit_amount: '0.00',
          credit_amount: '15000.00',
          is_reconciled: false,
        },
        {
          bank_allocation_id: 'alloc_mock_2',
          voucher_id: 'vch_mock_2',
          voucher_line_id: 'line_mock_2',
          voucher_date: '2026-04-12',
          voucher_number: 'RCT-001',
          voucher_type: 'Receipt',
          particulars: 'Apex Retailers',
          transaction_type: 'NEFT',
          instrument_number: 'NEFT-554433',
          instrument_date: '2026-04-12',
          favouring_name: 'Apex Retailers',
          bank_date: null,
          debit_amount: '25000.00',
          credit_amount: '0.00',
          is_reconciled: false,
        },
      ],
    };
  }
  return await invoke<BankReconciliationStatement>('get_bank_reconciliation', {
    bankLedgerId,
    fromDate,
    toDate,
    includeReconciled,
    companyId: companyId || null,
  });
}

export async function reconcileBankEntries(
  entries: ReconcileEntryInput[],
  companyId?: string
): Promise<ReconcileResult> {
  if (!isTauriEnv()) {
    return {
      success: true,
      updated_count: entries.length,
      updated_ids: entries.map(e => e.bank_allocation_id),
    };
  }
  return await invoke<ReconcileResult>('reconcile_bank_entries', {
    entries,
    companyId: companyId || null,
  });
}

export async function previewBankStatementAutoMatch(
  bankLedgerId: string,
  csvContent: string,
  companyId?: string
): Promise<BankStatementMatchPreview> {
  if (!isTauriEnv()) {
    return {
      total_statement_rows: 2,
      exact_matches: 1,
      suggested_matches: 1,
      unmatched: 0,
      rows: [
        {
          csv_row_index: 0,
          statement_date: '2026-04-11',
          statement_ref: 'CHQ-100200',
          statement_description: 'Cheque Clearing Precision Tools',
          statement_amount: '15000.00',
          statement_dr_cr: 'Dr',
          matched_bank_allocation_id: 'alloc_mock_1',
          matched_voucher_number: 'PMT-001',
          matched_particulars: 'Precision Tools Ltd',
          suggested_bank_date: '2026-04-11',
          match_type: 'exact',
          confidence: 'HIGH',
        },
        {
          csv_row_index: 1,
          statement_date: '2026-04-13',
          statement_ref: 'NEFT-554433',
          statement_description: 'NEFT INWARD APEX',
          statement_amount: '25000.00',
          statement_dr_cr: 'Cr',
          matched_bank_allocation_id: 'alloc_mock_2',
          matched_voucher_number: 'RCT-001',
          matched_particulars: 'Apex Retailers',
          suggested_bank_date: '2026-04-13',
          match_type: 'suggested',
          confidence: 'MEDIUM',
        },
      ],
    };
  }
  return await invoke<BankStatementMatchPreview>('preview_bank_statement_auto_match', {
    bankLedgerId,
    csvContent,
    companyId: companyId || null,
  });
}

// Milestone 9: Indian TDS & TCS API Functions
export async function listTdsSections(companyId?: string): Promise<TdsNatureOfPayment[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'tds_sec_194c',
        company_id: companyId || 'mock_cmp',
        section_code: '194C',
        name: 'Payment to Contractors / Subcontractors',
        rate_individual_huf: '1.00',
        rate_others: '2.00',
        rate_without_pan: '20.00',
        single_threshold_limit: '30000.00',
        aggregate_threshold_limit: '100000.00',
        tds_payable_ledger_id: 'led_tds_payable',
        is_tcs: 0,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
      {
        id: 'tds_sec_194j',
        company_id: companyId || 'mock_cmp',
        section_code: '194J',
        name: 'Fees for Professional or Technical Services',
        rate_individual_huf: '10.00',
        rate_others: '10.00',
        rate_without_pan: '20.00',
        single_threshold_limit: '30000.00',
        aggregate_threshold_limit: '30000.00',
        tds_payable_ledger_id: 'led_tds_payable',
        is_tcs: 0,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
      {
        id: 'tds_sec_194i',
        company_id: companyId || 'mock_cmp',
        section_code: '194I',
        name: 'Rent on Land, Building or Furniture',
        rate_individual_huf: '10.00',
        rate_others: '10.00',
        rate_without_pan: '20.00',
        single_threshold_limit: '240000.00',
        aggregate_threshold_limit: '240000.00',
        tds_payable_ledger_id: 'led_tds_payable',
        is_tcs: 0,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
    ];
  }
  return await invoke<TdsNatureOfPayment[]>('list_tds_sections', {
    companyId: companyId || null,
  });
}

export async function createTdsSection(input: CreateTdsNatureOfPaymentInput): Promise<TdsNatureOfPayment> {
  if (!isTauriEnv()) {
    return {
      id: `tds_${Date.now()}`,
      company_id: input.company_id,
      section_code: input.section_code,
      name: input.name,
      rate_individual_huf: input.rate_individual_huf,
      rate_others: input.rate_others,
      rate_without_pan: input.rate_without_pan || '20.00',
      single_threshold_limit: input.single_threshold_limit || '0.00',
      aggregate_threshold_limit: input.aggregate_threshold_limit || '0.00',
      tds_payable_ledger_id: input.tds_payable_ledger_id || null,
      is_tcs: input.is_tcs ? 1 : 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<TdsNatureOfPayment>('create_tds_section', { input });
}

export async function getCompanyTdsProfile(companyId?: string): Promise<CompanyTdsProfile | null> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'mock_cmp',
      tan: 'BLRP12345A',
      deductor_type: 'Company',
      tds_enabled: 1,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    };
  }
  return await invoke<CompanyTdsProfile | null>('get_company_tds_profile', {
    companyId: companyId || null,
  });
}

export async function saveCompanyTdsProfile(input: SaveCompanyTdsProfileInput): Promise<CompanyTdsProfile> {
  if (!isTauriEnv()) {
    return {
      company_id: input.company_id,
      tan: input.tan || null,
      deductor_type: input.deductor_type,
      tds_enabled: input.tds_enabled ? 1 : 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<CompanyTdsProfile>('save_company_tds_profile', { input });
}

export async function getLedgerTdsProfile(ledgerId: string, companyId?: string): Promise<LedgerTdsProfile | null> {
  if (!isTauriEnv()) {
    return {
      id: `ltp_${ledgerId}`,
      company_id: companyId || 'mock_cmp',
      ledger_id: ledgerId,
      pan: 'ABCCD1234F',
      deductee_type: 'Company_Domestic',
      lower_deduction_cert_no: null,
      lower_deduction_rate: null,
      created_at: '2026-04-01T00:00:00Z',
      updated_at: '2026-04-01T00:00:00Z',
    };
  }
  return await invoke<LedgerTdsProfile | null>('get_ledger_tds_profile', {
    ledgerId,
    companyId: companyId || null,
  });
}

export async function saveLedgerTdsProfile(input: SaveLedgerTdsProfileInput): Promise<LedgerTdsProfile> {
  if (!isTauriEnv()) {
    return {
      id: `ltp_${input.ledger_id}`,
      company_id: input.company_id,
      ledger_id: input.ledger_id,
      pan: input.pan || null,
      deductee_type: input.deductee_type,
      lower_deduction_cert_no: input.lower_deduction_cert_no || null,
      lower_deduction_rate: input.lower_deduction_rate || null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<LedgerTdsProfile>('save_ledger_tds_profile', { input });
}

export async function previewTdsCalculation(input: TdsCalculationPreviewInput): Promise<TdsCalculationPreviewResult> {
  if (!isTauriEnv()) {
    const taxAmt = parseFloat(input.taxable_amount || '0');
    const tdsAmt = taxAmt * 0.1;
    return {
      section_code: input.section_code,
      section_name: 'Mock TDS Section',
      deductee_type: 'Company_Domestic',
      party_pan: 'ABCCD1234F',
      is_pan_missing: false,
      is_pan_missing_higher_rate: false,
      taxable_base: input.taxable_amount,
      applicable_rate: '10.00',
      threshold_exceeded: true,
      single_threshold: '30000.00',
      aggregate_threshold: '30000.00',
      prior_aggregate_amount: '0.00',
      current_aggregate_amount: input.taxable_amount,
      tds_amount: tdsAmt.toFixed(2),
      net_payable_amount: (taxAmt - tdsAmt).toFixed(2),
      tds_payable_ledger_id: 'led_tds_payable',
    };
  }
  return await invoke<TdsCalculationPreviewResult>('preview_tds_calculation', { input });
}

export async function recordTdsChallanPayment(input: RecordTdsChallanPaymentInput): Promise<number> {
  if (!isTauriEnv()) {
    return input.voucher_tds_detail_ids.length;
  }
  return await invoke<number>('record_tds_challan_payment', { input });
}

export async function getTdsOutstanding(asOfDate: string, companyId?: string): Promise<TdsOutstandingReport> {
  if (!isTauriEnv()) {
    return {
      as_of_date: asOfDate,
      total_unpaid_tds: '10000.00',
      sections: [
        {
          section_code: '194J',
          section_name: 'Fees for Professional or Technical Services',
          unpaid_tds_amount: '10000.00',
          voucher_count: 1,
          vouchers: [
            {
              voucher_tds_id: 'vtd_mock_1',
              voucher_id: 'vch_mock_1',
              voucher_number: 'PUR/2026/001',
              voucher_date: '2026-05-15',
              party_ledger_id: 'led_supp_consultant',
              party_name: 'TechServe Solutions',
              party_pan: 'ABCCD1234F',
              section_code: '194J',
              gross_amount: '100000.00',
              tds_rate: '10.00',
              tds_amount: '10000.00',
              due_date: '2026-06-07',
              status: 'Due',
            },
          ],
        },
      ],
    };
  }
  return await invoke<TdsOutstandingReport>('get_tds_outstanding', {
    asOfDate,
    companyId: companyId || null,
  });
}

export async function getForm26qSummary(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<Form26qSummary> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'mock_cmp',
      financial_year: financialYear,
      quarter,
      deductor_tan: 'BLRP12345A',
      deductor_name: 'Apex Electronics Pvt Ltd',
      total_amount_paid: '100000.00',
      total_tds_deducted: '10000.00',
      total_tds_deposited: '10000.00',
      total_tds_unpaid: '0.00',
      challans: [
        {
          challan_bsr_code: '0210012',
          challan_date: '2026-06-05',
          challan_serial_no: '00042',
          payment_voucher_id: null,
          total_tds_deposited: '10000.00',
          voucher_count: 1,
        },
      ],
      deductions: [
        {
          voucher_tds_id: 'vtd_mock_1',
          voucher_number: 'PUR/2026/001',
          deductee_pan: 'ABCCD1234F',
          deductee_name: 'TechServe Solutions',
          deductee_type: 'Company_Domestic',
          section_code: '194J',
          payment_date: '2026-05-15',
          amount_paid: '100000.00',
          tds_rate: '10.00',
          tds_deducted: '10000.00',
          is_pan_missing_higher_rate: false,
          challan_serial_no: '00042',
          challan_date: '2026-06-05',
          challan_bsr_code: '0210012',
          status: 'Deposited',
        },
      ],
    };
  }
  return await invoke<Form26qSummary>('get_form_26q_summary', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function exportForm26qJson(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    const summary = await getForm26qSummary(financialYear, quarter, companyId);
    return JSON.stringify(summary, null, 2);
  }
  return await invoke<string>('export_form_26q_json', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function getForm27qSummary(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<any> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || '',
      financial_year: financialYear,
      quarter,
      deductor_tan: null,
      total_amount_paid: '0',
      total_tds_deducted: '0',
      total_tds_deposited: '0',
      deductions: [],
    };
  }
  return await invoke('get_form_27q_summary', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function exportForm27qJson(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    const summary = await getForm27qSummary(financialYear, quarter, companyId);
    return JSON.stringify(summary, null, 2);
  }
  return await invoke<string>('export_form_27q_json', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function getForm27eqSummary(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<any> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || '',
      financial_year: financialYear,
      quarter,
      deductor_tan: null,
      total_amount_paid: '0',
      total_tds_deducted: '0',
      total_tds_deposited: '0',
      deductions: [],
    };
  }
  return await invoke('get_form_27eq_summary', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function exportForm27eqJson(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    const summary = await getForm27eqSummary(financialYear, quarter, companyId);
    return JSON.stringify(summary, null, 2);
  }
  return await invoke<string>('export_form_27eq_json', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

export async function listPtSlabs(
  stateCode?: string,
  companyId?: string
): Promise<any[]> {
  if (!isTauriEnv()) return [];
  return await invoke('list_pt_slabs', {
    stateCode: stateCode || null,
    companyId: companyId || null,
  });
}

export async function exportEpfoEcrFile(
  payPeriod: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) return '';
  return await invoke<string>('export_epfo_ecr_file', {
    payPeriod,
    companyId: companyId || null,
  });
}

export async function exportTrialBalancePdf(
  asOfDate: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_trial_balance_pdf', {
    asOfDate,
    companyId: companyId || null,
  });
}

export async function exportProfitAndLossPdf(
  fromDate: string,
  toDate: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_profit_and_loss_pdf', {
    fromDate,
    toDate,
    companyId: companyId || null,
  });
}

export async function exportBalanceSheetPdf(
  asOfDate: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_balance_sheet_pdf', {
    asOfDate,
    companyId: companyId || null,
  });
}

export async function exportLedgerStatementPdf(
  ledgerId: string,
  fromDate: string,
  toDate: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_ledger_statement_pdf', {
    ledgerId,
    fromDate,
    toDate,
    companyId: companyId || null,
  });
}

export async function exportOutstandingAgingPdf(
  reportType: string,
  asOfDate: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_outstanding_aging_pdf', {
    reportType,
    asOfDate,
    companyId: companyId || null,
  });
}

export async function exportForm26qCsv(
  financialYear: string,
  quarter: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return [
      `# Deductor TAN: BLRP12345A, FY: ${financialYear}, Quarter: ${quarter}`,
      'Voucher No,Deductee PAN,Deductee Name,Deductee Type,Section,Payment Date,Gross Amount,TDS Rate,TDS Deducted,Challan BSR,Challan Date,Challan Serial,Status',
      'PUR/2026/001,ABCCD1234F,TechServe Solutions,Company_Domestic,194J,2026-05-15,100000.00,10.00,10000.00,0210012,2026-06-05,00042,Deposited',
    ].join('\n');
  }
  return await invoke<string>('export_form_26q_csv', {
    financialYear,
    quarter,
    companyId: companyId || null,
  });
}

// -----------------------------------------------------------------------------
// Milestone 10: GST Compliance (GSTR-2B, e-Invoice, e-Way Bill)
// -----------------------------------------------------------------------------

export async function importAndReconcileGstr2b(
  returnPeriod: string,
  jsonPayload: string,
  companyId?: string
): Promise<Gstr2bReconciliationReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_demo_01',
      return_period: returnPeriod,
      batch_id: 'batch_mock_01',
      imported_at: new Date().toISOString(),
      summary: {
        total_portal_invoices: 2,
        matched_count: 1,
        mismatch_count: 0,
        missing_in_books_count: 1,
        missing_in_2b_count: 1,
        matched_itc: '18000.00',
        mismatch_itc: '0.00',
        missing_in_books_itc: '9000.00',
        at_risk_itc: '5400.00',
        total_eligible_itc: '18000.00',
      },
      invoices: [
        {
          id: 'row_1',
          batch_id: 'batch_mock_01',
          supplier_gstin: '29ABCDE1234F1Z5',
          supplier_name: 'Apex Industrial Tools',
          invoice_number: 'INV-2026-001',
          invoice_date: '2026-04-15',
          invoice_value: '118000.00',
          taxable_value: '100000.00',
          cgst_amount: '9000.00',
          sgst_amount: '9000.00',
          igst_amount: '0.00',
          cess_amount: '0.00',
          total_tax: '18000.00',
          itc_availability: 'Y',
          reconciliation_status: 'Matched',
          matched_voucher_id: 'v_pur_01',
          matched_voucher_number: 'PUR/001',
          books_taxable_value: '100000.00',
          books_total_tax: '18000.00',
          tax_difference: '0.00',
        },
        {
          id: 'row_2',
          batch_id: 'batch_mock_01',
          supplier_gstin: '27AABCT3456K1Z2',
          supplier_name: 'Metro Fasteners',
          invoice_number: 'INV-8899',
          invoice_date: '2026-04-20',
          invoice_value: '59000.00',
          taxable_value: '50000.00',
          cgst_amount: '0.00',
          sgst_amount: '0.00',
          igst_amount: '9000.00',
          cess_amount: '0.00',
          total_tax: '9000.00',
          itc_availability: 'Y',
          reconciliation_status: 'MissingInBooks',
          matched_voucher_id: null,
          matched_voucher_number: null,
          books_taxable_value: null,
          books_total_tax: null,
          tax_difference: '9000.00',
        },
        {
          id: 'row_3',
          batch_id: 'batch_mock_01',
          supplier_gstin: '29XYZDE9876P1Z1',
          supplier_name: 'Delta Engineering',
          invoice_number: 'PUR/002',
          invoice_date: '2026-04-22',
          invoice_value: '35400.00',
          taxable_value: '30000.00',
          cgst_amount: '2700.00',
          sgst_amount: '2700.00',
          igst_amount: '0.00',
          cess_amount: '0.00',
          total_tax: '5400.00',
          itc_availability: 'Y',
          reconciliation_status: 'MissingIn2B',
          matched_voucher_id: 'v_pur_02',
          matched_voucher_number: 'PUR/002',
          books_taxable_value: '30000.00',
          books_total_tax: '5400.00',
          tax_difference: '-5400.00',
        },
      ],
    };
  }
  return await invoke<Gstr2bReconciliationReport>('import_and_reconcile_gstr2b', {
    companyId: companyId || null,
    returnPeriod,
    jsonPayload,
  });
}

export async function getGstr2bReconciliationReport(
  returnPeriod: string,
  companyId?: string
): Promise<Gstr2bReconciliationReport> {
  if (!isTauriEnv()) {
    return importAndReconcileGstr2b(returnPeriod, '{}', companyId);
  }
  return await invoke<Gstr2bReconciliationReport>('get_gstr2b_reconciliation_report', {
    companyId: companyId || null,
    returnPeriod,
  });
}

export async function exportGstr2bReconciliationCsv(
  returnPeriod: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return [
      'Status,Supplier GSTIN,Supplier Name,Invoice No,Invoice Date,Portal Taxable,Portal Tax,Books Tax,Variance,ITC Available,Matched Voucher No',
      'Matched,29ABCDE1234F1Z5,Apex Industrial Tools,INV-2026-001,2026-04-15,100000.00,18000.00,18000.00,0.00,Y,PUR/001',
      'MissingInBooks,27AABCT3456K1Z2,Metro Fasteners,INV-8899,2026-04-20,50000.00,9000.00,0.00,9000.00,Y,',
      'MissingIn2B,29XYZDE9876P1Z1,Delta Engineering,PUR/002,2026-04-22,30000.00,5400.00,5400.00,-5400.00,Y,PUR/002',
    ].join('\r\n');
  }
  return await invoke<string>('export_gstr2b_reconciliation_csv', {
    companyId: companyId || null,
    returnPeriod,
  });
}

export async function exportEinvoiceJson(
  voucherId: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return JSON.stringify({
      Version: '1.1',
      TranDtls: { TaxSch: 'GST', SupTyp: 'B2B', RegRev: 'N', EcmGstin: null, IgstOnIntra: 'N' },
      DocDtls: { Typ: 'INV', No: 'SALES-001', Dt: '15/04/2026' },
      SellerDtls: { Gstin: '29ABCDE1234F1Z5', LglNm: 'OpenTally Demo', TrdNm: 'OpenTally', Addr1: 'Bangalore', Loc: 'Bangalore', Pin: 560001, Stcd: '29' },
      BuyerDtls: { Gstin: '29XYZDE9876P1Z1', LglNm: 'Customer Corp', TrdNm: 'Customer Corp', Pos: '29', Addr1: 'N/A', Loc: 'City', Pin: 560001, Stcd: '29' },
      ItemList: [],
      ValDtls: { AssVal: 10000, CgstVal: 900, SgstVal: 900, IgstVal: 0, CesVal: 0, RndOffAmt: 0, TotInvVal: 11800 },
    }, null, 2);
  }
  return await invoke<string>('export_einvoice_json', {
    companyId: companyId || null,
    voucherId,
  });
}

export async function exportEwayBillJson(
  voucherId: string,
  transport: TransportDetailsInput,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return JSON.stringify({
      version: '1.0.0621',
      billLists: [{
        userGstin: '29ABCDE1234F1Z5',
        supplyType: 'O',
        subSupplyType: '1',
        docType: 'INV',
        docNo: 'SALES-001',
        transMode: transport.transport_mode,
        transDistance: transport.distance_km.toString(),
        vehicleNo: transport.vehicle_no || null,
      }],
    }, null, 2);
  }
  return await invoke<string>('export_eway_bill_json', {
    companyId: companyId || null,
    voucherId,
    transport,
  });
}

export async function saveVoucherEinvoiceMetadata(
  voucherId: string,
  input: VoucherEinvoiceMetadataInput,
  companyId?: string
): Promise<VoucherEinvoiceMetadataDto> {
  if (!isTauriEnv()) {
    return {
      id: 'mock_meta_1',
      company_id: companyId || 'cmp_demo_01',
      voucher_id: voucherId,
      irn: input.irn || null,
      ack_no: input.ack_no || null,
      ack_date: input.ack_date || null,
      signed_qr_code: input.signed_qr_code || null,
      einvoice_status: (input.einvoice_status as any) || 'Pending',
      eway_bill_no: input.eway_bill_no || null,
      eway_bill_date: input.eway_bill_date || null,
      eway_valid_upto: input.eway_valid_upto || null,
      transporter_id: input.transporter_id || null,
      transporter_name: input.transporter_name || null,
      transport_mode: input.transport_mode || null,
      vehicle_no: input.vehicle_no || null,
      distance_km: input.distance_km || null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<VoucherEinvoiceMetadataDto>('save_voucher_einvoice_metadata', {
    companyId: companyId || null,
    voucherId,
    input,
  });
}

export async function getVoucherEinvoiceMetadata(
  voucherId: string,
  companyId?: string
): Promise<VoucherEinvoiceMetadataDto | null> {
  if (!isTauriEnv()) {
    return null;
  }
  return await invoke<VoucherEinvoiceMetadataDto | null>('get_voucher_einvoice_metadata', {
    companyId: companyId || null,
    voucherId,
  });
}

// ==========================================
// Milestone 11: Multi-Location Godowns, Batches, Stock Journals & Cost Centres
// ==========================================

export async function getGodownSummaryReport(
  godownId: string,
  asOfDate?: string,
  companyId?: string
): Promise<GodownSummaryReport> {
  if (!isTauriEnv()) {
    return {
      godown_id: godownId,
      godown_name: 'Central Godown',
      address: 'Industrial Area Phase 2, Peenya, Bangalore',
      is_external: false,
      items: [
        {
          stock_item_id: 'item_1',
          stock_item_name: 'Copper Fitting 25mm',
          base_unit: 'Nos',
          inflow_quantity: '100',
          outflow_quantity: '20',
          closing_quantity: '80',
          average_rate: '150.00',
          closing_value: '12000.00',
        },
      ],
      total_godown_value: '12000.00',
    };
  }
  return await invoke<GodownSummaryReport>('get_godown_summary_report', {
    godownId,
    companyId: companyId || null,
    asOfDate: asOfDate || null,
  });
}

export async function listStockItemBatches(
  stockItemId: string,
  companyId?: string
): Promise<StockItemBatch[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'batch_mock_1',
        company_id: companyId || 'cmp_mock',
        stock_item_id: stockItemId,
        batch_number: 'BATCH-2025-A',
        mfg_date: '2025-01-01',
        expiry_date: '2026-06-30',
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
  }
  return await invoke<StockItemBatch[]>('list_stock_item_batches', {
    stockItemId,
    companyId: companyId || null,
  });
}

export async function createStockItemBatch(
  stockItemId: string,
  req: StockItemBatchRequest,
  companyId?: string
): Promise<StockItemBatch> {
  if (!isTauriEnv()) {
    return {
      id: `batch_${Date.now()}`,
      company_id: companyId || 'cmp_mock',
      stock_item_id: stockItemId,
      batch_number: req.batch_number,
      mfg_date: req.mfg_date || null,
      expiry_date: req.expiry_date || null,
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<StockItemBatch>('create_stock_item_batch', {
    stockItemId,
    req,
    companyId: companyId || null,
  });
}

export async function getBatchExpiryReport(
  asOfDate?: string,
  companyId?: string
): Promise<BatchExpiryReportRow[]> {
  if (!isTauriEnv()) {
    return [
      {
        stock_item_id: 'item_1',
        stock_item_name: 'Amoxicillin 500mg',
        batch_id: 'batch_mock_1',
        batch_number: 'BATCH-2025-A',
        mfg_date: '2025-01-01',
        expiry_date: '2025-12-31',
        closing_quantity: '500',
        unit_cost: '12.50',
        closing_value: '6250.00',
        days_until_expiry: 35,
        status: 'ExpiringIn60Days',
      },
    ];
  }
  return await invoke<BatchExpiryReportRow[]>('get_batch_expiry_report', {
    companyId: companyId || null,
    asOfDate: asOfDate || null,
  });
}

export async function postStockJournalVoucher(
  req: StockJournalRequest,
  companyId?: string
): Promise<StockJournalResponse> {
  if (!isTauriEnv()) {
    return {
      voucher_id: `vch_sj_${Date.now()}`,
      voucher_number: req.voucher_number || 'SJ-TEST-001',
      voucher_date: req.voucher_date,
      total_source_value: '15000.00',
      total_destination_value: '15000.00',
      lines_count: req.source_lines.length + req.destination_lines.length,
    };
  }
  return await invoke<StockJournalResponse>('post_stock_journal_voucher', {
    req,
    companyId: companyId || null,
  });
}

export async function listStockJournalVouchers(
  companyId?: string,
  limit?: number
): Promise<StockJournalResponse[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<StockJournalResponse[]>('list_stock_journal_vouchers', {
    companyId: companyId || null,
    limit: limit || null,
  });
}

export async function listCostCategories(
  companyId?: string
): Promise<CostCategory[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'cat_primary',
        company_id: companyId || 'cmp_mock',
        name: 'Primary Cost Category',
        allocate_revenue: true,
        allocate_non_revenue: false,
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      {
        id: 'cat_projects',
        company_id: companyId || 'cmp_mock',
        name: 'Projects & Contracts',
        allocate_revenue: true,
        allocate_non_revenue: true,
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
  }
  return await invoke<CostCategory[]>('list_cost_categories', {
    companyId: companyId || null,
  });
}

export async function createCostCategory(
  req: CostCategoryRequest,
  companyId?: string
): Promise<CostCategory> {
  if (!isTauriEnv()) {
    return {
      id: `cat_${Date.now()}`,
      company_id: companyId || 'cmp_mock',
      name: req.name,
      allocate_revenue: req.allocate_revenue,
      allocate_non_revenue: req.allocate_non_revenue,
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<CostCategory>('create_cost_category', {
    req,
    companyId: companyId || null,
  });
}

export async function listCostCentres(
  categoryId?: string,
  companyId?: string
): Promise<CostCentre[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'cc_north',
        company_id: companyId || 'cmp_mock',
        category_id: categoryId || 'cat_primary',
        name: 'North Region Sales',
        parent_id: null,
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      {
        id: 'cc_south',
        company_id: companyId || 'cmp_mock',
        category_id: categoryId || 'cat_primary',
        name: 'South Region Sales',
        parent_id: null,
        is_active: true,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
    ];
  }
  return await invoke<CostCentre[]>('list_cost_centres', {
    categoryId: categoryId || null,
    companyId: companyId || null,
  });
}

export async function createCostCentre(
  req: CostCentreRequest,
  companyId?: string
): Promise<CostCentre> {
  if (!isTauriEnv()) {
    return {
      id: `cc_${Date.now()}`,
      company_id: companyId || 'cmp_mock',
      category_id: req.category_id,
      name: req.name,
      parent_id: req.parent_id || null,
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  }
  return await invoke<CostCentre>('create_cost_centre', {
    req,
    companyId: companyId || null,
  });
}

export async function getVoucherCostAllocations(
  voucherId: string,
  companyId?: string
): Promise<VoucherCostAllocationItem[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<VoucherCostAllocationItem[]>('get_voucher_cost_allocations', {
    voucherId,
    companyId: companyId || null,
  });
}

export async function getCostCentreBreakupReport(
  companyId?: string,
  fromDate?: string,
  toDate?: string
): Promise<CostCentreBreakupReport> {
  if (!isTauriEnv()) {
    return {
      from_date: fromDate || null,
      to_date: toDate || null,
      centres: [
        {
          cost_centre_id: 'cc_north',
          cost_centre_name: 'North Region Sales',
          category_id: 'cat_primary',
          category_name: 'Primary Cost Category',
          debit_total: '25000.00',
          credit_total: '0.00',
          net_amount: '25000.00',
          allocations_count: 2,
        },
        {
          cost_centre_id: 'cc_south',
          cost_centre_name: 'South Region Sales',
          category_id: 'cat_primary',
          category_name: 'Primary Cost Category',
          debit_total: '35000.00',
          credit_total: '0.00',
          net_amount: '35000.00',
          allocations_count: 3,
        },
      ],
      total_debit: '60000.00',
      total_credit: '0.00',
      net_total: '60000.00',
    };
  }
  return await invoke<CostCentreBreakupReport>('get_cost_centre_breakup_report', {
    companyId: companyId || null,
    fromDate: fromDate || null,
    toDate: toDate || null,
  });
}

// =============================================================================
// Milestone 12: Ratio Analysis, Budgets, Year Closing & MCA Audit Certification
// =============================================================================

export async function getRatioAnalysis(
  companyId?: string,
  fromDate?: string,
  toDate?: string
): Promise<RatioAnalysisReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_sample',
      from_date: fromDate || '2024-04-01',
      to_date: toDate || '2025-03-31',
      working_capital: '450000.00',
      cash_in_hand: '25000.00',
      bank_accounts: '380000.00',
      bank_od_ac: '0.00',
      sundry_debtors: '185000.00',
      sundry_creditors: '140000.00',
      sales_accounts: '1250000.00',
      purchase_accounts: '720000.00',
      closing_stock: '150000.00',
      net_profit: '235000.00',
      current_ratio: '3.21',
      quick_ratio: '2.14',
      debt_equity_ratio: '0.28',
      gross_profit_pct: '42.40%',
      net_profit_pct: '18.80%',
      operating_cost_pct: '38.80%',
      receivables_turnover_days: '54 days',
      inventory_turnover: '4.80',
      return_on_working_capital_pct: '52.22%',
    };
  }
  return await invoke<RatioAnalysisReport>('get_ratio_analysis', {
    companyId: companyId || null,
    fromDate: fromDate || null,
    toDate: toDate || null,
  });
}

export async function listBudgets(companyId?: string): Promise<Budget[]> {
  if (!isTauriEnv()) {
    return [
      {
        id: 'bdg_1',
        company_id: companyId || 'cmp_sample',
        name: 'Annual Operations Budget 2024-25',
        from_date: '2024-04-01',
        to_date: '2025-03-31',
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
    ];
  }
  return await invoke<Budget[]>('list_budgets', {
    companyId: companyId || null,
  });
}

export async function createBudget(
  req: BudgetRequest,
  companyId?: string
): Promise<BudgetDetails> {
  if (!isTauriEnv()) {
    return {
      budget: {
        id: 'bdg_mock',
        company_id: companyId || 'cmp_sample',
        name: req.name,
        from_date: req.from_date,
        to_date: req.to_date,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: req.lines.map((l, idx) => ({
        id: `bdg_line_${idx}`,
        company_id: companyId || 'cmp_sample',
        budget_id: 'bdg_mock',
        target_type: l.target_type,
        target_id: l.target_id,
        budget_type: l.budget_type,
        amount: String(l.amount),
        dr_cr: l.dr_cr,
        created_at: new Date().toISOString(),
      })),
    };
  }
  return await invoke<BudgetDetails>('create_budget', {
    companyId: companyId || null,
    req,
  });
}

export async function getBudget(
  budgetId: string,
  companyId?: string
): Promise<BudgetDetails> {
  if (!isTauriEnv()) {
    return {
      budget: {
        id: budgetId,
        company_id: companyId || 'cmp_sample',
        name: 'Annual Operations Budget 2024-25',
        from_date: '2024-04-01',
        to_date: '2025-03-31',
        created_at: '2024-04-01T00:00:00Z',
        updated_at: '2024-04-01T00:00:00Z',
      },
      lines: [
        {
          id: 'line_1',
          company_id: companyId || 'cmp_sample',
          budget_id: budgetId,
          target_type: 'Group',
          target_id: 'grp_indirect_expenses',
          budget_type: 'OnNetTransactions',
          amount: '50000.00',
          dr_cr: 'Dr',
          created_at: '2024-04-01T00:00:00Z',
        },
      ],
    };
  }
  return await invoke<BudgetDetails>('get_budget', {
    companyId: companyId || null,
    budgetId,
  });
}

export async function deleteBudget(
  budgetId: string,
  companyId?: string
): Promise<void> {
  if (!isTauriEnv()) {
    return;
  }
  return await invoke<void>('delete_budget', {
    companyId: companyId || null,
    budgetId,
  });
}

export async function getBudgetVarianceReport(
  budgetId: string,
  asOfDate: string,
  companyId?: string
): Promise<BudgetVarianceReport> {
  if (!isTauriEnv()) {
    return {
      budget_id: budgetId,
      budget_name: 'Annual Operations Budget 2024-25',
      company_id: companyId || 'cmp_sample',
      as_of_date: asOfDate,
      from_date: '2024-04-01',
      to_date: '2025-03-31',
      rows: [
        {
          line_id: 'l1',
          target_type: 'Group',
          target_id: 'grp_indirect_expenses',
          target_name: 'Indirect Expenses',
          budget_type: 'OnNetTransactions',
          dr_cr: 'Dr',
          budget_amount: '50000.00',
          actual_amount: '42000.00',
          variance_amount: '8000.00',
          utilization_pct: '84.00%',
          is_over_budget: false,
        },
      ],
      total_budget: '50000.00',
      total_actual: '42000.00',
      total_variance: '8000.00',
    };
  }
  return await invoke<BudgetVarianceReport>('get_budget_variance_report', {
    companyId: companyId || null,
    budgetId,
    asOfDate,
  });
}

export async function previewYearEndClosing(
  periodEnd: string,
  companyId?: string
): Promise<YearEndClosingPreview> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_sample',
      financial_year_label: 'FY 2024-25',
      period_start: '2024-04-01',
      period_end: periodEnd,
      next_fy_start: '2025-04-01',
      trial_balance_difference: '0.00',
      is_equilibrium: true,
      net_profit_or_loss: '235000.00',
      is_profit: true,
      nominal_transfers: [
        {
          ledger_id: 'led_sales',
          ledger_name: 'Sales Accounts',
          account_group_nature: 'Income',
          closing_debit: '0.00',
          closing_credit: '1250000.00',
          transfer_entry_type: 'Dr',
          transfer_amount: '1250000.00',
        },
        {
          ledger_id: 'led_pur',
          ledger_name: 'Purchase Accounts',
          account_group_nature: 'Expenses',
          closing_debit: '720000.00',
          closing_credit: '0.00',
          transfer_entry_type: 'Cr',
          transfer_amount: '720000.00',
        },
      ],
      balance_sheet_balances: [
        {
          ledger_id: 'led_bank',
          ledger_name: 'HDFC Bank A/c',
          account_group_nature: 'Assets',
          balance_type: 'Dr',
          amount: '380000.00',
        },
      ],
    };
  }
  return await invoke<YearEndClosingPreview>('preview_year_end_closing', {
    companyId: companyId || null,
    periodEnd,
  });
}

export async function executeYearEndClosing(
  periodEnd: string,
  closedBy?: string,
  companyId?: string
): Promise<FinancialYearClosingRecord> {
  if (!isTauriEnv()) {
    return {
      id: 'fyc_mock',
      company_id: companyId || 'cmp_sample',
      financial_year_label: 'FY 2024-25',
      period_start: '2024-04-01',
      period_end: periodEnd,
      next_fy_start: '2025-04-01',
      closing_voucher_id: 'vch_closing_mock',
      net_profit_or_loss: '235000.00',
      is_locked: true,
      closed_at: new Date().toISOString(),
      closed_by: closedBy || 'system',
    };
  }
  return await invoke<FinancialYearClosingRecord>('execute_year_end_closing', {
    companyId: companyId || null,
    periodEnd,
    closedBy: closedBy || null,
  });
}

export async function listClosedYears(
  companyId?: string
): Promise<FinancialYearClosingRecord[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<FinancialYearClosingRecord[]>('list_closed_years', {
    companyId: companyId || null,
  });
}

export async function verifyAuditTrailIntegrity(
  companyId?: string
): Promise<AuditCertificationReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_sample',
      generated_at: new Date().toISOString(),
      total_entries: 42,
      chained_entries: 42,
      legacy_unchained_entries: 0,
      tampered_entries: 0,
      broken_chain_links: 0,
      is_valid: true,
      certification_status: 'VERIFIED_CLEAN',
      first_entry_timestamp: '2024-04-01T10:00:00Z',
      latest_entry_timestamp: '2025-03-31T18:00:00Z',
      latest_entry_hash: '3a7b9c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b',
      issues: [],
    };
  }
  return await invoke<AuditCertificationReport>('verify_audit_trail_integrity', {
    companyId: companyId || null,
  });
}

export async function exportAuditCertificationCsv(
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) {
    return 'Summary Parameter,Value\nCompany ID,"cmp_sample"\nStatus,"VERIFIED_CLEAN"\n';
  }
  return await invoke<string>('export_audit_certification_csv', {
    companyId: companyId || null,
  });
}

// =========================================================================
// MILESTONE 13: ORDER PROCESSING
// =========================================================================

export interface PurchaseOrder {
  id: string;
  company_id: string;
  order_number: string;
  order_date: string;
  due_date: string | null;
  party_ledger_id: string;
  party_ledger_name?: string | null;
  reference_no: string | null;
  narration: string | null;
  status: 'Open' | 'PartiallyFulfilled' | 'Fulfilled' | 'Cancelled';
  total_amount: string;
  created_at: string;
  updated_at: string;
}

export interface PurchaseOrderLine {
  id: string;
  company_id: string;
  order_id: string;
  stock_item_id: string;
  stock_item_name?: string | null;
  godown_id: string | null;
  godown_name?: string | null;
  ordered_qty: string;
  fulfilled_qty: string;
  pending_qty: string;
  unit_rate: string;
  discount_kind?: string | null;
  discount_value?: string | null;
  line_amount: string;
  created_at: string;
}

export interface PurchaseOrderWithLines {
  order: PurchaseOrder;
  lines: PurchaseOrderLine[];
}

export interface SalesOrder {
  id: string;
  company_id: string;
  order_number: string;
  order_date: string;
  due_date: string | null;
  party_ledger_id: string;
  party_ledger_name?: string | null;
  reference_no: string | null;
  narration: string | null;
  status: 'Open' | 'PartiallyFulfilled' | 'Fulfilled' | 'Cancelled';
  total_amount: string;
  created_at: string;
  updated_at: string;
}

export interface SalesOrderLine {
  id: string;
  company_id: string;
  order_id: string;
  stock_item_id: string;
  stock_item_name?: string | null;
  godown_id: string | null;
  godown_name?: string | null;
  ordered_qty: string;
  fulfilled_qty: string;
  pending_qty: string;
  unit_rate: string;
  discount_kind?: string | null;
  discount_value?: string | null;
  line_amount: string;
  created_at: string;
}

export interface SalesOrderWithLines {
  order: SalesOrder;
  lines: SalesOrderLine[];
}

export interface CreateOrderLineInput {
  stock_item_id: string;
  godown_id?: string | null;
  ordered_qty: string;
  unit_rate: string;
  discount_kind?: string | null;
  discount_value?: string | null;
}

export interface CreatePurchaseOrderInput {
  order_number: string;
  order_date: string;
  due_date?: string | null;
  party_ledger_id: string;
  reference_no?: string | null;
  narration?: string | null;
  lines: CreateOrderLineInput[];
}

export interface CreateSalesOrderInput {
  order_number: string;
  order_date: string;
  due_date?: string | null;
  party_ledger_id: string;
  reference_no?: string | null;
  narration?: string | null;
  lines: CreateOrderLineInput[];
}

export interface FulfillOrderLineInput {
  order_line_id: string;
  fulfill_qty: string;
  unit_rate?: string | null;
  godown_id?: string | null;
  batch_number?: string | null;
  mfg_date?: string | null;
  expiry_date?: string | null;
}

export interface ConvertPurchaseOrderInput {
  order_id: string;
  invoice_date: string;
  series_id: string;
  commercial_account_ledger_id?: string | null;
  lines: FulfillOrderLineInput[];
  tax_lines?: any[] | null;
  narration?: string | null;
  reference_no?: string | null;
}

export interface ConvertSalesOrderInput {
  order_id: string;
  invoice_date: string;
  series_id: string;
  commercial_account_ledger_id?: string | null;
  lines: FulfillOrderLineInput[];
  tax_lines?: any[] | null;
  narration?: string | null;
  reference_no?: string | null;
}

export interface OrderFulfillmentRecord {
  id: string;
  company_id: string;
  order_type: string;
  order_id: string;
  order_line_id: string;
  voucher_id: string;
  commercial_doc_id: string | null;
  fulfilled_qty: string;
  fulfillment_date: string;
  is_cancelled: boolean;
  created_at: string;
}

export interface ConvertOrderResult {
  voucher_id: string;
  voucher_number: string;
  order_status: string;
  fulfillments: OrderFulfillmentRecord[];
}

export interface PendingOrderItem {
  order_id: string;
  order_number: string;
  order_date: string;
  due_date: string | null;
  party_ledger_id: string;
  party_name: string;
  stock_item_id: string;
  stock_item_name: string;
  godown_id: string | null;
  godown_name: string | null;
  ordered_qty: string;
  fulfilled_qty: string;
  pending_qty: string;
  unit_rate: string;
  pending_amount: string;
  is_overdue: boolean;
}

export interface PendingOrdersReport {
  company_id: string;
  order_type: string;
  as_of_date: string;
  items: PendingOrderItem[];
  total_pending_qty: string;
  total_pending_amount: string;
}

export async function listPurchaseOrders(
  statusFilter?: string,
  companyId?: string
): Promise<PurchaseOrder[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<PurchaseOrder[]>('list_purchase_orders', {
    companyId: companyId || null,
    statusFilter: statusFilter || null,
  });
}

export async function createPurchaseOrder(
  input: CreatePurchaseOrderInput,
  companyId?: string
): Promise<PurchaseOrderWithLines> {
  if (!isTauriEnv()) {
    return {
      order: {
        id: 'po_mock_1',
        company_id: companyId || 'cmp_sample',
        order_number: input.order_number,
        order_date: input.order_date,
        due_date: input.due_date || null,
        party_ledger_id: input.party_ledger_id,
        party_ledger_name: 'Mock Supplier',
        reference_no: input.reference_no || null,
        narration: input.narration || null,
        status: 'Open',
        total_amount: '50000.00',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: [],
    };
  }
  return await invoke<PurchaseOrderWithLines>('create_purchase_order', {
    companyId: companyId || null,
    input,
  });
}

export async function getPurchaseOrder(
  orderId: string,
  companyId?: string
): Promise<PurchaseOrderWithLines> {
  if (!isTauriEnv()) {
    return {
      order: {
        id: orderId,
        company_id: companyId || 'cmp_sample',
        order_number: 'PO-001',
        order_date: '2026-05-01',
        due_date: '2026-05-15',
        party_ledger_id: 'led_sup_1',
        party_ledger_name: 'Acme Supplies',
        reference_no: 'REF-101',
        narration: 'Urgent order',
        status: 'Open',
        total_amount: '15000.00',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: [],
    };
  }
  return await invoke<PurchaseOrderWithLines>('get_purchase_order', {
    companyId: companyId || null,
    orderId,
  });
}

export async function cancelPurchaseOrder(
  orderId: string,
  reason: string,
  companyId?: string
): Promise<void> {
  if (!isTauriEnv()) {
    return;
  }
  await invoke<void>('cancel_purchase_order', {
    companyId: companyId || null,
    orderId,
    reason,
  });
}

export async function convertPurchaseOrderToInvoice(
  input: ConvertPurchaseOrderInput,
  companyId?: string
): Promise<ConvertOrderResult> {
  if (!isTauriEnv()) {
    return {
      voucher_id: 'vch_inv_mock',
      voucher_number: 'PUR-001',
      order_status: 'Fulfilled',
      fulfillments: [],
    };
  }
  return await invoke<ConvertOrderResult>('convert_purchase_order_to_invoice', {
    companyId: companyId || null,
    input,
  });
}

export async function listSalesOrders(
  statusFilter?: string,
  companyId?: string
): Promise<SalesOrder[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<SalesOrder[]>('list_sales_orders', {
    companyId: companyId || null,
    statusFilter: statusFilter || null,
  });
}

export async function createSalesOrder(
  input: CreateSalesOrderInput,
  companyId?: string
): Promise<SalesOrderWithLines> {
  if (!isTauriEnv()) {
    return {
      order: {
        id: 'so_mock_1',
        company_id: companyId || 'cmp_sample',
        order_number: input.order_number,
        order_date: input.order_date,
        due_date: input.due_date || null,
        party_ledger_id: input.party_ledger_id,
        party_ledger_name: 'Mock Customer',
        reference_no: input.reference_no || null,
        narration: input.narration || null,
        status: 'Open',
        total_amount: '40000.00',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: [],
    };
  }
  return await invoke<SalesOrderWithLines>('create_sales_order', {
    companyId: companyId || null,
    input,
  });
}

export async function getSalesOrder(
  orderId: string,
  companyId?: string
): Promise<SalesOrderWithLines> {
  if (!isTauriEnv()) {
    return {
      order: {
        id: orderId,
        company_id: companyId || 'cmp_sample',
        order_number: 'SO-001',
        order_date: '2026-05-01',
        due_date: '2026-05-20',
        party_ledger_id: 'led_cust_1',
        party_ledger_name: 'Apex Traders',
        reference_no: 'PO-CLIENT-99',
        narration: 'Customer order',
        status: 'Open',
        total_amount: '25000.00',
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      lines: [],
    };
  }
  return await invoke<SalesOrderWithLines>('get_sales_order', {
    companyId: companyId || null,
    orderId,
  });
}

export async function cancelSalesOrder(
  orderId: string,
  reason: string,
  companyId?: string
): Promise<void> {
  if (!isTauriEnv()) {
    return;
  }
  await invoke<void>('cancel_sales_order', {
    companyId: companyId || null,
    orderId,
    reason,
  });
}

export async function convertSalesOrderToInvoice(
  input: ConvertSalesOrderInput,
  companyId?: string
): Promise<ConvertOrderResult> {
  if (!isTauriEnv()) {
    return {
      voucher_id: 'vch_sales_mock',
      voucher_number: 'SALES-001',
      order_status: 'Fulfilled',
      fulfillments: [],
    };
  }
  return await invoke<ConvertOrderResult>('convert_sales_order_to_invoice', {
    companyId: companyId || null,
    input,
  });
}

export async function getPendingPurchaseOrders(
  asOfDate: string,
  companyId?: string
): Promise<PendingOrdersReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_sample',
      order_type: 'PurchaseOrder',
      as_of_date: asOfDate,
      items: [],
      total_pending_qty: '0',
      total_pending_amount: '0.00',
    };
  }
  return await invoke<PendingOrdersReport>('get_pending_purchase_orders', {
    companyId: companyId || null,
    asOfDate,
  });
}

export async function getPendingSalesOrders(
  asOfDate: string,
  companyId?: string
): Promise<PendingOrdersReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'cmp_sample',
      order_type: 'SalesOrder',
      as_of_date: asOfDate,
      items: [],
      total_pending_qty: '0',
      total_pending_amount: '0.00',
    };
  }
  return await invoke<PendingOrdersReport>('get_pending_sales_orders', {
    companyId: companyId || null,
    asOfDate,
  });
}

// ---------------------------------------------------------------------------
// Milestone 14: Security Control & Role-Based Access Control (RBAC) API
// ---------------------------------------------------------------------------

export type UserRole = 'Admin' | 'Accountant' | 'DataEntry' | 'Auditor';

export interface CompanyUser {
  id: string;
  company_id: string;
  username: string;
  role: string;
  is_active: number;
  created_at: string;
  last_login_at?: string | null;
}

export interface AuthSessionInfo {
  user_id: string;
  username: string;
  role: UserRole;
  logged_in_at: string;
}

export interface SecurityStatusDto {
  security_enabled: boolean;
  has_users: boolean;
  active_user?: AuthSessionInfo | null;
}

export interface EnableSecurityInput {
  company_id: string;
  admin_username: string;
  admin_password: string;
}

export interface LoginInput {
  company_id: string;
  username: string;
  password: string;
}

export interface CreateUserInput {
  company_id: string;
  username: string;
  password: string;
  role: UserRole;
}

export interface UpdateUserRoleInput {
  company_id: string;
  user_id: string;
  role: UserRole;
}

export interface UpdateUserPasswordInput {
  company_id: string;
  user_id: string;
  new_password: string;
}

export interface DeactivateUserInput {
  company_id: string;
  user_id: string;
}

export async function getCompanySecurityStatus(
  companyId?: string
): Promise<SecurityStatusDto> {
  if (!isTauriEnv()) {
    return {
      security_enabled: false,
      has_users: false,
      active_user: null,
    };
  }
  return await invoke<SecurityStatusDto>('get_company_security_status', {
    companyId: companyId || null,
  });
}

export async function enableCompanySecurity(
  input: EnableSecurityInput
): Promise<AuthSessionInfo> {
  if (!isTauriEnv()) {
    return {
      user_id: 'usr_admin_mock',
      username: input.admin_username,
      role: 'Admin',
      logged_in_at: new Date().toISOString(),
    };
  }
  return await invoke<AuthSessionInfo>('enable_company_security', { input });
}

export async function authenticateCompanyUser(
  input: LoginInput
): Promise<AuthSessionInfo> {
  if (!isTauriEnv()) {
    return {
      user_id: 'usr_mock',
      username: input.username,
      role: 'Admin',
      logged_in_at: new Date().toISOString(),
    };
  }
  return await invoke<AuthSessionInfo>('authenticate_company_user', { input });
}

export async function logoutCompanySession(companyId?: string): Promise<void> {
  if (!isTauriEnv()) return;
  await invoke<void>('logout_company_session', { companyId: companyId || null });
}

export async function listCompanyUsers(
  companyId?: string
): Promise<CompanyUser[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<CompanyUser[]>('list_company_users', {
    companyId: companyId || null,
  });
}

export async function createCompanyUser(
  input: CreateUserInput
): Promise<CompanyUser> {
  if (!isTauriEnv()) {
    return {
      id: 'usr_new_mock',
      company_id: input.company_id,
      username: input.username,
      role: input.role,
      is_active: 1,
      created_at: new Date().toISOString(),
    };
  }
  return await invoke<CompanyUser>('create_company_user', { input });
}

export async function updateCompanyUserRole(
  input: UpdateUserRoleInput
): Promise<CompanyUser> {
  if (!isTauriEnv()) {
    return {
      id: input.user_id,
      company_id: input.company_id,
      username: 'mock_user',
      role: input.role,
      is_active: 1,
      created_at: new Date().toISOString(),
    };
  }
  return await invoke<CompanyUser>('update_company_user_role', { input });
}

export async function updateCompanyUserPassword(
  input: UpdateUserPasswordInput
): Promise<void> {
  if (!isTauriEnv()) return;
  await invoke<void>('update_company_user_password', { input });
}

export async function deactivateCompanyUser(
  input: DeactivateUserInput
): Promise<void> {
  if (!isTauriEnv()) return;
  await invoke<void>('deactivate_company_user', { input });
}

// ---------------------------------------------------------------------------
// Milestone 14: Manufacturing Bill of Materials (BOM) & Production Journal API
// ---------------------------------------------------------------------------

export interface BomComponentDetail {
  id: string;
  raw_stock_item_id: string;
  raw_stock_item_name: string;
  source_godown_id?: string | null;
  source_godown_name?: string | null;
  required_qty: string;
}

export interface BomDetail {
  id: string;
  company_id: string;
  finished_stock_item_id: string;
  finished_stock_item_name: string;
  bom_name: string;
  base_output_qty: string;
  default_godown_id?: string | null;
  default_godown_name?: string | null;
  is_active: boolean;
  created_at: string;
  updated_at: string;
  components: BomComponentDetail[];
}

export interface CreateBomComponentInput {
  raw_stock_item_id: string;
  source_godown_id?: string | null;
  required_qty: string;
}

export interface CreateBomInput {
  company_id: string;
  finished_stock_item_id: string;
  bom_name: string;
  base_output_qty: string;
  default_godown_id?: string | null;
  components: CreateBomComponentInput[];
}

export interface AdditionalOverheadCostInput {
  overhead_ledger_id: string;
  amount: string;
  description?: string | null;
}

export interface ExecuteManufacturingJournalInput {
  company_id: string;
  bom_id: string;
  production_date: string;
  produced_qty: string;
  destination_godown_id: string;
  batch_number?: string | null;
  mfg_date?: string | null;
  expiry_date?: string | null;
  additional_overheads: AdditionalOverheadCostInput[];
  narration?: string | null;
}

export interface ConsumedComponentResult {
  raw_stock_item_id: string;
  raw_stock_item_name: string;
  godown_id: string;
  consumed_qty: string;
  consumed_cost: string;
}

export interface ManufacturingJournalResult {
  stock_voucher_id: string;
  finished_goods_cost_layer_id: string;
  produced_qty: string;
  landed_unit_cost: string;
  total_raw_material_cost: string;
  total_overhead_cost: string;
  total_manufacturing_cost: string;
  consumed_components: ConsumedComponentResult[];
}

export async function listBoms(
  companyId?: string,
  finishedStockItemId?: string
): Promise<BomDetail[]> {
  if (!isTauriEnv()) {
    return [];
  }
  return await invoke<BomDetail[]>('list_boms', {
    companyId: companyId || null,
    finishedStockItemId: finishedStockItemId || null,
  });
}

export async function getBomDetail(
  bomId: string,
  companyId?: string
): Promise<BomDetail> {
  if (!isTauriEnv()) {
    return {
      id: bomId,
      company_id: companyId || 'cmp_sample',
      finished_stock_item_id: 'item_sample',
      finished_stock_item_name: 'Sample Finished Item',
      bom_name: 'Standard Recipe',
      base_output_qty: '1',
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      components: [],
    };
  }
  return await invoke<BomDetail>('get_bom_detail', {
    companyId: companyId || null,
    bomId,
  });
}

export async function createBom(input: CreateBomInput): Promise<BomDetail> {
  if (!isTauriEnv()) {
    return {
      id: 'bom_new_mock',
      company_id: input.company_id,
      finished_stock_item_id: input.finished_stock_item_id,
      finished_stock_item_name: 'Mock Item',
      bom_name: input.bom_name,
      base_output_qty: input.base_output_qty,
      is_active: true,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      components: [],
    };
  }
  return await invoke<BomDetail>('create_bom', { input });
}

export async function executeManufacturingJournal(
  input: ExecuteManufacturingJournalInput
): Promise<ManufacturingJournalResult> {
  if (!isTauriEnv()) {
    return {
      stock_voucher_id: 'sv_mfg_mock',
      finished_goods_cost_layer_id: 'layer_fg_mock',
      produced_qty: input.produced_qty,
      landed_unit_cost: '100.0000',
      total_raw_material_cost: '90.00',
      total_overhead_cost: '10.00',
      total_manufacturing_cost: '100.00',
      consumed_components: [],
    };
  }
  return await invoke<ManufacturingJournalResult>('execute_manufacturing_journal', {
    input,
  });
}

// ---------------------------------------------------------------------------
// Milestone 15: Indian Payroll Engine, Statutory PF/ESI/PT & Payslip PDF
// ---------------------------------------------------------------------------

export interface Employee {
  id: string;
  company_id: string;
  employee_code: string;
  name: string;
  department_group_id?: string | null;
  designation?: string | null;
  date_of_joining: string;
  date_of_exit?: string | null;
  pan?: string | null;
  uan?: string | null;
  esic_number?: string | null;
  bank_account_no?: string | null;
  bank_ifsc?: string | null;
  basic_salary: string;
  hra: string;
  other_allowances: string;
  pf_applicable: boolean;
  esi_applicable: boolean;
  pt_applicable: boolean;
  is_active: boolean;
  created_at: string;
}

export interface CreateEmployeeInput {
  employee_code: string;
  name: string;
  department_group_id?: string | null;
  designation?: string | null;
  date_of_joining: string;
  date_of_exit?: string | null;
  pan?: string | null;
  uan?: string | null;
  esic_number?: string | null;
  bank_account_no?: string | null;
  bank_ifsc?: string | null;
  basic_salary: string;
  hra?: string;
  other_allowances?: string;
  pf_applicable?: boolean;
  esi_applicable?: boolean;
  pt_applicable?: boolean;
}

export interface PayrollAttendanceInput {
  employee_id: string;
  total_working_days: string;
  days_present: string;
  other_deductions?: string;
}

export interface PayrollRunInput {
  pay_period: string;
  run_date: string;
  narration?: string | null;
  attendances: PayrollAttendanceInput[];
}

export interface ComputedPayslipPreview {
  employee_id: string;
  employee_code: string;
  employee_name: string;
  designation?: string | null;
  total_working_days: string;
  days_present: string;
  lop_days: string;
  prorated_basic: string;
  prorated_hra: string;
  prorated_allowances: string;
  gross_earnings: string;
  employee_pf: string;
  employer_epf: string;
  employer_eps: string;
  employee_esi: string;
  employer_esi: string;
  professional_tax: string;
  other_deductions: string;
  total_deductions: string;
  net_pay: string;
}

export interface PayrollPreviewResult {
  pay_period: string;
  total_gross: string;
  total_deductions: string;
  total_employer_contributions: string;
  total_net_pay: string;
  total_employee_pf: string;
  total_employer_epf: string;
  total_employer_eps: string;
  total_employee_esi: string;
  total_employer_esi: string;
  total_pt: string;
  payslips: ComputedPayslipPreview[];
}

export interface PayrollRun {
  id: string;
  company_id: string;
  pay_period: string;
  run_date: string;
  voucher_id: string;
  total_gross: string;
  total_deductions: string;
  total_employer_contributions: string;
  total_net_pay: string;
  status: 'Posted' | 'Cancelled' | string;
  created_at: string;
}

export interface PayrollPayslip {
  id: string;
  company_id: string;
  payroll_run_id: string;
  employee_id: string;
  total_working_days: string;
  days_present: string;
  lop_days: string;
  prorated_basic: string;
  prorated_hra: string;
  prorated_allowances: string;
  gross_earnings: string;
  employee_pf: string;
  employer_epf: string;
  employer_eps: string;
  employee_esi: string;
  employer_esi: string;
  professional_tax: string;
  other_deductions: string;
  total_deductions: string;
  net_pay: string;
  created_at: string;
}

export interface PayrollRunResult {
  run: PayrollRun;
  voucher_number: string;
  payslips: PayrollPayslip[];
}

export interface PayrollStatutoryRow {
  payslip_id: string;
  employee_id: string;
  employee_code: string;
  employee_name: string;
  pan?: string | null;
  uan?: string | null;
  esic_number?: string | null;
  total_working_days: string;
  days_present: string;
  lop_days: string;
  gross_earnings: string;
  employee_pf: string;
  employer_epf: string;
  employer_eps: string;
  employee_esi: string;
  employer_esi: string;
  professional_tax: string;
  other_deductions: string;
  total_deductions: string;
  net_pay: string;
}

export interface PayrollStatutorySummary {
  pay_period: string;
  run_id?: string | null;
  run_status?: string | null;
  voucher_id?: string | null;
  total_employees: number;
  total_gross: string;
  total_employee_pf: string;
  total_employer_epf: string;
  total_employer_eps: string;
  total_pf_remittance: string;
  total_employee_esi: string;
  total_employer_esi: string;
  total_esi_remittance: string;
  total_pt_remittance: string;
  total_net_pay: string;
  rows: PayrollStatutoryRow[];
}

export async function listEmployees(
  companyId?: string,
  includeInactive = false
): Promise<Employee[]> {
  if (!isTauriEnv()) return [];
  return await invoke<Employee[]>('list_employees', {
    companyId: companyId || null,
    includeInactive,
  });
}

export async function createEmployee(
  input: CreateEmployeeInput,
  companyId?: string
): Promise<Employee> {
  if (!isTauriEnv()) {
    return {
      id: `emp_${Date.now()}`,
      company_id: companyId || 'cmp_sample',
      employee_code: input.employee_code,
      name: input.name,
      designation: input.designation || null,
      date_of_joining: input.date_of_joining,
      basic_salary: input.basic_salary,
      hra: input.hra || '0',
      other_allowances: input.other_allowances || '0',
      pf_applicable: input.pf_applicable ?? true,
      esi_applicable: input.esi_applicable ?? false,
      pt_applicable: input.pt_applicable ?? true,
      is_active: true,
      created_at: new Date().toISOString(),
    };
  }
  return await invoke<Employee>('create_employee', {
    companyId: companyId || null,
    input,
  });
}

export async function deactivateEmployee(
  employeeId: string,
  companyId?: string
): Promise<void> {
  if (!isTauriEnv()) return;
  return await invoke<void>('deactivate_employee', {
    companyId: companyId || null,
    employeeId,
  });
}

export async function previewPayrollRun(
  input: PayrollRunInput,
  companyId?: string
): Promise<PayrollPreviewResult> {
  if (!isTauriEnv()) {
    return {
      pay_period: input.pay_period,
      total_gross: '0',
      total_deductions: '0',
      total_employer_contributions: '0',
      total_net_pay: '0',
      total_employee_pf: '0',
      total_employer_epf: '0',
      total_employer_eps: '0',
      total_employee_esi: '0',
      total_employer_esi: '0',
      total_pt: '0',
      payslips: [],
    };
  }
  return await invoke<PayrollPreviewResult>('preview_payroll_run', {
    companyId: companyId || null,
    input,
  });
}

export async function executePayrollRun(
  input: PayrollRunInput,
  companyId?: string
): Promise<PayrollRunResult> {
  if (!isTauriEnv()) {
    throw new Error('executePayrollRun requires Tauri backend');
  }
  return await invoke<PayrollRunResult>('execute_payroll_run', {
    companyId: companyId || null,
    input,
  });
}

export async function listPayrollRuns(companyId?: string): Promise<PayrollRun[]> {
  if (!isTauriEnv()) return [];
  return await invoke<PayrollRun[]>('list_payroll_runs', {
    companyId: companyId || null,
  });
}

export async function getPayrollStatutorySummary(
  payPeriod: string,
  companyId?: string
): Promise<PayrollStatutorySummary> {
  if (!isTauriEnv()) {
    return {
      pay_period: payPeriod,
      total_employees: 0,
      total_gross: '0',
      total_employee_pf: '0',
      total_employer_epf: '0',
      total_employer_eps: '0',
      total_pf_remittance: '0',
      total_employee_esi: '0',
      total_employer_esi: '0',
      total_esi_remittance: '0',
      total_pt_remittance: '0',
      total_net_pay: '0',
      rows: [],
    };
  }
  return await invoke<PayrollStatutorySummary>('get_payroll_statutory_summary', {
    companyId: companyId || null,
    payPeriod,
  });
}

export async function exportPayslipPdf(
  payslipId: string,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return await invoke<number[]>('export_payslip_pdf', {
    companyId: companyId || null,
    payslipId,
  });
}

export async function exportPayrollCsv(
  payPeriod: string,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) return '';
  return await invoke<string>('export_payroll_csv', {
    companyId: companyId || null,
    payPeriod,
  });
}

// ---------------------------------------------------------------------------
// Milestone 17: Multi-User LAN Host / Client Mode API & Transport
// ---------------------------------------------------------------------------

import type {
  LanSessionDto,
  LanServerStatusDto,
  LanClientConfig,
  LanLoginResponse,
} from '../types';

export type {
  LanSessionDto,
  LanServerStatusDto,
  LanClientConfig,
  LanLoginResponse,
};

let currentLanClientConfig: LanClientConfig | null = null;

try {
  const cached = sessionStorage.getItem('opentally_lan_client');
  if (cached) {
    currentLanClientConfig = JSON.parse(cached);
  }
} catch {
  // Ignore sessionStorage errors
}

export function getLanClientConfig(): LanClientConfig | null {
  return currentLanClientConfig;
}

export function setLanClientConfig(config: LanClientConfig | null): void {
  currentLanClientConfig = config;
  try {
    if (config) {
      sessionStorage.setItem('opentally_lan_client', JSON.stringify(config));
    } else {
      sessionStorage.removeItem('opentally_lan_client');
    }
  } catch {
    // Ignore
  }
}

export function isLanClientMode(): boolean {
  return currentLanClientConfig !== null;
}

export async function callLanRpc<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
  if (!currentLanClientConfig) {
    throw new Error('Not connected to a LAN server');
  }

  const url = `http://${currentLanClientConfig.host}:${currentLanClientConfig.port}/api/v1/rpc`;
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${currentLanClientConfig.token}`,
    },
    body: JSON.stringify({
      command,
      params: {
        company_id: currentLanClientConfig.company_id,
        ...params,
      },
    }),
  });

  if (res.status === 401) {
    setLanClientConfig(null);
    throw new Error('LAN session expired or unauthorized. Please re-authenticate.');
  }

  if (res.status === 403) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error?.message || 'Access Denied: You do not have permission to perform this action.');
  }

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error?.message || `LAN Server Error (${res.status})`);
  }

  const data = await res.json();
  if (data.error) {
    throw new Error(data.error.message || 'LAN RPC Error');
  }

  return data.result as T;
}

export async function startLanServer(
  bindAddress?: string,
  port?: number
): Promise<LanServerStatusDto> {
  if (!isTauriEnv()) {
    return {
      is_running: true,
      bind_address: bindAddress || '0.0.0.0',
      port: port || 7900,
      local_ip_addresses: ['192.168.1.100', '127.0.0.1'],
      active_sessions: [],
    };
  }
  return await invoke<LanServerStatusDto>('start_lan_server', {
    bindAddress: bindAddress || null,
    port: port || null,
  });
}

export async function stopLanServer(): Promise<LanServerStatusDto> {
  if (!isTauriEnv()) {
    return {
      is_running: false,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['127.0.0.1'],
      active_sessions: [],
    };
  }
  return await invoke<LanServerStatusDto>('stop_lan_server');
}

export async function getLanServerStatus(): Promise<LanServerStatusDto> {
  if (!isTauriEnv()) {
    return {
      is_running: false,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['127.0.0.1'],
      active_sessions: [],
    };
  }
  return await invoke<LanServerStatusDto>('get_lan_server_status');
}

export async function revokeLanSession(sessionId: string): Promise<LanServerStatusDto> {
  if (!isTauriEnv()) {
    return {
      is_running: true,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['127.0.0.1'],
      active_sessions: [],
    };
  }
  return await invoke<LanServerStatusDto>('revoke_lan_session', { sessionId });
}

export async function testLanHealth(
  host: string,
  port: number
): Promise<{ app: string; version: string; mode: string; status: string }> {
  const res = await fetch(`http://${host}:${port}/api/v1/health`);
  if (!res.ok) {
    throw new Error(`LAN server health check failed with status ${res.status}`);
  }
  return await res.json();
}

export async function loginLanClient(
  host: string,
  port: number,
  companyId: string,
  username: string,
  password?: string
): Promise<LanLoginResponse> {
  const res = await fetch(`http://${host}:${port}/api/v1/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      company_id: companyId,
      username,
      password: password || null,
    }),
  });

  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.message || `LAN login failed (${res.status})`);
  }

  const data: LanLoginResponse = await res.json();
  setLanClientConfig({
    host,
    port,
    company_id: data.company_id,
    username: data.username,
    token: data.token,
    role: data.role,
  });
  return data;
}

export async function logoutLanClient(): Promise<void> {
  if (currentLanClientConfig) {
    try {
      await fetch(
        `http://${currentLanClientConfig.host}:${currentLanClientConfig.port}/api/v1/auth/logout`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${currentLanClientConfig.token}`,
          },
        }
      );
    } catch {
      // Ignore network failures on logout
    }
    setLanClientConfig(null);
  }
}

// ==========================================
// Milestone 16: Multi-Currency & Cheque Printing
// ==========================================

export async function seedCompanyCurrencyDefaults(companyId?: string): Promise<CurrencySeedSummary> {
  if (!isTauriEnv()) {
    return {
      base_currency_seeded: true,
      currencies_seeded_count: 5,
      forex_ledger_id: 'mock_led_forex',
      cheque_templates_seeded_count: 4,
    };
  }
  return invoke<CurrencySeedSummary>('seed_company_currency_defaults', { companyId: companyId || null });
}

export async function listCurrencies(companyId?: string): Promise<Currency[]> {
  if (!isTauriEnv()) {
    return [
      { id: 'cur_inr', company_id: 'default', code: 'INR', symbol: '₹', formal_name: 'Indian Rupee', decimal_places: 2, is_base: true, created_at: '2026-04-01' },
      { id: 'cur_usd', company_id: 'default', code: 'USD', symbol: '$', formal_name: 'US Dollar', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
      { id: 'cur_eur', company_id: 'default', code: 'EUR', symbol: '€', formal_name: 'Euro', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
      { id: 'cur_gbp', company_id: 'default', code: 'GBP', symbol: '£', formal_name: 'British Pound', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
      { id: 'cur_aed', company_id: 'default', code: 'AED', symbol: 'د.إ', formal_name: 'UAE Dirham', decimal_places: 2, is_base: false, created_at: '2026-04-01' },
    ];
  }
  return invoke<Currency[]>('list_currencies', { companyId: companyId || null });
}

export async function createCurrency(input: CreateCurrencyInput, companyId?: string): Promise<Currency> {
  if (!isTauriEnv()) {
    return {
      id: `cur_${input.code.toLowerCase()}`,
      company_id: companyId || 'default',
      code: input.code,
      symbol: input.symbol,
      formal_name: input.formal_name,
      decimal_places: input.decimal_places ?? 2,
      is_base: false,
      created_at: new Date().toISOString(),
    };
  }
  return invoke<Currency>('create_currency', { input, companyId: companyId || null });
}

export async function setExchangeRate(input: SetExchangeRateInput, companyId?: string): Promise<ExchangeRate> {
  if (!isTauriEnv()) {
    return {
      id: `exr_${input.currency_code.toLowerCase()}_${input.effective_date}`,
      company_id: companyId || 'default',
      currency_code: input.currency_code,
      effective_date: input.effective_date,
      standard_rate: input.standard_rate,
      selling_rate: input.selling_rate || input.standard_rate,
      buying_rate: input.buying_rate || input.standard_rate,
      created_at: new Date().toISOString(),
    };
  }
  return invoke<ExchangeRate>('set_exchange_rate', { input, companyId: companyId || null });
}

export async function listExchangeRates(currencyCode?: string, companyId?: string): Promise<ExchangeRate[]> {
  if (!isTauriEnv()) return [];
  return invoke<ExchangeRate[]>('list_exchange_rates', {
    currencyCode: currencyCode || null,
    companyId: companyId || null,
  });
}

export async function lookupExchangeRate(
  currencyCode: string,
  date: string,
  companyId?: string
): Promise<EffectiveRateDto> {
  if (!isTauriEnv()) {
    return {
      currency_code: currencyCode,
      effective_date: date,
      standard_rate: '83.50',
      selling_rate: '83.50',
      buying_rate: '83.50',
      is_base: currencyCode === 'INR',
    };
  }
  return invoke<EffectiveRateDto>('lookup_exchange_rate', {
    currencyCode,
    date,
    companyId: companyId || null,
  });
}

export async function convertCurrencyAmount(
  foreignAmount: string,
  fromCurrency: string,
  toCurrency: string,
  date: string,
  rateType?: string,
  companyId?: string
): Promise<{ converted_amount: string; effective_rate: string }> {
  if (!isTauriEnv()) {
    return {
      converted_amount: (parseFloat(foreignAmount) * 83.5).toFixed(2),
      effective_rate: '83.50',
    };
  }
  return invoke<{ converted_amount: string; effective_rate: string }>('convert_currency_amount', {
    foreignAmount,
    fromCurrency,
    toCurrency,
    date,
    rateType: rateType || null,
    companyId: companyId || null,
  });
}

export async function listOpenForeignBills(
  partyLedgerId?: string,
  companyId?: string
): Promise<OpenForeignBill[]> {
  if (!isTauriEnv()) return [];
  return invoke<OpenForeignBill[]>('list_open_foreign_bills', {
    partyLedgerId: partyLedgerId || null,
    companyId: companyId || null,
  });
}

export async function settleForeignBillWithForex(
  input: SettleForeignBillInput,
  companyId?: string
): Promise<RealizedForexResult> {
  if (!isTauriEnv()) {
    throw new Error('Settling foreign bills requires the native OpenTally application');
  }
  return invoke<RealizedForexResult>('settle_foreign_bill_with_forex', {
    input,
    companyId: companyId || null,
  });
}

export async function getUnrealizedForexReport(
  asOfDate: string,
  companyId?: string
): Promise<UnrealizedForexReport> {
  if (!isTauriEnv()) {
    return {
      company_id: companyId || 'default',
      as_of_date: asOfDate,
      bills: [],
      total_book_value_inr: '0.00',
      total_revalued_value_inr: '0.00',
      total_unrealized_gain: '0.00',
      total_unrealized_loss: '0.00',
      net_unrealized_gain_loss: '0.00',
    };
  }
  return invoke<UnrealizedForexReport>('get_unrealized_forex_report', {
    asOfDate,
    companyId: companyId || null,
  });
}

export async function listChequeTemplates(companyId?: string): Promise<ChequeTemplateDto[]> {
  if (!isTauriEnv()) {
    return [
      { id: 'tmpl_hdfc', company_id: 'default', bank_name: 'HDFC Bank', width_mm: 202, height_mm: 92, date_x_mm: 148, date_y_mm: 9, payee_x_mm: 22, payee_y_mm: 25, words_x_mm: 26, words_y_mm: 36, amount_x_mm: 152, amount_y_mm: 42, is_account_payee_default: true },
      { id: 'tmpl_sbi', company_id: 'default', bank_name: 'State Bank of India', width_mm: 202, height_mm: 92, date_x_mm: 147, date_y_mm: 8.5, payee_x_mm: 20, payee_y_mm: 24, words_x_mm: 24, words_y_mm: 35, amount_x_mm: 150, amount_y_mm: 41, is_account_payee_default: true },
      { id: 'tmpl_icici', company_id: 'default', bank_name: 'ICICI Bank', width_mm: 202, height_mm: 92, date_x_mm: 149, date_y_mm: 9, payee_x_mm: 22, payee_y_mm: 25, words_x_mm: 26, words_y_mm: 36, amount_x_mm: 152, amount_y_mm: 42, is_account_payee_default: true },
      { id: 'tmpl_axis', company_id: 'default', bank_name: 'Axis Bank', width_mm: 202, height_mm: 92, date_x_mm: 148, date_y_mm: 9, payee_x_mm: 21, payee_y_mm: 24.5, words_x_mm: 25, words_y_mm: 35.5, amount_x_mm: 151, amount_y_mm: 41.5, is_account_payee_default: true },
    ];
  }
  return invoke<ChequeTemplateDto[]>('list_cheque_templates', { companyId: companyId || null });
}

export async function upsertChequeTemplate(
  input: UpsertChequeTemplateInput,
  companyId?: string
): Promise<ChequeTemplateDto> {
  if (!isTauriEnv()) {
    throw new Error('Cheque templates require the native OpenTally application');
  }
  return invoke<ChequeTemplateDto>('upsert_cheque_template', {
    input,
    companyId: companyId || null,
  });
}

export async function generateChequePdfBytes(
  voucherId: string,
  templateId?: string,
  isAccountPayee?: boolean,
  companyId?: string
): Promise<number[]> {
  if (!isTauriEnv()) return [];
  return invoke<number[]>('generate_cheque_pdf_bytes', {
    voucherId,
    templateId: templateId || null,
    isAccountPayee: isAccountPayee ?? null,
    companyId: companyId || null,
  });
}

export async function exportChequePdf(
  voucherId: string,
  outputPath: string,
  templateId?: string,
  isAccountPayee?: boolean,
  companyId?: string
): Promise<string> {
  if (!isTauriEnv()) return outputPath;
  return invoke<string>('export_cheque_pdf', {
    voucherId,
    outputPath,
    templateId: templateId || null,
    isAccountPayee: isAccountPayee ?? null,
    companyId: companyId || null,
  });
}


// ==========================================
// Milestone 18: BYO-GSP & Custom Report Extensions
// ==========================================

export async function getCompanyGspConfig(): Promise<CompanyGspConfigDto> {
  if (!isTauriEnv()) {
    return {
      company_id: 'default',
      provider_mode: 'Sandbox',
      provider_name: 'OpenTally Sandbox',
      base_url: null,
      client_id: null,
      has_api_secret: false,
      is_enabled: true,
      updated_at: new Date().toISOString(),
    };
  }
  return invoke<CompanyGspConfigDto>('get_company_gsp_config');
}

export async function saveCompanyGspConfig(
  input: SaveCompanyGspConfigInput
): Promise<CompanyGspConfigDto> {
  if (!isTauriEnv()) {
    return {
      company_id: 'default',
      provider_mode: input.provider_mode,
      provider_name: input.provider_name,
      base_url: input.base_url || null,
      client_id: input.client_id || null,
      has_api_secret: Boolean(input.api_secret_ref),
      is_enabled: input.is_enabled,
      updated_at: new Date().toISOString(),
    };
  }
  return invoke<CompanyGspConfigDto>('save_company_gsp_config', { input });
}

export async function pushEinvoiceToGsp(voucherId: string): Promise<GspEinvoiceResponse> {
  if (!isTauriEnv()) {
    return {
      irn: 'a'.repeat(64),
      ack_no: '112233445566778',
      ack_date: new Date().toISOString(),
      signed_qr_code: 'SANDBOX-QR',
      provider_mode: 'Sandbox',
    };
  }
  return invoke<GspEinvoiceResponse>('push_einvoice_to_gsp', { voucherId });
}

export async function pushEwayBillToGsp(voucherId: string): Promise<GspEwayBillResponse> {
  if (!isTauriEnv()) {
    return {
      ewb_no: '123456789012',
      ewb_date: new Date().toISOString(),
      ewb_valid_upto: new Date(Date.now() + 86400000 * 3).toISOString(),
      provider_mode: 'Sandbox',
    };
  }
  return invoke<GspEwayBillResponse>('push_eway_bill_to_gsp', { voucherId });
}

export async function listCustomReports(): Promise<CustomReportDefinitionDto[]> {
  if (!isTauriEnv()) return [];
  return invoke<CustomReportDefinitionDto[]>('list_custom_reports');
}

export async function saveCustomReport(
  input: SaveCustomReportInput
): Promise<CustomReportDefinitionDto> {
  if (!isTauriEnv()) {
    throw new Error('Custom reports require the native OpenTally application');
  }
  return invoke<CustomReportDefinitionDto>('save_custom_report', { input });
}

export async function deleteCustomReport(reportId: string): Promise<void> {
  if (!isTauriEnv()) return;
  return invoke<void>('delete_custom_report', { reportId });
}

export async function runCustomReport(reportId: string): Promise<CustomReportExecutionResult> {
  if (!isTauriEnv()) {
    throw new Error('Running custom reports requires the native OpenTally application');
  }
  return invoke<CustomReportExecutionResult>('run_custom_report', { reportId });
}

export async function exportCustomReportCsv(reportId: string): Promise<string> {
  if (!isTauriEnv()) return '';
  return invoke<string>('export_custom_report_csv', { reportId });
}

export async function exportCustomReportTemplate(reportId: string): Promise<string> {
  if (!isTauriEnv()) return '{}';
  return invoke<string>('export_custom_report_template', { reportId });
}

export async function importCustomReportTemplate(
  templateJson: string
): Promise<CustomReportDefinitionDto> {
  if (!isTauriEnv()) {
    throw new Error('Importing custom report templates requires the native OpenTally application');
  }
  return invoke<CustomReportDefinitionDto>('import_custom_report_template', { templateJson });
}


// Milestone 19: GSTN Strictness & CMP-08
export async function validateGstr1Export(
  fromDate: string,
  toDate: string
): Promise<import('../types').GstnValidationReport> {
  return invoke('validate_gstr1_export', { fromDate, toDate });
}

export async function getCmp08QuarterlySummary(
  financialYear: number,
  quarter: number
): Promise<import('../types').Cmp08QuarterlySummary> {
  return invoke('get_cmp08_quarterly_summary', { financialYear, quarter });
}


// Milestone 20: Compound Units & Tiered Price Levels
export async function createCompoundUnit(
  name: string,
  symbol: string,
  firstUnitId: string,
  secondUnitId: string,
  conversionFactor: string
): Promise<import('../types').CompoundUnitDto> {
  return invoke('create_compound_unit', {
    name,
    symbol,
    firstUnitId,
    secondUnitId,
    conversionFactor,
  });
}

export async function listPriceLevels(): Promise<import('../types').PriceLevelDto[]> {
  return invoke('list_price_levels');
}

export async function createPriceLevel(
  name: string
): Promise<import('../types').PriceLevelDto> {
  return invoke('create_price_level', { name });
}

export async function upsertPriceListEntry(
  priceLevelId: string,
  stockItemId: string,
  effectiveFrom: string,
  minQty: string,
  maxQty: string | null,
  unitRate: string,
  discountPct: string
): Promise<import('../types').PriceListEntryDto> {
  return invoke('upsert_price_list_entry', {
    priceLevelId,
    stockItemId,
    effectiveFrom,
    minQty,
    maxQty,
    unitRate,
    discountPct,
  });
}

export async function listPriceListEntries(
  priceLevelId?: string
): Promise<import('../types').PriceListEntryDto[]> {
  return invoke('list_price_list_entries', { priceLevelId: priceLevelId || null });
}

export async function resolveItemPrice(
  stockItemId: string,
  partyLedgerId: string | null,
  priceLevelId: string | null,
  quantity: string,
  asOfDate: string
): Promise<import('../types').ResolvedItemPriceDto> {
  return invoke('resolve_item_price', {
    stockItemId,
    partyLedgerId,
    priceLevelId,
    quantity,
    asOfDate,
  });
}
