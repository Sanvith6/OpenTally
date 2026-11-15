# Sprint 6 Stage 18 — Sales Voucher Foundation Implementation Plan

**Status:** Implemented for the approved foundation scope.

## Boundary

Sales documents are a company-scoped business extension linked one-to-one to an existing accounting voucher draft. The existing voucher header remains the lifecycle and future permanent-numbering source of truth. Sales-specific lines store inventory metadata and exact Decimal calculation inputs/results. Draft creation uses the existing `vt_sales` voucher type and active numbering series, but does not allocate a permanent number or sequence.

Excluded: final accounting mappings/posting, receivables, inventory accounting journals, GST/statutory tax calculation, returns, credit notes, receipt/payment allocation, and durable cross-domain posting execution.

## Schema

Migration `0.4.5-sprint6 -> 0.4.6-sprint6` adds `sales_documents` and `sales_document_lines` with composite company foreign keys, draft-only status, inactive tax marker, deterministic line ordering, due-date source/provenance, and document totals. It is additive and restart-idempotent.

## Backend

- Validates active customer-role parties in the active company.
- Supports accounting-only and inventory-linked lines.
- Validates exact non-negative Decimal quantity/rate/discount/charge values.
- Calculates gross subtotal, line/document discounts, additional charges, and net total without invented rounding.
- Calculates party-term due dates or requires a reason for explicit overrides.
- Supports create, get, list/search, full draft update, deterministic line reorder, and draft deletion.
- Writes sales-document audit records and reuses existing voucher draft deletion/header services.

## Frontend

Gateway navigation and `SalesVoucherView` provide draft register search, customer selection, series/date/reference/due-date groundwork, accounting-only or inventory-linked lines, discount controls, add/remove line controls, validation feedback, and deferred-policy messaging.

## Verification targets

Dedicated SQLite tests cover customer validation/isolation, exact totals, inventory line validation, discounts, due dates and override provenance, draft update/delete, numbering non-consumption, and migration restart idempotence. Frontend tests cover loading/empty state, validation, and line workflow controls.
