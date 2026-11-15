# Sprint 3 Stage 8C — Trial Balance

## Architecture

`reports::trial_balance::TrialBalanceService` is a read-only report service. It loads the active company financial-year start, eligible ledgers and groups, opening balances, and voucher lines through the existing cancellation-safe read-only query path. Calculation is performed with `rust_decimal::Decimal`; report logic does not mutate posting tables or duplicate voucher-posting rules.

## Behavior

- The as-of date is inclusive and must be on or after the company financial-year start.
- Opening debit/credit columns come from the stored opening balance for that financial year.
- Period debit/credit columns include only voucher lines dated from financial-year start through the as-of date.
- Only `posted` and `reversed` vouchers are included. Draft and cancelled vouchers are excluded. A reversal is represented by the original reversed voucher and its posted compensating voucher, so their net accounting effect is preserved.
- Rows are company-scoped, optionally filtered by account group, ledger, or case-insensitive ledger-name search, and ordered by group name, ledger name, then ledger ID.
- Zero closing balances are hidden by default and can be included explicitly.
- Closing presentation respects account nature: Assets/Expenses are debit-normal; Liabilities/Income are credit-normal. Abnormal balances appear in the opposite column.
- Difference is `total debit - total credit` and is displayed rather than silently rounded or forced to zero.

The frontend uses the native `trial_balance` command and exports the returned exact decimal strings to CSV. It does not use JavaScript floating-point arithmetic.

## Limitations

The current report loads the selected account rows and their eligible lines into memory before calculation. Pagination and indexed aggregation can be added when report-volume measurements justify them. SQLite durability and database snapshot behavior remain those provided by the existing database manager; this report does not claim a stronger snapshot or durability guarantee.
