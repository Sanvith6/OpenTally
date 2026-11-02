# Skill: REA Cleanroom Tally Research Protocol

## Overview
This skill guides the Research & Evidence Acquisition (REA) engineer in conducting non-invasive, black-box behavioural research against TallyPrime releases and public documentation.

## Evidence Confidence Hierarchy
All research findings must be classified into one of five standardized confidence levels:

1. `DIRECT_OBSERVATION`: Behavior directly executed and verified on a live, running target instance with recorded request/response or UI state transition.
2. `OFFICIAL_DOCUMENTATION`: Behavior explicitly documented in official Tally Solutions user guides, developer references, or published technical notes.
3. `INFERENCE`: Logical deduction derived from documented rules or accounting standards, but not yet directly confirmed in the target software.
4. `HYPOTHESIS`: Plausible behavioral model formulated to explain observed phenomena or edge cases; requires explicit verification.
5. `UNKNOWN`: Unresolved behavior, missing specification, or conflicting documentation that must be tested on a live instance or referred for legal/accounting review.

## Evidence Capture Procedure
1. Create a synthetic test company (e.g., "National Trade Enterprises - Synthetic FY 2024-25").
2. Isolate test parameters and document initial state.
3. Perform the discrete user action or submit the formatted XML payload.
4. Capture raw output, screen state, or HTTP response.
5. Calculate SHA-256 hash of raw artifacts.
6. Sanitize artifact of any machine-specific identifiers.
7. Save YAML record in `01-clean-room-specs/tallyprime/evidence/`.
