# ADR-001: Embedded Database Engine Selection (SQLite + WAL + SQLCipher)

- **Status:** **Accepted**
- **Date:** 2026-11-01
- **Deciders:** OpenTally Architecture Team
- **Technical Context:** Desktop Core Engine (`02-open-tally/src-tauri/`)

---

## 1. Context & Requirements

OpenTally requires an embedded, high-integrity database engine to serve as the local storage backend for desktop accounting. Unlike web applications that connect to a managed multi-tenant RDBMS, OpenTally operates as a single-user or small-office desktop system running within Tauri, where each accounting company is represented as an isolated, self-contained data store.

The database must satisfy the following architectural requirements:

1. **Strict ACID Transactions & Immediate Atomicity:** Essential for double-entry financial posting where ledger balances, inventory movements, bill allocations, and tax lines must commit or rollback together.
2. **Recursive Common Table Expressions (CTEs):** Essential for hierarchical Chart of Accounts traversal (Primary Groups -> Sub-Groups -> Ledgers) and real-time trial balance aggregations.
3. **Single-File Portability:** Company data files must be portable single files that users can backup, archive, email, or restore easily, mirroring the ergonomics familiar to desktop accounting practitioners.
4. **Transparent Encryption at Rest:** Financial, payroll, and GST data must be encrypted to safeguard sensitive business records on local workstations.
5. **Concurrent Access & High-Throughput Reads:** The engine must support concurrent GUI read queries (e.g. searching accounts, viewing day books) while background tasks (e.g., HTTP/XML synchronization on port 9000 or bulk import) execute write transactions.
6. **Robust Backup & Recovery:** Non-blocking online snapshot capability while the user is actively working.
7. **Rust Ecosystem & `sqlx` Compatibility:** Native async support, compile-time query verification, connection pooling, and seamless integration with `rust_decimal`.

---

## 2. Decision

We confirm and adopt **SQLite** in **WAL (Write-Ahead Logging) mode** with **SQLCipher (AES-256-CBC encryption)**, accessed in Rust via **`sqlx`** and utilizing **`rust_decimal`** for monetary values.

---

## 3. Detailed Evaluation Against Requirements

| Evaluation Criterion | SQLite + WAL + SQLCipher Capability | Assessment |
| :--- | :--- | :---: |
| **ACID Transactions** | Provides full ACID compliance with `BEGIN IMMEDIATE` and `BEGIN EXCLUSIVE` transaction controls, guaranteeing serialized voucher posting and zero partial commits. | **EXCELLENT** |
| **Recursive CTEs** | Full support for `WITH RECURSIVE` queries, enabling fast sub-tree balance rollups across deep Chart of Accounts hierarchies without requiring complex nested-set models. | **EXCELLENT** |
| **Single-File Portability** | The entire database (schema, indices, data, audit log) resides in a single `.db` / `.otdata` file, making company creation, backup, and movement effortless. | **EXCELLENT** |
| **Encryption at Rest** | SQLCipher provides page-level transparent AES-256-CBC encryption with PBKDF2/Argon2 key derivation, ensuring complete security of the database file including journal files. | **EXCELLENT** |
| **Concurrent Access** | In WAL mode, SQLite supports **concurrent multiple readers simultaneously with one active writer**. GUI lookups never block, nor are they blocked by, active voucher writes. | **STRONG** |
| **Backup and Recovery** | Supports zero-downtime backups via the SQLite Online Backup API and `VACUUM INTO 'backup.db'`, allowing background backups without locking the active workspace. | **EXCELLENT** |
| **Rust & `sqlx` Integration** | `sqlx` provides first-class async pooling (`SqlitePool`), query macros for compile-time syntax and schema verification, and direct driver support for SQLite/SQLCipher. | **EXCELLENT** |
| **Monetary Decimal Support** | Monetary quantities are mapped via `rust_decimal::Decimal` and stored as `TEXT` (or 64-bit integer paise minor units), eliminating floating-point rounding errors. | **EXCELLENT** |

---

## 4. Trade-offs & Considered Alternatives

### Alternative A: Pure Rust Embedded Key-Value / B-Tree Store (e.g., `redb`, `Sled`)
- *Pros:* 100% safe Rust, zero C-dependency toolchain, lightweight.
- *Cons:* Lacks declarative SQL, query planner, and native recursive CTEs. Implementing hierarchical ledger balance rollups, arbitrary date-range filtering, and compound GST joins would require building an ad-hoc query engine in application code.
- *Verdict:* Rejected due to extreme engineering overhead and maintenance risk for accounting queries.

