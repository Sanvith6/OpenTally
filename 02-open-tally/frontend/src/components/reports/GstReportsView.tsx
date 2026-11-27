import React, { useCallback, useEffect, useState } from 'react';
import {
  Company,
  Gstr1SummaryReport,
  Gstr3bSummaryReport,
  HsnSacCode,
  GstLedgerMapping,
  Gstr2bReconciliationReport,
  Cmp08QuarterlySummary,
  GstnValidationReport,
} from '../../types';
import {
  getCompanyGstProfile,
  getGstr1Summary,
  getGstr3bSummary,
  listHsnSacCodes,
  saveCompanyGstProfile,
  createHsnSacCode,
  getGstLedgerMapping,
  exportGstr1Json,
  exportGstr3bJson,
  getGstr2bReconciliationReport,
  importAndReconcileGstr2b,
  exportGstr2bReconciliationCsv,
  getCompanyGspConfig,
  saveCompanyGspConfig,
  validateGstr1Export,
  getCmp08QuarterlySummary,
} from '../../services/api';
import './Reports.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
  onViewVoucher?: (voucherId: string) => void;
  initialTab?: 'gstr1' | 'gstr3b' | 'gstr2b' | 'settings';
}

const getMonthStart = () => {
  const d = new Date();
  return new Date(d.getFullYear(), d.getMonth(), 1).toISOString().slice(0, 10);
};

const getToday = () => new Date().toISOString().slice(0, 10);

const getCurrentPeriod = () => {
  const d = new Date();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const y = String(d.getFullYear());
  return `${m}${y}`;
};

