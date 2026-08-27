//! §4.7 Export.

use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, CmdResult};
use crate::events::emit_bundle_state_changed;
use crate::state::{resolve_bundle_dir, AppState};

fn bundle_key_of(bundle_dir: &std::path::Path) -> String {
    bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

/// The desktop app is OKF-only now (round 6): there is no format left for a
/// generic `export_bundle(formats)` command to choose between (`graph-json`/
/// `rdf` generation was dropped from this app's UI once Graph stopped
/// needing a separate exported artifact -- see GraphScreen.tsx's own doc
/// comment), so that command was removed entirely rather than kept around
/// permanently un-callable. `sync_okf_documents` below -- which already
/// existed, unchanged -- is the one remaining "make sure the bundle's OKF
/// tree is current" action; `sopkb_export::export_bundle`/`export_graph_json`/
/// `export_rdf` themselves are untouched in the shared `sopkb-export` crate,
/// since `sopkb-cli`/`sopkb-server` still use them independently of this app.
///
/// Renamed from the internal `sync_okf_bundle` (§4.7) purely to avoid a Tauri
/// command whose name collides with the backend function it calls.
#[tauri::command(rename_all = "snake_case")]
pub fn sync_okf_documents(app: AppHandle, state: State<AppState>, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let _guard = state.begin_mutation();
    let result = sopkb_export::sync_okf_bundle(&bundle_dir).map_err(AppError::from)?;
    emit_bundle_state_changed(&app, &bundle_key_of(&bundle_dir), &["okf"]);
    Ok(result)
}

/// `default_export_dir` requires a readable manifest and raises without one (C §3.2)
/// -- unlike E's original signature, this command genuinely can fail.
#[tauri::command(rename_all = "snake_case")]
pub fn get_export_dir(state: State<AppState>, key: Option<String>) -> CmdResult<String> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let export_dir = sopkb_export::default_export_dir(&bundle_dir).map_err(AppError::from)?;
    Ok(export_dir.display().to_string())
}

/// `export_bundle`/`sync_okf_bundle` artifact entries carry a `path` relative to the
/// *bundle* directory (`export_path_for_manifest`, so the manifest stays portable if the
/// bundle moves) -- not an absolute path, and not relative to the app's working directory,
/// which for a double-clicked desktop app is unrelated to either. Resolving against
/// `bundle_dir` here is required for `reveal_item_in_dir` to find anything; passing the raw
/// relative string through (the previous behavior) silently failed on every click.
#[tauri::command(rename_all = "snake_case")]
pub fn reveal_path(app: AppHandle, state: State<AppState>, path: String, key: Option<String>) -> CmdResult<()> {
    let resolved = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(&path)
    } else {
        resolve_bundle_dir(&state, key.as_deref())?.join(&path)
    };
    app.opener()
        .reveal_item_in_dir(&resolved)
        .map_err(|err| AppError::new("Io", format!("could not reveal {}: {err}", resolved.display())))
}
