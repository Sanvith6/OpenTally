import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  Company,
  VoucherType,
  VoucherNumberSeries,
  CreateVoucherSeriesRequest,
  SeriesRestartPeriod,
} from '../types';
import {
  listVoucherTypes,
  listVoucherSeries,
  createVoucherSeries,
  updateVoucherSeries,
  deactivateVoucherSeries,
  deleteVoucherSeries,
} from '../services/api';
import {
  CreateSeriesModal,
  EditSeriesModal,
  DeactivateSeriesModal,
  DeleteSeriesModal,
} from './vouchers/SeriesModals';
import './VoucherMastersView.css';

interface VoucherMastersViewProps {
  activeCompany: Company | null;
  onBackToGateway: () => void;
  onSwitchToEntry?: () => void;
}

export function VoucherMastersView({
  activeCompany,
  onBackToGateway,
  onSwitchToEntry,
}: VoucherMastersViewProps) {
  const [voucherTypes, setVoucherTypes] = useState<VoucherType[]>([]);
  const [selectedTypeId, setSelectedTypeId] = useState<string | null>(null);
  const [seriesList, setSeriesList] = useState<VoucherNumberSeries[]>([]);
  const [searchTerm, setSearchTerm] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notification, setNotification] = useState<string | null>(null);

  // Modals state
  const [isCreateOpen, setIsCreateOpen] = useState(false);
  const [isEditOpen, setIsEditOpen] = useState(false);
  const [editingSeries, setEditingSeries] = useState<VoucherNumberSeries | null>(null);
  const [isDeactivateOpen, setIsDeactivateOpen] = useState(false);
  const [deactivatingSeries, setDeactivatingSeries] = useState<VoucherNumberSeries | null>(null);
  const [isDeleteOpen, setIsDeleteOpen] = useState(false);
  const [deletingSeries, setDeletingSeries] = useState<VoucherNumberSeries | null>(null);

  // Form states
  const [seriesName, setSeriesName] = useState('');
  const [seriesCode, setSeriesCode] = useState('');
  const [numberingMethod, setNumberingMethod] = useState<'automatic' | 'manual'>('automatic');
  const [restartPeriod, setRestartPeriod] = useState<SeriesRestartPeriod | ''>('');
  const [applicableFrom, setApplicableFrom] = useState('');
  const [prefix, setPrefix] = useState('');
  const [suffix, setSuffix] = useState('');
  const [startNumber, setStartNumber] = useState(1);
  const [width, setWidth] = useState(4);
  const [formError, setFormError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState('');
  const [editActive, setEditActive] = useState(true);
  const [editError, setEditError] = useState<string | null>(null);
  const [isEditSubmitting, setIsEditSubmitting] = useState(false);

  const showNotification = (msg: string) => {
    setNotification(msg);
    setTimeout(() => {
      setNotification((curr) => (curr === msg ? null : curr));
    }, 4000);
  };

  // Load Voucher Types
  const loadVoucherTypes = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const types = await listVoucherTypes();
      setVoucherTypes(types);
      if (types.length > 0 && !selectedTypeId) {
        setSelectedTypeId(types[0].id);
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [selectedTypeId]);

  useEffect(() => {
    loadVoucherTypes();
  }, [loadVoucherTypes, activeCompany]);

  // Load Series for selected Voucher Type
  const loadSeries = useCallback(async (typeId: string) => {
    try {
      const data = await listVoucherSeries(typeId);
      setSeriesList(data);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    if (selectedTypeId) {
      loadSeries(selectedTypeId);
    } else {
      setSeriesList([]);
    }
  }, [selectedTypeId, loadSeries]);

  const selectedVoucherType = useMemo(
    () => voucherTypes.find((vt) => vt.id === selectedTypeId) || null,
    [voucherTypes, selectedTypeId]
  );

  const filteredVoucherTypes = useMemo(() => {
    if (!searchTerm.trim()) return voucherTypes;
    const term = searchTerm.toLowerCase();
    return voucherTypes.filter(
      (vt) =>
        vt.name.toLowerCase().includes(term) ||
        vt.category.toLowerCase().includes(term) ||
        vt.id.toLowerCase().includes(term)
    );
  }, [voucherTypes, searchTerm]);

  // Reset Create Form
  const resetCreateForm = useCallback(() => {
    setSeriesName('');
    setSeriesCode('');
    setNumberingMethod('automatic');
    setRestartPeriod('');
    setApplicableFrom(activeCompany?.financial_year_begins || '2026-04-01');
    setPrefix('');
    setSuffix('');
    setStartNumber(1);
    setWidth(4);
    setFormError(null);
  }, [activeCompany]);

  const handleOpenCreateModal = useCallback(() => {
    resetCreateForm();
    setIsCreateOpen(true);
  }, [resetCreateForm]);

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't intercept if an input is focused and key is not Escape
      const target = e.target as HTMLElement | null;
      const isInput =
        target && (target.tagName === 'INPUT' || target.tagName === 'SELECT' || target.tagName === 'TEXTAREA');

      if (e.key === 'Escape') {
        e.preventDefault();
        if (isCreateOpen) setIsCreateOpen(false);
        else if (isEditOpen) setIsEditOpen(false);
        else if (isDeactivateOpen) setIsDeactivateOpen(false);
        else if (isDeleteOpen) setIsDeleteOpen(false);
        else onBackToGateway();
      } else if (e.altKey && (e.key === 'n' || e.key === 'N') && !isCreateOpen && !isEditOpen) {
        e.preventDefault();
        handleOpenCreateModal();
      } else if (!isInput && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
        e.preventDefault();
        if (filteredVoucherTypes.length === 0) return;
        const currentIdx = filteredVoucherTypes.findIndex((vt) => vt.id === selectedTypeId);
        if (e.key === 'ArrowDown') {
          const nextIdx = (currentIdx + 1) % filteredVoucherTypes.length;
          setSelectedTypeId(filteredVoucherTypes[nextIdx].id);
        } else {
          const prevIdx = (currentIdx - 1 + filteredVoucherTypes.length) % filteredVoucherTypes.length;
          setSelectedTypeId(filteredVoucherTypes[prevIdx].id);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [
    isCreateOpen,
    isEditOpen,
    isDeactivateOpen,
    isDeleteOpen,
    onBackToGateway,
    handleOpenCreateModal,
    filteredVoucherTypes,
    selectedTypeId,
  ]);

  // Handle Create Series Submit
  const handleCreateSeries = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedTypeId) return;

    if (!seriesName.trim()) {
      setFormError('Series name is required');
      return;
    }
    if (!seriesCode.trim()) {
      setFormError('Series code is required');
      return;
    }
    if (!restartPeriod) {
      setFormError('Explicit selection of Restart Period is mandatory (yearly, monthly, daily, or never)');
      return;
    }

    try {
      setIsSubmitting(true);
      setFormError(null);
      const req: CreateVoucherSeriesRequest = {
        voucher_type_id: selectedTypeId,
        series_name: seriesName.trim(),
        series_code: seriesCode.trim().toUpperCase(),
        numbering_method: numberingMethod,
        restart_period: restartPeriod as SeriesRestartPeriod,
        applicable_from: applicableFrom ? applicableFrom : undefined,
        prefix: prefix ? prefix : undefined,
        suffix: suffix ? suffix : undefined,
        start_number: startNumber,
        width: width,
      };

      const created = await createVoucherSeries(req);
      showNotification(`Numbering Series '${created.series_name}' (${created.series_code}) created successfully.`);
      setIsCreateOpen(false);
      loadSeries(selectedTypeId);
    } catch (err: unknown) {
      setFormError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  // Handle Edit Series Submit
  const handleEditSeries = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingSeries || !selectedTypeId) return;

    if (!editName.trim()) {
      setEditError('Series name cannot be empty');
      return;
    }

    try {
      setIsEditSubmitting(true);
      setEditError(null);
      await updateVoucherSeries(editingSeries.id, {
        series_name: editName.trim(),
        is_active: editActive,
      });
      showNotification(`Numbering Series '${editName}' updated successfully.`);
      setIsEditOpen(false);
      loadSeries(selectedTypeId);
    } catch (err: unknown) {
      setEditError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsEditSubmitting(false);
    }
  };

  // Handle Deactivate Series
  const handleConfirmDeactivate = async () => {
    if (!deactivatingSeries || !selectedTypeId) return;
    try {
      await deactivateVoucherSeries(deactivatingSeries.id);
      showNotification(`Series '${deactivatingSeries.series_name}' has been deactivated.`);
      setIsDeactivateOpen(false);
      setDeactivatingSeries(null);
      loadSeries(selectedTypeId);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  // Handle Delete Series
  const handleConfirmDelete = async () => {
    if (!deletingSeries || !selectedTypeId) return;
    try {
      await deleteVoucherSeries(deletingSeries.id);
      showNotification(`Series '${deletingSeries.series_name}' deleted successfully.`);
      setIsDeleteOpen(false);
      setDeletingSeries(null);
      loadSeries(selectedTypeId);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
      setIsDeleteOpen(false);
      setDeletingSeries(null);
    }
  };

  const openEditModal = (series: VoucherNumberSeries) => {
    setEditingSeries(series);
    setEditName(series.series_name);
    setEditActive(series.is_active);
    setEditError(null);
    setIsEditOpen(true);
  };

  const openDeactivateModal = (series: VoucherNumberSeries) => {
    setDeactivatingSeries(series);
    setIsDeactivateOpen(true);
  };

  const openDeleteModal = (series: VoucherNumberSeries) => {
    setDeletingSeries(series);
    setIsDeleteOpen(true);
  };

  return (
    <div className="ot-vm-container">
      {/* Top Header */}
      <header className="ot-vm-header">
        <div className="ot-vm-title-group">
          <h2>
            <span>&#9776;</span> Voucher Type &amp; Numbering-Series Masters
          </h2>
          <div className="ot-vm-breadcrumbs">
            Gateway of OpenTally &gt; Masters &gt; Voucher Types &bull; Company: {activeCompany?.name || 'Default'}
          </div>
        </div>
        <div className="ot-vm-header-actions">
          {onSwitchToEntry && (
            <button
              className="ot-btn ot-btn-primary"
              onClick={onSwitchToEntry}
              title="Open Voucher Entry Workspace (V)"
            >
              Voucher Entry Workspace &rarr;
            </button>
          )}
          <button
            className="ot-btn ot-btn-secondary"
            onClick={onBackToGateway}
            title="Return to Gateway (Esc)"
          >
            &larr; Back to Gateway [Esc]
          </button>
        </div>
      </header>

      {/* Notification Banner */}
      {notification && (
        <div className="ot-notification-banner" style={{ margin: '8px 24px 0' }}>
          <span className="ot-notification-icon">&#9432;</span>
          <span className="ot-notification-text">{notification}</span>
          <button className="ot-notification-close" onClick={() => setNotification(null)}>
            &times;
          </button>
        </div>
      )}

      {/* Global Error Banner */}
      {error && (
        <div className="ot-vm-error-banner" style={{ margin: '8px 24px 0' }}>
          <strong>Error:</strong> {error}
        </div>
      )}

      {/* Main Split Layout */}
      <div className="ot-vm-body">
        {/* Left Sidebar: Voucher Types */}
        <aside className="ot-vm-sidebar">
          <div className="ot-vm-search-box">
            <input
              type="text"
              className="ot-vm-search-input"
              placeholder="Search 24 Voucher Types..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
            />
          </div>

          {loading ? (
            <div className="ot-vm-empty-state">Loading voucher types...</div>
          ) : (
            <ul className="ot-vm-types-list">
              {filteredVoucherTypes.map((vt) => {
                const isSelected = vt.id === selectedTypeId;
                return (
                  <li
                    key={vt.id}
                    className={`ot-vm-type-item ${isSelected ? 'selected' : ''}`}
                    onClick={() => setSelectedTypeId(vt.id)}
                    tabIndex={0}
                  >
                    <div className="ot-vm-type-row-main">
                      <span className="ot-vm-type-name">{vt.name}</span>
                      <span className={`ot-badge ot-badge-${vt.category}`}>{vt.category}</span>
                    </div>
                    <div className="ot-vm-type-row-sub">
                      <span className="ot-badge ot-badge-method">{vt.numbering_method}</span>
                      {vt.affects_ledger_balance && (
                        <span className="ot-badge" style={{ backgroundColor: '#065f46', color: '#a7f3d0' }}>
                          Ledger
                        </span>
                      )}
                      {vt.affects_inventory && (
                        <span className="ot-badge" style={{ backgroundColor: '#92400e', color: '#fde68a' }}>
                          Inventory
                        </span>
                      )}
                    </div>
                  </li>
                );
              })}
              {filteredVoucherTypes.length === 0 && (
                <li className="ot-vm-empty-state">No voucher types match '{searchTerm}'</li>
              )}
            </ul>
          )}
        </aside>

        {/* Right Content: Selected Voucher Type & Series List */}
        <main className="ot-vm-content">
          {selectedVoucherType ? (
            <>
              {/* Voucher Type Details Header */}
              <div className="ot-vm-detail-header">
                <div>
                  <h3>
                    {selectedVoucherType.name}
                    <span
                      className={`ot-badge ot-badge-${selectedVoucherType.category}`}
                      style={{ marginLeft: '10px' }}
                    >
                      {selectedVoucherType.category}
                    </span>
                    {selectedVoucherType.is_predefined && (
                      <span
                        className="ot-badge"
                        style={{ marginLeft: '6px', backgroundColor: '#374151', color: '#93c5fd' }}
                      >
                        System Predefined
                      </span>
                    )}
                  </h3>
                  <div className="ot-vm-detail-props">
                    <div className="ot-vm-prop-item">
                      Method: <span>{selectedVoucherType.numbering_method}</span>
                    </div>
                    <div className="ot-vm-prop-item">
                      Affects Ledger Balance:{' '}
                      <span>{selectedVoucherType.affects_ledger_balance ? 'Yes' : 'No'}</span>
                    </div>
                    <div className="ot-vm-prop-item">
                      Affects Inventory:{' '}
                      <span>{selectedVoucherType.affects_inventory ? 'Yes' : 'No'}</span>
                    </div>
                  </div>
                </div>
                <div>
                  <button className="ot-vm-btn-primary" onClick={handleOpenCreateModal}>
                    + Create Series [Alt+N]
                  </button>
                </div>
              </div>

              {/* Numbering Series Table Section */}
              <div className="ot-vm-series-section-header">
                <h4>Numbering Series Masters ({seriesList.length})</h4>
              </div>

              <div className="ot-vm-table-container">
                <table className="ot-vm-table">
                  <thead>
                    <tr>
                      <th>Series Name</th>
                      <th>Series Code</th>
                      <th>Numbering Method</th>
                      <th>Restart Frequency</th>
                      <th>Status</th>
                      <th>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {seriesList.map((series) => (
                      <tr key={series.id}>
                        <td style={{ fontWeight: 600 }}>{series.series_name}</td>
                        <td className="ot-vm-series-code-cell">{series.series_code}</td>
                        <td>
                          <span className="ot-badge ot-badge-method">{series.numbering_method}</span>
                        </td>
                        <td style={{ textTransform: 'capitalize' }}>{series.restart_period}</td>
                        <td>
                          <span
                            className={`ot-badge ${series.is_active ? 'ot-badge-active' : 'ot-badge-inactive'}`}
                          >
                            {series.is_active ? 'Active' : 'Inactive'}
                          </span>
                        </td>
                        <td className="ot-vm-actions-cell">
                          <button
                            className="ot-vm-btn-icon"
                            onClick={() => openEditModal(series)}
                            title="Edit Series"
                          >
                            Edit
                          </button>
                          {series.is_active ? (
                            <button
                              className="ot-vm-btn-icon ot-vm-btn-deactivate"
                              onClick={() => openDeactivateModal(series)}
                              title="Deactivate Series (Safe preservation)"
                            >
                              Deactivate
                            </button>
                          ) : null}
                          <button
                            className="ot-vm-btn-icon ot-vm-btn-delete"
                            onClick={() => openDeleteModal(series)}
                            title="Delete Series (if no vouchers/configs exist)"
                          >
                            Delete
                          </button>
                        </td>
                      </tr>
                    ))}
                    {seriesList.length === 0 && (
                      <tr>
                        <td colSpan={6} className="ot-vm-empty-state">
                          No numbering series found for this voucher type. Click "+ Create Series [Alt+N]" to add one.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </>
          ) : (
            <div className="ot-vm-empty-state">Select a voucher type from the left pane to view numbering series.</div>
          )}
        </main>
      </div>

      {/* Modals */}
      <CreateSeriesModal
        isOpen={isCreateOpen}
        selectedVoucherType={selectedVoucherType}
        onClose={() => setIsCreateOpen(false)}
        onSubmit={handleCreateSeries}
        seriesName={seriesName}
        setSeriesName={setSeriesName}
        seriesCode={seriesCode}
        setSeriesCode={setSeriesCode}
        numberingMethod={numberingMethod}
        setNumberingMethod={setNumberingMethod}
        restartPeriod={restartPeriod}
        setRestartPeriod={setRestartPeriod}
        applicableFrom={applicableFrom}
        setApplicableFrom={setApplicableFrom}
        startNumber={startNumber}
        setStartNumber={setStartNumber}
        prefix={prefix}
        setPrefix={setPrefix}
        width={width}
        setWidth={setWidth}
        suffix={suffix}
        setSuffix={setSuffix}
        isSubmitting={isSubmitting}
        formError={formError}
      />

      <EditSeriesModal
        isOpen={isEditOpen}
        editingSeries={editingSeries}
        onClose={() => setIsEditOpen(false)}
        onSubmit={handleEditSeries}
        editName={editName}
        setEditName={setEditName}
        editActive={editActive}
        setEditActive={setEditActive}
        isSubmitting={isEditSubmitting}
        formError={editError}
      />

      <DeactivateSeriesModal
        isOpen={isDeactivateOpen}
        deactivatingSeries={deactivatingSeries}
        onClose={() => setIsDeactivateOpen(false)}
        onConfirm={handleConfirmDeactivate}
      />

      <DeleteSeriesModal
        isOpen={isDeleteOpen}
        deletingSeries={deletingSeries}
        onClose={() => setIsDeleteOpen(false)}
        onConfirm={handleConfirmDelete}
      />
    </div>
  );
}
