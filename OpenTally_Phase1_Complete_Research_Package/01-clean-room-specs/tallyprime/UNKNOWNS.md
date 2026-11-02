# OpenTally Ambiguity, Unknowns & Architecture Review Register

This register tracks all unresolved behavioral questions, statutory ambiguities, and architectural items requiring live empirical verification or professional legal/accounting review.

| Unknown ID | Domain | Description | Status | Independent Review Flag | Impact on OpenTally Engine |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `UNK-ADR-002` | Architecture | HTTP Server framework selection for Port 9000 XML listener (Axum vs Actix vs tiny_http vs Hyper). | PENDING_APPROVAL | **ARCHITECTURE REVIEW (ADR-002)** | Defines Rust HTTP daemon dependency & async runtime. |
| `UNK-PREC-01` | Accounting | Currency minor unit configuration vs tax intermediate precision policy in `rust_decimal`. | PENDING_APPROVAL | **ACCOUNTING ARCHITECT REVIEW** | Dynamic decimal precision rules across currencies. |
| `UNK-TAX-01` | GST & Taxation | Sub-paise tax rounding discrepancy: Difference between summing line-item rounded tax vs calculating tax on invoice total. | UNRESOLVED | **CHARTERED ACCOUNTANT REVIEW** (Section 170 CGST Act interpretation) | High: Tax allocation ledger balance & rounding ledger logic. |
| `UNK-TAX-02` | GST & Taxation | Place of supply determination when billing address differs from shipping address in bill-to-ship-to transactions. | UNRESOLVED | **LEGAL / TAX COUNSEL REVIEW** (Section 10(1)(b) IGST Act) | Critical: Tax split (CGST+SGST vs IGST). |
| `UNK-SEC-01` | Security & MCA | Edit Log retention and continuity requirements upon Financial Year company data splitting. | UNRESOLVED | **CORPORATE LAW REVIEW** (MCA Rule 3(1) Companies Rules, 2014) | High: Audit trail archive retention policy. |
| `UNK-PAY-01` | Payroll | Statutory ceiling limit handling for Employee Provident Fund (EPF on ₹15,000 wage ceiling vs actual salary). | UNRESOLVED | **LABOUR LAW / HR REVIEW** (EPFO Act & Scheme 1952) | Medium: Pay head formula definition. |
| `UNK-PII-01` | Privacy & Security| Storage, masking, and export of employee Aadhaar numbers and bank account details. | UNRESOLVED | **DATA PRIVACY COUNSEL** (DPDP Act, 2023 / Aadhaar Act) | Critical: Field-level encryption in SQLite. |
| `UNK-VOUCH-01` | Voucher Engine | Concurrency lock resolution and numbering gaps when two users save in "Multi-user Automatic" series simultaneously. | UNRESOLVED | **DATABASE ARCHITECT REVIEW** | High: SQLite WAL transaction lock & sequence allocation. |
| `UNK-XML-01` | Interoperability| Maximum payload buffering and timeout limits on HTTP Port 9000 during massive XML batch imports (>10,000 vouchers). | UNRESOLVED | **EMPIRICAL TEST REQUIRED** | Streaming XML parser buffer architecture. |
| `UNK-XML-02` | Interoperability| Exact XML error schema returned when a transaction fails double-entry balance check via HTTP import. | UNRESOLVED | **EMPIRICAL TEST REQUIRED** | Response XML schema fidelity. |
