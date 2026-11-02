Phase 1 — Behavioural Research & Evidence Capture
OpenTally research programme
Setup completed
Objective

Build a sufficiently detailed, evidence-backed understanding of the accounting workflows and external behaviour that OpenTally intends to support, before implementation begins.

Research outputs
- Functional specifications
- Workflow and state-transition documentation
- Accounting rules and test scenarios
- XML/HTTP interoperability specifications
- Evidence manifests and unknowns register
- Prioritized implementation requirements

1. What we need to research
I would divide Phase 1 into ten workstreams. We should not limit OpenTally to basic debit and credit entries: TallyPrime covers accounting, inventory, taxation, payroll, banking and reporting. 
TallyHelp+1

Workstream 1 — Company creation and configuration









Company lifecycle and settings

Research:
- Create, alter, select and close a company.
- Company name, address, country and state.
- Financial year and books beginning date.
- Base currency and decimal precision.
- GST registration details.
- Enable/disable accounting, inventory and payroll features.
- Multiple companies and company selection.
- Backup, restore and financial-year transition.


Questions we need to answer:
- Can multiple companies be maintained independently?
- How do opening balances work?
- How are financial years represented?
- What happens to balances when the financial period changes?
- Which settings are company-wide and which are transaction-specific?
Official reference: TallyPrime documentation and Backup and restore.
Workstream 2 — Chart of Accounts and accounting masters









The foundation of the accounting engine

Research:
- Primary and secondary groups.
- Ledger creation, alteration and deletion.
- Parent-child group relationships.
- Cash and bank ledgers.
- Customer and supplier ledgers.
- Opening balances.
- Bill-wise accounting.
- Cost centres and cost categories.
- Interest calculations.
- Multiple currencies.


We need to understand how ledger classification affects Balance Sheet and Profit & Loss reporting.

For example:
Assets
 ├── Current Assets
 │    ├── Cash-in-Hand
 │    ├── Bank Accounts
 │    └── Sundry Debtors
 └── Fixed Assets

Liabilities
 ├── Current Liabilities
 │    ├── Sundry Creditors
 │    └── Duties & Taxes

Income
 ├── Sales Accounts
 └── Direct Income

Expenses
 ├── Direct Expenses
 └── Indirect Expenses


This is an illustrative hierarchy, not a complete reproduction of Tally's predefined chart.

Official references: Ledgers and Groups and Chart of Accounts. 
TallyHelp+2

Workstream 3 — Voucher engine









The most important functional workstream

Research:
- Sales
- Purchase
- Payment
- Receipt
- Contra
- Journal
- Debit Note
- Credit Note
- Optional and memorandum vouchers
- Reversing journals
- Inventory vouchers
- Order vouchers


For every voucher type, investigate:
1. How the user creates it.
2. Which fields are mandatory.
3. Which ledgers can be selected.
4. How debit and credit entries are generated.
5. Whether inventory is affected.
6. Whether GST is calculated.
7. How numbering works.
8. What happens when it is altered, cancelled or reversed.
9. Which reports reflect the transaction.
Voucher numbering deserves its own research: Tally documents Automatic, Automatic (Manual Override), Manual, Multi-user Auto and None. 
TallyHelp+1

Workstream 4 — Inventory management









Inventory and stock movement

Research:
- Stock groups and stock categories.
- Stock items and units of measurement.
- Alternative units.
- Godowns and locations.
- Batch tracking and expiry dates.
- Stock journals and physical stock.
- Stock valuation.
- Reorder levels.
- Purchase and sales order processing.
- Manufacturing and Bills of Materials.
- Job work and consignment.


We need to understand how a purchase changes stock, how a sales transaction reduces stock and how stock transfers differ from accounting transactions.

Official references: Inventory management and Manufacturing. 
TallyHelp+2

Workstream 5 — Indian taxation and GST









Tax configuration and calculations

Research:
- GST registration and company configuration.
- GSTIN and party registration details.
- CGST, SGST, IGST and cess.
- HSN and SAC.
- Place of supply.
- Intra-state and inter-state transactions.
- Reverse charge mechanism.
- Exempt, nil-rated and non-GST supplies.
- Input tax credit.
- GST reports and return-related workflows.
- TDS and TCS.


We must investigate tax calculations at line-item and invoice level, rounding, overrides and tax-rule effective dates.

Tally's documentation describes GST setup across company, party, sales/purchase ledgers, stock items and tax ledgers. 
TallyHelp+1


Important: Tally documentation tells us how its product works; it is not sufficient evidence that our implementation complies with current Indian tax law. That requires separate authoritative statutory research and professional validation.
Workstream 6 — Financial reports and business analysis









Reporting engine

Research:
- Trial Balance.
- Balance Sheet.
- Profit & Loss.
- Day Book.
- Cash Flow.
- Funds Flow.
- Ledger Vouchers.
- Outstanding Receivables.
- Outstanding Payables.
- Stock Summary.
- Cash and Bank Books.
- GST reports.
- Ratio analysis.


For every report, document its filters, period selection, drill-down behaviour, opening/closing balances, grouping and calculations.
Workstream 7 — XML/HTTP integration









Critical for interoperability

Research:
- HTTP server configuration.
- Default port 9000.
- XML request envelope.
- Import Data requests.
- Export Data requests.
- Collection and report requests.
- Master import/export.
- Voucher import/export.
- Response envelopes.
- Error handling.
- Character encoding.
- Large payloads and request boundaries.


Tally's official integration documentation confirms XML-over-HTTP communication and the default HTTP port of 9000. 
TallyHelp+1


