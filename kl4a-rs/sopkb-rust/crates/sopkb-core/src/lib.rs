//! error, ids, models, store, inventory, normalize. See docs/port/PORT_PLAN.md §3.1 (sopkb-core)
//! and §6.2-6.3 (Phase 1-2).

#[cfg(feature = "docx")]
pub mod docx;
pub mod error;
pub mod ids;
pub mod inventory;
pub mod knowledge_lifecycle;
pub mod lifecycle;
pub mod models;
pub mod normalize;
pub mod parallel;
pub mod prompt_overrides;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod store;
pub mod validate_report;

pub use error::{Result, SopkbError};
