# OpenTally Changelog

All notable changes to OpenTally are documented in this file.
OpenTally follows [Semantic Versioning](https://semver.org/) and strict clean-room software engineering governance.

---

## [2.0.0] — 2026-11-25

Full enterprise release delivering TallyPrime-grade Indian banking, statutory taxation (TDS/TCS, GSTR-2B, NIC e-Invoice & e-Way Bill), multi-location inventory & manufacturing BOM, management accounting, automated FY closing with MCA 2023 cryptographic audit certification, order processing, RBAC security, Indian payroll, and live WebView2 Playwright E2E hardening.

### Added (Milestones 8–15)
- **Milestone 8 — Banking & Bank Reconciliation Statement (BRS) (`v1.1.0-banking`):**
  - Bank allocation & instrument tracking (`Cheque`, `NEFT`, `RTGS`, `IMPS`, `UPI`, `DD`, `Card`, `ECS`) on `F4` Contra, `F5` Payment, and `F6` Receipt.
  - Full BRS workspace reconciling Book Balance vs. Bank Balance, bank date clearing, and deterministic CSV bank statement auto-matching.
- **Milestone 9 — Indian TDS & TCS Engine (`v1.2.0-tds`):**
  - TAN & PAN validation, Nature of Payment section masters (`194C`, `194J`, `194I`, `194H`, `194A`, `194Q`, `206C(1H)`), single and cumulative FY threshold tracking, and Section 206AA (20% missing-PAN) enforcement.
  - GST-exclusive TDS deduction on `F9` Purchase / `F7` Journal, Challan tagging, and Form 26Q Q1–Q4 summary & CSV/JSON export.
- **Milestone 10 — GSTR-2B Purchase ITC Reconciliation, e-Invoice & e-Way Bill JSON (`v1.3.0-gst-compliance`):**
  - Portal GSTR-2B JSON import and deterministic 4-way reconciliation (`Matched`, `Mismatch`, `Missing in Books`, `Missing in 2B` flagging At-Risk ITC).
  - NIC e-Invoice (`v1.1` schema) JSON generator & IRN metadata tracking, plus NIC e-Way Bill JSON generator & consignment metadata.
- **Milestone 11 — Multi-Location Godowns, Stock Journals (`Alt+F7`), Batches/Expiry & Cost Centres (`v1.4.0-godowns-costing`):**
  - Extended godown hierarchy (`address`, `is_external`, `is_active`), dual-column Stock Journals (`Alt+F7`) for inter-godown transfers and consumption/production.
  - Batch & expiry tracking with Batch Expiry Aging report, Cost Categories & Cost Centres with multi-dimensional voucher allocation & breakup report.
- **Milestone 12 — Ratio Analysis, Budgets, Year-End Closing & MCA 2023 Audit Certification (`v1.5.0-closing-budgets`):**
  - Two-pane TallyPrime Ratio Analysis (`Principal Groups` + `Principal Ratios`).
  - Group, Ledger, and Cost Centre Budgets & Variance reports.
  - Automated Financial Year-End Closing rolling nominal P&L into Retained Earnings / P&L, carrying forward Balance Sheet & FIFO stock opening layers, and enforcing `PeriodLocked` freeze against backdated mutations.
  - MCA 2023 Audit Trail Forensic Certification verifying SHA-256 hash chaining over `audit_log` with exportable certification reports.
- **Milestone 13 — Order Processing (`Ctrl+F8` / `Ctrl+F9`) (`v1.6.0-orders`):**
  - Sales Orders (`Ctrl+F8`) and Purchase Orders (`Ctrl+F9`) isolated from General Ledger and FIFO stock until converted, with partial fulfillment tracking, Pending Orders report, and atomic order-to-invoice conversion.
- **Milestone 14 — Security Control (Users & RBAC) & Manufacturing BOM (`v1.7.0-security-bom`):**
  - Per-company Argon2id authentication and 4-role RBAC (`Admin`, `Accountant`, `DataEntry`, `Auditor`), sole active Admin protection, live database-backed session validation inside `AppState::resolve_actor`, and real user attribution in `audit_log`.
  - Multi-component Manufacturing Bill of Materials (BOM) with self-referential cycle rejection, proportional scaling, raw-material FIFO consumption, overhead capitalization, batch linking, and exact finished-goods landed unit costing.
- **Milestone 15 — Indian Payroll Engine (`Ctrl+F4`) & A4 Payslip PDF (`v1.8.0-payroll`):**
  - `F11` Payroll enablement & auto-seeding of 7 standard Payroll ledgers (`Salary Expense`, `Employer PF Contribution`, `Employer ESI Contribution`, `Salary Payable`, `PF Payable`, `ESI Payable`, `Professional Tax Payable`).
  - Employee masters (PAN/UAN validation), attendance & LOP proration, EPF/EPS (`₹15,000` basic ceiling), ESI (`₹21,000` gross ceiling), Professional Tax (`₹25,000` slab), atomic `vt_payroll` GL voucher posting, voucher cancellation hook, CWE-1236-safe Payroll CSV export, and pure-Rust A4 `%PDF-1.4` Salary Payslip PDF generation.

### Fixed & Hardened (Live WebView2 Playwright CDP Audit)
- **Idempotent Startup Migrations (`run_all_migrations`):** Fixed startup crash when reopening databases already migrated past `0.1.0-sprint0` (up to `v1.8.0-payroll`).
- **Isolated Data Path (`OPENTALLY_DATA_DIR`):** Added environment variable override for custom/E2E SQLite storage directories.
- **Startup Company Selector:** Automatically opens `SelectCompanyView` (`F3`) on startup when no active company is selected.
- **Voucher Entry Navigation & `Esc` Propagation:** Switching `F4`/`F5`/`F6`/`F7` inside Voucher Entry replaces the active voucher frame rather than stacking duplicate history entries, and `Escape` stops propagation to prevent double-popping.
- **Gateway Hotkey Disambiguation:** Reassigned `Budgets & Scenarios` hotkey from `T` to `2` so `T` deterministically opens `Trial Balance`.
- **F11 Stale Closure Fix:** Added `enablePayroll` and `autoSeedPayrollLedgers` to `CompanyFeaturesModal` `handleSave` dependency array so toggling Payroll in `F11` reliably persists and auto-seeds ledgers.

---

## [1.0.0] — 2026-11-12

Initial clean-room production release covering Milestones 1–7.

### Added (Milestones 1–7)
- Pure double-entry accounting engine (`rust_decimal`, 29 standard Indian account groups, trigger-protected reserved ledgers, MCA immutable `audit_log`, `ReadOnlyConnectionGuard`).
- Atomic commercial invoicing (`F8` Sales, `F9` Purchase) with FIFO / Weighted Average perpetual inventory and `Alt+F5` Debit Note / `Alt+F6` Credit Note lifecycle.
- Bill-wise Outstanding Tracking (`New Ref`, `Agst Ref`, `Advance`, `On Account`) and AR/AP overdue aging reports.
- Indian GST calculation engine, HSN/SAC masters, GSTR-1 & GSTR-3B JSON exports, and pure-Rust A4 Tax Invoice PDF generation.
- Streaming clean-room Tally XML import (with dry-run preview & XXE protection) and `<ENVELOPE>` XML export.
- WAL-safe SQLite hot-backup (`sqlite3_backup`) and SHA-256 verified restore.
- TallyPrime keyboard-first workspace (`F2`–`F11`, `Alt+G` Go To palette, `Esc` unwinding, `F11` Features, and 1-click Indian SME Sample Company).
