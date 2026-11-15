# OpenTally Sprint 6 Stage 16 Implementation Plan

**Status:** Proposed; implementation requires architecture review  
**Prerequisite:** Approval of the unresolved decisions in the companion architecture-decision document.

## 1. Delivery principles

- Extend existing ledgers, vouchers, numbering, stock movements, valuation, and audit services.
- Use additive SQLite migrations only; preserve existing companies and posted history.
- Keep all financial values as canonical decimal text and calculate with `rust_decimal::Decimal`.
- Make company ID an explicit input to every service query and composite foreign key.
- Build SQLite integration tests before enabling a new posting path.
- Stop a phase when its acceptance criteria and rollback tests pass; do not combine all phases into one uncontrolled change.

## 2. Phase sequence

### Phase 16A — Foundation migration and party master

Inspect and settle the deployed ledger classification values, then add:

- `party_profiles` linked to `ledgers`;
- `party_addresses`;
- `party_credit_terms`;
- indexes for company, role, active state, name, and linked ledger;
- immutable/audit requirements for posted-referenced party data;
- Rust request/response models and `PartyService`.

Commands:

- `list_parties`, `get_party`;
- `create_party`, `update_party`, `set_party_active`;
- address and credit-term operations.

Tests:

- customer, supplier, and both-role creation;
- ledger link validation;
- duplicate name/ledger rejection;
- company isolation;
- inactive-party posting rejection;
- opening balance remains ledger-authoritative.

### Phase 16B — Trade document draft foundation

Add `trade_documents`, `trade_document_lines`, `trade_document_adjustments`, and typed link storage. Implement draft-only services first:

- sales and purchase document creation;
- line add/update/delete/reorder;
- accounting-only versus inventory-linked validation;
- supplier invoice number/date for purchases;
- exact subtotal, discount, charge, tax-placeholder, and grand-total calculation;
- draft retrieval, listing, filtering, and deletion/update rules.

Do not allocate permanent voucher numbers for drafts. Display a local draft identifier until the existing voucher posting service assigns a number.

### Phase 16C — Existing voucher-type and numbering integration

Confirm existing sales/purchase voucher types and series behavior. Add only missing purchase types or configurations through migration. Implement an adapter that constructs `CreateVoucherDraftRequest`/`VoucherLineInput` for the existing voucher service.

Acceptance:

- permanent number comes from the existing series;
- no second sequence table exists;
- duplicate posting is rejected or returns the persisted result;
- cancellation, reversal, correction, and audit records use existing services.

### Phase 16D — Accounting-only sales and purchases

Implement the approved mapping service:

- resolve party ledger;
- resolve sales/purchase ledger;
- resolve approved discount/charge/tax mappings;
- create and post one existing accounting voucher;
- persist the trade-to-accounting link in the same durable lifecycle boundary;
- expose posting errors without leaving a partially posted document.

Add exact Decimal accounting examples and balance assertions for each supported branch.

### Phase 16E — Inventory-linked sales and purchases

Implement orchestration through existing stock services:

1. Validate commercial lines and stock item/unit/godown.
2. Create a stock draft using `StockVoucherRequest`.
3. Post delivery for sales or receipt for purchases through `StockVoucherService`.
4. Read persisted valuation attribution; never recompute cost from current stock.
5. Run existing inventory accounting integration.
6. Create/post the commercial accounting voucher using approved mappings.
7. Persist typed links and final status atomically or through a recoverable idempotent state machine.

The exact ordering and shared transaction mechanism must be selected in Phase 16C before implementation. Failure at any step must leave no unlinked posted trade document and no orphan stock/accounting record.

### Phase 16F — Lifecycle, register, and reconciliation

Add:

- trade register and detail queries;
- posted/cancelled/reversed status projection;
- cancellation and reversal adapters;
- link consistency reconciliation;
- due-date and ledger-derived outstanding groundwork;
- export integration using the existing export engine.

Reconciliation must be read-only and identify missing/duplicate links, mismatched totals, wrong company references, and missing valuation attribution.

### Phase 16G — Frontend integration

Build in this order:

1. Party master.
2. Sales draft/editor.
3. Purchase draft/editor.
4. Register and detail view.
5. Posting/lifecycle status and errors.
6. Inventory-linked line controls and valuation evidence.

