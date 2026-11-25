// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ManufacturingBomView } from '../ManufacturingBomView';
import * as api from '../../../services/api';
import {
  BomDetail,
  Company,
  Godown,
  Ledger,
  ManufacturingJournalResult,
  StockItem,
} from '../../../types';

const mockCompany: Company = {
  id: 'cmp_mfg_1',
  name: 'Apex Industrial Gears Pvt Ltd',
  mailing_name: 'Apex Gears',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockStockItems: StockItem[] = [
  {
    id: 'it_gear_assy',
    company_id: 'cmp_mfg_1',
    name: 'Precision Gearbox Assembly',
    stock_group_id: 'grp_1',
    stock_category_id: null,
    base_unit_id: 'u_nos',
    alternate_unit_id: null,
    sku: null,
    description: null,
    opening_quantity: '0',
    opening_rate: '0',
    opening_value: '0',
    reorder_level: '0',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
  {
    id: 'it_pinion',
    company_id: 'cmp_mfg_1',
    name: 'Steel Pinion 40T',
    stock_group_id: 'grp_1',
    stock_category_id: null,
    base_unit_id: 'u_nos',
    alternate_unit_id: null,
    sku: null,
    description: null,
    opening_quantity: '100',
    opening_rate: '250',
    opening_value: '25000',
    reorder_level: '10',
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockGodowns: Godown[] = [
  {
    id: 'g_main',
    company_id: 'cmp_mfg_1',
    parent_id: null,
    name: 'Main Assembly Godown',
    description: null,
    address: null,
    is_external: false,
    is_active: true,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockLedgers: Ledger[] = [
  {
    id: 'led_power',
    company_id: 'cmp_mfg_1',
    group_id: 'grp_exp',
    name: 'Factory Power & Fuel',
    ledger_classification: 'standard',
    is_reserved: false,
    created_at: '2026-04-01T00:00:00Z',
    updated_at: '2026-04-01T00:00:00Z',
  },
];

const mockBoms: BomDetail[] = [
  {
    id: 'bom_gear_std',
    company_id: 'cmp_mfg_1',
    finished_stock_item_id: 'it_gear_assy',
    finished_stock_item_name: 'Precision Gearbox Assembly',
    bom_name: 'Standard Gearbox Recipe',
    base_output_qty: '1',
    default_godown_id: 'g_main',
    default_godown_name: 'Main Assembly Godown',
    is_active: true,
    created_at: '2026-04-01T12:00:00Z',
    updated_at: '2026-04-01T12:00:00Z',
    components: [
      {
        id: 'comp_1',
        raw_stock_item_id: 'it_pinion',
        raw_stock_item_name: 'Steel Pinion 40T',
        source_godown_id: 'g_main',
        source_godown_name: 'Main Assembly Godown',
        required_qty: '2',
      },
    ],
  },
];

const mockMftResult: ManufacturingJournalResult = {
  stock_voucher_id: 'sv_prod_1',
  finished_goods_cost_layer_id: 'fg_layer_1',
  produced_qty: '10',
  landed_unit_cost: '600.0000',
  total_raw_material_cost: '5000.00',
  total_overhead_cost: '1000.00',
  total_manufacturing_cost: '6000.00',
  consumed_components: [
    {
      raw_stock_item_id: 'it_pinion',
      raw_stock_item_name: 'Steel Pinion 40T',
      godown_id: 'g_main',
      consumed_qty: '20',
      consumed_cost: '5000.00',
    },
  ],
};

describe('ManufacturingBomView Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listStockItems').mockResolvedValue(mockStockItems);
    vi.spyOn(api, 'listGodowns').mockResolvedValue(mockGodowns);
    vi.spyOn(api, 'listLedgers').mockResolvedValue(mockLedgers);
  });

  it('renders BOM recipes list and allows creating a new BOM recipe', async () => {
    vi.spyOn(api, 'listBoms').mockResolvedValue(mockBoms);
    const createBomSpy = vi.spyOn(api, 'createBom').mockResolvedValue(mockBoms[0]);

    render(<ManufacturingBomView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Standard Gearbox Recipe')).toBeTruthy();
      expect(screen.getByText('Precision Gearbox Assembly')).toBeTruthy();
    });

    const addRecipeBtn = screen.getByRole('button', { name: /\+ Define New BOM Recipe/i });
    fireEvent.click(addRecipeBtn);

    const nameInput = screen.getByPlaceholderText(/e\.g\. Standard Recipe \/ Batch of 100/i);
    fireEvent.change(nameInput, { target: { value: 'High Torque Gearbox' } });

    // Select finished good (selects[0]) and raw component (selects[2])
    const selects = screen.getAllByRole('combobox');
    fireEvent.change(selects[0], { target: { value: 'it_gear_assy' } });
    fireEvent.change(selects[2], { target: { value: 'it_pinion' } });

    const saveBtn = screen.getByRole('button', { name: /Save Recipe/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(createBomSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          company_id: 'cmp_mfg_1',
          finished_stock_item_id: 'it_gear_assy',
          bom_name: 'High Torque Gearbox',
          base_output_qty: '1',
        })
      );
    });
  });

  it('switches to Manufacturing Journal tab and executes production', async () => {
    vi.spyOn(api, 'listBoms').mockResolvedValue(mockBoms);
    const execMftSpy = vi.spyOn(api, 'executeManufacturingJournal').mockResolvedValue(mockMftResult);

    render(<ManufacturingBomView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('Standard Gearbox Recipe')).toBeTruthy();
    });

    // Switch tab
    const produceTab = screen.getByRole('button', { name: /Manufacturing Stock Journal \(Production\)/i });
    fireEvent.click(produceTab);

    // Select BOM & Destination Godown from comboboxes
    const selects = screen.getAllByRole('combobox');
    fireEvent.change(selects[0], { target: { value: 'bom_gear_std' } });
    fireEvent.change(selects[1], { target: { value: 'g_main' } });

    // Enter produced quantity
    const qtyInput = screen.getByPlaceholderText(/e\.g\. 10/i);
    fireEvent.change(qtyInput, { target: { value: '10' } });

    const execBtn = screen.getByRole('button', { name: /Post Manufacturing Journal/i });
    fireEvent.click(execBtn);

    await waitFor(() => {
      expect(execMftSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          company_id: 'cmp_mfg_1',
          bom_id: 'bom_gear_std',
          produced_qty: '10',
          destination_godown_id: 'g_main',
        })
      );
      expect(screen.getByText(/Production completed successfully/i)).toBeTruthy();
      expect(screen.getAllByText(/600\.0000/).length).toBeGreaterThan(0); // landed unit cost
    });
  });
});
