# OpenTally Clean-Room Engineering Rules

This rule defines mandatory governance for preserving legal and architectural clean-room boundaries between TallyPrime behavioral research and OpenTally implementation.

> [!WARNING]
> **Legal Disclaimer:**
> Compliance with these rules represents an internal engineering methodology designed to ensure independent creation. These procedures do not guarantee legal immunity and cannot be cited as legally certified compliance without independent legal review by qualified intellectual property counsel.

---

## 1. Domain Separation

1. **Dirty Room (`01-clean-room-specs/`):**
   - **Role:** Researches observable behavior, protocol schemas, and mathematical accounting rules.
   - **Tools:** Operated using Reverse Engineering Anything (REA) MCP tools and black-box network/export observation.
   - **Outputs:**
     - Observable evidence manifests complying with [`EVIDENCE_SCHEMA.md`](file:///c:/project/OpenTally/01-clean-room-specs/tallyprime/evidence/EVIDENCE_SCHEMA.md).
     - Tracked protocol ambiguities in [`UNKNOWNS.md`](file:///c:/project/OpenTally/01-clean-room-specs/tallyprime/UNKNOWNS.md).
     - Functional requirements, data schemas, mathematical formulations, and state machine diagrams.
     - Concrete, sanitized, reproducible input/output test vectors.

2. **Clean Room (`02-open-tally/`):**
   - **Role:** Independent implementation created strictly from written functional specifications authored in `01-clean-room-specs/`.
   - **Tools:** Governed by Everything Claude Code (ECC) agents (`planner`, `architect`, `tdd-guide`, `code-reviewer`) and skills.
   - **Implementation Stack:** Rust, Tauri, SQLite + WAL + SQLCipher via `sqlx`, and TypeScript / React.

---

## 2. Invariants & Prohibitions

1. **Absolute Artifact Prohibition:** Under no circumstances may any of the following artifacts enter `02-open-tally/`:
   - Proprietary executable binaries or DLLs.
   - Proprietary database files (e.g. `.tsf` files, company data folders).
   - Raw TDL (Tally Definition Language) source code or macro definitions.
   - Extracted proprietary UI assets, fonts, icons, bitmaps, or brand graphics.
   - Disassembled, decompiled, or disassembled proprietary source code.
   - Copied internal implementation details or algorithm translations.

2. **Specification Gate:** An OpenTally feature implementation in `02-open-tally/` may only begin once an approved specification document exists in `01-clean-room-specs/`.

3. **Tooling Policy & Governance Guardrails:**
   - REA binary analysis tools (`open_binary`, `read_bytes`, `procedure_assembly`, Frida, Rizin) are restricted by operational policy to authorized research tasks in `01-clean-room-specs/`.
   - **Important:** Written rules constitute operational and behavioral guardrails; they do not represent an operating system or hypervisor-level technical tool firewall. Agents must strictly adhere to domain separation.

4. **Third-Party Code Reuse & Rule Precedence:**
   - All third-party library adoption requires verified licensing (e.g., MIT, Apache-2.0) and clean provenance.
   - **Rule Precedence:** Generic ECC research-and-reuse instructions (such as Step 0 in `common-development-workflow.md`) are explicitly subordinate to this rule. They must **never** be used to scrape, port, or adopt unlicensed third-party Tally tools, reverse-engineered scripts, or decompilation repositories.

5. **Observed Behavior vs. Accounting Correctness:**
   - **Wire Interoperability:** External HTTP/XML endpoints must parse and accept observed legacy formatting and quirks.
   - **Accounting Integrity:** Internal ledger storage, transaction commits, and statutory calculations must strictly obey double-entry invariants, GAAP/Ind AS standards, and statutory law. Observable software bugs in the reference product must not be replicated as core accounting rules.

---

## 3. Review & Verification Checklist

Before approving any pull request or completing an implementation task in `02-open-tally/`:
- [ ] Implementation is traced directly to a formal specification in `01-clean-room-specs/`.
- [ ] Automated unit or integration tests exist covering the specification's test vectors.
- [ ] Code is net-new, authored independently in idiomatic Rust or TypeScript without proprietary contamination.
- [ ] Any third-party crate or npm package has been vetted for permissive licensing and provenance.
- [ ] Automated code review has been completed by `code-reviewer` agent.
