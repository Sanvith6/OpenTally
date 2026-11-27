import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { TallyXmlModal } from '../TallyXmlModal';
import * as api from '../../../services/api';
import { Company, TallyXmlImportPreview, TallyXmlImportResult } from '../../../types';

const company: Company = {
  id: 'cmp-1',
  name: 'Acme Test Corp',
  mailing_name: 'Acme Test Corp',
  country: 'India',
  state: 'Maharashtra',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '',
  updated_at: '',
};

class MockFileReader {
  onload: ((e: any) => void) | null = null;
  onerror: (() => void) | null = null;
  readAsText(_blob: Blob) {
    setTimeout(() => {
      if (this.onload) {
        this.onload({ target: { result: '<ENVELOPE></ENVELOPE>' } });
      }
    }, 0);
  }
}

describe('TallyXmlModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal('FileReader', MockFileReader);
  });

  it('does not render when isOpen is false', () => {
    const { container } = render(
      <TallyXmlModal isOpen={false} onClose={vi.fn()} activeCompany={company} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders modal with tabs when isOpen is true', () => {
    render(<TallyXmlModal isOpen={true} onClose={vi.fn()} activeCompany={company} />);

    expect(screen.getByText('Tally XML Interoperability')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Import Tally XML/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Export Tally XML/i })).toBeTruthy();
    expect(screen.getByText(/Choose XML File/i)).toBeTruthy();
  });

  it('switches between Import and Export tabs', () => {
    const { container } = render(
      <TallyXmlModal isOpen={true} onClose={vi.fn()} activeCompany={company} />
    );

    const exportTab = screen.getByRole('button', { name: /Export Tally XML/i });
    fireEvent.click(exportTab);

    expect(screen.getByText(/Export Company Data \(Tally XML\)/i)).toBeTruthy();
    expect(container.querySelector('.ot-primary-btn')).toBeTruthy();

    const importTab = screen.getByRole('button', { name: /Import Tally XML/i });
    fireEvent.click(importTab);

    expect(screen.getByText(/Choose XML File/i)).toBeTruthy();
  });

  it('handles XML file selection, triggers dry-run preview, and shows import summary', async () => {
    const mockPreview: TallyXmlImportPreview = {
      groups_to_create: 2,
      groups_skipped: 1,
      ledgers_to_create: 5,
      ledgers_skipped: 0,
      stock_items_to_create: 3,
      stock_items_skipped: 0,
      vouchers_to_create: 10,
      vouchers_skipped: 2,
      can_import: true,
      errors: [
        {
          item_type: 'GROUP',
          identifier: 'Sundry Debtors',
          message: 'Group already exists; skipping.',
          severity: 'WARNING',
        },
      ],
    };

    vi.spyOn(api, 'previewTallyXmlImport').mockResolvedValue(mockPreview);

    const { container } = render(
      <TallyXmlModal isOpen={true} onClose={vi.fn()} activeCompany={company} />
    );

    const file = new File(['<ENVELOPE></ENVELOPE>'], 'company_data.xml', { type: 'text/xml' });
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;

    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(() => {
      expect(api.previewTallyXmlImport).toHaveBeenCalledWith('<ENVELOPE></ENVELOPE>');
    });

    expect(await screen.findByText('Dry-Run Import Analysis')).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
    expect(screen.getByText('1 existing')).toBeTruthy();
    expect(screen.getByText('5')).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
    expect(screen.getByText('10')).toBeTruthy();
    expect(screen.getByRole('button', { name: /Confirm Atomic Import/i })).toBeTruthy();
  });

  it('executes atomic import on confirmation and triggers callback', async () => {
    const mockPreview: TallyXmlImportPreview = {
      groups_to_create: 1,
      groups_skipped: 0,
      ledgers_to_create: 1,
      ledgers_skipped: 0,
      stock_items_to_create: 0,
      stock_items_skipped: 0,
      vouchers_to_create: 1,
      vouchers_skipped: 0,
      can_import: true,
      errors: [],
    };

    const mockResult: TallyXmlImportResult = {
      success: true,
      groups_created: 1,
      ledgers_created: 1,
      stock_items_created: 0,
      vouchers_created: 1,
      message: 'Successfully imported 2 masters and 1 vouchers.',
      reconciliation: {
        source_voucher_count: 1,
        imported_voucher_count: 1,
        source_total_debit: '1000.00',
        imported_total_debit: '1000.00',
        source_total_credit: '1000.00',
        imported_total_credit: '1000.00',
        imported_trial_balance_difference: '0.00',
        bill_allocations_imported: 2,
        inventory_lines_imported: 1,
        bank_allocations_imported: 1,
        cost_centre_allocations_imported: 1,
      },
    };

    vi.spyOn(api, 'previewTallyXmlImport').mockResolvedValue(mockPreview);
    vi.spyOn(api, 'executeTallyXmlImport').mockResolvedValue(mockResult);

    const onImportSuccess = vi.fn();
    const { container } = render(
      <TallyXmlModal
        isOpen={true}
        onClose={vi.fn()}
        activeCompany={company}
        onImportSuccess={onImportSuccess}
      />
    );

    const file = new File(['<ENVELOPE></ENVELOPE>'], 'tally.xml', { type: 'text/xml' });
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [file] } });

    const confirmBtn = await screen.findByRole('button', { name: /Confirm Atomic Import/i });
    expect(confirmBtn).toBeTruthy();

    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(api.executeTallyXmlImport).toHaveBeenCalledWith('<ENVELOPE></ENVELOPE>');
      expect(onImportSuccess).toHaveBeenCalled();
    });

    expect(screen.getByText('Successfully imported 2 masters and 1 vouchers.')).toBeTruthy();
    expect(screen.getByText('Migration Reconciliation Report')).toBeTruthy();
    expect(screen.getByText(/Bills: 2/)).toBeTruthy();
  });

  it('triggers onClose when Close button or Esc key is pressed', () => {
    const onClose = vi.fn();
    render(<TallyXmlModal isOpen={true} onClose={onClose} activeCompany={company} />);

    fireEvent.click(screen.getByRole('button', { name: 'Close modal' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
