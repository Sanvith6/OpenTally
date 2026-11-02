# OpenTally Core Engine (Rust / Tauri 2)

This directory contains the Rust desktop backend for OpenTally:

## Architecture Modules
- **Database Engine (`database`):** SQLite connection pool with Write-Ahead Logging (`WAL`), foreign-key enforcement, and transactional migrations via `sqlx` (see [`ADR-001`](file:///c:/project/OpenTally/02-open-tally/docs/ADR-001-embedded-database-selection.md)). *Note: SQLCipher page-level encryption and OS Keyring key management remain pending; Sprint 0 files are standard unencrypted SQLite.*
- **Accounting Engine (`accounting`):** Specification boundary for double-entry invariants and exact decimal arithmetic using `rust_decimal` (zero floating-point math). *Note: Posting validation and Chart of Accounts tables are deferred to Sprint 1.*
- **Voucher Pipeline (`vouchers`):** Foundation stub for 24 canonical voucher types (scheduled for Sprint 2).
- **Interoperability Listener (`interoperability`):** Placeholder for Port 9000 XML server (framework selection pending ADR-002 review).
- **Desktop Commands (`commands`):** Tauri IPC bridge exposing `get_app_health` command to the React frontend.
