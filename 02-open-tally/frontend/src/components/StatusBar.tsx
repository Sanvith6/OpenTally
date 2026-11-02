import { HealthResponse } from '../types';

interface StatusBarProps {
  health: HealthResponse | null;
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
}

export function StatusBar({ health, loading, error, onRefresh }: StatusBarProps) {
  const formatTime = (isoString?: string) => {
    if (!isoString) return '--:--:--';
    try {
      const d = new Date(isoString);
      return d.toLocaleTimeString();
    } catch {
      return isoString;
    }
  };

  return (
    <footer className="ot-statusbar">
      <div className="ot-status-left">
        {loading ? (
          <span className="ot-status-indicator ot-pulse">Connecting to Rust Backend...</span>
        ) : error ? (
          <span className="ot-status-indicator ot-error">IPC Failure: {error}</span>
        ) : (
          <span className="ot-status-indicator ot-online">
            <span className="ot-dot"></span>
            Rust Core: <strong>{health?.status.toUpperCase()}</strong> (v{health?.app_version})
          </span>
        )}

        {health?.database && (
          <span className="ot-status-segment">
            DB: <strong>{health.database.journal_mode.toUpperCase()}</strong> (FK: {health.database.foreign_keys_enabled ? 'ON' : 'OFF'}) | Schema: {health.database.schema_version}
          </span>
        )}

        {health?.security && (
          <span className="ot-status-segment">
            Security: <strong>{health.security.encryption_active ? 'ENCRYPTED' : 'UNENCRYPTED'}</strong> (SQLCipher pending)
          </span>
        )}
      </div>

      <div className="ot-status-center">
        <span className="ot-status-tag">Tauri 2 IPC</span>
        <button className="ot-refresh-btn" onClick={onRefresh} title="Ping Rust Backend via get_app_health">
          &#8635; Ping Backend
        </button>
      </div>

      <div className="ot-status-right">
        <span className="ot-status-time">Sys Time: {formatTime(health?.timestamp)}</span>
        <span className="ot-status-mode">MODE: NAV</span>
      </div>
    </footer>
  );
}
