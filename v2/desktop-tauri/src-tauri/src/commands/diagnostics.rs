//! Task #38: exportable diagnostic bundle. No Python equivalent -- a genuinely new
//! desktop-only capability so a user hitting a problem can hand over one file
//! instead of being walked through finding `sopkb-startup.log`, `settings.json`,
//! and a bundle's `.sopkb/` state by hand (or, worse, pasting a screenshot that
//! includes their API key). Never includes a raw API key or full knowledge-item/
//! section content -- only counts, statuses, and the redacted settings shape
//! `ProfileView` already uses at the IPC boundary (`has_api_key: bool`, never the
//! key itself).

use std::io::Write as _;
use std::path::Path;

use serde_json::{json, Value};
use tauri::{AppHandle, State};
use zip::write::SimpleFileOptions;

use crate::dialogs;
use crate::error::{AppError, CmdResult};
use crate::state::AppState;

/// Same redaction convention as `ProfileView::from_profile` (§4.8): only
/// `has_api_key`/`has_..._prompt_override` booleans cross this boundary, never the
/// raw secret or the (potentially domain-specific/sensitive) prompt text itself.
fn redacted_settings_json() -> Value {
    let config = sopkb_config::load_config();
    let profiles: Vec<Value> = config
        .profiles
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "name": p.name,
                "base_url": p.base_url,
                "auth_style": p.auth_style,
                "model": p.model,
                "max_output_tokens": p.max_output_tokens,
                "timeout_seconds": p.timeout_seconds,
                "reasoning_effort": p.reasoning_effort,
                "has_mining_prompt_override": !p.mining_prompt.trim().is_empty(),
                "has_chat_prompt_override": !p.chat_prompt.trim().is_empty(),
                "has_api_key": !p.api_key.trim().is_empty(),
            })
        })
        .collect();
    json!({
        "profiles": profiles,
        "default_profile_id": config.default_profile_id,
        "reviewer_name": config.reviewer_name,
        "max_parallel_workers": config.max_parallel_workers,
    })
}

/// Prefers the copy beside the executable (where `startup_log::log` writes first),
/// falling back to the OS temp dir exactly like `startup_log::compute_log_path`
/// does -- kept as a separate, read-only lookup here rather than exposing that
/// module's own path resolution, since this side only ever reads, never appends.
fn startup_log_text() -> Option<String> {
    if let Some(beside_exe) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("sopkb-startup.log"))) {
        if let Ok(text) = std::fs::read_to_string(&beside_exe) {
            return Some(text);
        }
    }
    std::fs::read_to_string(std::env::temp_dir().join("sopkb-startup.log")).ok()
}

/// `None` when no bundle is selected -- diagnostics are still useful without one
/// (e.g. a launch/settings problem before any bundle was ever opened).
fn bundle_diagnostics(context: &sopkb_workbench::WorkbenchContext) -> Option<Value> {
    let key = context.selected_bundle.clone()?;
    let bundle_dir = sopkb_workbench::bundle_dir_for_key(&context.root, &key).ok()?;
    let summary = sopkb_workbench::describe_bundle(&bundle_dir).ok().map(|s| {
        json!({
            "title": s.title,
            "status": s.status,
            "source_count": s.source_count,
            "knowledge_item_count": s.knowledge_item_count,
            "created_at": s.created_at,
        })
    });
    let legacy_state_status = super::bundles::legacy_state_status_at(&bundle_dir);
    let last_ingest_run = sopkb_core::store::read_state_json(&bundle_dir, "ingest_run.json", Value::Null).unwrap_or(Value::Null);
    let overrides = sopkb_core::prompt_overrides::read_bundle_prompt_overrides(&bundle_dir);
    Some(json!({
        "key": key,
        "bundle_dir": bundle_dir.display().to_string(),
        "summary": summary,
        "legacy_state_status": legacy_state_status,
        "last_ingest_run": last_ingest_run,
        "has_mining_prompt_override": !overrides.mining_prompt.trim().is_empty(),
        "has_chat_prompt_override": !overrides.chat_prompt.trim().is_empty(),
    }))
}

