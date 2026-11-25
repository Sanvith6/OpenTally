import React, { useState, useEffect, useCallback } from 'react';
import { BackupManifest, BackupStatus, RestoreResult } from '../../types';
import { createDatabaseBackup, getBackupStatus, restoreDatabaseBackup } from '../../services/api';
import './BackupRestoreModal.css';

interface BackupRestoreModalProps {
  isOpen: boolean;
  onClose: () => void;
  onShowNotification?: (msg: string) => void;
}

export const BackupRestoreModal: React.FC<BackupRestoreModalProps> = ({
  isOpen,
  onClose,
  onShowNotification,
}) => {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionSuccess, setActionSuccess] = useState<string | null>(null);

  // Backup Form
  const [backupPath, setBackupPath] = useState('');
  const [backingUp, setBackingUp] = useState(false);

  // Restore Form
  const [restorePath, setRestorePath] = useState('');
  const [restoring, setRestoring] = useState(false);
  const [confirmRestore, setConfirmRestore] = useState(false);

  const loadStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getBackupStatus();
      setStatus(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      const ts = new Date().toISOString().replace(/[-:T]/g, '_').substring(0, 15);
      setBackupPath(`backups/opentally_backup_${ts}.db`);
      setRestorePath('');
      setConfirmRestore(false);
      setError(null);
      setActionSuccess(null);
      void loadStatus();
    }
  }, [isOpen, loadStatus]);

  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleEsc, true);
    return () => window.removeEventListener('keydown', handleEsc, true);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const handleCreateBackup = async () => {
    if (!backupPath.trim()) {
      setError('Please provide a destination backup path');
      return;
    }
    setBackingUp(true);
    setError(null);
    setActionSuccess(null);
    try {
      const manifest: BackupManifest = await createDatabaseBackup(backupPath.trim());
      setActionSuccess(
        `Backup created successfully: ${manifest.backup_file} (SHA-256: ${manifest.sha256_checksum.substring(0, 12)}...)`
      );
      if (onShowNotification) {
        onShowNotification(`Backup created: ${manifest.backup_file}`);
      }
      await loadStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBackingUp(false);
    }
  };

  const handleRestore = async () => {
    if (!restorePath.trim()) {
      setError('Please provide a backup file path to restore from');
      return;
    }
    setRestoring(true);
    setError(null);
    setActionSuccess(null);
    try {
      const res: RestoreResult = await restoreDatabaseBackup(restorePath.trim());
      if (res.success) {
        setActionSuccess(`Restore successful! Schema version: ${res.schema_version}. Safety snapshot created.`);
        if (onShowNotification) {
          onShowNotification('Database restored successfully from backup.');
        }
        setConfirmRestore(false);
        await loadStatus();
      } else {
        setError(res.message || 'Restoration failed');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRestoring(false);
    }
  };

  return (
    <div className="ot-modal-overlay" onClick={onClose} role="dialog" aria-modal="true">
      <div className="ot-modal-card ot-backup-modal" onClick={(e) => e.stopPropagation()}>
        <div className="ot-modal-header">
          <h2>WAL-Safe SQLite Backup & Restore</h2>
          <button type="button" className="ot-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>

        <div className="ot-modal-body">
          {error && <div className="ot-alert ot-alert-danger">{error}</div>}
          {actionSuccess && <div className="ot-alert ot-alert-success">{actionSuccess}</div>}

          {/* Engine Status Card */}
          <div className="ot-backup-status-card">
            <div className="ot-status-row">
              <span className="ot-status-label">Database Engine:</span>
              <span className="ot-status-val">{status?.database_engine || 'SQLite 3'}</span>
            </div>
            <div className="ot-status-row">
              <span className="ot-status-label">Journal Mode:</span>
              <span className={`ot-status-badge ${status?.is_wal_active ? 'badge-wal' : ''}`}>
                {status?.is_wal_active ? 'WAL (Write-Ahead Logging)' : 'Standard'}
              </span>
            </div>
            <div className="ot-status-row">
              <span className="ot-status-label">Schema Version:</span>
              <span className="ot-status-val">{status?.current_schema_version || 'v0.7.0-notes'}</span>
            </div>
            <div className="ot-status-row">
              <span className="ot-status-label">Last Backup:</span>
              <span className="ot-status-val">
                {status?.last_backup_file
                  ? `${status.last_backup_file} (${status.last_backup_timestamp?.substring(0, 19).replace('T', ' ')})`
                  : 'No backup recorded'}
              </span>
            </div>
            {status?.last_backup_sha256 && (
              <div className="ot-status-row">
                <span className="ot-status-label">Checksum:</span>
                <span className="ot-status-mono">{status.last_backup_sha256}</span>
              </div>
            )}
          </div>

          {/* Section 1: Create Backup */}
          <div className="ot-backup-section">
            <h3>1. Create Database Snapshot</h3>
            <p className="ot-section-desc">
              Creates a crash-safe point-in-time copy using <code>VACUUM INTO</code> with a verified SHA-256 manifest.
              Safe to run live while vouchers are being entered.
            </p>
            <div className="ot-input-group">
              <label htmlFor="backup-dest-input">Destination File Path:</label>
              <input
                id="backup-dest-input"
                type="text"
                className="ot-input"
                value={backupPath}
                onChange={(e) => setBackupPath(e.target.value)}
                placeholder="backups/opentally_backup.db"
                disabled={backingUp}
              />
            </div>
            <button
              type="button"
              className="ot-btn ot-btn-primary"
              onClick={handleCreateBackup}
              disabled={backingUp || loading}
            >
              {backingUp ? 'Creating Backup...' : 'Create Backup Snapshot'}
            </button>
          </div>

          {/* Section 2: Restore Backup */}
          <div className="ot-backup-section ot-restore-section">
            <h3>2. Restore Database from Backup</h3>
            <p className="ot-section-desc">
              Verifies SHA-256 sidecar manifest and runs <code>PRAGMA integrity_check</code> before restoring.
              An automatic pre-restore safety snapshot will be preserved.
            </p>
            <div className="ot-input-group">
              <label htmlFor="restore-src-input">Backup File Path to Restore:</label>
              <input
                id="restore-src-input"
                type="text"
                className="ot-input"
                value={restorePath}
                onChange={(e) => {
                  setRestorePath(e.target.value);
                  setConfirmRestore(false);
                }}
                placeholder="backups/opentally_backup_2026_05_15.db"
                disabled={restoring}
              />
            </div>

            {!confirmRestore ? (
              <button
                type="button"
                className="ot-btn ot-btn-warning"
                onClick={() => setConfirmRestore(true)}
                disabled={restoring || !restorePath.trim()}
              >
                Prepare Restore...
              </button>
            ) : (
              <div className="ot-confirm-restore-box">
                <p className="ot-warning-text">
                  &#9888; Warning: Restoring will overwrite all current data with the contents of this backup. Are you sure you want to proceed?
                </p>
                <div className="ot-btn-row">
                  <button
                    type="button"
                    className="ot-btn ot-btn-danger"
                    onClick={handleRestore}
                    disabled={restoring}
                  >
                    {restoring ? 'Verifying & Restoring...' : 'Yes, Confirm & Restore Now'}
                  </button>
                  <button
                    type="button"
                    className="ot-btn"
                    onClick={() => setConfirmRestore(false)}
                    disabled={restoring}
                  >
                    Cancel
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>

        <div className="ot-modal-footer">
          <button type="button" className="ot-btn" onClick={onClose}>
            Close (Esc)
          </button>
        </div>
      </div>
    </div>
  );
};
