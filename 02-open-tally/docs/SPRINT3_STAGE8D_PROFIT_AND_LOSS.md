# Sprint 3 Stage 8D — Profit & Loss

## Scope and architecture

`reports::profit_and_loss::ProfitAndLossService` is a read-only report service. It queries company-scoped income and expense ledgers and their eligible voucher lines in two parameterized queries, then aggregates exact `rust_decimal::Decimal` values in Rust. It does not change voucher posting, cancellation, reversal, numbering, or the SQLite schema.

## Classification

Classification uses stored Chart of Accounts fields, not display-name matching:

- `account_groups.nature = Income` identifies income accounts.
- `account_groups.nature = Expenses` identifies expense accounts.
- `account_groups.affects_gross_profit = 1` identifies direct groups.
- `account_groups.affects_gross_profit = 0` identifies indirect groups.
- Hidden groups are excluded.

This matches the seeded canonical groups (`Direct Incomes`, `Sales Accounts`, `Direct Expenses`, `Purchase Accounts`, and their indirect counterparts) while allowing their stored child groups and ledgers to be reported. A custom group is classified by its persisted inherited accounting semantics, not its name.

## Formulas

For each ledger, with exact decimal movement totals:

- Income contribution = credit − debit.
- Expense contribution = debit − credit.
- Direct income subtotal = sum of direct income contributions.
- Direct expense subtotal = sum of direct expense contributions.
- Gross result = direct income subtotal − direct expense subtotal.
- Net result = gross result + indirect income subtotal − indirect expense subtotal.

Result amounts are serialized as non-negative exact decimal strings with a separate `Profit`/`Loss` type. Ledger contributions remain signed so abnormal entries are not hidden or forced to balance.

## Voucher and period behavior

The requested date range is inclusive and validated. Only `posted` and `reversed` vouchers are read; drafts and cancelled vouchers are excluded. Reversal vouchers are included with their actual inverted lines, preserving the existing reporting convention and net effect. Opening balances are intentionally excluded because P&L reports period activity rather than brought-forward balance-sheet positions.

Every query applies the selected company ID. Optional account-group and case-insensitive ledger-name filters are applied in SQL. Rows are ordered by account-group name, ledger name, and ledger ID. Zero-contribution rows are hidden unless explicitly requested.

## CSV export and limitations

The frontend exports the structured report with company/period metadata, section headings, rows, subtotals, gross result, and net result. Values are escaped, formula-injection protected, and emitted with CRLF line endings. The existing voucher export engine is intentionally not forced to serialize this different report shape.

The current implementation loads the selected ledger rows and eligible lines into memory for exact aggregation. This avoids unsafe SQLite text aggregation but is O(L + M) memory for L ledgers and M voucher lines; database-side aggregation or pagination can be considered after measured report-volume requirements. Durability and snapshot guarantees remain those of the existing database manager.