fn build_manifest(context: &sopkb_workbench::WorkbenchContext) -> Value {
    json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "workbench_root": context.root.display().to_string(),
        "settings_path": context.settings_path.display().to_string(),
        "settings": redacted_settings_json(),
        "selected_bundle": bundle_diagnostics(context),
    })
}

/// Writes `diagnostics.json` (pretty-printed, so a human can read it directly) plus
/// `sopkb-startup.log` (when one exists) into a zip at `dest`. Pure I/O, no domain
/// logic -- kept free of `AppState`/Tauri types so it's usable straight from a test.
fn write_diagnostics_zip(dest: &Path, manifest: &Value, startup_log: Option<&str>) -> std::io::Result<()> {
    let file = std::fs::File::create(dest)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("diagnostics.json", options)?;
    zip.write_all(serde_json::to_string_pretty(manifest).unwrap_or_default().as_bytes())?;

    if let Some(log) = startup_log {
        zip.start_file("sopkb-startup.log", options)?;
        zip.write_all(log.as_bytes())?;
    }

    zip.finish()?;
    Ok(())
}

/// Asks the user where to save via a native "save as" dialog, then writes the zip
/// there. Returns `None` (not an error) if the user cancels -- the same "cancel is
/// a normal outcome" convention every other picker command in `dialogs.rs` follows.
/// The dialog itself must run on the async command's own task (its callback isn't
/// itself `async`); the actual zip write is real blocking file I/O, so it runs on
/// `spawn_blocking` like every other command that touches the filesystem for more
/// than a single small read/write.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_diagnostics_bundle(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let default_name = "sopkb-diagnostics.zip";
    let Some(dest) = dialogs::pick_save_file(&app, default_name, "Zip archive", &["zip"]).await? else {
        return Ok(None);
    };

    let context = state.context();
    let manifest = build_manifest(&context);
    let startup_log = startup_log_text();
    let dest_path = std::path::PathBuf::from(&dest);

    tauri::async_runtime::spawn_blocking(move || write_diagnostics_zip(&dest_path, &manifest, startup_log.as_deref()))
        .await
        .map_err(|err| AppError::new("Io", format!("diagnostics export task did not complete: {err}")))?
        .map_err(|err| AppError::new("Io", format!("could not write diagnostics zip: {err}")))?;

    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_settings_json_never_carries_a_raw_api_key() {
        // `"has_api_key"` legitimately contains the substring `api_key`, so this checks
        // for the raw field's own quoted-key form specifically, not a bare substring.
        let text = serde_json::to_string(&redacted_settings_json()).unwrap();
        assert!(!text.contains("\"api_key\""), "raw api_key field name must never appear -- only has_api_key");
    }

    #[test]
    fn write_diagnostics_zip_produces_a_readable_archive_with_both_entries() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        write_diagnostics_zip(&dest, &json!({"hello": "world"}), Some("log line\n")).unwrap();

        let file = std::fs::File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_string()).collect();
        names.sort();
        assert_eq!(names, vec!["diagnostics.json".to_string(), "sopkb-startup.log".to_string()]);
    }

    #[test]
    fn write_diagnostics_zip_omits_the_log_entry_when_none_is_available() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        write_diagnostics_zip(&dest, &json!({}), None).unwrap();

        let file = std::fs::File::open(&dest).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 1);
    }

    #[test]
    fn bundle_diagnostics_is_none_when_no_bundle_is_selected() {
        let dir = tempfile::tempdir().unwrap();
        let context = sopkb_workbench::WorkbenchContext {
            root: dir.path().to_path_buf(),
            mode: sopkb_workbench::WorkbenchMode::WorkbenchRoot,
            bundles_root: dir.path().join("bundles"),
            selected_bundle: None,
            generation: 1,
            settings_path: dir.path().join("settings.json"),
            error: None,
        };
        assert!(bundle_diagnostics(&context).is_none());
    }
}
