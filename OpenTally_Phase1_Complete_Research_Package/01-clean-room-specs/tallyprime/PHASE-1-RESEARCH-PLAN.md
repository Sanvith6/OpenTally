# OpenTally Phase 1: Comprehensive Behavioural Research Plan

## 1. Objectives & Executive Summary
The primary goal of Phase 1 is to construct a complete, evidence-backed, clean-room functional knowledge base and specification package for TallyPrime Release 4.1 and Edit Log 3.0/2.1. This knowledge base serves as the unambiguous, legally uncontaminated blueprint for the Phase 2 development of OpenTally.

## 2. Research Workstream Execution Schedule

```
+-----------------------------------------------------------------------------------+
| Milestone 1: Setup, Governance, Schema & Target Environment                       |
| - Clean-room rules, REA skill, accounting governance, evidence schema             |
| - Target environment verification & synthetic company profile                     |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| Milestone 2: Functional Specifications Across All 10 Workstreams                  |
| - WS1: Company Management (Creation, Alteration, FY Split, GST Config)            |
| - WS2: Chart of Accounts (28 Groups, Ledgers, Bill-wise, Cost Centres)            |
| - WS3: Voucher Engine (18 Voucher Types, Numbering Series, Debit/Credit Postings) |
| - WS4: Inventory Engine (Items, Units, Godowns, Batches, Valuation, Movement)     |
| - WS5: GST & Taxation (Intra/Inter-State, Place of Supply, RCM, TDS, Rounding)   |
| - WS6: Financial Reports (TB, BS, P&L, Day Book, Drill-down, Cash/Funds Flow)     |
| - WS7: XML/HTTP Interoperability (Port 9000, Envelope, Master/Voucher XSD, Sync)  |
| - WS8: Security & Audit (Roles, Backdated Restrictions, Edit Log Audit Trail)     |
| - WS9: Banking (Bank Ledgers, BRS, Cheque Printing, e-Payments)                   |
| - WS10: Payroll (Employees, Pay Heads, Salary Structures, PF/ESI/PT)              |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| Milestone 3: Evidence Manifests, Test Scenarios, Gap Analysis & Exit Deliverable  |
| - Schema-compliant YAML evidence records                                          |
| - Comprehensive 38-scenario verification test matrix                              |
| - Unknowns and ambiguity register update                                          |
| - Formal Phase 1 Exit Report & Master Knowledge Base Document                     |
+-----------------------------------------------------------------------------------+
```

## 3. Methodological Rigor & Confidence Standards
Every functional statement is traceable to:
1. Official vendor documentation (User manuals, Help articles, Release notes).
2. Developer guides (Tally XML Data Interchange, ODBC interface guide).
3. Statutory tax mandates (CGST Act, MCA Audit Trail notifications).
Where observed behavior diverges from statutory expectations or vendor guides conflict, both positions are documented and registered in `UNKNOWNS.md`.