export function GstReportsView({ activeCompany, onBack, onViewVoucher, initialTab }: Props) {
  const [tab, setTab] = useState<'gstr1' | 'gstr3b' | 'gstr2b' | 'cmp08' | 'settings'>(initialTab || 'gstr1');
  const [cmp08, setCmp08] = useState<Cmp08QuarterlySummary | null>(null);
  const [cmp08Year, setCmp08Year] = useState<number>(2026);
  const [cmp08Quarter, setCmp08Quarter] = useState<number>(1);
  const [gstnValidation, setGstnValidation] = useState<GstnValidationReport | null>(null);
  const [fromDate, setFromDate] = useState(getMonthStart());
  const [toDate, setToDate] = useState(getToday());
  const [returnPeriod, setReturnPeriod] = useState(getCurrentPeriod());

  const [gstr1, setGstr1] = useState<Gstr1SummaryReport | null>(null);
  const [gstr3b, setGstr3b] = useState<Gstr3bSummaryReport | null>(null);
  const [gstr2b, setGstr2b] = useState<Gstr2bReconciliationReport | null>(null);
  const [gstr2bFilter, setGstr2bFilter] = useState<'All' | 'Matched' | 'Mismatch' | 'MissingInBooks' | 'MissingIn2B'>('All');
  const [hsnCodes, setHsnCodes] = useState<HsnSacCode[]>([]);
  const [ledgerMapping, setLedgerMapping] = useState<GstLedgerMapping | null>(null);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // Settings form states
  const [gstinInput, setGstinInput] = useState('');
  const [stateCodeInput, setStateCodeInput] = useState('27');
  const [regTypeInput, setRegTypeInput] = useState('Regular');
  const [gstEnabledInput, setGstEnabledInput] = useState(true);

  // New HSN Form states
  const [newHsnCode, setNewHsnCode] = useState('');
  const [newHsnType, setNewHsnType] = useState<'HSN' | 'SAC'>('HSN');
  const [newHsnDesc, setNewHsnDesc] = useState('');
  const [newHsnRate, setNewHsnRate] = useState('18.00');
  const [newHsnCess, setNewHsnCess] = useState('0.00');

  // BYO-GSP / IRP Connector states
  const [gspMode, setGspMode] = useState<'Sandbox' | 'HttpGsp'>('Sandbox');
  const [gspName, setGspName] = useState('Deterministic IRP Sandbox');
  const [gspBaseUrl, setGspBaseUrl] = useState('');
  const [gspClientId, setGspClientId] = useState('');
  const [gspSecretRef, setGspSecretRef] = useState('');
  const [gspEnabled, setGspEnabled] = useState(true);
  const [gspHasSecret, setGspHasSecret] = useState(false);
  const [savingGsp, setSavingGsp] = useState(false);

  const loadData = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      if (tab === 'gstr1') {
        const data = await getGstr1Summary(fromDate, toDate);
        setGstr1(data);
      } else if (tab === 'gstr3b') {
        const data = await getGstr3bSummary(fromDate, toDate);
        setGstr3b(data);
      } else if (tab === 'gstr2b') {
        const data = await getGstr2bReconciliationReport(returnPeriod);
        setGstr2b(data);
      } else if (tab === 'cmp08') {
        if (typeof getCmp08QuarterlySummary === 'function') {
          const data = await getCmp08QuarterlySummary(cmp08Year, cmp08Quarter);
          setCmp08(data);
        }
      } else if (tab === 'settings') {
        const [profile, codes, mapping, gspCfg] = await Promise.all([
          getCompanyGstProfile(),
          listHsnSacCodes(),
          getGstLedgerMapping(),
          typeof getCompanyGspConfig === 'function'
            ? getCompanyGspConfig().catch(() => null)
            : Promise.resolve(null),
        ]);
        setHsnCodes(codes);
        setLedgerMapping(mapping);
        if (profile) {
          setGstinInput(profile.gstin || '');
          setStateCodeInput(profile.state_code);
          setRegTypeInput(profile.registration_type);
          setGstEnabledInput(profile.gst_enabled === 1 || profile.gst_enabled === true);
        }
        if (gspCfg) {
          setGspMode(gspCfg.provider_mode);
          setGspName(gspCfg.provider_name);
          setGspBaseUrl(gspCfg.base_url || '');
          setGspClientId(gspCfg.client_id || '');
          setGspEnabled(gspCfg.is_enabled);
          setGspHasSecret(gspCfg.has_api_secret);
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, tab, fromDate, toDate, returnPeriod]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const handleUploadGstr2b = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setError(null);
    setNotice(null);
    setLoading(true);
    try {
      if (file.size > 25 * 1024 * 1024) {
        throw new Error('GSTR-2B file exceeds 25MB limit');
      }
      const text = await file.text();
      const report = await importAndReconcileGstr2b(returnPeriod, text);
      setGstr2b(report);
      setNotice(
        `GSTR-2B imported and reconciled successfully (${report.summary.matched_count} matched, ${report.summary.mismatch_count} mismatches, ${report.summary.missing_in_books_count} missing in books).`
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
      e.target.value = '';
    }
  };

  const handleExportGstr2bCsv = async () => {
    setError(null);
    try {
      const csvStr = await exportGstr2bReconciliationCsv(returnPeriod);
      const blob = new Blob([csvStr], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `GSTR2B_Reconciliation_${returnPeriod}.csv`;
      a.click();
      URL.revokeObjectURL(url);
      setNotice('GSTR-2B reconciliation CSV exported successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setError(null);
    setNotice(null);
    try {
      await saveCompanyGstProfile(
        gstinInput.trim() || null,
        stateCodeInput.trim(),
        regTypeInput,
        gstEnabledInput
      );
      setNotice('Company GST Profile updated successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCreateHsn = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setError(null);
    setNotice(null);
    try {
      const created = await createHsnSacCode(
        newHsnCode.trim(),
        newHsnDesc.trim() || null,
        newHsnType,
        newHsnRate.trim(),
        newHsnCess.trim(),
        fromDate
      );
      setHsnCodes((prev) => [created, ...prev]);
      setNotice(`HSN/SAC Code '${created.code}' registered.`);
      setNewHsnCode('');
      setNewHsnDesc('');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSaveGsp = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!activeCompany) return;
    setError(null);
    setNotice(null);
    setSavingGsp(true);
    try {
      const saved = await saveCompanyGspConfig({
        provider_mode: gspMode,
        provider_name: gspName.trim() || 'Deterministic IRP Sandbox',
        base_url: gspBaseUrl.trim() || undefined,
        client_id: gspClientId.trim() || undefined,
        api_secret_ref: gspSecretRef.trim() || undefined,
        is_enabled: gspEnabled,
      });
      setGspHasSecret(saved.has_api_secret);
      setGspSecretRef('');
      setNotice(`GSP / IRP connector saved (${saved.provider_mode}: ${saved.provider_name}).`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingGsp(false);
    }
  };

  return (
    <section className="ot-report-screen" data-testid="gst-reports-view">
      <header className="ot-report-header">
        <div>
          <h2>GST Statutory Reports & Configuration</h2>
          <p>
            GSTR-1 outward supplies, GSTR-3B return summary, and tax masters for{' '}
            {activeCompany?.name || 'the active company'}.
          </p>
        </div>
        <button className="ot-report-back" onClick={onBack}>
          Back to Gateway
        </button>
      </header>

      <div className="ot-report-card">
        {/* Navigation Tabs */}
        <div className="ot-report-tabs" role="tablist">
          <button
            className={`ot-tab-btn ${tab === 'gstr1' ? 'active' : ''}`}
            onClick={() => setTab('gstr1')}
            role="tab"
            aria-selected={tab === 'gstr1'}
          >
            GSTR-1 (Outward Supplies)
          </button>
          <button
            className={`ot-tab-btn ${tab === 'gstr3b' ? 'active' : ''}`}
            onClick={() => setTab('gstr3b')}
            role="tab"
            aria-selected={tab === 'gstr3b'}
          >
            GSTR-3B (Monthly Summary)
          </button>
          <button
            className={`ot-tab-btn ${tab === 'gstr2b' ? 'active' : ''}`}
            onClick={() => setTab('gstr2b')}
            role="tab"
            aria-selected={tab === 'gstr2b'}
          >
            GSTR-2B (ITC Reconciliation)
          </button>
          <button
            className={`ot-tab-btn ${tab === 'cmp08' ? 'active' : ''}`}
            onClick={() => setTab('cmp08')}
            role="tab"
            aria-selected={tab === 'cmp08'}
          >
            CMP-08 Quarterly
          </button>
          <button
            className={`ot-tab-btn ${tab === 'settings' ? 'active' : ''}`}
            onClick={() => setTab('settings')}
            role="tab"
            aria-selected={tab === 'settings'}
          >
            GST Settings & Masters
          </button>
        </div>

        {/* Date Filter Bar for GSTR-1 & GSTR-3B */}
        {(tab === 'gstr1' || tab === 'gstr3b') && (
          <div className="ot-inventory-toolbar" style={{ marginTop: '1rem' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
              <span>From:</span>
              <input
                type="date"
                aria-label="From date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
              />
            </label>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
              <span>To:</span>
              <input
                type="date"
                aria-label="To date"
                value={toDate}
                onChange={(e) => setToDate(e.target.value)}
              />
            </label>
            <button onClick={() => void loadData()} disabled={loading}>
              {loading ? 'Refreshing…' : 'Refresh'}
            </button>
            {tab === 'gstr1' && (
              <>
                <button
                  type="button"
                  onClick={async () => {
                    try {
                      const rep = await validateGstr1Export(fromDate, toDate);
                      setGstnValidation(rep);
                    } catch (err) {
                      setError(err instanceof Error ? err.message : String(err));
                    }
                  }}
                >
                  Validate for GSTN Portal
                </button>
                {gstnValidation && (
                  <span
                    className={`ot-badge ${gstnValidation.is_valid ? 'badge-credit' : 'badge-debit'}`}
                    data-testid="gstn-readiness-badge"
                  >
                    {gstnValidation.is_valid
                      ? `GSTN Ready (0 Errors, ${gstnValidation.warning_count} Warnings)`
                      : `${gstnValidation.error_count} Schema Errors`}
                  </span>
                )}
              </>
            )}
            {tab === 'gstr1' && (
              <button
                type="button"
                className="ot-primary-btn"
                onClick={async () => {
                  try {
                    const jsonStr = await exportGstr1Json(fromDate, toDate);
                    const blob = new Blob([jsonStr], { type: 'application/json' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `GSTR1_${fromDate}_to_${toDate}.json`;
                    a.click();
                    URL.revokeObjectURL(url);
                    setNotice('GSTR-1 JSON downloaded successfully.');
                  } catch (err) {
                    setError(err instanceof Error ? err.message : String(err));
                  }
                }}
                style={{ marginLeft: 'auto' }}
              >
                Export GSTR-1 JSON
              </button>
            )}
            {tab === 'gstr3b' && (
              <button
                type="button"
                className="ot-primary-btn"
                onClick={async () => {
                  try {
                    const jsonStr = await exportGstr3bJson(fromDate, toDate);
                    const blob = new Blob([jsonStr], { type: 'application/json' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `GSTR3B_${fromDate}_to_${toDate}.json`;
                    a.click();
                    URL.revokeObjectURL(url);
                    setNotice('GSTR-3B JSON downloaded successfully.');
                  } catch (err) {
                    setError(err instanceof Error ? err.message : String(err));
                  }
                }}
                style={{ marginLeft: 'auto' }}
              >
                Export GSTR-3B JSON
              </button>
            )}
          </div>
        )}

        {/* Return Period & Upload Bar for GSTR-2B */}
        {tab === 'gstr2b' && (
          <div className="ot-inventory-toolbar" style={{ marginTop: '1rem', display: 'flex', gap: '1rem', alignItems: 'center', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
              <span>Period (MMYYYY):</span>
              <input
                type="text"
                maxLength={6}
                style={{ width: '90px' }}
                value={returnPeriod}
                onChange={(e) => setReturnPeriod(e.target.value)}
                placeholder="MMYYYY"
                aria-label="Return period"
              />
            </label>
            <button onClick={() => void loadData()} disabled={loading}>
              {loading ? 'Refreshing…' : 'Fetch'}
            </button>
            <label className="ot-primary-btn" style={{ cursor: 'pointer', margin: 0, padding: '0.4rem 0.8rem' }}>
              Upload GSTR-2B JSON
              <input
                type="file"
                accept=".json,application/json"
                style={{ display: 'none' }}
                aria-label="Upload GSTR-2B JSON"
                onChange={handleUploadGstr2b}
              />
            </label>
            <button
              type="button"
              className="ot-btn"
              onClick={handleExportGstr2bCsv}
              style={{ marginLeft: 'auto' }}
            >
              Export Reconciliation CSV
            </button>
          </div>
        )}

        {notice && (
          <div className="ot-report-success" role="status" style={{ marginTop: '1rem' }}>
            {notice}
          </div>
        )}
        {error && (
          <div className="ot-report-error" role="alert" style={{ marginTop: '1rem' }}>
            {error}
          </div>
        )}

        {loading && <div className="ot-report-loading" style={{ margin: '1.5rem 0' }}>Loading GST data…</div>}

        {/* -------------------- GSTR-1 TAB -------------------- */}
        {!loading && tab === 'gstr1' && gstr1 && (
          <div data-testid="gstr1-view" style={{ marginTop: '1.5rem' }}>
            {/* KPI Cards */}
            <div className="ot-report-summary">
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total Taxable Value</span>
                <span className="ot-summary-val">₹ {gstr1.total_taxable_value}</span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total CGST</span>
                <span className="ot-summary-val">₹ {gstr1.total_cgst}</span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total SGST</span>
                <span className="ot-summary-val">₹ {gstr1.total_sgst}</span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total IGST</span>
                <span className="ot-summary-val">₹ {gstr1.total_igst}</span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total Cess</span>
                <span className="ot-summary-val">₹ {gstr1.total_cess}</span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total Invoice Value</span>
                <span className="ot-summary-val" style={{ color: '#097969', fontWeight: 'bold' }}>
                  ₹ {gstr1.total_invoice_value}
                </span>
              </div>
            </div>

            {/* Table 4: B2B Invoices */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 4: B2B Invoices (Supplies to Registered Persons)</h3>
              {gstr1.b2b_invoices.length === 0 ? (
                <div className="ot-report-empty">No B2B outward invoices recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>Voucher #</th>
                      <th>Date</th>
                      <th>Party Name</th>
                      <th>GSTIN</th>
                      <th>POS</th>
                      <th>Type</th>
                      <th>Taxable Value (₹)</th>
                      <th>CGST (₹)</th>
                      <th>SGST (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Cess (₹)</th>
                      <th>Invoice Total (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.b2b_invoices.map((inv) => (
                      <tr key={inv.voucher_id}>
                        <td>
                          {onViewVoucher ? (
                            <button
                              type="button"
                              className="ot-drilldown-btn"
                              style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold' }}
                              onClick={() => onViewVoucher(inv.voucher_id)}
                              title="View Voucher Details"
                            >
                              {inv.voucher_number}
                            </button>
                          ) : (
                            <strong>{inv.voucher_number}</strong>
                          )}
                        </td>
                        <td>{inv.voucher_date}</td>
                        <td>{inv.party_name}</td>
                        <td><code>{inv.party_gstin}</code></td>
                        <td>{inv.place_of_supply_state_code}</td>
                        <td>{inv.supply_type}</td>
                        <td style={{ textAlign: 'right' }}>{inv.taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{inv.cgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{inv.sgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{inv.igst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{inv.cess_amount}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{inv.invoice_value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Table 5: B2C Large Invoices */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 5: B2C (Large) Invoices (Inter-State Unregistered &gt; ₹2,50,000)</h3>
              {gstr1.b2c_large_invoices.length === 0 ? (
                <div className="ot-report-empty">No B2C Large outward invoices recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>Voucher #</th>
                      <th>Date</th>
                      <th>Party Name</th>
                      <th>POS State</th>
                      <th>Taxable Value (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Cess (₹)</th>
                      <th>Invoice Value (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.b2c_large_invoices.map((inv) => (
                      <tr key={inv.voucher_id}>
                        <td>
                          {onViewVoucher ? (
                            <button
                              type="button"
                              className="ot-drilldown-btn"
                              style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold' }}
                              onClick={() => onViewVoucher(inv.voucher_id)}
                              title="View Voucher Details"
                            >
                              {inv.voucher_number}
                            </button>
                          ) : (
                            <strong>{inv.voucher_number}</strong>
                          )}
                        </td>
                        <td>{inv.voucher_date}</td>
                        <td>{inv.party_name}</td>
                        <td>{inv.place_of_supply_state_code}</td>
                        <td style={{ textAlign: 'right' }}>{inv.taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{inv.igst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{inv.cess_amount}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{inv.invoice_value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Table 7: B2C Small Summary */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 7: B2C (Small) Summary (Intra-State + Inter-State &le; ₹2,50,000)</h3>
              {gstr1.b2c_small_summary.length === 0 ? (
                <div className="ot-report-empty">No B2C Small outward supplies recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>POS State</th>
                      <th>Supply Type</th>
                      <th>GST Rate (%)</th>
                      <th>Taxable Value (₹)</th>
                      <th>CGST (₹)</th>
                      <th>SGST (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Cess (₹)</th>
                      <th>Total Tax (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.b2c_small_summary.map((row, idx) => (
                      <tr key={idx}>
                        <td>{row.place_of_supply_state_code}</td>
                        <td>{row.supply_type}</td>
                        <td>{row.gst_rate}%</td>
                        <td style={{ textAlign: 'right' }}>{row.taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{row.cgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.sgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.igst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.cess_amount}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{row.total_tax}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Table 9B: Credit / Debit Notes (Registered - CDNR) */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 9B: Credit / Debit Notes (Registered Persons - CDNR)</h3>
              {(!gstr1.cdnr_table_9b || gstr1.cdnr_table_9b.length === 0) ? (
                <div className="ot-report-empty">No registered Credit/Debit Notes recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>Note #</th>
                      <th>Note Date</th>
                      <th>Type</th>
                      <th>Original Inv #</th>
                      <th>Party Name</th>
                      <th>GSTIN</th>
                      <th>Taxable Value (₹)</th>
                      <th>CGST (₹)</th>
                      <th>SGST (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Total Note Value (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.cdnr_table_9b.map((row) => (
                      <tr key={row.note_voucher_id}>
                        <td>
                          {onViewVoucher ? (
                            <button
                              type="button"
                              className="ot-drilldown-btn"
                              style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold' }}
                              onClick={() => onViewVoucher(row.note_voucher_id)}
                              title="View Note Details"
                            >
                              {row.note_voucher_number}
                            </button>
                          ) : (
                            <strong>{row.note_voucher_number}</strong>
                          )}
                        </td>
                        <td>{row.note_date}</td>
                        <td><span className={`ot-badge ${row.note_type === 'C' ? 'badge-credit' : 'badge-debit'}`}>{row.note_type === 'C' ? 'Credit Note' : 'Debit Note'}</span></td>
                        <td>{row.original_invoice_number}</td>
                        <td>{row.party_name}</td>
                        <td><code>{row.party_gstin}</code></td>
                        <td style={{ textAlign: 'right' }}>{row.taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{row.cgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.sgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.igst_amount}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{row.note_value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Table 9B: Credit / Debit Notes (Unregistered - CDNUR) */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 9B: Credit / Debit Notes (Unregistered Persons - CDNUR)</h3>
              {(!gstr1.cdnur_table_9b || gstr1.cdnur_table_9b.length === 0) ? (
                <div className="ot-report-empty">No unregistered Credit/Debit Notes recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>Note #</th>
                      <th>Note Date</th>
                      <th>Type</th>
                      <th>Original Inv #</th>
                      <th>Party Name</th>
                      <th>POS State</th>
                      <th>Taxable Value (₹)</th>
                      <th>CGST (₹)</th>
                      <th>SGST (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Total Note Value (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.cdnur_table_9b.map((row) => (
                      <tr key={row.note_voucher_id}>
                        <td>
                          {onViewVoucher ? (
                            <button
                              type="button"
                              className="ot-drilldown-btn"
                              style={{ background: 'none', border: 'none', color: '#38bdf8', textDecoration: 'underline', cursor: 'pointer', padding: 0, font: 'inherit', fontWeight: 'bold' }}
                              onClick={() => onViewVoucher(row.note_voucher_id)}
                              title="View Note Details"
                            >
                              {row.note_voucher_number}
                            </button>
                          ) : (
                            <strong>{row.note_voucher_number}</strong>
                          )}
                        </td>
                        <td>{row.note_date}</td>
                        <td><span className={`ot-badge ${row.note_type === 'C' ? 'badge-credit' : 'badge-debit'}`}>{row.note_type === 'C' ? 'Credit Note' : 'Debit Note'}</span></td>
                        <td>{row.original_invoice_number}</td>
                        <td>{row.party_name}</td>
                        <td>{row.place_of_supply_state_code}</td>
                        <td style={{ textAlign: 'right' }}>{row.taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{row.cgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.sgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.igst_amount}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{row.note_value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* Table 12: HSN Summary */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 12: HSN / SAC Summary</h3>
              {gstr1.hsn_summary.length === 0 ? (
                <div className="ot-report-empty">No HSN summary data recorded for this period.</div>
              ) : (
                <table className="ot-report-table">
                  <thead>
                    <tr>
                      <th>HSN/SAC Code</th>
                      <th>Taxable Value (₹)</th>
                      <th>CGST (₹)</th>
                      <th>SGST (₹)</th>
                      <th>IGST (₹)</th>
                      <th>Cess (₹)</th>
                      <th>Total Tax (₹)</th>
                      <th>Total Value (₹)</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr1.hsn_summary.map((row) => (
                      <tr key={row.hsn_sac_code}>
                        <td><code>{row.hsn_sac_code}</code></td>
                        <td style={{ textAlign: 'right' }}>{row.total_taxable_value}</td>
                        <td style={{ textAlign: 'right' }}>{row.cgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.sgst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.igst_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.cess_amount}</td>
                        <td style={{ textAlign: 'right' }}>{row.total_tax}</td>
                        <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{row.total_value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        )}

        {/* -------------------- GSTR-3B TAB -------------------- */}
        {!loading && tab === 'gstr3b' && gstr3b && (
          <div data-testid="gstr3b-view" style={{ marginTop: '1.5rem' }}>
            {/* KPI Cards */}
            <div className="ot-report-summary">
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total Outward Tax</span>
                <span className="ot-summary-val" style={{ color: '#d9534f', fontWeight: 'bold' }}>
                  ₹ {gstr3b.total_output_tax.total}
                </span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Total Eligible ITC</span>
                <span className="ot-summary-val" style={{ color: '#0275d8', fontWeight: 'bold' }}>
                  ₹ {gstr3b.total_eligible_itc.total}
                </span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Net Tax Payable (Cash)</span>
                <span className="ot-summary-val" style={{ color: '#d9534f', fontWeight: 'bold' }}>
                  ₹ {gstr3b.net_tax_payable.total}
                </span>
              </div>
              <div className="ot-summary-box">
                <span className="ot-summary-label">Closing ITC Balance (c/f)</span>
                <span className="ot-summary-val" style={{ color: '#5cb85c', fontWeight: 'bold' }}>
                  ₹ {gstr3b.closing_itc_balance.total}
                </span>
              </div>
            </div>

            {/* Table 3.1: Details of Outward Supplies and Inward Supplies Liable to Reverse Charge */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 3.1: Details of Outward Supplies and Inward Supplies Liable to Reverse Charge</h3>
              <table className="ot-report-table">
                <thead>
                  <tr>
                    <th>Nature of Supply</th>
                    <th>Total Taxable (₹)</th>
                    <th>IGST (₹)</th>
                    <th>CGST (₹)</th>
                    <th>SGST (₹)</th>
                    <th>Cess (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {gstr3b.table_3_1.map((row, idx) => (
                    <tr key={idx}>
                      <td><strong>{row.nature_of_supplies}</strong></td>
                      <td style={{ textAlign: 'right' }}>{row.total_taxable_value}</td>
                      <td style={{ textAlign: 'right' }}>{row.integrated_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.central_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.state_ut_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.cess}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            {/* Table 4: Eligible ITC */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 4: Eligible Input Tax Credit (ITC)</h3>
              <table className="ot-report-table">
                <thead>
                  <tr>
                    <th>ITC Category</th>
                    <th>IGST (₹)</th>
                    <th>CGST (₹)</th>
                    <th>SGST (₹)</th>
                    <th>Cess (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  {gstr3b.table_4_itc.map((row, idx) => (
                    <tr key={idx}>
                      <td><strong>{row.itc_details}</strong></td>
                      <td style={{ textAlign: 'right' }}>{row.integrated_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.central_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.state_ut_tax}</td>
                      <td style={{ textAlign: 'right' }}>{row.cess}</td>
                    </tr>
                  ))}
                  <tr style={{ background: '#f8f9fa' }}>
                    <td><strong>(C) Net ITC Available</strong></td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{gstr3b.total_eligible_itc.integrated_tax}</td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{gstr3b.total_eligible_itc.central_tax}</td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{gstr3b.total_eligible_itc.state_ut_tax}</td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold' }}>{gstr3b.total_eligible_itc.cess}</td>
                  </tr>
                </tbody>
              </table>
            </div>

            {/* Table 6.1: Payment of Tax & Balance */}
            <div style={{ marginTop: '2rem' }}>
              <h3>Table 6.1: Net Tax Position (Payable vs Carry Forward Balance)</h3>
              <table className="ot-report-table">
                <thead>
                  <tr>
                    <th>Description</th>
                    <th>IGST (₹)</th>
                    <th>CGST (₹)</th>
                    <th>SGST (₹)</th>
                    <th>Cess (₹)</th>
                    <th>Total (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td><strong>Net Tax Payable (Cash to Deposit)</strong></td>
                    <td style={{ textAlign: 'right', color: '#d9534f' }}>{gstr3b.net_tax_payable.integrated_tax}</td>
                    <td style={{ textAlign: 'right', color: '#d9534f' }}>{gstr3b.net_tax_payable.central_tax}</td>
                    <td style={{ textAlign: 'right', color: '#d9534f' }}>{gstr3b.net_tax_payable.state_ut_tax}</td>
                    <td style={{ textAlign: 'right', color: '#d9534f' }}>{gstr3b.net_tax_payable.cess}</td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold', color: '#d9534f' }}>
                      {gstr3b.net_tax_payable.total}
                    </td>
                  </tr>
                  <tr>
                    <td><strong>Closing ITC Balance (To Carry Forward)</strong></td>
                    <td style={{ textAlign: 'right', color: '#5cb85c' }}>{gstr3b.closing_itc_balance.integrated_tax}</td>
                    <td style={{ textAlign: 'right', color: '#5cb85c' }}>{gstr3b.closing_itc_balance.central_tax}</td>
                    <td style={{ textAlign: 'right', color: '#5cb85c' }}>{gstr3b.closing_itc_balance.state_ut_tax}</td>
                    <td style={{ textAlign: 'right', color: '#5cb85c' }}>{gstr3b.closing_itc_balance.cess}</td>
                    <td style={{ textAlign: 'right', fontWeight: 'bold', color: '#5cb85c' }}>
                      {gstr3b.closing_itc_balance.total}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        )}

        {/* -------------------- GSTR-2B TAB -------------------- */}
        {!loading && tab === 'gstr2b' && (
          <div data-testid="gstr2b-view" style={{ marginTop: '1.5rem' }}>
            {gstr2b ? (
              <>
                {/* KPI Cards */}
                <div className="ot-report-summary">
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">Total Portal Invoices</span>
                    <span className="ot-summary-val">{gstr2b.summary.total_portal_invoices}</span>
                  </div>
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">Matched ITC</span>
                    <span className="ot-summary-val" style={{ color: '#097969', fontWeight: 'bold' }}>
                      ₹ {gstr2b.summary.matched_itc}
                    </span>
                    <small style={{ color: '#666' }}>{gstr2b.summary.matched_count} Invoices</small>
                  </div>
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">Mismatch Variance</span>
                    <span className="ot-summary-val" style={{ color: '#d97706', fontWeight: 'bold' }}>
                      ₹ {gstr2b.summary.mismatch_itc}
                    </span>
                    <small style={{ color: '#666' }}>{gstr2b.summary.mismatch_count} Invoices</small>
                  </div>
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">Missing in Books</span>
                    <span className="ot-summary-val" style={{ color: '#b45309' }}>
                      ₹ {gstr2b.summary.missing_in_books_itc}
                    </span>
                    <small style={{ color: '#666' }}>{gstr2b.summary.missing_in_books_count} Invoices</small>
                  </div>
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">At-Risk ITC (Missing in 2B)</span>
                    <span className="ot-summary-val" style={{ color: '#dc2626', fontWeight: 'bold' }}>
                      ₹ {gstr2b.summary.at_risk_itc}
                    </span>
                    <small style={{ color: '#666' }}>{gstr2b.summary.missing_in_2b_count} Invoices</small>
                  </div>
                  <div className="ot-summary-box">
                    <span className="ot-summary-label">Total Eligible ITC</span>
                    <span className="ot-summary-val" style={{ color: '#097969', fontWeight: 'bold' }}>
                      ₹ {gstr2b.summary.total_eligible_itc}
                    </span>
                  </div>
                </div>

                {/* Filter Pills */}
                <div style={{ margin: '1.5rem 0 1rem 0', display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
                  {(['All', 'Matched', 'Mismatch', 'MissingInBooks', 'MissingIn2B'] as const).map((st) => (
                    <button
                      key={st}
                      type="button"
                      className={`ot-tab-btn ${gstr2bFilter === st ? 'active' : ''}`}
                      onClick={() => setGstr2bFilter(st)}
                      style={{ padding: '0.3rem 0.8rem', fontSize: '0.85rem' }}
                    >
                      {st === 'MissingInBooks' ? 'Missing in Books' : st === 'MissingIn2B' ? 'Missing in 2B (At-Risk)' : st}
                    </button>
                  ))}
                </div>

                {/* Table of Reconciled Invoices */}
                <table className="ot-report-table" data-testid="gstr2b-table">
                  <thead>
                    <tr>
                      <th>Status</th>
                      <th>Supplier GSTIN</th>
                      <th>Supplier Name</th>
                      <th>Invoice No</th>
                      <th>Invoice Date</th>
                      <th style={{ textAlign: 'right' }}>Portal Taxable (₹)</th>
                      <th style={{ textAlign: 'right' }}>Portal Tax (₹)</th>
                      <th style={{ textAlign: 'right' }}>Books Tax (₹)</th>
                      <th style={{ textAlign: 'right' }}>Tax Diff (₹)</th>
                      <th>ITC Avail</th>
                      <th>Matched Voucher</th>
                    </tr>
                  </thead>
                  <tbody>
                    {gstr2b.invoices
                      .filter((inv) => gstr2bFilter === 'All' || inv.reconciliation_status === gstr2bFilter)
                      .map((inv) => {
                        const badgeColor =
                          inv.reconciliation_status === 'Matched'
                            ? '#097969'
                            : inv.reconciliation_status === 'Mismatch'
                            ? '#d97706'
                            : inv.reconciliation_status === 'MissingInBooks'
                            ? '#b45309'
                            : '#dc2626';
                        return (
                          <tr key={inv.id}>
                            <td>
                              <span
                                style={{
                                  display: 'inline-block',
                                  padding: '0.2rem 0.5rem',
                                  borderRadius: '4px',
                                  color: '#fff',
                                  backgroundColor: badgeColor,
                                  fontSize: '0.75rem',
                                  fontWeight: 600,
                                }}
                              >
                                {inv.reconciliation_status}
                              </span>
                            </td>
                            <td><code>{inv.supplier_gstin}</code></td>
                            <td>{inv.supplier_name || '—'}</td>
                            <td><strong>{inv.invoice_number}</strong></td>
                            <td>{inv.invoice_date}</td>
                            <td style={{ textAlign: 'right' }}>{inv.taxable_value}</td>
                            <td style={{ textAlign: 'right', fontWeight: 600 }}>{inv.total_tax}</td>
                            <td style={{ textAlign: 'right' }}>{inv.books_total_tax || '—'}</td>
                            <td style={{ textAlign: 'right', color: inv.tax_difference !== '0.00' ? '#dc2626' : undefined }}>
                              {inv.tax_difference}
                            </td>
                            <td style={{ textAlign: 'center' }}>{inv.itc_availability}</td>
                            <td>
                              {inv.matched_voucher_id ? (
                                <button
                                  type="button"
                                  className="ot-link-btn"
                                  onClick={() => onViewVoucher?.(inv.matched_voucher_id!)}
                                  style={{ textDecoration: 'underline', background: 'none', border: 'none', color: '#097969', cursor: 'pointer', padding: 0 }}
                                >
                                  {inv.matched_voucher_number || inv.matched_voucher_id.slice(0, 8)}
                                </button>
                              ) : (
                                '—'
                              )}
                            </td>
                          </tr>
                        );
                      })}
                  </tbody>
                </table>
              </>
            ) : (
              <div className="ot-report-empty">
                No GSTR-2B data found for period {returnPeriod}. Upload government portal GSTR-2B JSON or click Fetch.
              </div>
            )}
          </div>
        )}

        {/* -------------------- CMP-08 TAB -------------------- */}
        {!loading && tab === 'cmp08' && (
          <div data-testid="cmp08-view" style={{ marginTop: '1.5rem' }}>
            <div className="ot-inventory-toolbar" style={{ marginBottom: '1rem' }}>
              <label>
                FY:
                <input
                  type="number"
                  aria-label="CMP-08 Financial Year"
                  value={cmp08Year}
                  onChange={(e) => setCmp08Year(Number(e.target.value) || 2026)}
                  style={{ width: '90px', marginLeft: '0.5rem' }}
                />
              </label>
              <label>
                Quarter:
                <select
                  aria-label="CMP-08 Quarter"
                  value={cmp08Quarter}
                  onChange={(e) => setCmp08Quarter(Number(e.target.value))}
                  style={{ marginLeft: '0.5rem' }}
                >
                  <option value={1}>Q1 (Apr-Jun)</option>
                  <option value={2}>Q2 (Jul-Sep)</option>
                  <option value={3}>Q3 (Oct-Dec)</option>
                  <option value={4}>Q4 (Jan-Mar)</option>
                </select>
              </label>
              <button type="button" onClick={() => void loadData()}>
                Refresh CMP-08
              </button>
            </div>
            {cmp08 && (
              <table className="ot-report-table">
                <thead>
                  <tr>
                    <th>CMP-08 Particulars</th>
                    <th data-numeric="true">Amount (₹)</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td>Outward Supplies Turnover ({cmp08.from_date} to {cmp08.to_date})</td>
                    <td data-numeric="true">{cmp08.outward_turnover}</td>
                  </tr>
                  <tr>
                    <td>Composition Tax Payable (@ {cmp08.composition_rate}%)</td>
                    <td data-numeric="true">{cmp08.composition_tax_payable}</td>
                  </tr>
                  <tr>
                    <td>Inward Supplies Liable to Reverse Charge (Taxable Value)</td>
                    <td data-numeric="true">{cmp08.rcm_taxable_value}</td>
                  </tr>
                  <tr>
                    <td>RCM Tax Payable</td>
                    <td data-numeric="true">{cmp08.rcm_tax_payable}</td>
                  </tr>
                  <tr className="ot-grand-total-row">
                    <td>Total CMP-08 Quarterly Liability</td>
                    <td data-numeric="true">{cmp08.total_cmp08_liability}</td>
                  </tr>
                </tbody>
              </table>
            )}
          </div>
        )}

        {/* -------------------- SETTINGS / MASTERS TAB -------------------- */}
        {!loading && tab === 'settings' && (
          <div data-testid="gst-settings-view" style={{ marginTop: '1.5rem' }}>
            {/* Company GST Profile Form */}
            <div className="ot-inventory-form" style={{ marginBottom: '2rem' }}>
              <h3>Company GST Profile</h3>
              <p className="ot-report-muted">
                Statutory registration details for {activeCompany?.name}.
              </p>
              <form onSubmit={handleSaveProfile}>
                <div className="ot-inventory-fields">
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>GSTIN (15 Alphanumeric Characters):</span>
                    <input
                      aria-label="GSTIN"
                      placeholder="e.g. 27AABCT3518Q1Z6"
                      value={gstinInput}
                      onChange={(e) => setGstinInput(e.target.value)}
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>State Code (2-digit GST state code):</span>
                    <input
                      aria-label="State Code"
                      placeholder="e.g. 27"
                      value={stateCodeInput}
                      onChange={(e) => setStateCodeInput(e.target.value)}
                      maxLength={2}
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Registration Type:</span>
                    <select
                      aria-label="Registration Type"
                      value={regTypeInput}
                      onChange={(e) => setRegTypeInput(e.target.value)}
                    >
                      <option value="Regular">Regular</option>
                      <option value="Composition">Composition</option>
                      <option value="Unregistered">Unregistered</option>
                    </select>
                  </label>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginTop: '1.5rem' }}>
                    <input
                      type="checkbox"
                      aria-label="GST Enabled"
                      checked={gstEnabledInput}
                      onChange={(e) => setGstEnabledInput(e.target.checked)}
                    />
                    <span>GST Enabled for Commercial Posting</span>
                  </label>
                </div>
                <button type="submit" style={{ marginTop: '1rem' }}>
                  Save GST Profile
                </button>
              </form>
            </div>

            {/* BYO-GSP / IRP Connector Settings */}
            <div className="ot-inventory-form" style={{ marginBottom: '2rem' }} data-testid="gsp-settings-card">
              <h3>BYO-GSP / IRP Connector (e-Invoice &amp; e-Way Bill)</h3>
              <p className="ot-report-muted">
                Configure offline deterministic Sandbox Simulator or live HTTPS GSP credentials.
              </p>
              <form onSubmit={handleSaveGsp}>
                <div className="ot-inventory-fields">
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Provider Mode:</span>
                    <select
                      aria-label="GSP Provider Mode"
                      value={gspMode}
                      onChange={(e) => setGspMode(e.target.value as 'Sandbox' | 'HttpGsp')}
                    >
                      <option value="Sandbox">Sandbox Simulator (Offline Deterministic)</option>
                      <option value="HttpGsp">BYO-GSP HTTPS Endpoint</option>
                    </select>
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Provider Display Name:</span>
                    <input
                      aria-label="GSP Provider Name"
                      value={gspName}
                      onChange={(e) => setGspName(e.target.value)}
                      placeholder="e.g. Deterministic IRP Sandbox"
                    />
                  </label>
                  {gspMode === 'HttpGsp' && (
                    <>
                      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                        <span>IRP / GSP Base URL (HTTPS):</span>
                        <input
                          aria-label="GSP Base URL"
                          value={gspBaseUrl}
                          onChange={(e) => setGspBaseUrl(e.target.value)}
                          placeholder="https://gsp.example.in/api/v1"
                        />
                      </label>
                      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                        <span>Client ID:</span>
                        <input
                          aria-label="GSP Client ID"
                          value={gspClientId}
                          onChange={(e) => setGspClientId(e.target.value)}
                        />
                      </label>
                      <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                        <span>API Secret {gspHasSecret ? '(Configured — leave blank to keep)' : ''}:</span>
                        <input
                          type="password"
                          aria-label="GSP API Secret"
                          value={gspSecretRef}
                          onChange={(e) => setGspSecretRef(e.target.value)}
                          placeholder="Secret / token"
                        />
                      </label>
                    </>
                  )}
                  <label style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginTop: '1.5rem' }}>
                    <input
                      type="checkbox"
                      aria-label="GSP Connector Enabled"
                      checked={gspEnabled}
                      onChange={(e) => setGspEnabled(e.target.checked)}
                    />
                    <span>Enable GSP Push from Voucher Inspector</span>
                  </label>
                </div>
                <button type="submit" disabled={savingGsp} style={{ marginTop: '1rem' }}>
                  {savingGsp ? 'Saving GSP Config...' : 'Save GSP Settings'}
                </button>
              </form>
            </div>

            {/* HSN / SAC Masters Form & List */}
            <div className="ot-inventory-form" style={{ marginBottom: '2rem' }}>
              <h3>Register New HSN / SAC Code</h3>
              <form onSubmit={handleCreateHsn}>
                <div className="ot-inventory-fields">
                  <input
                    aria-label="HSN Code"
                    placeholder="HSN / SAC Code (e.g. 8471)"
                    value={newHsnCode}
                    onChange={(e) => setNewHsnCode(e.target.value)}
                    required
                  />
                  <select
                    aria-label="Code Type"
                    value={newHsnType}
                    onChange={(e) => setNewHsnType(e.target.value as 'HSN' | 'SAC')}
                  >
                    <option value="HSN">HSN (Goods)</option>
                    <option value="SAC">SAC (Services)</option>
                  </select>
                  <input
                    placeholder="Description (Optional)"
                    value={newHsnDesc}
                    onChange={(e) => setNewHsnDesc(e.target.value)}
                  />
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Default GST Rate (%):</span>
                    <input
                      aria-label="Default GST Rate"
                      placeholder="18.00"
                      value={newHsnRate}
                      onChange={(e) => setNewHsnRate(e.target.value)}
                      required
                    />
                  </label>
                  <label style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                    <span>Cess Rate (%):</span>
                    <input
                      aria-label="Cess Rate"
                      placeholder="0.00"
                      value={newHsnCess}
                      onChange={(e) => setNewHsnCess(e.target.value)}
                      required
                    />
                  </label>
                </div>
                <button type="submit" style={{ marginTop: '1rem' }}>
                  Register Code
                </button>
              </form>

              <h4 style={{ marginTop: '1.5rem' }}>Registered HSN/SAC Codes</h4>
              {hsnCodes.length === 0 ? (
                <div className="ot-report-empty">No HSN/SAC codes registered yet.</div>
              ) : (
                <table className="ot-report-table" style={{ marginTop: '0.5rem' }}>
                  <thead>
                    <tr>
                      <th>Code</th>
                      <th>Type</th>
                      <th>Description</th>
                      <th>GST Rate (%)</th>
                      <th>Cess Rate (%)</th>
                      <th>Effective From</th>
                    </tr>
                  </thead>
                  <tbody>
                    {hsnCodes.map((c) => (
                      <tr key={c.id}>
                        <td><code>{c.code}</code></td>
                        <td>{c.code_type}</td>
                        <td>{c.description || '—'}</td>
                        <td>{c.default_gst_rate}%</td>
                        <td>{c.cess_rate}%</td>
                        <td>{c.effective_from}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>

            {/* GST Ledger Mappings Display */}
            {ledgerMapping && (
              <div className="ot-inventory-form">
                <h3>Default GST Ledger Mappings</h3>
                <p className="ot-report-muted">
                  Auto-resolved output tax and input tax credit (ITC) ledger linkages.
                </p>
                <div className="ot-inventory-fields" style={{ fontSize: '0.9rem' }}>
                  <div><strong>Output CGST:</strong> {ledgerMapping.output_cgst_ledger_id}</div>
                  <div><strong>Output SGST:</strong> {ledgerMapping.output_sgst_ledger_id}</div>
                  <div><strong>Output IGST:</strong> {ledgerMapping.output_igst_ledger_id}</div>
                  <div><strong>Output Cess:</strong> {ledgerMapping.output_cess_ledger_id || '—'}</div>
                  <div><strong>Input CGST (ITC):</strong> {ledgerMapping.input_cgst_ledger_id}</div>
                  <div><strong>Input SGST (ITC):</strong> {ledgerMapping.input_sgst_ledger_id}</div>
                  <div><strong>Input IGST (ITC):</strong> {ledgerMapping.input_igst_ledger_id}</div>
                  <div><strong>Input Cess (ITC):</strong> {ledgerMapping.input_cess_ledger_id || '—'}</div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
