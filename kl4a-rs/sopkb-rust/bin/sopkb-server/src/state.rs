//! Shared app state + the same "which bundle directory does this request operate
//! on" resolution as `desktop-tauri/src-tauri/src/state.rs::resolve_bundle_dir`
//! (kept as a parallel copy for the same standalone-crate reason `error.rs`/
//! `events.rs` are).

use std::path::PathBuf;
use std::sync::Arc;

use sopkb_workbench::WorkbenchHandle;

use crate::error::ApiError;
use crate::events::EventBus;

/// `axum::extract::State<T>` requires `T: Clone`; `WorkbenchHandle` itself holds a
/// `Mutex` directly (not `Arc`-wrapped internally, unlike this app's Tauri sibling
/// which relies on Tauri's own managed-state `Arc`), so this wraps it explicitly.
#[derive(Clone)]
pub struct AppState {
    pub workbench: Arc<WorkbenchHandle>,
    pub events: EventBus,
    pub token: Arc<str>,
}

pub fn resolve_bundle_dir(handle: &WorkbenchHandle, key_override: Option<&str>) -> Result<PathBuf, ApiError> {
    let context = handle.context();
    let key = match key_override {
        Some(k) if !k.is_empty() => k.to_string(),
        _ => context.selected_bundle.clone().ok_or_else(|| ApiError::invalid_input("no bundle is selected; pass a key or select a bundle first"))?,
    };
    sopkb_workbench::bundle_dir_for_key(&context.root, &key).map_err(ApiError::from)
}

pub fn bundle_key_of(bundle_dir: &std::path::Path) -> String {
    bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_bundle_dir_without_a_selection_or_override_is_invalid_input() {
        let handle = WorkbenchHandle::launch(Some(tempfile::tempdir().unwrap().path()));
        let err = resolve_bundle_dir(&handle, None).unwrap_err();
        assert_eq!(err.kind, "InvalidInput");
    }
}
