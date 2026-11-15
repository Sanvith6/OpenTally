# OpenTally Sprint 6 Stage 16 Architecture Decisions

**Status:** Proposed decisions for review  
**Date:** 2026-11-15  
**Deciders:** OpenTally maintainers and accounting-domain reviewers

This document records Stage 16 decisions as proposals. It does not authorize implementation or alter accepted Sprint 5 behavior.

## Decision 1 — Extend existing ledgers for parties

**Status:** proposed

### Context

The current accounting engine owns `ledgers`, voucher lines, opening balances, and balance calculations. Creating a second customer/supplier ledger system would split balances and make receivables/payables inconsistent with existing reports.

### Decision

Store party metadata in a company-scoped extension table linked to one existing ledger. Use role flags for customer, supplier, or both. The ledger remains authoritative for accounting identity and balances.

### Alternatives considered

- **Independent customer and supplier ledgers:** rejected because it duplicates accounting identity and makes double-entry balances unreliable.
- **Party fields directly on `ledgers`:** rejected because it couples operational contact/address data to the accounting master and makes one ledger’s multiple roles harder to evolve.

### Consequences

Existing reports and voucher lines remain authoritative. Party deletion is restricted by the linked ledger and financial history. A migration must reconcile the current deployed ledger-classification check before using a party classification.

## Decision 2 — Use a trade-document extension over generic vouchers

**Status:** proposed

### Context

Existing vouchers contain accounting header and ledger-line data but do not model stock item, quantity, rate, discount, due date, supplier invoice metadata, or commercial references. Adding those fields directly to generic voucher lines would weaken the voucher engine’s separation of concerns.

### Decision

Add `trade_documents` and `trade_document_lines` linked to existing accounting vouchers. Commercial facts live in the extension; the existing voucher remains authoritative for posted accounting.

### Alternatives considered

- **Add item fields to `voucher_lines`:** rejected because generic ledger lines are used by all voucher types and must remain accounting-focused.
- **Create a separate sales/purchase accounting engine:** rejected by project constraints and would duplicate posting, numbering, audit, and reversal logic.

### Consequences

The trade layer needs link consistency checks and an orchestration boundary. It gains a clean path for accounting-only documents and future tax/charge details without changing historical voucher structure.

## Decision 3 — Reuse the existing voucher numbering and lifecycle services

**Status:** proposed

### Context

`VoucherDraftService`, `VoucherPostingService`, `VoucherCancellationService`, `VoucherReversalService`, and `VoucherCorrectionService` already enforce numbering, status transitions, audit history, and immutability. A second numbering allocator would create collisions and incompatible histories.

### Decision

Sales and purchase permanent numbers come from existing voucher types and series. Trade drafts have no independent permanent number. Trade lifecycle commands delegate to the existing voucher lifecycle services and persist extension status/link evidence.

### Alternatives considered

- **A trade-specific sequence table:** rejected because it duplicates sequence recovery and collision controls.
- **Use random UUIDs as invoice numbers:** rejected because they do not satisfy existing accounting numbering behavior or user expectations.

### Consequences

Trade posting must coordinate with existing voucher posting and must handle the fact that current public services own their own transactions. A transaction-aware adapter or durable posting state machine is required.

## Decision 4 — Delegate stock and valuation to the existing inventory engine

**Status:** proposed

### Context

The inventory subsystem already owns stock movements, negative-stock validation, FIFO/WAC, attribution, reversal copying, and inventory accounting integration. Reimplementing any of these in sales/purchase code risks divergent stock and accounting balances.

### Decision

Inventory-linked sales create an existing delivery stock voucher; inventory-linked purchases create an existing receipt stock voucher. Sales/purchase code consumes persisted attribution and records typed links; it never inserts movement, cost-layer, consumption, or attribution rows directly.

### Alternatives considered

- **Write stock movements directly from invoice posting:** rejected because it bypasses existing atomic validation and attribution.
- **Recalculate COGS from current stock at invoice time:** rejected because Stage 15 requires persisted posting-time attribution.

### Consequences

Trade posting depends on a safe orchestration boundary across the existing stock and accounting transactions. Cross-godown and allocation behavior remain governed by the current inventory rules.