Reuse `App.tsx` view routing, Gateway entries, existing form/table CSS, active company context, typed `api.ts` wrappers, and keyboard conventions. Add frontend tests for validation, draft actions, filters, detail links, loading, and error states.

## 3. Proposed service boundaries

```text
PartyService
TradeDocumentDraftService
TradeDocumentQueryService
TradeTotalsService        (pure Decimal calculations)
TradePostingService       (orchestrator/state machine)
TradeAccountingMapper     (policy/config resolution)
TradeInventoryLinkService (adapter to StockVoucherService)
TradeLifecycleService     (cancel/reverse/correct adapters)
TradeReconciliationService
```

`TradeTotalsService` must not write to SQLite. `TradeAccountingMapper` must not post vouchers. `TradePostingService` is the only coordinator allowed to transition a trade document to posted.

## 4. Proposed Tauri command contracts

Names are proposed and must follow the existing active-company command pattern.

```text
list_parties(filter) -> Vec<PartySummary>
get_party(id) -> PartyDetails
create_party(request) -> PartyDetails
update_party(id, request) -> PartyDetails
set_party_active(id, active) -> PartyDetails

create_trade_draft(request) -> TradeDocumentDetails
get_trade_document(id) -> TradeDocumentDetails
list_trade_documents(filter) -> PaginatedResponse<TradeRegisterItem>
update_trade_draft(id, request) -> TradeDocumentDetails
delete_trade_draft(id) -> ()
post_trade_document(id) -> TradeDocumentDetails
cancel_trade_document(id, reason) -> TradeDocumentDetails
reverse_trade_document(id, request) -> TradeDocumentDetails
get_trade_links(id) -> TradeLinkDetails
get_trade_reconciliation(filter) -> Vec<ReconciliationFinding>
```

Requests must carry date strings and decimal strings; responses must not expose Rust floating-point values. Error responses use the existing `OpenTallyError` serialization.

## 5. Migration rollout

1. Verify current metadata version is `0.4.4-sprint5` in the real startup path.
2. Add the Stage 16 migration as a new additive version; do not rewrite earlier migration SQL.
3. Create tables and indexes in one transaction.
4. Add idempotent startup handling and migration tests for fresh install and upgrade from existing Stage 15 databases.
5. Do not backfill parties or trade documents from historical vouchers without an explicit, auditable policy.
6. Keep foreign keys enabled and use composite `(company_id, id)` references.

## 6. Test plan by phase

### Database and master tests

- fresh migration and upgrade convergence;
- rollback on migration failure;
- company-scoped uniqueness and foreign-key isolation;
- party-to-ledger link and deletion restrictions;
- address/default and credit-term constraints.

### Draft and arithmetic tests

- accounting-only and inventory-linked drafts;
- exact rate × quantity − discount + charges arithmetic;
- date boundaries and due-date validation;
- line ordering and deterministic totals;
- invalid/negative/non-canonical decimal rejection.

### Posting integration tests

- sales accounting-only balance;
- purchase accounting-only balance;
- sales delivery with persisted FIFO/WAC attribution;
- purchase receipt with persisted layer attribution;
- permanent numbering from existing series;
- duplicate posting idempotency;
- rollback after accounting, stock, valuation, or link failure;
- company isolation.

### Lifecycle tests

- cancellation and reversal through existing services;
- original accounting and stock movements remain immutable;
- reversal links are one-to-one;
- audit entries identify trade, accounting, and stock records;
- read-only reconciliation detects broken links.

### Frontend tests

- party roles and ledger selection;
- sales/purchase draft forms;
- accounting-only versus stock-linked validation;
- exact displayed totals as strings/Decimal.js values;
- register filtering and detail navigation;
- loading, native-runtime, and error states.

## 7. Acceptance criteria

Stage 16 implementation is acceptable only when:

- parties reuse existing ledgers and are company-isolated;
- sales and purchases support accounting-only and inventory-linked drafts;
- permanent numbering comes exclusively from existing voucher services;
- posting is atomic or durably recoverable and idempotent;
- inventory movements and valuation attribution are created only by existing inventory services;
- accounting entries balance exactly and use approved mappings;
- cancellation/reversal preserve immutable history;
- due dates and invoice references are persisted without duplicated outstanding totals;
- migration, backend integration, frontend, and rollback tests pass;
- unresolved tax and ledger-policy decisions are either approved or the affected branch is explicitly disabled.

