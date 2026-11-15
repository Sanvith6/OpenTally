# Sprint 5 Stage 14 — Inventory & Accounting Integration

Stage 14 connects posted inventory receipts, deliveries, and physical adjustments to the existing double-entry voucher engine.

## Architecture

`InventoryAccountingService` is a separate integration boundary. It reads immutable posted stock vouchers and Stage 12 valuation results, validates company-local account mappings, previews exact `Decimal` journal lines, and creates/post journals through `VoucherDraftService` and `VoucherPostingService`.

The `inventory_account_mappings` table stores one company-scoped configuration. The `inventory_accounting_integrations` table is the durable idempotency and retry register (`pending`, `integrated`, or `failed`). No automatic historical backfill is performed.

## Accounting effects

- Receipt: Dr Inventory Asset, Cr Receipt Offset, using quantity × supplied unit cost.
- Delivery: Dr COGS, Cr Inventory Asset, using the exact Stage 12 valuation result available to the integration.
- Physical adjustment: a positive difference is Dr Inventory Asset / Cr Adjustment Gain; a negative difference is Dr Adjustment Expense / Cr Inventory Asset.
- Transfer: inventory-only movement; no company-wide journal is created.
- Stock-voucher reversals: create a compensating accounting voucher through the existing immutable voucher reversal service. Original accounting lines are not edited.

All amounts are represented as decimal strings and the preview requires balanced debit and credit totals. Draft or cancelled stock vouchers cannot be integrated. Company IDs are included in every lookup and foreign-key relationship.

## Failure and retry behavior

The integration register is inserted before journal creation. A repeated request for an integrated or pending stock voucher returns the existing register entry. Failed entries may be retried; errors are retained in the register. A journal posting failure does not report integration success.

The register and journal operation are deliberately separate SQLite transactions because the existing voucher posting service owns its own serialized lifecycle transaction. This leaves an auditable failed attempt if journal creation fails.

## Limitations

- There is no automatic backfill of previously posted inventory.
- Transfer journals are not generated because both godowns belong to the same company and the stock ledger already records the movement.
- Delivery and adjustment amount derivation depends on the available Stage 12 as-of-date valuation snapshot. Same-day unrelated stock activity can make a date-only snapshot insufficient for perfect per-voucher cost attribution; such attribution should be extended before introducing more valuation methods.
- Account mappings are company-level. No multi-company or cross-company ledger references are permitted.
- This integration does not implement GST, manufacturing, or full invoicing.

## Verification

Backend coverage includes mapping validation, balanced receipt preview, durable idempotency, company-scoped persistence, and exact Decimal amounts. The existing Rust regression suite remains the compatibility gate; frontend coverage verifies the mapping, preview, and integration controls.