## Decision 5 — Keep outstanding balances ledger-derived

**Status:** proposed

### Context

Duplicated invoice totals become stale when vouchers are cancelled, reversed, corrected, or partially allocated. The accounting ledger already contains posted debit/credit facts.

### Decision

Customer and supplier outstanding groundwork uses posted ledger lines associated with party ledgers. Trade documents store due dates and invoice references, not a competing outstanding total. Receipt/payment allocation is deferred unless required by the approved posting design.

### Alternatives considered

- **Maintain a mutable `party_outstanding` total:** rejected because it can drift from posted accounting history.
- **Calculate from draft and posted trade totals:** rejected because drafts, reversals, discounts, and accounting-only entries create divergence.

### Consequences

Initial outstanding reports are ledger-level and may be explicitly unallocated. A future allocation table must be append-only and reference existing accounting voucher lines.

## Decision 6 — Tax is configuration-first and policy-gated

**Status:** proposed

### Context

The request requires tax groundwork but does not authorize a full GST engine or legal policy invention. Tax rates, applicability, rounding, jurisdiction, and ledger mapping are not yet approved in the current implementation.

### Decision

Create an extensible tax configuration/document-evidence boundary only. Do not hardcode rates or claim GST compliance. Posting must reject a tax branch when required approved configuration is missing.

### Alternatives considered

- **Hardcode GST rates and formulas now:** rejected because rates and legal rules are policy-sensitive and time-varying.
- **Ignore tax fields entirely:** rejected because commercial documents need a stable place for future tax evidence and totals.

### Consequences

Tax-inclusive pricing, tax allocation, rounding, jurisdiction, and tax-ledger mapping require explicit approval before implementation. Untaxed accounting-only and inventory-linked branches can proceed only if their policy is approved.

## Decision 7 — Prefer typed link tables over polymorphic foreign keys

**Status:** proposed

### Context

SQLite cannot enforce a foreign key whose target table varies by a type column. A single `linked_id` polymorphic table would weaken company isolation and allow orphan references.

### Decision

Use typed link records or explicit nullable composite foreign-key columns for accounting voucher, stock voucher, and inventory-accounting integration references. Every target reference includes `company_id`.

### Alternatives considered

- **One polymorphic `linked_id` plus `link_type`:** rejected because SQLite cannot enforce the target relation.
- **No persisted links, infer by reference number:** rejected because reference numbers are not a durable relational identity.

### Consequences

The schema is slightly wider, but reconciliation can identify exact missing, duplicate, cross-company, and mismatched links.

## Decision 8 — Require a shared transaction boundary or recoverable state machine

**Status:** proposed; approval required before posting implementation

### Context

The existing posting services each own a transaction. A sales/purchase posting may need to create a trade record, accounting voucher, stock voucher, valuation attribution, inventory accounting entry, and link records. Sequential calls can leave partial state after a failure.

### Decision

Before implementation, choose either transaction-aware lower-level services sharing one SQLite transaction or a durable `posting`/`failed` state machine with idempotent recovery and reconciliation. A simple sequence of public service calls is not accepted as atomic.

### Alternatives considered

- **Sequential public calls with best-effort cleanup:** rejected because cleanup cannot guarantee recovery after process termination.
- **Distributed/eventual posting without durable state:** rejected because local accounting requires deterministic user-visible lifecycle and reconciliation.

### Consequences

This is the largest Stage 16 dependency. It may require carefully extracting transaction-aware internals without changing accepted service behavior.

## Unresolved policy decisions requiring approval

1. Exact sales income, purchase, discount, charge, tax, customer, and supplier ledger mapping.
2. Whether stock-linked sales/purchases use one combined accounting voucher or linked commercial plus inventory journals.
3. Discount and additional-charge allocation across multiple stock lines.
4. Treatment of tax-inclusive prices and rounding residuals.
5. Required tax jurisdictions, registration fields, and tax applicability rules.
6. Whether credit limits block posting, warn only, or are informational.
7. Due-date behavior when credit terms and an explicit due date disagree.
8. Whether accounting-only purchase lines require a ledger per line or a document-level purchase ledger.
9. The shared transaction versus recoverable state-machine choice.
10. Whether historical vouchers may receive manually reviewed trade links; automatic backfill is not proposed.

