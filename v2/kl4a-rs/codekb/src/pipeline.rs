//! Port of `kl4a/codekb/pipeline.py`.
//!
//! The whole-repo C0-C5 code-bundle pipeline: create → scan → parse →
//! relations → (optional) architecture → mine → (optional) render →
//! validate, plus the confirmation-before-rebuild logic
//! ([`rebuild_risk`]/[`same_repo`]) and the asynchronous entry point
//! ([`start_code_bundle_build`]) that runs the whole thing on a worker
//! thread and returns immediately. Ported symbol-for-symbol from the
//! Python source (verified via tools-code MCP `code_symbols_get`).
//!
//! MCP evidence for `start_code_bundle_build`/`test_code_pipeline_async.py`
//! confirms this runs on an OS thread (`threading.Thread`) with a
//! `RunRecorder` (`begin_stage`/`end_stage`, "second run rejected while one
//! is active", "disabled stages marked skipped not pending") — **not**
//! `asyncio`, despite the test filename. Ported to `std::thread::spawn` +
//! whatever synchronization `crate::run_state` (a sibling module, not in
//! this batch) provides, not tokio.
//!
//! ## Cross-batch dependencies (NOT part of this batch — report only)
//!
//! - `crate::config::standard_code_bundle_dir(repo_dir: &Path) -> PathBuf`
//! - `crate::bundle::create_code_bundle(bundle_dir, repo_dir, title: Option<&str>,
//!   mining_mode: Option<&str>) -> Result<()>`
//! - `crate::inventory::scan_code_repo(repo_dir, bundle_dir) -> Result<Value>`
//!   (returns `{"sources": [...]}`)
//! - `crate::architecture::detect_architecture(bundle_dir) -> Result<Value>`
//!   (returns `{"summary": {"endpoints": N, ...}, ...}`)
//! - `crate::knowledge::{mine_code_bundle, HYBRID_PROVIDERS, CodeAuthorFn}` —
//!   `mine_code_bundle(bundle_dir, provider: &str, author: Option<Arc<CodeAuthorFn>>,
//!   procedure_author: Option<Arc<CodeAuthorFn>>) -> Result<Value>` (returns
//!   `{"items": [...], "enrichment": {"attempted": bool, "accepted": N,
//!   "rejected": [...] , ...}}`); `HYBRID_PROVIDERS` is a provider-id set
//!   (`{"hybrid", "llm", "azure-llm"} | LLM_PROVIDER_IDS` — the exact
//!   membership depends on `LLM_PROVIDER_IDS`, itself cross-batch, so
//!   treated here as an opaque `contains(&str) -> bool` predicate).
//!   `CodeAuthorFn` is assumed to be `dyn Fn(&Value) -> Result<Value> + Send
//!   + Sync + 'static` — it must be `Send + Sync + 'static` (hence `Arc`,
//!   not a borrowed reference) so `start_code_bundle_build` can hand it to
//!   a spawned worker thread; a plain Python closure has no such
//!   restriction, so this is a real, deliberate signature change forced by
//!   Rust's thread-safety rules, not an oversight.
//! - `crate::render::render_human_layer(bundle_dir) -> Result<()>`
//! - `crate::validate::validate_code_bundle(bundle_dir) -> Result<(Vec<String>, Vec<String>)>`
//!   (errors, warnings)
//! - `crate::llm_settings::apply_to_env()` — saved workbench settings stand
//!   in for exported environment variables, so hybrid mining works from the
//!   UI without the user editing their shell profile.
//! - `crate::llm_settings::active_provider_id() -> String`
//! - `crate::bundle_store::load_manifest(bundle_dir) -> Result<Value>`
//! - `crate::state::{read_json, STATE_DIR}`
//! - `crate::run_state::{new_run_state, start_background_run, RunRecorder}` —
//!   `new_run_state(repo_dir: &str, mining_mode: &str, provider: &str,
//!   skipped: &HashSet<String>) -> Value` (a `RunState`-shaped record);
//!   `start_background_run(bundle_dir, state, work: impl FnOnce(&dyn Recorder)
//!   -> Result<Value> + Send + 'static) -> Value` (spawns the worker thread,
//!   returns a "started" summary immediately — this is *the* `threading.Thread`
//!   call site; rejects a second concurrent run per the MCP-confirmed
//!   behavior above). `RunRecorder` is expected to implement the [`Recorder`]
//!   trait defined below.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::bundle::create_code_bundle;
use crate::bundle_store::{display_path, load_manifest};
use crate::config::standard_code_bundle_dir;
use crate::architecture::detect_architecture;
use crate::inventory::scan_code_repo;
use crate::knowledge::{self, mine_code_bundle};
use crate::llm_settings::{active_provider_id, apply_to_env};
use crate::parse::parse_code_bundle;
use crate::relations::generate_code_relations;
use crate::render::render_human_layer;
use crate::run_state::{new_run_state, start_background_run};
use crate::state::{read_json, STATE_DIR};
use crate::validate::validate_code_bundle;

