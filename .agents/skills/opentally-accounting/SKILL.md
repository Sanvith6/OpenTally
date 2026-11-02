---
name: opentally-accounting
description: Domain standards and implementation guide for building the double-entry accounting engine, ledger hierarchies, voucher pipelines, and Indian taxation rules in 02-open-tally.
---

# OpenTally Accounting Implementation Skill

Use this skill when implementing accounting domain models, transaction ledgers, voucher posting engines, inventory allocation, and GST calculations in `02-open-tally/`.

> [!IMPORTANT]
> **Statutory Notice:**
> Accounting and statutory compliance specifications (such as MCA audit trail rules or GST calculation mechanics) documented in this repository represent engineering requirements designed to be verifiable against applicable legislation. They do not constitute certified statutory or tax compliance until independently audited and verified by qualified chartered accountants or legal counsel.

---

## 1. Core Financial & Invariant Standards

1. **Double-Entry Invariant:**
   - Every financial voucher consists of balanced debit and credit entries:
     $$\sum \text{Debits} \equiv \sum \text{Credits}$$
   - The double-entry balance check is an absolute invariant enforced inside SQLite immediate transactions before commit. Any unbalanced attempt must trigger an explicit transaction rollback.

2. **Decimal Monetary Precision & Rounding Policies:**
   - **Zero Floating-Point Arithmetic:** Financial calculations must never use IEEE 754 floating-point types (`f32`, `f64`, JavaScript `number`).
   - **Rust Implementation:** Use `rust_decimal::Decimal` (128-bit fixed-point decimal) for all monetary quantities, rates, and unit costs. In SQLite, persist values as exact `TEXT` or scaled integer minor units (paise).
   - **TypeScript Implementation:** Use `Decimal.js` or integer minor units.
   - **Rounding Policies:** Rounding rules must be explicit per domain operation:
     - Half-Up (`ROUND_HALF_UP`) for standard commercial invoices.
     - Banker's Rounding (`ROUND_HALF_EVEN`) where specified by tax standards.
     - Document rounding granularity (e.g. 2 decimal places for INR currency amounts, 4 decimal places for unit prices and foreign exchange rates).

3. **Transaction Atomicity & Rollback:**
   - Voucher posting must be fully atomic across all impacted sub-domains:
     - Voucher header record
     - General ledger journal entries
     - Bill-by-bill reference tracking (Agst Ref, New Ref, Advance)
     - Inventory stock ledger & batch allocation
     - Statutory tax line items
     - Audit log history
   - If any step fails (e.g., negative stock constraint violation, foreign key failure), the entire transaction must abort and roll back completely.

4. **Voucher Immutability & Lifecycle Rules:**
   - **No Hard Deletion of Posted Vouchers:** Once a voucher has transitioned to the `POSTED` state, physical row deletion (`DELETE FROM vouchers`) is strictly prohibited.
   - **Cancellation Semantics:** A posted voucher may be marked `CANCELLED`. A cancelled voucher zeroes out all associated ledger and stock impacts, preserves its assigned voucher number, retains its audit history, and records the cancellation timestamp, user, and reason.
   - **Reversal Semantics:** Alternatively, corrections may be executed via a dedicated Reversing Journal Voucher referencing the original voucher.
   - **Draft Vouchers:** Unposted draft vouchers may be purged or discarded without ledger impact.

5. **Append-Only Audit History & Field-Level Change Records:**
   - Maintain an append-only audit trail (edit log) for all financial masters and vouchers.
   - For every state modification or cancellation, record:
     - Primary key of modified entity.
     - Operation type (`CREATE`, `UPDATE`, `CANCEL`).
     - Timestamp in ISO 8601 UTC.
     - Operating identity (user ID or subagent ID).
     - Field-level diff (JSON before-and-after values).
   - Edit log tables must be protected against modification or deletion.

6. **Effective-Dated, Configurable Tax Rules:**
   - Indian GST (CGST, SGST, IGST, UTGST, Cess), TDS, and TCS rules must be decoupled from voucher posting logic.
   - Tax rates, thresholds, HSN/SAC mappings, and place-of-supply matrices must be modeled as date-aware entities with `effective_from` and `effective_to` boundaries.
   - No tax rates may be hardcoded into Rust business logic.

7. **Separation of Compatibility Behavior from Internal Invariants:**
   - External XML interfaces may accept and emit legacy formatting quirks (e.g., legacy nil values or specific XML envelope shapes).
   - Core accounting engine tables and ledger balances must strictly reflect GAAP, Ind AS, and statutory invariants.

---

## 2. Implementation Methodology (TDD)

1. **Locate Specification:** Read the corresponding specification in `01-clean-room-specs/tallyprime/`.
2. **Write Failing Tests:** Create unit and integration test fixtures using the test vectors defined in the specification.
3. **Implement Core Logic:** Implement the minimal code in Rust (`02-open-tally/src-tauri/`) or TypeScript (`02-open-tally/frontend/`) to pass tests.
4. **Code Review:** Proactively trigger `code-reviewer` and `rust-reviewer` / `typescript-reviewer` to verify immutability, error handling, and security.
