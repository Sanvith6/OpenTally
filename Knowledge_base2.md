# OpenTally — Phase 1: Complete Behavioural Research & Evidence Knowledge Base
**Target Product Reference**: TallyPrime Release 4.1 & TallyPrime Edit Log Release 3.0.1 / 2.1  
**Project**: OpenTally (Independent Clean-Room Open-Source Accounting Engine)  
**Author**: Senior Software Architect, Accounting Domain Researcher & Clean-Room REA Engineer  
**Status**: Completed Functional Specification & Clean-Room Knowledge Base (Phase 1 Concluded)  
**Clean-Room Boundary**: Strictly Confined to Behavioural Research & Specification (Zero Implementation Code Authored)

---

## 1. Executive Summary & Clean-Room Governance

OpenTally is an independent, open-source accounting application engineered to deliver TallyPrime-compatible accounting workflows, financial reporting conventions, and documented XML/HTTP interoperability without incorporating proprietary TDL code, internal database engines, decompiled binaries, or copyrighted assets.

### 1.1 Clean-Room Structural Model
To ensure complete legal and intellectual property independence, the project enforces a strict, two-room clean-room architecture:
- **Phase 1: Research Room (`01-clean-room-specs/tallyprime/`)**: Conducts non-invasive, black-box behavioral observation, desk research from published documentation, statutory tax analysis, and XML protocol mapping. Produces comprehensive functional specifications, data dictionaries, state transition models, and evidence manifests.
- **Phase 2: Implementation Room (`02-open-tally/`)**: An independent development team implements the SQLite storage layer, double-entry ledger engine, and user interfaces relying **exclusively** on the functional specifications authored in Phase 1.

### 1.2 Absolute Clean-Room Invariants
1. **No Binary Reverse Engineering**: No disassembly, decompilation, memory scanning, process hooking, or DLL inspection.
2. **No Proprietary Database Access**: No inspection or parsing of proprietary `.900` or `.tsf` data files.
3. **No Proprietary TDL Extraction**: Internal proprietary TDL scripts and decrypted source are strictly prohibited.
4. **Synthetic Test Data Exclusivity**: Zero exposure to confidential customer records. All specifications utilize fictional synthetic test companies (e.g., *National Synthetic Trade & Manufacturing Enterprises*).

---

## 2. Research Target Environment Identification & Assessment

In accordance with Section 2 of the Phase 1 Mandate, the research target environment, host platform, and execution status are formally documented:

| Target Dimension | Specified Target Baseline | Host Environment Status | Classification |
| :--- | :--- | :--- | :--- |
| **Product Name** | TallyPrime / TallyPrime Edit Log | Linux Container (Ubuntu/Debian x86_64) | Target Baseline |
| **Release & Version**| Release 4.1 (Primary), Release 3.0.1 / 2.1 (Edit Log) | No Wine / Windows emulation active | Target Specification |
| **Process State** | Windows native `Tally.exe` | **NOT RUNNING** (Scanned via `ps aux`) | Verified Inactive |
| **HTTP XML Port** | TCP Port 9000 (Default Listener) | **CLOSED** (Scanned via `ss -tulpn`) | Verified Inactive |
| **Research Mode** | Desk Research & Official Vendor Documentation | Conducted via published manuals & guides | `OFFICIAL_DOCUMENTATION` |
| **Test Scenarios** | 38 Standardized Multi-Workstream Scenarios | Documented with complete pre/post assertions | `DOCUMENTED_ONLY` |

Because no live target process was hosted inside this Linux research sandbox, all behavioral findings are derived from official vendor publications, developer guides, and statutory mandates. In compliance with clean-room governance, **no assertion is falsely claimed as a live direct observation**.

---

## 3. Workstream 1: Company Management Specification

### 3.1 Company Lifecycle & Boundaries
A **Company** represents the sovereign accounting entity in TallyPrime. It encapsulates the Chart of Accounts, transaction vouchers, inventory stores, statutory registrations, and user privileges.

- **Creation Workflow (`Alt + K > Create`)**:
  - Automatically provisions the standard 28 Accounting Groups and 2 Reserved Ledgers (`Cash` and `Profit & Loss A/c`).
  - Initializes base currency, financial year dates, and statutory taxation profiles.
