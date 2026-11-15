# Sprint 4 Stage 11 — Stock Voucher and Movement Engine

## Architecture

Migration `0.4.0-sprint4 -> 0.4.1-sprint4` adds independent `stock_vouchers`, `stock_voucher_lines`, and immutable `stock_movements` tables. Accounting voucher tables and posting semantics are unchanged.

Movement quantities are signed base-unit Decimal text: receipts and transfer-ins are positive, deliveries and transfer-outs are negative, and physical adjustments use the signed physical-minus-system difference. Posted stock movements are the authoritative movement source; balances aggregate only movements whose voucher remains posted.

## Lifecycle and policy

Draft vouchers validate references but create no movements. Posting runs in one SQLite transaction and validates item/company ownership, base unit, godowns, precision, and delivery/transfer availability before creating movements. A repeated post is rejected because only drafts can post and movement uniqueness is scoped to voucher line, godown, and type.

Deliveries and transfers use a no-negative-stock policy. SQLite serializes the write transaction; availability is checked inside the same transaction as movement insertion, so a failed write rolls back the voucher status and all movements. Transfers write source and destination movements in the same transaction.

Cancellation marks the voucher cancelled and balances exclude its movements. Reversal creates a separate posted compensating voucher and inverse movements, then marks the original reversed; original movement rows are never updated or deleted.

## Quantity and opening stock

Stage 11 accepts only the configured base unit and Decimal quantities with at most four decimal places, constrained by the base unit precision. Alternate/compound conversion is intentionally not inferred. Stage 10 opening quantity has no godown allocation, so it is included once in company-wide balances and not assigned to any godown balance; godown availability is based on posted movements.

## UI and queries

The Inventory Transactions screen provides receipt, delivery, transfer, physical adjustment, and stock-register tabs. The register includes voucher search and godown-wise balance output. Balance queries support as-of date, item/group/category/godown/search filters and exact Decimal aggregation in Rust after one joined movement query. No valuation or monetary calculation is performed.

## Known limitations

Draft editing is available through the backend command; the compact UI currently saves and posts a single line directly. Cancellation does not create a separate compensating voucher; reversal is the compensating lifecycle operation. Opening stock remains company-level until a future opening-godown design is approved.
