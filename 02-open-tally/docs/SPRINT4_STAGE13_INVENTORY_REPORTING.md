# Sprint 4 Stage 13 — Inventory Reports and Reconciliation

## Architecture

Stage 13 is read-only over the Stage 11 movement ledger and Stage 12 valuation replay. `InventoryReportsService` provides stock summary, movement register, stock ledger, and reconciliation queries. It does not create, repair, delete, or mutate stock or valuation records.

## Reports

- Stock Summary combines the authoritative stock balance service with valuation rows and warns when non-zero stock has zero or unavailable cost.
- Stock Movement Register exposes chronological movement records, including cancelled and reversed history, with voucher status and reversal references. Results are deterministically ordered and paginated at a maximum of 500 rows per page.
- Stock Ledger computes an opening balance and exact Decimal running balance for a selected item and optional godown. Cancelled movements are excluded from the running balance; reversed originals and compensating reversal movements remain visible and effective.
- Inventory Valuation reuses Stage 12 FIFO/Weighted Average Cost replay and exposes as-of date, search, unit cost, and total value.
- Reconciliation checks orphan references, duplicate movement keys, negative movement-derived quantity, and missing/zero valuation data. Findings include severity and explanation.

## CSV export

The Inventory Reports UI exports all five report views with deterministic columns, CRLF CSV rows, quote escaping, metadata-oriented filenames, and formula-injection protection. Browser exports use a download blob; native atomic destination finalization remains provided by the existing voucher export engine.

## Limitations

- Group, category, and godown filtering are fully supported by backend report filters; the compact UI currently exposes item, date, search, and zero-stock controls.
- Opening stock remains company-level because Stage 10 did not allocate opening quantity to godowns.
- Valuation reports inherit Stage 12 limitations for zero-cost legacy receipts and replay-based layer reconstruction.
- Reconciliation reports findings only; there is no automatic repair workflow.
- No accounting, GST, manufacturing, or valuation-to-ledger integration is included.
