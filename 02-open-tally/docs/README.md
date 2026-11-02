# OpenTally Architecture Documentation

This directory contains Architecture Decision Records (ADRs), system design blueprints, API specifications, and technical reference guides for the clean-room implementation of OpenTally (`02-open-tally/`).

---

## Technical Governance & Reconciliation

- [**Phase 1 Discrepancy & Correction Log**](file:///c:/project/OpenTally/02-open-tally/docs/PHASE-1-DISCREPANCY-CORRECTION-LOG.md) – Reconciles scenario counts (38 vs 46), voucher types (18 vs 24), execution status, and pending ADRs without modifying historical records.

---

## Architecture Decision Records (ADRs)

| ADR ID | Title | Status | Date |
| :--- | :--- | :---: | :---: |
| [ADR-001](file:///c:/project/OpenTally/02-open-tally/docs/ADR-001-embedded-database-selection.md) | Embedded Database Engine Selection (SQLite + WAL + SQLCipher) | `Accepted` | 2026-11-01 |
| ADR-002 | HTTP/XML Daemon Framework Selection (Port 9000: Axum / Actix / tiny_http / Hyper) | `Pending` | — |
| ADR-003 | Desktop GUI Client State Architecture (Zustand / TanStack Query / Redux Toolkit) | `Pending` | — |
