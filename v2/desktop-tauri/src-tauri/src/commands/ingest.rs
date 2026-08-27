//! §4.3 Ingest pipeline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::dialogs::{pick_files, pick_folder};
use crate::dto::{WireIngestResult, WireStagedUpload};
use crate::error::{AppError, CmdResult};
use crate::events::{emit_bundle_state_changed, emit_ingest_progress};
use crate::state::{resolve_bundle_dir, AppState};

const SOURCE_FILE_EXTENSIONS: &[&str] = &["md", "txt", "docx", "pdf"];

#[tauri::command(rename_all = "snake_case")]
pub async fn pick_source_files(app: AppHandle) -> CmdResult<Vec<String>> {
    pick_files(&app, "Source documents", SOURCE_FILE_EXTENSIONS).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn pick_source_folder(app: AppHandle) -> CmdResult<Option<String>> {
    pick_folder(&app).await
}

/// §4.3's `mode: "files"|"folder"` distinguishes a flat multi-file pick (each file's
/// `relative_name` is just its own basename) from a folder-shaped pick where the
/// frontend has already expanded a directory into a flat list of absolute paths and
/// wants their relative structure preserved during staging -- otherwise two
/// same-named files from different subfolders would silently collide on disk.
/// `"folder"` mode approximates that structure by stripping each path's longest
/// common ancestor directory across the whole batch. (A native folder *pick* itself,
/// per docs/port/DECISIONS.md Q8, bypasses staging entirely via `IngestSource::Folder`
/// -- this mode exists for the case where staging is still wanted with structure
/// preserved.) Pure/testable independent of any filesystem access.
pub(crate) fn relative_names_for_stage(paths: &[PathBuf], mode: &str) -> Vec<String> {
    let basename = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if mode != "folder" {
        return paths.iter().map(|p| basename(p)).collect();
    }
    let Some(common) = common_ancestor_dir(paths) else {
        return paths.iter().map(|p| basename(p)).collect();
    };
    paths
        .iter()
        .map(|p| match p.strip_prefix(&common) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"),
            _ => basename(p),
        })
        .collect()
}

/// Longest common ancestor DIRECTORY of every path's parent (not the files
/// themselves), so a single-entry batch still yields `Some(parent)` rather than
/// collapsing every path to nothing.
fn common_ancestor_dir(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut iter = paths.iter();
    let mut common = iter.next()?.parent()?.to_path_buf();
    for path in iter {
        let parent = path.parent()?;
        while !parent.starts_with(&common) {
            if !common.pop() {
                return None;
            }
        }
    }
    Some(common)
}

#[tauri::command(rename_all = "snake_case")]
pub fn stage_source_files(
    app: AppHandle,
    state: State<AppState>,
    paths: Vec<String>,
    mode: String,
    replace: bool,
    key: Option<String>,
) -> CmdResult<WireStagedUpload> {
    if mode != "files" && mode != "folder" {
        return Err(AppError::invalid_input(format!("mode must be \"files\" or \"folder\", got {mode:?}")));
    }
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let relative_names = relative_names_for_stage(&path_bufs, &mode);
    let uploads: Vec<sopkb_workbench::UploadSource> =
        path_bufs.into_iter().zip(relative_names).map(|(path, relative_name)| sopkb_workbench::UploadSource { path, relative_name }).collect();

    let staged = state.stage_uploaded_files_guarded(&bundle_dir, &uploads, replace).map_err(AppError::from)?;
    // Not emitted as `bundle://state-changed`: staging only writes into
    // `.sopkb/uploads/current`, which doesn't correspond to any scope in §4.11's
    // closed `scope` vocabulary ("inventory"|"sections"|"items"|"reviews"|
    // "exports"|"okf") -- nothing about the bundle's actual content has changed
    // yet, only what a *future* scan would read.
    let _ = &app;
    Ok(staged.into())
}

fn staging_dir_for(bundle_dir: &Path) -> PathBuf {
    sopkb_core::store::state_path(bundle_dir, "uploads").join("current")
}