## Review gate

No Stage 16 production implementation should begin until these decisions are reviewed, the migration target is confirmed against schema `0.4.4-sprint5`, and the transaction-boundary design is approved.

## Independent audit disposition

The source audit classified existing capabilities and proposed capabilities separately:

- **VERIFIED:** company-scoped accounting and inventory schema, existing voucher types `vt_sales`/`vt_pur`, permanent numbering services, `BEGIN IMMEDIATE` posting/cancellation/reversal, Decimal double-entry validation, stock movement posting, FIFO/WAC, immutable Stage 15 attribution, Tauri active-company command boundaries, and React `invoke`/routing patterns.
- **PROPOSED:** party extension tables, trade documents, typed link tables, sales/purchase mapping, tax groundwork, trade UI, and the posting workflow.
- **UNVERIFIED:** database support for the model enum's `party` ledger classification, a single cross-domain transaction, combined-versus-linked inventory accounting journals, and cross-process integration completion behavior.

The audit found these issues and corrected the companion documents:

| Severity | Finding | Disposition |
|---|---|---|
| CRITICAL | Existing public services do not share one transaction across accounting, stock, attribution, and inventory accounting. | The design now requires transaction-aware internals or a durable resumable workflow before posting. |
| HIGH | The base SQL ledger classification check visibly supports only `standard`, `cash`, and `profit_and_loss`; the extra Rust enum values are not evidence of deployed support. | Party classification is now an explicit migration/compatibility decision, not a verified capability. |
| HIGH | Polymorphic link IDs cannot receive SQLite target-table foreign-key enforcement. | The design now uses separate typed accounting, stock, and inventory-accounting link tables. |
| HIGH | Current inventory accounting mappings do not implement commercial sales/purchase mappings. | These remain policy-gated proposed mappings; current Stage 15 behavior is preserved. |
| MEDIUM | Current Stage 15 serialization is process-local and does not establish a cross-process completion guarantee. | Stage 16 requires durable uniqueness, workflow state, retries, and independent-connection tests. |
| LOW | Existing browser mocks are not a persistence path. | Stage 16 native persistence UI must fail explicitly outside Tauri. |

**Historical audit recommendation:** NOT READY FOR IMPLEMENTATION APPROVAL. This was superseded by the user's explicit approval record dated 2026-11-15; the deferred professional-review decisions remain excluded from implementation.

## Final decision register

No decision in this register is approved on the user's behalf. The recommendations are technical recommendations for review and do not constitute accounting or tax policy.

### D16-01 — Cross-domain posting boundary

**Exact question:** Should a sales/purchase post use one shared SQLite transaction across accounting, stock, valuation attribution, and inventory-accounting records, or use a durable multi-step workflow because the current public services own separate transactions?

**Options:**

1. **Shared transaction internals.** Extract transaction-aware internal functions and pass one SQLite transaction through all steps.
2. **Durable workflow.** Persist each committed step and resume idempotently using workflow states and typed links.
3. **Sequential public calls with cleanup.** Rejected; process termination can leave partial records and cleanup cannot prove atomicity.

| Dimension | Shared transaction | Durable workflow |
|---|---|---|
| Advantages | Strongest local atomicity; simpler final state | Fits current service boundaries; restart and retry are explicit; partial progress is observable |
| Disadvantages | Requires invasive service refactoring and careful transaction ownership | Intermediate states exist; reconciliation and recovery are mandatory |
| Accounting implication | One commit for all entries if every operation is transaction-compatible | A trade is not posted until all links exist; accounting vouchers may temporarily exist in an intermediate workflow state |
| Database/migration implication | Transaction-aware APIs; fewer workflow tables | Adds workflow table, typed links, unique idempotency keys, and recovery indexes |
| Retry/reversal implication | Rollback removes uncommitted work; reversal uses existing lifecycle after commit | Retry resumes the missing state; reversal is a new idempotent operation referencing all original links |

**Recommendation:** Option 2, durable workflow, for the first implementation. It is the smallest safe boundary compatible with the current independent service transactions. A later refactor may adopt shared transaction internals without changing the external workflow contract.

**Professional review:** Not required for the technical choice, but accounting review is required for the intermediate-state user experience.

