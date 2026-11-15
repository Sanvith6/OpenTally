# Sprint 5 Stage 15 — Inventory Accounting Integrity

Stage 15 adds an immutable valuation-attribution ledger between stock movements and accounting integration.

## Design

Schema `0.4.4-sprint5` adds `inventory_valuation_attributions` and `inventory_valuation_layer_uses`. Attribution is written in the stock-posting transaction after each movement is inserted. Each record is company-scoped and uniquely tied to one stock movement, voucher line, item, valuation method, quantity, unit cost, total value, posting timestamp, and calculation context.

FIFO attribution records the source movement/voucher, quantity, unit cost, and total for every consumed layer. Weighted-average attribution records the posting-time quantity/value context and resulting cost. Physical adjustments use the posting-time available basis. Reversal attribution copies the original persisted result and links it with `reversal_of_attribution_id`; it never recalculates from current stock.

Stage 14 accounting integration now reads the persisted attribution total and method whenever the Stage 15 migration is installed. It no longer derives an individual voucher amount from date-end snapshots. The Stage 14 fallback remains only for legacy test/database states that have not run the additive migration.

## Integrity and compatibility

Attribution and movements are committed together. A unique company/movement constraint prevents duplicate attribution. Integration remains durable and idempotent: retries reuse the persisted attribution and existing accounting voucher. Existing posted history is not backfilled because exact historical layer consumption cannot be reconstructed safely; legacy records remain unavailable until reviewed.

The attribution endpoint is read-only. No automatic repair or balancing voucher is created. Existing accounting numbering, mappings, voucher immutability, and company isolation remain unchanged.

## Limitations

The current stock engine models company-level opening stock and does not allocate opening quantity to a godown. Attribution follows the existing movement ordering (`movement_date`, movement ID). Cross-godown valuation allocation and full reconciliation of every accounting ledger remain future work. GST, manufacturing, and full invoicing are outside Stage 15.

Same-day replay is explicitly ordered by `movement_date, rowid` in the attribution query. SQLite rowid is the persisted insertion key for the append-only stock-movement table; later movements therefore cannot change an already persisted attribution. A future schema revision may promote this ordering key to an explicit application column if cross-database portability requires it.

The accounting integration test boundary supports controlled test-only failures before voucher creation, during voucher creation, and after draft creation. Failed attempts are marked failed, draft cleanup is attempted for post-draft failures, and retry reuses the immutable attribution.

## Completion-pass verification

The detailed attribution inspection screen is read-only and shows voucher metadata, movement/item references, persisted quantity, unit cost, total, method, reversal link, integration status, and FIFO source-layer rows when present. Missing attribution is explicitly shown as unavailable rather than inferred.

Baseline before this pass: 368 Rust tests and 39 frontend tests. Previously added Stage 15 tests: 6. The previous acceptance gate added 3 tests; this edge-case pass added 3 dedicated SQLite tests. Final regression: 380 Rust tests passed and 39 frontend tests passed; all 12 dedicated Stage 15 tests passed. Frontend production build, `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and `git diff --check` passed. Duplicate-voucher reconciliation now uses independently persisted posted vouchers and reports both references. Concurrent integration uses two SQLite connections and returns one shared accounting voucher. Zero-value and invalid-entry cases are covered at the existing draft-validation boundary because the SQLite schema rejects those rows before persistence.
