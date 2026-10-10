// Rust port of tools/kl4a/codekb (Python). Modules added incrementally by the port batches.
// Coordinator wires `pub mod` / `pub use` declarations here after each batch reports back.

pub mod adapters;
pub mod agent;
pub mod architecture;
pub mod author;
pub mod bundle;
pub mod cache;
pub mod canonical;
pub mod cli;
pub mod config;
pub mod context;
pub mod entrypoints;
pub mod ids;
pub mod inventory;
pub mod knowledge;
pub mod layout;
pub mod lifecycle;
pub mod mcp;
pub mod model;
pub mod parse;
pub mod pipeline;
pub mod procedures;
pub mod relations;
pub mod render;
pub mod run_state;
pub mod server;
pub mod state;
pub mod trace;
pub mod trace_validate;
pub mod transform;
pub mod validate;
pub mod web;

// `kl4a/kl4a/*.py` is a shared package that is a sibling of both `codekb` and
// `apikb` (not part of either), ported once as the standalone `kl4a-core`
// crate. Several files in this crate were written against the assumption
// that `bundle_store`/`okf_writer`/`hashing`/`llm_provider`/`llm_settings`
// are local modules (`crate::bundle_store::...` etc.) -- re-exporting the
// real crate's modules under those same names here keeps every such call
// site compiling unchanged rather than rewriting each one to
// `kl4a_core::...`. `ids` is deliberately NOT re-exported this way: this
// crate already has its own, distinct `kl4a/codekb/ids.py` port at
// `codekb::ids` (`as_posix`/`code_*_id_for`), plus its own `pub(crate)`
// vendored copies of `kl4a_core::ids`'s `bounded_id`/`slugify` (see that
// file's module doc) -- `kl4a_core::ids` itself is unused here.
//
// `kl4a-core::okf_writer` already names its OKF document type/error
// `OkfDocument`/`OkfDocumentError` (Rust-cased, matching every call site in
// this crate -- `validate.rs`, `render.rs` -- verbatim), so no aliasing is
// needed here.
pub use kl4a_core::{bundle_store, hashing, llm_provider, llm_settings, okf_writer};
