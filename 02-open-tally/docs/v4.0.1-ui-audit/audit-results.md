# OpenTally 4.0.1 live UI audit

Native Tauri/WebView2 CDP run. Viewports: 1280×800 and 1440×900.

| Step | Result | Check | Notes |
|---:|:---:|---|---|
| 1 | PASS | Startup & Onboarding (SelectCompanyView -> Sample Company & 2-Step F11) | Sample company & 2-step F11 GSTIN auto-detection verified |
| 2 | PASS | Top Utility Bar, Gateway Uniqueness, FunctionKeyBar & F11 Esc unwind | Top bar, 40 unique hotkeys, FunctionKeyBar & F11 Esc unwind verified |
| 3 | PASS | Chart of Accounts ([A]) | 29 predefined groups & ledgers rendered cleanly |
| 4 | PASS | Core Vouchers (F4/F5/F6/F7) + Signature Color Ribbons | F4/F5/F6/F7 ribbons & shortcuts switched cleanly without page reload |
| 5 | PASS | Commercial Vouchers (F8 Sales & F9 Purchase) + Ribbons | F8 Sales & F9 Purchase ribbons rendered cleanly with 0 NaN/overflow |
| 6 | PASS | Credit/Debit Notes (Alt+F6/Alt+F5) & Stock Journal (Alt+F7) | Alt+F6, Alt+F5, Alt+F7 Stock Journal rendered cleanly |
| 7 | PASS | Order Processing (Ctrl+F8 / Ctrl+F9) | Order Processing (Ctrl+F8) rendered cleanly |
| 8 | PASS | Financial Statements (TB, P&L, BS, DayBook) & Voucher Detail Inspector | Trial Balance & Day Book -> Voucher Detail Inspector verified |
| 9 | PASS | Outstanding & Aging + Banking & BRS | Outstanding & Aging and Bank Reconciliation rendered cleanly |
| 10 | PASS | Indian GST Reports (GSTR-1/3B/2B) & TDS Reports (26Q) | GST Reports & TDS Reports rendered cleanly |
| 11 | PASS | Inventory Pro (Godown, Batch) & Manufacturing BOM via Alt+G | Godown Summary, Batch Expiry & Manufacturing BOM verified via Alt+G |
| 12 | PASS | Management Accounting (Ratio Analysis, Cost Centres & Budgets) | Ratio Analysis, Cost Centres & Budgets verified (0 NaN/Infinity) |
| 13 | PASS | Indian Payroll (Ctrl+F4) - Employee, LOP Preview & Payslip | Payroll F11 seeding, Employee CRUD, LOP preview, GL post & Payslip PDF verified |
| 14 | PASS | Year-End Closing & MCA 2023 Forensic Audit Trail Certification | MCA 2023 Audit Trail verified clean with 0 broken hash links |
| 15 | PASS | Alt+G Go-To, Alt+I Tally XML, Alt+B Backup & Security Control (0) | Alt+G, Alt+I, Alt+B & Security Control (0) Esc unwinding verified cleanly |
| 16 | PASS | Statements & Ledgers (BS [B], P&L [P], Cash Flow [C], Ledger [L]) | Balance Sheet, P&L, Cash Flow & Ledger Report verified cleanly via unique hotkeys |
| 17 | PASS | Milestone 16: Multi-Currency, Forex & Cheque Printing UI ([9]) | Multi-Currency, Forex & Cheque Printing UI verified cleanly |
| 18 | PASS | Milestone 17: LAN Host / Multi-User Gold Modal ([-]) | LAN Host Modal (port 7900, Zero-Footprint default, Esc close) verified cleanly |
| 19 | PASS | Milestone 18: Custom Reports ([=]) & BYO-GSP Connector Settings | Custom Report Extensions & GST BYO-GSP Settings verified cleanly |
| 20 | PASS | Remaining Screens (Parties, Inventory Valuation/Reports) & 1440x900 Sweep | All 20+ v3.0.0 screens & 1280x800/1440x900 layout checks passed cleanly |
| 21 | PASS | All Gateway routes, hotkey uniqueness and Esc (1280x800) | 40 routes opened and unwound; unique accelerators |
| 22 | PASS | M19-M22 statutory, pricing, quantity and payroll controls (1280x800) | GSTN validator, CMP-08, price slabs, actual/billed, work state/regime, ECR and27Q/27EQ |
| 23 | PASS | M21 XML export and dry-run roundtrip (1280x800) | Rust XML envelope exported and parsed by native dry-run (12688 chars) |
| 24 | PASS | M24 five real Rust statement PDF exports (1280x800) | All5 Ctrl+P actions returned completePDF documents from registered Rust commands |
| 25 | PASS | M25 job-work, extension tabs and hardware bridge (1280x800) | All M25 tabs, real frame parser and exact by-product cost command |
| 26 | PASS | All Gateway routes, hotkey uniqueness and Esc (1440x900) | 40 routes opened and unwound; unique accelerators |
| 27 | PASS | M19-M22 statutory, pricing, quantity and payroll controls (1440x900) | GSTN validator, CMP-08, price slabs, actual/billed, work state/regime, ECR and27Q/27EQ |
| 28 | PASS | M21 XML export and dry-run roundtrip (1440x900) | Rust XML envelope exported and parsed by native dry-run (12688 chars) |
| 29 | PASS | M24 five real Rust statement PDF exports (1440x900) | All5 Ctrl+P actions returned completePDF documents from registered Rust commands |
| 30 | PASS | M25 job-work, extension tabs and hardware bridge (1440x900) | All M25 tabs, real frame parser and exact by-product cost command |

Console errors: 0. Page errors: 0. PDF exports verified: 10.
