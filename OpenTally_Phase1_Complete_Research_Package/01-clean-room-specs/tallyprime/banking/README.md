# Workstream 9: Banking Engine Specification

## 1. Bank Ledgers & Master Configuration
A ledger configured under `Bank Accounts` or `Bank OD A/c` supports specialized banking parameters:
- Account Holder Name, Account Number, IFS Code, Bank Name, Branch.
- Cheque Book Management: Definition of cheque number ranges (e.g., `100001` to `100100`).
- Cheque Printing Dimensions: Custom layout configuration for printing payee, date, and amount directly onto bank-issued physical cheques.

## 2. Bank Reconciliation Statement (BRS)
- Reconciles the company's ledger book balance with the actual balance reflected on the bank statement.
- **Workflow**:
  1. User navigates to `Gateway of Tally > Banking > Bank Reconciliation`.
  2. Selects the Bank Ledger.
  3. System displays all un-cleared Payment and Receipt transactions.
  4. For each voucher, the user enters the **Bank Date** (the date funds cleared the bank).
  5. The report computes:
     - `Balance as per Company Books`.
     - `Amounts not reflected in Bank (Cheques issued but not presented / Cheques deposited but not cleared)`.
     - `Balance as per Bank`.
- **Auto BRS**: Supports importing digital bank statements (in Excel, MT940, or CSV formats) and automatically matching transaction amounts and instrument numbers.
