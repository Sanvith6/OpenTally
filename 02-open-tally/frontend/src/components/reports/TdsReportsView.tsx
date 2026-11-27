import React, { useCallback, useEffect, useState, Fragment } from 'react';
import {
  Company,
  Form26qSummary,
  RecordTdsChallanPaymentInput,
  TdsOutstandingReport,
} from '../../types';
import {
  exportForm26qCsv,
  exportForm26qJson,
  getForm26qSummary,
  getTdsOutstanding,
  recordTdsChallanPayment,
} from '../../services/api';
import './Reports.css';
import './TdsReportsView.css';

interface Props {
  activeCompany: Company | null;
  onBack: () => void;
  onViewVoucher?: (voucherId: string) => void;
}

const getToday = () => new Date().toISOString().slice(0, 10);

export function TdsReportsView({ activeCompany, onBack, onViewVoucher }: Props) {
  const [activeTab, setActiveTab] = useState<'outstanding' | 'challan' | 'form26q' | 'form27q' | 'form27eq'>('outstanding');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // Outstanding State
  const [asOfDate, setAsOfDate] = useState(getToday());
  const [outstandingData, setOutstandingData] = useState<TdsOutstandingReport | null>(null);
  const [selectedVoucherIds, setSelectedVoucherIds] = useState<string[]>([]);

  // Challan Form State
  const [challanBsrCode, setChallanBsrCode] = useState('');
  const [challanDate, setChallanDate] = useState(getToday());
  const [challanSerialNo, setChallanSerialNo] = useState('');
  const [taggingSaving, setTaggingSaving] = useState(false);

  // Form 26Q State
  const [financialYear, setFinancialYear] = useState('2026-2027');
  const [quarter, setQuarter] = useState<'Q1' | 'Q2' | 'Q3' | 'Q4'>('Q1');
  const [form26qData, setForm26qData] = useState<Form26qSummary | null>(null);

  // Load Outstanding Data
  const loadOutstanding = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const res = await getTdsOutstanding(asOfDate);
      setOutstandingData(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, asOfDate]);

  // Load Form 26Q Data
  const loadForm26q = useCallback(async () => {
    if (!activeCompany) return;
    setLoading(true);
    setError(null);
    try {
      const res = await getForm26qSummary(financialYear, quarter);
      setForm26qData(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany, financialYear, quarter]);

  useEffect(() => {
    if (activeTab === 'outstanding' || activeTab === 'challan') {
      void loadOutstanding();
    } else if (activeTab === 'form26q') {
      void loadForm26q();
    }
  }, [activeTab, loadOutstanding, loadForm26q]);

  // Global Esc key navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onBack();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onBack]);

  const toggleSelectVoucher = (id: string) => {
    setSelectedVoucherIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]
    );
  };

  const handleSelectAllUnpaid = () => {
    if (!outstandingData) return;
    const allIds = outstandingData.sections.flatMap((s) => s.vouchers.map((v) => v.voucher_tds_id));
    if (selectedVoucherIds.length === allIds.length) {
      setSelectedVoucherIds([]);
    } else {
      setSelectedVoucherIds(allIds);
    }
  };

  const handleTagChallan = async (e: React.FormEvent) => {
    e.preventDefault();
    if (selectedVoucherIds.length === 0) {
      setError('Please select at least one unpaid TDS voucher to tag with challan.');
      return;
    }
    if (!/^\d{7}$/.test(challanBsrCode.trim())) {
      setError('BSR Code must be exactly 7 numeric digits.');
      return;
    }
    if (!/^\d{5}$/.test(challanSerialNo.trim())) {
      setError('Challan Serial No must be exactly 5 numeric digits.');
      return;
    }

    setTaggingSaving(true);
    setError(null);
    try {
      const input: RecordTdsChallanPaymentInput = {
        company_id: activeCompany?.id || '',
        voucher_tds_detail_ids: selectedVoucherIds,
        challan_bsr_code: challanBsrCode.trim(),
        challan_date: challanDate,
        challan_serial_no: challanSerialNo.trim(),
      };
      const count = await recordTdsChallanPayment(input);
      setNotice(`Challan payment successfully tagged to ${count} TDS vouchers.`);
      setSelectedVoucherIds([]);
      setChallanBsrCode('');
      setChallanSerialNo('');
      await loadOutstanding();
      setActiveTab('outstanding');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setTaggingSaving(false);
    }
  };

  const handleExportJson = async () => {
    setError(null);
    try {
      const jsonStr = await exportForm26qJson(financialYear, quarter);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `Form26Q_${financialYear}_${quarter}.json`;
      a.click();
      URL.revokeObjectURL(url);
      setNotice('Form 26Q JSON exported successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportCsv = async () => {
    setError(null);
    try {
      const csvStr = await exportForm26qCsv(financialYear, quarter);
      const blob = new Blob([csvStr], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `Form26Q_${financialYear}_${quarter}.csv`;
      a.click();
      URL.revokeObjectURL(url);
      setNotice('Form 26Q CSV exported successfully.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="ot-report-screen" data-testid="tds-reports-view">
      <div className="ot-report-header">
        <div>
          <h2>TDS & TCS Statutory Reports</h2>
          <p>
            {activeCompany ? `${activeCompany.name} · ` : ''}
            Tax Deducted at Source (TDS), Challan Tagging & Form 26Q Filing
          </p>
        </div>
        <button className="ot-report-back" onClick={onBack} title="Back to Gateway (Esc)">
          Back (Esc)
        </button>
      </div>

      {notice && (
        <div className="ot-report-success" role="status" style={{ marginBottom: '16px' }}>
          {notice}
        </div>
      )}

      {error && (
        <div className="ot-report-error" role="alert" style={{ marginBottom: '16px' }}>
          {error}
        </div>
      )}

      <div className="ot-tds-tabs">
        <button
          className={`ot-tds-tab-btn ${activeTab === 'outstanding' ? 'active' : ''}`}
          data-testid="tab-tds-outstanding"
          onClick={() => setActiveTab('outstanding')}
        >
          TDS Outstanding Remittance
        </button>
        <button
          className={`ot-tds-tab-btn ${activeTab === 'challan' ? 'active' : ''}`}
          data-testid="tab-tds-challan"
          onClick={() => setActiveTab('challan')}
        >
          Tag Challan Payment {selectedVoucherIds.length > 0 ? `(${selectedVoucherIds.length})` : ''}
        </button>
        <button
          className={`ot-tds-tab-btn ${activeTab === 'form26q' ? 'active' : ''}`}
          data-testid="tab-tds-form26q"
          onClick={() => setActiveTab('form26q')}
        >
          Form 26Q Quarterly Return
        </button>
        <button
          className={`ot-tds-tab-btn ${activeTab === 'form27q' ? 'active' : ''}`}
          data-testid="tab-tds-form27q"
          onClick={() => setActiveTab('form27q')}
        >
          Form 27Q (NRI)
        </button>
        <button
          className={`ot-tds-tab-btn ${activeTab === 'form27eq' ? 'active' : ''}`}
          data-testid="tab-tds-form27eq"
          onClick={() => setActiveTab('form27eq')}
        >
          Form 27EQ (TCS)
        </button>
      </div>

      {/* Tab 1: TDS Outstanding Remittance */}
      {activeTab === 'outstanding' && (
        <div className="ot-tds-container">
          <div className="ot-tds-toolbar">
            <div className="ot-tds-toolbar-left">
              <div className="ot-report-field">
                <label htmlFor="input-as-of-date">As of Date</label>
                <input
                  id="input-as-of-date"
                  data-testid="input-as-of-date"
                  type="date"
                  value={asOfDate}
                  onChange={(e) => setAsOfDate(e.target.value)}
                />
              </div>
              <button
                className="ot-report-action"
                onClick={() => void loadOutstanding()}
                disabled={loading}
              >
                Refresh
              </button>
            </div>
            {selectedVoucherIds.length > 0 && (
              <div className="ot-tds-toolbar-right">
                <button
                  className="ot-tds-export-btn primary"
                  data-testid="btn-proceed-challan"
                  onClick={() => setActiveTab('challan')}
                >
                  Proceed to Challan Tagging ({selectedVoucherIds.length} vouchers)
                </button>
              </div>
            )}
          </div>

          {loading ? (
            <div className="ot-report-loading">Loading TDS Outstanding...</div>
          ) : !outstandingData || outstandingData.sections.length === 0 ? (
            <div className="ot-report-empty">No unpaid TDS obligations found as of {asOfDate}.</div>
          ) : (
            <>
              <div className="ot-tds-summary-grid">
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Total Unpaid TDS</span>
                  <span className="ot-tds-card-value highlight-red" data-testid="total-unpaid-tds">
                    ₹ {outstandingData.total_unpaid_tds}
                  </span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Sections with Outstandings</span>
                  <span className="ot-tds-card-value">{outstandingData.sections.length}</span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Total Unpaid Vouchers</span>
                  <span className="ot-tds-card-value">
                    {outstandingData.sections.reduce((acc, s) => acc + s.voucher_count, 0)}
                  </span>
                </div>
              </div>

              <div className="ot-report-card">
                <table className="ot-report-table" data-testid="table-tds-outstanding">
                  <thead>
                    <tr>
                      <th style={{ width: '40px' }}>
                        <input
                          type="checkbox"
                          title="Select / Unselect All"
                          onChange={handleSelectAllUnpaid}
                          checked={
                            selectedVoucherIds.length > 0 &&
                            selectedVoucherIds.length ===
                              outstandingData.sections.flatMap((s) => s.vouchers).length
                          }
                        />
                      </th>
                      <th>Voucher No</th>
                      <th>Voucher Date</th>
                      <th>Party Name</th>
                      <th>Party PAN</th>
                      <th>Section</th>
                      <th className="money">Gross Amount</th>
                      <th className="money">Rate %</th>
                      <th className="money">TDS Amount</th>
                      <th>Due Date</th>
                      <th>Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {outstandingData.sections.map((section) => (
                      <Fragment key={section.section_code}>
                        <tr className="ot-report-section-row">
                          <th colSpan={8}>
                            Section {section.section_code} — {section.section_name} ({section.voucher_count} vouchers)
                          </th>
                          <th className="money" style={{ color: '#b91c1c' }}>
                            ₹ {section.unpaid_tds_amount}
                          </th>
                          <th colSpan={2}></th>
                        </tr>
                        {section.vouchers.map((v) => {
                          const isSelected = selectedVoucherIds.includes(v.voucher_tds_id);
                          return (
                            <tr key={v.voucher_tds_id} className={isSelected ? 'ot-row-selected' : ''}>
                              <td>
                                <input
                                  type="checkbox"
                                  data-testid={`checkbox-voucher-${v.voucher_tds_id}`}
                                  checked={isSelected}
                                  onChange={() => toggleSelectVoucher(v.voucher_tds_id)}
                                />
                              </td>
                              <td>
                                {onViewVoucher ? (
                                  <a
                                    href="#voucher"
                                    onClick={(e) => {
                                      e.preventDefault();
                                      onViewVoucher(v.voucher_id);
                                    }}
                                  >
                                    {v.voucher_number}
                                  </a>
                                ) : (
                                  <strong>{v.voucher_number}</strong>
                                )}
                              </td>
                              <td>{v.voucher_date}</td>
                              <td>{v.party_name}</td>
                              <td>
                                <code>{v.party_pan || 'MISSING (Sec 206AA)'}</code>
                              </td>
                              <td>
                                <code>{v.section_code}</code>
                              </td>
                              <td className="money">₹ {v.gross_amount}</td>
                              <td className="money">{v.tds_rate}%</td>
                              <td className="money" style={{ color: '#b91c1c', fontWeight: 600 }}>
                                ₹ {v.tds_amount}
                              </td>
                              <td>{v.due_date}</td>
                              <td>
                                <span
                                  className={`ot-tds-badge ${
                                    v.status.toLowerCase() === 'overdue' ? 'overdue' : 'due'
                                  }`}
                                >
                                  {v.status}
                                </span>
                              </td>
                            </tr>
                          );
                        })}
                      </Fragment>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </div>
      )}

      {/* Tab 2: Tag Challan Payment */}
      {activeTab === 'challan' && (
        <div className="ot-tds-container">
          <form className="ot-tds-challan-form" onSubmit={handleTagChallan}>
            <h3>Record & Tag Challan Remittance to Government</h3>
            <p className="ot-report-muted" style={{ margin: 0 }}>
              Tag the BSR code, Challan Date, and 5-digit Challan Serial Number to deposit TDS
              obligations under Section 200.
            </p>

            <div className="ot-tds-form-row">
              <div className="ot-report-field">
                <label htmlFor="input-bsr-code">BSR Code (7 digits) *</label>
                <input
                  id="input-bsr-code"
                  data-testid="input-bsr-code"
                  type="text"
                  maxLength={7}
                  placeholder="e.g. 0210001"
                  value={challanBsrCode}
                  onChange={(e) => setChallanBsrCode(e.target.value.replace(/\D/g, ''))}
                  required
                />
              </div>

              <div className="ot-report-field">
                <label htmlFor="input-challan-date">Challan Date *</label>
                <input
                  id="input-challan-date"
                  data-testid="input-challan-date"
                  type="date"
                  value={challanDate}
                  onChange={(e) => setChallanDate(e.target.value)}
                  required
                />
              </div>

              <div className="ot-report-field">
                <label htmlFor="input-challan-serial">Challan Serial No (5 digits) *</label>
                <input
                  id="input-challan-serial"
                  data-testid="input-challan-serial"
                  type="text"
                  maxLength={5}
                  placeholder="e.g. 00123"
                  value={challanSerialNo}
                  onChange={(e) => setChallanSerialNo(e.target.value.replace(/\D/g, ''))}
                  required
                />
              </div>

              <div style={{ alignSelf: 'flex-end' }}>
                <button
                  type="submit"
                  className="ot-report-action"
                  data-testid="btn-tag-challan-submit"
                  disabled={taggingSaving || selectedVoucherIds.length === 0}
                >
                  {taggingSaving ? 'Tagging...' : `Deposit ${selectedVoucherIds.length} Vouchers`}
                </button>
              </div>
            </div>
          </form>

          <h4>Select Unpaid TDS Vouchers to Include in this Challan:</h4>
          {!outstandingData || outstandingData.sections.length === 0 ? (
            <div className="ot-report-empty">No unpaid TDS vouchers available.</div>
          ) : (
            <div className="ot-report-card">
              <table className="ot-report-table">
                <thead>
                  <tr>
                    <th style={{ width: '40px' }}>
                      <input
                        type="checkbox"
                        onChange={handleSelectAllUnpaid}
                        checked={
                          selectedVoucherIds.length > 0 &&
                          selectedVoucherIds.length ===
                            outstandingData.sections.flatMap((s) => s.vouchers).length
                        }
                      />
                    </th>
                    <th>Voucher No</th>
                    <th>Date</th>
                    <th>Party</th>
                    <th>PAN</th>
                    <th>Section</th>
                    <th className="money">TDS Amount</th>
                    <th>Due Date</th>
                  </tr>
                </thead>
                <tbody>
                  {outstandingData.sections.flatMap((s) => s.vouchers).map((v) => {
                    const isSelected = selectedVoucherIds.includes(v.voucher_tds_id);
                    return (
                      <tr key={v.voucher_tds_id}>
                        <td>
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => toggleSelectVoucher(v.voucher_tds_id)}
                          />
                        </td>
                        <td>{v.voucher_number}</td>
                        <td>{v.voucher_date}</td>
                        <td>{v.party_name}</td>
                        <td>{v.party_pan || 'MISSING'}</td>
                        <td>{v.section_code}</td>
                        <td className="money" style={{ color: '#b91c1c', fontWeight: 600 }}>
                          ₹ {v.tds_amount}
                        </td>
                        <td>{v.due_date}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* Tab 3: Form 26Q Quarterly Return */}
      {activeTab === 'form26q' && (
        <div className="ot-tds-container">
          <div className="ot-tds-toolbar">
            <div className="ot-tds-toolbar-left">
              <div className="ot-report-field">
                <label htmlFor="select-fy">Financial Year</label>
                <select
                  id="select-fy"
                  data-testid="select-fy"
                  value={financialYear}
                  onChange={(e) => setFinancialYear(e.target.value)}
                >
                  <option value="2026-2027">2026-2027</option>
                  <option value="2025-2026">2025-2026</option>
                  <option value="2024-2025">2024-2025</option>
                </select>
              </div>

              <div className="ot-report-field">
                <label>Quarter</label>
                <div className="ot-tds-quarter-pills">
                  {(['Q1', 'Q2', 'Q3', 'Q4'] as const).map((q) => (
                    <button
                      key={q}
                      type="button"
                      data-testid={`pill-quarter-${q}`}
                      className={`ot-tds-quarter-pill ${quarter === q ? 'active' : ''}`}
                      onClick={() => setQuarter(q)}
                    >
                      {q}
                    </button>
                  ))}
                </div>
              </div>

              <button
                className="ot-report-action"
                onClick={() => void loadForm26q()}
                disabled={loading}
              >
                Refresh
              </button>
            </div>

            <div className="ot-tds-toolbar-right">
              <button
                className="ot-tds-export-btn primary"
                data-testid="btn-export-26q-json"
                onClick={handleExportJson}
                disabled={!form26qData}
              >
                📥 Export Form 26Q JSON
              </button>
              <button
                className="ot-tds-export-btn"
                data-testid="btn-export-26q-csv"
                onClick={handleExportCsv}
                disabled={!form26qData}
              >
                📊 Export 26Q CSV
              </button>
            </div>
          </div>

          {loading ? (
            <div className="ot-report-loading">Loading Form 26Q Summary...</div>
          ) : !form26qData ? (
            <div className="ot-report-empty">No Form 26Q data available for {financialYear} {quarter}.</div>
          ) : (
            <>
              <div className="ot-tds-summary-grid">
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Deductor TAN</span>
                  <span className="ot-tds-card-value highlight-blue">
                    {form26qData.deductor_tan || 'NOT CONFIGURED'}
                  </span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Total Amount Paid</span>
                  <span className="ot-tds-card-value">₹ {form26qData.total_amount_paid}</span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Total TDS Deducted</span>
                  <span className="ot-tds-card-value highlight-red">
                    ₹ {form26qData.total_tds_deducted}
                  </span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Total TDS Deposited</span>
                  <span className="ot-tds-card-value highlight-green">
                    ₹ {form26qData.total_tds_deposited}
                  </span>
                </div>
                <div className="ot-tds-card">
                  <span className="ot-tds-card-title">Balance Unpaid</span>
                  <span className="ot-tds-card-value highlight-red">
                    ₹ {form26qData.total_tds_unpaid}
                  </span>
                </div>
              </div>

              {/* Challan Annexure */}
              <div className="ot-report-card" style={{ marginTop: '16px' }}>
                <div style={{ padding: '12px 16px', background: '#f8fafc', borderBottom: '1px solid #e2e8f0' }}>
                  <h4 style={{ margin: 0 }}>Challans Annexure ({form26qData.challans.length} Challans)</h4>
                </div>
                {form26qData.challans.length === 0 ? (
                  <div className="ot-report-empty">No challans recorded for this quarter.</div>
                ) : (
                  <table className="ot-report-table" data-testid="table-26q-challans">
                    <thead>
                      <tr>
                        <th>BSR Code</th>
                        <th>Challan Date</th>
                        <th>Serial No</th>
                        <th>Vouchers Covered</th>
                        <th className="money">Total Deposited</th>
                      </tr>
                    </thead>
                    <tbody>
                      {form26qData.challans.map((ch, idx) => (
                        <tr key={idx}>
                          <td><code>{ch.challan_bsr_code}</code></td>
                          <td>{ch.challan_date}</td>
                          <td><code>{ch.challan_serial_no}</code></td>
                          <td>{ch.voucher_count}</td>
                          <td className="money" style={{ color: '#15803d', fontWeight: 600 }}>
                            ₹ {ch.total_tds_deposited}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>

              {/* Deductions Annexure */}
              <div className="ot-report-card" style={{ marginTop: '16px' }}>
                <div style={{ padding: '12px 16px', background: '#f8fafc', borderBottom: '1px solid #e2e8f0' }}>
                  <h4 style={{ margin: 0 }}>
                    Deductee Annexure ({form26qData.deductions.length} Deductions)
                  </h4>
                </div>
                {form26qData.deductions.length === 0 ? (
                  <div className="ot-report-empty">No TDS deductions recorded in this quarter.</div>
                ) : (
                  <table className="ot-report-table" data-testid="table-26q-deductions">
                    <thead>
                      <tr>
                        <th>Voucher No</th>
                        <th>Date</th>
                        <th>Deductee Name</th>
                        <th>PAN</th>
                        <th>Section</th>
                        <th className="money">Amount Paid</th>
                        <th className="money">Rate %</th>
                        <th className="money">TDS Deducted</th>
                        <th>Challan Ref</th>
                        <th>Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {form26qData.deductions.map((d) => (
                        <tr key={d.voucher_tds_id}>
                          <td><strong>{d.voucher_number}</strong></td>
                          <td>{d.payment_date}</td>
                          <td>{d.deductee_name}</td>
                          <td>
                            <code>{d.deductee_pan || 'MISSING'}</code>
                            {d.is_pan_missing_higher_rate && (
                              <span
                                className="ot-tds-badge higher-rate"
                                style={{ marginLeft: '6px' }}
                                title="20% Higher Rate under Section 206AA"
                              >
                                Sec 206AA
                              </span>
                            )}
                          </td>
                          <td><code>{d.section_code}</code></td>
                          <td className="money">₹ {d.amount_paid}</td>
                          <td className="money">{d.tds_rate}%</td>
                          <td className="money" style={{ color: '#b91c1c', fontWeight: 600 }}>
                            ₹ {d.tds_deducted}
                          </td>
                          <td>
                            {d.challan_serial_no ? (
                              <span>
                                {d.challan_bsr_code} / {d.challan_serial_no} ({d.challan_date})
                              </span>
                            ) : (
                              <span className="ot-report-muted">—</span>
                            )}
                          </td>
                          <td>
                            <span
                              className={`ot-tds-badge ${
                                d.status === 'Paid' ? 'paid' : d.status === 'Overdue' ? 'overdue' : 'due'
                              }`}
                            >
                              {d.status}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}
