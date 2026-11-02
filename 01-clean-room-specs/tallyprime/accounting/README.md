# Accounting Specifications Domain

This directory contains clean-room specifications for the core double-entry accounting engine, ledger hierarchies, and financial reporting in OpenTally.

> [!NOTE]
> All accounting specifications in this directory represent requirements to be verified against applicable accounting standards (GAAP, Ind AS) and statutory requirements, rather than claims of existing legal certification.

---

## 1. Specification Coverage

1. **Chart of Accounts Hierarchy:**
   - Primary Groups (Assets, Liabilities, Income, Expenses).
   - Sub-Groups with inherited balance behaviors.
   - Posting Ledgers and Parent Group associations.

2. **Double-Entry & Balance Invariants:**
   - Every financial voucher must satisfy $\sum \text{Debits} = \sum \text{Credits}$.
   - All balance derivations must use exact fixed-point decimal arithmetic (zero floating-point math).

3. **Multi-Currency & Foreign Exchange:**
   - Base Currency vs. Foreign Currency transactions.
   - Forex gain/loss computation rules and period-end revaluations.

4. **Financial Statement Derivations:**
   - Recursive balance rollups for Trial Balance, Profit & Loss Account, and Balance Sheet.
   - Real-time closing balance computation rules.

5. **Observed Compatibility vs. Financial Correctness:**
   - Distinguish legacy export formatting conventions from internal balance storage integrity.
