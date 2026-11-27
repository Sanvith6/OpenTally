import React, { useState, useEffect, useCallback } from 'react';
import {
  Company,
  Employee,
  CreateEmployeeInput,
  PayrollAttendanceInput,
  PayrollStatutorySummary,
} from '../../types';
import {
  listEmployees,
  createEmployee,
  deactivateEmployee,
  executePayrollRun,
  getPayrollStatutorySummary,
  exportPayslipPdf,
  exportPayrollCsv,
  exportEpfoEcrFile,
} from '../../services/api';

interface PayrollViewProps {
  activeCompany: Company;
  onBack: () => void;
}

type PayrollTab = 'employees' | 'process' | 'register';

/** Parses a decimal string into integer paise (minor units) without floating-point arithmetic. */
export function parseToPaise(val: string | number | undefined): number {
  const s = String(val ?? '0').trim();
  if (!s || !/^\d+(\.\d+)?$/.test(s)) return 0;
  const [whole, frac = ''] = s.split('.');
  const padded = (frac + '00').slice(0, 2);
  return parseInt(whole, 10) * 100 + parseInt(padded, 10);
}

/** Formats integer paise to INR decimal string (e.g., 1500000 -> "15000.00"). */
export function formatPaise(paise: number): string {
  const sign = paise < 0 ? '-' : '';
  const abs = Math.abs(Math.round(paise));
  const rupees = Math.floor(abs / 100);
  const rem = abs % 100;
  return `${sign}${rupees}.${String(rem).padStart(2, '0')}`;
}

/** Pure integer-paise statutory computation matching backend rust_decimal rules. */
export function computeEmployeeStatutoryPreview(
  emp: Employee,
  workingDays: string,
  daysPresent: string,
  otherDeductions: string
) {
  const wd = parseToPaise(workingDays);
  const dp = parseToPaise(daysPresent);
  if (wd <= 0 || dp < 0 || dp > wd) {
    return {
      lopDays: '0',
      grossPaise: 0,
      eePfPaise: 0,
      erEpfPaise: 0,
      erEpsPaise: 0,
      eeEsiPaise: 0,
      erEsiPaise: 0,
      ptPaise: 0,
      totalDedPaise: 0,
      netPayPaise: 0,
    };
  }

  const lopPaise = wd - dp;
  const basicPaise = parseToPaise(emp.basic_salary);
  const hraPaise = parseToPaise(emp.hra);
  const allowPaise = parseToPaise(emp.other_allowances);
  const otherDedPaise = parseToPaise(otherDeductions);

  const proratedBasic = Math.round((basicPaise * dp) / wd);
  const proratedHra = Math.round((hraPaise * dp) / wd);
  const proratedAllow = Math.round((allowPaise * dp) / wd);
  const grossPaise = proratedBasic + proratedHra + proratedAllow;

  // EPF / EPS (15,000.00 = 1,500,000 paise ceiling; rounded to nearest whole rupee = 100 paise)
  let eePfPaise = 0;
  let erEpsPaise = 0;
  let erEpfPaise = 0;
  if (emp.pf_applicable) {
    const pfBase = Math.min(proratedBasic, 1500000);
    eePfPaise = Math.round((pfBase * 12) / 100 / 100) * 100;
    const rawEps = Math.round((pfBase * 833) / 10000 / 100) * 100;
    erEpsPaise = Math.min(rawEps, 125000);
    erEpfPaise = eePfPaise - erEpsPaise;
  }

  // ESI (21,000.00 = 2,100,000 paise threshold; ceiling to whole rupee)
  let eeEsiPaise = 0;
  let erEsiPaise = 0;
  if (emp.esi_applicable && grossPaise > 0 && grossPaise <= 2100000) {
    eeEsiPaise = Math.ceil((grossPaise * 75) / 10000 / 100) * 100;
    erEsiPaise = Math.ceil((grossPaise * 325) / 10000 / 100) * 100;
  }

  // PT (>= 25,000.00 = 2,500,000 paise -> 200.00 = 20,000 paise)
  const ptPaise = emp.pt_applicable && grossPaise >= 2500000 ? 20000 : 0;

  const totalDedPaise = eePfPaise + eeEsiPaise + ptPaise + otherDedPaise;
  const netPayPaise = grossPaise - totalDedPaise;

  return {
    lopDays: (lopPaise / 100).toString(),
    grossPaise,
    eePfPaise,
    erEpfPaise,
    erEpsPaise,
    eeEsiPaise,
    erEsiPaise,
    ptPaise,
    totalDedPaise,
    netPayPaise,
  };
}