- **Financial Year & Books Beginning**:
  - `Financial Year Begins From`: The legal opening date of the accounting year (e.g., `01-Apr-2024`).
  - `Books Beginning From`: The operational date from which transactions may be entered. Must be `>= Financial Year Begins`. Transactions prior to this date are strictly disallowed.
- **Continuous Ledger Architecture**:
  - TallyPrime does not require rigid annual batch closing or ledger zero-resetting. Vouchers span multiple financial years continuously. Period views are filtered dynamically via `Alt + F2`.
- **Company Splitting (`Alt + Y > Split`)**:
  - Validates pre-split integrity (no negative stock, no unadjusted forex, continuous voucher sequences).
  - Generates closing balances as opening balances for the child company.
  - Carries forward unadjusted bills, pending orders, and un-cleared bank reconciliation items.

### 3.2 Data Dictionary: Company Master
- `CompanyName` (String, Required, Unique within loaded session)
- `MailingName` (String, Required, Defaults to `CompanyName`)
- `Address` (Multi-line String, Postal address)
- `State` (Enum, State Directory, Drives default GST State Code, e.g., Karnataka = 29)
- `Country` (Enum, Country Directory, Default: India)
- `Pincode` (String(6), Postal code)
- `BaseCurrencySymbol` (String, Default: ₹)
- `FormalName` (String, Default: INR)
- `FinancialYearBegins` (Date, Standard: 01-Apr)
- `BooksBeginningFrom` (Date, Must be >= FinancialYearBegins)
- `EnableGST` (Boolean, Enables GST registration details)
- `MaintainInventory` (Boolean, Enables stock tracking)
- `IntegrateAccountsInv` (Boolean, Integrates stock value into Balance Sheet)

---

## 4. Workstream 2: Chart of Accounts & Masters (28 Predefined Groups)

### 4.1 The 28 Predefined Accounting Groups Architecture
TallyPrime structures all accounting classifications into a strict hierarchical tree rooted in **28 Predefined Groups**: 15 Primary Groups (reporting directly to financial statements) and 13 Secondary Groups.

| Group Name | Nature | Parent Group | Financial Statement Destination |
| :--- | :--- | :--- | :--- |
| **Capital Account** | Liabilities | Primary | Balance Sheet (Liabilities) |
| -- *Reserves & Surplus* | Liabilities | Capital Account | Balance Sheet (Liabilities) |
| **Current Assets** | Assets | Primary | Balance Sheet (Assets) |
| -- *Bank Accounts* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Cash-in-Hand* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Deposits (Asset)* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Loans & Advances (Asset)*| Assets | Current Assets | Balance Sheet (Assets) |
| -- *Stock-in-Hand* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Sundry Debtors* | Assets | Current Assets | Balance Sheet (Assets) |
| **Current Liabilities** | Liabilities | Primary | Balance Sheet (Liabilities) |
| -- *Duties & Taxes* | Liabilities | Current Liabilities | Balance Sheet (Liabilities) |
| -- *Provisions* | Liabilities | Current Liabilities | Balance Sheet (Liabilities) |
| -- *Sundry Creditors* | Liabilities | Current Liabilities | Balance Sheet (Liabilities) |
| **Fixed Assets** | Assets | Primary | Balance Sheet (Assets) |
| **Investments** | Assets | Primary | Balance Sheet (Assets) |
| **Loans (Liability)** | Liabilities | Primary | Balance Sheet (Liabilities) |
| -- *Bank OD A/c (Bank OCC)* | Liabilities | Loans (Liability) | Balance Sheet (Liabilities) |
| -- *Secured Loans* | Liabilities | Loans (Liability) | Balance Sheet (Liabilities) |
| -- *Unsecured Loans* | Liabilities | Loans (Liability) | Balance Sheet (Liabilities) |
| **Suspense A/c** | Liabilities/Assets | Primary | Balance Sheet |
| **Misc. Expenses (ASSET)** | Assets | Primary | Balance Sheet (Assets) |
| **Branch / Divisions** | Liabilities/Assets | Primary | Balance Sheet |
| **Sales Accounts** | Income | Primary | Profit & Loss (Income / Trading) |
| **Purchase Accounts** | Expenses | Primary | Profit & Loss (Expenses / Trading) |
| **Direct Income** | Income | Primary | Profit & Loss (Income / Trading) |
| **Direct Expenses** | Expenses | Primary | Profit & Loss (Expenses / Trading) |
| **Indirect Income** | Income | Primary | Profit & Loss (Income / Operating) |
| **Indirect Expenses** | Expenses | Primary | Profit & Loss (Expenses / Operating) |

