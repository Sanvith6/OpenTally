import React, { useState, useEffect, useCallback } from 'react';
import {
  Company,
  CustomReportDataSource,
  CustomReportDefinitionDto,
  CustomReportExecutionResult,
} from '../../types';
import {
  listCustomReports,
  saveCustomReport,
  deleteCustomReport,
  runCustomReport,
  exportCustomReportCsv,
  exportCustomReportTemplate,
  importCustomReportTemplate,
} from '../../services/api';
import './CustomReportsView.css';

interface CustomReportsViewProps {
  activeCompany: Company | null;
  onBack: () => void;
}

const DATA_SOURCE_COLUMNS: Record<CustomReportDataSource, string[]> = {
  Vouchers: ['id', 'voucher_number', 'date', 'status', 'voucher_type_id', 'reference_no', 'narration', 'total_amount'],
  Ledgers: ['id', 'name', 'group_id', 'ledger_classification', 'is_reserved'],
  StockItems: ['id', 'name', 'sku', 'hsn_sac_code', 'opening_qty', 'opening_value'],
  OutstandingBills: ['id', 'bill_name', 'party_ledger_id', 'allocation_type', 'dr_cr', 'amount', 'due_date'],
  GstInvoices: ['id', 'document_number', 'document_date', 'status', 'grand_total', 'total_tax'],
};

