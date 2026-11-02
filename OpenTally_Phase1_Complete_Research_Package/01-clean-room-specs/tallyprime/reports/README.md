# Workstream 6: Financial & Inventory Reports Specification

## 1. Core Financial Reporting Engine
TallyPrime's reporting engine is characterized by real-time calculation, hierarchical rollups, drill-down interactivity, and period filtering.

### 1.1 Trial Balance
- **Purpose**: Verifies that total debits equal total credits across all open ledgers.
- **Display Modes**:
  - `Grouped View`: Aggregated at the 15 Primary Group levels, expandable to child groups and ledgers (`Alt + F5`).
  - `Ledger View`: Alphabetical flat listing of all active ledgers with opening, debit, credit, and closing balances.
- **Mathematical Invariant**: `SUM(Closing Debits) == SUM(Closing Credits)`. Any discrepancy is dynamically posted to the `Diff in Opening Balances` line on the Trial Balance.

### 1.2 Profit & Loss Statement (Income Statement)
- **Structure**: Horizontal (T-format) or Vertical (Schedule III format).
- **Left Side (Trading Expenses)**: Opening Stock, Purchases (less returns), Direct Expenses, Gross Profit c/d.
- **Right Side (Trading Income)**: Sales (less returns), Direct Incomes, Closing Stock.
- **Second Tier (Indirect Operations)**: Gross Profit b/d + Indirect Incomes minus Indirect Expenses yields **Net Profit / Loss**.
- **Dynamic Closing**: Net Profit transfers automatically to `Capital Account > Reserves & Surplus` on the Balance Sheet.

### 1.3 Balance Sheet
- **Structure**: Liabilities & Equity on Left, Assets on Right (Indian standard GAAP presentation).
- **Formula**: `Capital + Reserves/Surplus + Loans + Current Liabilities == Fixed Assets + Investments + Current Assets + Misc Expenses`.
- **Dynamic Integration**: Stock-in-Hand value is computed from inventory valuation tables if integration is enabled.

### 1.4 Cash Flow & Funds Flow Statements
- **Cash Flow**: Reconciles net cash changes broken into Operating, Investing, and Financing activities.
- **Funds Flow**: Analyzes working capital movements (`Current Assets - Current Liabilities`).

### 1.5 Outstanding Reports
- **Receivables (`Gateway of Tally > Display > Statements of Accounts > Outstandings > Receivables`)**: Displays unpaid Sales bills with pending amounts, due dates, and overdue days.
- **Payables**: Displays unpaid Purchase bills.
- **Bill-wise Drill-down**: Pressing Enter on any bill opens the underlying transaction voucher.