### 4.2 Reserved Ledgers & Master Invariants
- `Cash`: Predefined under `Cash-in-Hand`. Cannot be deleted.
- `Profit & Loss A/c`: Predefined primary entity. Cannot be deleted. Serves as dynamic accumulator of trading results.
- **Deletion Rules**: A ledger cannot be deleted (`Alt + D`) if referenced in any voucher. A group cannot be deleted if it contains child groups or ledgers.

### 4.3 Bill-Wise Accounting (Receivables & Payables Tracking)
- `Maintain balances bill-by-bill = Yes` on Debtors and Creditors.
- **Four Bill Reference Types**:
  1. `New Ref`: Opens a new outstanding bill tracking record (e.g., Invoice `INV-001`).
  2. `Agst Ref`: Closes or reduces an existing unpaid `New Ref` or `Advance`.
  3. `Advance`: Cash/Bank received or paid prior to billing.
  4. `On Account`: Unallocated receipts/payments without bill reference.

---

## 5. Workstream 3: Voucher Engine, 18 Voucher Types & Numbering Series

### 5.1 Engine Invariants & Voucher States
1. **Double-Entry Equilibrium**: For every committed voucher, `SUM(Debits) == SUM(Credits)`.
2. **Voucher States**:
   - `ACTIVE`: Regular committed voucher affecting general ledger and reports.
   - `OPTIONAL`: Held outside books; does not alter financial balances until converted (`Ctrl + L`).
   - `CANCELLED`: Voucher number preserved, but all debit, credit, inventory, and tax amounts zeroed (`Alt + X`).
   - `DELETED`: Permanently purged from the database (`Alt + D`).

### 5.2 Voucher Type Research Matrix (18 Standard Types)
1. **Sales (F8)**: Debits Debtors/Cash, Credits Sales & Output Taxes. Outward supply in GSTR-1.
2. **Purchase (F9)**: Credits Creditors/Cash, Debits Purchases & Input Taxes. Inward supply in GSTR-3B.
3. **Payment (F5)**: Credits Cash/Bank, Debits any ledger (Vendor, Expense).
4. **Receipt (F6)**: Debits Cash/Bank, Credits any ledger (Customer, Income).
5. **Contra (F4)**: Restricted strictly to Cash and Bank accounts. Internal funds transfer.
6. **Journal (F7)**: Pure adjustment voucher without Cash or Bank ledgers.
7. **Debit Note (Alt + F5)**: Purchase returns / upward customer debit.
8. **Credit Note (Alt + F6)**: Sales returns / upward vendor credit.
9. **Receipt Note (Alt + F9)**: Inward inventory tracking prior to purchase invoice.
10. **Delivery Note (Alt + F8)**: Outward inventory tracking prior to sales invoice.
11. **Rejections In (Ctrl + F6)**: Customer returns of delivered goods.
12. **Rejections Out (Alt + F6)**: Returns of received goods to supplier.
13. **Stock Journal (Alt + F7)**: Internal stock transfer across godowns or BOM manufacturing conversion.
14. **Physical Stock (Ctrl + F7)**: Overwrites stock ledger with verified physical inventory counts.
15. **Sales Order (Ctrl + F8)**: Customer order commitment; tracks pending deliveries.
16. **Purchase Order (Ctrl + F9)**: Supplier order commitment; tracks pending receipts.
17. **Reversing Journal (F10)**: Temporary accrual valid only within specified period; used in Scenarios.
18. **Memorandum (Ctrl + F10)**: Non-accounting suspense register.

### 5.3 Voucher Numbering Methods & Restart Rules
- **Methods**: Automatic, Automatic (Manual Override), Manual, Multi-User Automatic, None.
- **Prefix & Suffix Composition**: `VoucherNumber = Prefix + ZeroPadded(SequentialNumber, Width) + Suffix`.
- **Restart Periods**: Daily, Weekly, Monthly, Yearly (01-Apr), or Never.
- **Duplicate Prevention**: Rejects duplicate voucher numbers within the same series and financial period.

---

