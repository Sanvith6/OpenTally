import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SelectCompanyView } from '../SelectCompanyView';
import * as api from '../../../services/api';
import { Company } from '../../../types';

const mockCompanies: Company[] = [
  {
    id: 'cmp-1',
    name: 'Bharat Precision Components Pvt Ltd',
    mailing_name: 'Bharat Precision Components Pvt Ltd',
    country: 'India',
    state: 'Karnataka',
    financial_year_begins: '2025-04-01',
    books_beginning_from: '2025-04-01',
    currency_symbol: '₹',
    currency_formal_name: 'INR',
    currency_decimal_places: 2,
    status: 'active',
    created_at: '',
    updated_at: '',
  },
  {
    id: 'cmp-2',
    name: 'Maharashtra Traders Ltd',
    mailing_name: 'Maharashtra Traders Ltd',
    country: 'India',
    state: 'Maharashtra',
    financial_year_begins: '2025-04-01',
    books_beginning_from: '2025-04-01',
    currency_symbol: '₹',
    currency_formal_name: 'INR',
    currency_decimal_places: 2,
    status: 'active',
    created_at: '',
    updated_at: '',
  },
];

describe('SelectCompanyView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'listCompanies').mockResolvedValue(mockCompanies);
    vi.spyOn(api, 'selectCompany').mockImplementation(async (id) => {
      const c = mockCompanies.find((item) => item.id === id);
      if (!c) throw new Error('Not found');
      return c;
    });
    vi.spyOn(api, 'createSampleCompany').mockResolvedValue(mockCompanies[0]);
  });

  it('renders the startup hub and company list', async () => {
    render(
      <SelectCompanyView
        activeCompanyId="cmp-1"
        onCompanySelected={vi.fn()}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
      />
    );

    expect(screen.getByText('Select Company')).toBeTruthy();
    expect(screen.getByTestId('btn-create-company')).toBeTruthy();
    expect(screen.getByTestId('btn-load-sample')).toBeTruthy();
    expect(screen.getByTestId('btn-import-xml')).toBeTruthy();
    expect(screen.getByTestId('btn-restore-backup')).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByText('Bharat Precision Components Pvt Ltd')).toBeTruthy();
      expect(screen.getByText('Maharashtra Traders Ltd')).toBeTruthy();
    });
  });

  it('triggers action callbacks when action buttons are clicked', () => {
    const onCreate = vi.fn();
    const onXml = vi.fn();
    const onRestore = vi.fn();

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={vi.fn()}
        onCreateCompany={onCreate}
        onImportXml={onXml}
        onRestoreBackup={onRestore}
      />
    );

    fireEvent.click(screen.getByTestId('btn-create-company'));
    expect(onCreate).toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('btn-import-xml'));
    expect(onXml).toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('btn-restore-backup'));
    expect(onRestore).toHaveBeenCalled();
  });

  it('loads sample company when S is clicked or pressed', async () => {
    const onSelect = vi.fn();

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={onSelect}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
      />
    );

    fireEvent.click(screen.getByTestId('btn-load-sample'));

    await waitFor(() => {
      expect(api.createSampleCompany).toHaveBeenCalled();
      expect(onSelect).toHaveBeenCalledWith(mockCompanies[0]);
    });
  });

  it('navigates rows with ArrowDown and selects with Enter', async () => {
    const onSelect = vi.fn();

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={onSelect}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Maharashtra Traders Ltd')).toBeTruthy();
    });

    // ArrowDown to move to second row
    fireEvent.keyDown(window, { key: 'ArrowDown' });

    // Enter to select
    fireEvent.keyDown(window, { key: 'Enter' });

    await waitFor(() => {
      expect(api.selectCompany).toHaveBeenCalledWith('cmp-2');
      expect(onSelect).toHaveBeenCalledWith(mockCompanies[1]);
    });
  });

  it('filters companies when typing in the search box', async () => {
    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={vi.fn()}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByText('Bharat Precision Components Pvt Ltd')).toBeTruthy();
    });

    const filterInput = screen.getByLabelText(/Search \/ Filter Company:/i);
    fireEvent.change(filterInput, { target: { value: 'Maharashtra' } });

    expect(screen.queryByText('Bharat Precision Components Pvt Ltd')).toBeNull();
    expect(screen.getByText('Maharashtra Traders Ltd')).toBeTruthy();
  });

  it('does not trigger C, S, I, R hotkeys while typing in the filter input', async () => {
    const onCreate = vi.fn();
    const onXml = vi.fn();
    const onRestore = vi.fn();

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={vi.fn()}
        onCreateCompany={onCreate}
        onImportXml={onXml}
        onRestoreBackup={onRestore}
      />
    );

    const filterInput = screen.getByLabelText(/Search \/ Filter Company:/i);
    filterInput.focus();

    // Type letters into input - should not trigger action callbacks
    fireEvent.keyDown(filterInput, { key: 'c' });
    fireEvent.keyDown(filterInput, { key: 's' });
    fireEvent.keyDown(filterInput, { key: 'i' });
    fireEvent.keyDown(filterInput, { key: 'r' });

    expect(onCreate).not.toHaveBeenCalled();
    expect(onXml).not.toHaveBeenCalled();
    expect(onRestore).not.toHaveBeenCalled();
  });

  it('stops propagation and calls onClose on Escape key', async () => {
    const onClose = vi.fn();

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={vi.fn()}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
        onClose={onClose}
      />
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
