# Sprint 3 Stage 8F — Financial Reporting Integrity

## Scope

Stage 8F hardens the five existing reports—Day Book, Ledger Report, Trial Balance, Profit & Loss, and Balance Sheet—against one realistic SQLite fixture and verifies that they derive consistent results from the same posted accounting data.

## Reusable fixture

`reports::integration_tests` creates a temporary SQLite database, runs the current migrations, creates a company and canonical account groups, inserts asset/capital/income/expense ledgers and opening balances, then creates posted, draft, cancelled, and reversed vouchers. Each test owns a `tempfile` database and does not touch project data. Two tests cover cross-report agreement, company isolation, and as-of boundaries.

The fixture verifies:

- Opening asset and capital balances.
- Income and expense movements.
- Draft and cancelled exclusion.
- Original/reversing voucher net effect.
- Exact date boundary behavior.
- Trial Balance ↔ Ledger Report closing balance.
- Trial Balance/P&L agreement on income and expense activity.
- Trial Balance/Balance Sheet equation and one-time current-period P&L equity inclusion.
- Day Book eligible-voucher count.

## Defect found and corrected

The fixture found that Balance Sheet had a separate local current-period P&L aggregation path that diverged from Stage 8D reversal handling. Balance Sheet now reuses `ProfitAndLossService` for the current-period result and adds that result exactly once to equity. The original local P&L accumulation was removed, preventing report drift.

## Accounting limitations

The schema still has no separate Equity nature or retained-earnings field. Balance Sheet uses canonical `Capital Account` ancestry for equity and reports accumulated-earnings support as a limitation. No schema enhancement was introduced. The report never fabricates a balancing ledger or hides a nonzero difference.

## Performance observations

Reports use parameterized, company-scoped read-only queries and avoid N+1 ledger queries. Exact Decimal aggregation remains in memory over selected rows and voucher lines. Balance Sheet now performs the shared P&L query through Stage 8D for consistency; this is an intentional correctness tradeoff until a shared snapshot/query abstraction is designed. Existing query-only connection safeguards remain unchanged.
