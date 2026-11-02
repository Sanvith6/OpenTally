# OpenTally Canonical Test Scenario Register (Phase 1 Baseline)

This document establishes the canonical register of **46 unique test scenarios** across all ten workstreams.
Because no live TallyPrime process was active on the research environment, all scenarios are classified with verified pre-execution desk statuses: `DOCUMENTED_ONLY` or `NEEDS_REVIEW`. Zero tests are marked as executed.

## Summary Status Counts (Reconciled Canonical Total: 46)
- **Total Canonical Scenarios**: **46** (100% unique IDs, zero duplicates, zero missing sequences)
- **DOCUMENTED_ONLY**: **44** (Complete specification derived from official vendor documentation and statutory acts)
- **NEEDS_REVIEW**: **2** (`TC-GST-04` RCM liability timing & `TC-GST-07` Section 170 tax rounding)
- **EXECUTED / DIRECTLY_OBSERVED**: **0** (Direct behavioural verification on live process remains pending)

### Workstream Allocation Breakdown (Sum = 46)
1. **WS1: Company Management**: 4 Scenarios (`TC-COMP-01` to `TC-COMP-04`)
2. **WS2: Chart of Accounts & Masters**: 3 Scenarios (`TC-ACC-01` to `TC-ACC-03`)
3. **WS3: Voucher Engine & Numbering**: 13 Scenarios (`TC-VCH-01` to `TC-VCH-07`, `TC-NUM-01` to `TC-NUM-06`)
4. **WS4: Inventory Engine**: 8 Scenarios (`TC-INV-01` to `TC-INV-08`)
5. **WS5: GST & Taxation**: 7 Scenarios (`TC-GST-01` to `TC-GST-07`)
6. **WS6: Financial Reports**: 2 Scenarios (`TC-REP-01` to `TC-REP-02`)
7. **WS7: Interoperability (XML/HTTP)**: 4 Scenarios (`TC-XML-01` to `TC-XML-04`)
8. **WS8: Security & Audit**: 3 Scenarios (`TC-SEC-01` to `TC-SEC-03`)
9. **WS9: Banking**: 1 Scenario (`TC-BANK-01`)
10. **WS10: Payroll**: 1 Scenario (`TC-PAY-01`)
**Total**: 4 + 3 + 13 + 8 + 7 + 2 + 4 + 3 + 1 + 1 = **46 Scenarios**.

---

## Canonical Scenario Register Table

