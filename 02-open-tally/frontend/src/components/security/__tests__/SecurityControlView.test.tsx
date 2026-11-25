// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SecurityControlView } from '../SecurityControlView';
import * as api from '../../../services/api';
import { Company, CompanyUser, SecurityStatusDto } from '../../../types';

const mockCompany: Company = {
  id: 'cmp_test_1',
  name: 'Acme Manufacturing Pvt Ltd',
  mailing_name: 'Acme Mfg',
  country: 'India',
  state: 'Maharashtra',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'INR',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockDisabledStatus: SecurityStatusDto = {
  security_enabled: false,
  has_users: false,
  active_user: undefined,
};

const mockEnabledStatus: SecurityStatusDto = {
  security_enabled: true,
  has_users: true,
  active_user: {
    user_id: 'u_admin',
    username: 'admin',
    role: 'Admin',
    logged_in_at: '2026-04-01T10:00:00Z',
  },
};

const mockUsers: CompanyUser[] = [
  {
    id: 'u_admin',
    company_id: 'cmp_test_1',
    username: 'admin',
    role: 'Admin',
    is_active: 1,
    created_at: '2026-04-01T10:00:00Z',
    last_login_at: '2026-04-01T10:00:00Z',
  },
  {
    id: 'u_acct',
    company_id: 'cmp_test_1',
    username: 'accountant_ravi',
    role: 'Accountant',
    is_active: 1,
    created_at: '2026-04-02T10:00:00Z',
  },
];

describe('SecurityControlView Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders security disabled state and allows enabling security', async () => {
    vi.spyOn(api, 'getCompanySecurityStatus').mockResolvedValue(mockDisabledStatus);
    const enableSpy = vi.spyOn(api, 'enableCompanySecurity').mockResolvedValue(mockEnabledStatus.active_user!);
    vi.spyOn(api, 'listCompanyUsers').mockResolvedValue([]);

    render(<SecurityControlView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('DISABLED')).toBeTruthy();
      expect(screen.getByText(/Enable Company Security/i)).toBeTruthy();
    });

    const passInput = screen.getByPlaceholderText(/Min 4 characters/i);
    fireEvent.change(passInput, { target: { value: 'Secret123!' } });

    const enableBtn = screen.getByRole('button', { name: /Enable Security Control/i });
    fireEvent.click(enableBtn);

    await waitFor(() => {
      expect(enableSpy).toHaveBeenCalledWith({
        company_id: 'cmp_test_1',
        admin_username: 'admin',
        admin_password: 'Secret123!',
      });
    });
  });

  it('renders user list when security is enabled', async () => {
    vi.spyOn(api, 'getCompanySecurityStatus').mockResolvedValue(mockEnabledStatus);
    vi.spyOn(api, 'listCompanyUsers').mockResolvedValue(mockUsers);

    render(<SecurityControlView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('accountant_ravi')).toBeTruthy();
      expect(screen.getAllByText('Admin').length).toBeGreaterThan(0);
      expect(screen.getByText(/Company Users \(RBAC\)/i)).toBeTruthy();
    });
  });

  it('allows adding a new user with role', async () => {
    vi.spyOn(api, 'getCompanySecurityStatus').mockResolvedValue(mockEnabledStatus);
    vi.spyOn(api, 'listCompanyUsers').mockResolvedValue(mockUsers);
    const createSpy = vi.spyOn(api, 'createCompanyUser').mockResolvedValue({
      id: 'u_data',
      company_id: 'cmp_test_1',
      username: 'data_entry_user',
      role: 'DataEntry',
      is_active: 1,
      created_at: '2026-04-03T10:00:00Z',
    });

    render(<SecurityControlView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('accountant_ravi')).toBeTruthy();
    });

    const addBtn = screen.getByRole('button', { name: /\+ Add User/i });
    fireEvent.click(addBtn);

    const userInput = screen.getByPlaceholderText(/e\.g\. accountant1/i);
    fireEvent.change(userInput, { target: { value: 'data_entry_user' } });

    const passInput = screen.getByPlaceholderText(/Min 4 chars/i);
    fireEvent.change(passInput, { target: { value: 'UserPass123!' } });

    const saveBtn = screen.getByRole('button', { name: /Save User/i });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(createSpy).toHaveBeenCalledWith({
        company_id: 'cmp_test_1',
        username: 'data_entry_user',
        password: 'UserPass123!',
        role: 'DataEntry',
      });
    });
  });

  it('allows updating a user role', async () => {
    vi.spyOn(api, 'getCompanySecurityStatus').mockResolvedValue(mockEnabledStatus);
    vi.spyOn(api, 'listCompanyUsers').mockResolvedValue(mockUsers);
    const updateRoleSpy = vi.spyOn(api, 'updateCompanyUserRole').mockResolvedValue({
      ...mockUsers[1],
      role: 'Admin',
    });

    render(<SecurityControlView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('accountant_ravi')).toBeTruthy();
    });

    const selects = screen.getAllByRole('combobox');
    // Change role of second user
    fireEvent.change(selects[1], { target: { value: 'Admin' } });

    await waitFor(() => {
      expect(updateRoleSpy).toHaveBeenCalledWith({
        company_id: 'cmp_test_1',
        user_id: 'u_acct',
        role: 'Admin',
      });
    });
  });

  it('allows deactivating a user', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.spyOn(api, 'getCompanySecurityStatus').mockResolvedValue(mockEnabledStatus);
    vi.spyOn(api, 'listCompanyUsers').mockResolvedValue(mockUsers);
    const deactSpy = vi.spyOn(api, 'deactivateCompanyUser').mockResolvedValue();

    render(<SecurityControlView activeCompany={mockCompany} onBack={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('accountant_ravi')).toBeTruthy();
    });

    const deactBtns = screen.getAllByRole('button', { name: /Deactivate/i });
    fireEvent.click(deactBtns[1]); // second user deactivation

    await waitFor(() => {
      expect(deactSpy).toHaveBeenCalledWith({
        company_id: 'cmp_test_1',
        user_id: 'u_acct',
      });
    });
  });
});