/// Port of `kl4a.codekb.pipeline.DEFAULT_STATIC_PROVIDER`.
pub const DEFAULT_STATIC_PROVIDER: &str = "fixture";

/// Progress-reporting hook a pipeline run accepts. Both
/// [`NullRecorder`] (this file) and `crate::run_state::RunRecorder`
/// (a sibling batch's real, stateful recorder) implement this trait, so
/// `build_code_bundle_from_repo`/`start_code_bundle_build` can accept
/// either behind a `&mut dyn Recorder` — mirroring Python's duck-typed
/// `recorder: Any` parameter with `begin_stage`/`end_stage` calls.
///
/// The trait itself now lives in `crate::run_state` (`RunRecorder`'s home);
/// an earlier batch had independently defined an identical-shaped trait
/// here for `NullRecorder`. Reconciled to one definition per the
/// coordinator's integration pass — this file only implements it.
pub use crate::run_state::Recorder;

/// Port of `kl4a.codekb.pipeline._NullRecorder` — a no-op [`Recorder`],
/// used when the caller passes no recorder of its own.
pub struct NullRecorder;

impl Recorder for NullRecorder {
    /// Port of `kl4a.codekb.pipeline._NullRecorder.begin_stage`.
    fn begin_stage(&mut self, _key: &str) {}
    /// Port of `kl4a.codekb.pipeline._NullRecorder.end_stage`.
    fn end_stage(&mut self, _key: &str, _detail: Option<&str>) {}
}

/// Port of `kl4a.codekb.pipeline.same_repo`.
///
/// Compares two repository paths as the filesystem would: case-insensitive
/// on Windows, and tolerant of separator/trailing-slash differences, so a
/// path typed by hand still matches the recorded one.
pub fn same_repo(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let (Ok(l), Ok(r)) = (std::fs::canonicalize(left), std::fs::canonicalize(right)) else {
        return false;
    };
    normcase(&l) == normcase(&r)
}

/// Mirrors Python's `os.path.normcase`: lower-cases on Windows (where
/// paths are case-insensitive), a no-op elsewhere.
fn normcase(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s
    }
}

/// Port of `kl4a.codekb.pipeline.rebuild_risk`.
///
/// Decides whether running the pipeline needs an explicit confirmation.
/// Confirmation appears only when there is something to lose: pointing an
/// existing bundle with active claims at a *different* repository (every
/// symbol changes, so every existing claim is retired and replaced). A
/// rerun against the same repository, or a bundle with no claims yet,
/// needs none.
pub fn rebuild_risk(bundle_dir: &Path, repo_dir: &str) -> Result<Value> {
    let recorded = configured_repo_root(bundle_dir);
    let claims = read_json(&bundle_dir.join(STATE_DIR).join("code_knowledge.json"), json!({"items": []}));
    let active: Vec<&Value> = claims
        .get("items")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|item| item.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active") == "active").collect())
        .unwrap_or_default();

    if active.is_empty() {
        return Ok(json!({
            "confirm_required": false,
            "reason": "first_run",
            "active_claims": 0,
            "recorded_repo": recorded,
        }));
    }
    if !recorded.is_empty() && !same_repo(&recorded, repo_dir) {
        return Ok(json!({
            "confirm_required": true,
            "reason": "different_repo",
            "active_claims": active.len(),
            "recorded_repo": recorded,
        }));
    }
    Ok(json!({
        "confirm_required": false,
        "reason": "same_repo",
        "active_claims": active.len(),
        "recorded_repo": recorded,
    }))
}

