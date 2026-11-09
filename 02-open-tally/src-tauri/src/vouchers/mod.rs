//! Voucher Engine Module (Sprint 2 Stage 2)
//!
//! Handles Voucher Types and Numbering Series Masters according to
//! `01-clean-room-specs/tallyprime/vouchers/`.

pub mod formatter;
pub mod model;
pub mod service;
pub mod validator;

#[cfg(test)]
pub mod stage3_tests;
#[cfg(test)]
pub mod stage4_tests;
#[cfg(test)]
pub mod stage5a_tests;
#[cfg(test)]
pub mod stage5b_tests;
#[cfg(test)]
pub mod stage6_tests;
#[cfg(test)]
pub mod stage7_domain_tests;
#[cfg(test)]
pub mod tests;

pub use formatter::CanonicalVoucherFormatter;
pub use model::*;
pub use service::{
    derive_period_start, validate_period_consistency, DoubleEntryValidator,
    VoucherCancellationService, VoucherCorrectionService, VoucherDraftService,
    VoucherPostingService, VoucherReversalService, VoucherSequenceService, VoucherSeriesService,
    VoucherTypeService,
};
pub use validator::*;
