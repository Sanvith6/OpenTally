# OpenTally Phase 1: Comprehensive Reconciliation & Final Audit Report

**Author**: Senior Software Architect, Accounting Domain Researcher & Clean-Room REA Engineer  
**Date**: November 1, 2026  
**Status**: Final Pre-Implementation Audit Corrections Concluded  
**Scope**: Documentation Verification, Provenance Audit, Precision Policy, Scenario Harmonization & ADR Alignment  

---

## 1. Executive Summary of Audit Corrections
This report formally records the resolution of the five specific audit items required prior to Phase 2 implementation approval:
1. **Evidence Hash Provenance**: Clarified that SHA-256 hashes represent internally authored sanitized specification files, not raw external vendor documents. All 11 manifests now record original source URLs, retrieval dates, and explicitly mark local external artifact retention as `NOT_RETAINED_LOCALLY`.
2. **Canonical Scenario Harmonization (46 Scenarios)**: Reconciled the previous 42 vs 46 discrepancy. The 42 figure represented the count of physical rows in the initial table, while 46 represented the total workstream scenario allocation. `TEST-SCENARIOS.md` now contains all **46 canonical test scenarios** with complete unique IDs and metadata.
3. **Deletion Behaviour Terminology**: Corrected all references to `Alt + D` deletion, sequence shifts, and Edit Log records to explicitly label them as `OFFICIALLY_DOCUMENTED_TALLYPRIME_BEHAVIOUR`, contrasting them with OpenTally's proposed strict invariant prohibiting hard deletion of posted transactions.
4. **HTTP Framework Neutrality (ADR-002 Pending)**: Removed premature claims that Axum or Actix has been selected. Port 9000 listener is maintained as a strict requirement, while framework selection remains an open decision under ADR-002 evaluating Axum, Actix, tiny_http, and Hyper.
5. **Monetary Precision & Arithmetic Policy**: Removed hardcoded assumptions of universal four-decimal precision. Established a multi-layer precision policy distinguishing internal arithmetic (`rust_decimal`), currency minor units (e.g., 2 for INR), UoM quantity precision (0-4), intermediate tax precision, and invoice total rounding.

---

## 2. Canonical Test Scenario Register (Reconciled Count = 46)

| Workstream | Canonical IDs | Scenario Scope Summary | Documented Status | Executed Status |
| :--- | :--- | :--- | :--- | :--- |
| **WS1: Company Management** | `TC-COMP-01` to `TC-COMP-04` (4) | Creation, Alteration, Books Start Lock, FY Split | `DOCUMENTED_ONLY` (4) | 0 Executed |
| **WS2: Chart of Accounts** | `TC-ACC-01` to `TC-ACC-03` (3) | Customer, Supplier, Sales Account Masters | `DOCUMENTED_ONLY` (3) | 0 Executed |
| **WS3: Voucher Engine & Num**| `TC-VCH-01` to `07`, `NUM-01` to `06` (13)| 7 Accounting Vouchers + 6 Numbering Series Tests | `DOCUMENTED_ONLY` (13) | 0 Executed |
| **WS4: Inventory Engine** | `TC-INV-01` to `TC-INV-08` (8) | Items, Inward/Outward, Godowns, Batches, BOM | `DOCUMENTED_ONLY` (8) | 0 Executed |
| **WS5: GST & Taxation** | `TC-GST-01` to `TC-GST-07` (7) | Intra/Inter-state, ITC, RCM, Credit/Debit, Rounding| `DOCUMENTED_ONLY` (5), `NEEDS_REVIEW` (2)| 0 Executed |
| **WS6: Financial Reports** | `TC-REP-01` to `TC-REP-02` (2) | Trial Balance Equilibrium, P&L to BS Sync | `DOCUMENTED_ONLY` (2) | 0 Executed |
| **WS7: Interoperability** | `TC-XML-01` to `TC-XML-04` (4) | Port 9000, XML Import/Export, Unicode, Errors | `DOCUMENTED_ONLY` (4) | 0 Executed |
| **WS8: Security & Audit** | `TC-SEC-01` to `TC-SEC-03` (3) | RBAC Denials, Backdated Lock, Edit Log Audit | `DOCUMENTED_ONLY` (3) | 0 Executed |
| **WS9: Banking** | `TC-BANK-01` (1) | Bank Reconciliation Statement (BRS) Uncleared Math| `DOCUMENTED_ONLY` (1) | 0 Executed |
| **WS10: Payroll** | `TC-PAY-01` (1) | Attendance & Auto-Payroll Voucher Computation | `DOCUMENTED_ONLY` (1) | 0 Executed |
| **Total Canonical Scenarios**| **46 Unique Scenarios** | **100% Workstream Coverage** | **DOC: 44 \| REVIEW: 2** | **0 Executed** |

---

## 3. Evidence Manifest Provenance Audit (11 Manifests)

