# Workstream 5: GST & Statutory Taxation Specification

## 1. Indian GST Architecture in TallyPrime
TallyPrime implements a declarative, rule-based GST engine covering Central GST (CGST), State GST (SGST), Union Territory GST (UTGST), Integrated GST (IGST), and GST Compensation Cess.

### 1.1 Tax Determination Decision Logic
The application evaluates supply type and applicable tax rates using the following hierarchy:
1. **Place of Supply (PoS) vs Supplier State**:
   - `Supplier State == Place of Supply (Buyer State)` -> **Intra-State Supply**: Split evenly into **CGST + SGST** (or UTGST).
   - `Supplier State != Place of Supply` -> **Inter-State Supply**: Taxed entirely as **IGST**.
   - `Export / SEZ Supply`: Treated as Inter-State; classified as Taxable with Bond/LUT or Taxable with Payment of Tax.
2. **Rate Inheritance Hierarchy**:
   The engine searches for applicable HSN/SAC and tax rate in the following order:
   `Transaction Line Item Override` -> `Stock Item Master` -> `Stock Group Master` -> `Company GST Master`.
   The lowest specific entity overrides higher general defaults.

## 2. Tax Ledgers & Account Postings
- Tax ledgers must be created under the predefined group **`Duties & Taxes`**.
- `Type of Duty/Tax`: Must be set to `GST`.
- `Tax Type`: Must be specified as `Central Tax`, `State Tax`, `Integrated Tax`, or `Cess`.
- In a voucher, when an item is entered with 18% GST:
  - If Intra-state (Karnataka to Karnataka):
    - User selects `CGST` ledger -> System auto-calculates 9% on taxable value.
    - User selects `SGST` ledger -> System auto-calculates 9% on taxable value.
  - If Inter-state (Karnataka to Maharashtra):
    - User selects `IGST` ledger -> System auto-calculates 18% on taxable value.
  - If the user selects an incompatible tax ledger (e.g., selecting IGST on an Intra-State transaction), TallyPrime sets the calculated tax amount to `0.00` and displays a mismatch warning in Tax Analysis (`Alt + A`).

## 3. Reverse Charge Mechanism (RCM)
- Applicable on specified purchases from unregistered dealers (URD) or notified goods/services (e.g., GTA, Legal Services).
- Inward supply recorded in Purchase voucher with `Is Reverse Charge Applicable = Yes`.
- **Accounting Posting**:
  - Does *not* add tax to the supplier's invoice payable balance.
  - Generates tax liability in GSTR-3B Table 3.1(d).
  - Liability is discharged via a Journal voucher (`Stat Adjustment: Increase of Tax Liability & Input Tax Credit`).

## 4. Statutory vs Application Rounding Differences
- **Statutory Mandate (Section 170, CGST Act)**: The amount of tax, interest, penalty, or fine payable shall be rounded off to the nearest rupee (amounts >= 50 paise rounded up, < 50 paise disregarded).
- **Application Behavior**:
  - TallyPrime calculates line-item tax to 2 decimal places.
  - Invoice-level total rounding requires adding a `Round Off` ledger grouped under `Indirect Expenses` configured with `Type of Ledger = Invoice Rounding`.
  - OpenTally Engine must replicate this exact dual-tier rounding: line items stored in exact paise; invoice-level rounding ledger absorbed into total payable.

## 5. TDS & TCS Workflows
- **TDS (Tax Deducted at Source)**:
  - Deductor (Company) deducts tax from payments to vendors under sections (e.g., 194C Contractor, 194J Professional Services).
  - Deducts from party bill and credits `TDS Payable` under `Duties & Taxes`.
- **TCS (Tax Collected at Source)**:
  - Collector collects tax from buyer (e.g., Section 206C(1H) on sales of goods exceeding ₹50 Lakhs).
  - Adds TCS amount to Sales invoice, increasing receivable balance.
