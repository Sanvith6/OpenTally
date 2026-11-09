import React from 'react';
import {
  VoucherType,
  VoucherNumberSeries,
  SeriesRestartPeriod,
} from '../../types/voucher';

export interface CreateSeriesModalProps {
  isOpen: boolean;
  selectedVoucherType: VoucherType | null;
  onClose: () => void;
  onSubmit: (e: React.FormEvent) => void;
  seriesName: string;
  setSeriesName: (val: string) => void;
  seriesCode: string;
  setSeriesCode: (val: string) => void;
  numberingMethod: 'automatic' | 'manual';
  setNumberingMethod: (val: 'automatic' | 'manual') => void;
  restartPeriod: SeriesRestartPeriod | '';
  setRestartPeriod: (val: SeriesRestartPeriod) => void;
  applicableFrom: string;
  setApplicableFrom: (val: string) => void;
  startNumber: number;
  setStartNumber: (val: number) => void;
  prefix: string;
  setPrefix: (val: string) => void;
  width: number;
  setWidth: (val: number) => void;
  suffix: string;
  setSuffix: (val: string) => void;
  isSubmitting: boolean;
  formError: string | null;
}

export function CreateSeriesModal({
  isOpen,
  selectedVoucherType,
  onClose,
  onSubmit,
  seriesName,
  setSeriesName,
  seriesCode,
  setSeriesCode,
  numberingMethod,
  setNumberingMethod,
  restartPeriod,
  setRestartPeriod,
  applicableFrom,
  setApplicableFrom,
  startNumber,
  setStartNumber,
  prefix,
  setPrefix,
  width,
  setWidth,
  suffix,
  setSuffix,
  isSubmitting,
  formError,
}: CreateSeriesModalProps) {
  if (!isOpen) return null;

  return (
    <div className="ot-vm-modal-overlay">
      <div className="ot-vm-modal">
        <div className="ot-vm-modal-header">
          <h3>Create Numbering Series &mdash; {selectedVoucherType?.name}</h3>
          <button className="ot-vm-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <form onSubmit={onSubmit} noValidate>
          <div className="ot-vm-modal-body">
            {formError && <div className="ot-vm-error-banner">{formError}</div>}

            <div className="ot-vm-form-row">
              <label htmlFor="series-name">Series Name *</label>
              <input
                id="series-name"
                type="text"
                className="ot-vm-form-input"
                placeholder="e.g. Main Tax Invoice, Export Sales"
                value={seriesName}
                onChange={(e) => setSeriesName(e.target.value)}
                autoFocus
                required
              />
            </div>

            <div className="ot-vm-form-row">
              <label htmlFor="series-code">Series Code (Namespace Anchor) *</label>
              <input
                id="series-code"
                type="text"
                className="ot-vm-form-input"
                placeholder="e.g. TAX, EXP, MAN, PRI (2-10 uppercase chars)"
                value={seriesCode}
                onChange={(e) => setSeriesCode(e.target.value.toUpperCase())}
                maxLength={10}
                required
              />
              <span className="ot-vm-field-help">
                Immutable namespace anchor. Guaranteed disjoint across series per uq_series_code.
              </span>
            </div>

            <div className="ot-vm-form-row">
              <label htmlFor="numbering-method">Numbering Method</label>
              <select
                id="numbering-method"
                className="ot-vm-form-select"
                value={numberingMethod}
                onChange={(e) => setNumberingMethod(e.target.value as 'automatic' | 'manual')}
              >
                <option value="automatic">Automatic (Sequential counter)</option>
                <option value="manual">Manual (User entered with format validation)</option>
              </select>
            </div>

            <div className="ot-vm-form-row">
              <label htmlFor="restart-period">Restart Period * (No Default Fallback)</label>
              <select
                id="restart-period"
                className="ot-vm-form-select"
                value={restartPeriod}
                onChange={(e) => setRestartPeriod(e.target.value as SeriesRestartPeriod)}
                required
              >
                <option value="">-- Select Mandatory Reset Policy --</option>
                <option value="yearly">Yearly (Resets each Financial Year)</option>
                <option value="monthly">Monthly (Resets on 1st of each Month)</option>
                <option value="daily">Daily (Resets each Day)</option>
                <option value="never">Never (Continuous sequential)</option>
              </select>
              <span className="ot-vm-field-warning">
                Warning: Series restart_period is permanently immutable once created (Trigger 12).
              </span>
            </div>

            <div style={{ borderTop: '1px solid #374151', paddingTop: '10px' }}>
              <div style={{ fontWeight: 600, fontSize: '0.85rem', color: '#93c5fd', marginBottom: '8px' }}>
                Initial Configuration (Optional)
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px' }}>
                <div className="ot-vm-form-row">
                  <label htmlFor="app-from">Effective From</label>
                  <input
                    id="app-from"
                    type="date"
                    className="ot-vm-form-input"
                    value={applicableFrom}
                    onChange={(e) => setApplicableFrom(e.target.value)}
                  />
                </div>
                <div className="ot-vm-form-row">
                  <label htmlFor="start-num">Start Number</label>
                  <input
                    id="start-num"
                    type="number"
                    className="ot-vm-form-input"
                    min={1}
                    value={startNumber}
                    onChange={(e) => setStartNumber(parseInt(e.target.value) || 1)}
                  />
                </div>
              </div>

              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px', marginTop: '8px' }}>
                <div className="ot-vm-form-row">
                  <label htmlFor="prefix-tmpl">Prefix Template</label>
                  <input
                    id="prefix-tmpl"
                    type="text"
                    className="ot-vm-form-input"
                    placeholder="e.g. INV/{YY-YY}/"
                    value={prefix}
                    onChange={(e) => setPrefix(e.target.value)}
                  />
                </div>
                <div className="ot-vm-form-row">
                  <label htmlFor="width-input">Zero Padding Width</label>
                  <input
                    id="width-input"
                    type="number"
                    className="ot-vm-form-input"
                    min={1}
                    max={10}
                    value={width}
                    onChange={(e) => setWidth(parseInt(e.target.value) || 4)}
                  />
                </div>
              </div>

              <div className="ot-vm-form-row" style={{ marginTop: '8px' }}>
                <label htmlFor="suffix-tmpl">Suffix Template</label>
                <input
                  id="suffix-tmpl"
                  type="text"
                  className="ot-vm-form-input"
                  placeholder="e.g. /26-27 or blank"
                  value={suffix}
                  onChange={(e) => setSuffix(e.target.value)}
                />
              </div>
            </div>
          </div>

          <div className="ot-vm-modal-footer">
            <button
              type="button"
              className="ot-vm-btn-secondary"
              onClick={onClose}
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button type="submit" className="ot-vm-btn-primary" disabled={isSubmitting}>
              {isSubmitting ? 'Creating...' : 'Create Numbering Series'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export interface EditSeriesModalProps {
  isOpen: boolean;
  editingSeries: VoucherNumberSeries | null;
  onClose: () => void;
  onSubmit: (e: React.FormEvent) => void;
  editName: string;
  setEditName: (val: string) => void;
  editActive: boolean;
  setEditActive: (val: boolean) => void;
  isSubmitting: boolean;
  formError: string | null;
}

export function EditSeriesModal({
  isOpen,
  editingSeries,
  onClose,
  onSubmit,
  editName,
  setEditName,
  editActive,
  setEditActive,
  isSubmitting,
  formError,
}: EditSeriesModalProps) {
  if (!isOpen || !editingSeries) return null;

  return (
    <div className="ot-vm-modal-overlay">
      <div className="ot-vm-modal">
        <div className="ot-vm-modal-header">
          <h3>Edit Series &mdash; {editingSeries.series_code}</h3>
          <button className="ot-vm-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <form onSubmit={onSubmit}>
          <div className="ot-vm-modal-body">
            {formError && <div className="ot-vm-error-banner">{formError}</div>}

            <div className="ot-vm-form-row">
              <label htmlFor="edit-name">Series Name *</label>
              <input
                id="edit-name"
                type="text"
                className="ot-vm-form-input"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                required
                autoFocus
              />
            </div>

            <div className="ot-vm-form-row">
              <label>Series Code (Immutable)</label>
              <input
                type="text"
                className="ot-vm-form-input"
                value={editingSeries.series_code}
                disabled
              />
              <span className="ot-vm-field-help">Protected against mutation by Trigger 8a once vouchers exist.</span>
            </div>

            <div className="ot-vm-form-row">
              <label>Restart Period (Immutable)</label>
              <input
                type="text"
                className="ot-vm-form-input"
                value={editingSeries.restart_period}
                disabled
              />
              <span className="ot-vm-field-help">Permanently immutable per Trigger 12.</span>
            </div>

            <div className="ot-vm-form-row" style={{ flexDirection: 'row', alignItems: 'center', gap: '10px' }}>
              <input
                id="edit-active"
                type="checkbox"
                checked={editActive}
                onChange={(e) => setEditActive(e.target.checked)}
                style={{ width: '18px', height: '18px' }}
              />
              <label htmlFor="edit-active" style={{ cursor: 'pointer', margin: 0 }}>
                Active (Permit new voucher issuance)
              </label>
            </div>
          </div>

          <div className="ot-vm-modal-footer">
            <button
              type="button"
              className="ot-vm-btn-secondary"
              onClick={onClose}
              disabled={isSubmitting}
            >
              Cancel
            </button>
            <button type="submit" className="ot-vm-btn-primary" disabled={isSubmitting}>
              {isSubmitting ? 'Saving...' : 'Save Changes'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

export interface DeactivateSeriesModalProps {
  isOpen: boolean;
  deactivatingSeries: VoucherNumberSeries | null;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeactivateSeriesModal({
  isOpen,
  deactivatingSeries,
  onClose,
  onConfirm,
}: DeactivateSeriesModalProps) {
  if (!isOpen || !deactivatingSeries) return null;

  return (
    <div className="ot-vm-modal-overlay">
      <div className="ot-vm-modal" style={{ width: '440px' }}>
        <div className="ot-vm-modal-header" style={{ borderColor: '#b45309' }}>
          <h3 style={{ color: '#fcd34d' }}>Deactivate Numbering Series</h3>
          <button className="ot-vm-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <div className="ot-vm-modal-body">
          <p>
            Are you sure you want to deactivate series{' '}
            <strong>'{deactivatingSeries.series_name}' ({deactivatingSeries.series_code})</strong>?
          </p>
          <p className="ot-vm-field-help" style={{ color: '#d1d5db' }}>
            Deactivation prevents newly created vouchers from selecting this series, while preserving historical
            accounting integrity and sequence audit trails. This is the recommended operational workflow.
          </p>
        </div>
        <div className="ot-vm-modal-footer">
          <button
            type="button"
            className="ot-vm-btn-secondary"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="ot-vm-btn-primary"
            style={{ backgroundColor: '#b45309' }}
            onClick={onConfirm}
          >
            Confirm Deactivation
          </button>
        </div>
      </div>
    </div>
  );
}

export interface DeleteSeriesModalProps {
  isOpen: boolean;
  deletingSeries: VoucherNumberSeries | null;
  onClose: () => void;
  onConfirm: () => void;
}

export function DeleteSeriesModal({
  isOpen,
  deletingSeries,
  onClose,
  onConfirm,
}: DeleteSeriesModalProps) {
  if (!isOpen || !deletingSeries) return null;

  return (
    <div className="ot-vm-modal-overlay">
      <div className="ot-vm-modal" style={{ width: '450px' }}>
        <div className="ot-vm-modal-header" style={{ borderColor: '#ef4444' }}>
          <h3 style={{ color: '#f87171' }}>Delete Numbering Series</h3>
          <button className="ot-vm-modal-close" onClick={onClose}>
            &times;
          </button>
        </div>
        <div className="ot-vm-modal-body">
          <p>
            Attempting to delete series{' '}
            <strong>'{deletingSeries.series_name}' ({deletingSeries.series_code})</strong>.
          </p>
          <p className="ot-vm-field-warning">
            <strong>Attention:</strong> If any configurations, sequences, or permanent financial vouchers exist in this
            series, SQLite Foreign Key constraints and Trigger 8b will strictly reject deletion. If transactions
            exist, please choose <em>Deactivate</em> instead.
          </p>
        </div>
        <div className="ot-vm-modal-footer">
          <button
            type="button"
            className="ot-vm-btn-secondary"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="ot-vm-btn-primary"
            style={{ backgroundColor: '#dc2626' }}
            onClick={onConfirm}
          >
            Attempt Deletion
          </button>
        </div>
      </div>
    </div>
  );
}
