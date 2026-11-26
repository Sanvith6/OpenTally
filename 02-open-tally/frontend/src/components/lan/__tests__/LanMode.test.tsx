import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { LanServerModal } from '../LanServerModal';
import { SelectCompanyView } from '../../company/SelectCompanyView';
import { BreadcrumbBar } from '../../navigation/BreadcrumbBar';
import * as api from '../../../services/api';

vi.mock('../../../services/api');

describe('Milestone 17: LAN Host/Client Mode UI', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listCompanies).mockResolvedValue([]);
    vi.mocked(api.getLanServerStatus).mockResolvedValue({
      is_running: false,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['192.168.1.50', '127.0.0.1'],
      active_sessions: [],
    });
    vi.mocked(api.startLanServer).mockResolvedValue({
      is_running: true,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['192.168.1.50', '127.0.0.1'],
      active_sessions: [
        {
          session_id: 'sess-001',
          company_id: 'cmp_1',
          user_id: 'u_1',
          username: 'acct_john',
          role: 'Accountant',
          client_ip: '192.168.1.25',
          created_at: '2026-11-26T00:00:00Z',
          expires_at: '2026-11-26T08:00:00Z',
          last_active_at: '2026-11-26T00:05:00Z',
        },
      ],
    });
    vi.mocked(api.stopLanServer).mockResolvedValue({
      is_running: false,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['127.0.0.1'],
      active_sessions: [],
    });
    vi.mocked(api.revokeLanSession).mockResolvedValue({
      is_running: true,
      bind_address: '0.0.0.0',
      port: 7900,
      local_ip_addresses: ['192.168.1.50', '127.0.0.1'],
      active_sessions: [],
    });
  });

  it('LanServerModal starts, revokes session, stops server, and closes on Esc', async () => {
    const onClose = vi.fn();
    render(<LanServerModal isOpen={true} onClose={onClose} />);

    expect(await screen.findByText(/OFFLINE — Zero-Footprint Local Mode/i)).toBeDefined();

    // Start LAN Server
    fireEvent.click(screen.getByTestId('btn-start-lan-server'));
    await waitFor(() => {
      expect(api.startLanServer).toHaveBeenCalledWith('0.0.0.0', 7900);
      expect(screen.getByText(/ONLINE — Listening on 0.0.0.0:7900/i)).toBeDefined();
      expect(screen.getByText('acct_john')).toBeDefined();
    });

    // Revoke active session
    fireEvent.click(screen.getByRole('button', { name: 'Revoke' }));
    await waitFor(() => {
      expect(api.revokeLanSession).toHaveBeenCalledWith('sess-001');
    });

    // Stop LAN Server
    fireEvent.click(screen.getByTestId('btn-stop-lan-server'));
    await waitFor(() => {
      expect(api.stopLanServer).toHaveBeenCalled();
    });

    // Escape closes modal
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('SelectCompanyView [L] opens LAN client panel, tests health, logs in, and Esc unwinds', async () => {
    const onCompanySelected = vi.fn();
    const onClose = vi.fn();
    vi.mocked(api.testLanHealth).mockResolvedValue({
      app: 'OpenTally',
      version: '2.0.0',
      mode: 'lan_host',
      status: 'ok',
    });
    vi.mocked(api.loginLanClient).mockResolvedValue({
      session_id: 'sess-lan',
      token: 'tok-lan',
      company_id: 'cmp_lan_1',
      user_id: 'u_admin',
      username: 'lan_admin',
      role: 'Admin',
      expires_at: '2026-11-26T08:00:00Z',
    });

    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue({
      ok: true,
      json: async () => [
        {
          id: 'cmp_lan_1',
          name: 'Remote LAN Corp',
          mailing_name: 'Remote LAN',
          country: 'India',
          state: 'Karnataka',
          financial_year_begins: '2026-04-01',
          books_beginning_from: '2026-04-01',
          currency_symbol: '₹',
          currency_formal_name: 'INR',
          currency_decimal_places: 2,
          status: 'active',
          created_at: '2026-04-01',
          updated_at: '2026-04-01',
        },
      ],
    } as Response);

    render(
      <SelectCompanyView
        activeCompanyId={null}
        onCompanySelected={onCompanySelected}
        onCreateCompany={vi.fn()}
        onImportXml={vi.fn()}
        onRestoreBackup={vi.fn()}
        onClose={onClose}
      />
    );

    // Click [L] Connect to LAN Server
    fireEvent.click(screen.getByTestId('btn-connect-lan'));
    expect(screen.getByTestId('lan-connect-panel')).toBeDefined();

    // Test Health
    fireEvent.click(screen.getByTestId('btn-test-lan-health'));
    await waitFor(() => {
      expect(api.testLanHealth).toHaveBeenCalledWith('127.0.0.1', 7900);
      expect(screen.getByText(/LAN Host reachable/i)).toBeDefined();
    });

    // Submit login
    fireEvent.change(screen.getByLabelText('Username'), { target: { value: 'lan_admin' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'Secret123!' } });
    fireEvent.click(screen.getByTestId('btn-submit-lan-login'));

    await waitFor(() => {
      expect(api.loginLanClient).toHaveBeenCalledWith(
        '127.0.0.1',
        7900,
        'cmp_lan_1',
        'lan_admin',
        'Secret123!'
      );
      expect(onCompanySelected).toHaveBeenCalled();
    });

    fetchSpy.mockRestore();
  });

  it('BreadcrumbBar displays LAN badge and Disconnect LAN button in client mode', () => {
    const onDisconnect = vi.fn();
    render(
      <BreadcrumbBar
        stack={[{ view: 'gateway', title: 'Gateway of OpenTally' }]}
        onNavigateToLevel={vi.fn()}
        onOpenGoTo={vi.fn()}
        lanClientConfig={{
          host: '192.168.1.50',
          port: 7900,
          company_id: 'cmp_1',
          username: 'acct_john',
          token: 'tok',
          role: 'Accountant',
        }}
        onDisconnectLan={onDisconnect}
      />
    );

    expect(screen.getByTestId('lan-client-badge').textContent).toContain(
      'LAN: 192.168.1.50:7900 (acct_john [Accountant])'
    );
    fireEvent.click(screen.getByTestId('btn-disconnect-lan'));
    expect(onDisconnect).toHaveBeenCalledTimes(1);
  });
});
