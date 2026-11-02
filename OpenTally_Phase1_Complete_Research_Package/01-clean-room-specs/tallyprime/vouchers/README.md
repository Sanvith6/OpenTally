# Workstream 3: Voucher Engine Specification & Lifecycle Governance

## 1. Deletion Behaviour Terminology & Provenance Distinction
Because no live TallyPrime process was executed in this research phase, all descriptions of application deletion mechanics are derived from official vendor user manuals and release documentation. They must be distinguished from OpenTally's internal accounting invariants:

```
+---------------------------------------------------------------------------------------------------+
| CLASSIFICATION                      | SPECIFICATION & LIFECYCLE BEHAVIOUR                         |
|-------------------------------------+-------------------------------------------------------------|
| OFFICIALLY_DOCUMENTED_TALLYPRIME    | - TallyPrime user manual documents Alt + D deletion.        |
| BEHAVIOUR (TallyHelp User Guide)    | - Permanently purges voucher record from active company data|
|                                     | - In Edit Log editions, records an audit log entry          |
|                                     |   (Action = "Deleted"), but data row is removed from books  |
|                                     | - Downstream numbers shift in Automatic series              |
|-------------------------------------+-------------------------------------------------------------|
| DIRECTLY_OBSERVED_BEHAVIOUR         | - NONE (Pending live runtime execution in clean-room lab)   |
|-------------------------------------+-------------------------------------------------------------|
| OPENTALLY_PROPOSED_ACCOUNTING       | - Hard deletion of POSTED financial vouchers is STRICTLY    |
| GOVERNANCE INVARIANT                |   PROHIBITED.                                               |
|                                     | - Any deletion request on a posted voucher is converted     |
|                                     |   to CANCELLED (zeroing amounts, preserving number/audit).  |
|                                     | - Unsaved Drafts may be discarded without audit logging.    |
+---------------------------------------------------------------------------------------------------+
```

## 2. Reconciled OpenTally Voucher Lifecycle States
1. **Unsaved Drafts (`DRAFT`)**:
   - In-memory UI buffer or temporary scratch table. Can be edited, discarded, or cleared without audit trail generation.
2. **Saved Unposted Records (`PENDING_APPROVAL` / `PARKED`)**:
   - Pre-commit stage for dual-authorization; non-financial; does not affect ledger balances.
3. **Posted Vouchers (`POSTED`)**:
   - Committed, balanced double-entry vouchers (`SUM(Debits) == SUM(Credits)`).
   - **Hard deletion is prohibited**.
4. **Cancelled Vouchers (`CANCELLED`)**:
   - Replicates Tally's documented `Alt + X` behavior. Retains voucher number, creation date, party identity, and immutable audit trail, while setting debit, credit, tax, and quantity values to zero.
5. **Reversed Vouchers (`REVERSED`)**:
   - Vouchers negated by a linked offsetting transaction (Credit Note, Debit Note, or Reversing Journal). Both original and reversal entries remain permanently in the general ledger.
6. **Optional Vouchers (`OPTIONAL`)**:
   - Replicates Tally's documented `Ctrl + L` behavior. Maintained in a separate scenario register; excluded from legal financial books until converted to `POSTED`.

---

## 3. Predefined Voucher Types Baseline (24 Documented in TallyPrime Release 4.1)
As confirmed by official TallyHelp documentation, TallyPrime provisions 24 predefined voucher types categorized into:
- **Accounting (10)**: Contra (`F4`), Payment (`F5`), Receipt (`F6`), Journal (`F7`), Sales (`F8`), Purchase (`F9`), Credit Note (`Alt + F6`), Debit Note (`Alt + F5`), Reversing Journal (`F10`), Memorandum (`Ctrl + F10`).
- **Inventory (8)**: Delivery Note (`Alt + F8`), Receipt Note (`Alt + F9`), Rejections In (`Ctrl + F6`), Rejections Out (`F10`), Stock Journal (`Alt + F7`), Physical Stock (`Ctrl + F7`), Material In (`Ctrl + J / F10`), Material Out (`Alt + J / F10`).
- **Orders (4)**: Sales Order (`Ctrl + F8`), Purchase Order (`Ctrl + F9`), Job Work In Order (`Ctrl + W / F10`), Job Work Out Order (`Alt + W / F10`).
- **Payroll (2)**: Attendance (`Ctrl + F5`), Payroll (`Ctrl + F4`).