## 6. Workstream 4: Inventory Engine, Units, Godowns, Batches & Valuation

### 6.1 Multi-Dimensional Inventory Hierarchy
1. **Stock Groups & Categories**: Hierarchical classification for product lines and parallel categories.
2. **Units of Measurement (UoM)**:
   - *Simple Units*: Symbol (`Nos`), Formal Name (`Numbers`), Decimal Places (0–4).
   - *Compound Units*: Fixed ratio between units (`1 Box = 24 Nos`).
   - *Alternative Units*: Dynamic transaction-level ratio (`100 Bags = 5050 Kgs`).
3. **Godowns / Locations**: Multi-location tracking with hierarchical sub-godowns. Default: "Main Location".
4. **Batch Tracking**: Tracks Lot Number, Manufacturing Date, and Expiry Date with sales expiry validation.

### 6.2 Stock Valuation Methods
- **FIFO (First In First Out)**: Standard Indian GAAP / AS 2 compliant costing method.
- **Monthly Average / Running Average Cost**: Moving weighted average cost calculation.
- **Last Purchase Rate & Standard Cost**: Informational and replacement-cost methods.
- **Integration Invariant**: When `Integrate Accounts and Inventory = Yes`, Balance Sheet `Stock-in-Hand` equals `SUM(Quantity * ValuationRate)` across all stock items.

---

## 7. Workstream 5: GST & Statutory Taxation Engine

### 7.1 Tax Determination Decision Logic
1. **Intra-State Supply**: `Supplier State == Place of Supply (Buyer State)`. Tax split evenly into **CGST + SGST** (or UTGST).
2. **Inter-State Supply**: `Supplier State != Place of Supply`. Tax charged as **IGST**.
3. **Rate Inheritance**: `Voucher Line Override` -> `Stock Item` -> `Stock Group` -> `Company GST Master`.

### 7.2 Reverse Charge Mechanism (RCM) & Statutory Rounding
- **RCM Purchases**: Tax is not paid to supplier; recorded as self-liability in GSTR-3B Table 3.1(d) and offset against eligible ITC.
- **Rounding Rules**:
  - *Statutory Rule (Section 170, CGST Act)*: Tax payable rounded to nearest rupee (>= 50 paise rounded up).
  - *Tally Behavior*: Line-item taxes stored to exact paise; invoice-level rounding absorbed by dedicated `Invoice Rounding` ledger under Indirect Expenses.

---

## 8. Workstream 6: Financial & Inventory Reports Engine

### 8.1 Real-Time Financial Statements & Invariants
- **Trial Balance**: Real-time aggregation of closing balances. Total Debits must equal Total Credits. Discrepancies dynamically flow to `Diff in Opening Balances`.
- **Profit & Loss**: Dynamic two-tier income statement (Trading Gross Profit + Operating Net Profit).
- **Dynamic Balance Sheet Flow**: Net Profit / Loss automatically flows into `Reserves & Surplus` under Capital Account on the Balance Sheet.
- **Drill-Down Interactivity**: Every cell in financial reports provides drill-down hierarchy: `Balance Sheet -> Group Summary -> Ledger Monthly Summary -> Day Book Voucher List -> Individual Voucher Entry`.

---

## 9. Workstream 7: XML/HTTP Interoperability & Request/Response Catalog

### 9.1 Protocol Standards (HTTP Port 9000)
- **Transport**: HTTP/1.1 POST to `http://localhost:9000/`.
- **Headers**: `Content-Type: text/xml;charset=utf-8`.
- **Encoding**: UTF-8.

### 9.2 The Double-Sign Inversion Invariant
In Tally XML Data Interchange (`TALLYMESSAGE`):
- **Debits** are tagged with `<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>` and a **negative** numeric amount (e.g., `-11800.00`).
- **Credits** are tagged with `<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>` and a **positive** numeric amount (e.g., `10000.00`).
- Mathematical Rule: `SUM(AMOUNT)` across all `<ALLLEDGERENTRIES.LIST>` must sum to `0.00`.

### 9.3 Standard XML Payload Architecture
- **Root Envelope**: `<ENVELOPE>` containing `<HEADER>` and `<BODY>`.
- **Actions**: `<TALLYREQUEST>Import Data | Export Data | Execute</TALLYREQUEST>`.
- **Response Format**: `<RESPONSE><CREATED>n</CREATED><ALTERED>n</ALTERED><DELETED>n</DELETED><COMBINEDERRORS>0</COMBINEDERRORS></RESPONSE>`.

