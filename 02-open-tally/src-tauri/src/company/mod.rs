//! Company Management Module (Phase 2, Sprint 1)
//!
//! Provides company domain models, validation, lifecycle management,
//! transactional seeding (29 predefined groups + 2 reserved ledgers),
//! and multi-company isolation.

pub mod model;
pub mod seeder;
pub mod service;

#[cfg(test)]
pub mod tests;

pub use model::*;
pub use service::CompanyService;
