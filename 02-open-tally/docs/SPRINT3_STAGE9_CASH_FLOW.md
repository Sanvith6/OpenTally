# Sprint 3 Stage 9 — Cash Flow Statement

## Cash/bank identification

The current schema has no separate bank classification. Cash Flow identifies accounts through stored metadata: reserved `ledger_classification = 'cash'` and ledgers under canonical `Cash-in-Hand` or `Bank Accounts` account-group ancestry. Arbitrary ledger-name matching is not used. Other cash-like custom groups remain unsupported and are reported as a limitation.

## Calculation

The service reads company-scoped `posted` and `reversed` voucher lines from financial-year start through the report end date, plus stored opening balances. Draft and cancelled vouchers are excluded. A cash debit is positive; a cash credit is negative. Opening cash is stored opening cash plus eligible cash movement before the report start. Closing cash is opening cash plus classified period activity.

For each voucher with one cash/bank line, non-cash counterpart lines are classified using canonical account-group ancestry:

- Fixed Assets and Investments → investing.
- Capital Account, Loans (Liability), and Bank OD A/c → financing.
- Other supported counterparts → operating.
- Vouchers with multiple cash/bank lines and non-cash counterparts are omitted with a warning because the current model does not provide a safe allocation rule. Pure cash/bank transfers are excluded.

`Net Change = Operating + Investing + Financing`.

`Reconciliation Difference = (Opening + Net Change) − Actual Closing Cash/Bank Ledger Balance`.

The difference is exposed without artificial balancing. All amounts use `rust_decimal::Decimal` and serialize as exact strings.

## Limitations and performance

Fixed-asset/investment, capital/loan, and cash/bank classifications are limited to canonical stored group metadata. Generic custom group semantics cannot be inferred safely. The service uses parameterized read-only queries and in-memory grouping of the selected company’s FY-to-date lines; this is O(L + M) memory. No schema migration or voucher lifecycle change was made.
