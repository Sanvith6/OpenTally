# OpenTally Clean-Room Engineering Rules

## 1. Purpose and Authority
OpenTally is an independent, clean-room implementation of an enterprise accounting engine providing TallyPrime-compatible workflows, XML interoperability, and reporting conventions. To protect the project, contributors, and users from intellectual property claims, the following rules are binding on all contributors and automated agents.

## 2. The Clean-Room Separation Model
- **Dirty / Research Room (Phase 1)**: Observes external, publicly documented, and authorized user-facing behavior and external protocol interfaces. Produces functional specifications and evidence manifests in `01-clean-room-specs/`.
- **Clean / Implementation Room (Phase 2)**: Uses *only* the specifications written in `01-clean-room-specs/` to develop OpenTally in `02-open-tally/`. Developers in the Clean Room have never inspected proprietary code, binaries, or decompilations.

## 3. Prohibited Sources & Techniques
1. **No Binary Reverse Engineering**: Disassembly, decompilation (e.g., IDA Pro, Ghidra, radare2), process debugging (x64dbg, gdb), memory inspection, API hooking, or DLL injection of TallyPrime binaries is strictly forbidden.
2. **No Proprietary File Format Inspection**: Proprietary internal database structures (such as `company.900`, `tsf` files, etc.) must never be opened, hex-edited, or analyzed. OpenTally stores data in SQLite.
3. **No Proprietary TDL Extraction**: Internal proprietary TDL scripts, licensed TDL source, or decrypted TDL definitions must not be ingested.
4. **No Third-Party Tally Clones / Reverse-Engineered Repositories**: Scraping, copying, or consulting unauthorized reverse-engineered repositories is prohibited.
5. **No Customer / Production Data**: All behavioral tests and XML interchanges must use strictly synthetic, fictional test companies and transactions.

## 4. Permitted Sources
1. Official user manuals, help documentation, knowledge base articles, and release notes published by Tally Solutions.
2. Official developer documentation and public XML/ODBC interface guides.
3. Black-box behavioral observations through the standard graphical user interface running on an authorized test machine.
4. Black-box HTTP/XML network requests sent to documented integration ports (e.g., port 9000).
5. Indian statutory tax laws, GST circulars, CBIC notifications, and Institute of Chartered Accountants of India (ICAI) guidance notes.

## 5. Evidence Provenance & Cryptographic Hashing
Every behavioral observation must be documented in an individual YAML evidence record conforming to `EVIDENCE_SCHEMA.md`, including SHA-256 hashes of raw input/output payloads and explicit confidence classifications.
