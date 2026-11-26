interface HeaderProps {
  appName: string;
  version: string;
  environment: string;
  companyName?: string | null;
  periodLabel?: string | null;
  username?: string | null;
  userRole?: string | null;
  lanBadge?: string | null;
  onCompanyClick?: () => void;
  onBackupClick?: () => void;
  onXmlClick?: () => void;
  onGoToClick?: () => void;
  onFeaturesClick?: () => void;
}

export function Header({
  appName,
  version,
  environment,
  companyName,
  periodLabel,
  username,
  userRole,
  lanBadge,
  onCompanyClick,
  onBackupClick,
  onXmlClick,
  onGoToClick,
  onFeaturesClick,
}: HeaderProps) {
  return (
    <header className="ot-header" data-testid="top-utility-bar">
      <div className="ot-header-left">
        <div className="ot-logo">
          <span className="ot-logo-bracket">[</span>
          <span className="ot-logo-text">{appName}</span>
          <span className="ot-logo-bracket">]</span>
        </div>
        <div className="ot-header-menu-triggers">
          <button
            type="button"
            className="ot-top-trigger-btn"
            onClick={onCompanyClick}
            title="Select / Change Company (Alt+K / F3)"
          >
            Alt+<span className="ot-hotkey-underline">K</span>: Company
          </button>
          <button
            type="button"
            className="ot-top-trigger-btn"
            onClick={onBackupClick}
            title="Backup & Restore Data (Alt+B)"
          >
            Alt+<span className="ot-hotkey-underline">B</span>: Data
          </button>
          <button
            type="button"
            className="ot-top-trigger-btn"
            onClick={onXmlClick}
            title="Exchange / Tally XML (Alt+I)"
          >
            Alt+<span className="ot-hotkey-underline">I</span>: Exchange
          </button>
          <button
            type="button"
            className="ot-top-trigger-btn"
            onClick={onFeaturesClick}
            title="Company Features (F11)"
          >
            <span className="ot-hotkey-underline">F11</span>: Features
          </button>
        </div>
      </div>

      <div className="ot-header-center">
        <button
          type="button"
          className="ot-top-trigger-btn ot-top-trigger-pill"
          onClick={onGoToClick}
          title="Open Go To Command Palette (Alt+G)"
        >
          Alt+<span className="ot-hotkey-underline">G</span>: Go To
        </button>
        <span className="ot-env-badge">{environment.toUpperCase()}</span>
      </div>

      <div className="ot-header-right">
        <div className="ot-header-context-badges">
          {companyName && (
            <span className="ot-top-ctx-pill" title={companyName}>
              {companyName}
            </span>
          )}
          {periodLabel && <span className="ot-top-ctx-pill">{periodLabel}</span>}
          {username && (
            <span className="ot-top-ctx-pill">
              {username} ({userRole || 'Operator'})
            </span>
          )}
          {lanBadge && <span className="ot-top-ctx-pill ot-top-ctx-lan">{lanBadge}</span>}
        </div>
        <span className="ot-version">v{version}</span>
      </div>
    </header>
  );
}
