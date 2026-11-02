# OpenTally — Clean-Room Desktop Application (`02-open-tally`)

This directory contains the independent clean-room implementation of the **OpenTally** accounting engine and desktop application.

---

## Directory Structure

```text
02-open-tally/
├── docs/                                  # Architectural Decision Records (ADRs) & technical designs
│   ├── README.md                          # ADR index
│   ├── ADR-001-embedded-database-selection.md  # SQLite + WAL + SQLCipher architecture
│   ├── OPENTALLY_SPRINT1_FINAL_COMPLETION_REPORT.md # Sprint 1 completion documentation
│   ├── OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md # Voucher engine specification (v2.9.4)
│   └── PHASE-1-DISCREPANCY-CORRECTION-LOG.md   # Reconciled Phase 1 research discrepancies
├── frontend/                              # Desktop GUI (TypeScript + React 18 + Vite + Tauri API)
│   ├── src/
│   │   ├── components/                    # Chart of Accounts, Company creation modal, Gateway, Status bars
│   │   ├── hooks/                         # Keyboard navigation & hotkeys handler
│   │   ├── services/                      # Tauri IPC invocation wrappers
│   │   ├── types/                         # IPC request/response data contracts
│   │   ├── App.tsx                        # Main application view & status orchestrator
│   │   └── main.tsx                       # Entry point
│   ├── package.json
│   └── vite.config.ts
└── src-tauri/                             # Core Desktop Accounting Engine (Rust + Tauri 2 + SQLite)
    ├── src/
    │   ├── accounting/                    # 29 account groups, 2 reserved ledgers, CoA tree, validation
    │   ├── app/                           # AppState container
    │   ├── commands/                      # 11 registered Tauri IPC command handlers
    │   ├── company/                       # Company lifecycle, FY dates, validation, leap-year calculations
    │   ├── config/                        # Application configuration
    │   ├── database/                      # SQLite pool, WAL mode, programmatic migrations
    │   ├── errors/                        # Typed error definitions (OpenTallyError)
    │   ├── logging/                       # Tracing and diagnostic logging
    │   ├── lib.rs                         # Tauri builder & lifecycle management
    │   └── main.rs                        # Executable entry point
    ├── Cargo.toml
    └── tauri.conf.json
```

---

## Current Status & Implemented Features

### Sprint 1: Company Management & Chart of Accounts (COMPLETED & VERIFIED)
- **Embedded Database**: SQLite engine with Write-Ahead Logging (`PRAGMA journal_mode = WAL`) and strict foreign keys (`PRAGMA foreign_keys = ON`). Programmatic migration runner (`run_sprint1_migration`, `run_stage3_migration`).
- **Multi-Company Management**: Full CRUD lifecycle, active company persistence across app restarts, statutory Indian FY date validation (`04-01`), leap-year handling (Feb 29), and soft-archival (`is_archived = 1`).
- **Standard Chart of Accounts**:
  - 29 standard predefined account groups seeded automatically on company creation.
  - 2 reserved system ledgers (`Cash` under `Cash-in-Hand`, `Profit & Loss A/c` under system group) protected by database triggers.
  - Multi-level recursive tree rollup for parent-child groups.
  - Circular hierarchy prevention and multi-company boundary protection via composite foreign keys.
- **Opening Balances**: Exact decimal normalization with `rust_decimal`, scale checking against `currency_decimal_places`, and double-entry equilibrium validation rollup.
- **Tauri IPC Commands**: 11 commands registered and tested.

### Sprint 2: Voucher Management & Posting Engine (DESIGNED — v2.9.4)
- Technical design approved in `docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md`.
- 18-column voucher header schema with immutability triggers.
- Normalized numbering series master (`voucher_number_series`) and effective-dated configuration versioning.
- Atomic 13-step reversal workflow under predefined `Reversing Journal` (`vt_reversing_journal`) with dedicated `REV` series.

---

## Running & Testing

### Prerequisites
- Rust 1.80+ or 1.99+ (`cargo`, `rustc`, `clippy`, `rustfmt`)
- Node.js 20+ & npm

### Running Automated Tests
```powershell
# Run backend tests (52 tests)
cargo test --manifest-path src-tauri/Cargo.toml

# Run frontend tests (12 tests)
cd frontend
npm test
cd ..
```

### Running the Desktop Application
```powershell
cd src-tauri
cargo run
```
Or launch with hot module reload:
```powershell
cd frontend
npm run tauri dev
```

---

## Clean-Room Governance
- Net-new, independent implementation authored from scratch based exclusively on `01-clean-room-specs/`.
- Strict prohibition of proprietary executable binaries, `.tsf` files, raw TDL source, or decompiled code.
- 100% test passing baseline: 52 Rust backend tests + 12 frontend tests = 64 tests passing.