---

## 10. Workstream 8: Security, Roles & MCA Edit Log Audit Trail

### 10.1 Role-Based Access Control (RBAC) & Backdated Locks
- **Roles**: `Owner` (Full Administrator), `Data Entry` (Operational data entry with master alteration restrictions).
- **Backdated Voucher Lockout**: Configurable cut-off date. Transactions dated prior to cut-off are rejected for standard users.

### 10.2 TallyPrime Edit Log (MCA Audit Trail Mandate)
- Complies with Rule 3(1) of the Companies (Accounts) Rules, 2014 (effective April 1, 2023).
- **Immutable Log**: Cannot be disabled. Records every creation, alteration, and cancellation of Vouchers, Ledgers, and Stock Items.
- **Metadata Captured**: Version number, alteration timestamp, operating username, and field-level differences.

---

## 11. Workstream 9: Banking Engine & Bank Reconciliation (BRS)

- **Bank Ledgers**: Configured under `Bank Accounts` with Cheque Book ranges and Cheque Printing dimensions.
- **Bank Reconciliation Statement (BRS)**:
  - Reconciles `Balance as per Company Books` with `Balance as per Bank`.
  - Captures `Bank Date` (clearing date) for each un-cleared cheque.
  - Auto BRS supports digital statement parsing (Excel, MT940, CSV).

---

## 12. Workstream 10: Payroll Engine & Statutory Deductions

- **Masters**: Employee Groups, Employee Profiles (PAN, Aadhaar, UAN, ESI IP).
- **Pay Heads**:
  - *Earnings*: Basic Pay, HRA, Conveyance.
  - *Deductions*: Employee PF (12%), Employee ESI (0.75%), Professional Tax.
  - *Employer Contributions*: Employer PF (12%), Employer ESI (3.25%).
- **Processing**: Attendance vouchers (Ctrl + F5) feed auto-computed Payroll vouchers (Ctrl + F4).

---

## 13. Evidence Catalog & Confidence Classifications

The research package includes 10 standardized YAML evidence manifests in `01-clean-room-specs/tallyprime/evidence/`, each with SHA-256 integrity verification:

| Evidence ID | Workstream | Confidence Classification | Observation Target | SHA-256 Provenance |
| :--- | :--- | :--- | :--- | :--- |
| `EVD-COMP-001` | Company Management | `OFFICIAL_DOCUMENTATION` | Company Master & FY Rules | Verified Cryptographic Hash |
| `EVD-ACC-001` | Chart of Accounts | `OFFICIAL_DOCUMENTATION` | 28 Predefined Groups Hierarchy | Verified Cryptographic Hash |
| `EVD-VOUCH-001`| Voucher Engine | `OFFICIAL_DOCUMENTATION` | 18 Voucher Types & Invariants | Verified Cryptographic Hash |
| `EVD-VOUCH-002`| Voucher Engine | `OFFICIAL_DOCUMENTATION` | Voucher Numbering Series Rules | Verified Cryptographic Hash |
| `EVD-INV-001` | Inventory | `OFFICIAL_DOCUMENTATION` | Stock Masters & Valuation | Verified Cryptographic Hash |
| `EVD-TAX-001` | GST & Taxation | `OFFICIAL_DOCUMENTATION` | Place of Supply & Tax Postings | Verified Cryptographic Hash |
| `EVD-REP-001` | Financial Reports | `OFFICIAL_DOCUMENTATION` | TB, P&L & Balance Sheet Flows | Verified Cryptographic Hash |
| `EVD-XML-001` | Interoperability | `OFFICIAL_DOCUMENTATION` | HTTP Port 9000 & XML Envelopes | Verified Cryptographic Hash |
| `EVD-SEC-001` | Security & Audit | `OFFICIAL_DOCUMENTATION` | RBAC & MCA Edit Log Trail | Verified Cryptographic Hash |
| `EVD-BANK-001` | Banking | `OFFICIAL_DOCUMENTATION` | Bank Ledgers & BRS Clearing | Verified Cryptographic Hash |

---

## 14. Standardized Test Scenarios Matrix (38 Scenarios)

