# OpenTally – Known Unknowns & Ambiguities Ledger

This document tracks unresolved behaviors, protocol ambiguities, and unverified assumptions discovered during the research and specification of TallyPrime observable behaviors.

Each item in this ledger must be tracked until resolved with empirical evidence or explicitly classified as an accepted compatibility quirk.

---

## 1. Classification & Status Taxonomy

- **Status:**
  - `OPEN`: Newly discovered unknown; investigation not yet started.
  - `INVESTIGATING`: Active behavioral trace or packet analysis underway.
  - `RESOLVED`: Sufficient empirical evidence gathered; specification updated.
  - `ACCEPTED_QUIRK`: Confirmed observable quirk/bug in target product; documented for compatibility handling without violating internal accounting invariants.
  - `DEFERRED`: Non-critical edge case deferred to later phase.

- **Impact Area:** `INTEROP_WIRE`, `ACCOUNTING_ENGINE`, `VOUCHER_LIFECYCLE`, `INVENTORY_VALUATION`, `TAX_COMPUTATION`, `SECURITY`.

---

## 2. Active Unknowns Ledger

| Unknown ID | Category / Area | Observed Phenomenon / Ambiguity | Working Hypothesis | Clean-Room Risk / Impact | Verification Method | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :---: |
| **UNK-001** | `INTEROP_WIRE` | Behavior of HTTP XML listener on Port 9000 when receiving partial/chunked HTTP POST requests or malformed XML root tags. | Assumed to return HTTP 200 with an embedded `<STATUS>` or `<RESPONSE>` error tag rather than HTTP 4xx/5xx. | Medium: Parser crash or socket hang in OpenTally if behavior is mismatched. | PCAP capture of malformed payload responses via local test harness. | `OPEN` |
| **UNK-002** | `ACCOUNTING_ENGINE` | Rounding behavior in multi-item invoices where line-item tax sum differs by fractional paise from calculated total tax. | May use line-level half-up rounding or invoice-level cumulative rounding depending on voucher configuration flags. | High: Tax computation discrepancies between OpenTally and target exports. | Controlled test vouchers with odd-cent rates (e.g. 18% on ₹10.33 x 3 items). | `OPEN` |
| **UNK-003** | `VOUCHER_LIFECYCLE` | Exact database state of vouchers marked as "Optional" or "Memo" vouchers regarding ledger impact and numbering sequence. | Optional vouchers likely do not increment main ledger balances but may reserve or skip voucher numbering. | Medium: Broken sequence numbering or unintended ledger drift. | Export collection analysis comparing Optional vs. Regular voucher exports. | `OPEN` |
| **UNK-004** | `INTEROP_WIRE` | Character encoding behavior for non-ASCII / Unicode strings (e.g., regional Indian scripts) in XML export envelopes. | UTF-8 or legacy ANSI/Windows-1252 depending on export configuration parameters. | Medium: Text corruption in ledger names and narration fields. | XML payload inspection with UTF-8 Indian language ledger masters. | `OPEN` |
| **UNK-005** | `ACCOUNTING_ENGINE` | Cross-Nature Sub-Group Reparenting Semantics when child ledgers or opening balances already exist under the group. | TallyPrime may block moving a group across primary natures (e.g. Assets to Liabilities) if transactions exist, or recalculate Balance Sheet dynamically. | High: Discrepancies in balance sheet categorization and trial balance rollup. | Live behavioral test moving an active sub-group with balances in reference environment. | `OPEN` |
| **UNK-006** | `ACCOUNTING_ENGINE` | Duplicate Group Names Under Differing Parents (e.g. 'North Region' under Debtors vs 'North Region' under Creditors). | TallyPrime enforces globally unique group names within a company master regardless of parent group. | Medium: Scope of unique index on account_groups (global per company vs per parent). | Master creation test attempting duplicate group names across different parent nodes. | `OPEN` |
| **UNK-007** | `INTEROP_WIRE` | TDL XML Collection Request Structure for recursive Chart of Accounts Tree hierarchy vs flat master lists, and the exact XML `<PARENT>` representation for the reserved `Profit & Loss A/c` ledger. | TDL XML export returns flat `<GROUP>` and `<LEDGER>` envelopes with `<PARENT>` attributes; tree reconstruction is done on client side. Profit & Loss A/c is hypothesized to emit empty `<PARENT/>` or omit parent attribute. | Medium: Format mismatch in external backup, parent resolution errors, and integration queries. | Wireshark capture of Port 9000 collection query for Chart of Accounts. | `OPEN` |

---

## 3. Resolution Protocol

When resolving an unknown:
1. Reference the specific evidence ID (from `01-clean-room-specs/tallyprime/evidence/`) providing proof.
2. Update the status in this ledger to `RESOLVED` or `ACCEPTED_QUIRK`.
3. Link the resolution to the updated specification file in `01-clean-room-specs/tallyprime/`.
4. Never mark an unknown resolved based on speculation or undocumented memory.

