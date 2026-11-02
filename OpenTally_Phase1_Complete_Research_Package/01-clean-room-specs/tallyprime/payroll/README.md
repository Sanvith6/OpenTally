# Workstream 10: Payroll Engine Specification

## 1. Payroll Architecture & Master Configuration
When `Maintain Payroll = Yes` is enabled in Company features, TallyPrime provisions payroll capabilities:
1. **Employee Groups**: Categorization by department or location (e.g., "Engineering", "Sales", "Operations").
2. **Employee Masters**: Contains Personal Details, Date of Joining, Designation, Bank Details, PAN, Aadhaar, PF UAN, and ESI Insurance Number.
3. **Attendance / Production Types**:
   - `Present`: Positive attendance.
   - `Absent`: Leave without pay.
   - `Overtime`: Production type measured in hours.
4. **Pay Heads**:
   - **Earnings for Employees**: Basic Pay, HRA, Conveyance, Special Allowance.
   - **Deductions from Employees**: Employee PF (12%), Employee ESI (0.75%), Professional Tax (PT).
   - **Employer's Statutory Contributions**: Employer PF (12%), Employer ESI (3.25%), EPS.
   - **Calculation Types**: As Flat Rate, On Attendance, As Computed Value (Formula-driven, e.g., HRA = 40% of Basic).

## 2. Payroll Vouchers & Processing
- **Attendance Voucher (Ctrl + F5)**: Records days/hours worked per employee.
- **Payroll Voucher (Ctrl + F4)**: Auto-computes gross earnings, statutory deductions, net payable salaries, and credits `Salary Payable` ledger.
- **Payslip Generation**: Standard itemized payslip displaying earnings, deductions, attendance summary, and net pay.