/// Every staged file's path relative to `dir`, forward-slash-joined regardless of
/// platform so a `"folder"`-mode stage's subdirectory structure round-trips
/// consistently to the frontend. Sorted for a stable, predictable render order
/// (readdir order is not guaranteed and would otherwise reshuffle the list on
/// every reload).
fn list_files_recursive(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, prefix: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let rel = prefix.join(entry.file_name());
            if path.is_dir() {
                walk(&path, &rel, out);
            } else {
                out.push(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, Path::new(""), &mut out);
    out.sort();
    out
}

/// No existing `sopkb-workbench` function peeks at the current staging directory
/// without mutating it (confirmed: `upload.rs` only offers `stage_uploaded_files`
/// and `reset_upload_directory`) -- a thin local read, pure enough to unit test with
/// a tempdir.
pub(crate) fn peek_staged_sources(staging_dir: &Path) -> Option<WireStagedUpload> {
    if !staging_dir.is_dir() {
        return None;
    }
    let files = list_files_recursive(staging_dir);
    if files.is_empty() {
        return None;
    }
    Some(WireStagedUpload { staging_dir: staging_dir.display().to_string(), file_count: files.len(), skipped: Vec::new(), files })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_staged_sources(state: State<AppState>, key: Option<String>) -> CmdResult<Option<WireStagedUpload>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(peek_staged_sources(&staging_dir_for(&bundle_dir)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_staged_sources(state: State<AppState>, key: Option<String>) -> CmdResult<()> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let _guard = state.begin_mutation();
    sopkb_workbench::reset_upload_directory(&staging_dir_for(&bundle_dir)).map_err(AppError::from)
}

/// Removes exactly one staged file by its relative path (as returned in
/// `WireStagedUpload::files`), leaving the rest of the batch staged -- the
/// counterpart `clear_staged_sources` only ever wipes everything. `relative_path`
/// is resolved against the staging dir and the result is required to still be
/// *inside* it (via `Path::components`, rejecting any `..`/absolute segment)
/// before deletion, so a malformed/malicious relative path can never escape the
/// staging directory -- the same class of guard `is_bundle_dir` gives bundle
/// deletion (`bundles::delete_bundle`).
#[tauri::command(rename_all = "snake_case")]
pub fn remove_staged_source(state: State<AppState>, relative_path: String, key: Option<String>) -> CmdResult<Option<WireStagedUpload>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let staging_dir = staging_dir_for(&bundle_dir);
    if relative_path.is_empty() || relative_path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return Err(AppError::invalid_input(format!("invalid staged file path: {relative_path:?}")));
    }
    let target = staging_dir.join(&relative_path);
    if !target.starts_with(&staging_dir) || !target.is_file() {
        return Err(AppError::not_found(format!("staged file not found: {relative_path}")));
    }
    let _guard = state.begin_mutation();
    std::fs::remove_file(&target).map_err(|err| AppError::new("Io", format!("could not remove {}: {err}", target.display())))?;
    // A now-empty parent directory left behind by a folder-mode stage is cosmetic
    // clutter, not a correctness problem (nothing reads directory *presence*),
    // so it's left alone rather than pruned -- `clear_staged_sources` already
    // handles a full reset.
    Ok(peek_staged_sources(&staging_dir))
}

fn bundle_key_of(bundle_dir: &Path) -> String {
    bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
}

// ---------------------------------------------------------------------------
// Individual step commands (§4.3: "not optional" -- each independently useful,
// not just internal pipeline plumbing). Each wraps its step + the derived
// `sync_okf_bundle` in `sopkb_review::with_bundle_lock` -- nothing at the
// `sopkb-core`/`sopkb-mining`/`sopkb-review::validate_bundle` level locks this
// two-call sequence itself (only `sopkb-review`'s five review actions and
// `sopkb-workbench`'s own multi-step orchestration do), so this is the first
// wrap, not a redundant one, and matches P-W1's rationale: a desktop UI can fire
// commands far faster than a human clicking through a browser. Also wrapped in
// `WorkbenchHandle::begin_mutation()` per §4.3's own instruction.
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn scan_sources(app: AppHandle, state: State<AppState>, source_dir: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let source_path = PathBuf::from(&source_dir);
    let _guard = state.begin_mutation();

    emit_ingest_progress(&app, "scan", "started", source_dir.clone());
    let outcome = sopkb_review::with_bundle_lock(&bundle_dir, || -> Result<usize, sopkb_core::error::SopkbError> {
        let sources = sopkb_core::inventory::scan_sources(&source_path, &bundle_dir)?;
        sopkb_export::sync_okf_bundle(&bundle_dir)?;
        Ok(sources.len())
    });
    let sources_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            emit_ingest_progress(&app, "scan", "failed", e.to_string());
            return Err(AppError::from(e));
        }
    };
    emit_ingest_progress(&app, "scan", "done", format!("{sources_count} sources"));
    emit_bundle_state_changed(&app, &bundle_key, &["inventory", "okf"]);

    let inventory = sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", serde_json::json!({"warnings": []})).map_err(AppError::from)?;
    let warnings = inventory.get("warnings").cloned().unwrap_or_else(|| serde_json::json!([]));
    Ok(serde_json::json!({ "sources": sources_count, "warnings": warnings }))
}

