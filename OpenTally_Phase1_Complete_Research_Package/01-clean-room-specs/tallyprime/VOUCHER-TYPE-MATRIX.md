# OpenTally Version-Aware Predefined Voucher Type Matrix

## 1. Discrepancy Analysis: 16 vs 18 vs 22 vs 24 vs 28 Voucher Types
Historic documentation and third-party tutorials cite differing numbers of "predefined voucher types" in Tally. Research reconciles this version-dependent categorization:

1. **Classic Tally (16 Types)**: Core accounting vouchers (Contra, Payment, Receipt, Journal, Sales, Purchase, Debit Note, Credit Note, Memo, Reversing Journal) + Basic inventory (Delivery Note, Receipt Note, Rejections In, Rejections Out, Stock Journal, Physical Stock).
2. **Tally.ERP 9 Baseline (18 Types)**: Added formal Sales Order and Purchase Order vouchers to the standard predefined list.
3. **Tally.ERP 9 with Payroll & Job Work (22–24 Types)**: Added Payroll (Ctrl+F4), Attendance (Ctrl+F5), Job Work In Order, Job Work Out Order, Material In, and Material Out.
4. **TallyPrime Release 4.1 (24 Documented Predefined Voucher Types)**:
   Official TallyHelp documentation explicitly confirms: *"TallyPrime provides 24 predefined voucher types to record different business transactions."*
5. **Why "28 Types" Appears in Some Literature**:
   Some training syllabi count specialized system classes or legacy tax-specific types as independent voucher types:
   - POS Invoice (which is a Voucher Class under Sales).
   - Manufacturing Journal (which is a Voucher Class under Stock Journal).
   - Legacy pre-GST statutory forms (Excise Purchase, Excise Sales, Service Tax, VAT).
   - Adding these 4 sub-types/classes to the 24 base types yielded 28 in legacy educational guides.

---

## 2. Complete 24 Predefined Voucher Types in TallyPrime Release 4.1

| # | Voucher Type Name | Functional Category | Primary Shortcut | Legacy / Conflict Notes | Inventory Impact | Accounting Impact | Statutory / GST Impact | XML Type Identifier |
| :- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | **Contra** | Accounting | `F4` | Standard | None | Dr Bank/Cash, Cr Bank/Cash | None | `Contra` |
| 2 | **Payment** | Accounting | `F5` | Standard | None | Dr Expense/Liability, Cr Bank/Cash | TDS, RCM Advance | `Payment` |
| 3 | **Receipt** | Accounting | `F6` | Standard | None | Dr Bank/Cash, Cr Income/Debtor | Advance Receipt GST | `Receipt` |
| 4 | **Journal** | Accounting | `F7` | Standard | None | Dr Any, Cr Any (No Cash/Bank) | RCM Adjustments, ITC | `Journal` |
| 5 | **Sales** | Accounting / Trading| `F8` | Standard | Optional (Item Inv) | Dr Debtors/Cash, Cr Sales, Cr Tax | Outward Supply (GSTR-1) | `Sales` |
| 6 | **Purchase** | Accounting / Trading| `F9` | Standard | Optional (Item Inv) | Cr Creditors/Cash, Dr Purchase, Dr Tax| Inward Supply / ITC (GSTR-3B) | `Purchase` |
| 7 | **Credit Note** | Accounting | `Alt + F6` | In ERP9 was `Ctrl+F8` | Optional | Dr Sales Returns/Tax, Cr Debtors | Output Tax Reduction (GSTR-1 9B) | `Credit Note` |
| 8 | **Debit Note** | Accounting | `Alt + F5` | In ERP9 was `Ctrl+F9` | Optional | Dr Creditors, Cr Purchase Returns/Tax | ITC Reversal (GSTR-3B 4B) | `Debit Note` |
| 9 | **Reversing Journal** | Accounting / Scenario| `F10 > Rev Jnl` | In ERP9 was `F10` | None | Temporary scenario accrual | Excluded from legal books | `Reversing Journal` |
| 10 | **Memorandum** | Accounting / Non-Fin | `Ctrl + F10` | Standard | None | None (Suspended Register) | None | `Memorandum` |
| 11 | **Sales Order** | Order | `Ctrl + F8` | In ERP9 was `Alt+F5` | None (Commitment) | None | Order Tracking | `Sales Order` |
| 12 | **Purchase Order**| Order | `Ctrl + F9` | In ERP9 was `Alt+F4` | None (Commitment) | None | Order Tracking | `Purchase Order` |
| 13 | **Delivery Note** | Inventory | `Alt + F8` | Standard | Outward Stock Movement | None (Sales Bill Pending) | e-Way Bill tracking | `Delivery Note` |
| 14 | **Receipt Note** | Inventory | `Alt + F9` | Standard | Inward Stock Movement | None (Purchase Bill Pending) | e-Way Bill tracking | `Receipt Note` |
| 15 | **Rejections In** | Inventory | `Ctrl + F6` | Standard | Inward Stock Return | None | Reverses Delivery Note | `Rejections In` |
| 16 | **Rejections Out**| Inventory | `Alt + F6 / F10` | Clashes with Credit Note (`Alt+F6`) in Prime; accessed via `F10 > Rejections Out` | Outward Stock Return | None | Reverses Receipt Note | `Rejections Out` |
| 17 | **Stock Journal** | Inventory | `Alt + F7` | Standard | Inter-Godown Movement / Conversion | None (Transfers Valuation) | Job Work Movement | `Stock Journal` |
| 18 | **Physical Stock** | Inventory | `Ctrl + F7` | Standard | Discrepancy Adjustment | Adjusts Stock Value in BS | Inventory Discrepancy | `Physical Stock` |
| 19 | **Material In** | Inventory / Job Work| `Ctrl + J / F10` | Job Work feature | Inward Job Material | None | Job Work Register | `Material In` |
| 20 | **Material Out** | Inventory / Job Work| `Alt + J / F10` | Job Work feature | Outward Job Material | None | Job Work Register | `Material Out` |
| 21 | **Job Work In Order** | Order / Job Work | `Ctrl + W / F10` | Job Work feature | None | None | Job Work Order Register | `Job Work In Order` |
| 22 | **Job Work Out Order**| Order / Job Work | `Alt + W / F10` | Job Work feature | None | None | Job Work Order Register | `Job Work Out Order` |
| 23 | **Attendance** | Payroll | `Ctrl + F5` | Standard Payroll | None | None | Attendance records | `Attendance` |
| 24 | **Payroll** | Payroll | `Ctrl + F4` | Standard Payroll | None | Dr Salary Expense, Cr Payable/Deductions | PF/ESI/PT Contributions | `Payroll` |

---

## 3. Keyboard Shortcut Audit & Conflict Identification
- **Conflict 1 (Credit Note vs Rejections Out)**: In classic Tally.ERP 9, `Alt + F6` opened Rejections Out. In TallyPrime, `Alt + F6` was reassigned to **Credit Note**. Rejections Out was moved under the `F10: Other Vouchers` menu.
- **Conflict 2 (Debit Note vs Purchase Order)**: In classic Tally.ERP 9, `Ctrl + F9` opened Debit Note. In TallyPrime, `Alt + F5` was assigned to **Debit Note**, while `Ctrl + F9` is **Purchase Order**.
- **Unified Selection via `F10`**: TallyPrime introduced `F10` ("Other Vouchers") as a global search and selection palette to prevent keyboard shortcut collision. OpenTally will implement a command palette (`Ctrl + K` / `F10`) alongside standard shortcut bindings.