/// Port of `kl4a.codekb.pipeline.configured_repo_root`.
///
/// The repository a bundle was created for, as recorded in its manifest.
/// Available before the first scan, unlike the inventory's own repository
/// record.
pub fn configured_repo_root(bundle_dir: &Path) -> String {
    let Ok(manifest) = load_manifest(bundle_dir) else { return String::new() };
    manifest
        .get("codekb")
        .and_then(|c| c.get("repo_root"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// Port of `kl4a.codekb.pipeline.configured_mining`.
pub fn configured_mining(bundle_dir: &Path) -> Value {
    let Ok(manifest) = load_manifest(bundle_dir) else { return json!({}) };
    manifest.get("codekb").and_then(|c| c.get("mining")).cloned().unwrap_or_else(|| json!({}))
}

/// Port of `kl4a.codekb.pipeline.configured_mining_provider`.
///
/// Resolve the provider a bundle should mine with when none is given
/// explicitly. Mining mode is bundle configuration, chosen when the
/// repository folder path is first handed to the workbench, so later
/// `code mine` runs inherit it rather than silently reverting to static.
///
/// Hybrid mode has no bundle-recorded provider to fall back on (nothing
/// ever writes one into the manifest) — the LLM backend actually used is
/// always whatever kl4a's shared `llm_settings` currently resolves to
/// (saved setting, then environment variable, then `.env`), the same
/// mechanism sopkb's own hybrid commands rely on. Asking for that live,
/// rather than a hardcoded vendor default, keeps the reported provider
/// honest about which backend a hybrid run actually talked to.
pub fn configured_mining_provider(bundle_dir: &Path) -> String {
    let mining = configured_mining(bundle_dir);
    let mode = mining.get("mode").and_then(Value::as_str).unwrap_or("static");
    if mode == "hybrid" || mode == "llm" {
        mining.get("provider").and_then(Value::as_str).map(str::to_string).unwrap_or_else(active_provider_id)
    } else {
        DEFAULT_STATIC_PROVIDER.to_string()
    }
}

/// Options accepted by [`build_code_bundle_from_repo`], mirroring every one
/// of the Python function's keyword-only parameters (deliberately not
/// dropping any of them, however optional-looking).
pub struct BuildCodeBundleOptions<'a> {
    pub bundle_dir: Option<PathBuf>,
    pub title: Option<&'a str>,
    pub mining_mode: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub render: bool,
    pub author: Option<Arc<crate::author::CodeAuthorFn>>,
    pub procedure_author: Option<Arc<crate::author::CodeAuthorFn>>,
    pub architecture: bool,
    pub recorder: Option<&'a mut dyn Recorder>,
}

impl<'a> Default for BuildCodeBundleOptions<'a> {
    fn default() -> Self {
        BuildCodeBundleOptions {
            bundle_dir: None,
            title: None,
            mining_mode: None,
            provider: None,
            render: true,
            author: None,
            procedure_author: None,
            architecture: true,
            recorder: None,
        }
    }
}

/// Port of `kl4a.codekb.pipeline.build_code_bundle_from_repo`.
///
/// Runs the whole C0-C5 pipeline against a repository folder path: the
/// single entry point that takes a repo path and returns a validated
/// bundle, so the static/hybrid decision is made once, at the point where
/// the repository is handed over, and is then recorded in the manifest.
/// `recorder` optionally receives `begin_stage`/`end_stage` calls so a
/// caller can observe progress while the run is still going — recording is
/// reporting, not control flow, so every stage below runs unconditionally
/// except `architecture` (opt-out via `options.architecture`) and `render`
/// (opt-out via `options.render`), exactly matching the Python `if`s.
pub fn build_code_bundle_from_repo(repo_dir: &Path, options: BuildCodeBundleOptions) -> Result<Value> {
    let repo_dir = repo_dir.canonicalize().map_err(|_| anyhow!("repository directory does not exist: {}", repo_dir.display()))?;
    if !repo_dir.is_dir() {
        return Err(anyhow!("repository directory does not exist: {}", repo_dir.display()));
    }
    let bundle_dir = match options.bundle_dir {
        Some(b) => b,
        None => standard_code_bundle_dir(&repo_dir, None)?,
    };
    let mut null_recorder = NullRecorder;
    let track: &mut dyn Recorder = options.recorder.unwrap_or(&mut null_recorder);

    track.begin_stage("create");
    create_code_bundle(&bundle_dir, &repo_dir, options.title, options.mining_mode)?;
    track.end_stage("create", None);

    track.begin_stage("scan");
    let inventory = scan_code_repo(&repo_dir, &bundle_dir)?;
    let source_count = inventory.get("sources").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
    track.end_stage("scan", Some(&format!("{source_count} source(s)")));

    track.begin_stage("parse");
    let parsed = parse_code_bundle(&bundle_dir, None)?;
    let module_count = parsed.get("modules").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
    let symbol_count = parsed.get("symbols").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
    track.end_stage("parse", Some(&format!("{module_count} module(s), {symbol_count} symbol(s)")));

    track.begin_stage("relations");
    let relations = generate_code_relations(&bundle_dir)?;
    let relation_count = relations.get("relations").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
    track.end_stage("relations", Some(&format!("{relation_count} relation(s)")));

    let mut architecture_state = json!({});
    if options.architecture {
        track.begin_stage("architecture");
        architecture_state = detect_architecture(&bundle_dir)?;
        let endpoints = architecture_state.get("summary").and_then(|s| s.get("endpoints")).and_then(Value::as_i64).unwrap_or(0);
        track.end_stage("architecture", Some(&format!("{endpoints} endpoint(s)")));
    }

    let resolved_provider = options.provider.map(str::to_string).unwrap_or_else(|| configured_mining_provider(&bundle_dir));
    if knowledge::HYBRID_PROVIDERS.contains(resolved_provider.as_str()) && options.author.is_none() {
        // Saved workbench settings stand in for exported environment
        // variables, so hybrid works from the UI without the user editing
        // their shell profile.
        apply_to_env();
    }
    track.begin_stage("mine");
    let knowledge_result = mine_code_bundle(&bundle_dir, &resolved_provider, options.author.as_deref(), options.procedure_author.as_deref())?;
    let items = knowledge_result.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
    let enrichment = knowledge_result.get("enrichment").cloned().unwrap_or_else(|| json!({}));
    let mut mine_detail = format!("{} claim(s)", items.len());
    if enrichment.get("attempted").and_then(Value::as_bool).unwrap_or(false) {
        let accepted = enrichment.get("accepted").and_then(Value::as_i64).unwrap_or(0);
        let rejected = enrichment.get("rejected").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0);
        mine_detail.push_str(&format!(", LLM {accepted} accepted / {rejected} rejected"));
    }
    track.end_stage("mine", Some(&mine_detail));

    if options.render {
        track.begin_stage("render");
        render_human_layer(&bundle_dir)?;
        track.end_stage("render", None);
    }

    track.begin_stage("validate");
    let (errors, warnings) = validate_code_bundle(&bundle_dir)?;
    track.end_stage("validate", Some(&format!("{} error(s), {} warning(s)", errors.len(), warnings.len())));

    let active: Vec<&Value> = items
        .iter()
        .filter(|item| item.get("lifecycle_status").and_then(Value::as_str).unwrap_or("active") == "active")
        .collect();
    let review_required = active.iter().filter(|item| item.get("review_required").and_then(Value::as_bool).unwrap_or(false)).count();

    Ok(json!({
        // Fix (Low finding): `bundle_dir` here was built by
        // `standard_code_bundle_dir`/joined onto an already-`canonicalize()`d
        // `repo_dir`, so on Windows its `Display` leaks the `\\?\`
        // extended-length-path prefix into this user-facing summary field
        // (and from there into the CLI's "Built code bundle at ..." line —
        // confirmed behaviorally: before this fix, `codekb build` printed
        // `Built code bundle at \\?\C:\Users\...` where Python's equivalent
        // run prints the plain `C:\Users\...`). `display_path` strips it,
        // matching every other display/storage path in this crate.
        "bundle_dir": display_path(&bundle_dir),
        "mining_mode": if knowledge::HYBRID_PROVIDERS.contains(resolved_provider.as_str()) { "hybrid" } else { "static" },
        "provider": resolved_provider,
        "sources": source_count,
        "modules": module_count,
        "symbols": symbol_count,
        "relations": relation_count,
        "endpoints": architecture_state.get("summary").and_then(|s| s.get("endpoints")).cloned().unwrap_or(json!(0)),
        "knowledge_items": active.len(),
        "review_required": review_required,
        "enrichment": enrichment,
        "errors": errors.len(),
        "warnings": warnings.len(),
    }))
}

/// Options accepted by [`start_code_bundle_build`], mirroring every one of
/// the Python function's keyword-only parameters.
pub struct StartBuildOptions<'a> {
    pub title: Option<&'a str>,
    pub mining_mode: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub render: bool,
    pub architecture: bool,
    pub author: Option<Arc<crate::author::CodeAuthorFn>>,
    pub procedure_author: Option<Arc<crate::author::CodeAuthorFn>>,
}

