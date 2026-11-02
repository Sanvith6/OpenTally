# OpenTally Phase 1: Research Gaps & Uncertainty Analysis (Reconciled)

## 1. Distinction Between Desk Research and Direct Verification
- **Desk Research Status**: **COMPLETED**. All 10 workstreams have comprehensive functional specifications derived from published vendor user manuals, developer references, and statutory laws.
- **Direct Behavioural Verification**: **PENDING**. Because no live TallyPrime process or listening port 9000 was active in the Linux sandbox, no empirical network requests or GUI state transitions were directly observed.
- **Evidence Provenance**: All 11 evidence manifests (`EVD-COMP-001` through `EVD-PAY-001`) are classified as `OFFICIAL_DOCUMENTATION` with verifiable SHA-256 hashes of the specification records. The raw external HTML vendor pages were accessed online and not archived locally.
- **Scenario Alignment**: All 46 canonical test scenarios are documented in `TEST-SCENARIOS.md` (44 `DOCUMENTED_ONLY`, 2 `NEEDS_REVIEW`, 0 `EXECUTED`).

## 2. Identified Functional & Empirical Gaps by Workstream

| Workstream | Canonical Scenarios | Desk Research Status | Empirical Verification | Critical Unverified Gaps |
| :--- | :--- | :--- | :--- | :--- |
| **WS1: Company Management** | 4 Scenarios (`TC-COMP-01..04`) | Completed | Pending | Order carry-forward logic during company split. |
| **WS2: Chart of Accounts** | 3 Scenarios (`TC-ACC-01..03`) | Completed | Pending | Multi-currency cost centre allocation behavior. |
| **WS3: Voucher Engine** | 13 Scenarios (`TC-VCH-01..07, NUM-01..06`)| Completed | Pending | Multi-user numbering concurrency lock behavior. |
| **WS4: Inventory Engine** | 8 Scenarios (`TC-INV-01..08`) | Completed | Pending | Negative stock interaction with moving average costing. |
| **WS5: GST & Taxation** | 7 Scenarios (`TC-GST-01..07`) | Completed | Pending | Sub-paise rounding discrepancies (Section 170). |
| **WS6: Financial Reports** | 2 Scenarios (`TC-REP-01..02`) | Completed | Pending | Drill-down performance on large datasets. |
| **WS7: Interoperability** | 4 Scenarios (`TC-XML-01..04`) | Completed | Pending | XML error schema on malformed tags or balance errors; ADR-002 pending. |
| **WS8: Security & Audit** | 3 Scenarios (`TC-SEC-01..03`) | Completed | Pending | Edit Log field diff format on narration alteration. |
| **WS9: Banking** | 1 Scenario (`TC-BANK-01`) | Completed | Pending | MT940 statement auto-reconciliation heuristics. |
| **WS10: Payroll** | 1 Scenario (`TC-PAY-01`) | Completed | Pending | Wage ceiling cap calculation rules for EPF/ESI. |
| **Total** | **46 Canonical Scenarios** | **Completed** | **Pending** | **Zero application code authored.** |
