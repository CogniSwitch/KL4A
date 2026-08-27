//! App-managed state and the "which bundle directory does this command operate on"
//! resolution shared by nearly every bundle-scoped command (PORT_PLAN.md §4: "Bundle-
//! scoped commands take no bundle parameter -- they operate on
//! `WorkbenchContext.selected_bundle` -- with an optional `key` override").

use std::path::PathBuf;

use sopkb_workbench::WorkbenchHandle;

use crate::error::AppError;

/// `WorkbenchHandle` already guards its own internal state with a `Mutex` (see
/// `sopkb-workbench/src/context.rs`'s doc comment: "Guarded by a single lock, held as
/// app-managed state"), so it is registered with Tauri directly -- no extra outer
/// `Mutex`/`Arc` wrapper is needed; `tauri::State` already hands out a shared
/// reference and every method on `WorkbenchHandle` takes `&self`.
pub type AppState = WorkbenchHandle;

/// Resolves the bundle directory a command should operate on: `key_override` if
/// given, else `WorkbenchContext.selected_bundle`, else `InvalidInput` (no bundle
/// selected and no override supplied).
pub fn resolve_bundle_dir(handle: &AppState, key_override: Option<&str>) -> Result<PathBuf, AppError> {
    let context = handle.context();
    let key = match key_override {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => context
            .selected_bundle
            .clone()
            .ok_or_else(|| AppError::invalid_input("no bundle is selected; pass a key or select a bundle first"))?,
    };
    sopkb_workbench::bundle_dir_for_key(&context.root, &key).map_err(AppError::from)
}