impl<'a> Default for StartBuildOptions<'a> {
    fn default() -> Self {
        StartBuildOptions {
            title: None,
            mining_mode: None,
            provider: None,
            render: true,
            architecture: true,
            author: None,
            procedure_author: None,
        }
    }
}

/// Port of `kl4a.codekb.pipeline.start_code_bundle_build`.
///
/// Starts a pipeline run on a worker thread and returns immediately. The
/// caller gets control back at once so a long build does not hold a
/// request open; progress is readable from bundle state while the run
/// continues (see `crate::run_state`).
///
/// The nested Python `work(recorder)` closure — the actual thread body —
/// is the `work` closure passed to `start_background_run` below.
pub fn start_code_bundle_build(
    repo_dir: &Path,
    bundle_dir: &Path,
    options: StartBuildOptions,
) -> Result<std::thread::JoinHandle<()>> {
    let repo_dir = repo_dir.canonicalize().map_err(|_| anyhow!("repository directory does not exist: {}", repo_dir.display()))?;
    if !repo_dir.is_dir() {
        return Err(anyhow!("repository directory does not exist: {}", repo_dir.display()));
    }

    let mut skipped: HashSet<String> = HashSet::new();
    if !options.render {
        skipped.insert("render".to_string());
    }
    if !options.architecture {
        skipped.insert("architecture".to_string());
    }
    let resolved_provider = options.provider.map(str::to_string).unwrap_or_else(|| configured_mining_provider(bundle_dir));
    let mining_mode = if knowledge::HYBRID_PROVIDERS.contains(resolved_provider.as_str()) { "hybrid" } else { "static" };
    let skipped_refs: Vec<&str> = skipped.iter().map(String::as_str).collect();
    let state = new_run_state(&display_path(&repo_dir), mining_mode, &resolved_provider, Some(&skipped_refs));

    let bundle_dir_owned = bundle_dir.to_path_buf();
    let title = options.title.map(str::to_string);
    let mining_mode_opt = options.mining_mode.map(str::to_string);
    let provider_opt = options.provider.map(str::to_string);
    let render = options.render;
    let architecture = options.architecture;
    let author = options.author;
    let procedure_author = options.procedure_author;

    // Port of `start_code_bundle_build.<locals>.work` — the thread body.
    // Passes `author`/`procedure_author` through unchanged, exactly as the
    // Python closure captures and forwards the outer function's own
    // parameters of the same name.
    let work = move |recorder: &mut dyn Recorder| -> Result<Value> {
        build_code_bundle_from_repo(
            &repo_dir,
            BuildCodeBundleOptions {
                bundle_dir: Some(bundle_dir_owned.clone()),
                title: title.as_deref(),
                mining_mode: mining_mode_opt.as_deref(),
                provider: provider_opt.as_deref(),
                render,
                author: author.clone(),
                procedure_author: procedure_author.clone(),
                architecture,
                recorder: Some(recorder),
            },
        )
    };

    // Port of `return start_background_run(bundle_dir, state, work)`
    // (`kl4a/codekb/pipeline.py:182`, confirmed via tools-code MCP): Python
    // returns the `threading.Thread` itself, not a summary `Value` --
    // `run_state.start_background_run`'s grounded Rust signature returns
    // `Result<JoinHandle<()>>`, so this function's return type mirrors that
    // instead of the `Result<Value>` an earlier draft of this file assumed.
    start_background_run(bundle_dir.to_path_buf(), state, work)
}
