//! The Tauri command surface, PORT_PLAN.md §4. One module per spec subsection;
//! every `#[tauri::command]` fn body is argument marshalling + one call into the
//! backend + error mapping, per the port plan's own instruction that no domain
//! logic lives in `desktop-tauri`. Non-trivial marshalling/shaping is factored into
//! plain functions (declared `pub(crate)` or private, unit-tested in-module) that
//! the command wrapper calls into -- see each module's `#[cfg(test)]` block.

pub mod agent;
pub mod bundles;
pub mod context;
pub mod diagnostics;
pub mod export;
pub mod ingest;
pub mod mcp;
pub mod reads;
pub mod relations;
pub mod review;
pub mod settings;