**Approval status:** `APPROVED`  
**Approved by user:** 2026-11-15

### D16-02 — Party-ledger classification and migration

**Exact question:** How should customer/supplier parties link to accounting ledgers when the Rust model contains `party` but the deployed SQL check currently visibly accepts only `standard`, `cash`, and `profit_and_loss`?

**Options:**

1. **Add `party` to the ledger classification check through an additive table-rebuild migration.** Preserve existing values and add migration tests.
2. **Keep ledger classification unchanged and store party role only in `party_profiles`.** Use an existing permitted ledger classification and rely on the composite ledger link.
3. **Create separate customer/supplier ledgers.** Rejected; duplicates accounting identity and balances.

| Dimension | Add `party` classification | Role only in extension |
|---|---|---|
| Advantages | Explicit semantic validation and future mapping queries | Lowest schema risk; no ledger table rebuild |
| Disadvantages | Requires SQLite constraint migration and compatibility verification | Ledger classification cannot distinguish party accounts directly |
| Accounting implication | Party ledgers can be selected by classification | Party role is authoritative only in extension metadata; ledger remains balance authority |
| Database/migration implication | Additive rebuild or compatible constraint migration; preserve all rows | Only new party tables and composite foreign keys |
| Retry/reversal implication | No difference; links remain to `(company_id, ledger_id)` | No difference; lifecycle remains in voucher services |

**Recommendation:** Option 1 only if the migration can be proven safe on all supported databases; otherwise Option 2 is the fallback. In both cases, one party profile links to exactly one existing ledger, and a profile may have both role flags.

**Professional review:** Accounting review recommended for classification semantics; no tax review.

**Approval status:** `APPROVED`  
**Approved by user:** 2026-11-15  
**Restriction:** The existing SQL ledger classification is not modified unless migration safety is demonstrated. Stage 17 may use extension role flags as the safe fallback.

### D16-03 — Sales, purchase, discount, and charge mappings

**Exact question:** Which existing ledgers and debit/credit rules should map commercial sales, purchases, discounts, and additional charges?

**Options:**

1. **Company-level explicit mapping configuration.** Store party, sales, purchase, discount-allowed, discount-received, charge, cash, bank, and future tax ledger IDs with effective configuration.
2. **Infer mappings from account-group nature or ledger names.** Rejected; names and group nature are insufficient accounting policy.
3. **Hardcode default ledgers.** Rejected; violates company-specific accounting and review requirements.

| Dimension | Explicit configuration | Inference/hardcoding |
|---|---|---|
| Advantages | Auditable, company-specific, fail-closed | Less setup |
| Disadvantages | Requires configuration UI and validation | Ambiguous, unsafe, and difficult to migrate |
| Accounting implication | Mapping approval determines debit/credit direction and discount treatment | Could silently post to incorrect accounts |
| Database/migration implication | New company-scoped mapping table with composite ledger FKs and effective versions | No new table, but no reliable integrity |
| Retry/reversal implication | Persist mapping version/config reference with the workflow; reversal uses original mapping context | Recalculation could change historical meaning |

**Recommendation:** Option 1. Store the mapping/configuration identity used at posting so retries and reversals never recalculate from later settings. Do not post a branch with a missing mapping.

**Professional review:** `REQUIRED` from a qualified accounting reviewer before any mapping is enabled.

**Approval status:** `PROFESSIONAL REVIEW REQUIRED`  
**Approval status confirmed by user:** 2026-11-15

### D16-04 — Combined versus linked inventory accounting journals

**Exact question:** For inventory-linked sales/purchases, should the commercial accounting entry and inventory valuation entry be one accounting voucher or separate linked vouchers?

**Options:**

1. **One combined accounting voucher.** Party/cash, sales/purchase, discount/charge, and inventory/COGS lines post together.
2. **Linked journals.** One commercial voucher plus the existing inventory accounting integration journal, linked through typed records.

| Dimension | Combined voucher | Linked journals |
|---|---|---|
| Advantages | One visible accounting event; one balance validation | Reuses current inventory accounting service with smaller change |
| Disadvantages | Requires extending existing mapping and orchestration deeply | Two posted vouchers require link reconciliation and clear user presentation |
| Accounting implication | All lines must balance together and reverse together | Each voucher balances; combined business effect must reconcile across links |
| Database/migration implication | Trade link to one voucher plus line provenance | Multiple typed links and workflow states; no change to existing journal schema |
| Retry/reversal implication | One idempotent voucher/reversal | Each linked voucher must be reused/reversed exactly once |

