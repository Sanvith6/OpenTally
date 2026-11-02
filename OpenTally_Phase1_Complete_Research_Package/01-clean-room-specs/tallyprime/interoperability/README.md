# Workstream 7: XML/HTTP Interoperability Specification

## 1. Architectural Transport Requirements vs Open Implementation Decisions

```
+---------------------------------------------------------------------------------------------------+
| ARCHITECTURAL REQUIREMENT           | STATUS: HARD INVARIANT                                      |
|                                     | OpenTally MUST provide a local HTTP server listening on TCP  |
|                                     | Port 9000 (configurable) to support Tally XML interchange.  |
+-------------------------------------+-------------------------------------------------------------+
| HTTP FRAMEWORK SELECTION            | STATUS: OPEN ARCHITECTURAL DECISION (ADR-002 PENDING)       |
|                                     | The specific Rust HTTP server library is NOT yet selected.   |
|                                     | Candidate alternatives under evaluation:                    |
|                                     |   - Alternative A: Axum (Tokio-based, robust routing)       |
|                                     |   - Alternative B: Actix-web (High throughput, mature)       |
|                                     |   - Alternative C: tiny_http (Minimalist, zero async bloat)  |
|                                     |   - Alternative D: Hyper (Low-level, maximum control)       |
|                                     | Final selection will be determined upon ADR-002 review.     |
+---------------------------------------------------------------------------------------------------+
```

## 2. Documented Protocol & XML Schema Standards
- **Transport**: HTTP/1.1 POST to `http://localhost:9000/`.
- **Headers**: `Content-Type: text/xml;charset=utf-8`.
- **Character Encoding**: UTF-8.
- **Double-Sign Inversion Invariant**:
  - Debits: `<ISDEEMEDPOSITIVE>Yes</ISDEEMEDPOSITIVE>` with negative numeric values.
  - Credits: `<ISDEEMEDPOSITIVE>No</ISDEEMEDPOSITIVE>` with positive numeric values.
  - Balance Check: `SUM(AMOUNT)` across all ledger lines must equal `0.00`.
- **Universal Envelope**:
  All requests and responses wrap within root `<ENVELOPE>` containing `<HEADER>` and `<BODY>`.
