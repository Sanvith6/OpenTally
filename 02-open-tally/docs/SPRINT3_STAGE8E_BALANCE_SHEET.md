# Sprint 3 Stage 8E — Balance Sheet

## Accounting design

The current schema has four account-group natures (`Assets`, `Liabilities`, `Income`, `Expenses`) and no independent Equity nature or retained-earnings column. Balance Sheet classification therefore uses the stored canonical Chart of Accounts hierarchy:

- `Capital Account` and descendants are Equity.
- `Fixed Assets`, `Current Assets`, and descendants are their corresponding asset sections; other asset groups are Other Assets.
- `Loans (Liability)` and descendants are Long-term Liabilities.
- `Current Liabilities` and descendants are Current Liabilities; other liability groups are Other Liabilities.
- Income and expense groups are P&L accounts and are not displayed as assets or liabilities.

These are canonical group definitions from the existing seeder, not ledger-name matching. Unsupported custom hierarchies fail closed with a report warning/error rather than being silently assigned to an invented category.

## Point-in-time calculation

For a selected company and as-of date, the service combines each ledger's stored opening balance for the company financial year with eligible voucher movements from financial-year start through the as-of date, inclusive. This avoids adding opening balances twice. The existing books-beginning validation prevents posted vouchers outside the valid book period; no schema change was required.

Only `posted` and `reversed` vouchers are included. Draft and cancelled vouchers are excluded. Reversals contribute their actual posted compensating lines. Every query is company-scoped and uses the existing cancellation-safe read-only query path.

Assets use debit-normal sign (`Dr − Cr`); liabilities and equity use credit-normal sign (`Cr − Dr`). Rows expose debit, credit, signed normal net balance, presentation amount, and balance type. No balancing ledger or artificial adjustment is inserted.

## Profit/loss and equation

Current-period P&L is calculated from financial-year start through the as-of date using the Stage 8D semantics: income is `Cr − Dr`, expenses are `Dr − Cr`, and net result is income less expenses. It is added once to equity. Existing capital/opening equity rows are not modified, and income/expense ledgers are not also included in assets or liabilities.

`Total Equity = persisted equity balances + current-period profit/loss`

`Total Liabilities + Equity = Total Liabilities + Total Equity`

`Balance Difference = Total Assets − (Total Liabilities + Total Equity)`

`is_balanced` is true only when the exact Decimal difference is zero. Accumulated earnings from earlier financial years cannot be separately identified because the current schema has no retained-earnings field; the report exposes this as a warning and reports stored opening capital/equity balances as-is.

## Export and limitations

The frontend exports company/as-of metadata, section rows, subtotals, current-period result, total liabilities plus equity, difference, and status. CSV values are exact strings, escaped, formula-injection protected, deterministic, and CRLF-delimited. The voucher export engine is not forced into this statement format.

The current implementation uses two company-scoped read-only queries and in-memory Decimal aggregation over selected ledgers and eligible lines. This is O(L + M) memory and is documented for future measured optimization. Existing database snapshot/durability guarantees are unchanged.