**Recommendation:** Option 2 for the first implementation because it preserves accepted Stage 15 behavior and minimizes changes to inventory accounting. The UI and reconciliation must clearly present the linked journals as one commercial operation.

**Professional review:** `REQUIRED` from a qualified accounting reviewer because presentation and reversal semantics affect financial reporting.

**Approval status:** `PROFESSIONAL REVIEW REQUIRED`  
**Approval status confirmed by user:** 2026-11-15

### D16-05 — Tax pricing, applicability, rounding, and tax ledger policy

**Exact question:** Which technical tax behavior and statutory policy should govern inclusive/exclusive pricing, applicability, components, rounding, and tax-ledger mapping?

**Options:**

1. **Configuration-first tax engine.** Store rule IDs, effective dates, applicability, inclusive/exclusive mode, component rates, precision, rounding method, and mapped tax ledgers; fail closed without approved configuration.
2. **Document-only tax placeholders.** Persist tax evidence supplied by the user but do not calculate or post tax until a later approved stage.
3. **Hardcoded GST implementation.** Rejected; no rates or statutory policy may be invented here.

| Dimension | Configuration-first | Document-only placeholders |
|---|---|---|
| Advantages | Extensible and reviewable; supports future calculation | Lowest immediate statutory risk |
| Disadvantages | Requires detailed policy and rules before activation | Cannot produce calculated tax postings |
| Accounting implication | Tax lines and ledgers are generated only from approved rules | Tax amounts remain non-posting evidence until approved |
| Database/migration implication | Effective-dated rule/component/mapping tables | Smaller evidence tables; later migration required |
| Retry/reversal implication | Persist rule/version and calculated components; reverse original tax context | Reverse evidence only; no recalculation |

**Recommendation:** Option 1 as the technical target, with Option 2 as the only enabled behavior until policy review is complete. No GST rate, threshold, jurisdiction, or compliance claim is included.

**Professional review:** `REQUIRED` from qualified tax and accounting professionals.

**Approval status:** `PROFESSIONAL REVIEW REQUIRED`  
**Approval status confirmed by user:** 2026-11-15

### D16-06 — Credit-limit enforcement

**Exact question:** Should a party credit limit block posting, warn, or remain informational?

**Options:**

1. **Hard block.** Reject posting when authoritative posted ledger exposure plus the proposed document exceeds the configured limit.
2. **Warning.** Allow posting but persist an audit-visible warning.
3. **Informational only.** Display exposure without affecting posting.

**Recommendation:** Option 2 for the initial product, provided exposure is clearly labeled ledger-derived and unallocated. Hard blocking requires approved treatment of opening balances, reversals, receipts/payments, and allocations.

**Accounting implication:** No ledger entry changes; the policy affects workflow acceptance only.

**Database/migration implication:** Credit limit is party metadata; no duplicated exposure total. A read-only exposure query must use posted ledger lines.

**Retry/reversal implication:** A retry must use the same validation snapshot or explicitly revalidate policy; a reversal must never mutate the original credit-limit evidence.

**Professional review:** Accounting/business policy review recommended.

**Approval status:** `APPROVED`  
**Approved by user:** 2026-11-15  
**Restriction:** Warning-only behavior; credit-limit excess must not block transactions.

### D16-07 — Due-date calculation and overrides

**Exact question:** How should due dates be calculated from credit terms, and may an explicit invoice due date override the calculated date?

**Options:**

1. **Explicit due date wins, with validation.** Calculate a default from document date plus credit days, allow an explicit date, and retain both source and final date.
2. **Credit terms always win.** Ignore or reject explicit dates that differ.
3. **Explicit date only.** Require the user to enter a due date and do not calculate one.

**Recommendation:** Option 1. Persist `due_date_source` (`terms` or `explicit`), the applied credit-term version, and the final due date. Reject dates before the document date unless an approved policy permits them.

