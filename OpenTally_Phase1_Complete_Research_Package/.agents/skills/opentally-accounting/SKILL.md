# Skill: OpenTally Accounting Domain & Statutory Governance

## 1. Double-Entry Accounting Invariants
Every financial transaction recorded in OpenTally must uphold fundamental accounting invariants:
1. **Dual Aspect Balance**: For every voucher, `SUM(Debits) == SUM(Credits)`. No unbalanced transaction may be committed.
2. **Accounting Equation**: `Assets = Liabilities + Equity (Capital + Retained Earnings/Reserves)`.
3. **Closing Balance Derivation**:
   - For Debit-normal accounts (Assets, Expenses): `Closing = Opening + Debits - Credits`.
   - For Credit-normal accounts (Liabilities, Equity, Income): `Closing = Opening + Credits - Debits`.
4. **P&L to Balance Sheet Flow**: Net Profit / Loss calculated in the Profit & Loss statement must dynamically close into the `Reserves & Surplus` or `Profit & Loss A/c` on the Balance Sheet.

## 2. Indian Statutory Taxation Governance (GST, TDS, TCS)
- **GST Framework**: Governed by the Central Goods and Services Tax Act, 2017 (CGST Act), State GST Acts (SGST), and Integrated GST Act (IGST).
- **Place of Supply (PoS)**:
  - Intra-State Supply (`Supplier State == Place of Supply`): CGST + SGST (or UTGST).
  - Inter-State Supply (`Supplier State != Place of Supply`): IGST.
- **Statutory vs Application Behavior**:
  - The software's calculation (tax rounding, invoice level vs item level) must be clearly distinguished from legal statutory compliance.
  - Rounding rule under Section 170 of the CGST Act requires rounding tax amounts to the nearest rupee (50 paise and above rounded up). TallyPrime allows configuration for line-item vs invoice-level rounding.
