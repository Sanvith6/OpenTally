//! Company Management Module (Phase 2, Sprint 1)
//!
//! Provides company domain models, validation, lifecycle management,
//! transactional seeding (29 predefined groups + 2 reserved ledgers),
//! and multi-company isolation.

pub mod closing;
pub mod features;
pub mod model;
pub mod sample;
pub mod seeder;
pub mod service;

#[cfg(test)]
pub mod tests;

pub use closing::*;
pub use features::*;
pub use model::*;
pub use sample::*;
pub use service::CompanyService;