| Manifest ID | Workstream | Original Vendor Source / Reference | Local Artifact Status | Sanitized Spec Reference | Verified Spec SHA-256 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `EVD-COMP-001` | WS1 | TallyHelp: Company Management | Online Web Portal (Not Retained) | `company-management/README.md` | `a1fc6ef982df4b03...` |
| `EVD-ACC-001` | WS2 | TallyHelp: Chart of Accounts & Groups | Online Web Portal (Not Retained) | `accounting/README.md` | `9b2c3d551e127814...` |
| `EVD-VOUCH-001`| WS3 | TallyHelp: Manage Voucher Types | Online Web Portal (Not Retained) | `vouchers/README.md` | `77d33d964ff0fec5...` |
| `EVD-VOUCH-002`| WS3 | TallyHelp: Voucher Numbering | Online Web Portal (Not Retained) | `vouchers/VOUCHER-NUMBERING.md` | `05b10ec9e5d4cb05...` |
| `EVD-INV-001` | WS4 | TallyHelp: Inventory Management | Online Web Portal (Not Retained) | `inventory/README.md` | `fa93c66f6874ebf7...` |
| `EVD-TAX-001` | WS5 | TallyHelp: GST / CBIC CGST Act 2017 | Online Web Portal (Not Retained) | `taxation/README.md` | `e2a1492ba643cb46...` |
| `EVD-REP-001` | WS6 | TallyHelp: Financial Statements | Online Web Portal (Not Retained) | `reports/README.md` | `3720745501865bb2...` |
| `EVD-XML-001` | WS7 | Tally XML Data Interchange Reference | Online Developer Ref (Not Retained)| `interoperability/README.md` | `7be26b0a7970d249...` |
| `EVD-SEC-001` | WS8 | TallyHelp: Security / MCA Rule 3(1) | Online Web Portal (Not Retained) | `security/README.md` | `61ea3357564d682df...` |
| `EVD-BANK-001` | WS9 | TallyHelp: Banking & BRS | Online Web Portal (Not Retained) | `banking/README.md` | `018fb62bf36f959c...` |
| `EVD-PAY-001` | WS10| TallyHelp: Payroll Management | Online Web Portal (Not Retained) | `payroll/README.md` | `c51121d4512e0861...` |

---

## 4. Deletion Terminology Re-Classification
- **TallyPrime Documented Behavior**: The vendor manual documents that `Alt + D` permanently purges the transaction row from the active company data file, shifting numbers in automatic series. In Edit Log editions, an audit log entry records `Action = "Deleted"`.
- **OpenTally Target Policy**: Hard deletion of posted financial vouchers is **strictly prohibited**. A deletion operation on a posted voucher is converted to `CANCELLED` (zeroing financial and inventory balances while retaining the voucher number and audit log). Unsaved drafts can be discarded freely.

---

## 5. Architectural Alignment & Remaining Open Decisions
1. **Open Decision `UNK-ADR-002` (HTTP Daemon Framework)**: Port 9000 listener is required. Framework selection between Axum, Actix, tiny_http, and Hyper remains pending ADR-002 review.
2. **Open Decision `UNK-PREC-01` (Precision Policy)**: `rust_decimal` is adopted for all non-floating-point arithmetic. Ledgers round to currency minor units (e.g., 2 for INR), while tax intermediate calculations maintain high precision.
3. **Legal/Accounting Review Flags**:
   - `UNK-TAX-01`: Section 170 CGST Act sub-paise rounding off analysis.
   - `UNK-TAX-02`: Bill-to/Ship-to Place of Supply determination.
   - `UNK-SEC-01`: MCA Rule 3(1) Edit Log data retention upon company splitting.
   - `UNK-PAY-01`: Wage ceiling caps for EPF/ESI contributions.
   - `UNK-PII-01`: Aadhaar and banking data masking/encryption under DPDP Act, 2023.

---

## 6. Final Workstream PASS / NEEDS REVIEW / FAIL Matrix

| Workstream | Documentation Quality | Provenance Transparency | Scenario Register | Architectural Alignment | Final Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **WS1: Company Management** | PASS | PASS (`EVD-COMP-001`) | PASS (4 Scenarios) | PASS (Relational Company) | **PASS** |
| **WS2: Chart of Accounts** | PASS | PASS (`EVD-ACC-001`) | PASS (3 Scenarios) | PASS (Hierarchical Groups)| **PASS** |
| **WS3: Voucher Engine** | PASS | PASS (`EVD-VOUCH-001/002`)| PASS (13 Scenarios) | PASS (No-Hard-Delete Policy)| **PASS** |
| **WS4: Inventory Engine** | PASS | PASS (`EVD-INV-001`) | PASS (8 Scenarios) | PASS (Dynamic FIFO/UoM) | **PASS** |
| **WS5: GST & Taxation** | PASS | PASS (`EVD-TAX-001`) | NEEDS REVIEW (2 Scenarios)| PASS (`rust_decimal` Rules)| **NEEDS REVIEW** |
| **WS6: Financial Reports** | PASS | PASS (`EVD-REP-001`) | PASS (2 Scenarios) | PASS (SQL Dynamic Rollup) | **PASS** |
| **WS7: Interoperability** | PASS | PASS (`EVD-XML-001`) | PASS (4 Scenarios) | PASS (ADR-002 Undecided) | **PASS** |
| **WS8: Security & Audit** | PASS | PASS (`EVD-SEC-001`) | PASS (3 Scenarios) | PASS (Immutable Edit Log) | **PASS** |
| **WS9: Banking** | PASS | PASS (`EVD-BANK-001`) | PASS (1 Scenario) | PASS (BRS Table Model) | **PASS** |
| **WS10: Payroll** | PASS | PASS (`EVD-PAY-001`) | PASS (1 Scenario) | PASS (Statutory Config) | **PASS** |