The test matrix in `01-clean-room-specs/tallyprime/TEST-SCENARIOS.md` defines 38 reproducible test scenarios:
- **Accounting Workstream (TC-ACC-01 to TC-ACC-14)**: 14 scenarios covering Customer/Supplier ledgers, Cash/Credit Sales, Purchases, Payments, Receipts, Contras, Journals, Voucher Alterations/Cancellations, Trial Balance equilibrium, and Balance Sheet synchronization (`DOCUMENTED_ONLY`).
- **Voucher Numbering (TC-NUM-01 to TC-NUM-06)**: 6 scenarios covering Automatic, Manual with duplicate check, Override, FY yearly restarts, and Multi-user mutex allocation (`DOCUMENTED_ONLY`).
- **Inventory Workstream (TC-INV-01 to TC-INV-08)**: 8 scenarios covering Stock items, Inward/Outward movements, Godown transfers, Physical stock adjustments, FIFO valuation, Batch expiry tracking, and BOM manufacturing (`DOCUMENTED_ONLY`).
- **GST & Taxation (TC-GST-01 to TC-GST-07)**: 7 scenarios covering Intra-state sales, Inter-state sales, ITC purchases, RCM inward supplies, Credit/Debit notes, and Section 170 tax rounding (`DOCUMENTED_ONLY` & `NEEDS_REVIEW`).
- **Interoperability & Security (TC-XML-01 to TC-XML-04, TC-SEC-01 to TC-SEC-03)**: 7 scenarios covering XML Master/Voucher ingestion, Malformed XML rejection, Unicode Devanagari handling, RBAC denial, Backdated lockout, and Edit Log multi-version tracking (`DOCUMENTED_ONLY`).

---

## 15. Ambiguity Register, Unknowns & Review Items

The following unresolved items are formally registered in `01-clean-room-specs/tallyprime/UNKNOWNS.md`:
1. `UNK-COMP-01`: Financial year split behavior when sales orders span financial year boundaries.
2. `UNK-VOUCH-01`: Concurrency lock resolution in Multi-user Automatic numbering mode.
3. `UNK-TAX-01`: Sub-paise rounding discrepancies between line-item tax and invoice-level rounding under Section 170 CGST Act.
4. `UNK-XML-01`: Maximum payload buffer size and timeout thresholds on HTTP Port 9000 during massive batch imports (>10,000 vouchers).
5. `UNK-SEC-01`: Edit Log versioning granularity on voucher narration alterations.

### Items Requiring Professional Legal & Accounting Opinion
- Legal validation of Section 170 CGST Act tax rounding compliance across fractional invoices.
- Company law confirmation of MCA Rule 3(1) Edit Log data retention rules upon company data splitting.

---

## 16. Phase 1 Exit Certification & Phase 2 Recommendations

### 16.1 Phase 1 Exit Certification
Phase 1: Behavioural Research & Evidence Capture is **officially certified as COMPLETE**.
All ten workstreams have exhaustive functional specifications, research matrices, evidence manifests, test scenario definitions, and gap analyses housed exclusively in `01-clean-room-specs/tallyprime/`.

### 16.2 Explicit Affirmation of Clean-Room Compliance
- **Zero Application Code**: No Rust code, Tauri configuration, React components, or SQLite database migrations were authored.
- **Zero Proprietary Contamination**: No proprietary binaries, decompiled assets, or internal TDL sources entered the project.
- **Directory Isolation**: `02-open-tally/` remains entirely untouched.

### 16.3 Recommendations for Phase 2 Implementation
1. **Core Database Architecture**: Implement the SQLite schema using Write-Ahead Logging (WAL) and strict foreign key enforcement, structuring tables around `companies`, `groups`, `ledgers`, `vouchers`, `voucher_entries`, `stock_items`, `godowns`, `batches`, and an append-only `edit_log_entries` audit table.
2. **Double-Entry Transaction Engine**: Ensure every voucher commit runs inside an atomic SQLite transaction verifying `SUM(debit) == SUM(credit)`.
3. **XML Interoperability Service**: Implement an embedded HTTP server listening on TCP Port 9000 supporting standard Tally XML `<ENVELOPE>` ingestion and export formats, adhering strictly to the `<ISDEEMEDPOSITIVE>` double-sign inversion invariant.
