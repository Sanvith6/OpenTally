---
name: rea-cleanroom-tally
description: Structured workflow for capturing, analyzing, and documenting TallyPrime observable behavior, HTTP/XML wire protocols, and exported data structures using REA tools under clean-room legal safeguards.
---

# REA Clean-Room TallyPrime Research Skill

Use this skill when investigating TallyPrime observable behavior, XML request/response structures, and protocol contracts to author clean-room functional specifications in `01-clean-room-specs/`.

> [!NOTE]
> This skill operates exclusively within the **Dirty Room** domain. REA tools are operational instruments restricted to behavioral research and specification creation. Written procedures represent governance guardrails, not an operating-system-level sandbox.

---

## 1. Core Principles

- Focus exclusively on **observable behavior**: network traffic on TCP port 9000, exported XML/ODBC formats, external file layouts, and reproducible input/output vectors.
- Document functional requirements, data schemas, mathematical formulas, and business logic flows in `01-clean-room-specs/tallyprime/`.
- **Absolute Boundary:** Never transfer proprietary binaries, database files (`.tsf`), raw TDL sources, extracted UI assets, or disassembled/decompiled code into `02-open-tally/`.
- **Quirks vs. Correctness:** Explicitly categorize observed idiosyncrasies as `COMPATIBILITY_BEHAVIOR` (external wire quirks to accept/emit) versus `STATUTORY_INVARIANT` (standard financial accounting rules). Never require the clean-room accounting engine to adopt non-compliant accounting bugs.

---

## 2. Investigation Runbook

### Step 1: Capture Observable Wire Traffic & Evidence
- Capture HTTP/XML transactions sent to/from TallyPrime using `inspect_pcap_capture` or network logging.
- Extract request and response envelope formats:
  - `<ENVELOPE>`
  - `<HEADER>` with `<TALLYREQUEST>`
  - `<BODY>` containing `<DATA>`, `<EXPORTDATA>`, or `<IMPORTDATA>`
- Create a dedicated evidence manifest following [`EVIDENCE_SCHEMA.md`](file:///c:/project/OpenTally/01-clean-room-specs/tallyprime/evidence/EVIDENCE_SCHEMA.md) using the template at [`EVIDENCE_TEMPLATE.yaml`](file:///c:/project/OpenTally/01-clean-room-specs/tallyprime/evidence/EVIDENCE_TEMPLATE.yaml).
- Compute SHA-256 digests of raw evidence files before sanitizing.
- Store sanitized, anonymized fixtures in `01-clean-room-specs/tallyprime/evidence/`.

### Step 2: Track Protocol Unknowns & Ambiguities
- Whenever an ambiguous behavior, unknown parameter, or conflicting response is observed, log it immediately in [`UNKNOWNS.md`](file:///c:/project/OpenTally/01-clean-room-specs/tallyprime/UNKNOWNS.md).
- Classify confidence (`VERIFIED_OBSERVATION`, `INTERPRETATION`, `HYPOTHESIS`, or `UNKNOWN`).
- Never invent unobserved metadata.

### Step 3: Author Clean-Room Specifications
- Write structured specification files in the appropriate subdirectory of `01-clean-room-specs/tallyprime/`:
  - `accounting/`: Ledgers, groups, multi-currency.
  - `vouchers/`: Numbering schemes, allocations, posting lifecycles.
  - `inventory/`: Stock items, godowns, valuation.
  - `taxation/`: GST, TDS, TCS calculation rules.
  - `interoperability/`: HTTP/XML server protocols, TDL collection queries.
- Every specification must include:
  1. Scope and purpose.
  2. Input data schema and field constraints.
  3. Transformation and calculation rules.
  4. Output data schema.
  5. Concrete, sanitized test vectors (Inputs -> Expected Outputs).
  6. Provenance references linking back to evidence IDs in `evidence/`.