**Accounting implication:** Due date does not alter journal debit/credit; it affects receivable/payable aging and future allocation.

**Database/migration implication:** Add due-date source and term snapshot fields to the trade document; do not duplicate outstanding amounts.

**Retry/reversal implication:** Retry preserves the persisted due date and term snapshot. Reversal references the original due-date evidence and does not recalculate it.

**Professional review:** Accounting policy review recommended.

**Approval status:** `APPROVED`  
**Approved by user:** 2026-11-15  
**Restriction:** Explicit due-date override wins and its provenance must be persisted; invoice workflows remain deferred.

## Durable posting workflow register

The recommended workflow is not distributed atomicity. Each step commits through an existing service transaction; the workflow makes partial completion durable, observable, retryable, and reconcilable.

### States and transitions

```text
draft
  -> posting
posting
  -> accounting_posted       (accounting-only or accounting step succeeds)
  -> failed                  (validation, policy, or transient failure)
accounting_posted
  -> stock_posted            (inventory-linked stock voucher succeeds)
  -> posted                  (accounting-only links complete)
  -> failed
stock_posted
  -> inventory_accounting_posted
  -> failed
inventory_accounting_posted
  -> posted
failed
  -> posting                  (eligible retry with same idempotency key)
posted
  -> reversal_requested      (separate lifecycle operation)
reversal_requested
  -> reversed | failed
draft
  -> cancelled               (draft cancellation only)
```

`reversal_requested` is workflow state, not a new accounting status; the final trade and existing voucher lifecycle states remain authoritative.

### Idempotency and recovery rules

- Generate a UUIDv4 `post` idempotency key when the trade draft is first submitted for posting; persist it before work begins. Client retries reuse that key. Reversal uses a separate persisted operation key.
- Enforce `UNIQUE(company_id, trade_document_id)` and `UNIQUE(company_id, idempotency_key)` on workflow rows.
- Use typed links: `trade_accounting_links → vouchers`, `trade_stock_links → stock_vouchers`, `trade_inventory_accounting_links → inventory_accounting_integrations`, and line-level links from trade lines to generated stock-voucher lines. Valuation is reached through the stock movement and immutable Stage 15 attribution chain.
- Before each step, query the typed link and target status. Existing valid links are reused; missing links are created. A second process therefore cannot create a second record when the database uniqueness constraints are present.
- Track `attempt_count`, `last_error`, `last_error_class`, `started_at`, and `updated_at`. Classify errors as `validation`, `policy`, `database_transient`, `external_runtime`, or `integrity_manual_review`.
- Retry `database_transient` and recoverable `external_runtime` failures after busy-timeout/backoff. Do not automatically retry validation, policy, or integrity/manual-review failures.
- On application restart, scan nonterminal workflows. Resume only from the persisted state after rechecking every prior link; never infer completion from a reference number alone.
- Reconciliation reports a workflow whose state and links disagree, missing attribution, duplicate links, missing accounting vouchers, missing stock vouchers, or a failed workflow with partially posted records. Reconciliation never repairs records.
- Cancellation is allowed only before permanent posting or through the existing voucher cancellation service after posting. Reversal creates new accounting/stock records, copies persisted attribution through the existing reversal path, and links the new records to the original trade document.
- Every workflow transition, retry, failure, link creation, cancellation, and reversal writes immutable audit evidence containing company, trade document, operation key, prior/new state, actor, and error class.

## Exact accounting examples

All figures below are Decimal strings. Ledger names are mapping roles, not fixed IDs. Rows marked `POLICY` are not approved accounting policy.