### Alternative B: Embedded Analytical Engine (e.g., `DuckDB`)
- *Pros:* Extremely fast analytical aggregation across millions of rows; columnar storage; rich SQL syntax.
- *Cons:* Optimized for OLAP batch read/append rather than high-frequency OLTP single-voucher write transactions. Lacks mature transparent page-level encryption comparable to SQLCipher. Concurrency model is less suited for continuous write/read interleaving on desktop.
- *Verdict:* Not selected as the primary transactional store; may be evaluated in future phases strictly as a read-only reporting cache if dataset scale demands it.

### Alternative C: External Client-Server RDBMS (e.g., PostgreSQL)
- *Pros:* Limitless multi-user scalability, robust row-level security.
- *Cons:* Requires installing, configuring, and managing a separate background database server process. Completely breaks the desktop offline-first experience, single-file portability, and zero-configuration install goal.
- *Verdict:* Incompatible with OpenTally's desktop-first architecture.

---

## 5. Implementation Strategy & Configuration Standards

1. **Connection Initialization Pragmas:**
   Every SQLite connection opened by `sqlx` in OpenTally must execute the following setup pragmas:
   ```sql
   PRAGMA journal_mode = WAL;          -- Enable Write-Ahead Logging
   PRAGMA synchronous = NORMAL;        -- Optimal durability / performance balance in WAL mode
   PRAGMA foreign_keys = ON;           -- Enforce relational foreign key constraints
   PRAGMA busy_timeout = 5000;         -- 5000ms busy wait before failing on concurrent write
   PRAGMA cache_size = -64000;         -- 64MB memory cache per connection
   ```

2. **Decimal Storage Convention:**
   - Currency fields (e.g., `debit_amount`, `credit_amount`, `tax_rate`) must be defined as `TEXT NOT NULL` in table schemas.
   - Rust code binds and decodes these columns directly to `rust_decimal::Decimal` via `sqlx::types::Decimal`.
   - Indexing on monetary amounts (if needed for range queries) uses integer minor units (paise).

3. **Audit Log Schema Isolation:**
   - Audit trail tables (`audit_log`, `voucher_edit_history`) reside in the same encrypted SQLite file, ensuring transactions and their edit logs are committed within the exact same atomic transaction boundary.

---

## 6. Implementation Status & Operational Reality (Sprint 0 Baseline)

To maintain rigorous technical truth and prevent misleading documentation, the target architecture decisions are explicitly distinguished from current Sprint 0 runtime implementation status:

| Dimension | Target Architectural Decision | Current Sprint 0 Runtime Status | Verification Evidence |
| :--- | :--- | :---: | :--- |
| **SQLite Engine** | Embedded SQLite 3 via `sqlx` async pool | **IMPLEMENTED & VERIFIED** | Running in `DatabaseManager`; pool connects and queries successfully. |
| **WAL Mode** | `PRAGMA journal_mode = WAL` | **IMPLEMENTED & VERIFIED** | Validated on disk: `opentally.db-wal` (12KB) & `opentally.db-shm` created; PRAGMA returns `wal`. |
| **Foreign Keys** | `PRAGMA foreign_keys = ON` | **IMPLEMENTED & VERIFIED** | Validated at runtime: `PRAGMA foreign_keys` returns `1`. |
| **Busy Timeout** | `PRAGMA busy_timeout = 5000` | **IMPLEMENTED & VERIFIED** | Set on `SqliteConnectOptions`. |
| **Migration Support** | Safe, transactional versioning | **IMPLEMENTED & VERIFIED** | `_opentally_metadata` created inside explicit transaction; existing data preserved across restarts. |
| **SQLCipher Availability**| Page-level AES-256-CBC encryption | **PENDING / NOT ACTIVE** | `sqlx` is compiled with standard SQLite driver. SQLCipher library is not linked. |
| **Actual Encryption Status**| Fully encrypted at rest | **UNENCRYPTED** | Database files (`opentally.db`) are standard unencrypted SQLite files readable by standard SQLite tools. |
| **Key Management** | OS Keyring / Windows DPAPI derivation | **UNCONFIGURED** | Key derivation specification pending ADR review. No cryptographic keys are hardcoded. |

> [!CAUTION]
> **Reporting Rule:** Under no circumstances shall OpenTally database storage be documented or reported as "encrypted" until SQLCipher is technically compiled, key derivation is integrated with the operating system credential manager, and encryption is empirically verified via raw file header inspection.


