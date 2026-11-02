# OpenTally Evidence Metadata Schema

This specification establishes the mandatory metadata schema for all observable behavioral evidence gathered in `01-clean-room-specs/tallyprime/evidence/`.

Verifiable evidence provenance is an indispensable requirement of clean-room software engineering. Every evidence recording must adhere strictly to this schema.

---

## 1. Core Principles

1. **Empirical Truth Only:** Record only what was directly observed through permitted black-box analysis methods (wire traffic, exported files, public docs, UI behavior).
2. **Never Invent Metadata:** If an observation detail (e.g., specific sub-build hash or exact port flag) was not captured, record it explicitly as `UNOBSERVED` or `UNKNOWN`. Never synthesize or estimate metadata.
3. **Immutability & Integrity:** Raw evidence files must be accompanied by their SHA-256 cryptographic digest.
4. **Sanitization:** All evidence must be purged of confidential customer names, passwords, proprietary company data, PAN numbers, and active API tokens before being committed.

---

## 2. Field Definitions

Every evidence entry (typically accompanied by a `.yaml` manifest alongside the raw capture) must provide the following fields:

| Field Name | Type | Requirement | Description |
| :--- | :--- | :--- | :--- |
| `evidence_id` | String | **Mandatory** | Unique identifier matching pattern `EVD-[AREA]-[0-9]{3,}` (e.g. `EVD-WIRE-001`, `EVD-VCH-012`). |
| `target_product` | Object | **Mandatory** | Exact target software details: `name`, `release`, `build_number`, `bitness` (x86/x64), and `platform` (OS version). |
| `observation_timestamp` | String | **Mandatory** | Timestamp of observation formatted in ISO 8601 UTC (e.g. `2026-11-01T10:15:30Z`). |
| `observation_method` | String | **Mandatory** | Specific black-box capture mechanism used: `PCAP_CAPTURE`, `HTTP_CLIENT_INTERACTION`, `LOCAL_EXPORT_ANALYSIS`, `ODBC_QUERY_TRACE`, `PROCESS_OBSERVATION`. |
| `source_and_target_identity` | Object | **Mandatory** | Origin and destination of transaction (e.g. `client: "OpenTally Test Client 127.0.0.1"`, `server: "TallyPrime 127.0.0.1:9000"`). |
| `original_evidence_sha256` | String | **Mandatory** | Cryptographic SHA-256 hex digest of the raw capture or exported artifact. |
| `sanitized_evidence_location` | String | **Mandatory** | Relative path to the sanitized fixture within `01-clean-room-specs/tallyprime/evidence/`. |
| `observation_description` | String | **Mandatory** | Detailed, step-by-step description of the input supplied and the empirical output observed. |
| `confidence_classification` | Enum | **Mandatory** | Confidence level of the finding (see Section 3). |
| `limitations` | List[String] | **Mandatory** | Known boundaries, untested parameter spaces, or host-specific conditions. |
| `related_specification` | List[String] | **Mandatory** | Relative paths to downstream specification documents in `01-clean-room-specs/` informed by this evidence. |
| `unresolved_questions` | List[String] | **Optional** | Open questions or protocol gaps triggered by this evidence, mapped to `UNKNOWNS.md`. |

---

## 3. Confidence Classification Taxonomy

To prevent speculative deductions from polluting specifications, all observations must be classified:

- **`VERIFIED_OBSERVATION`**: Direct, empirically captured, reproducible fact (e.g., an exact HTTP response body or packet trace on loopback).
- **`INTERPRETATION`**: A reasoned deductive inference derived from one or more verified observations (e.g., deducing that an XML attribute is an enumeration based on 5 discrete captured values).
- **`HYPOTHESIS`**: An educated premise regarding behavior that has not yet been experimentally confirmed. Must be verified before specification sign-off.
- **`UNKNOWN`**: Explicitly unobserved behavior or an unresolved ambiguity. Must be logged in `UNKNOWNS.md`.

---

## 4. Storage & File Organization

Evidence files must be organized under `01-clean-room-specs/tallyprime/evidence/` as:
```text
evidence/
├── EVIDENCE_SCHEMA.md
├── EVIDENCE_TEMPLATE.yaml
├── wire/
│   ├── EVD-WIRE-001.yaml
│   └── EVD-WIRE-001.pcapng (or .xml)
├── xml/
│   ├── EVD-XML-001.yaml
│   └── EVD-XML-001-response.xml
└── ...
```

