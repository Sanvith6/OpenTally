# OpenTally Skill Registry & Methodologies

This document indexes the clean-room research skills, accounting governance rules, and verification standards used across the OpenTally project.

## Registered Skills

| Skill ID | Name | Path | Purpose |
| :--- | :--- | :--- | :--- |
| `SKILL-REA-01` | REA Cleanroom Tally | `.agents/skills/rea-cleanroom-tally/SKILL.md` | Protocol for black-box behavioral testing, XML capture, and evidence manifest generation. |
| `SKILL-ACC-01` | OpenTally Accounting | `.agents/skills/opentally-accounting/SKILL.md` | Accounting governance, statutory GST workflows, double-entry invariants, and report formulas. |
| `RULE-CR-01` | Clean Room Governance | `.agents/rules/opentally-clean-room.md` | Mandatory legal, architectural, and operational clean-room boundaries. |

## Application Guidelines
- All observations must be logged with evidence IDs conforming to `01-clean-room-specs/tallyprime/evidence/EVIDENCE_SCHEMA.md`.
- No specification may make definitive assertions without citing evidence or marking the item as `UNVERIFIED_DESK_RESEARCH`, `INFERENCE`, or `HYPOTHESIS`.
