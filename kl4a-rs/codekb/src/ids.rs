//! Port of `kl4a/codekb/ids.py`.
//!
//! Deterministic, bounded-length id builders for every Code Knowledge Bundle
//! resource kind (source, module, symbol, evidence, relation, knowledge item,
//! procedure) plus the `as_posix` path-normalization helper they all rely on.
//!
//! Ported symbol-for-symbol from the Python source (verified via tools-code
//! MCP `code_symbols_get`); every function here is a one-line delegation to
//! `bounded_id`, so there is no dispatch/branching to enumerate beyond what's
//! ported below.
//!
//! ## Cross-batch note: `bounded_id` / `slugify`
//!
//! The Python `kl4a.codekb.ids` module imports `bounded_id` (and, indirectly
//! through it, `slugify`) from `kl4a.kl4a.ids` — a *different* source file
//! (`kl4a/kl4a/ids.py`), not part of this batch (which is
//! `kl4a/codekb/{agent,architecture,author,ids}.py`). That file is a small,
//! shared, cross-cutting utility used by `codekb`, `apikb`, and `sopkb`
//! alike (e.g. `kl4a.codekb.architecture` also calls `bounded_id` directly).
//!
//! This batch vendored local copies of `bounded_id`/`slugify` rather than
//! leave every function in this file (and in `architecture.rs`)
//! uncompilable pending another batch porting `kl4a/kl4a/ids.py`. That
//! module has since landed as the `kl4a-core` crate's `ids` module
//! (`kl4a_core::ids::{slugify, bounded_id}`, confirmed identical via
//! `code_symbols_get(symbol-kl4a-kl4a-ids-bounded-id)` /
//! `(symbol-kl4a-kl4a-ids-slugify)`), so the coordinator re-points this
//! module at it here instead of keeping the duplicate vendored copies (both
//! function bodies matched byte-for-byte against the real Python source, so
//! this is a pure de-duplication, not a behavior change). Re-exported at the
//! same `pub(crate)` visibility so `architecture.rs`'s existing
//! `crate::ids::bounded_id` call sites keep compiling unchanged.

use std::path::Path;

pub(crate) use kl4a_core::ids::{bounded_id, slugify};

/// Port of `kl4a.codekb.ids.as_posix`.
///
/// Python dispatches on `isinstance(path, Path)`: a `Path` gets
/// `Path.as_posix()`, a plain `str` gets a literal `path.replace("\\", "/")`.
/// Both branches produce the same observable result for any path that is
/// otherwise well-formed, so a single `impl AsRef<Path>` entry point covers
/// both call shapes without losing either branch's behavior.
pub fn as_posix(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

/// Port of `kl4a.codekb.ids.code_source_id_for`.
pub fn code_source_id_for(relative_path: impl AsRef<Path>) -> String {
    bounded_id("source", &as_posix(relative_path), 96)
}

/// Port of `kl4a.codekb.ids.code_module_id_for`.
pub fn code_module_id_for(module_name: &str) -> String {
    bounded_id("module", module_name, 96)
}

/// Port of `kl4a.codekb.ids.code_symbol_id_for`.
pub fn code_symbol_id_for(qualified_name: &str) -> String {
    bounded_id("symbol", qualified_name, 112)
}

/// Port of `kl4a.codekb.ids.code_evidence_id_for`.
pub fn code_evidence_id_for(symbol_id: &str) -> String {
    bounded_id("evidence", symbol_id, 120)
}

/// Port of `kl4a.codekb.ids.code_relation_id_for`.
pub fn code_relation_id_for(subject_id: &str, predicate: &str, object_id: &str) -> String {
    bounded_id("kr", &format!("{subject_id}-{predicate}-{object_id}"), 120)
}

/// Port of `kl4a.codekb.ids.code_knowledge_id_for`.
pub fn code_knowledge_id_for(symbol_id: &str, suffix: &str) -> String {
    bounded_id("ki", &format!("{symbol_id}-{suffix}"), 120)
}

/// Port of `kl4a.codekb.ids.code_procedure_id_for`.
pub fn code_procedure_id_for(entry_symbol_id: &str) -> String {
    bounded_id("proc", entry_symbol_id, 96)
}
