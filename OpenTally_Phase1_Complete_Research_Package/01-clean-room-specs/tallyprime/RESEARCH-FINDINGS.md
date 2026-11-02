# OpenTally Phase 1: Consolidated Research Findings

## 1. Executive Summary
Phase 1 behavioral research has established the comprehensive operational mechanics, data schemas, mathematical invariants, and protocol interfaces of TallyPrime Release 4.1. The findings provide complete clarity for designing the OpenTally clean-room SQLite engine without reliance on proprietary binaries or reverse engineering.

## 2. Core Architectural Discoveries
1. **The Continuous Ledger Model**:
   - TallyPrime does not lock historical financial years or force destructive year-end ledger balance resets.
   - Financial statements are dynamically computed views over a single continuous timeline of immutable/alterable vouchers.
2. **Double-Sign Inversion in XML Interchange**:
   - An essential finding from Workstream 7 is that Tally's XML data interchange inverts the standard sign representation: Debits are denoted with `<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>` and negative numeric amounts, while Credits are denoted with `<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>` and positive amounts. This must be honored in OpenTally's XML parser.
3. **The 28 Predefined Groups as Invariant Anchors**:
   - Every financial report (Balance Sheet, P&L, Trial Balance) strictly derives from the root categorization of the 28 predefined groups. Custom user groups must always declare a parent group that eventually resolves to one of the 15 primary groups.
4. **Edit Log Immutability (MCA Compliance)**:
   - TallyPrime Edit Log enforces permanent, tamper-evident audit logging. The OpenTally SQLite architecture will implement this via SQLite append-only audit trigger tables recording `voucher_id`, `version`, `timestamp`, `user_id`, `action`, and JSON diff payloads.