| Scenario ID | Workstream | Scenario Title | Status | Executed | Evidence Reference |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `TC-COMP-01` | WS1: Company Management | Create Company with Baseline Defaults & FY | `DOCUMENTED_ONLY` | NO | `EVD-COMP-001` |
| `TC-COMP-02` | WS1: Company Management | Alter Company Identity, Contact & State Details | `DOCUMENTED_ONLY` | NO | `EVD-COMP-001` |
| `TC-COMP-03` | WS1: Company Management | Books Beginning Forward Alteration Disallowance | `DOCUMENTED_ONLY` | NO | `EVD-COMP-001` |
| `TC-COMP-04` | WS1: Company Management | Financial Year Split Integrity & Opening Balances | `DOCUMENTED_ONLY` | NO | `EVD-COMP-001` |
| `TC-ACC-01` | WS2: Chart of Accounts | Create Customer Ledger with Bill-wise Details | `DOCUMENTED_ONLY` | NO | `EVD-ACC-001` |
| `TC-ACC-02` | WS2: Chart of Accounts | Create Supplier Ledger under Sundry Creditors | `DOCUMENTED_ONLY` | NO | `EVD-ACC-001` |
| `TC-ACC-03` | WS2: Chart of Accounts | Create Sales Account with HSN/SAC Mapping | `DOCUMENTED_ONLY` | NO | `EVD-ACC-001` |
| `TC-VCH-01` | WS3: Voucher Engine | Record Cash Sales Voucher (F8) | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-02` | WS3: Voucher Engine | Record Credit Sales Voucher with Bill Allocation | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-03` | WS3: Voucher Engine | Record Purchase Voucher (F9) with Input Tax | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-04` | WS3: Voucher Engine | Record Payment Voucher (F5) against Outstanding Bill | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-05` | WS3: Voucher Engine | Record Receipt Voucher (F6) against Outstanding Bill | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-06` | WS3: Voucher Engine | Record Contra Voucher (F4) for Cash Deposit | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-VCH-07` | WS3: Voucher Engine | Record Journal Voucher (F7) for Adjustment Entries | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-001` |
| `TC-NUM-01` | WS3: Voucher Numbering | Automatic Sequential Numbering Generation | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-NUM-02` | WS3: Voucher Numbering | Voucher Sequence Shift upon Deletion (Doc Tally) | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-NUM-03` | WS3: Voucher Numbering | Manual Numbering with Duplicate Check Enforcement | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-NUM-04` | WS3: Voucher Numbering | Automatic Numbering with Manual User Override | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-NUM-05` | WS3: Voucher Numbering | Yearly Restart Period on Financial Year Boundary | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-NUM-06` | WS3: Voucher Numbering | Multi-User Automatic Number Allocation Concurrency | `DOCUMENTED_ONLY` | NO | `EVD-VOUCH-002` |
| `TC-INV-01` | WS4: Inventory Engine | Create Stock Item with Simple Unit of Measure | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-02` | WS4: Inventory Engine | Inward Stock Purchase & Inventory Value Accretion | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-03` | WS4: Inventory Engine | Outward Stock Sale & Cost of Goods Sold Allocation | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-04` | WS4: Inventory Engine | Inter-Godown Stock Transfer via Stock Journal | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-05` | WS4: Inventory Engine | Physical Stock Adjustment & Variance Reconciliation | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-06` | WS4: Inventory Engine | FIFO Costing Method Inventory Valuation | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-07` | WS4: Inventory Engine | Batch Tracking, Manufacturing & Expiry Warning | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-INV-08` | WS4: Inventory Engine | Manufacturing Journal Assembly via BOM | `DOCUMENTED_ONLY` | NO | `EVD-INV-001` |
| `TC-GST-01` | WS5: GST & Taxation | Local Intra-State GST Supply (CGST + SGST) | `DOCUMENTED_ONLY` | NO | `EVD-TAX-001` |
| `TC-GST-02` | WS5: GST & Taxation | Inter-State Supply with IGST Allocation | `DOCUMENTED_ONLY` | NO | `EVD-TAX-001` |
| `TC-GST-03` | WS5: GST & Taxation | Inward Supply with Input Tax Credit Eligibility | `DOCUMENTED_ONLY` | NO | `EVD-TAX-001` |
| `TC-GST-04` | WS5: GST & Taxation | Inward Reverse Charge Mechanism (RCM) Supply | `NEEDS_REVIEW` | NO | `EVD-TAX-001` |
| `TC-GST-05` | WS5: GST & Taxation | Sales Credit Note Output Tax Reduction (GSTR-1 9B)| `DOCUMENTED_ONLY` | NO | `EVD-TAX-001` |
| `TC-GST-06` | WS5: GST & Taxation | Purchase Debit Note ITC Reversal (GSTR-3B 4B) | `DOCUMENTED_ONLY` | NO | `EVD-TAX-001` |
| `TC-GST-07` | WS5: GST & Taxation | Multi-Item Sub-Paise Tax Rounding Analysis | `NEEDS_REVIEW` | NO | `EVD-TAX-001` |
| `TC-REP-01` | WS6: Financial Reports | Trial Balance Debit/Credit Equilibrium Verification| `DOCUMENTED_ONLY` | NO | `EVD-REP-001` |
| `TC-REP-02` | WS6: Financial Reports | Balance Sheet Dynamic Net Profit Sync from P&L | `DOCUMENTED_ONLY` | NO | `EVD-REP-001` |
| `TC-XML-01` | WS7: Interoperability | XML Master Ledger Import via Port 9000 | `DOCUMENTED_ONLY` | NO | `EVD-XML-001` |
| `TC-XML-02` | WS7: Interoperability | XML Voucher Import with Double-Sign Inversion | `DOCUMENTED_ONLY` | NO | `EVD-XML-001` |
| `TC-XML-03` | WS7: Interoperability | Malformed XML Tag Rejection & Error Response | `DOCUMENTED_ONLY` | NO | `EVD-XML-001` |
| `TC-XML-04` | WS7: Interoperability | Unicode / Multilingual Devanagari Payload Storage | `DOCUMENTED_ONLY` | NO | `EVD-XML-001` |
| `TC-SEC-01` | WS8: Security & Audit | Role-Based Access Control (Data Entry Alter Deny) | `DOCUMENTED_ONLY` | NO | `EVD-SEC-001` |
| `TC-SEC-02` | WS8: Security & Audit | Backdated Voucher Cut-Off Lock Enforcement | `DOCUMENTED_ONLY` | NO | `EVD-SEC-001` |
| `TC-SEC-03` | WS8: Security & Audit | MCA Edit Log Multi-Version Audit Trail Tracking | `DOCUMENTED_ONLY` | NO | `EVD-SEC-001` |
| `TC-BANK-01` | WS9: Banking | Bank Reconciliation Statement (BRS) Uncleared Math| `DOCUMENTED_ONLY` | NO | `EVD-BANK-001` |
| `TC-PAY-01` | WS10: Payroll | Attendance Voucher & Auto-Payroll Calculation | `DOCUMENTED_ONLY` | NO | `EVD-PAY-001` |
