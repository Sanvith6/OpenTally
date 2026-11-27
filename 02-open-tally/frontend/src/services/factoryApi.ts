import { invoke } from '@tauri-apps/api/core';

/** All quantities and money cross IPC as decimal strings, never JS numbers. */
export interface JobWorkLineInput { stock_item_id: string; qty: string; unit_cost: string; scrap_qty?: string | null }
export interface DispatchJobWorkRequest {
  challan_number: string; challan_date: string; job_worker_ledger_id: string;
  source_godown_id: string; external_godown_id: string; goods_category: 'Input' | 'CapitalGood';
  narration: string | null; lines: JobWorkLineInput[];
}
export interface ReceiveJobWorkRequest {
  challan_number: string; challan_date: string; linked_out_challan_id: string;
  destination_godown_id: string; finished_stock_item_id: string; finished_qty: string;
  returned_raw_qty: string; scrap_qty: string; job_work_charges: string;
  scrap_recovery_value: string; narration: string | null;
}
export interface JobWorkChallan {
  id: string; challan_number: string; challan_type: string; challan_date: string;
  statutory_due_date: string; status: string; goods_category: string;
}
export interface Itc04OutRow {
  challan_id: string; challan_number: string; challan_date: string; job_worker_name: string;
  job_worker_gstin: string | null; goods_category: string; stock_item_name: string;
  hsn_code: string | null; dispatched_qty: string; taxable_value: string;
  statutory_due_date: string; is_overdue: boolean;
}
export interface Itc04InRow {
  in_challan_number: string; in_challan_date: string; original_out_challan_number: string;
  original_out_challan_date: string; received_qty: string; scrap_qty: string; pending_qty: string;
}
export interface Itc04Report { company_id: string; from_date: string; to_date: string; table_4: Itc04OutRow[]; table_5a: Itc04InRow[] }
export interface CustomFieldDefinition {
  id: string; company_id: string; entity_scope: string; field_key: string; label: string;
  field_type: string; is_required: number; show_on_pdf: number; created_at: string;
}
export interface CreateCustomFieldInput {
  entity_scope: string; field_key: string; label: string; field_type: string;
  is_required: boolean; show_on_pdf: boolean;
}
export interface WebhookSubscription { id: string; name: string; event_type: string; target_url: string; is_active: boolean; created_at: string }
export interface CreateWebhookInput { name: string; event_type: string; target_url: string; hmac_secret: string }
export interface WeighbridgeReading { gross_weight: string; tare_weight: string; net_weight: string; unit: string }

export async function dispatchJobWorkMaterialOut(companyId: string, request: DispatchJobWorkRequest): Promise<JobWorkChallan> {
  return invoke('dispatch_job_work_material_out', { companyId, request });
}
export async function receiveJobWorkMaterialIn(companyId: string, request: ReceiveJobWorkRequest): Promise<JobWorkChallan> {
  return invoke('receive_job_work_material_in', { companyId, request });
}
export async function loadItc04Report(companyId: string, fromDate: string, toDate: string): Promise<Itc04Report> {
  return invoke('get_itc04_report', { companyId, fromDate, toDate });
}
export async function downloadItc04Json(companyId: string, fromDate: string, toDate: string): Promise<string> {
  return invoke('export_itc04_json', { companyId, fromDate, toDate });
}
export async function listCustomFieldDefinitions(companyId: string): Promise<CustomFieldDefinition[]> {
  return invoke('list_custom_field_definitions', { companyId, entityScope: null });
}
export async function createCustomFieldDefinition(companyId: string, input: CreateCustomFieldInput): Promise<CustomFieldDefinition> {
  return invoke('create_custom_field_definition', { companyId, input });
}
export async function listWebhookSubscriptions(companyId: string): Promise<WebhookSubscription[]> {
  return invoke('list_webhook_subscriptions', { companyId });
}
export async function createWebhookSubscription(companyId: string, input: CreateWebhookInput): Promise<WebhookSubscription> {
  return invoke('create_webhook_subscription', { companyId, input });
}
export async function parseFactoryWeighbridge(rawFrame: string, tareWeight: string): Promise<WeighbridgeReading> {
  return invoke('parse_weighbridge_reading', { rawFrame, tareWeight });
}
export async function lookupStockBarcode(companyId: string, barcode: string): Promise<[string, string] | null> {
  return invoke('lookup_stock_item_by_barcode', { companyId, barcode });
}
export async function expandMultilevelBom(companyId: string, bomId: string, targetQty: string): Promise<[string, string][]> {
  return invoke('expand_multilevel_bom', { companyId, bomId, targetQty });
}
export async function computeNetUnitCost(rawCost: string, overheadCost: string, byproductRecoveryValue: string, producedQty: string): Promise<string> {
  return invoke('compute_net_unit_cost_with_byproducts', { rawCost, overheadCost, byproductRecoveryValue, producedQty });
}

export function saveFactoryJson(filename: string, contents: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type: 'application/json;charset=utf-8' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

export function factoryError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
