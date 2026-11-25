import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CompanyFeaturesModal } from '../CompanyFeaturesModal';
import * as api from '../../../services/api';
import { CompanyFeaturesDto } from '../../../types';

const mockFeaturesDto: CompanyFeaturesDto = {
  company_id: 'cmp-1',
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

describe('CompanyFeaturesModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(api, 'getCompanyFeatures').mockResolvedValue(mockFeaturesDto);
    vi.spyOn(api, 'saveCompanyFeatures').mockResolvedValue(mockFeaturesDto);
  });

  it('does not render when isOpen is false', () => {
    const { container } = render(
      <CompanyFeaturesModal
        isOpen={false}
        companyId="cmp-1"
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders F11 modal when isOpen is true', async () => {
    render(
      <CompanyFeaturesModal
        isOpen={true}
        companyId="cmp-1"
        companyName="Acme Corp"
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />
    );

    expect(screen.getByText(/F11: Company Features/i)).toBeTruthy();
    expect(screen.getByText(/(Acme Corp)/i)).toBeTruthy();

    await waitFor(() => {
      expect(api.getCompanyFeatures).toHaveBeenCalledWith('cmp-1');
    });
  });

  it('shows onboarding banner when isInitialSetup is true', () => {
    render(
      <CompanyFeaturesModal
        isOpen={true}
        companyId="cmp-1"
        isInitialSetup={true}
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />
    );

    expect(screen.getByTestId('onboarding-guidance-banner')).toBeTruthy();
    expect(screen.getByText(/Company created successfully!/i)).toBeTruthy();
  });

  it('performs live GSTIN state detection and Mod-36 validation', async () => {
    render(
      <CompanyFeaturesModal
        isOpen={true}
        companyId="cmp-1"
        onClose={vi.fn()}
        onSaved={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('gstin-valid-badge')).toBeTruthy();
    });

    const gstinInput = screen.getByLabelText(/GSTIN \/ UIN/i);

    // Invalid GSTIN: wrong length / invalid checksum
    fireEvent.change(gstinInput, { target: { value: '29ABCDE1234F1Z9' } });
    expect(screen.getByTestId('gstin-invalid-badge')).toBeTruthy();

    // Valid Maharashtra GSTIN: 27 + PAN + 1Z + check
    // 27AAPFU0939F1ZV
    fireEvent.change(gstinInput, { target: { value: '29ABCDE1234F1ZW' } });
    expect(screen.getByTestId('gstin-valid-badge')).toBeTruthy();
    expect(screen.getByText(/Valid \(Karnataka\)/i)).toBeTruthy();
  });

  it('saves features on button click and calls onSaved', async () => {
    const onSaved = vi.fn();
    const onClose = vi.fn();

    render(
      <CompanyFeaturesModal
        isOpen={true}
        companyId="cmp-1"
        onClose={onClose}
        onSaved={onSaved}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId('save-features-btn')).toBeTruthy();
    });

    const saveBtn = screen.getByTestId('save-features-btn');
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(api.saveCompanyFeatures).toHaveBeenCalled();
      expect(onSaved).toHaveBeenCalledWith(mockFeaturesDto);
      expect(onClose).toHaveBeenCalled();
    });
  });

  it('supports Ctrl+A to save and Esc to close', async () => {
    const onSaved = vi.fn();
    const onClose = vi.fn();

    render(
      <CompanyFeaturesModal
        isOpen={true}
        companyId="cmp-1"
        onClose={onClose}
        onSaved={onSaved}
      />
    );

    // Press Escape
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();

    // Press Ctrl+A
    fireEvent.keyDown(window, { key: 'a', ctrlKey: true });
    await waitFor(() => {
      expect(api.saveCompanyFeatures).toHaveBeenCalled();
    });
  });
});