export const PayrollView: React.FC<PayrollViewProps> = ({ activeCompany, onBack }) => {
  const [activeTab, setActiveTab] = useState<PayrollTab>('employees');
  const [employees, setEmployees] = useState<Employee[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  // Employee creation form
  const [showAddForm, setShowAddForm] = useState(false);
  const [empCode, setEmpCode] = useState('');
  const [empName, setEmpName] = useState('');
  const [designation, setDesignation] = useState('');
  const [doj, setDoj] = useState('2026-04-01');
  const [pan, setPan] = useState('');
  const [uan, setUan] = useState('');
  const [basicSalary, setBasicSalary] = useState('15000');
  const [hra, setHra] = useState('5000');
  const [otherAllowances, setOtherAllowances] = useState('0');
  const [pfApplicable, setPfApplicable] = useState(true);
  const [esiApplicable, setEsiApplicable] = useState(true);
  const [ptApplicable, setPtApplicable] = useState(true);
  const [workStateCode, setWorkStateCode] = useState('29');
  const [taxRegime, setTaxRegime] = useState<'New' | 'Old'>('New');
  const [annualDeclaredDeductions, setAnnualDeclaredDeductions] = useState('0');

  // Process Monthly Payroll state
  const [payPeriod, setPayPeriod] = useState('2026-04');
  const [runDate, setRunDate] = useState('2026-04-30');
  const [attendanceMap, setAttendanceMap] = useState<
    Record<string, { workingDays: string; presentDays: string; otherDed: string }>
  >({});

  // Register state
  const [summary, setSummary] = useState<PayrollStatutorySummary | null>(null);

  const loadEmployees = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await listEmployees(activeCompany.id, false);
      setEmployees(list);
      setAttendanceMap((prev) => {
        const next = { ...prev };
        for (const e of list) {
          if (!next[e.id]) {
            next[e.id] = { workingDays: '30', presentDays: '30', otherDed: '0' };
          }
        }
        return next;
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany.id]);

  const loadSummary = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await getPayrollStatutorySummary(payPeriod, activeCompany.id);
      setSummary(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [activeCompany.id, payPeriod]);

  useEffect(() => {
    loadEmployees();
  }, [loadEmployees]);

  useEffect(() => {
    if (activeTab === 'register') {
      loadSummary();
    }
  }, [activeTab, loadSummary]);

  const handlePostPayroll = useCallback(async () => {
    if (employees.length === 0) {
      setError('No active employees to process payroll');
      return;
    }
    setLoading(true);
    setError(null);
    setStatusMsg(null);
    try {
      const attendances: PayrollAttendanceInput[] = employees.map((e) => {
        const att = attendanceMap[e.id] || {
          workingDays: '30',
          presentDays: '30',
          otherDed: '0',
        };
        return {
          employee_id: e.id,
          total_working_days: att.workingDays,
          days_present: att.presentDays,
          other_deductions: att.otherDed,
        };
      });
      const res = await executePayrollRun(
        { pay_period: payPeriod, run_date: runDate, attendances },
        activeCompany.id
      );
      setStatusMsg(
        `Payroll posted for ${res.run.pay_period} (Voucher: ${res.voucher_number}, Net Pay: INR ${res.run.total_net_pay})`
      );
      setActiveTab('register');
      await loadSummary();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [employees, attendanceMap, payPeriod, runDate, activeCompany.id, loadSummary]);

  // Capture-phase Esc unwinding & Ctrl+A accept shortcut
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        if (showAddForm) {
          setShowAddForm(false);
        } else {
          onBack();
        }
      } else if ((e.ctrlKey || e.metaKey) && (e.key === 'a' || e.key === 'A')) {
        if (activeTab === 'process') {
          e.preventDefault();
          e.stopPropagation();
          handlePostPayroll();
        }
      }
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [showAddForm, onBack, activeTab, handlePostPayroll]);

  const handleCreateEmployee = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    try {
      const input: CreateEmployeeInput = {
        employee_code: empCode,
        name: empName,
        designation: designation || null,
        date_of_joining: doj,
        pan: pan || null,
        uan: uan || null,
        basic_salary: basicSalary,
        hra,
        other_allowances: otherAllowances,
        pf_applicable: pfApplicable,
        esi_applicable: esiApplicable,
        pt_applicable: ptApplicable,
        work_state_code: workStateCode,
        tax_regime: taxRegime,
        annual_declared_deductions: annualDeclaredDeductions,
      } as CreateEmployeeInput;
      await createEmployee(input, activeCompany.id);
      setShowAddForm(false);
      setEmpCode('');
      setEmpName('');
      setDesignation('');
      setPan('');
      setUan('');
      setStatusMsg(`Employee '${input.name}' created successfully.`);
      await loadEmployees();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleDeactivate = async (employeeId: string) => {
    setLoading(true);
    setError(null);
    try {
      await deactivateEmployee(employeeId, activeCompany.id);
      await loadEmployees();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleDownloadPdf = async (payslipId: string, employeeCode: string) => {
    try {
      const bytes = await exportPayslipPdf(payslipId, activeCompany.id);
      const blob = new Blob([new Uint8Array(bytes)], { type: 'application/pdf' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `Payslip_${employeeCode}_${payPeriod}.pdf`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportCsv = async () => {
    try {
      const csv = await exportPayrollCsv(payPeriod, activeCompany.id);
      const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `Payroll_Register_${payPeriod}.csv`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleExportEcr = async () => {
    try {
      const contents = await exportEpfoEcrFile(payPeriod, activeCompany.id);
      const blob = new Blob([contents], { type: 'text/plain;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = `EPFO_ECR_${payPeriod}.txt`;
      anchor.click();
      URL.revokeObjectURL(url);
      setStatusMsg(contents ? `EPFO ECR exported for ${payPeriod}.` : `No posted PF payroll rows exist for ${payPeriod}; empty ECR exported.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="ot-view-container" data-testid="payroll-view">
      <div className="ot-view-header">
        <div>
          <h2>Indian Payroll &amp; Statutory Compliance (Ctrl+F4)</h2>
          <p className="ot-subtitle">{activeCompany.name} — EPF, ESI, Professional Tax &amp; Payslips</p>
        </div>
        <div className="ot-header-actions">
          <button type="button" className="ot-btn ot-btn-secondary" onClick={onBack}>
            Back (Esc)
          </button>
        </div>
      </div>

      <div className="ot-tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'employees'}
          className={`ot-tab ${activeTab === 'employees' ? 'active' : ''}`}
          onClick={() => setActiveTab('employees')}
        >
          Employees Master
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'process'}
          className={`ot-tab ${activeTab === 'process' ? 'active' : ''}`}
          onClick={() => setActiveTab('process')}
        >
          Process Monthly Payroll
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === 'register'}
          className={`ot-tab ${activeTab === 'register' ? 'active' : ''}`}
          onClick={() => setActiveTab('register')}
        >
          Payslips &amp; Statutory Register
        </button>
      </div>

      {error && (
        <div className="ot-error-banner" role="alert">
          {error}
        </div>
      )}
      {statusMsg && (
        <div className="ot-success-banner" role="status">
          {statusMsg}
        </div>
      )}

      {activeTab === 'employees' && (
        <div className="ot-panel">
          <div className="ot-panel-header">
            <h3>Active Employees ({employees.length})</h3>
            <button
              type="button"
              className="ot-btn ot-btn-primary"
              onClick={() => setShowAddForm((v) => !v)}
              data-testid="add-employee-btn"
            >
              {showAddForm ? 'Close Form' : '+ Add Employee'}
            </button>
          </div>

          {showAddForm && (
            <form onSubmit={handleCreateEmployee} className="ot-form-grid" data-testid="employee-form">
              <label>
                Employee Code *
                <input
                  type="text"
                  required
                  value={empCode}
                  onChange={(e) => setEmpCode(e.target.value)}
                  placeholder="EMP-001"
                  data-testid="input-emp-code"
                />
              </label>
              <label>
                Full Name *
                <input
                  type="text"
                  required
                  value={empName}
                  onChange={(e) => setEmpName(e.target.value)}
                  placeholder="Ramesh Kumar"
                  data-testid="input-emp-name"
                />
              </label>
              <label>
                Designation
                <input
                  type="text"
                  value={designation}
                  onChange={(e) => setDesignation(e.target.value)}
                  placeholder="Senior Accountant"
                />
              </label>
              <label>
                Date of Joining *
                <input
                  type="date"
                  required
                  value={doj}
                  onChange={(e) => setDoj(e.target.value)}
                />
              </label>
              <label>
                PAN
                <input
                  type="text"
                  maxLength={10}
                  value={pan}
                  onChange={(e) => setPan(e.target.value.toUpperCase())}
                  placeholder="ABCDE1234F"
                />
              </label>
              <label>
                EPF UAN (12 digits)
                <input
                  type="text"
                  maxLength={12}
                  value={uan}
                  onChange={(e) => setUan(e.target.value)}
                  placeholder="100123456789"
                />
              </label>
              <label>
                Basic Salary (INR) *
                <input
                  type="text"
                  required
                  value={basicSalary}
                  onChange={(e) => setBasicSalary(e.target.value)}
                  data-testid="input-basic-salary"
                />
              </label>
              <label>
                HRA (INR)
                <input
                  type="text"
                  value={hra}
                  onChange={(e) => setHra(e.target.value)}
                  data-testid="input-hra"
                />
              </label>
              <label>
                Other Allowances (INR)
                <input
                  type="text"
                  value={otherAllowances}
                  onChange={(e) => setOtherAllowances(e.target.value)}
                />
              </label>
              <label>
                Work State (PT)
                <select
                  value={workStateCode}
                  onChange={(e) => setWorkStateCode(e.target.value)}
                  data-testid="select-work-state"
                >
                  <option value="29">Karnataka (29)</option>
                  <option value="27">Maharashtra (27)</option>
                  <option value="24">Gujarat (24)</option>
                  <option value="19">West Bengal (19)</option>
                  <option value="37">Andhra Pradesh (37)</option>
                  <option value="36">Telangana (36)</option>
                  <option value="33">Tamil Nadu (33)</option>
                  <option value="32">Kerala (32)</option>
                </select>
              </label>
              <label>
                Tax Regime (Sec 192)
                <select
                  value={taxRegime}
                  onChange={(e) => setTaxRegime(e.target.value as 'New' | 'Old')}
                  data-testid="select-tax-regime"
                >
                  <option value="New">New Regime (FY 25-26)</option>
                  <option value="Old">Old Regime</option>
                </select>
              </label>
              <label>
                Declared Deductions (80C/80D/HRA)
                <input
                  type="text"
                  value={annualDeclaredDeductions}
                  onChange={(e) => setAnnualDeclaredDeductions(e.target.value)}
                  data-testid="input-declared-deductions"
                />
              </label>
              <div className="ot-checkbox-group">
                <label>
                  <input
                    type="checkbox"
                    checked={pfApplicable}
                    onChange={(e) => setPfApplicable(e.target.checked)}
                  />
                  PF Applicable
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={esiApplicable}
                    onChange={(e) => setEsiApplicable(e.target.checked)}
                  />
                  ESI Applicable
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={ptApplicable}
                    onChange={(e) => setPtApplicable(e.target.checked)}
                  />
                  PT Applicable
                </label>
              </div>
              <div className="ot-form-actions">
                <button type="submit" className="ot-btn ot-btn-primary" disabled={loading} data-testid="save-employee-btn">
                  Save Employee
                </button>
              </div>
            </form>
          )}

          <table className="ot-table">
            <thead>
              <tr>
                <th>Code</th>
                <th>Name</th>
                <th>Designation</th>
                <th>Basic</th>
                <th>HRA</th>
                <th>Allowances</th>
                <th>PF / ESI / PT</th>
                <th>Action</th>
              </tr>
            </thead>
            <tbody>
              {employees.map((emp) => (
                <tr key={emp.id}>
                  <td>{emp.employee_code}</td>
                  <td>{emp.name}</td>
                  <td>{emp.designation || '-'}</td>
                  <td>{emp.basic_salary}</td>
                  <td>{emp.hra}</td>
                  <td>{emp.other_allowances}</td>
                  <td>
                    {emp.pf_applicable ? 'PF ' : ''}
                    {emp.esi_applicable ? 'ESI ' : ''}
                    {emp.pt_applicable ? 'PT' : ''}
                  </td>
                  <td>
                    <button
                      type="button"
                      className="ot-btn ot-btn-sm ot-btn-danger"
                      onClick={() => handleDeactivate(emp.id)}
                    >
                      Deactivate
                    </button>
                  </td>
                </tr>
              ))}
              {employees.length === 0 && (
                <tr>
                  <td colSpan={8}>No active employees found. Add employees to begin payroll.</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {activeTab === 'process' && (
        <div className="ot-panel">
          <div className="ot-panel-header">
            <div className="ot-inline-fields">
              <label>
                Pay Period (YYYY-MM)
                <input
                  type="text"
                  value={payPeriod}
                  onChange={(e) => setPayPeriod(e.target.value)}
                  data-testid="input-pay-period"
                />
              </label>
              <label>
                Run / Voucher Date
                <input
                  type="date"
                  value={runDate}
                  onChange={(e) => setRunDate(e.target.value)}
                />
              </label>
            </div>
            <button
              type="button"
              className="ot-btn ot-btn-primary"
              onClick={handlePostPayroll}
              disabled={loading || employees.length === 0}
              data-testid="post-payroll-btn"
            >
              Post Payroll (Ctrl+A)
            </button>
          </div>

          <table className="ot-table">
            <thead>
              <tr>
                <th>Code</th>
                <th>Name</th>
                <th>Working Days</th>
                <th>Present</th>
                <th>LOP</th>
                <th>Other Ded</th>
                <th>Gross</th>
                <th>EE PF</th>
                <th>EE ESI</th>
                <th>PT</th>
                <th>Net Pay</th>
              </tr>
            </thead>
            <tbody>
              {employees.map((emp) => {
                const att = attendanceMap[emp.id] || {
                  workingDays: '30',
                  presentDays: '30',
                  otherDed: '0',
                };
                const calc = computeEmployeeStatutoryPreview(
                  emp,
                  att.workingDays,
                  att.presentDays,
                  att.otherDed
                );
                return (
                  <tr key={emp.id}>
                    <td>{emp.employee_code}</td>
                    <td>{emp.name}</td>
                    <td>
                      <input
                        type="text"
                        value={att.workingDays}
                        onChange={(e) =>
                          setAttendanceMap((prev) => ({
                            ...prev,
                            [emp.id]: { ...att, workingDays: e.target.value },
                          }))
                        }
                        aria-label={`Working days for ${emp.name}`}
                      />
                    </td>
                    <td>
                      <input
                        type="text"
                        value={att.presentDays}
                        onChange={(e) =>
                          setAttendanceMap((prev) => ({
                            ...prev,
                            [emp.id]: { ...att, presentDays: e.target.value },
                          }))
                        }
                        aria-label={`Days present for ${emp.name}`}
                        data-testid={`present-days-${emp.employee_code}`}
                      />
                    </td>
                    <td data-testid={`lop-days-${emp.employee_code}`}>{calc.lopDays}</td>
                    <td>
                      <input
                        type="text"
                        value={att.otherDed}
                        onChange={(e) =>
                          setAttendanceMap((prev) => ({
                            ...prev,
                            [emp.id]: { ...att, otherDed: e.target.value },
                          }))
                        }
                        aria-label={`Other deductions for ${emp.name}`}
                      />
                    </td>
                    <td data-testid={`gross-${emp.employee_code}`}>{formatPaise(calc.grossPaise)}</td>
                    <td>{formatPaise(calc.eePfPaise)}</td>
                    <td>{formatPaise(calc.eeEsiPaise)}</td>
                    <td>{formatPaise(calc.ptPaise)}</td>
                    <td data-testid={`net-pay-${emp.employee_code}`}>
                      <strong>{formatPaise(calc.netPayPaise)}</strong>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {activeTab === 'register' && (
        <div className="ot-panel">
          <div className="ot-panel-header">
            <div className="ot-inline-fields">
              <label>
                Pay Period (YYYY-MM)
                <input
                  type="text"
                  value={payPeriod}
                  onChange={(e) => setPayPeriod(e.target.value)}
                />
              </label>
              <button type="button" className="ot-btn ot-btn-secondary" onClick={loadSummary}>
                Refresh
              </button>
            </div>
            <button
              type="button"
              className="ot-btn ot-btn-primary"
              onClick={handleExportCsv}
              data-testid="export-payroll-csv-btn"
            >
              Export Payroll CSV
            </button>
            <button type="button" className="ot-btn ot-btn-secondary" onClick={() => void handleExportEcr()}
              data-testid="export-epfo-ecr-btn">
              Export EPFO ECR
            </button>
          </div>

          {summary && (
            <>
              <div className="ot-summary-cards">
                <div className="ot-card">
                  <span>Status</span>
                  <strong>{summary.run_status || 'No Run'}</strong>
                </div>
                <div className="ot-card">
                  <span>Total Gross</span>
                  <strong>INR {summary.total_gross}</strong>
                </div>
                <div className="ot-card">
                  <span>PF Remittance</span>
                  <strong>INR {summary.total_pf_remittance}</strong>
                </div>
                <div className="ot-card">
                  <span>ESI Remittance</span>
                  <strong>INR {summary.total_esi_remittance}</strong>
                </div>
                <div className="ot-card">
                  <span>PT Remittance</span>
                  <strong>INR {summary.total_pt_remittance}</strong>
                </div>
                <div className="ot-card">
                  <span>Total Net Pay</span>
                  <strong>INR {summary.total_net_pay}</strong>
                </div>
              </div>

              <table className="ot-table">
                <thead>
                  <tr>
                    <th>Code</th>
                    <th>Employee</th>
                    <th>Present / WD</th>
                    <th>Gross</th>
                    <th>EE PF</th>
                    <th>EE ESI</th>
                    <th>PT</th>
                    <th>Net Pay</th>
                    <th>Payslip PDF</th>
                  </tr>
                </thead>
                <tbody>
                  {summary.rows.map((r) => (
                    <tr key={r.payslip_id}>
                      <td>{r.employee_code}</td>
                      <td>{r.employee_name}</td>
                      <td>
                        {r.days_present} / {r.total_working_days}
                      </td>
                      <td>{r.gross_earnings}</td>
                      <td>{r.employee_pf}</td>
                      <td>{r.employee_esi}</td>
                      <td>{r.professional_tax}</td>
                      <td>{r.net_pay}</td>
                      <td>
                        <button
                          type="button"
                          className="ot-btn ot-btn-sm ot-btn-primary"
                          onClick={() => handleDownloadPdf(r.payslip_id, r.employee_code)}
                          data-testid={`download-payslip-${r.employee_code}`}
                        >
                          Download Payslip PDF
                        </button>
                      </td>
                    </tr>
                  ))}
                  {summary.rows.length === 0 && (
                    <tr>
                      <td colSpan={9}>No payroll run found for {payPeriod}.</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </>
          )}
        </div>
      )}
    </div>
  );
};
