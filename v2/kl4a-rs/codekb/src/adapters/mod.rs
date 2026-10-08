//! Port of `kl4a/codekb/adapters/__init__.py`.
//!
//! The Python `__init__.py` has no logic beyond its `__all__` re-export
//! list (`COBOL_EXTENSIONS`, `generate_cobol_relations`, `parse_cobol_bundle`,
//! `parse_cobol_repository`, `parse_cobol_sources` — confirmed via
//! tools-code MCP `code_symbols_search`); the `pub use` block below mirrors
//! that list exactly. `cobol_architecture::detect_cobol_architecture` is
//! deliberately *not* re-exported here because the Python `__all__` does not
//! include it either — callers reach it via the qualified submodule path
//! (`crate::adapters::cobol_architecture::detect_cobol_architecture`), just
//! as Python callers do `from kl4a.codekb.adapters import cobol_architecture`
//! rather than a flat import.

pub mod cobol;
pub mod cobol_adapter;
pub mod cobol_architecture;

pub use cobol::{parse_cobol_repository, parse_cobol_sources, COBOL_EXTENSIONS};
pub use cobol_adapter::{generate_cobol_relations, parse_cobol_bundle};