export const CustomReportsView: React.FC<CustomReportsViewProps> = ({ activeCompany, onBack }) => {
  const [reports, setReports] = useState<CustomReportDefinitionDto[]>([]);
  const [selectedReport, setSelectedReport] = useState<CustomReportDefinitionDto | null>(null);
  const [execResult, setExecResult] = useState<CustomReportExecutionResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  // Builder state
  const [showBuilder, setShowBuilder] = useState(false);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [dataSource, setDataSource] = useState<CustomReportDataSource>('Vouchers');
  const [columns, setColumns] = useState<string[]>(['voucher_number', 'date', 'total_amount']);
  const [sortBy, setSortBy] = useState<string>('date');
  const [sortDir, setSortDir] = useState<'ASC' | 'DESC'>('ASC');

  // Template import modal
  const [showImport, setShowImport] = useState(false);
  const [importJson, setImportJson] = useState('');

  const loadReports = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const list = await listCustomReports();
      setReports(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany]);

  useEffect(() => {
    loadReports();
  }, [loadReports]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        if (showImport) setShowImport(false);
        else if (showBuilder) setShowBuilder(false);
        else onBack();
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [showImport, showBuilder, onBack]);

  const handleToggleColumn = (col: string) => {
    setColumns((prev) => (prev.includes(col) ? prev.filter((c) => c !== col) : [...prev, col]));
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setMessage(null);
    try {
      const saved = await saveCustomReport({
        name,
        description,
        data_source: dataSource,
        columns,
        sort_by: sortBy || undefined,
        sort_dir: sortDir,
      });
      setShowBuilder(false);
      setName('');
      setDescription('');
      setMessage(`Saved custom report '${saved.name}'.`);
      await loadReports();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleRun = async (r: CustomReportDefinitionDto) => {
    setSelectedReport(r);
    setError(null);
    setLoading(true);
    try {
      const res = await runCustomReport(r.id);
      setExecResult(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleExportCsv = async (reportId: string) => {
    try {
      const csv = await exportCustomReportCsv(reportId);
      setMessage(`CSV generated (${csv.split('\n').length - 1} lines).`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportTemplate = async (reportId: string) => {
    try {
      const tmpl = await exportCustomReportTemplate(reportId);
      if (navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(tmpl).catch(() => {});
      }
      setMessage('Exported .opentally-report.json template.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleImportTemplate = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      const imported = await importCustomReportTemplate(importJson);
      setShowImport(false);
      setImportJson('');
      setMessage(`Imported template '${imported.name}'.`);
      await loadReports();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDelete = async (reportId: string) => {
    try {
      await deleteCustomReport(reportId);
      if (selectedReport?.id === reportId) {
        setSelectedReport(null);
        setExecResult(null);
      }
      await loadReports();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="ot-crpt-container" data-testid="custom-reports-view">
      <header className="ot-crpt-header">
        <div>
          <h2>Custom Report Extensions (Declarative Builder)</h2>
          <span className="ot-crpt-sub">{activeCompany?.name || 'No Company Selected'}</span>
        </div>
        <div className="ot-crpt-actions">
          <button
            type="button"
            className="ot-btn ot-btn-secondary"
            onClick={() => setShowImport(true)}
            data-testid="btn-import-report-template"
          >
            Import .opentally-report.json
          </button>
          <button
            type="button"
            className="ot-btn ot-btn-primary"
            onClick={() => setShowBuilder(true)}
            data-testid="btn-new-custom-report"
          >
            + New Custom Report
          </button>
          <button type="button" className="ot-btn ot-btn-secondary" onClick={onBack}>
            Esc: Back
          </button>
        </div>
      </header>

      {error && <div className="ot-crpt-alert ot-crpt-alert-err">{error}</div>}
      {message && <div className="ot-crpt-alert ot-crpt-alert-ok">{message}</div>}

      <div className="ot-crpt-layout">
        <aside className="ot-crpt-sidebar">
          <h3>Saved Reports ({reports.length})</h3>
          {reports.map((r) => (
            <div
              key={r.id}
              className={`ot-crpt-item ${selectedReport?.id === r.id ? 'active' : ''}`}
            >
              <div className="ot-crpt-item-title" onClick={() => handleRun(r)}>
                <strong>{r.name}</strong>
                <span>{r.data_source}</span>
              </div>
              <div className="ot-crpt-item-btns">
                <button type="button" onClick={() => handleRun(r)}>
                  Run
                </button>
                <button type="button" onClick={() => handleExportCsv(r.id)}>
                  CSV
                </button>
                <button type="button" onClick={() => handleExportTemplate(r.id)}>
                  Template
                </button>
                <button type="button" onClick={() => handleDelete(r.id)}>
                  Del
                </button>
              </div>
            </div>
          ))}
          {reports.length === 0 && !loading && (
            <div className="ot-crpt-empty">No custom reports yet. Click + New Custom Report.</div>
          )}
        </aside>

        <main className="ot-crpt-main">
          {execResult ? (
            <div>
              <div className="ot-crpt-result-header">
                <h3>
                  {execResult.report_name} ({execResult.row_count} rows)
                </h3>
              </div>
              <table className="ot-crpt-table">
                <thead>
                  <tr>
                    {execResult.columns.map((c) => (
                      <th key={c}>{c}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {execResult.rows.map((row, idx) => (
                    <tr key={idx}>
                      {execResult.columns.map((c) => (
                        <td key={c}>{row[c] ?? ''}</td>
                      ))}
                    </tr>
                  ))}
                </tbody>
                {Object.keys(execResult.numeric_totals).length > 0 && (
                  <tfoot>
                    <tr>
                      {execResult.columns.map((c, idx) => (
                        <td key={c}>
                          <strong>
                            {execResult.numeric_totals[c]
                              ? execResult.numeric_totals[c]
                              : idx === 0
                                ? 'TOTAL'
                                : ''}
                          </strong>
                        </td>
                      ))}
                    </tr>
                  </tfoot>
                )}
              </table>
            </div>
          ) : (
            <div className="ot-crpt-empty">Select a custom report on the left and click Run.</div>
          )}
        </main>
      </div>

      {showBuilder && (
        <div className="ot-crpt-modal-backdrop">
          <form className="ot-crpt-modal" onSubmit={handleSave}>
            <h3>Create Declarative Custom Report</h3>
            <label>
              Report Name
              <input
                type="text"
                required
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Monthly Sales Register"
              />
            </label>
            <label>
              Description
              <input
                type="text"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>
            <label>
              Data Source (Allowlisted)
              <select
                value={dataSource}
                onChange={(e) => {
                  const ds = e.target.value as CustomReportDataSource;
                  setDataSource(ds);
                  setColumns(DATA_SOURCE_COLUMNS[ds].slice(0, 3));
                  setSortBy(DATA_SOURCE_COLUMNS[ds][0]);
                }}
              >
                {Object.keys(DATA_SOURCE_COLUMNS).map((ds) => (
                  <option key={ds} value={ds}>
                    {ds}
                  </option>
                ))}
              </select>
            </label>
            <div className="ot-crpt-cols-box">
              <span>Columns:</span>
              <div className="ot-crpt-cols-grid">
                {DATA_SOURCE_COLUMNS[dataSource].map((col) => (
                  <label key={col} className="ot-crpt-col-chk">
                    <input
                      type="checkbox"
                      checked={columns.includes(col)}
                      onChange={() => handleToggleColumn(col)}
                    />
                    {col}
                  </label>
                ))}
              </div>
            </div>
            <div className="ot-crpt-sort-row">
              <label>
                Sort By
                <select value={sortBy} onChange={(e) => setSortBy(e.target.value)}>
                  {DATA_SOURCE_COLUMNS[dataSource].map((col) => (
                    <option key={col} value={col}>
                      {col}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Direction
                <select
                  value={sortDir}
                  onChange={(e) => setSortDir(e.target.value as 'ASC' | 'DESC')}
                >
                  <option value="ASC">ASC</option>
                  <option value="DESC">DESC</option>
                </select>
              </label>
            </div>
            <div className="ot-crpt-modal-actions">
              <button
                type="button"
                className="ot-btn ot-btn-secondary"
                onClick={() => setShowBuilder(false)}
              >
                Cancel (Esc)
              </button>
              <button type="submit" className="ot-btn ot-btn-primary">
                Save Report
              </button>
            </div>
          </form>
        </div>
      )}

      {showImport && (
        <div className="ot-crpt-modal-backdrop">
          <form className="ot-crpt-modal" onSubmit={handleImportTemplate}>
            <h3>Import .opentally-report.json Template</h3>
            <textarea
              rows={8}
              required
              value={importJson}
              onChange={(e) => setImportJson(e.target.value)}
              placeholder='{"format":"opentally-custom-report","version":"3.0.0",...}'
            />
            <div className="ot-crpt-modal-actions">
              <button
                type="button"
                className="ot-btn ot-btn-secondary"
                onClick={() => setShowImport(false)}
              >
                Cancel (Esc)
              </button>
              <button type="submit" className="ot-btn ot-btn-primary">
                Import Template
              </button>
            </div>
          </form>
        </div>
      )}
    </div>
  );
};
