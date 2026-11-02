//! Voucher Engine Module (Sprint 0 Foundation)
//!
//! Note on Voucher Invariants:
//! - Hard deletion of posted vouchers is strictly prohibited.
//! - Vouchers must transition to CANCELLED state or be offset by reversing entries.
//! - All voucher postings must commit atomically across ledger, stock, and audit tables.
//!
//! Scope Notice:
//! The 24 predefined voucher types, numbering series, and posting pipelines
//! will be implemented in Phase 2, Sprint 2 in accordance with
//! `01-clean-room-specs/tallyprime/vouchers/`.

/// Placeholder descriptor for the voucher subsystem.
pub struct VoucherSubsystem;

impl VoucherSubsystem {
    pub const MODULE_NAME: &'static str = "vouchers";
    pub const STATUS: &'static str = "foundation_ready";
}
