# OpenTally — Clean-Room Indian Accounting & ERP Engine (v2.0.0)

OpenTally is a high-performance, open-source desktop accounting, inventory, taxation, manufacturing, and payroll application interoperable with standard TallyPrime observable workflows and data formats, engineered from first principles under strict clean-room governance.

Built with **Tauri v2**, **Rust** (core double-entry engine, SQLite + WAL + SQLCipher, `rust_decimal`), and **TypeScript / React 18** (keyboard-first desktop UI).

---

## Key Capabilities (`v2.0.0` Release — Milestones 1–15)

### 1. Pure Double-Entry Accounting Core & MCA Audit Certification
- **Standard Chart of Accounts**: 29 Indian standard account groups, recursive multi-level rollup, and database trigger-protected reserved ledgers (`Cash`, `Profit & Loss A/c`).
- **Exact Decimal Arithmetic**: Zero IEEE-754 floating-point math (`rust_decimal` in Rust, integer minor units / decimal strings in TypeScript) with canonical string storage in SQLite.
- **MCA 2023 Forensic Audit Trail**: Immutable `audit_log` with SHA-256 cryptographic hash chaining, live `AuditCertificationService` chain verification, and authenticated user attribution via `AppState::resolve_actor`.
- **Automated FY Year-End Closing & Period Lock**: Rolls nominal P&L accounts into Retained Earnings / Profit & Loss, carries forward Balance Sheet & FIFO stock opening layers, and enforces `PeriodLocked` freeze against backdated writes.
- **Multi-Company Isolation**: Strict tenant partitioning across all tables (`WHERE company_id = ?`).

### 2. Commercial Invoicing, Orders & Multi-Location Inventory
- **Atomic Sales (`F8`) & Purchase (`F9`)**: Atomically commits double-entry GL vouchers, GST splits, Bill-Wise allocations, and FIFO inventory cost layers in a single `BEGIN IMMEDIATE` transaction.
- **Order Processing (`Ctrl+F8` / `Ctrl+F9`)**: Sales Orders and Purchase Orders isolated from GL/stock until converted, with partial fulfillment tracking and Pending Orders outstanding reports.
- **Credit Notes (`Alt+F6`) & Debit Notes (`Alt+F5`)**: Sales and purchase return notes with GST reversal, stock adjustments, and original invoice linkage.
- **Multi-Location Godowns & Stock Journals (`Alt+F7`)**: Inter-godown transfers, godown summaries, batch/expiry aging tracking, and dual-column Stock Journals.
- **Manufacturing Bill of Materials (BOM)**: Multi-component recipes with cycle protection, proportional scaling, raw-material FIFO consumption, overhead capitalization, batch linking, and exact finished-goods landed unit costing.

### 3. Indian Statutory Compliance (GST, GSTR-2B, e-Invoice, TDS/TCS, Payroll)
- **GST Returns & GSTR-2B Reconciliation**: GSTR-1, GSTR-3B, portal GSTR-2B JSON import with 4-way ITC reconciliation (`Matched`, `Mismatch`, `Missing in Books`, `Missing in 2B`), NIC e-Invoice (`v1.1`), and e-Way Bill JSON exports.
- **TDS & TCS Engine**: TAN/PAN validation, Nature of Payment sections (`194C`, `194J`, `194I`, `194H`, `194A`, `194Q`, `206C(1H)`), FY thresholds, Section 206AA 20% missing-PAN enforcement, Challan tagging, and Form 26Q Q1–Q4 exports.
- **Indian Payroll (`Ctrl+F4`) & A4 Payslip PDFs**: `F11` auto-seeding of 7 Payroll ledgers, Employee masters (PAN/UAN), LOP proration, EPF/EPS (`₹15,000` ceiling), ESI (`₹21,000` ceiling), PT slab (`₹25,000`), atomic `vt_payroll` GL posting, voucher cancellation hooks, CWE-1236-safe CSV export, and pure-Rust A4 `%PDF-1.4` Payslip PDFs.

### 4. Banking (BRS), Management Accounting & Security Control
- **Banking & BRS**: Instrument allocations (`Cheque`, `NEFT`, `RTGS`, `IMPS`, `UPI`, etc.) and CSV bank statement deterministic auto-matching.
- **Management Accounting**: Two-pane Ratio Analysis, Cost Categories & Centres with breakup reports, and Group/Ledger/Cost Centre Budgets & Variance tracking.
- **Security Control (Argon2id RBAC)**: Optional per-company login (`Admin`, `Accountant`, `DataEntry`, `Auditor`), sole active Admin guardrails, and database-backed session validation.
- **Bill-Wise AR/AP & Aging**: `New Ref`, `Agst Ref`, `Advance`, `On Account` tracking and overdue aging intervals backed by `ReadOnlyConnectionGuard`.

