# OpenTally Sprint 6 Stage 18 — Sales Voucher Foundation Completion Report

**Status:** Complete for the authorized foundation scope
**Date:** 2026-11-15

## Delivered

- Added schema migration `0.4.5-sprint6 -> 0.4.6-sprint6` for company-scoped sales documents and lines.
- Linked each sales document to one existing accounting voucher draft; no duplicate ledger master or competing voucher lifecycle was introduced.
- Added exact Decimal sales calculations, accounting-only/inventory-linked line validation, line discounts, document discount groundwork, additional-charge groundwork, due-date defaults, explicit override provenance, list/search, update, reorder, and draft deletion.
- Registered Tauri commands and added the Sales Vouchers Gateway route and React workflow.
- Hardened all existing migration startup guards to accept later schema versions and added restart-idempotence coverage.

## Dedicated SQLite tests

- `sales_draft_creation_calculates_exact_totals_and_default_due_date` — pass
- `customer_role_active_and_company_validation_are_enforced` — pass
- `inventory_linked_line_and_discount_validation_are_supported` — pass
- `draft_update_reorder_delete_and_override_provenance_are_persisted` — pass
- `stage18_migration_chain_is_restart_idempotent` — pass

Frontend tests:

- `renders the draft workflow and deferred-policy notice` — pass
- `validates customer, series, and line description before save` — pass
- `supports accounting-only and inventory line controls` — pass

Final verification: Rust `cargo test --no-default-features` passed **392 tests** (387 pre-existing plus 5 Stage 18 tests); dedicated `sales::stage18_tests` passed **5/5**. Frontend `npm.cmd test -- --run` passed **45 tests in 10 files** (42 pre-existing plus 3 Stage 18 tests); `npm.cmd run build` passed. `cargo fmt --check`, `cargo check --no-default-features`, `cargo clippy --all-targets --no-default-features -- -D warnings`, and `git diff --check` passed. React `act(...)` warnings from existing component tests are non-failing warnings.

## Explicit limitations

No final sales accounting posting, receivable posting, inventory accounting journal, GST/statutory tax calculation, returns, credit notes, payment allocation, outstanding balance, or cross-domain workflow execution is included. Tax remains inactive pending D16-05 professional review; D16-03 and D16-04 remain deferred.
