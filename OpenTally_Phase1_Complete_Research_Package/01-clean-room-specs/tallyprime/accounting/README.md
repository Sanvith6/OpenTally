# Workstream 2: Chart of Accounts & Masters Specification

## 1. Predefined Accounting Groups Architecture
TallyPrime implements a hierarchical, directed-tree Chart of Accounts structured around **28 Predefined Groups**:
- **15 Primary Groups** (Top-level classifications reporting directly to Balance Sheet or Profit & Loss).
- **13 Secondary Groups** (Sub-groups nested beneath primary groups for standard business operations).

### Complete 28 Predefined Groups Hierarchy Table

| Group Name | Nature | Belongs To (Parent) | Financial Statement Destination |
| :--- | :--- | :--- | :--- |
| **Capital Account** | Liabilities | Primary | Balance Sheet (Liabilities) |
| -- *Reserves & Surplus* | Liabilities | Capital Account | Balance Sheet (Liabilities) |
| **Current Assets** | Assets | Primary | Balance Sheet (Assets) |
| -- *Bank Accounts* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Cash-in-Hand* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Deposits (Asset)* | Assets | Current Assets | Balance Sheet (Assets) |
| -- *Loans & Advances (Asset)* | Assets | Current Assets | Balance Sheet (Assets) |
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
| **Indirect Income** | Income | Primary | Profit & Loss (Income / Indirect) |
| **Indirect Expenses** | Expenses | Primary | Profit & Loss (Expenses / Indirect) |

## 2. Ledger Masters & Reserved Ledgers
1. **Reserved Ledgers**:
   - `Cash`: Predefined under `Cash-in-Hand`. Cannot be deleted. Opening balance can be altered.
   - `Profit & Loss A/c`: Predefined primary entity. Does not belong to any standard group; acts as the accumulator of net trading and operating results. Cannot be deleted.
2. **Ledger Deletion Restrictions**:
   - A ledger *cannot* be deleted (`Alt + D`) if it is referenced in any transaction voucher.
   - A group *cannot* be deleted if it contains child groups or child ledgers.

## 3. Bill-Wise Accounting (Outstanding Management)
- Supported primarily on ledgers under `Sundry Debtors` and `Sundry Creditors`.
- Configuration: `Maintain balances bill-by-bill = Yes`.
- **Bill Reference Types**:
  1. `New Ref`: Initiates a new outstanding receivable/payable tracking record (e.g., Sales Invoice `INV-2024-001`).
  2. `Agst Ref`: Adjusts against an existing unpaid `New Ref` or `Advance`. Displays a selection table showing pending bills, original amounts, and balance due.
  3. `Advance`: Tracks cash/bank received or paid prior to invoice issuance; subjects to subsequent `Agst Ref` adjustment.
  4. `On Account`: Lump-sum receipt/payment without referencing a specific invoice bill number.

## 4. Cost Centres & Cost Categories
- Enables multi-dimensional management accounting without multiplying Chart of Accounts accounts.
- **Cost Category**: Represents an independent classification dimension (e.g., "Divisions", "Projects", "Sales Executives").
- **Cost Centre**: A specific unit under a category (e.g., "Project Bangalore Metro", "Executive Rahul").
- In transactions, ledger amounts are sub-allocated across cost centres. The sum of allocated percentages or amounts must exactly equal the total ledger transaction amount.


---

## 5. Monetary Precision, Currency Minor Units & Rounding Policy

A critical architectural distinction is established regarding decimal precision across accounting, taxation, and storage layers:

```
+---------------------------------------------------------------------------------------------------+
| LAYER                               | PRECISION POLICY            | ARITHMETIC MECHANISM          |
|-------------------------------------+-----------------------------+-------------------------------|
| 1. Internal Engine Arithmetic        | High-precision / Exact      | rust_decimal (28-digit exact) |
| 2. Currency Minor Units (Ledger)    | Currency-specific (0 to 4)  | INR: 2 (Paise), JPY: 0, BHD: 3|
| 3. Stock Item Quantities & Rates    | UoM-specific (0 to 4)       | Configured per UoM Master     |
| 4. Intermediate Tax Computations    | Unrounded / High-precision  | Stored to 4-6 fractional dec  |
| 5. Final Voucher Line Posting       | Rounded to Minor Units      | Standard Banker's / Half-Up   |
| 6. Invoice-Level Total Rounding     | Statutory / Nearest Rupee   | Section 170 CGST Act Rounding |
+---------------------------------------------------------------------------------------------------+
```

### Detailed Precision Governance
1. **No Universal Four-Decimal Hardcoding**: Four decimal places must **never** be universally hardcoded across all ledgers or database columns.
2. **Currency Minor Units**: Each currency master defines its decimal precision (e.g. INR = 2, USD = 2, JPY = 0, KWD = 3). All general ledger debit/credit postings in that currency round strictly to its configured minor units.
3. **Stock Quantities & Rates**: Units of Measure independently configure decimal display (0 to 4 places). Unit purchase/sales rates may require up to 4 decimal places for precision item pricing.
4. **Intermediate Tax Precision**: Multi-item tax rate allocations (e.g., 9% CGST on ₹137.45 = ₹12.3705) maintain intermediate precision in `rust_decimal` to avoid cumulative fractional drift.
5. **Final Posting & Statutory Rounding**:
   - Line items round to currency minor units (paise for INR).
   - Any residual fractional discrepancy between line-item sums and total invoice tax is absorbed into an `Invoice Rounding` ledger under Indirect Expenses (matching TallyPrime behavior) or rounded to the nearest rupee pursuant to Section 170 of the CGST Act.
6. **Proposed Implementation Library**: `rust_decimal` is proposed for all financial and inventory arithmetic to completely eliminate IEEE-754 binary floating-point representation errors.
