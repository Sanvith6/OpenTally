interface HeaderProps {
  appName: string;
  version: string;
  environment: string;
}

export function Header({ appName, version, environment }: HeaderProps) {
  return (
    <header className="ot-header">
      <div className="ot-header-left">
        <div className="ot-logo">
          <span className="ot-logo-bracket">[</span>
          <span className="ot-logo-text">{appName}</span>
          <span className="ot-logo-bracket">]</span>
        </div>
        <span className="ot-tagline">Open-Source Accounting Suite</span>
      </div>

      <div className="ot-header-center">
        <span className="ot-mode-badge">PHASE 2 &bull; SPRINT 0</span>
        <span className="ot-env-badge">{environment.toUpperCase()}</span>
      </div>

      <div className="ot-header-right">
        <span className="ot-hotkey-hint"><kbd>F1</kbd> Help</span>
        <span className="ot-hotkey-hint"><kbd>Alt+K</kbd> Company</span>
        <span className="ot-hotkey-hint"><kbd>Alt+F2</kbd> Period</span>
        <span className="ot-hotkey-hint"><kbd>Esc</kbd> Exit</span>
        <span className="ot-version">v{version}</span>
      </div>
    </header>
  );
};

