//! Mirrors `desktop-tauri/src-tauri/src/commands/ingest.rs` (§4.3). The combined
//! `run_ingest_pipeline` below is a parallel re-implementation of that file's
//! `run_ingest_steps` (same step order: scan -> normalize -> mine -> validate ->
//! sync (iff any of the first four ran) -> export), broadcasting the same
//! `ingest://progress` shape over SSE instead of a Tauri event -- necessarily
//! duplicated rather than shared, since that function takes a Tauri `&AppHandle`
//! and this crate must not depend on Tauri. NOT ported, disclosed as a gap:
//! `.sopkb/ingest_run.json` resume-on-reload persistence and `cancel_ingest`
//! (`WorkbenchHandle::request_cancel` exists and is wired into the closures below
//! for cooperative cancellation support in `sopkb_core`/`sopkb_mining`'s own
//! parallel workers, but there is no HTTP endpoint yet to actually trigger it).

use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::events::EventBus;
use crate::routes::bundles::KeyQuery;
use crate::state::{bundle_key_of, resolve_bundle_dir, AppState};

#[derive(Debug, Deserialize)]
pub struct ScanBody {
    pub source_dir: String,
    pub key: Option<String>,
}

pub async fn scan_sources(State(state): State<AppState>, Json(body): Json<ScanBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let source_dir = PathBuf::from(&body.source_dir);
    let _guard = state.workbench.begin_mutation();
    let events = state.events.clone();

    events.ingest_progress("scan", "started", body.source_dir.clone());
    let bundle_dir_for_task = bundle_dir.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        sopkb_review::with_bundle_lock(&bundle_dir_for_task, || -> Result<usize, sopkb_core::error::SopkbError> {
            let sources = sopkb_core::inventory::scan_sources(&source_dir, &bundle_dir_for_task)?;
            sopkb_export::sync_okf_bundle(&bundle_dir_for_task)?;
            Ok(sources.len())
        })
    })
    .await
    .map_err(|e| ApiError::new("Io", format!("scan task did not complete: {e}")))?;

    let sources_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            events.ingest_progress("scan", "failed", e.to_string());
            return Err(ApiError::from(e));
        }
    };
    events.ingest_progress("scan", "done", format!("{sources_count} sources"));
    events.bundle_state_changed(&bundle_key, &["inventory", "okf"]);
    Ok(Json(json!({ "sources": sources_count })))
}

pub async fn normalize_sources(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();
    let events = state.events.clone();

    events.ingest_progress("normalize", "started", "");
    let bundle_dir_for_task = bundle_dir.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        sopkb_review::with_bundle_lock(&bundle_dir_for_task, || -> Result<usize, sopkb_core::error::SopkbError> {
            let sections = sopkb_core::normalize::normalize_sources(&bundle_dir_for_task, None, Some(sopkb_config::max_parallel_workers()))?;
            sopkb_export::sync_okf_bundle(&bundle_dir_for_task)?;
            Ok(sections.len())
        })
    })
    .await
    .map_err(|e| ApiError::new("Io", format!("normalize task did not complete: {e}")))?;

    let sections_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            events.ingest_progress("normalize", "failed", e.to_string());
            return Err(ApiError::from(e));
        }
    };
    events.ingest_progress("normalize", "done", format!("{sections_count} sections"));
    events.bundle_state_changed(&bundle_key, &["sections", "okf"]);
    Ok(Json(json!({ "sections": sections_count })))
}

#[derive(Debug, Deserialize)]
pub struct MineBody {
    pub provider: String,
    pub profile_id: Option<String>,
    pub key: Option<String>,
}

