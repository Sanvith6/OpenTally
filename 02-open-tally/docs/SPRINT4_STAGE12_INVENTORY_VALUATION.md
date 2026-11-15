# Sprint 4 Stage 12 — Inventory Valuation

## Scope

Stage 12 adds exact-decimal inventory valuation over the immutable Stage 11 stock movement ledger. It does not modify accounting vouchers, GST, manufacturing, or stock movement semantics.

## Policy

- The valuation method is configured per company and defaults to FIFO.
- A method may be selected before posted valuation history exists; changing it afterward is rejected because it would make historical reports ambiguous.
- Opening quantity and opening value are one authoritative initial layer. Opening value divided by opening quantity is the opening rate; zero quantity has zero value.
- Receipts may carry an optional non-negative unit cost and derived line value. Older quantity-only receipts remain unvalued at zero cost rather than receiving an invented cost.
- FIFO consumes oldest available valued layers first. Weighted Average Cost maintains exact quantity/value totals and applies the current average to deliveries.
- Company-wide transfers are value-neutral. Godown-level layer allocation is not inferred from company-level opening stock.
- Negative valued stock is rejected. Historical reports replay posted movements through the requested date, excluding drafts and cancelled/reversed originals.
- Decimal arithmetic uses `rust_decimal`; no floating-point calculations are used. Values are not rounded during replay.

## Persistence and queries

Migration `0.4.2-sprint4` adds receipt cost columns, valuation configuration, cost-layer, and consumption tables. The report service exposes configuration, summary, FIFO layer, and valuation-history commands. The current report replay is authoritative for reproducibility; the layer tables provide the auditable persistence boundary for future materialization.

## Known limitations

- Existing Stage 11 receipts without a unit cost are visible as quantity with zero valuation and must be corrected through a future controlled revaluation workflow.
- Opening stock is company-level because Stage 10 intentionally did not allocate it to godowns; godown valuation therefore excludes an invented opening allocation.
- Transfer and reversal lineage is currently reconstructed from immutable movements for reports; no accounting integration is performed in this stage.
