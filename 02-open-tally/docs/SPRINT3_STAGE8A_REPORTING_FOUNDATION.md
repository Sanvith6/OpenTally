# Sprint 3 Stage 8A — Reporting Foundation

Stage 8A introduces the shared backend foundation for accounting reports without implementing a report-specific screen or report calculation.

## Established contract

- `ReportDateRange` uses inclusive ISO `YYYY-MM-DD` boundaries and rejects invalid or inverted ranges.
- `ReportFilter` scopes a report to one company period and optionally one ledger.
- `ReportQueryService` executes through the existing cancellation-safe read-only SQLite connection lifecycle.
- Financial movements include voucher statuses `posted` and `reversed`; `draft` and `cancelled` rows are excluded.
- Voucher lines are returned in deterministic date, sequence, voucher, line-number, and line-ID order.
- Opening balances come from `opening_balances` for the company financial year.
- `rust_decimal::Decimal` is used for all report arithmetic; no floating-point values are introduced.

## Deliberate boundaries

Stage 8A does not implement Day Book, Ledger, Trial Balance, Profit & Loss, Balance Sheet, Cash Book, Bank Book, frontend navigation, or CSV report presentation. Those belong to Stages 8B–8F.

The current reversal model retains the original voucher as `reversed` and posts an inverted reversing voucher. Both are therefore query-visible financial activity, preserving the audit trail while netting the ledger movement. This interpretation is documented here because no separate Stage 8 clean-room report specification currently exists.

No schema migration or accounting posting behavior was changed.