## 8. Stop condition

Do not begin Stage 16 implementation until the architecture and decision documents have been reviewed and the unresolved policy decisions have explicit answers.

## 9. Audit corrections and implementation gate

The independent audit verified the existing voucher, numbering, double-entry, inventory, valuation, attribution, integration, migration, Tauri, and React boundaries against the source tree. It also found that the current public accounting and stock services own separate transactions; therefore Phase 16E must not describe sequential service calls as atomic.

The corrected Phase 16E gate is:

1. Add a durable `trade_posting_workflows` row with a unique company/document/idempotency key.
2. Claim or resume the workflow under a SQLite write transaction.
3. Before each step, inspect typed links for an already-created accounting voucher, stock voucher, or inventory integration.
4. Execute the next step through transaction-aware internals if approved; otherwise persist the next state after each committed step.
5. Retry only the missing step; never regenerate an existing linked record.
6. Mark the trade document posted only after every required link and valuation attribution is present.
7. Mark failed attempts with an error and expose them to read-only reconciliation.

No acceptance criterion may rely on a process-local mutex as the sole cross-connection or cross-process protection. Database uniqueness, durable workflow state, and integration tests using independent SQLite connections are required.

### Audit issue classification

- **CRITICAL:** No current implementation provides one atomic transaction across trade accounting, stock movement, attribution, and inventory accounting. Resolved in the plan by requiring shared transaction internals or a durable idempotent workflow before posting implementation.
- **HIGH:** The current SQL ledger classification constraint does not visibly accept the additional `party` enum value. Resolved as an explicit Phase 16A migration/compatibility gate; not claimed as currently available.
- **HIGH:** Current inventory accounting mappings do not cover sales income, purchase payable, discounts, charges, or tax. Kept as proposed policy/configuration work and blocked on approval.
- **HIGH:** A polymorphic link ID would weaken SQLite referential integrity. Corrected to typed link tables with composite company-scoped foreign keys.
- **MEDIUM:** Cross-process integration completion ordering is not verified by current Stage 15. Stage 16 requires durable database idempotency and independent-connection retry tests.
- **LOW:** Existing browser-only API mocks do not provide native persistence. Stage 16 persistence screens must fail explicitly outside the Tauri runtime rather than fabricate records.

### Audit readiness recommendation

The original audit marked this plan **NOT READY FOR IMPLEMENTATION APPROVAL**. The user's 2026-11-15 approval record supersedes that gate for D16-01, D16-02, D16-06, and D16-07; D16-03, D16-04, and D16-05 remain excluded pending professional review.

## 10. Final approval gate

Implementation is blocked until the user explicitly reviews the final register in `SPRINT6_STAGE16_ARCHITECTURE_DECISIONS.md`:

- D16-01: posting boundary;
- D16-02: party-ledger classification;
- D16-03: commercial ledger mappings;
- D16-04: combined versus linked inventory journals;
- D16-05: tax policy and calculation boundary;
- D16-06: credit-limit enforcement;
- D16-07: due-date and override behavior.

This plan does not itself grant approval. The separate user approval record authorizes D16-01, D16-02, D16-06, and D16-07. D16-03, D16-04, and D16-05 require professional accounting/tax review before implementation.

## 11. Approval record

**Approval date:** 2026-11-15  
**Stage 16 status:** **APPROVED WITH DEFERRED PROFESSIONAL ACCOUNTING/TAX DECISIONS.**

The user approved D16-01 (durable idempotent workflow), D16-02 (safe `party` migration only when demonstrated, otherwise extension roles), D16-06 (warning-only credit limits), and D16-07 (explicit due-date override with provenance). D16-03, D16-04, and D16-05 remain professional-review gates and are excluded from Stage 17.

Stage 17 is authorized only for Party Masters. It must not create invoice posting, commercial ledger mappings, tax calculations, inventory journal policy, receipt/payment allocation, outstanding-balance computation, or sales/purchase reversal workflows.
