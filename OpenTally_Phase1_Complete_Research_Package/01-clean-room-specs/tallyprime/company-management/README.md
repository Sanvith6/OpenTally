# Workstream 1: Company Management Specification

## 1. Domain Overview & Purpose
In TallyPrime, the **Company** represents the top-level organizational, legal, and accounting entity. It bounds the Chart of Accounts, transaction vouchers, inventory repositories, statutory taxation registrations, and security contexts. A single installation can host multiple independent companies stored as distinct subdirectories within the Data Directory.

## 2. Company Lifecycle & User Workflows
1. **Creation Workflow (`Alt + K > Create`)**:
   - Initializes a new company folder (numeric folder ID, e.g., `10000`, `10001`).
   - Sets baseline accounting parameters, currency symbols, and statutory country defaults.
   - Automatically provisions predefined masters: 28 Accounting Groups, 2 Reserved Ledgers (`Cash` and `Profit & Loss A/c`), standard Voucher Types, and Currencies.
2. **Alteration Workflow (`Alt + K > Alter`)**:
   - Modifies existing company metadata (Name, Address, Financial Year dates, Statutory features).
   - *Restrictions*: Financial Year Beginning date cannot be altered if vouchers already exist prior to the new proposed beginning date.
3. **Selection & Closure (`Alt + K > Select / Shut`)**:
   - Allows switching between multiple loaded companies.
   - When multiple companies are loaded, one is active as the primary context; reports can be generated comparatively across loaded companies.
4. **Backup & Restore (`Alt + Y > Backup / Restore`)**:
   - Creates a consolidated archive file (traditionally `TBK900.001` or ZIP-based container) containing the encrypted folder contents.
   - Restore unpacks the archive into a designated data directory, validating directory availability and preventing accidental overwrite without explicit user confirmation.
5. **Data Splitting (`Alt + Y > Split > Split Data`)**:
   - Splits a multi-year company into two separate companies (e.g., Company A ending 31-Mar-2024 and Company B beginning 01-Apr-2024).
   - Generates opening balances for the new company by calculating closing balances of the prior company as of the split date.
   - Transfers pending sales/purchase orders, bills, and unadjusted advances into the new company.

## 3. Data Dictionary & Field Specifications

| Field Name | Type | Required | Default | Validation & Constraints |
| :--- | :--- | :--- | :--- | :--- |
| `CompanyName` | String(100) | YES | None | Cannot be empty; unique within active loaded session. |
| `MailingName` | String(100) | YES | Inherits `CompanyName` | Used on invoices, vouchers, and statutory returns. |
| `Address` | Multi-line String | NO | Empty | Up to 5 lines for postal communication. |
| `State` | Enum (State List) | YES | None (Prompted) | Determines GST State Code (e.g., Karnataka = 29, Maharashtra = 27). |
| `Country` | Enum (Country List)| YES | India | Drives default statutory tax engine, currency format, and legal terms. |
| `Pincode` | String(10) | NO | Empty | 6-digit postal code for Indian companies; validated for format. |
| `Telephone` / `Mobile`| String(20) | NO | Empty | Contact numbers. |
| `Email` / `Website` | String(100) | NO | Empty | Validated for standard RFC email patterns. |
| `BaseCurrencySymbol` | String(5) | YES | ₹ (INR for India) | Standard currency glyph. |
| `FormalName` | String(25) | YES | INR (Indian Rupee) | Full legal name of currency. |
| `FinancialYearBegins`| Date | YES | `01-Apr-YYYY` | Must be standard first day of tax year (April 1 in India). |
| `BooksBeginningFrom` | Date | YES | Inherits `FinancialYearBegins` | Must be `>= FinancialYearBegins` and `<= FinancialYearBegins + 1 Year`. |
| `EnableGST` | Boolean | YES | No | When Yes, opens detailed GST Registration dialogue. |
| `EnableTDS` | Boolean | NO | No | Enables Tax Deducted at Source configuration. |
| `EnableTCS` | Boolean | NO | No | Enables Tax Collected at Source configuration. |
| `MaintainInventory` | Boolean | YES | Yes | Enables stock items, godowns, and inventory vouchers. |
| `IntegrateAccountsInv`| Boolean | YES | Yes | When Yes, stock value from inventory flows into Balance Sheet. |

## 4. State Transitions & Rules
- **State: Initializing -> Active**: Upon creation, standard 28 groups and default ledgers are committed.
- **State: Active -> Split**: Running pre-split verification verifies: (a) No unadjusted forex gains/losses, (b) No negative stock balances, (c) All voucher numbering series are continuous.
- **Year-End Transition Rule**: Unlike legacy accounting packages that require a rigid annual "closing book" batch job, TallyPrime operates on a **Continuous Ledger Architecture**. Vouchers can be entered seamlessly across financial year boundaries (`01-Apr-2023` to `31-Mar-2025`) without locking or forcing a split, provided the user changes the active period (`Alt + F2`).

## 5. Error & Edge Case Behavior
- **Duplicate Company Creation**: Permitted in different data directories; flagged if identical name and folder exist in the same path.
- **Altering Books Beginning Date Forward**: If vouchers exist between the old date and new date, TallyPrime blocks the alteration with: *"Vouchers exist prior to the specified Books Beginning date. Cannot alter."*