pub async fn mine_knowledge(State(state): State<AppState>, Json(body): Json<MineBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();
    let events = state.events.clone();

    events.ingest_progress("mine", "started", format!("provider={}", body.provider));
    let MineBody { provider, profile_id, .. } = body;
    let bundle_dir_for_task = bundle_dir.clone();
    let events_for_task = events.clone();
    let is_cancelled_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let is_cancelled_flag_for_task = is_cancelled_flag.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let on_progress = |done: usize, total: usize| events_for_task.ingest_progress("mine", "progress", format!("{done}/{total} sections mined"));
        let is_cancelled = move || is_cancelled_flag_for_task.load(std::sync::atomic::Ordering::Relaxed);
        sopkb_review::with_bundle_lock(&bundle_dir_for_task, || -> Result<usize, sopkb_core::error::SopkbError> {
            let items = sopkb_mining::mine_bundle(&bundle_dir_for_task, &provider, profile_id.as_deref(), Some(&on_progress), Some(&is_cancelled))?;
            sopkb_export::sync_okf_bundle(&bundle_dir_for_task)?;
            Ok(items.len())
        })
    })
    .await
    .map_err(|e| ApiError::new("Io", format!("mine task did not complete: {e}")))?;

    let items_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            events.ingest_progress("mine", "failed", e.to_string());
            return Err(ApiError::from(e));
        }
    };
    events.ingest_progress("mine", "done", format!("{items_count} items"));
    events.bundle_state_changed(&bundle_key, &["items", "okf"]);
    Ok(Json(json!({ "items": items_count })))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IngestSourceWire {
    Staged,
    Folder { path: String },
}

#[derive(Debug, Deserialize)]
pub struct IngestRequestBody {
    pub source: IngestSourceWire,
    pub scan: bool,
    pub normalize: bool,
    pub mine: bool,
    pub validate: bool,
    pub export: bool,
    pub mine_provider: String,
    pub profile_id: Option<String>,
    pub key: Option<String>,
}

fn staging_dir_for(bundle_dir: &Path) -> PathBuf {
    sopkb_core::store::state_path(bundle_dir, "uploads").join("current")
}

fn count_files_recursive(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries.filter_map(|e| e.ok()).map(|e| e.path()).map(|p| if p.is_dir() { count_files_recursive(&p) } else { 1 }).sum()
}

pub async fn get_staged_sources(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let staging_dir = staging_dir_for(&bundle_dir);
    if !staging_dir.is_dir() {
        return Ok(Json(Value::Null));
    }
    let file_count = count_files_recursive(&staging_dir);
    if file_count == 0 {
        return Ok(Json(Value::Null));
    }
    Ok(Json(json!({ "staging_dir": staging_dir.display().to_string(), "file_count": file_count, "skipped": [] })))
}

pub async fn clear_staged_sources(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let _guard = state.workbench.begin_mutation();
    sopkb_workbench::reset_upload_directory(&staging_dir_for(&bundle_dir))?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Debug, Deserialize)]
pub struct PreviewBody {
    pub source: IngestSourceWire,
    pub key: Option<String>,
}

pub async fn preview_ingest_pipeline(State(state): State<AppState>, Json(body): Json<PreviewBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let source_dir = resolve_source_dir(&bundle_dir, &body.source);
    if !source_dir.is_dir() {
        return Err(ApiError::not_found(format!("source directory does not exist: {}", source_dir.display())));
    }
    let mut result = sopkb_core::inventory::classify_source_updates(&source_dir, &bundle_dir)?;
    if let Value::Object(ref mut map) = result {
        map.insert("source_dir".to_string(), json!(source_dir.display().to_string()));
    }
    Ok(Json(result))
}

fn resolve_source_dir(bundle_dir: &Path, source: &IngestSourceWire) -> PathBuf {
    match source {
        IngestSourceWire::Staged => staging_dir_for(bundle_dir),
        IngestSourceWire::Folder { path } => PathBuf::from(path),
    }
}

