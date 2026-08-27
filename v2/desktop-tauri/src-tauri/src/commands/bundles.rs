//! §4.2 Bundle management.

use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::dto::{WireBundleCard, WireBundleSummary, WireCreateProjectResult};
use crate::error::{AppError, CmdResult};
use crate::events::emit_bundle_index_changed;
use crate::state::{resolve_bundle_dir, AppState};

#[tauri::command(rename_all = "snake_case")]
pub fn list_bundles(state: State<AppState>) -> Vec<WireBundleCard> {
    let ctx = state.context();
    sopkb_workbench::list_bundles(&ctx.root).into_iter().map(Into::into).collect()
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_project(app: AppHandle, state: State<AppState>, title: String) -> CmdResult<WireCreateProjectResult> {
    let ctx = state.context();
    let _guard = state.begin_mutation();
    let result = sopkb_workbench::create_project(&ctx.root, &title).map_err(AppError::from)?;
    emit_bundle_index_changed(&app, &ctx.bundles_root);
    Ok(result.into())
}

#[tauri::command(rename_all = "snake_case")]
pub fn describe_bundle(state: State<AppState>, key: Option<String>) -> CmdResult<WireBundleSummary> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let summary = sopkb_workbench::describe_bundle(&bundle_dir).map_err(AppError::from)?;
    Ok(summary.into())
}

