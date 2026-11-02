# Taxation & Statutory Specifications Domain

This directory contains clean-room specifications for Indian statutory compliance, Goods and Services Tax (GST), Tax Deducted at Source (TDS), and Tax Collected at Source (TCS).

> [!NOTE]
> Statutory calculation rules (e.g. GST slabs, place-of-supply logic, TDS thresholds) documented here represent technical specifications derived from empirical observation and published rules. They are requirements to be verified against the Central Goods and Services Tax (CGST) Act, State GST acts, and Income Tax provisions, and do not constitute certified legal or tax advice.

---

## 1. Core Architectural Requirements

1. **Configurable & Effective-Dated Architecture:**
   - Tax rates, thresholds, exemptions, and classifications must never be hardcoded into application source code.
   - All statutory rules must be modeled as date-bounded configurations with `effective_from` and `effective_to` timestamps.
   - When a tax rate change notification is issued (e.g. GST slab adjustment), updates must be achievable via configuration/master data updates without requiring code recompilation.

2. **GST Computation Coverage:**
   - **Intra-State Transactions:** Dual split into Central GST (CGST) and State/UT GST (SGST/UTGST).
   - **Inter-State Transactions:** Integrated GST (IGST).
   - **Place of Supply Matrix:** Deterministic determination based on Supplier State Code, Recipient State Code, and Delivery Location.
   - **HSN/SAC Classification:** Item-level and service-level rate mappings with mandatory digit length validations based on turnover thresholds.
   - **Reverse Charge Mechanism (RCM):** Auto-generation of self-invoices and offsetting tax liability/input tax credit (ITC) entries.
   - **GST Cess:** Percentage and quantity-based cess calculations.

3. **Decimal Precision & Rounding Rules:**
   - Tax line calculations must use exact fixed decimal types (`rust_decimal`).
   - Line-item vs. Invoice-total rounding policies must be explicitly parameterized:
     - Line-by-line calculation with sum rounding.
     - Tax calculated on taxable invoice sum with line apportioning.
   - Rounding behavior quirks observed in TallyPrime must be tracked in `UNKNOWNS.md` (e.g. `UNK-002`).

4. **TDS & TCS Processing:**
   - Section-based threshold tracking (e.g. Section 194C, 194J, 194Q, 206C).
   - PAN availability validation (standard rate vs. higher Section 206AA rate).
   - Cumulative turnover accumulation across financial year boundaries.

5. **Reporting & Return Export Compatibility:**
   - Specification of GSTR-1, GSTR-3B, and e-Invoice JSON export schemas.
   - Separation of external reporting formatting (compatibility layer) from the internal ledger tax tracking balances.
