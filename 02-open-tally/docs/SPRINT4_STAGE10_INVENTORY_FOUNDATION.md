# Sprint 4 Stage 10 — Inventory Foundation

## Scope

Stage 10 adds inventory master data only. Stock vouchers, stock valuation, manufacturing, GST, and accounting integration remain future stages.

## Schema

Migration `0.3.1-sprint2 -> 0.4.0-sprint4` creates company-scoped `stock_groups`, `stock_categories`, `units`, `stock_items`, and `godowns`. Composite foreign keys prevent cross-company references. Group and godown parents use restrictive deletion; item references restrict deletion of groups, categories, and units. Search and hierarchy indexes support deterministic master lists.

Quantities, rates, values, reorder levels, and conversion factors are stored as canonical decimal text, never floating point. Application validation accepts non-negative values with at most four decimal places. Stock item opening value must equal opening quantity multiplied by opening rate exactly; no implicit rounding is performed.

Units support simple units and compound-unit metadata (`base_unit_id`, `secondary_unit_id`, `conversion_factor`). The current UI creates simple units; compound units are available through the service contract for later workflow support.

## Services and auditability

`InventoryService` provides list, create, update, activate/deactivate, and safe-delete operations for all five masters. All queries require company ID, duplicate names/symbols are case-insensitive within a company, and hierarchy updates reject cycles. Mutations append `CREATE`, `UPDATE`, or `DELETE` events to the existing immutable audit log.

## UI

The Gateway’s Inventory Info entry opens a tabbed Inventory Masters screen for Stock Groups, Categories, Units, Stock Items, and Godowns. It includes search, create/edit forms, related-master selection, activate/deactivate, delete confirmation, loading, empty, and error states. It uses existing report styling and does not alter accounting navigation.

## Limitations

Opening balances are master metadata only and do not create stock ledger movements. Alternate-unit editing and compound-unit creation are service-level capabilities but are intentionally deferred from the compact first UI. No inventory transaction or valuation behavior is implemented in this stage.