| Example | Debit | Credit | Inventory/valuation and reversal |
|---|---|---|---|
| Credit sale with inventory: goods `100`, cost `60` | Customer `100`; COGS `60` | Sales `100`; Inventory Asset `60` | Delivery movement and persisted FIFO/WAC attribution for `60`; reversal credits customer/COGS and debits sales/inventory through new reversing records. **POLICY:** combined versus linked journals and ledger mappings. |
| Cash sale with inventory: goods `100`, cost `60` | Cash/Bank `100`; COGS `60` | Sales `100`; Inventory Asset `60` | Same delivery attribution and reversal. **POLICY:** cash/bank selection. |
| Credit purchase with inventory: goods `100` | Inventory/Purchase `100` | Supplier `100` | Receipt layer and attribution at approved unit cost; reversal credits inventory/purchase and debits supplier through new records. **POLICY:** purchase ledger and payable mapping. |
| Accounting-only sale `100` | Customer `100` | Sales `100` | No stock movement or valuation attribution. **POLICY:** party and sales ledger mapping. |
| Accounting-only purchase `100` | Purchase/Expense `100` | Supplier `100` | No stock movement or valuation attribution. **POLICY:** purchase ledger mapping. |
| Sales discount: gross `100`, discount `10`, receivable `90` | Customer `90`; Discount Allowed `10` | Sales `100` | No change to stock cost unless approved allocation says otherwise. **POLICY:** discount presentation and tax treatment. |
| Purchase discount: gross `100`, discount `10`, payable `90` | Purchase/Inventory `90` | Supplier `90` | Receipt cost is `90` only if approved net-cost policy applies. **POLICY:** discount treatment is unresolved. |
| Additional charge `5` on goods `100` | Customer/Supplier side as applicable `105` | Sales/Purchase `100`; Charge `5` | Allocation to inventory cost is policy-dependent. **POLICY:** charge classification and taxability. |
| Sales reversal of the first example | Sales `100`; Inventory Asset `60` | Customer `100`; COGS `60` | New reversal stock voucher copies original attribution; original records remain unchanged. |
| Purchase reversal of the purchase example | Supplier `100` | Purchase/Inventory `100` | New reversal receipt/delivery semantics require policy approval; original receipt attribution remains unchanged. |

## Tax policy separation

**Technical engine design:** effective-dated tax rules, components, inclusive/exclusive mode, applicability predicates, Decimal rates, explicit precision/rounding method, mapped tax ledgers, persisted rule/version IDs, and immutable calculated tax evidence.

**OpenTally product decisions:** where tax configuration is entered, whether tax is line-level or document-level in the UI, how tax evidence is displayed/exported, whether missing configuration blocks posting, and how tax components are linked to trade lines.

**Professional statutory decisions:** rates, thresholds, jurisdiction, registration treatment, place of supply, statutory document requirements, rounding/legal tolerances, return/reporting obligations, and compliance claims. No GST rate or statutory conclusion is made here.

## Final approval sheet

| Decision | Recommended option | Status | Approval consequence |
|---|---|---|---|
| D16-01 Posting boundary | Durable workflow first | `APPROVED` | Determines recovery schema and implementation boundary. |
| D16-02 Party classification | Add `party` only if migration is proven safe; otherwise extension role flags | `APPROVED` | Stage 17 uses the safe extension-role fallback unless a future migration proves safe. |
| D16-03 Commercial mappings | Explicit company-level mapping configuration | `PROFESSIONAL REVIEW REQUIRED` | Accounting review must approve directions and ledgers. |
| D16-04 Inventory journals | Linked journals for first implementation | `PROFESSIONAL REVIEW REQUIRED` | Accounting review must approve presentation and reversal semantics. |
| D16-05 Tax policy | Configuration-first target; placeholders until reviewed | `PROFESSIONAL REVIEW REQUIRED` | Tax/accounting review required before tax posting. |
| D16-06 Credit limits | Warning initially | `APPROVED` | No transaction is blocked by the stored limit. |
| D16-07 Due dates | Explicit date wins with persisted source and term snapshot | `APPROVED` | Applies to the future workflow; no invoice workflow is implemented in Stage 17. |

## Formal Stage 16 approval summary

**Approval date:** 2026-11-15  
**Stage 16 status:** **APPROVED WITH DEFERRED PROFESSIONAL ACCOUNTING/TAX DECISIONS.**

D16-01, D16-02, D16-06, and D16-07 are approved with the restrictions recorded above. D16-03, D16-04, and D16-05 remain `PROFESSIONAL REVIEW REQUIRED` and must not be implemented or silently assumed. Stage 17 Party Masters is authorized; sales/purchase posting, commercial mappings, tax, inventory journal policy, outstanding balances, and invoice reversal workflows remain excluded.

**Final status:** `APPROVED WITH DEFERRED PROFESSIONAL ACCOUNTING/TAX DECISIONS`.
