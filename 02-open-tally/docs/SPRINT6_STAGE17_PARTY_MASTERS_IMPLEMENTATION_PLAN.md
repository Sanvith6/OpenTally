# OpenTally Sprint 6 Stage 17 — Party Masters Implementation Plan

**Status:** Authorized by user on 2026-11-15  
**Scope:** Party master only  
**Excluded:** Sales/purchase posting, commercial mappings, tax, inventory journal policy, receipt/payment allocation, outstanding-balance engine, and invoice reversal workflows.

## 1. Approved architecture constraints

- Reuse one existing company-scoped accounting ledger per party.
- Support customer, supplier, or both roles through extension metadata.
- Do not create a party ledger or party balance system.
- Use the approved warning-only credit-limit behavior; never block a transaction.
- Preserve the approved future due-date rule, but do not build invoice workflows.
- Do not alter accepted Sprint 5 valuation, inventory, or accounting integration behavior.

## 2. Ledger classification decision

The current SQL schema check in `src-tauri/src/database/migrations.rs` accepts `standard`, `cash`, and `profit_and_loss`, while the Rust enum contains additional values. Stage 17 must first prove whether an additive classification migration is safe by inspecting triggers, reserved-ledger rules, existing rows, and migration tests.

If safety is not demonstrated, use extension role flags only and leave `ledgers.ledger_classification` unchanged. This is the selected safe fallback. Party profiles still reference `(company_id, ledger_id)` with a composite foreign key.

## 3. Data model

Add an additive migration from schema `0.4.4-sprint5` to `0.4.5-sprint6` containing:

```sql
CREATE TABLE party_profiles (
  id TEXT NOT NULL,
  company_id TEXT NOT NULL,
  ledger_id TEXT NOT NULL,
  party_code TEXT NOT NULL COLLATE NOCASE,
  display_name TEXT NOT NULL COLLATE NOCASE,
  customer_enabled INTEGER NOT NULL DEFAULT 0 CHECK (customer_enabled IN (0,1)),
  supplier_enabled INTEGER NOT NULL DEFAULT 0 CHECK (supplier_enabled IN (0,1)),
  contact_person TEXT,
  email TEXT,
  phone TEXT,
  billing_address TEXT,
  shipping_address TEXT,
  notes TEXT,
  credit_period_days INTEGER NOT NULL DEFAULT 0 CHECK (credit_period_days >= 0),
  credit_limit TEXT NOT NULL DEFAULT '0',
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (company_id, id),
  UNIQUE (company_id, party_code),
  UNIQUE (company_id, ledger_id),
  FOREIGN KEY (company_id, ledger_id) REFERENCES ledgers(company_id, id) ON DELETE RESTRICT,
  CHECK (customer_enabled = 1 OR supplier_enabled = 1)
);
CREATE INDEX idx_party_profiles_company_name
  ON party_profiles(company_id, display_name, is_active);
CREATE INDEX idx_party_profiles_company_roles
  ON party_profiles(company_id, customer_enabled, supplier_enabled, is_active);
```

Stage 17 keeps billing and shipping addresses as party fields because invoice workflows are explicitly deferred. A later migration may normalize addresses without changing the accounting link. Opening balances remain in existing `opening_balances`; no party balance column is added.

## 4. Backend implementation sequence

1. Add migration function and startup registration.
2. Add `PartyProfile` and `PartyRequest` Rust models.
3. Add `PartyService` with company-scoped CRUD, search, role validation, ledger validation, credit-term validation, and warning-only credit-limit metadata.
4. Add immutable audit writes for create, update, activate/deactivate, and ledger-link changes.
5. Add Tauri commands following existing active-company resolution.
6. Add dedicated SQLite integration tests.

### Service rules

- Party code and display name are trimmed and non-empty; party code is unique case-insensitively within a company.
- Linked ledger must exist in the same company and must not be reserved.
- A party may enable both roles.
- Inactive parties remain queryable but are rejected for future party-master edits that would reactivate invalid metadata; Stage 17 does not post invoices.
- Credit period is a non-negative integer.
- Credit limit is a canonical non-negative Decimal string with at most four decimal places.
- Credit-limit utilization is not calculated because no approved outstanding-balance engine exists. The UI displays configuration only and labels utilization unavailable.
- Deactivation is a metadata state change and must not mutate ledger balances or historical vouchers.

## 5. Tauri contracts

```text
list_parties(search: Option<String>) -> Vec<PartyProfile>
get_party(id: String) -> PartyProfile
create_party(request: PartyRequest) -> PartyProfile
update_party(id: String, request: PartyRequest) -> PartyProfile
set_party_active(id: String, active: bool) -> PartyProfile
```

All commands resolve the active company from `AppState`; callers do not select another company through request payloads.

## 6. Frontend implementation

Add a Party Management view to the existing Gateway/App routing pattern. It must provide:

- searchable register with role and active-state columns;
- create/edit form;
- customer, supplier, or both role selection;
- existing-ledger selection filtered to the active company;
- party code, contact person, email, phone, billing/shipping address, notes;
- credit period and credit limit fields;
- warning text that utilization is unavailable;
- active/inactive action;
- loading, empty, validation, native-runtime, and error states.

Use existing `api.ts`, TypeScript types, report/inventory styling patterns, and active-company behavior. No invoice entry or posting controls are shown.

## 7. TDD test matrix

### SQLite integration tests

- customer creation;
- supplier creation;
- customer-and-supplier dual role;
- same-company ledger link;
- cross-company ledger rejection;
- reserved-ledger rejection;
- duplicate party code rejection;
- duplicate ledger link rejection;
- party update;
- deactivation and reactivation;
- credit-period validation;
- credit-limit Decimal validation;
- warning-only metadata behavior;
- migration fresh-install and Stage 15 upgrade compatibility;
- migration failure rollback where supported;
- immutable audit records for create/update/status changes;
- company isolation for list/get/update/deactivate.

### Frontend tests

- register loading, empty, search, role/status display;
- create customer, supplier, and dual-role forms;
- ledger selection and validation errors;
- duplicate/error response display;
- update and deactivate interactions;
- credit configuration and unavailable-utilization text;
- active-company and native-runtime error states.

## 8. Verification and evidence

Run:

```text
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test inventory::stage17_tests
npm.cmd test -- --run
npm.cmd run build
git diff --check
```

The completion report must record baseline and final Rust/frontend counts, migration schema version, exact Stage 17 test names, and any unsupported ledger-classification branch.

## 9. Acceptance criteria

Stage 17 is complete only when party data is company-isolated, ledger-linked through composite foreign keys, audited, searchable, editable, deactivatable, and covered by SQLite and frontend tests. No party balance is fabricated, no credit limit blocks posting, no invoice workflow exists, and all deferred D16-03/04/05 decisions remain untouched.
