//! Interoperability & XML Server Module (Sprint 0 Foundation)
//!
//! Scope Notice:
//! The local HTTP XML daemon listening on TCP Port 9000 will be implemented
//! following formal review and acceptance of ADR-002 (HTTP framework selection:
//! Axum vs Actix vs tiny_http vs Hyper).
//!
//! Behavioral contract reference: `01-clean-room-specs/tallyprime/interoperability/`.

/// Placeholder descriptor for the interoperability subsystem.
pub struct InteroperabilitySubsystem;

impl InteroperabilitySubsystem {
    pub const MODULE_NAME: &'static str = "interoperability";
    pub const DEFAULT_PORT: u16 = 9000;
    pub const STATUS: &'static str = "pending_adr_002";
}
