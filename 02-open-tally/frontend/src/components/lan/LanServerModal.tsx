import React, { useState, useEffect, useCallback } from 'react';
import {
  getLanServerStatus,
  startLanServer,
  stopLanServer,
  revokeLanSession,
  LanServerStatusDto,
} from '../../services/api';
import './LanServerModal.css';

interface LanServerModalProps {
  isOpen: boolean;
  onClose: () => void;
  onShowNotification?: (msg: string) => void;
}

export const LanServerModal: React.FC<LanServerModalProps> = ({
  isOpen,
  onClose,
  onShowNotification,
}) => {
  const [status, setStatus] = useState<LanServerStatusDto | null>(null);
  const [bindAddress, setBindAddress] = useState('0.0.0.0');
  const [portInput, setPortInput] = useState('7900');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copiedIp, setCopiedIp] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await getLanServerStatus();
      setStatus(s);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    refreshStatus();
    const timer = window.setInterval(refreshStatus, 5000);
    return () => window.clearInterval(timer);
  }, [isOpen, refreshStatus]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleStart = async () => {
    const parsedPort = parseInt(portInput, 10);
    if (Number.isNaN(parsedPort) || parsedPort < 1024 || parsedPort > 65535) {
      setError('Port must be between 1024 and 65535 (ADR-002 default: 7900).');
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const updated = await startLanServer(bindAddress.trim() || '0.0.0.0', parsedPort);
      setStatus(updated);
      onShowNotification?.(`LAN Host started on port ${updated.port}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleStop = async () => {
    setLoading(true);
    setError(null);
    try {
      const updated = await stopLanServer();
      setStatus(updated);
      onShowNotification?.('LAN Host stopped.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleRevoke = async (sessionId: string) => {
    setError(null);
    try {
      const updated = await revokeLanSession(sessionId);
      setStatus(updated);
      onShowNotification?.(`Revoked LAN session ${sessionId.slice(0, 8)}...`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCopyUrl = (ip: string, port: number) => {
    const url = `http://${ip}:${port}`;
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(url).catch(() => {});
    }
    setCopiedIp(ip);
    window.setTimeout(() => setCopiedIp(null), 2000);
  };

  const isRunning = status?.is_running ?? false;

  return (
    <div className="ot-lan-modal-backdrop" data-testid="lan-server-modal">
      <div className="ot-lan-modal-container" role="dialog" aria-label="OpenTally LAN Host Control">
        <header className="ot-lan-modal-header">
          <div className="ot-lan-modal-title">
            <span>OpenTally Gold — LAN Host Server</span>
          </div>
          <button
            type="button"
            className="ot-lan-modal-close-btn"
            onClick={onClose}
            aria-label="Close LAN Server Modal"
          >
            &times;
          </button>
        </header>

        <div className="ot-lan-modal-body">
          {error && (
            <div className="ot-hub-error" role="alert">
              {error}
            </div>
          )}

          <div
            className={`ot-lan-status-box ${
              isRunning ? 'ot-lan-status-box-active' : 'ot-lan-status-box-inactive'
            }`}
          >
            <div>
              <div className={isRunning ? 'ot-lan-badge-active' : 'ot-lan-badge-inactive'}>
                {isRunning
                  ? `ONLINE — Listening on ${status?.bind_address}:${status?.port}`
                  : 'OFFLINE — Zero-Footprint Local Mode (no ports open)'}
              </div>
              {isRunning && status?.local_ip_addresses && status.local_ip_addresses.length > 0 && (
                <div className="ot-lan-ip-list">
                  {status.local_ip_addresses.map((ip) => (
                    <div key={ip} className="ot-lan-ip-item">
                      <span>
                        http://{ip}:{status.port}
                      </span>
                      <button
                        type="button"
                        className="ot-lan-copy-btn"
                        onClick={() => handleCopyUrl(ip, status.port)}
                      >
                        {copiedIp === ip ? 'Copied' : 'Copy'}
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="ot-lan-actions">
              {!isRunning ? (
                <button
                  type="button"
                  className="ot-lan-btn ot-lan-btn-start"
                  onClick={handleStart}
                  disabled={loading}
                  data-testid="btn-start-lan-server"
                >
                  {loading ? 'Starting...' : 'Start LAN Server'}
                </button>
              ) : (
                <button
                  type="button"
                  className="ot-lan-btn ot-lan-btn-stop"
                  onClick={handleStop}
                  disabled={loading}
                  data-testid="btn-stop-lan-server"
                >
                  {loading ? 'Stopping...' : 'Stop LAN Server'}
                </button>
              )}
            </div>
          </div>

          {!isRunning && (
            <div className="ot-lan-form-grid">
              <div className="ot-lan-form-group">
                <label htmlFor="lan-bind-address">Bind Address</label>
                <input
                  id="lan-bind-address"
                  type="text"
                  value={bindAddress}
                  onChange={(e) => setBindAddress(e.target.value)}
                />
              </div>
              <div className="ot-lan-form-group">
                <label htmlFor="lan-port-input">HTTP/JSON-RPC Port (ADR-002 Default: 7900)</label>
                <input
                  id="lan-port-input"
                  type="number"
                  value={portInput}
                  onChange={(e) => setPortInput(e.target.value)}
                />
              </div>
            </div>
          )}

          <div>
            <h4 style={{ margin: '0 0 8px 0' }}>
              Active Connected LAN Clients ({status?.active_sessions.length ?? 0})
            </h4>
            {status && status.active_sessions.length > 0 ? (
              <table className="ot-lan-clients-table">
                <thead>
                  <tr>
                    <th>Username</th>
                    <th>Role</th>
                    <th>Company</th>
                    <th>Client IP</th>
                    <th>Action</th>
                  </tr>
                </thead>
                <tbody>
                  {status.active_sessions.map((s) => (
                    <tr key={s.session_id}>
                      <td>{s.username}</td>
                      <td>{s.role}</td>
                      <td>{s.company_id}</td>
                      <td>{s.client_ip || 'LAN'}</td>
                      <td>
                        <button
                          type="button"
                          className="ot-lan-revoke-btn"
                          onClick={() => handleRevoke(s.session_id)}
                        >
                          Revoke
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <div style={{ color: '#94a3b8', fontSize: '0.85rem' }}>
                No active remote LAN client sessions.
              </div>
            )}
          </div>
        </div>

        <footer className="ot-lan-modal-footer">
          <span>SQLite WAL + busy_timeout=10000ms | Argon2id RBAC Enforced</span>
          <span>Esc: Close</span>
        </footer>
      </div>
    </div>
  );
};
