# OpenTally Evidence Schema Specification (v1.0.0)

Every evidence file saved under `01-clean-room-specs/tallyprime/evidence/` must be a valid YAML document complying with the following schema:

```yaml
evidence_id: string              # Format: EVD-{WORKSTREAM}-{NUMBER}, e.g. EVD-COMP-001, EVD-VOUCH-012, EVD-XML-005
target_product: string           # e.g., "TallyPrime", "TallyPrime Edit Log"
target_version: string           # e.g., "Release 4.1", "Release 3.0.1"
target_build: string             # Build number if available, or "Desk Research / Official Docs"
platform: string                 # Operating system, e.g. "Windows 11 Pro 64-bit", "Windows Server 2022", "Linux Wine (Unsupported)"
observation_timestamp: string    # ISO 8601 UTC timestamp, e.g. "2026-11-01T06:50:00Z"
observation_method: string       # "LIVE_UI_INTERACTION", "LIVE_HTTP_XML", "DESK_OFFICIAL_DOCS", "CODE_ANALYSIS_DISALLOWED"
source_identity: string          # Name of researcher/agent and environment
target_identity: string          # Target system instance, port, or documentation URL
original_evidence_sha256: string # SHA-256 hash of original raw capture (or hash of documentation citation)
sanitized_evidence_location: string # Path to sanitized artifact or section in spec
observation_description: string  # Detailed description of tested action and observed response
confidence: string               # DIRECT_OBSERVATION | OFFICIAL_DOCUMENTATION | INFERENCE | HYPOTHESIS | UNKNOWN
limitations: list of strings     # Environmental or test constraints
related_spec: string             # Relative path to functional spec document
unresolved_questions: list of strings # Any open unknowns identified during observation
```