/// Parallel re-implementation of `desktop-tauri`'s `run_ingest_steps` -- see this
/// module's own doc comment for why it can't be shared instead of duplicated.
#[allow(clippy::too_many_arguments)]
fn run_ingest_steps(
    events: &EventBus,
    bundle_dir: &Path,
    source_dir: &Path,
    scan: bool,
    normalize: bool,
    mine: bool,
    validate: bool,
    export: bool,
    mine_provider: &str,
    profile_id: Option<&str>,
) -> Result<Value, sopkb_core::error::SopkbError> {
    let mut result = json!({});
    let mut any_ran = false;

    if scan {
        events.ingest_progress("scan", "started", "");
        let sources = sopkb_core::inventory::scan_sources(source_dir, bundle_dir)?;
        result["sources"] = json!(sources.len());
        any_ran = true;
        events.ingest_progress("scan", "done", format!("{} sources", sources.len()));
    }

    if normalize {
        events.ingest_progress("normalize", "started", "");
        let on_progress = |done: usize, total: usize| events.ingest_progress("normalize", "progress", format!("chunk {done}/{total} indexed"));
        let log_warning = |message: &str| sopkb_core::store::append_ingest_log(bundle_dir, message);
        let restructure = sopkb_workbench::provider_hook(mine_provider, profile_id, Some(&on_progress), None, Some(&log_warning));
        let sections = sopkb_core::normalize::normalize_sources(bundle_dir, restructure.as_deref(), Some(sopkb_config::max_parallel_workers()))?;
        result["sections"] = json!(sections.len());
        any_ran = true;
        events.ingest_progress("normalize", "done", format!("{} sections", sections.len()));
    }

    if mine {
        events.ingest_progress("mine", "started", format!("provider={mine_provider}"));
        let on_progress = |done: usize, total: usize| events.ingest_progress("mine", "progress", format!("{done}/{total} sections mined"));
        let items = sopkb_mining::mine_bundle(bundle_dir, mine_provider, profile_id, Some(&on_progress), None)?;
        result["items"] = json!(items.len());
        result["mine_provider"] = json!(mine_provider);
        any_ran = true;
        events.ingest_progress("mine", "done", format!("{} items", items.len()));
    }

    if validate {
        events.ingest_progress("validate", "started", "");
        let (errors, warnings) = sopkb_review::validate_bundle(bundle_dir)?;
        result["validation"] = json!({ "errors": errors.len(), "warnings": warnings.len() });
        any_ran = true;
        events.ingest_progress("validate", "done", format!("{} errors, {} warnings", errors.len(), warnings.len()));
    }

    if any_ran {
        events.ingest_progress("sync", "started", "");
        let okf_bundle = sopkb_export::sync_okf_bundle(bundle_dir)?;
        result["okf_bundle"] = okf_bundle;
        events.ingest_progress("sync", "done", "");
    }

    if export {
        events.ingest_progress("export", "started", "");
        let formats: Vec<String> = sopkb_workbench::DEFAULT_EXPORT_FORMATS.iter().map(|s| s.to_string()).collect();
        let exports = sopkb_export::export_bundle(bundle_dir, &formats)?;
        result["exports"] = json!(exports);
        events.ingest_progress("export", "done", format!("{} artifacts", exports.len()));
    }

    Ok(result)
}

pub async fn run_ingest_pipeline(State(state): State<AppState>, Json(body): Json<IngestRequestBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let source_dir = resolve_source_dir(&bundle_dir, &body.source);
    if !source_dir.is_dir() {
        return Err(ApiError::not_found(format!("source directory does not exist: {}", source_dir.display())));
    }

    let _guard = state.workbench.begin_mutation();
    let events = state.events.clone();
    let IngestRequestBody { scan, normalize, mine, validate, export, mine_provider, profile_id, .. } = body;

    let bundle_dir_for_task = bundle_dir.clone();
    let events_for_task = events.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        sopkb_review::with_bundle_lock(&bundle_dir_for_task, || {
            run_ingest_steps(&events_for_task, &bundle_dir_for_task, &source_dir, scan, normalize, mine, validate, export, &mine_provider, profile_id.as_deref())
        })
    })
    .await
    .map_err(|e| ApiError::new("Io", format!("ingest pipeline task did not complete: {e}")))?;

    let result = outcome?;

    let mut scope: Vec<&str> = Vec::new();
    if scan {
        scope.push("inventory");
    }
    if normalize {
        scope.push("sections");
    }
    if mine {
        scope.push("items");
    }
    if scan || normalize || mine || validate {
        scope.push("okf");
    }
    if export {
        scope.push("exports");
    }
    if !scope.is_empty() {
        events.bundle_state_changed(&bundle_key, &scope);
    }

    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_source_dir_staged_points_at_current_uploads() {
        let bundle_dir = PathBuf::from("/bundle");
        assert_eq!(resolve_source_dir(&bundle_dir, &IngestSourceWire::Staged), staging_dir_for(&bundle_dir));
    }

    #[test]
    fn resolve_source_dir_folder_uses_the_given_path_verbatim() {
        let bundle_dir = PathBuf::from("/bundle");
        let resolved = resolve_source_dir(&bundle_dir, &IngestSourceWire::Folder { path: "/elsewhere".to_string() });
        assert_eq!(resolved, PathBuf::from("/elsewhere"));
    }
}
