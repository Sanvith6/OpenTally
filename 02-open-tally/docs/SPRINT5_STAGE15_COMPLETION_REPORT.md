# Sprint 5 Stage 15 Completion Report

Implemented the immutable inventory valuation attribution boundary.

- Added additive migration `0.4.3-sprint5` → `0.4.4-sprint5`.
- Persisted per-movement and FIFO layer-use attribution during stock posting.
- Added reversal attribution links and duplicate protection.
- Updated accounting integration to consume persisted valuation amounts.
- Added read-only attribution retrieval through Tauri and frontend API types.
- Preserved Stage 14 compatibility for databases that have not yet run Stage 15 migration.
- Added Stage 15 architecture and backfill-policy documentation.

Completion pass additions:

- Added detailed read-only valuation evidence UI and typed attribution/layer-use models.
- Added Tauri retrieval for attribution details and FIFO source layers.
- Added reconciliation findings for missing attribution and persisted valuation/accounting mismatch.
- Updated the existing frontend integration test for the Inspect workflow.

Verification baseline was 368 Rust tests / 39 frontend tests. Previously added Stage 15 tests: 6. The previous acceptance gate added 3 tests. This edge-case pass added 3 tests. Final results were 380 Rust tests passed / 39 frontend tests passed. All 12 dedicated SQLite tests passed:

1. `fifo_delivery_persists_multiple_source_layers_and_reversal_copy`
2. `weighted_average_captures_posting_time_average_on_same_day`
3. `physical_adjustments_persist_exact_difference_and_basis`
4. `attribution_is_unique_and_reconciliation_is_read_only`
5. `duplicate_attribution_is_rejected_and_missing_attribution_is_reported`
6. `stage15_decimal_assertions_are_not_float_based`
7. `accounting_failure_retry_and_repeated_integration_are_idempotent`
8. `accounting_mapping_branches_have_exact_ledgers_and_directions`
9. `reconciliation_reports_accounting_imbalance_without_repair`
10. `reconciliation_reports_independently_persisted_duplicate_accounting_vouchers`
11. `reconciliation_reports_exact_accounting_debit_credit_edge_cases`
12. `concurrent_integration_is_serialized_and_idempotent_across_connections`

Coverage now includes controlled failure before voucher creation, failure during voucher creation, failure after draft creation with draft cleanup, deterministic retry, repeated integration idempotency, reversal retry, receipt/delivery/positive-adjustment/negative-adjustment mapping directions, missing mapping rejection, independently persisted duplicate accounting vouchers, exact credit-greater-than-debit and missing-line discrepancies, read-only reconciliation, and two-connection concurrent integration. Zero-value and invalid-entry cases are covered at the existing draft-validation boundary because the SQLite schema rejects those records before persistence. The concurrency guard is process-local; SQLite uniqueness prevents duplicate integration rows, but cross-process caller timing remains a known limitation.

Exact verification results are reported with the implementation handoff. No reset, clean, staging, or commit was performed.
