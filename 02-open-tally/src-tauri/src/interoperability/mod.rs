//! Clean-Room Tally XML Interoperability Subsystem
//!
//! Provides safe file-based XML interchange (Import with Dry-Run Preview & Export)
//! compliant with ADR-002 (file-based XML interchange only; does not bind TCP port 9000).

pub mod export;
pub mod import;
pub mod models;
pub mod parser;
pub mod security;

pub use export::export_company_tally_xml;
pub use import::{execute_tally_xml_import, preview_tally_xml_import};
pub use models::*;
pub use parser::parse_tally_xml;
pub use security::sanitize_and_decode_xml;

/// Historical descriptor preserved for system status reporting.
pub struct InteroperabilitySubsystem;

impl InteroperabilitySubsystem {
    pub const MODULE_NAME: &'static str = "interoperability";
    pub const DEFAULT_PORT: u16 = 9000;
    pub const STATUS: &'static str = "file_based_interchange_active";
}
