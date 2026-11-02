# OpenTally Multi-Agent Governance & Clean-Room Roles

This document defines the roles, operational boundaries, and communication protocols for agents working on the OpenTally project.

## 1. Clean-Room Agent Architecture

OpenTally employs a strict clean-room software engineering methodology to ensure complete intellectual property independence, freedom from copyright contamination, and robust functional interoperability.

```
+-------------------------------------------------------------------+
|               PHASE 1: RESEARCH & SPECIFICATION                   |
|                                                                   |
|  [Research Agent / REA]  ----->  [Evidence & Specs in 01/]        |
|  - Observes external behaviors   - Pure functional specifications |
|  - Reads official documentation  - Formal data dictionaries       |
|  - Captures sanitized XML/HTTP   - Accounting & state matrices    |
+-------------------------------------------------------------------+
                                 |
                          [CLEAN-ROOM WALL]
       No code, no proprietary binaries, no internal structures
                                 |
                                 v
+-------------------------------------------------------------------+
|               PHASE 2: IMPLEMENTATION (02-open-tally/)            |
|                                                                   |
|  [Independent Developer / Architect]                             |
|  - Reads ONLY 01-clean-room-specs/                                |
|  - Implements SQLite schema, domain engine, UI                   |
|  - Zero contact with proprietary binaries or internal code       |
+-------------------------------------------------------------------+
```

## 2. Agent Role Definitions

### 2.1 Research & Evidence Acquisition Agent (REA)
- **Scope**: External behavioral observation, desk research from published documentation, public API/interoperability analysis, and evidence capture.
- **Boundaries**: Strictly confined to `01-clean-room-specs/tallyprime/`. May NEVER create application code, Tauri configurations, database migrations, or UI components.
- **Tools**: Authorized protocol requests (HTTP/XML on designated ports), public documentation analysis, synthetic company test data capture.

### 2.2 Accounting Domain & Governance Analyst
- **Scope**: Verification of double-entry rules, debit/credit invariant checking, Indian statutory taxation principles (GST/TDS/TCS), and financial reporting reconciliations.
- **Boundaries**: Reviews behavioral findings against standard GAAP/Ind-AS and statutory rules; logs legal/statutory ambiguities in `UNKNOWNS.md`.

### 2.3 Quality Assurance & Specification Engineer
- **Scope**: Authoring reproducible test scenarios, verification matrices, XML catalog schemas, and gap analysis.
- **Boundaries**: Validates that all assertions in specifications carry verified confidence levels and evidence traceability.

## 3. Strict Prohibitions
1. No disassembly, decompilation, memory scanning, hooking, or binary reverse-engineering.
2. No access to proprietary `.900` or database storage files of proprietary applications.
3. No scraping or incorporation of third-party unlicensed reverse-engineered codebases.
4. No proprietary TDL (Tally Definition Language) code extraction.
5. No implementation work in `02-open-tally/` during Phase 1.
