# OpenTally

[![Status: Active Development](https://img.shields.io/badge/Status-Active%20Development-blue.svg)](#current-implementation-status)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Backend: Rust](https://img.shields.io/badge/Core-Rust%20%7C%20Tauri%202-orange.svg)](#technology-stack)
[![Frontend: React + TypeScript](https://img.shields.io/badge/Frontend-React%2018%20%7C%20TypeScript-blue.svg)](#technology-stack)
[![Database: SQLite WAL](https://img.shields.io/badge/Storage-SQLite%20WAL-lightgrey.svg)](#technology-stack)
[![Tests: 64 Passing](https://img.shields.io/badge/Tests-64%20Passing%20(100%25)-brightgreen.svg)](#testing--verification)

**OpenTally** is an independent, modern, high-performance, open-source accounting engine and desktop application designed to provide reliable double-entry accounting with observable behavior and wire-protocol interoperability with TallyPrime data formats.

Built with **Tauri 2**, **Rust**, **SQLite (WAL mode)**, and **TypeScript / React 18**, OpenTally delivers sub-millisecond local financial operations, uncompromising double-entry arithmetic, and a rapid, keyboard-first desktop experience.

> [!NOTE]
> **Open Source & Project Status:** OpenTally is currently **under active development**. Phase 1 (Behavioral Research & Specification) and Sprint 1 (Company Management & Chart of Accounts) are fully implemented, verified, and passing 100% of automated tests. Sprint 2 (Voucher Management Engine) is fully specified and awaiting implementation. Contributions and discussions are welcome!

---

## Table of Contents

- [Core Principles](#core-principles)
- [Architecture & Clean-Room Methodology](#architecture--clean-room-methodology)
- [Technology Stack](#technology-stack)
- [Project Directory Structure](#project-directory-structure)
- [Current Implementation Status](#current-implementation-status)
- [Quickstart & Development Guide](#quickstart--development-guide)
- [Testing & Verification](#testing--verification)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [Security & Bug Reports](#security--bug-reports)
- [License & Legal Disclaimer](#license--legal-disclaimer)

---

## Core Principles

1. **Exact Financial Precision**: All monetary values are calculated using fixed-point decimal arithmetic (`rust_decimal` in Rust, `Decimal.js` in TypeScript). Binary floating-point representation (`f32`/`f64`) is strictly prohibited in monetary logic.
2. **Immutable Double-Entry Accounting**: Vouchers once posted cannot be quietly modified in place. Adjustments occur through formal reversals and cancellations, preserving complete audit trails.
3. **Clean-Room Software Engineering**: Specifications are authored exclusively from observable behavior, public standards, and wire formats. The core software is authored net-new without proprietary source code, decompiled assets, or proprietary database files.
4. **Keyboard-First Ergonomics**: Native desktop hotkeys (`F4`–`F9`, `F2`, `Alt+F5`/`F6`, `Enter`, `Esc`) provide rapid, mouse-free accounting workflows.
5. **Local-First & Offline-Ready**: All data resides locally in a dedicated SQLite database operating with Write-Ahead Logging (WAL) for high concurrency and crash resilience.

---

## Architecture & Clean-Room Methodology

OpenTally is structured using a strict **Clean-Room Engineering** framework partitioned into two non-overlapping operational domains:

```text
OpenTally/
├── 01-clean-room-specs/   <-- DIRTY ROOM (Specification & Behavioral Research)
└── 02-open-tally/          <-- CLEAN ROOM (Independent Implementation & Testing)
```

```
┌────────────────────────────────────────────────────────────────────────┐
│                   01-clean-room-specs/ (DIRTY ROOM)                    │
│  - Observable wire protocol captures & schema manifests                │
│  - Mathematical balance & valuation formulas                           │
│  - Statutory Indian taxation and voucher numbering specifications       │
│  - Ambiguity tracking in UNKNOWNS.md                                   │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Written Specifications ONLY
                                    │ (Zero Code / Binary Transfer)
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                     02-open-tally/ (CLEAN ROOM)                        │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │  Frontend: React 18 + TypeScript + Vite + TailwindCSS          │   │
│   │  - Keyboard navigation, Gateway menu, reactive forms           │   │
│   └───────────────────────────────┬────────────────────────────────┘   │
│                                   │ Tauri IPC Commands                 │
│                                   ▼                                    │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │  Core Engine: Tauri 2 + Rust                                   │   │
│   │  - Decimal arithmetic (rust_decimal)                           │   │
│   │  - Atomic transactions (BEGIN IMMEDIATE)                       │   │
│   │  - Programmatic migration runner                               │   │
│   └───────────────────────────────┬────────────────────────────────┘   │
│                                   │ SQLx (Prepared Statements)         │
│                                   ▼                                    │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │  Embedded Database: SQLite (WAL Mode)                          │   │
│   │  - Strict foreign keys (PRAGMA foreign_keys = ON)              │   │
│   │  - Schema integrity check constraints & immutability triggers  │   │
│   └────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

- **Dirty Room (`01-clean-room-specs/`)**: Protocol analysis, observable behavioral documentation, and specification authoring. Reverse engineering tooling is restricted strictly to authorized research tasks producing written documentation.
- **Clean Room (`02-open-tally/`)**: Net-new, independent software implementation authored strictly from written functional specifications. Proprietary binaries, `.tsf` files, raw TDL source, and decompiled code are strictly forbidden.

---

## Technology Stack

| Layer | Technologies | Purpose |
|:---|:---|:---|
| **Desktop Shell** | [Tauri 2](https://v2.tauri.app/) | Secure, lightweight native window and OS integration |
| **Backend Core** | [Rust](https://www.rust-lang.org/) (2021 edition) | Safe, high-performance accounting logic and concurrency control |
| **Persistence** | [SQLite 3](https://www.sqlite.org/) via [sqlx](https://github.com/launchbadge/sqlx) | Embedded transactional storage with Write-Ahead Logging (WAL) |
| **Precision Math**| [`rust_decimal`](https://crates.io/crates/rust_decimal) | 128-bit fixed-point decimal arithmetic for monetary amounts |
| **Frontend UI** | [React 18](https://react.dev/), [TypeScript](https://www.typescriptlang.org/) | Declarative component hierarchy with strict type safety |
| **Bundler / Dev** | [Vite](https://vitejs.dev/) | Sub-second hot module replacement (HMR) and fast builds |
| **Styling & Icons** | [TailwindCSS](https://tailwindcss.com/), [Lucide React](https://lucide.dev/) | Keyboard-friendly, responsive interface styling |
| **Testing** | `cargo test`, [Vitest](https://vitest.dev/), Python SQLite harnesses | Comprehensive unit, integration, and invariant test coverage |

---

## Project Directory Structure

```text
OpenTally/
├── .agents/                                 # Antigravity & ECC engineering rules, agents, & skills
│   ├── rules/                              # Coding style, clean-room standards, testing requirements
│   ├── agents/                             # Specialized subagents (planner, architect, tdd-guide, etc.)
│   └── skills/                             # Accounting domain guides and verification workflows
├── 01-clean-room-specs/                    # Specification Domain (Dirty Room)
│   └── tallyprime/                         # Wire formats, voucher matrices, and behavioral specs
│       ├── accounting/                     # Ledger hierarchies and Chart of Accounts specifications
│       ├── evidence/                       # Observable evidence schemas and validation templates
│       ├── interoperability/               # HTTP/XML port 9000 communication catalog
│       ├── inventory/                      # Stock items, units, and valuation specifications
│       ├── taxation/                       # Indian GST, TDS, and TCS computation rules
│       ├── vouchers/                       # Voucher types, numbering schemes, and posting rules
│       └── UNKNOWNS.md                     # Structured protocol ambiguity tracking ledger
├── 02-open-tally/                          # Implementation Domain (Clean Room)
│   ├── docs/                               # Architectural Decision Records (ADRs) & Technical Designs
│   │   ├── ADR-001-embedded-database-selection.md     # SQLite WAL embedded database decision
│   │   ├── OPENTALLY_SPRINT1_FINAL_COMPLETION_REPORT.md # Sprint 1 completion documentation
│   │   ├── OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md # Voucher engine specification (v2.9.4)
│   │   └── README.md                       # Architecture document index
│   ├── frontend/                           # Desktop User Interface (TypeScript / React)
│   │   ├── src/
│   │   │   ├── components/                 # Chart of Accounts, Company creation, Modals, Status bars
│   │   │   ├── hooks/                      # Keyboard navigation and hotkey listeners
│   │   │   ├── services/                   # Tauri IPC bridge wrappers
│   │   │   ├── types/                      # Domain data contracts and Discriminated Unions
│   │   │   ├── App.tsx                     # Top-level application shell
│   │   │   └── main.tsx                    # React DOM entry point
│   │   ├── package.json
│   │   └── vite.config.ts
│   └── src-tauri/                          # Core Accounting Engine (Rust)
│       ├── src/
│       │   ├── accounting/                 # Chart of Accounts, 29 groups, 2 reserved ledgers, validation
│       │   ├── app/                        # Application state container
│       │   ├── commands/                   # 11 Tauri IPC command handlers
│       │   ├── company/                    # Company management, FY dates, validation
│       │   ├── config/                     # Configuration and paths
│       │   ├── database/                   # SQLite connection pool, WAL mode, programmatic migrations
│       │   ├── errors/                     # Structured typed error definitions (OpenTallyError)
│       │   ├── logging/                    # Tracing diagnostics
│       │   ├── lib.rs                      # Tauri runtime initialization & IPC registry
│       │   └── main.rs                     # Desktop executable entry point
│       ├── Cargo.toml
│       └── tauri.conf.json
├── OpenTally_Phase1_Complete_Research_Package/ # Phase 1 research documentation and knowledge package
├── scratch/                                # Standalone verification test scripts for design validation
├── AGENTS.md                               # Multi-agent clean-room orchestration rules
├── CONTRIBUTING.md                         # Contributor guidelines and clean-room policy
├── LICENSE                                 # MIT License
├── README.md                               # This file
├── SECURITY.md                             # Security policy and vulnerability disclosure
└── Skill.md                                # Tooling guidelines
```

---

## Current Implementation Status

### Phase 1: Behavioral Research & Specifications — COMPLETE
- 10 research domains analyzed and documented in `01-clean-room-specs/`.
- 10 evidence manifests compiled in `01-clean-room-specs/tallyprime/evidence/`.
- Wire protocol and XML catalog created for Port 9000 interoperability.
- Protocol ambiguity ledger maintained in `UNKNOWNS.md`.

### Sprint 1: Company Management & Chart of Accounts — COMPLETE & VERIFIED
- **Stage 1 (Desktop Shell)**: Tauri 2 desktop foundation with SQLite WAL mode and health IPC.
- **Stage 2 (Company Management)**: Full company lifecycle (create, update, list, select, archive) with calendar validation (real date verification, leap-year Feb 29 handling, Indian FY `04-01` alignment, and `books_beginning_from` validation).
- **Stage 3 (Chart of Accounts)**:
  - 29 standard predefined account groups seeded automatically.
  - 2 reserved system ledgers (`Cash` under `Cash-in-Hand`, `Profit & Loss A/c` under system group) protected by database triggers.
  - User-defined account groups with circular hierarchy prevention and multi-company isolation.
  - Standard user ledgers with opening balance validation, exact decimal scaling, and double-entry equilibrium rollup.
  - 11 registered and verified Tauri IPC commands.
- **Automated Tests**: **64 executed production tests passing (100%)**:
  - Rust Backend: 52 passing (`cargo test`).
  - Frontend Vitest: 12 passing (`npm test`).

### Sprint 2: Voucher Management & Posting Engine — TECHNICAL DESIGN APPROVED (v2.9.4)
- Comprehensive technical design document in `02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md`.
- Normalized numbering series master (`voucher_number_series`) with immutable reset frequencies (`yearly`, `monthly`, `daily`, `never`).
- Effective-dated numbering configurations (`voucher_number_configs`) with dynamic periodic tokens (`{YY-YY}`, `{YYYY-MM}`, etc.).
- Complete 13-step atomic voucher reversal pipeline under predefined `Reversing Journal` (`vt_reversing_journal`) with dedicated `REV` series.
- 18-column voucher header schema with SQLite triggers blocking direct mutation of posted transactions.
- Reconciled 12-step canonical `audit_log` migration script.
- 153 proposed test scenarios documented.

---

## Quickstart & Development Guide

### Prerequisites
- **Rust**: Version 1.80+ or 1.99+ (`rustup default stable`)
- **Node.js**: Version 20 LTS or newer (`node -v`)
- **npm** or **pnpm**
- **C++ Build Tools**: Visual Studio Build Tools with C++ on Windows (required for Tauri and SQLite compilation)

### 1. Clone the Repository
```bash
git clone -b feature/phase1 https://github.com/Sanvith6/OpenTally.git
cd OpenTally
```

### 2. Frontend Setup & Verification
```bash
cd 02-open-tally/frontend
npm install
npm test            # Run 12 Vitest frontend tests
npm run build       # Build production web bundle
cd ../..
```

### 3. Backend Setup & Verification
```bash
cd 02-open-tally/src-tauri
cargo test          # Run 52 Rust backend tests
cargo clippy        # Check linter warnings
cd ../..
```

### 4. Running the Desktop Application in Development Mode
```bash
cd 02-open-tally/src-tauri
cargo run
```
*Alternatively, from the frontend directory with live reload:*
```bash
cd 02-open-tally/frontend
npm run tauri dev
```

---

## Testing & Verification

OpenTally employs a multi-tiered testing strategy ensuring absolute mathematical and transactional correctness:

```bash
# 1. Run all Rust core unit and integration tests (52 tests)
cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml

# 2. Run all frontend component tests (12 tests)
npm --prefix 02-open-tally/frontend test -- --run

# 3. Run Sprint 2 Technical Design invariant verification (19 checks)
python scratch/verify_v294.py

# 4. Run standalone SQLite schema and trigger validation
python scratch/test_v294_schema_and_config.py
```

### Test Coverage Summary
- **Unit Tests**: Group tree recursion, opening balance decimal normalization, calendar date validation, leap-year calculations.
- **Integration Tests**: Multi-company isolation, transaction rollbacks on failure, database migrations, reserved ledger triggers.
- **Security Tests**: Cross-company foreign key rejections, hidden system group protections, immutable audit log triggers.

---

## Roadmap

| Sprint | Scope | Status |
|:---:|:---|:---:|
| **Sprint 0** | Desktop foundation (Tauri 2, React 18, SQLite WAL, IPC health) | **COMPLETED** |
| **Sprint 1** | Company Management, 29 Account Groups, 2 Reserved Ledgers, Chart of Accounts, Opening Balances | **COMPLETED** |
| **Sprint 2** | Voucher Management, 24 Voucher Types, Numbering Series Engine, 13-Step Reversals | **DESIGNED (v2.9.4)** |
| **Sprint 3** | Financial Reporting (Trial Balance, P&L, Balance Sheet, Day Book, Cash/Bank Books) | Planned |
| **Sprint 4** | Inventory Master (Stock Items, Units of Measure, Godowns/Warehouses, Batch Tracking) | Planned |
| **Sprint 5** | Statutory Taxation (Indian GST, e-Invoicing, Tax Ledgers, TDS/TCS computation) | Planned |
| **Sprint 6** | Port 9000 Local HTTP/XML Server, TDL Query Parser, Import/Export Pipeline | Planned |

---

## Contributing

We welcome contributions from accounting professionals, Rust developers, and frontend engineers!

Please read our [**CONTRIBUTING.md**](CONTRIBUTING.md) before submitting pull requests.

### Clean-Room Contribution Rule
To preserve independent copyright hygiene:
- Never submit decompiled code, proprietary binary dumps, proprietary database files (`.tsf`), or raw extracted TDL source code.
- Net-new code must be written independently from behavioral specifications, public accounting standards (GAAP / ICAI / MCA), or observable input/output behavior.

---

## Security & Bug Reports

For security issues and vulnerability disclosures, please review [**SECURITY.md**](SECURITY.md). Please do not open public issues for sensitive security vulnerabilities.

---

## License & Legal Disclaimer

### License
OpenTally is open-source software licensed under the [**MIT License**](LICENSE).

### Legal Disclaimer
OpenTally is an independent open-source project and is not affiliated, endorsed, or associated with Tally Solutions Pvt. Ltd. "Tally" and "TallyPrime" are registered trademarks of Tally Solutions Pvt. Ltd. Observable behaviors, wire formats, and public accounting interfaces referenced in this project are analyzed strictly under clean-room engineering principles for the purpose of software interoperability.
