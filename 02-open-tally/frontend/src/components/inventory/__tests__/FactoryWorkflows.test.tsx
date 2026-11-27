import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { JobWorkView } from '../JobWorkView';
import { FactoryExtensibilityView } from '../../extensions/FactoryExtensibilityView';
import { ManufacturingAdvancedPanel } from '../ManufacturingAdvancedPanel';
import { invoke } from '@tauri-apps/api/core';
import * as api from '../../../services/api';
import { Company } from '../../../types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
const company = { id: 'company-factory', name: 'Factory', financial_year_begins: '2026-04-01' } as Company;
beforeEach(() => {
  vi.spyOn(api, 'listStockItems').mockResolvedValue([{ id: 'stock-1', name: 'Steel', is_active: true }] as Awaited<ReturnType<typeof api.listStockItems>>);
  vi.spyOn(api, 'listLedgers').mockResolvedValue([{ id: 'worker-1', name: 'Job Worker' }] as Awaited<ReturnType<typeof api.listLedgers>>);
  vi.spyOn(api, 'listGodowns').mockResolvedValue([
    { id: 'internal', name: 'Factory Floor', is_external: false, is_active: true },
    { id: 'external', name: 'External Worker', is_external: true, is_active: true },
  ] as Awaited<ReturnType<typeof api.listGodowns>>);
  vi.mocked(invoke).mockImplementation(async (name) => {
    if (name === 'get_itc04_report') return { table_4: [], table_5a: [] };
    if (name === 'list_custom_field_definitions' || name === 'list_webhook_subscriptions') return [];
    if (name === 'parse_weighbridge_reading') return { gross_weight: '1234.567', tare_weight: '4.567', net_weight: '1230.000', unit: 'KG' };
    if (name === 'compute_net_unit_cost_with_byproducts') return '49.999999';
    return { id: 'saved', challan_number: 'JW-001', statutory_due_date: '2027-11-27' };
  });
});

describe('Factory workflows', () => {
  it('dispatches exact decimal strings and permits only external job-worker godowns', async () => {
    render(<JobWorkView activeCompany={company} onBack={vi.fn()} />);
    await screen.findByRole('option', { name: 'External Worker' });
    const external = screen.getByLabelText('External job-worker godown');
    expect(external.querySelectorAll('option')).toHaveLength(2);
    expect(screen.queryByRole('button', { name: 'Add Dispatch Line' })).toBeNull();
    fireEvent.change(screen.getByLabelText('Out challan number'), { target: { value: 'JW-001' } });
    fireEvent.change(screen.getByLabelText('Job-worker party'), { target: { value: 'worker-1' } });
    fireEvent.change(screen.getByLabelText('Source godown'), { target: { value: 'internal' } });
    fireEvent.change(external, { target: { value: 'external' } });
    fireEvent.change(screen.getByLabelText('Dispatch item 1'), { target: { value: 'stock-1' } });
    fireEvent.change(screen.getByLabelText('Dispatch quantity 1'), { target: { value: '0.000001' } });
    fireEvent.change(screen.getByLabelText('Dispatch unit cost 1'), { target: { value: '9999999999999.99' } });
    fireEvent.click(screen.getByRole('button', { name: 'Dispatch Material Out' }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('dispatch_job_work_material_out', expect.objectContaining({
      companyId: company.id, request: expect.objectContaining({ external_godown_id: 'external', lines: [expect.objectContaining({ qty: '0.000001', unit_cost: '9999999999999.99' })] }),
    })));
  });

  it('shows backend ITC-04 overdue evidence and JSON export', async () => {
    render(<JobWorkView activeCompany={company} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Form GST ITC-04' }));
    expect(await screen.findByRole('button', { name: 'Export ITC-04 JSON' })).toBeTruthy();
    expect(screen.getByText(/CGST Section 143/)).toBeTruthy();
    expect(screen.getByText('Table 4 · Material Out')).toBeTruthy();
    expect(screen.getByText('Table 5A · Material Returned')).toBeTruthy();
  });

  it('creates PDF-visible custom field definitions', async () => {
    render(<FactoryExtensibilityView activeCompany={company} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Custom Fields' }));
    expect(Array.from(screen.getByLabelText('Entity scope').querySelectorAll('option')).map((option) => option.value)).toEqual(['Voucher', 'Ledger', 'StockItem']);
    fireEvent.change(screen.getByLabelText('Field key'), { target: { value: 'vehicle_number' } });
    fireEvent.change(screen.getByLabelText('Field label'), { target: { value: 'Vehicle Number' } });
    fireEvent.click(screen.getByLabelText('Show on PDF'));
    fireEvent.click(screen.getByRole('button', { name: 'Create Custom Field' }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('create_custom_field_definition', expect.objectContaining({ input: expect.objectContaining({ show_on_pdf: true, field_key: 'vehicle_number' }) })));
  });

  it('clears webhook secrets after saving and never renders them in subscriptions', async () => {
    render(<FactoryExtensibilityView activeCompany={company} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Signed Event Webhooks' }));
    fireEvent.change(screen.getByLabelText('Webhook name'), { target: { value: 'Factory ERP' } });
    fireEvent.change(screen.getByLabelText('Webhook URL'), { target: { value: 'https://example.test/events' } });
    fireEvent.change(screen.getByLabelText('Signing secret'), { target: { value: 'never-display-me' } });
    fireEvent.click(screen.getByRole('button', { name: 'Create Webhook' }));
    await waitFor(() => expect((screen.getByLabelText('Signing secret') as HTMLInputElement).value).toBe(''));
    expect(invoke).toHaveBeenCalledWith('create_webhook_subscription', expect.objectContaining({ input: expect.objectContaining({ event_type: 'voucher.posted' }) }));
    expect(Array.from(screen.getByLabelText('Event').querySelectorAll('option')).map((option) => option.value)).toEqual(['voucher.posted', 'voucher.cancelled', 'stock.moved', 'jobwork.dispatched', 'jobwork.received']);
    expect(screen.queryByText('never-display-me')).toBeNull();
  });

  it('calls the decimal-preserving hardware and by-product backend calculators', async () => {
    const { unmount } = render(<FactoryExtensibilityView activeCompany={company} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Weighbridge / Barcode' }));
    fireEvent.click(screen.getByRole('button', { name: 'Parse Test Frame' }));
    expect(await screen.findByText('1230.000 KG')).toBeTruthy();
    unmount();
    render(<ManufacturingAdvancedPanel companyId={company.id} bomId="bom-1" />);
    fireEvent.change(screen.getByLabelText('Raw cost'), { target: { value: '1000000000000.01' } });
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Net Unit Cost' }));
    expect(await screen.findByText('49.999999')).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith('compute_net_unit_cost_with_byproducts', expect.objectContaining({ rawCost: '1000000000000.01' }));
  });
});
