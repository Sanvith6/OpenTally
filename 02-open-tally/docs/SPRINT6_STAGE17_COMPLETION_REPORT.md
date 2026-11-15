# OpenTally Sprint 6 Stage 17 — Party Masters Completion Report

**Status:** Complete for the authorized Party Masters scope
**Date:** 2026-11-15

## Delivered

- Additive schema migration `0.4.4-sprint5 -> 0.4.5-sprint6` creates `party_profiles` with company-scoped uniqueness, a composite `(company_id, ledger_id)` foreign key, role checks, credit-term checks, and indexes.
- Existing `ledgers.ledger_classification` is unchanged. Party roles are extension metadata because the current SQL classification constraint does not safely accept `party`.
- Company-scoped Party service supports create, list/search, get, update, and activate/deactivate. It validates same-company non-reserved ledgers, customer/supplier roles, non-negative terms, and exact Decimal credit limits (maximum four decimal places).
- Create/update/status changes write immutable `audit_log` records inside the same transaction.
- Tauri commands and the Party Masters frontend route are registered. The UI supports ledger selection, customer/supplier/dual roles, contact and address fields, credit configuration, search, edit, status changes, loading, empty, validation, and native-runtime error states.

## Explicit exclusions

No invoice or sales/purchase posting, commercial ledger mappings, tax calculation, inventory journal policy, receipt/payment allocation, outstanding-balance calculation, credit-limit blocking, due-date calculation, or reversal workflow was added. Credit limits are stored as warning-only configuration; utilization is explicitly unavailable.

## Test evidence

Dedicated SQLite integration tests:

- `customer_creation_is_audited_and_decimal_exact` — pass
- `dual_role_and_warning_only_credit_configuration_are_supported` — pass
- `company_and_reserved_ledger_isolation_is_enforced` — pass
- `duplicate_code_and_ledger_are_rejected_and_status_changes_are_audited` — pass
- `party_update_search_and_decimal_validation_are_company_scoped` — pass
- `migration_is_idempotent_and_preserves_existing_schema_data` — pass
- `migration_failure_keeps_schema_version_and_rolls_back_partial_ddl` — pass

Frontend Party Masters tests:

- `renders isolated master form, unavailable utilization notice, and empty state` — pass
- `validates required ledger, identity, and role fields before create` — pass
- `supports search input and role toggles` — pass

Full verification evidence: Rust `cargo test --no-default-features` passed **387 tests** (380 pre-existing plus 7 Stage 17 tests); dedicated `party::stage17_tests` passed **7/7**. Frontend `npm.cmd test -- --run` passed **42 tests in 9 files** (39 pre-existing plus 3 Party Masters tests), and `npm.cmd run build` passed. `cargo fmt --check`, `cargo check`, `cargo clippy --all-targets --no-default-features -- -D warnings`, and `git diff --check` passed. Existing test warnings about React `act(...)` in older inventory tests remained non-failing warnings.

## Known limitations

- No party classification migration is attempted; the extension-role fallback remains the approved safe boundary.
- No balances or credit utilization are fabricated before the approved outstanding-balance engine exists.
- The migration is additive and does not provide a downgrade operation; startup is idempotent at schema version `0.4.5-sprint6`.
- Deferred professional accounting/tax decisions D16-03, D16-04, and D16-05 remain outside this implementation.