/// Pure aggregation over the (already re-scanned) inventory JSON: sources whose
/// `normalize_sources` pass marked `parse_status: "failed"`, paired with the most
/// recent warning `normalize_sources` pushed onto that source's own `warnings` list.
pub(crate) fn extract_normalize_failures(inventory: &Value) -> Vec<Value> {
    inventory
        .get("sources")
        .and_then(|v| v.as_array())
        .map(|sources| {
            sources
                .iter()
                .filter(|s| s.get("parse_status").and_then(|v| v.as_str()) == Some("failed"))
                .map(|s| {
                    let source_id = s.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let warning = s.get("warnings").and_then(|v| v.as_array()).and_then(|w| w.last()).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    serde_json::json!({ "source_id": source_id, "warning": warning })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command(rename_all = "snake_case")]
pub fn normalize_sources(app: AppHandle, state: State<AppState>, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.begin_mutation();

    emit_ingest_progress(&app, "normalize", "started", "");
    let outcome = sopkb_review::with_bundle_lock(&bundle_dir, || -> Result<usize, sopkb_core::error::SopkbError> {
        // No provider argument on this standalone command (unlike run_ingest_steps'
        // own normalize step below) -- it's the one Tauri command with no live UI
        // caller (IngestScreen only ever drives the combined run_ingest_pipeline),
        // so it stays fixture-only rather than plumbing provider/profile_id through
        // a path nothing currently reaches.
        let sections = sopkb_core::normalize::normalize_sources(&bundle_dir, None, Some(sopkb_config::max_parallel_workers()))?;
        sopkb_export::sync_okf_bundle(&bundle_dir)?;
        Ok(sections.len())
    });
    let sections_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            emit_ingest_progress(&app, "normalize", "failed", e.to_string());
            return Err(AppError::from(e));
        }
    };
    emit_ingest_progress(&app, "normalize", "done", format!("{sections_count} sections"));
    emit_bundle_state_changed(&app, &bundle_key, &["sections", "okf"]);

    let inventory = sopkb_core::store::read_state_json(&bundle_dir, "inventory.json", serde_json::json!({"sources": []})).map_err(AppError::from)?;
    let failed = extract_normalize_failures(&inventory);
    Ok(serde_json::json!({ "sections": sections_count, "failed": failed }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn mine_knowledge(app: AppHandle, state: State<AppState>, provider: String, profile_id: Option<String>, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.begin_mutation();

    emit_ingest_progress(&app, "mine", "started", format!("provider={provider}"));
    let on_progress = |done: usize, total: usize| emit_ingest_progress(&app, "mine", "progress", format!("{done}/{total} sections mined"));
    let is_cancelled = || state.is_cancel_requested();
    let outcome = sopkb_review::with_bundle_lock(&bundle_dir, || -> Result<usize, sopkb_core::error::SopkbError> {
        let items = sopkb_mining::mine_bundle(&bundle_dir, &provider, profile_id.as_deref(), Some(&on_progress), Some(&is_cancelled))?;
        sopkb_export::sync_okf_bundle(&bundle_dir)?;
        Ok(items.len())
    });
    let items_count = match outcome {
        Ok(n) => n,
        Err(e) => {
            emit_ingest_progress(&app, "mine", "failed", e.to_string());
            return Err(AppError::from(e));
        }
    };
    emit_ingest_progress(&app, "mine", "done", format!("{items_count} items"));
    emit_bundle_state_changed(&app, &bundle_key, &["items", "okf"]);
    Ok(serde_json::json!({ "items": items_count, "provider": provider }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn validate_bundle(app: AppHandle, state: State<AppState>, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.begin_mutation();

    emit_ingest_progress(&app, "validate", "started", "");
    // `sopkb_review::validate_bundle` already writes the six reports and calls
    // `sync_okf_bundle` internally (per its own doc comment); wrapped in
    // `with_bundle_lock` here regardless since that internal sequence is not
    // itself confirmed to take the lock.
    let outcome = sopkb_review::with_bundle_lock(&bundle_dir, || sopkb_review::validate_bundle(&bundle_dir));
    let (errors, warnings) = match outcome {
        Ok(pair) => pair,
        Err(e) => {
            emit_ingest_progress(&app, "validate", "failed", e.to_string());
            return Err(AppError::from(e));
        }
    };
    emit_ingest_progress(&app, "validate", "done", format!("{} errors, {} warnings", errors.len(), warnings.len()));
    emit_bundle_state_changed(&app, &bundle_key, &["okf"]);
    Ok(serde_json::json!({ "errors": errors, "warnings": warnings }))
}

/// "Delete a source" (Sources screen) is really "retire" it -- `sopkb_review::
/// retire_source` already exists, fully ported from `source_lifecycle.py`, and was
/// already reachable from `sopkb-cli sources retire` -- but had no Tauri command
/// wiring it into the desktop app at all (the same "ported but unreachable from the
/// UI" gap `pick_source_folder` had before). Deliberately non-destructive, matching
/// that function's own doc comment: the source and its still-active knowledge items
/// are marked `retired` (reversible in principle, fully audit-logged), never
/// deleted -- originals, normalized text, and evidence all survive on disk.
///
/// Ends in the same full `validate_bundle` pass the five review actions do, so this
/// runs `async` + `spawn_blocking` from the start rather than risk reproducing the
/// exact "Comment hangs the UI" bug already found and fixed for those actions.
#[tauri::command(rename_all = "snake_case")]
pub async fn retire_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source_id: String,
    rationale: String,
    reviewer: Option<String>,
    key: Option<String>,
) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let resolved_reviewer = crate::commands::review::resolve_reviewer(reviewer.as_deref());
    let bundle_key = bundle_key_of(&bundle_dir);
    let bundle_dir_for_task = bundle_dir.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        sopkb_review::retire_source(&bundle_dir_for_task, &source_id, &resolved_reviewer, &rationale)
    })
    .await
    .map_err(|err| AppError::new("Io", format!("retire_source task did not complete: {err}")))?
    .map_err(AppError::from)?;
    emit_bundle_state_changed(&app, &bundle_key, &["inventory", "items", "okf"]);
    Ok(serde_json::json!({ "retired": outcome.did_change(), "event": outcome.event() }))
}

// ---------------------------------------------------------------------------
// run_ingest_pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IngestSourceWire {
    Staged,
    Folder { path: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestRequestWire {
    pub source: IngestSourceWire,
    pub scan: bool,
    pub normalize: bool,
    pub mine: bool,
    pub validate: bool,
    pub export: bool,
    pub mine_provider: String,
    pub profile_id: Option<String>,
    pub uploaded_file_count: Option<usize>,
    pub key: Option<String>,
}

fn resolve_source_dir(bundle_dir: &Path, source: &IngestSourceWire) -> PathBuf {
    match source {
        IngestSourceWire::Staged => staging_dir_for(bundle_dir),
        IngestSourceWire::Folder { path } => PathBuf::from(path),
    }
}

/// The five checkbox-driven ingest steps whose outcome is persisted across
/// reloads (`.sopkb/ingest_run.json`) -- matches the Python original's
/// `PIPELINE_STEPS` list on the `ui/sopkb-web-redesign` branch's
/// `tools/sopkb/sopkb/web_app.py` (THIS branch's own tools/sopkb has no such
/// list at all -- confirmed via `grep -r ingest_run tools/sopkb/` returning
/// zero hits here; see docs/port/CATCHUP_PLAN.md's "ui/sopkb-web-redesign
/// branch scan" idea #4, 2026-08-22). Deliberately excludes the Rust-only
/// derived "sync" step (this file's own step 5, gated on "did anything else
/// run" rather than its own checkbox) -- Python has no equivalent step to
/// persist a status for.
const INGEST_RUN_STEPS: [&str; 5] = ["scan", "normalize", "mine", "validate", "export"];

/// Persisted last-ingest-run status, `.sopkb/ingest_run.json`. Enables
/// resume-from-failure across reloads: naming which step failed and why, and
/// letting the frontend default already-`"done"` steps' checkboxes to
/// unchecked so resubmitting only re-runs what's left.
///
/// Ported from the `ui/sopkb-web-redesign` branch's `tools/sopkb/sopkb/web_app.py`
/// (that branch, not this one -- its actual source, pulled via `git show
/// origin/ui/sopkb-web-redesign:tools/sopkb/sopkb/web_app.py`):
///
/// ```python
/// def ingest_run_path(bundle_dir: Path) -> Path:              # line 915-916
///     return state_path(bundle_dir, "ingest_run.json")
///
/// def read_last_ingest_run(bundle_dir: Path) -> dict[str, object]:  # line 919-921
///     data = read_state_json(bundle_dir, "ingest_run.json", {})
///     return data if isinstance(data, dict) else {}
///
/// def write_ingest_run(                                        # line 924-933
///     bundle_dir: Path, status: dict[str, str], detail: dict[str, str], *, ok: bool,
/// ) -> None:
///     write_json(ingest_run_path(bundle_dir),
///         {"ok": ok, "finished_at": utc_now(), "status": status, "detail": detail})
/// ```
///
/// `step_checked(step)` (same file, lines 970-971) defaults a step's checkbox
/// to UNCHECKED only when `status.get(step) == "done"`; the resume banner
/// (lines 976-984) shows only when `not last_run.get("ok", True)`, naming the
/// first step whose status is `"error"`.
///
/// Written ONCE at the end of a `run_ingest_steps` call (success or failure),
/// not incrementally per step, matching the Python original exactly. Every
/// one of `INGEST_RUN_STEPS` not actually attempted (its checkbox was off, or
/// an earlier step's failure stopped the pipeline before reaching it) keeps
/// status `"pending"` -- CATCHUP_PLAN.md's own idea #4 summary calls this
/// third state "skipped", but the real source uses `"pending"`; this uses the
/// real source's literal string.
///
/// A fourth status, `"cancelled"`, has no Python equivalent (Python has no
/// cancel feature at all) -- a step whose own work was cancelled PARTWAY
/// THROUGH (normalize/mine, the only two that thread `is_cancelled`
/// internally) gets `"cancelled"`, not `"done"`, even though it returned a
/// real, non-empty result: `step_checked`'s "only \"done\" unchecks" rule
/// means recording a partially-cancelled step as `"done"` would make the next
/// run's checkbox default to unchecked, silently hiding the fact that some
/// sources/sections were skipped and never re-offering to redo them. A step
/// cancelled BEFORE it ever started (the `is_cancelled()` guard immediately
/// before each step's own `if` block) never reaches its success/failure match
/// arm at all and correctly keeps `"pending"` instead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestRunStatus {
    pub ok: bool,
    pub finished_at: String,
    pub status: BTreeMap<String, String>,
    pub detail: BTreeMap<String, String>,
}

/// `.sopkb/ingest_run.json`'s read side, via `sopkb_core::store::read_state_json`
/// (same helper `inventory.json`/`sections.json`/etc. all go through). Returns
/// `None` for "no prior run" (file missing entirely, OR present but not
/// matching this shape -- e.g. a bare `{}`, mirroring Python's
/// `data if isinstance(data, dict) else {}` coercion, which `render_ingest`
/// then treats as nothing-to-resume). A genuinely CORRUPT file (malformed
/// JSON syntax) still propagates as a real `Err`, matching
/// `sopkb_core::store::read_json`'s "never silently swallow a parse error"
/// contract -- only a well-formed-but-wrong-shape value maps to `None`.
pub(crate) fn read_last_ingest_run(bundle_dir: &Path) -> Result<Option<IngestRunStatus>, sopkb_core::error::SopkbError> {
    let value = sopkb_core::store::read_state_json(bundle_dir, "ingest_run.json", serde_json::json!({}))?;
    Ok(serde_json::from_value(value).ok())
}

/// `.sopkb/ingest_run.json`'s write side, via `sopkb_core::store::write_state_json`
/// (-> `write_json` -> `write_text_native` -> the crate-private `write_atomic`) --
/// the same atomic write-to-temp-then-rename path every other `.sopkb/*.json`
/// file in this codebase goes through, NOT a raw `std::fs::write` (which
/// truncates-then-writes and can leave a half-written status file on a
/// crash/forced-quit mid-write -- see `write_atomic`'s own doc comment on why
/// that was "the single largest correctness gap in the Rust core").
fn write_last_ingest_run(bundle_dir: &Path, run_status: &IngestRunStatus) -> Result<(), sopkb_core::error::SopkbError> {
    let value = serde_json::to_value(run_status).map_err(|e| sopkb_core::error::SopkbError::Value(e.to_string()))?;
    sopkb_core::store::write_state_json(bundle_dir, "ingest_run.json", &value)
}

/// Mirrors `sopkb_workbench::ingest::run_ingest_pipeline_locked`'s exact step
/// order and step-5 (`sync_okf_bundle`) derivation rule ("runs iff any of steps
/// 1-4 ran, regardless of `export`") -- re-implemented here, calling the same
/// underlying `sopkb-core`/`sopkb-mining`/`sopkb-review`/`sopkb-export` functions
/// directly, rather than delegating to that single blocking function, PURELY to
/// get real per-step `ingest://progress` events (§4.3: "must be async... mining is
/// one sequential HTTP call per section with no timeout budget headroom -- a
/// synchronous command would freeze the single-window app," and the individual
/// step commands above already need this exact sequence anyway). If
/// `run_ingest_pipeline_locked`'s order or step-5 gating ever changes, this must
/// change with it -- there is no way to derive one from the other automatically.
///
/// ALSO persists `.sopkb/ingest_run.json` exactly once, right before returning
/// (success or failure) -- see `IngestRunStatus`'s doc comment. A failure to
/// persist that file is logged and swallowed, never propagated: it's a
/// best-effort resume aid, and must never mask the pipeline's own real
/// success/failure, which is what the caller actually awaits.
/// `is_cancelled` is checked immediately before EACH of the five checkbox-driven
/// steps (never mid-step -- see `mine_with_author`'s doc comment for why a hard
/// mid-flight abort isn't offered). A step that never starts because cancellation
/// was already requested emits a `"cancelled"` progress status for that step and
/// the whole run stops there -- `Ok`, not `Err` (cancellation is a graceful early
/// stop, not a failure) -- so `any_ran`-derived sync/the persisted
/// `.sopkb/ingest_run.json` still reflect exactly what actually happened, with
/// every not-yet-reached step correctly left at its default `"pending"` status.
#[allow(clippy::too_many_arguments)]
fn run_ingest_steps(
    app: &AppHandle,
    bundle_dir: &Path,
    source_dir: &Path,
    scan: bool,
    normalize: bool,
    mine: bool,
    validate: bool,
    export: bool,
    mine_provider: &str,
    profile_id: Option<&str>,
    uploaded_file_count: Option<usize>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<sopkb_workbench::IngestResult, sopkb_core::error::SopkbError> {
    let mut result = sopkb_workbench::IngestResult { uploaded_files: uploaded_file_count, ..Default::default() };
    let mut any_ran = false;
    let mut status: BTreeMap<String, String> = INGEST_RUN_STEPS.iter().map(|s| (s.to_string(), "pending".to_string())).collect();
    let mut detail: BTreeMap<String, String> = BTreeMap::new();

    let outcome: Result<(), sopkb_core::error::SopkbError> = (|| {
        if scan && is_cancelled() {
            emit_ingest_progress(app, "scan", "cancelled", "");
            return Ok(());
        }
        if scan {
            emit_ingest_progress(app, "scan", "started", "");
            match sopkb_core::inventory::scan_sources(source_dir, bundle_dir) {
                Ok(sources) => {
                    result.sources = Some(sources.len());
                    any_ran = true;
                    let msg = format!("{} sources", sources.len());
                    emit_ingest_progress(app, "scan", "done", msg.clone());
                    status.insert("scan".to_string(), "done".to_string());
                    detail.insert("scan".to_string(), msg);
                }
                Err(e) => {
                    let msg = e.to_string();
                    emit_ingest_progress(app, "scan", "failed", msg.clone());
                    status.insert("scan".to_string(), "error".to_string());
                    detail.insert("scan".to_string(), msg);
                    return Err(e);
                }
            }
        }
        if normalize && is_cancelled() {
            emit_ingest_progress(app, "normalize", "cancelled", "");
            return Ok(());
        }
        if normalize {
            emit_ingest_progress(app, "normalize", "started", "");
            // Reuses this run's mine_provider/profile_id -- see
            // sopkb_workbench::ingest's identical choice for why normalize doesn't
            // get its own, independent provider selection.
            let on_normalize_progress =
                |done: usize, total: usize| emit_ingest_progress(app, "normalize", "progress", format!("chunk {done}/{total} indexed"));
            let log_warning = |message: &str| sopkb_core::store::append_ingest_log(bundle_dir, message);
            let restructure =
                sopkb_workbench::provider_hook(mine_provider, profile_id, Some(&on_normalize_progress), Some(is_cancelled), Some(&log_warning));
            match sopkb_core::normalize::normalize_sources(bundle_dir, restructure.as_deref(), Some(sopkb_config::max_parallel_workers())) {
                Ok(sections) => {
                    result.sections = Some(sections.len());
                    any_ran = true;
                    let msg = format!("{} sections", sections.len());
                    if is_cancelled() {
                        // Cancellation was requested WHILE this step was running (the
                        // guard above only catches a cancel requested BEFORE the step
                        // started): some sources' restructure calls were skipped, so
                        // `sections` is real but partial. Recording "done" here would
                        // make the resume UI's `step_checked` logic (see
                        // IngestRunStatus's doc comment) believe normalize fully
                        // finished and pre-uncheck it on the next run -- silently
                        // leaving those sources under-restructured forever.
                        emit_ingest_progress(app, "normalize", "cancelled", msg.clone());
                        status.insert("normalize".to_string(), "cancelled".to_string());
                    } else {
                        emit_ingest_progress(app, "normalize", "done", msg.clone());
                        status.insert("normalize".to_string(), "done".to_string());
                    }
                    detail.insert("normalize".to_string(), msg);
                }
                Err(e) => {
                    let msg = e.to_string();
                    emit_ingest_progress(app, "normalize", "failed", msg.clone());
                    status.insert("normalize".to_string(), "error".to_string());
                    detail.insert("normalize".to_string(), msg);
                    return Err(e);
                }
            }
        }
        if mine && is_cancelled() {
            emit_ingest_progress(app, "mine", "cancelled", "");
            return Ok(());
        }
        if mine {
            emit_ingest_progress(app, "mine", "started", format!("provider={mine_provider}"));
            let on_progress = |done: usize, total: usize| emit_ingest_progress(app, "mine", "progress", format!("{done}/{total} sections mined"));
            match sopkb_mining::mine_bundle(bundle_dir, mine_provider, profile_id, Some(&on_progress), Some(is_cancelled)) {
                Ok(items) => {
                    result.items = Some(items.len());
                    result.mine_provider = Some(mine_provider.to_string());
                    any_ran = true;
                    let msg = format!("{} items", items.len());
                    if is_cancelled() {
                        // Same reasoning as normalize's identical check above: some
                        // sections were skipped by mine_with_author's own per-section
                        // is_cancelled check, so `items` is real but partial -- must
                        // not be recorded as "done".
                        emit_ingest_progress(app, "mine", "cancelled", msg.clone());
                        status.insert("mine".to_string(), "cancelled".to_string());
                    } else {
                        emit_ingest_progress(app, "mine", "done", msg.clone());
                        status.insert("mine".to_string(), "done".to_string());
                    }
                    detail.insert("mine".to_string(), msg);
                }
                Err(e) => {
                    let msg = e.to_string();
                    emit_ingest_progress(app, "mine", "failed", msg.clone());
                    status.insert("mine".to_string(), "error".to_string());
                    detail.insert("mine".to_string(), msg);
                    return Err(e);
                }
            }
        }
        if validate && is_cancelled() {
            emit_ingest_progress(app, "validate", "cancelled", "");
            return Ok(());
        }
        if validate {
            emit_ingest_progress(app, "validate", "started", "");
            match sopkb_review::validate_bundle(bundle_dir) {
                Ok((errors, warnings)) => {
                    result.validation = Some(sopkb_workbench::ValidationCounts { errors: errors.len(), warnings: warnings.len() });
                    any_ran = true;
                    let msg = format!("{} errors, {} warnings", errors.len(), warnings.len());
                    emit_ingest_progress(app, "validate", "done", msg.clone());
                    status.insert("validate".to_string(), "done".to_string());
                    detail.insert("validate".to_string(), msg);
                }
                Err(e) => {
                    let msg = e.to_string();
                    emit_ingest_progress(app, "validate", "failed", msg.clone());
                    status.insert("validate".to_string(), "error".to_string());
                    detail.insert("validate".to_string(), msg);
                    return Err(e);
                }
            }
        }
        if any_ran {
            emit_ingest_progress(app, "sync", "started", "");
            match sopkb_export::sync_okf_bundle(bundle_dir) {
                Ok(okf_bundle) => {
                    result.okf_bundle = Some(okf_bundle);
                    emit_ingest_progress(app, "sync", "done", "");
                }
                Err(e) => {
                    emit_ingest_progress(app, "sync", "failed", e.to_string());
                    // Not one of INGEST_RUN_STEPS (Python has no "sync" checkbox to
                    // persist a status for -- see IngestRunStatus's doc comment):
                    // surfaced via `ok` only, not attributed to any single step.
                    return Err(e);
                }
            }
        }
        if export && is_cancelled() {
            emit_ingest_progress(app, "export", "cancelled", "");
            return Ok(());
        }
        if export {
            emit_ingest_progress(app, "export", "started", "");
            let formats: Vec<String> = sopkb_workbench::DEFAULT_EXPORT_FORMATS.iter().map(|s| s.to_string()).collect();
            match sopkb_export::export_bundle(bundle_dir, &formats) {
                Ok(exports) => {
                    let msg = format!("{} artifacts", exports.len());
                    emit_ingest_progress(app, "export", "done", msg.clone());
                    result.exports = Some(exports);
                    status.insert("export".to_string(), "done".to_string());
                    detail.insert("export".to_string(), msg);
                }
                Err(e) => {
                    let msg = e.to_string();
                    emit_ingest_progress(app, "export", "failed", msg.clone());
                    status.insert("export".to_string(), "error".to_string());
                    detail.insert("export".to_string(), msg);
                    return Err(e);
                }
            }
        }
        Ok(())
    })();

    let run_status = IngestRunStatus { ok: outcome.is_ok(), finished_at: sopkb_core::store::utc_now(), status, detail };
    if let Err(persist_err) = write_last_ingest_run(bundle_dir, &run_status) {
        eprintln!("failed to persist .sopkb/ingest_run.json: {persist_err}");
    }

    outcome.map(|_| result)
}

/// Requests that the CURRENTLY RUNNING ingest pipeline (if any) stop starting new
/// work. Cooperative, not instant -- see `WorkbenchHandle`'s `cancel_requested`
/// field and `run_ingest_steps`'s own doc comment for exactly what this does and
/// does not guarantee. Safe to call with nothing running (a harmless no-op the next
/// real run clears via `clear_cancel_request`); does not itself emit any
/// `ingest://progress` event -- the run being cancelled emits its own
/// `"cancelled"` status for whichever step it stops before starting.
#[tauri::command(rename_all = "snake_case")]
pub fn cancel_ingest(state: State<AppState>) -> CmdResult<()> {
    state.request_cancel();
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn run_ingest_pipeline(app: AppHandle, state: State<'_, AppState>, request: IngestRequestWire) -> CmdResult<WireIngestResult> {
    let bundle_dir = resolve_bundle_dir(&state, request.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let source_dir = resolve_source_dir(&bundle_dir, &request.source);
    if !source_dir.is_dir() {
        return Err(AppError::not_found(format!("source directory does not exist: {}", source_dir.display())));
    }

    let _guard = state.begin_mutation();
    // A cancellation requested during a PREVIOUS run must not immediately kill this
    // new one -- see `WorkbenchHandle::clear_cancel_request`'s own doc comment.
    state.clear_cancel_request();
    let IngestRequestWire { scan, normalize, mine, validate, export, mine_provider, profile_id, uploaded_file_count, .. } = request;

    let app_for_task = app.clone();
    let bundle_dir_for_task = bundle_dir.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        // `tauri::State<'_, AppState>` borrows from the command invocation's own
        // lifetime, which `spawn_blocking`'s `'static` bound rejects directly --
        // `AppHandle` (already cloned for the task above) is itself `'static` and can
        // re-fetch the exact same managed `AppState` from inside the blocking thread.
        let is_cancelled = || app_for_task.state::<AppState>().is_cancel_requested();
        sopkb_review::with_bundle_lock(&bundle_dir_for_task, || {
            run_ingest_steps(
                &app_for_task,
                &bundle_dir_for_task,
                &source_dir,
                scan,
                normalize,
                mine,
                validate,
                export,
                &mine_provider,
                profile_id.as_deref(),
                uploaded_file_count,
                &is_cancelled,
            )
        })
    })
    .await
    .map_err(|err| AppError::new("Io", format!("ingest pipeline task did not complete: {err}")))?;

    let result = outcome.map_err(AppError::from)?;

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
        emit_bundle_state_changed(&app, &bundle_key, &scope);
    }

    Ok(result.into())
}

/// Reads `.sopkb/ingest_run.json` for the resolved bundle. `None` is a NORMAL
/// first-run state (no prior run yet) -- never an error -- matching
/// `get_staged_sources` just above's identical "peek at previous state, `None`
/// if there isn't any" shape.
#[tauri::command(rename_all = "snake_case")]
pub fn get_last_ingest_run(state: State<AppState>, key: Option<String>) -> CmdResult<Option<IngestRunStatus>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    read_last_ingest_run(&bundle_dir).map_err(AppError::from)
}

// ---------------------------------------------------------------------------
// preview_ingest_pipeline (§4.3 catch-up item 3: "Preview source changes",
// docs/port/CATCHUP_PLAN.md Workstream 4). Read-only dry run -- shows what a
// real run would touch (path / classification / source id) without executing
// any pipeline step, so a destructive `run_ingest_pipeline` isn't the first
// look a user gets at what's about to change.
// ---------------------------------------------------------------------------

#[tauri::command(rename_all = "snake_case")]
pub fn preview_ingest_pipeline(state: State<AppState>, source: IngestSourceWire, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let source_dir = resolve_source_dir(&bundle_dir, &source);
    if !source_dir.is_dir() {
        return Err(AppError::not_found(format!("source directory does not exist: {}", source_dir.display())));
    }
    let mut result = sopkb_core::inventory::classify_source_updates(&source_dir, &bundle_dir).map_err(AppError::from)?;
    if let Value::Object(ref mut map) = result {
        map.insert("source_dir".to_string(), Value::String(source_dir.display().to_string()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_names_for_stage_files_mode_uses_basenames_only() {
        let paths = vec![PathBuf::from("/tmp/a/policy.md"), PathBuf::from("/tmp/b/notes.txt")];
        assert_eq!(relative_names_for_stage(&paths, "files"), vec!["policy.md".to_string(), "notes.txt".to_string()]);
    }

    #[test]
    fn relative_names_for_stage_folder_mode_preserves_relative_structure() {
        let paths = vec![
            PathBuf::from("/tmp/pick/policies/simple.md"),
            PathBuf::from("/tmp/pick/policies/nested/deep.md"),
            PathBuf::from("/tmp/pick/notes.md"),
        ];
        let names = relative_names_for_stage(&paths, "folder");
        assert_eq!(names, vec!["policies/simple.md".to_string(), "policies/nested/deep.md".to_string(), "notes.md".to_string()]);
    }

    #[test]
    fn relative_names_for_stage_folder_mode_single_file_falls_back_to_basename() {
        let paths = vec![PathBuf::from("/tmp/pick/only.md")];
        assert_eq!(relative_names_for_stage(&paths, "folder"), vec!["only.md".to_string()]);
    }

    #[test]
    fn relative_names_for_stage_folder_mode_no_common_ancestor_falls_back_to_basename() {
        // Different drive roots on Windows / disjoint absolute roots in general.
        let paths = vec![PathBuf::from("C:/a/one.md"), PathBuf::from("D:/b/two.md")];
        let names = relative_names_for_stage(&paths, "folder");
        assert_eq!(names, vec!["one.md".to_string(), "two.md".to_string()]);
    }

    #[test]
    fn peek_staged_sources_none_when_directory_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(peek_staged_sources(&dir.path().join("does-not-exist")).is_none());
    }

    #[test]
    fn peek_staged_sources_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        assert!(peek_staged_sources(&staging).is_none());
    }

    #[test]
    fn peek_staged_sources_counts_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(staging.join("sub")).unwrap();
        std::fs::write(staging.join("a.md"), "a").unwrap();
        std::fs::write(staging.join("sub").join("b.md"), "b").unwrap();
        let staged = peek_staged_sources(&staging).unwrap();
        assert_eq!(staged.file_count, 2);
        assert_eq!(staged.files, vec!["a.md".to_string(), "sub/b.md".to_string()]);
    }

    #[test]
    fn list_files_recursive_returns_forward_slash_relative_paths_sorted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("z")).unwrap();
        std::fs::write(dir.path().join("z").join("last.md"), "z").unwrap();
        std::fs::write(dir.path().join("first.md"), "a").unwrap();
        assert_eq!(list_files_recursive(dir.path()), vec!["first.md".to_string(), "z/last.md".to_string()]);
    }

    #[test]
    fn extract_normalize_failures_finds_only_failed_sources_with_last_warning() {
        let inventory = serde_json::json!({
            "sources": [
                {"id": "s1", "parse_status": "normalized", "warnings": []},
                {"id": "s2", "parse_status": "failed", "warnings": ["first warning", "normalization failed: bad encoding"]},
            ]
        });
        let failures = extract_normalize_failures(&inventory);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0]["source_id"], serde_json::json!("s2"));
        assert_eq!(failures[0]["warning"], serde_json::json!("normalization failed: bad encoding"));
    }

    #[test]
    fn extract_normalize_failures_empty_when_none_failed() {
        let inventory = serde_json::json!({"sources": [{"id": "s1", "parse_status": "normalized"}]});
        assert!(extract_normalize_failures(&inventory).is_empty());
    }

    #[test]
    fn extract_normalize_failures_missing_sources_key_is_empty() {
        assert!(extract_normalize_failures(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn resolve_source_dir_staged_points_at_current_uploads() {
        let bundle_dir = PathBuf::from("/bundle");
        let resolved = resolve_source_dir(&bundle_dir, &IngestSourceWire::Staged);
        assert_eq!(resolved, staging_dir_for(&bundle_dir));
    }

    #[test]
    fn resolve_source_dir_folder_uses_the_given_path_verbatim() {
        let bundle_dir = PathBuf::from("/bundle");
        let resolved = resolve_source_dir(&bundle_dir, &IngestSourceWire::Folder { path: "/elsewhere/sources".to_string() });
        assert_eq!(resolved, PathBuf::from("/elsewhere/sources"));
    }

    #[test]
    fn ingest_source_wire_deserializes_tagged_shape() {
        let staged: IngestSourceWire = serde_json::from_value(serde_json::json!({"kind": "staged"})).unwrap();
        assert!(matches!(staged, IngestSourceWire::Staged));
        let folder: IngestSourceWire = serde_json::from_value(serde_json::json!({"kind": "folder", "path": "/x"})).unwrap();
        assert!(matches!(folder, IngestSourceWire::Folder { path } if path == "/x"));
    }

    #[test]
    fn read_last_ingest_run_none_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_last_ingest_run(dir.path()).unwrap().is_none());
    }

    #[test]
    fn read_last_ingest_run_none_when_prior_file_is_empty_object() {
        // Matches Python's `data if isinstance(data, dict) else {}` coercion:
        // a `{}` file reads back as "no prior run" for the frontend's
        // purposes, not a hard schema error.
        let dir = tempfile::tempdir().unwrap();
        sopkb_core::store::write_state_json(dir.path(), "ingest_run.json", &serde_json::json!({})).unwrap();
        assert!(read_last_ingest_run(dir.path()).unwrap().is_none());
    }

    #[test]
    fn read_last_ingest_run_round_trips_a_written_status() {
        let dir = tempfile::tempdir().unwrap();
        let mut status = BTreeMap::new();
        status.insert("scan".to_string(), "done".to_string());
        status.insert("normalize".to_string(), "error".to_string());
        let mut detail = BTreeMap::new();
        detail.insert("scan".to_string(), "3 sources".to_string());
        detail.insert("normalize".to_string(), "boom".to_string());
        let run_status = IngestRunStatus { ok: false, finished_at: "2026-08-22T00:00:00Z".to_string(), status, detail };
        write_last_ingest_run(dir.path(), &run_status).unwrap();

        let read_back = read_last_ingest_run(dir.path()).unwrap().unwrap();
        assert!(!read_back.ok);
        assert_eq!(read_back.status.get("scan").map(String::as_str), Some("done"));
        assert_eq!(read_back.status.get("normalize").map(String::as_str), Some("error"));
        assert_eq!(read_back.detail.get("normalize").map(String::as_str), Some("boom"));
    }
}