### 5. Tally XML Interchange, Tax Invoice PDF & WAL Backup/Restore
- **Streaming Tally XML (`Alt+I`)**: Pure-Rust `quick-xml` parser/exporter with 50MB guard, XXE protection, dry-run preview, and `<ENVELOPE>` export (ADR-002 compliant).
- **WAL-Safe Backup & Restore (`Alt+B`)**: SQLite online backup API (`sqlite3_backup`) + SHA-256 verified restore.
- **1-Click Sample Company (`F3` → `S`)**: Realistic Indian SME dataset (`Bharath Precision Components Pvt Ltd`).

---

## Keyboard Shortcuts Quick Reference

| Shortcut | Context | Action |
|:---|:---|:---|
| `F2` | Global | Change Period / Date |
| `F3` | Global | Select Company (`Alt+F3` / `C` to Create Company) |
| `F4` | Global | Contra Voucher Entry |
| `Ctrl+F4` | Global | Indian Payroll & Statutory Compliance |
| `F5` / `Alt+F5` | Global | Payment Voucher / Debit Note (Purchase Return) |
| `F6` / `Alt+F6` | Global | Receipt Voucher / Credit Note (Sales Return) |
| `F7` / `Alt+F7` | Global | Journal Voucher / Stock Journal (Inter-Godown) |
| `F8` / `Ctrl+F8` | Global | Sales Invoicing (Commercial) / Sales Order |
| `F9` / `Ctrl+F9` | Global | Purchase Invoicing (Commercial) / Purchase Order |
| `F11` | Global | Company Features (Accounting, Inventory, GST, TDS, Payroll) |
| `Alt+G` / `Ctrl+G` | Global | Omnipresent "Go To" Command Palette & Global Search |
| `Alt+I` | Global | Tally XML Import / Export Modal |
| `Alt+B` | Global | Database Backup & Restore Modal |
| `Ctrl+A` | Voucher / Form | Accept & Save Form |
| `Ctrl+P` | Invoice / Report | Print / Export PDF |
| `Esc` | Global | Close Active Modal / Step Back in Navigation Stack |
| `A` / `M` / `P` | Gateway | Chart of Accounts / Inventory Masters / Profit & Loss |
| `T` / `B` / `C` / `D` / `L` | Gateway | Trial Balance / Balance Sheet / Cash Flow / Day Book / Ledger |
| `G` / `K` / `O` / `Q` / `Z` | Gateway | GST Reports / Banking (BRS) / Outstanding / Ratios / Year Closing & Audit |
| `2` / `Y` | Gateway | Budgets & Scenarios / Payroll (`Ctrl+F4`) |

---

## Configuration & Environment Variables

- **`OPENTALLY_DATA_DIR`**: Optional directory path overriding the OS default application data directory (`%APPDATA%\org.opentally.desktop`). When set, OpenTally stores and reads `opentally.db` inside `$OPENTALLY_DATA_DIR/opentally.db` — useful for portable deployments, multi-environment isolation, and live Playwright CDP E2E audits without touching production data.
- **`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`**: Set to `--remote-debugging-port=9222` on Windows before launching `opentally.exe` to attach Playwright directly via CDP (`http://127.0.0.1:9222`).

---

## Verification, E2E & Build Commands

### 1. Prerequisites
- **Rust**: 1.80+ (`cargo`, `rustc`, `clippy`, `rustfmt`)
- **Node.js**: 20+ & `npm`

### 2. Automated Backend & Frontend Test Suites
```powershell
# Run all 492 Rust unit, integration, and security tests
cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml

# Run all 128 Vitest React component and integration tests
npm --prefix 02-open-tally/frontend test -- --run
```

### 3. Linting, Formatting & Type Checks
```powershell
cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml --check
npm --prefix 02-open-tally/frontend run build
```

### 4. Live Desktop Playwright E2E Audit (WebView2 CDP)
```powershell
# Launch debug binary with isolated DB and WebView2 CDP enabled
$env:OPENTALLY_DATA_DIR = "$env:TEMP\opentally_e2e_db"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
.\02-open-tally\src-tauri\target\debug\opentally.exe

# Run 15-step real-app Playwright E2E verification
node 02-open-tally/frontend/e2e/playwright_tally_audit.mjs
```

### 5. Production Installer Packaging (`.exe` & `.msi`)
```powershell
npm --prefix 02-open-tally/frontend run tauri build
# Outputs:
#   02-open-tally/src-tauri/target/release/opentally.exe
#   02-open-tally/src-tauri/target/release/bundle/nsis/OpenTally_2.0.0_x64-setup.exe
#   02-open-tally/src-tauri/target/release/bundle/msi/OpenTally_2.0.0_x64_en-US.msi
```

---

## Clean-Room Governance & Legal Hygiene

- **Independent Authorship**: OpenTally is a net-new, clean-room software implementation authored solely from written behavioral specifications in `01-clean-room-specs/`.
- **Zero Proprietary Artifacts**: No proprietary executables, `.tsf` data files, proprietary TDL source, or decompiled code exist in `02-open-tally/`.
- **Safe Interoperability**: XML and statutory JSON routines operate strictly on open, observable schemas without reverse-engineering proprietary encryption or closed binary protocols.
