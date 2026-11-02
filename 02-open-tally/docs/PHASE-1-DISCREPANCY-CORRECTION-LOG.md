# OpenTally — Phase 1 Discrepancy & Correction Log

- **Document Version:** 1.0.0
- **Date:** 2026-11-01
- **Status:** **Active Baseline for Phase 2**
- **Scope:** Reconcile numerical and architectural discrepancies identified between initial Phase 1 summary artifacts and final canonical registers without mutating historical research records.

---

## 1. Summary of Identified Discrepancies

During the pre-implementation audit transitioning from Phase 1 (Research) to Phase 2 (Implementation), four critical discrepancies and open decision states were identified between historical summary artifacts and canonical registers:

| Dimension | Historical / Summary Document | Canonical / Reconciled Document | Reconciled Status for Phase 2 | Reason / Explanation |
| :--- | :--- | :--- | :---: | :--- |
| **Test Scenarios** | 38 Scenarios (`PHASE-1-EXIT-REPORT.md`, `Knowledge_base2.md`) | 46 Scenarios (`PHASE-1-RECONCILIATION-REPORT.md`, `TEST-SCENARIOS.md`) | **46 Canonical Scenarios** | The initial 38 figure counted preliminary scenario groups. The canonical register expanded granular coverage across all 10 workstreams (e.g. 6 dedicated numbering series scenarios `NUM-01` to `NUM-06`). |
| **Predefined Voucher Types** | 18 Types (`PHASE-1-RESEARCH-MATRIX.md`) | 24 Types (`VOUCHER-TYPE-MATRIX.md`, `PHASE-1-RECONCILIATION-REPORT.md`) | **24 Canonical Voucher Types** | 18 represented the legacy Tally.ERP 9 baseline. TallyPrime Release 4.1 officially defines 24 predefined voucher types (including Order, Inventory, Job Work, and Payroll types). |
| **Scenario Execution Status** | "Completed" in broad task tracking | 0 Executed, 44 Documented, 2 Needs Review | **0 Executed (Documented Only)** | No live Windows `Tally.exe` instance or port 9000 service was active in the research environment. All specifications are derived from official vendor documentation and statutory references. |
| **HTTP XML Framework** | Mention of Axum / Actix in early notes | Neutral / Undecided (`ADR-002 Pending`) | **Pending Review** | Port 9000 listener is a fixed requirement, but runtime framework selection between Axum, Actix, tiny_http, and Hyper remains pending ADR-002 review. |

---

## 2. Status of Architecture Decision Records (ADRs)

| ADR ID | Title | Status | Notes |
| :--- | :--- | :---: | :--- |
| **ADR-001** | Embedded Database Engine Selection (SQLite + WAL + SQLCipher) | **Accepted** | Formalized in `02-open-tally/docs/ADR-001-embedded-database-selection.md`. Adopts SQLite with WAL and SQLCipher via `sqlx` and `rust_decimal`. |
| **ADR-002** | HTTP/XML Daemon Framework Selection (Port 9000) | **Pending** | Pending evaluation of Axum, Actix-web, Hyper, and tiny_http. Not implemented in Sprint 0. |
| **ADR-003** | Frontend State Management Architecture | **Pending** | Evaluating Zustand + TanStack Query vs Redux Toolkit. Not finalized in Sprint 0. |

---

## 3. Policy for Phase 2 Implementation

1. **Canonical Reference:** Phase 2 implementation must treat `PHASE-1-RECONCILIATION-REPORT.md`, `TEST-SCENARIOS.md` (46 scenarios), and `VOUCHER-TYPE-MATRIX.md` (24 types) as the definitive behavioral references.
2. **Preservation of History:** Historical reports (`PHASE-1-EXIT-REPORT.md`, `PHASE-1-RESEARCH-MATRIX.md`) remain preserved as historical research milestones. No historical files are silently overwritten.
3. **No Speculative Implementation:** Features subject to pending ADRs or open questions in `UNKNOWNS.md` (e.g., sub-paise rounding off under Section 170 of CGST Act, or HTTP daemon framework) must not be implemented until formal approval.

