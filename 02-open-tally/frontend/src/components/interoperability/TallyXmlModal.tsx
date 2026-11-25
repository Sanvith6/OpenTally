import React, { useState, useEffect } from 'react';
import { Company, TallyXmlImportPreview } from '../../types';
import {
  exportCompanyTallyXml,
  previewTallyXmlImport,
  executeTallyXmlImport,
} from '../../services/api';
import './TallyXmlModal.css';

interface Props {
  isOpen: boolean;
  onClose: () => void;
  activeCompany: Company | null;
  onImportSuccess?: () => void;
}

export const TallyXmlModal: React.FC<Props> = ({
  isOpen,
  onClose,
  activeCompany,
  onImportSuccess,
}) => {
  const [activeTab, setActiveTab] = useState<'import' | 'export'>('import');

  // Import states
  const [xmlContent, setXmlContent] = useState('');
  const [selectedFileName, setSelectedFileName] = useState('');
  const [preview, setPreview] = useState<TallyXmlImportPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importSuccessMsg, setImportSuccessMsg] = useState<string | null>(null);

  // Export states
  const [fromDate, setFromDate] = useState('2026-04-01');
  const [toDate, setToDate] = useState('2027-03-31');
  const [exporting, setExporting] = useState(false);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  // Keyboard shortcut: Esc closes modal
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;
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

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    setSelectedFileName(file.name);
    setImportError(null);
    setImportSuccessMsg(null);
    setPreviewLoading(true);

    const reader = new FileReader();
    reader.onload = async (event) => {
      const text = event.target?.result as string;
      setXmlContent(text);
      try {
        const p = await previewTallyXmlImport(text);
        setPreview(p);
      } catch (err) {
        setImportError(err instanceof Error ? err.message : String(err));
        setPreview(null);
      } finally {
        setPreviewLoading(false);
      }
    };
    reader.onerror = () => {
      setImportError('Failed to read selected XML file.');
      setPreviewLoading(false);
    };
    reader.readAsText(file);
  };

  const handleConfirmImport = async () => {
    if (!xmlContent || !preview || !preview.can_import) return;

    setImporting(true);
    setImportError(null);
    setImportSuccessMsg(null);

    try {
      const result = await executeTallyXmlImport(xmlContent);
      if (result.success) {
        setImportSuccessMsg(result.message);
        if (onImportSuccess) {
          onImportSuccess();
        }
      } else {
        setImportError(result.message);
      }
    } catch (err) {
      setImportError(err instanceof Error ? err.message : String(err));
    } finally {
      setImporting(false);
    }
  };

  const handleExportXml = async () => {
    if (!activeCompany) return;
    setExporting(true);
    setExportNotice(null);
    setExportError(null);

    try {
      const xml = await exportCompanyTallyXml(fromDate, toDate);
      const blob = new Blob([xml], { type: 'application/xml;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `Tally_${activeCompany.name.replace(/\s+/g, '_')}_${fromDate}_to_${toDate}.xml`;
      a.click();
      URL.revokeObjectURL(url);
      setExportNotice('Tally XML export downloaded successfully.');
    } catch (err) {
      setExportError(err instanceof Error ? err.message : String(err));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="ot-modal-backdrop" onClick={onClose}>
      <div
        className="ot-modal-content ot-xml-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="xml-modal-title"
      >
        <div className="ot-modal-header">
          <h2 id="xml-modal-title">Tally XML Interoperability</h2>
          <button className="ot-modal-close-btn" onClick={onClose} aria-label="Close modal">
            &times;
          </button>
        </div>

        <div className="ot-xml-tabs">
          <button
            className={`ot-xml-tab-btn ${activeTab === 'import' ? 'active' : ''}`}
            onClick={() => setActiveTab('import')}
          >
            Import Tally XML
          </button>
          <button
            className={`ot-xml-tab-btn ${activeTab === 'export' ? 'active' : ''}`}
            onClick={() => setActiveTab('export')}
          >
            Export Tally XML
          </button>
        </div>

        {activeTab === 'import' && (
          <div>
            <div className="ot-xml-section">
              <h3 style={{ marginTop: 0, marginBottom: '8px', fontSize: '15px' }}>
                Upload & Dry-Run Preview
              </h3>
              <p style={{ fontSize: '13px', color: '#94a3b8', margin: '0 0 12px 0' }}>
                Select a standard Tally XML export file. OpenTally will parse, validate double-entry
                equilibrium, and simulate dependency resolution before touching the database.
              </p>

              <label className="ot-file-input-label" style={{ display: 'inline-block' }}>
                <input
                  type="file"
                  accept=".xml"
                  onChange={handleFileChange}
                  style={{ display: 'none' }}
                />
                <span className="ot-secondary-btn" style={{ cursor: 'pointer', display: 'inline-block' }}>
                  {selectedFileName ? `File: ${selectedFileName}` : 'Choose XML File…'}
                </span>
              </label>

              {previewLoading && (
                <div style={{ marginTop: '12px', color: '#38bdf8', fontSize: '13px' }}>
                  Parsing and validating Tally XML structure…
                </div>
              )}
            </div>

            {preview && (
              <div className="ot-xml-preview-card">
                <h4 style={{ margin: '0 0 10px 0', fontSize: '14px', color: '#f8fafc' }}>
                  Dry-Run Import Analysis
                </h4>

                <div className="ot-xml-stat-grid">
                  <div className="ot-xml-stat-box">
                    <div className="ot-xml-stat-title">Groups</div>
                    <div className="ot-xml-stat-num">{preview.groups_to_create}</div>
                    <div className="ot-xml-stat-sub">{preview.groups_skipped} existing</div>
                  </div>
                  <div className="ot-xml-stat-box">
                    <div className="ot-xml-stat-title">Ledgers</div>
                    <div className="ot-xml-stat-num">{preview.ledgers_to_create}</div>
                    <div className="ot-xml-stat-sub">{preview.ledgers_skipped} existing</div>
                  </div>
                  <div className="ot-xml-stat-box">
                    <div className="ot-xml-stat-title">Stock Items</div>
                    <div className="ot-xml-stat-num">{preview.stock_items_to_create}</div>
                    <div className="ot-xml-stat-sub">{preview.stock_items_skipped} existing</div>
                  </div>
                  <div className="ot-xml-stat-box">
                    <div className="ot-xml-stat-title">Vouchers</div>
                    <div className="ot-xml-stat-num">{preview.vouchers_to_create}</div>
                    <div className="ot-xml-stat-sub">{preview.vouchers_skipped} existing</div>
                  </div>
                </div>

                {preview.errors.length > 0 && (
                  <div style={{ marginTop: '14px' }}>
                    <div style={{ fontSize: '13px', fontWeight: 600, color: '#fca5a5', marginBottom: '6px' }}>
                      Validation Findings ({preview.errors.length}):
                    </div>
                    <table className="ot-xml-error-table">
                      <thead>
                        <tr>
                          <th>Severity</th>
                          <th>Type</th>
                          <th>Identifier</th>
                          <th>Message</th>
                        </tr>
                      </thead>
                      <tbody>
                        {preview.errors.map((err, idx) => (
                          <tr key={idx}>
                            <td>
                              <span className={err.severity === 'FATAL' ? 'badge-fatal' : 'badge-warning'}>
                                {err.severity}
                              </span>
                            </td>
                            <td>{err.item_type}</td>
                            <td>{err.identifier}</td>
                            <td>{err.message}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}

                <div style={{ marginTop: '16px', display: 'flex', gap: '10px', alignItems: 'center' }}>
                  <button
                    className="ot-primary-btn"
                    onClick={handleConfirmImport}
                    disabled={!preview.can_import || importing}
                  >
                    {importing ? 'Importing Atoms…' : 'Confirm Atomic Import'}
                  </button>
                  {!preview.can_import && (
                    <span style={{ fontSize: '12px', color: '#fca5a5' }}>
                      Fix blocking fatal errors before importing.
                    </span>
                  )}
                </div>
              </div>
            )}

            {importSuccessMsg && (
              <div className="ot-report-success" style={{ marginTop: '14px' }}>
                {importSuccessMsg}
              </div>
            )}

            {importError && (
              <div className="ot-report-error" style={{ marginTop: '14px' }}>
                {importError}
              </div>
            )}
          </div>
        )}

        {activeTab === 'export' && (
          <div>
            <div className="ot-xml-section">
              <h3 style={{ marginTop: 0, marginBottom: '8px', fontSize: '15px' }}>
                Export Company Data (Tally XML)
              </h3>
              <p style={{ fontSize: '13px', color: '#94a3b8', margin: '0 0 16px 0' }}>
                Generates a pure-Rust canonical XML envelope containing account groups, chart of
                accounts ledgers, units, godowns, inventory items, and balanced vouchers for export to
                external Tally-compatible systems.
              </p>

              <div style={{ display: 'flex', gap: '12px', marginBottom: '16px' }}>
                <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '13px' }}>
                  <span>From Date:</span>
                  <input
                    type="date"
                    value={fromDate}
                    onChange={(e) => setFromDate(e.target.value)}
                    style={{ padding: '6px 8px', borderRadius: '4px', border: '1px solid #334155' }}
                  />
                </label>
                <label style={{ display: 'flex', flexDirection: 'column', gap: '4px', fontSize: '13px' }}>
                  <span>To Date:</span>
                  <input
                    type="date"
                    value={toDate}
                    onChange={(e) => setToDate(e.target.value)}
                    style={{ padding: '6px 8px', borderRadius: '4px', border: '1px solid #334155' }}
                  />
                </label>
              </div>

              <button
                className="ot-primary-btn"
                onClick={handleExportXml}
                disabled={exporting}
              >
                {exporting ? 'Generating XML…' : 'Export Tally XML'}
              </button>

              {exportNotice && (
                <div className="ot-report-success" style={{ marginTop: '14px' }}>
                  {exportNotice}
                </div>
              )}

              {exportError && (
                <div className="ot-report-error" style={{ marginTop: '14px' }}>
                  {exportError}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
