---
name: Feature Request
about: Suggest an accounting, statutory, inventory, or UX improvement for OpenTally
title: "[Feature]: "
labels: enhancement
assignees: ""
---

## Problem / Accounting Workflow
Describe the Indian accounting, GST/TDS/Payroll, inventory, or keyboard-first workflow need.

## Proposed Solution
Describe the expected UI flow (including TallyPrime-compatible shortcuts if relevant), data inputs, and report outputs.

## Acceptance Criteria
- [ ] Monetary values remain exact (`rust_decimal` / integer paise — no floating-point)
- [ ] Mutating actions record immutable SHA-256 hash-chained `audit_log` entries
- [ ] Respects `PeriodLocked` freeze and RBAC permissions

## Clean-Room Reminder
- [ ] Describe desired behavior functionally; **do not** attach proprietary executables, decompiled listings, TDL scripts, or proprietary assets.