/// Permanently deletes a bundle (recursively removes its directory). Irreversible --
/// the frontend must confirm with the user before calling this. If the deleted
/// bundle was the currently-selected one, deselects it so no command is left
/// resolving to a directory that no longer exists.
#[tauri::command(rename_all = "snake_case")]
pub fn delete_bundle(app: AppHandle, state: State<AppState>, key: String) -> CmdResult<()> {
    let ctx = state.context();
    let _guard = state.begin_mutation();
    sopkb_workbench::delete_bundle(&ctx.root, &key).map_err(AppError::from)?;
    if ctx.selected_bundle.as_deref() == Some(key.as_str()) {
        state.deselect_bundle();
    }
    emit_bundle_index_changed(&app, &ctx.bundles_root);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn init_bundle(app: AppHandle, state: State<AppState>, path: String, title: Option<String>) -> CmdResult<WireBundleSummary> {
    let ctx = state.context();
    let _guard = state.begin_mutation();
    let summary = sopkb_workbench::init_bundle(Path::new(&path), title.as_deref()).map_err(AppError::from)?;
    emit_bundle_index_changed(&app, &ctx.bundles_root);
    Ok(summary.into())
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairResult {
    pub created: Vec<String>,
}

/// The directories `create_bundle` would create for a fresh bundle, restricted to
/// the ones that don't exist yet under `bundle_dir` -- computed BEFORE calling
/// `create_bundle` (which is documented idempotent-and-safe to call on an existing
/// bundle) so the diagnostic can honestly report what was actually missing, rather
/// than just echoing the full `REQUIRED_DIRS` list unconditionally.
fn missing_required_dirs(bundle_dir: &Path) -> Vec<String> {
    sopkb_core::store::REQUIRED_DIRS
        .iter()
        .filter(|rel| !bundle_dir.join(rel).is_dir())
        .map(|rel| (*rel).to_string())
        .collect()
}

pub(crate) fn repair_bundle_dirs_at(bundle_dir: &Path) -> Result<Vec<String>, sopkb_core::error::SopkbError> {
    let missing_before = missing_required_dirs(bundle_dir);
    // `create_bundle` on a bundle that already has a manifest.yaml is documented as
    // an idempotent re-init: it mkdir -p's every REQUIRED_DIRS entry again but does
    // NOT touch the manifest, so `title: None` is safe here -- it is never consulted
    // on the "existing" path.
    sopkb_core::store::create_bundle(bundle_dir, None)?;
    Ok(missing_before)
}

#[tauri::command(rename_all = "snake_case")]
pub fn repair_bundle_dirs(state: State<AppState>, key: Option<String>) -> CmdResult<RepairResult> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let _guard = state.begin_mutation();
    let created = repair_bundle_dirs_at(&bundle_dir).map_err(AppError::from)?;
    Ok(RepairResult { created })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LegacyStateStatusEntry {
    pub name: String,
    pub canonical_present: bool,
    pub legacy_present: bool,
    pub legacy_path: String,
}

/// Read-only diagnostic (F21 / A G-A22 / C G12): for each of the 9 known
/// canonical/legacy state-file pairs, report whether each side is present on disk.
/// No existing `sopkb-core` function surfaces this (`migrate_legacy_state` acts, it
/// doesn't report), so this walks `STATE_FILE_ALIASES` directly -- a read-only,
/// side-effect-free probe, safe to add here rather than growing `sopkb-core`'s API
/// for a single UI diagnostic.
pub(crate) fn legacy_state_status_at(bundle_dir: &Path) -> Vec<LegacyStateStatusEntry> {
    sopkb_core::store::STATE_FILE_ALIASES
        .iter()
        .map(|(canonical, legacy_relative)| LegacyStateStatusEntry {
            name: (*canonical).to_string(),
            canonical_present: bundle_dir.join(".sopkb").join(canonical).is_file(),
            legacy_present: bundle_dir.join(legacy_relative).is_file(),
            legacy_path: (*legacy_relative).to_string(),
        })
        .collect()
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_legacy_state_status(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<LegacyStateStatusEntry>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(legacy_state_status_at(&bundle_dir))
}

/// Task #37: per-bundle `mining_prompt`/`chat_prompt` overrides -- see
/// `sopkb_core::prompt_overrides`'s own doc comment for the precedence rules
/// (bundle override > profile override > built-in default). A missing or
/// malformed override file reads back as blank strings, never an error.
#[tauri::command(rename_all = "snake_case")]
pub fn get_bundle_prompt_overrides(state: State<AppState>, key: Option<String>) -> CmdResult<sopkb_core::prompt_overrides::BundlePromptOverrides> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(sopkb_core::prompt_overrides::read_bundle_prompt_overrides(&bundle_dir))
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_bundle_prompt_overrides(
    state: State<AppState>,
    overrides: sopkb_core::prompt_overrides::BundlePromptOverrides,
    key: Option<String>,
) -> CmdResult<()> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let _guard = state.begin_mutation();
    sopkb_core::prompt_overrides::write_bundle_prompt_overrides(&bundle_dir, &overrides).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_required_dirs_reports_only_absent_ones_on_a_fresh_bundle_minus_one() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();
        // Simulate the shipped example bundle's actual gap (A G-A24): missing .sopkb/cache.
        std::fs::remove_dir(bundle_dir.join(".sopkb").join("cache")).unwrap();
        let missing = missing_required_dirs(&bundle_dir);
        assert_eq!(missing, vec![".sopkb/cache".to_string()]);
    }

    #[test]
    fn repair_bundle_dirs_at_recreates_missing_dirs_and_reports_them() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();
        std::fs::remove_dir_all(bundle_dir.join("reports")).unwrap();
        assert!(!bundle_dir.join("reports").is_dir());

        let created = repair_bundle_dirs_at(&bundle_dir).unwrap();
        assert_eq!(created, vec!["reports".to_string()]);
        assert!(bundle_dir.join("reports").is_dir(), "repair must actually recreate the missing dir");
    }

    #[test]
    fn repair_bundle_dirs_at_on_a_healthy_bundle_reports_nothing_missing() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();
        let created = repair_bundle_dirs_at(&bundle_dir).unwrap();
        assert!(created.is_empty());
    }

    #[test]
    fn legacy_state_status_reports_all_nine_aliases_absent_on_a_fresh_bundle() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();
        let statuses = legacy_state_status_at(&bundle_dir);
        assert_eq!(statuses.len(), 9);
        for entry in &statuses {
            assert!(!entry.canonical_present, "{} should not exist yet", entry.name);
            assert!(!entry.legacy_present, "{} legacy alias should not exist yet", entry.name);
        }
    }

    #[test]
    fn legacy_state_status_detects_canonical_and_legacy_files_independently() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        sopkb_core::store::create_bundle(&bundle_dir, Some("B")).unwrap();

        std::fs::write(bundle_dir.join(".sopkb").join("items.json"), "[]").unwrap();
        let legacy_items = bundle_dir.join("knowledge").join("items.json");
        std::fs::create_dir_all(legacy_items.parent().unwrap()).unwrap();
        std::fs::write(&legacy_items, "[]").unwrap();

        let statuses = legacy_state_status_at(&bundle_dir);
        let items_entry = statuses.iter().find(|e| e.name == "items.json").unwrap();
        assert!(items_entry.canonical_present);
        assert!(items_entry.legacy_present);
        assert_eq!(items_entry.legacy_path, "knowledge/items.json");

        let other = statuses.iter().find(|e| e.name == "reviews.json").unwrap();
        assert!(!other.canonical_present);
        assert!(!other.legacy_present);
    }
}
