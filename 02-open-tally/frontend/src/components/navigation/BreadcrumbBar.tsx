import React from 'react';
import { NavigationStackEntry, LanClientConfig } from '../../types';
import './BreadcrumbBar.css';

interface BreadcrumbBarProps {
  stack: NavigationStackEntry[];
  onNavigateToLevel: (index: number) => void;
  onOpenGoTo: () => void;
  lanClientConfig?: LanClientConfig | null;
  onDisconnectLan?: () => void;
}

export const BreadcrumbBar: React.FC<BreadcrumbBarProps> = ({
  stack,
  onNavigateToLevel,
  onOpenGoTo,
  lanClientConfig,
  onDisconnectLan,
}) => {
  return (
    <nav className="ot-breadcrumb-bar" aria-label="Hierarchical navigation path">
      <div className="ot-breadcrumb-trail">
        {stack.map((entry, idx) => {
          const isLast = idx === stack.length - 1;
          return (
            <React.Fragment key={`${entry.view}-${idx}`}>
              {idx > 0 && <span className="ot-breadcrumb-separator">&gt;</span>}
              <button
                type="button"
                className={`ot-breadcrumb-item ${isLast ? 'ot-breadcrumb-item-active' : ''}`}
                onClick={() => !isLast && onNavigateToLevel(idx)}
                disabled={isLast}
                title={entry.title}
              >
                {entry.title}
              </button>
            </React.Fragment>
          );
        })}
      </div>

      <div className="ot-breadcrumb-actions">
        {lanClientConfig && (
          <div className="ot-lan-client-badge" data-testid="lan-client-badge">
            <span>
              LAN: {lanClientConfig.host}:{lanClientConfig.port} ({lanClientConfig.username} [
              {lanClientConfig.role}])
            </span>
            {onDisconnectLan && (
              <button
                type="button"
                className="ot-lan-disconnect-btn"
                onClick={onDisconnectLan}
                data-testid="btn-disconnect-lan"
              >
                Disconnect LAN
              </button>
            )}
          </div>
        )}
        <button
          type="button"
          className="ot-goto-pill-btn"
          onClick={onOpenGoTo}
          title="Open Go To Command Palette (Alt+G / Ctrl+G)"
          data-testid="open-goto-button"
        >
          <span className="ot-goto-pill-icon">&#128269;</span>
          <span className="ot-goto-pill-text">Go To</span>
          <kbd className="ot-goto-pill-kbd">Alt+G</kbd>
        </button>
      </div>
    </nav>
  );
};