This is particularly important because OpenTally should have an independently designed, documented interoperability layer rather than merely implementing a UI.
Workstream 8 — Security, users and audit trail
Research:
- User creation and authentication.
- Owner and Data Entry roles.
- Role-based access.
- Create, alter, display, print and preview permissions.
- Backdated voucher restrictions.
- User activity history.
- Master and voucher change history.
- Edit Log behaviour.
- Data encryption and password protection.
Tally documents user roles and restrictions on voucher and backdated transaction access. 
TallyHelp+1

Workstream 9 — Banking and reconciliation
Research:
- Bank ledgers.
- Bank statement imports.
- Bank reconciliation.
- Cheque printing.
- Payment advice.
- Deposit slips.
- Payment and receipt modes.
- Bank allocation and transaction references.
Workstream 10 — Payroll and extended functionality
Research:
- Employee groups and employee masters.
- Attendance and pay heads.
- Salary structures.
- Payroll vouchers.
- Payslips.
- PF, ESI and professional tax configuration.
- Payroll reports.
Payroll should be researched, but I would not put it ahead of the core accounting, voucher, inventory and GST specifications. 
TallyHelp+1


2. Research priority
Not everything needs equal depth immediately.
Priority	Workstream	Research depth
P0	Company configuration	Complete
P0	Chart of Accounts	Complete
P0	Voucher engine	Very deep
P0	XML/HTTP interoperability	Very deep
P1	Financial reports	Deep
P1	GST and taxation	Very deep
P1	Inventory	Deep
P1	Security and audit	Deep
P2	Banking	Detailed
P2	Payroll	Detailed

P0 means the specification must be sufficiently complete to support the core architecture. P1 and P2 can be researched in parallel, but they do not all need to block the first implementation milestone.
3. My initial desk-research findings
I've reviewed Tally's public documentation. Here are the important findings that should influence our research.
A. Tally is more than a voucher-entry application
The accounting model is connected to other business records:
Company Configuration
       |
       v
Chart of Accounts
       |
       v
Ledgers + Groups
       |
       v
Voucher Entry
       |
       +------> Accounting Entries
       |
       +------> Inventory Movements
       |
       +------> Tax Calculations
       |
       v
Financial Reports
       |
       v
Audit / Verification


This is a conceptual model for our research, not a claim about Tally's internal implementation.

For OpenTally, this means we should specify the relationships between vouchers, ledger postings, inventory movements and taxes before designing database tables.
B. Voucher numbering is more complex than an incrementing integer
The documented numbering methods include:
- Automatic
- Automatic with manual override
- Manual
- Multi-user automatic
- None
Tally also documents numbering series, prefixes, suffixes, duplicate prevention and restart configurations. 
TallyHelp+2


Research consequence: We need a separate voucher-numbering specification, not just a voucher_number field.
C. Inventory needs its own transaction model
Stock movements can happen through purchase and sales, stock journals, physical stock adjustments, godown transfers, manufacturing and job-work processes. Batch-wise tracking and stock valuation also affect reports. 
TallyHelp+3


Research consequence: Accounting entries and inventory movements should be related, but we shouldn't assume every inventory transaction produces the same kind of financial posting.
D. XML interoperability deserves special attention
The official documentation confirms:
- TallyPrime can run an HTTP server.
- XML requests and responses are supported.
- The default HTTP port is 9000.
- External applications can communicate with it. 
  TallyHelp+1
We need to test actual requests and responses against the selected TallyPrime version, including failures and malformed requests. Public documentation alone cannot establish every protocol detail.
E. I found a discrepancy worth recording
One official accounting page describes 28 predefined accounting voucher types, while the current voucher-types page describes 24 predefined voucher types. 
TallyHelp+1


We must not choose one number based on guesswork. It may reflect differences in release, terminology or documentation scope. Record it as an unresolved research question and verify it against the target installation.

This is exactly why our evidence system matters.

4. How we'll make Phase 1 genuinely complete
For every workstream, I want five kinds of research output:
Output	What it establishes
Behavioural specification	What the user can do and what the application visibly does
Workflow specification	Steps, states, transitions and validations
Data requirements	Fields, relationships, optionality and constraints
Accounting/business rules	Calculations, posting effects and reporting impact
Verification scenarios	How we can reproduce and confirm each observation

We should also classify every finding:
- VERIFIED_OBSERVATION — directly observed in the authorized target.
- DOCUMENTED_BEHAVIOR — stated in official public documentation.
- INFERENCE — a reasoned interpretation of observations.
- HYPOTHESIS — something to test.
- UNKNOWN — insufficient evidence.
The last four are not interchangeable. A public help page is not the same thing as a reproduced test, and an inference is not a confirmed behaviour.

Evidence capture standard
For example, for a Sales voucher:
Research ID: VCH-SALES-001

Scenario:
Create a sales voucher for a customer.

Preconditions:
- Test company exists.
- Customer ledger exists.
- Sales ledger exists.
- Required stock item exists, if using item invoice mode.

Actions:
1. Open Sales voucher.
2. Select customer.
3. Enter invoice date.
4. Add sales item or service.
5. Enter quantity and rate, if applicable.
6. Review tax calculation.
7. Save.

Record:
- Visible input fields.
- Default values.
- Mandatory fields.
- Validation messages.
- Voucher number.
- Displayed totals.
- Ledger impact.
- Inventory impact.
- Tax impact.
- Relevant report changes.

Evidence:
- Evidence ID.
- Exact target version and build.
- Observation timestamp.
- Observation method.
- Sanitized screenshots or request/response captures.
- SHA-256 digest.
- Confidence and limitations.

Unknowns:
- Unobserved edge cases.
- Ambiguous rounding.
- Unverified posting behaviour.


Use a synthetic test company with dummy names, transactions and GST details. Do not capture real business or customer data in our research evidence.