# OpenTally Phase 1 Scope & Boundary Specification

## 1. Purpose
This document defines the functional boundaries, workstream allocations, and strict out-of-scope boundaries for OpenTally Phase 1: Behavioural Research & Evidence Capture.

## 2. In-Scope Workstreams (P0, P1, P2 Priority)

### Priority P0: Core Accounting, Vouchers & Interoperability
1. **Workstream 1 — Company Management**: Company life cycle, statutory attributes (FY, books beginning), multi-state GST configuration, backup/restore semantics, and year-end split rules.
2. **Workstream 2 — Chart of Accounts & Masters**: Group hierarchy (28 predefined groups: 15 primary, 13 secondary), ledger master schema, bill-wise tracking, cost centres/categories.
3. **Workstream 3 — Voucher Engine**: 8 core accounting vouchers (Sales, Purchase, Payment, Receipt, Contra, Journal, Debit Note, Credit Note), 10 special/inventory voucher types, voucher numbering series, duplicate prevention, and balance-sheet impact.
4. **Workstream 7 — XML/HTTP Interoperability**: Protocol specifications for port 9000, XML request/response envelope schemas, Master and Voucher import/export (`ENVELOPE/BODY/DATA/TALLYMESSAGE`), Collection and Report queries, and error handling.

### Priority P1: Inventory, Taxation & Reporting
5. **Workstream 4 — Inventory Engine**: Stock items, units (simple/compound), godowns, batches (mfg/expiry), stock journals, physical adjustments, FIFO/Average valuation, and accounting-inventory integration.
6. **Workstream 5 — GST & Indian Taxation**: CGST/SGST/IGST/Cess determination, intra-state vs inter-state place of supply, reverse charge (RCM), tax rounding rules, e-Way bill / e-Invoice fields.
7. **Workstream 6 — Financial Reports**: Trial Balance, Balance Sheet, Profit & Loss, Day Book, Cash/Bank Books, Ledger Outstandings, drill-down logic, and financial reconciliation invariants.

### Priority P2: Governance, Banking & Payroll
8. **Workstream 8 — Security & Audit**: User roles (Owner, Data Entry), permissions, backdated voucher restrictions, and TallyPrime Edit Log (MCA audit trail compliance).
9. **Workstream 9 — Banking**: Bank ledgers, auto-reconciliation, BRS workflows, cheque printing parameters, deposit slips, payment advice.
10. **Workstream 10 — Payroll**: Employee masters, pay heads (earnings/deductions), salary structures, attendance, statutory deductions (PF, ESI, PT), payslip generation.

## 3. Strict Out-of-Scope Boundaries for Phase 1
- **Application Implementation**: No Rust, Tauri, React, TypeScript, or SQLite migration code is to be authored in Phase 1.
- **Proprietary TDL Execution**: OpenTally will not implement a bytecode interpreter for proprietary compiled `.tcp` or `.tdx` files.
- **Proprietary Binary Disassembly**: No inspection or disassembly of Tally.exe or internal dlls.
- **Proprietary Data Files**: No tools or parsers for proprietary `.900` database files.
