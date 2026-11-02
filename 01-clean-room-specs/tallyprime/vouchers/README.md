# Voucher Specifications Domain

This directory contains functional specifications for voucher processing, numbering sequences, ledger allocations, and lifecycle state machines in OpenTally.

> [!NOTE]
> All specifications in this directory represent requirements to be verified against applicable accounting standards and statutory regulations, rather than claims of existing legal certification.

---

## 1. Specification Coverage

1. **Standard Voucher Types:**
   - Sales, Purchase, Payment, Receipt, Contra, Journal, Debit Note, Credit Note.
   - Memo and Optional vouchers (behavioral quirks documented separately).

2. **Voucher Numbering Schemes:**
   - Automatic numbering with configurable prefix, suffix, and restart periodicity (Daily, Monthly, Yearly, Never).
   - Manual numbering with uniqueness verification.
   - Sequence continuity preservation: Number gaps must not occur due to deletion.

3. **Double-Entry & Decimal Precision Invariants:**
   - Every posted transaction must enforce:
     $$\sum \text{Debit Allocations} = \sum \text{Credit Allocations}$$
   - Calculations use exact fixed-point decimal representation (`rust_decimal`). Zero floating-point rounding errors permitted.

4. **Transaction Atomicity & Rollback:**
   - Multi-leg allocation must be atomic: Voucher header, ledger entries, bill adjustments, inventory batch movements, and tax calculations commit together or roll back entirely.

5. **Lifecycle State Machine & Immutability:**
   - **`DRAFT`**: Unposted entry; can be modified or purged without ledger impact.
   - **`POSTED`**: Immutable financial record. **Hard deletion of posted vouchers is strictly prohibited.**
   - **`CANCELLED`**: Nullifies all ledger and inventory impacts while retaining the voucher record, sequence number, and audit history.
   - **`REVERSED`**: Retains original voucher; links to an offsetting reversing journal entry.

6. **Audit Trail & Edit Log Compliance:**
   - Specifications must define the data model for capturing an append-only, tamper-evident audit log:
     - Timestamp (ISO 8601 UTC)
     - User / Subagent identifier
     - Action (`CREATE`, `UPDATE`, `CANCEL`)
     - Complete field-level before/after JSON diffs.

7. **Observed Compatibility vs. Accounting Integrity:**
   - Tally XML import/export compatibility requirements (e.g. accepting `<VOUCHER>` elements with legacy flags) must be documented in `01-clean-room-specs/tallyprime/interoperability/`.
   - Core voucher specifications must enforce strict financial integrity regardless of legacy import quirks.
