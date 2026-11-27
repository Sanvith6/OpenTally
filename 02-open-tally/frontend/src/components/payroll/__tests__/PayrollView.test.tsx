import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PayrollView } from '../PayrollView';
import * as api from '../../../services/api';
import { Company, Employee, PayrollStatutorySummary } from '../../../types';

vi.mock('../../../services/api', () => ({
  listEmployees: vi.fn(),
  createEmployee: vi.fn(),
  deactivateEmployee: vi.fn(),
  executePayrollRun: vi.fn(),
  getPayrollStatutorySummary: vi.fn(),
  exportPayslipPdf: vi.fn(),
  exportPayrollCsv: vi.fn(),
  exportEpfoEcrFile: vi.fn(),
}));

const mockCompany: Company = {
  id: 'cmp_1',
  name: 'Bharat Tech Pvt Ltd',
  mailing_name: 'Bharat Tech',
  country: 'India',
  state: 'Karnataka',
  financial_year_begins: '2026-04-01',
  books_beginning_from: '2026-04-01',
  currency_symbol: '₹',
  currency_formal_name: 'Indian Rupees',
  currency_decimal_places: 2,
  status: 'active',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-01T00:00:00Z',
};

const mockEmployee: Employee = {
  id: 'emp_1',
  company_id: 'cmp_1',
  employee_code: 'EMP-001',
  name: 'Ramesh Kumar',
  designation: 'Engineer',
  date_of_joining: '2026-04-01',
  basic_salary: '15000',
  hra: '5000',
  other_allowances: '0',
  pf_applicable: true,
  esi_applicable: true,
  pt_applicable: true,
  is_active: true,
  created_at: '2026-04-01T00:00:00Z',
};

const mockSummary: PayrollStatutorySummary = {
  pay_period: '2026-04',
  run_id: 'run_1',
  run_status: 'Posted',
  voucher_id: 'vch_1',
  total_employees: 1,
  total_gross: '20000',
  total_employee_pf: '1800',
  total_employer_epf: '550',
  total_employer_eps: '1250',
  total_pf_remittance: '3600',
  total_employee_esi: '150',
  total_employer_esi: '650',
  total_esi_remittance: '800',
  total_pt_remittance: '0',
  total_net_pay: '18050',
  rows: [
    {
      payslip_id: 'ps_1',
      employee_id: 'emp_1',
      employee_code: 'EMP-001',
      employee_name: 'Ramesh Kumar',
      total_working_days: '30',
      days_present: '30',
      lop_days: '0',
      gross_earnings: '20000',
      employee_pf: '1800',
      employer_epf: '550',
      employer_eps: '1250',
      employee_esi: '150',
      employer_esi: '650',
      professional_tax: '0',
      other_deductions: '0',
      total_deductions: '1950',
      net_pay: '18050',
    },
  ],
};

