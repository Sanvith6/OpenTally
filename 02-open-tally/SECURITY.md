# Security Policy

## Supported Versions

Security fixes and cryptographic audit updates are actively maintained for the current major release line:

| Version | Supported |
| ------- | --------- |
| 2.0.x   | Yes       |
| < 2.0   | No        |

## Reporting a Vulnerability

Please **do not** disclose security vulnerabilities in public GitHub issues or pull requests.

If you discover a security vulnerability in OpenTally—including issues affecting SQLCipher encryption, Argon2id password verification, RBAC authorization, MCA SHA-256 `audit_log` chain integrity, CWE-1236 CSV formula sanitization, XML/HTTP listener parsing on port `9000`, or path-traversal defenses—please report it responsibly:

1. **GitHub Private Vulnerability Reporting (Preferred):** Use the **Security -> Advisories -> Report a vulnerability** button in this repository.
2. **Include in your report:**
   - Affected OpenTally version and operating system (Windows, macOS, or Linux)
   - Detailed description and reproduction steps (or a minimal proof-of-concept)
   - Potential impact on accounting integrity, authentication, or local host security
3. **Response timeline:**
   - Initial acknowledgment within **72 hours**
   - Triage and remediation plan within **7 business days**
   - Coordinated disclosure after a patched release and updated `SHA256SUMS.txt` are published

## Security Architecture & Guarantees

- **Zero Floating-Point Monetary Math:** All ledger balances, taxes, payroll, FIFO stock valuation, and statutory reports use exact decimal arithmetic (`rust_decimal`) in Rust and integer minor units (`paise`) in TypeScript.
- **Database Protection:** Local company databases run on SQLite with WAL mode and SQLCipher support (`sqlx`), plus `ReadOnlyConnectionGuard` isolation for reporting queries.
- **Immutable MCA 2023 Audit Trail:** Audit entries are SHA-256 hash-chained per company and attributed to the live database-verified `AppState::resolve_actor` identity.
- **Clean-Room Isolation:** Release artifacts and `02-open-tally/` source trees are built independently from written functional specifications with zero proprietary binaries, assets, or dirty-room artifacts bundled.
