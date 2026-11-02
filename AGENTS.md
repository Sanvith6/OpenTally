# OpenTally – Agent Operating Guidelines & Project Architecture

## 1. Project Philosophy & Clean-Room Separation

OpenTally is structured using a strict **Clean-Room Software Engineering** framework to build a modern, open-source accounting engine and user interface interoperable with TallyPrime observable behaviors and data formats.

> [!IMPORTANT]
> **Legal Notice & Procedural Scope:**
> Clean-room guidelines and operational policies documented herein represent procedural engineering safeguards intended to preserve independent authorship and copyright hygiene. These procedures do not constitute legal advice and cannot be claimed as legally certified compliance without formal review by independent legal counsel.

The repository is strictly partitioned into two non-overlapping operational domains:

```text
OpenTally/
├── 01-clean-room-specs/   <-- DIRTY ROOM (Specification & Behavioral Research)
└── 02-open-tally/          <-- CLEAN ROOM (Independent Implementation & Testing)
```

### Strict Domain Rules:
1. **Dirty Room (`01-clean-room-specs/`):**
   - **Role:** Protocol analysis, observable behavioral documentation, and specification authoring.
   - **Tools:** Reverse Engineering Anything (REA) MCP tools, packet capture analysis (`inspect_pcap_capture`), network logging, external format inspection.
   - **Output:** Structured evidence manifests (`EVIDENCE_SCHEMA.md`), functional specifications, mathematical formulas, state machine diagrams, XML schemas, test vectors, and tracked unknowns (`UNKNOWNS.md`).
   - **Scope:** REA tooling is restricted strictly to authorized research tasks producing documentation in `01-clean-room-specs/`.

2. **Clean Room (`02-open-tally/`):**
   - **Role:** Net-new, independent software implementation authored strictly from written functional specifications in `01-clean-room-specs/`.
   - **Tools:** Everything Claude Code (ECC) agents (`planner`, `architect`, `tdd-guide`, `code-reviewer`) and skills.
   - **Stack:** Tauri + Rust (core engine, SQLite + WAL + SQLCipher via `sqlx`, `rust_decimal`, local port 9000 listener) + TypeScript / React (desktop GUI).
   - **Constraint:** Implementations must be derived solely from written behavioral specifications, verified by automated unit, integration, and E2E tests.

### Absolute Prohibitions:
- **No Proprietary Code or Artifacts:** Proprietary executable binaries, proprietary database files (e.g. `.tsf` or company data directories), raw TDL source code, extracted proprietary assets (fonts, icons, graphics, splash screens), decompiled/disassembled code, and copied implementation details are strictly prohibited from entering `02-open-tally/`.
- **Policy Guardrails vs. Technical Enforcement:** Written rules in this repository constitute operational governance guardrails. They do not constitute an operating system or hypervisor-level technical tool firewall. Agents and contributors must conscientiously respect domain boundaries.
- **Third-Party Code Reuse & Precedence:** Any adoption of third-party open-source code requires explicit license and provenance verification. Generic ECC research-and-reuse guidance (such as Step 0 in `common-development-workflow.md`) is strictly subordinate to clean-room rules; it must **never** be used to scrape, port, or wrap unlicensed third-party Tally tools, decompilations, or reverse-engineered repositories.

---

## 2. Directory Structure

- `AGENTS.md`: This file. High-level agent orchestration, architecture rules, and clean-room policies.
- `Skill.md`: Existing REA operational instructions and capabilities (preserved).
- `.agents/rules/`: Project coding style, testing standards, and clean-room governance rules.
- `.agents/agents/`: Specialized agent definitions (planner, architect, tdd-guide, code-reviewer, etc.).
- `.agents/skills/`: Custom and ECC skills for research, accounting domain logic, and TDD workflows.
- `.kilocode/`: Preserved Kilocode configuration directory.
- `01-clean-room-specs/tallyprime/`:
  - `UNKNOWNS.md`: Structured ledger tracking protocol ambiguities and open questions.
  - `evidence/`: Wire captures, schema dumps, `EVIDENCE_SCHEMA.md`, and raw observable evidence.
  - `accounting/`: Ledgers, groups, charts of accounts, and multi-currency specifications.
  - `vouchers/`: Voucher lifecycles, numbering, inventory allocation, and posting specs.
  - `inventory/`: Stock items, units, godowns/warehouses, batches, and valuation specs.
  - `taxation/`: Indian GST, TDS, TCS, and statutory computation specifications.
  - `interoperability/`: HTTP/XML server protocols, TDL collection queries, and import/export formats.
- `02-open-tally/`:
  - `docs/`: Architecture decision records (ADRs), system design documents, and user guides.
  - `src-tauri/`: Rust core application, accounting engine, SQLite database, and local HTTP/XML listener.
  - `frontend/`: TypeScript/React desktop user interface.
- `README.md`: Workspace overview and developer quickstart.

---

## 3. Engineering & Code Quality Standards

- **TDD Workflow**: Mandatory Test-Driven Development (Red -> Green -> Refactor) using `tdd-guide`. Minimum test coverage target is 80%.
- **Rust Standards**:
  - Idiomatic Rust, clean error handling via `thiserror` / `anyhow`.
  - Immutable data transformations where possible; zero unhandled `unwrap()` calls in production logic.
  - Exact financial arithmetic with `rust_decimal`; no IEEE 754 floating point (`f32`/`f64`) in monetary calculations.
  - Run `cargo clippy -- -D warnings` and `cargo fmt`.
- **TypeScript Standards**:
  - Strict mode enabled (`strict: true`).
  - Strict typing (avoid `any`; prefer discriminating unions and branded types).
  - Exact monetary representation via `Decimal.js` or integer minor units.
  - Component modularity under 400 lines per file.
- **Subagent Workflow**:
  - Proactively invoke `planner` for multi-step feature implementations.
  - Proactively invoke `architect` for architectural decisions.
  - Proactively invoke `tdd-guide` for test generation before implementation.
  - Proactively invoke `code-reviewer` after writing or modifying code.