describe('PayrollView', () => {
  const onBack = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.listEmployees).mockResolvedValue([mockEmployee]);
    vi.mocked(api.createEmployee).mockResolvedValue(mockEmployee);
    vi.mocked(api.executePayrollRun).mockResolvedValue({
      run: {
        id: 'run_1',
        company_id: 'cmp_1',
        pay_period: '2026-04',
        run_date: '2026-04-30',
        voucher_id: 'vch_1',
        total_gross: '20000',
        total_deductions: '1950',
        total_employer_contributions: '2450',
        total_net_pay: '18050',
        status: 'Posted',
        created_at: '2026-04-30T00:00:00Z',
      },
      voucher_number: 'PAY-2026-04-01',
      payslips: [],
    });
    vi.mocked(api.getPayrollStatutorySummary).mockResolvedValue(mockSummary);
    vi.mocked(api.exportPayslipPdf).mockResolvedValue([37, 80, 68, 70]);
    vi.mocked(api.exportPayrollCsv).mockResolvedValue('Employee Code,Net Pay\nEMP-001,18050\n');
    vi.mocked(api.exportEpfoEcrFile).mockResolvedValue('ECR-LINE');
    window.URL.createObjectURL = vi.fn(() => 'blob:mock');
    window.URL.revokeObjectURL = vi.fn();
  });

  it('renders employees master and allows creating a new employee', async () => {
    render(<PayrollView activeCompany={mockCompany} onBack={onBack} />);
    await waitFor(() => {
      expect(screen.getByText('Ramesh Kumar')).toBeTruthy();
    });

    fireEvent.click(screen.getByTestId('add-employee-btn'));
    fireEvent.change(screen.getByTestId('input-emp-code'), { target: { value: 'EMP-002' } });
    fireEvent.change(screen.getByTestId('input-emp-name'), { target: { value: 'Sita Devi' } });
    fireEvent.click(screen.getByTestId('save-employee-btn'));

    await waitFor(() => {
      expect(api.createEmployee).toHaveBeenCalledWith(
        expect.objectContaining({
          employee_code: 'EMP-002',
          name: 'Sita Devi',
        }),
        'cmp_1'
      );
    });
  });

  it('computes live LOP and statutory preview in integer paise and posts payroll', async () => {
    render(<PayrollView activeCompany={mockCompany} onBack={onBack} />);
    await waitFor(() => expect(screen.getByText('Ramesh Kumar')).toBeTruthy());

    fireEvent.click(screen.getByRole('tab', { name: 'Process Monthly Payroll' }));

    // 15 days present out of 30 -> LOP = 15, Gross = 10000.00, Net Pay = 9025.00
    fireEvent.change(screen.getByTestId('present-days-EMP-001'), { target: { value: '15' } });
    expect(screen.getByTestId('lop-days-EMP-001').textContent).toBe('15');
    expect(screen.getByTestId('gross-EMP-001').textContent).toBe('10000.00');
    expect(screen.getByTestId('net-pay-EMP-001').textContent).toBe('9025.00');

    fireEvent.click(screen.getByTestId('post-payroll-btn'));
    await waitFor(() => {
      expect(api.executePayrollRun).toHaveBeenCalledTimes(1);
    });
  });

  it('triggers payslip PDF download and CSV export on register tab', async () => {
    render(<PayrollView activeCompany={mockCompany} onBack={onBack} />);
    await waitFor(() => expect(screen.getByText('Ramesh Kumar')).toBeTruthy());

    fireEvent.click(screen.getByRole('tab', { name: /Payslips & Statutory Register/i }));
    await waitFor(() => {
      expect(screen.getByTestId('download-payslip-EMP-001')).toBeTruthy();
    });

    fireEvent.click(screen.getByTestId('download-payslip-EMP-001'));
    await waitFor(() => {
      expect(api.exportPayslipPdf).toHaveBeenCalledWith('ps_1', 'cmp_1');
    });

    fireEvent.click(screen.getByTestId('export-payroll-csv-btn'));
    await waitFor(() => {
      expect(api.exportPayrollCsv).toHaveBeenCalledWith('2026-04', 'cmp_1');
    });
  });

  it('exports the statutory EPFO ECR file for the selected pay period', async () => {
    render(<PayrollView activeCompany={mockCompany} onBack={onBack} />);
    await waitFor(() => expect(screen.getByText('Ramesh Kumar')).toBeTruthy());
    fireEvent.click(screen.getByRole('tab', { name: /Payslips & Statutory Register/i }));
    fireEvent.click(await screen.findByTestId('export-epfo-ecr-btn'));
    await waitFor(() => expect(api.exportEpfoEcrFile).toHaveBeenCalledWith('2026-04', 'cmp_1'));
  });

  it('unwinds modal form first and then navigates back on Esc key', async () => {
    render(<PayrollView activeCompany={mockCompany} onBack={onBack} />);
    await waitFor(() => expect(screen.getByText('Ramesh Kumar')).toBeTruthy());

    fireEvent.click(screen.getByTestId('add-employee-btn'));
    expect(screen.getByTestId('employee-form')).toBeTruthy();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('employee-form')).toBeNull();
    expect(onBack).not.toHaveBeenCalled();

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
