//! `WorkbenchContext` and the generation-counter state model.
//! docs/port/port-mapping-e-web-cli-contract.md §3 (mutable "current directory" state
//! model); PORT_PLAN.md §5.13 P-W2, P-W27, P-W28, P-W29, P-W30.

use crate::bundles::{self, BundleKey};
use crate::paths::resolve_best_effort;
use sopkb_core::error::{Result, SopkbError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

pub const DEFAULT_WORKBENCH_DIRNAME: &str = "SOP Knowledge Workbench";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchMode {
    SingleBundle,
    WorkbenchRoot,
    /// P-W30 FIX: root does not exist / is not readable / has a malformed
    /// `manifest.yaml`. Python has no equivalent -- a bad launch path there either
    /// crashes `build_server` or (via `desktop.py`'s own fallback) silently substitutes
    /// the default workbench dir with zero indication anything was wrong. This mode
    /// exists so a caller can show the user *why* nothing loaded.
    Degraded,
}

/// docs/port/port-mapping-e-web-cli-contract.md §3.2 + PORT_PLAN.md §4.1's full field
/// list: `{ root, mode, bundles_root, selected_bundle?, generation, settings_path, error? }`.
#[derive(Debug, Clone)]
pub struct WorkbenchContext {
    pub root: PathBuf,
    pub mode: WorkbenchMode,
    pub bundles_root: PathBuf,
    pub selected_bundle: Option<BundleKey>,
    pub generation: u64,
    pub settings_path: PathBuf,
    pub error: Option<String>,
}

/// `resolve_launch_target`'s return shape (docs/port/port-mapping-e-web-cli-contract.md
/// §4.1): `(path: string?) -> { root, mode, bundles_root, created_default: bool }`.
#[derive(Debug, Clone)]
pub struct ResolvedLaunchTarget {
    pub root: PathBuf,
    pub mode: WorkbenchMode,
    pub bundles_root: PathBuf,
    pub created_default: bool,
}

/// Same two-line pattern as `sopkb-config/src/settings.rs`'s private `home_dir()`
/// (checks `$HOME` then `%USERPROFILE%`) -- replicated locally rather than exposing
/// that crate's private helper, per PORT_PLAN.md's own "your call, not worth a big
/// design discussion" framing for this exact question.
fn home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.is_empty() {
            return PathBuf::from(profile);
        }
    }
    PathBuf::from(".")
}

/// `desktop.py:62-65` `default_workbench_dir()`: `~/SOP Knowledge Workbench`, created
/// with `mkdir -p` if absent (idempotent -- safe to call on every launch).
pub fn default_workbench_dir() -> PathBuf {
    let path = home_dir().join(DEFAULT_WORKBENCH_DIRNAME);
    let _ = std::fs::create_dir_all(&path);
    path
}

/// Only expands a bare leading `~` or `~/`/`~\` -- Python's `Path.expanduser()` also
/// supports `~otheruser`, which is not meaningfully actionable cross-platform here and
/// is not exercised by any caller in this codebase (both Python call sites this ports
/// -- `desktop.py:92`, `desktop_sidecar.py:57` -- only ever pass a plain CLI argument).
fn expand_user(path: &Path) -> PathBuf {
    if let Some(s) = path.to_str() {
        if s == "~" {
            return home_dir();
        }
        if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
            return home_dir().join(rest);
        }
    }
    path.to_path_buf()
}

fn expand_and_resolve(path: &Path) -> PathBuf {
    resolve_best_effort(&expand_user(path))
}

/// `desktop.py:91-96` / `desktop_sidecar.py:57-60`, both identical: candidate = `path`
/// if given (expanded + resolved), else `None`. If the candidate doesn't exist or
/// isn't a directory, fall back to `default_workbench_dir()` (mkdir -p'd,
/// `created_default: true`). Mode is then probed via `is_bundle_dir(root)`.
///
/// This function NEVER fails and NEVER produces `WorkbenchMode::Degraded` -- it is a
/// faithful port of Python's silent-fallback launch resolution, used only for the very
/// first "no context yet" launch (an empty/missing argument legitimately means "open
/// my home-dir workbench", not an error). Contrast with [`build_context`], which is
/// used for explicit switches and DOES surface a degraded context instead of
/// substituting silently (P-W30) -- picking a bad folder from a dialog deserves visible
/// feedback in a way that "you launched with no argument" does not.
pub fn resolve_launch_target(path: Option<&Path>) -> ResolvedLaunchTarget {
    let candidate = path.map(expand_and_resolve);
    let (root, created_default) = match candidate {
        Some(c) if c.is_dir() => (c, false),
        _ => (default_workbench_dir(), true),
    };
    let mode = if bundles::is_bundle_dir(&root) { WorkbenchMode::SingleBundle } else { WorkbenchMode::WorkbenchRoot };
    let bundles_root = bundles::knowledge_bundles_root(&root);
    ResolvedLaunchTarget { root, mode, bundles_root, created_default }
}

/// The "library's context-resolution entry point" (PORT_PLAN.md P-W30): builds a full
/// `WorkbenchContext` for `root` with NO fallback substitution. Returns a
/// `mode: Degraded` context (with `error` populated) instead of panicking or returning
/// `Err` when `root` doesn't exist, isn't a directory, or `is_bundle_dir(root)` is true
/// but its `manifest.yaml` fails to parse. `generation` is caller-supplied: 0 for a
/// fresh context, `previous_generation + 1` for a switch.
///
/// In `SingleBundle` mode, `selected_bundle` is set to the root's own directory name --
/// "selected_bundle = the root itself", per the state-machine diagram in
/// docs/port/port-mapping-e-web-cli-contract.md §3.2.
pub fn build_context(root: &Path, generation: u64) -> WorkbenchContext {
    let settings_path = sopkb_config::settings_path();
    let root_buf = root.to_path_buf();

    if !root.exists() {
        return WorkbenchContext {
            root: root_buf.clone(),
            mode: WorkbenchMode::Degraded,
            bundles_root: root_buf,
            selected_bundle: None,
            generation,
            settings_path,
            error: Some(format!("root does not exist: {}", root.display())),
        };
    }
    if !root.is_dir() {
        return WorkbenchContext {
            root: root_buf.clone(),
            mode: WorkbenchMode::Degraded,
            bundles_root: root_buf,
            selected_bundle: None,
            generation,
            settings_path,
            error: Some(format!("root is not a directory: {}", root.display())),
        };
    }

    if bundles::is_bundle_dir(root) {
        match sopkb_core::store::load_manifest(root) {
            Ok(_) => {
                let bundles_root = bundles::knowledge_bundles_root(root);
                let selected_bundle = root.file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
                WorkbenchContext {
                    root: root_buf,
                    mode: WorkbenchMode::SingleBundle,
                    bundles_root,
                    selected_bundle,
                    generation,
                    settings_path,
                    error: None,
                }
            }
            Err(e) => WorkbenchContext {
                root: root_buf.clone(),
                mode: WorkbenchMode::Degraded,
                bundles_root: root_buf,
                selected_bundle: None,
                generation,
                settings_path,
                error: Some(format!("malformed manifest.yaml: {e}")),
            },
        }
    } else {
        let bundles_root = bundles::knowledge_bundles_root(root);
        WorkbenchContext { root: root_buf, mode: WorkbenchMode::WorkbenchRoot, bundles_root, selected_bundle: None, generation, settings_path, error: None }
    }
}

struct Inner {
    context: WorkbenchContext,
    in_flight: u64,
}

/// App-managed shared state: one `WorkbenchContext` plus an in-flight-mutation counter,
/// guarded by a single lock (docs/port/port-mapping-e-web-cli-contract.md §3.2:
/// "Guarded by a single lock, held as app-managed state").
///
/// **Generation-counter design (PORT_PLAN.md P-W2), chosen approach: (a) refuse the
/// switch.** `set_workbench_root` refuses (returns `SopkbError::Conflict`, context left
/// byte-for-byte unchanged) while `in_flight > 0` for the CURRENT root, rather than
/// letting a long-running ingest keep writing after the user has navigated away and
/// then trying to detect and discard the stale write after the fact. Chosen over
/// approach (b) (let operations run to completion and refuse to publish under a stale
/// generation) because refusing up front is strictly simpler to reason about and test,
/// and matches the state diagram's own `GuardInFlight -> Refused` shape more directly
/// than a "carry the generation, discard late" design would. `generation` is still
/// bumped on every COMMITTED switch and carried on the context for any future caller
/// that wants to double-check staleness independently.
pub struct WorkbenchHandle {
    inner: Mutex<Inner>,
    /// Cooperative ingest-cancellation flag, deliberately OUTSIDE `inner`'s lock: it's
    /// checked from worker threads inside `mine_with_author`/`build_heading_index`'s
    /// `parallel_map` fan-out (see `sopkb_core::parallel::parallel_map`) between every
    /// unit of work, and a plain `AtomicBool::load` there is far cheaper than taking a
    /// `Mutex` from N threads on every iteration. "Cooperative" is the operative word:
    /// nothing here can abort an in-flight HTTP request (blocking `ureq`, no cheap
    /// external interrupt) or a unit of work already dispatched to a worker -- setting
    /// this flag only stops the pipeline from STARTING anything new, at the next point
    /// (a new section's author-call cycle, a new chunk's index call, or the next
    /// pipeline step) it's checked. See `request_cancel`/`is_cancel_requested`.
    cancel_requested: AtomicBool,
}

/// RAII marker for "a bundle-mutating operation is running under the context's current
/// root". Held for the duration of the operation; dropping it (including via panic
/// unwind) always decrements the counter.
pub struct MutationGuard<'a> {
    handle: &'a WorkbenchHandle,
}

impl Drop for MutationGuard<'_> {
    fn drop(&mut self) {
        let mut inner = self.handle.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.in_flight = inner.in_flight.saturating_sub(1);
    }
}

impl WorkbenchHandle {
    pub fn new(initial: WorkbenchContext) -> Self {
        Self { inner: Mutex::new(Inner { context: initial, in_flight: 0 }), cancel_requested: AtomicBool::new(false) }
    }

    /// Convenience constructor mirroring the `Uninitialized -> Resolving ->
    /// {SingleBundle|RootNoSelection|Degraded}` startup transition: resolves the
    /// launch target (silent-fallback semantics, see [`resolve_launch_target`]) and
    /// builds the initial context at generation 0.
    pub fn launch(path: Option<&Path>) -> Self {
        let target = resolve_launch_target(path);
        Self::new(build_context(&target.root, 0))
    }

    pub fn context(&self) -> WorkbenchContext {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).context.clone()
    }

    #[cfg(test)]
    fn in_flight_count(&self) -> u64 {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).in_flight
    }

    /// `set_workbench_root` (docs/port/port-mapping-e-web-cli-contract.md §4.1).
    /// Resolves the new root OUTSIDE the lock (disk I/O has no business holding a
    /// mutex), then takes the lock only to check for an in-flight mutation and swap
    /// the context. P-W27 PRESERVE: "bring the replacement up and confirm it before
    /// tearing the old one down" -- `build_context` never partially applies; either the
    /// whole new context (healthy or Degraded) replaces the old one, or nothing changes
    /// at all.
    pub fn set_workbench_root(&self, path: &Path) -> Result<WorkbenchContext> {
        let resolved_root = expand_and_resolve(path);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.in_flight > 0 {
            return Err(SopkbError::Conflict(format!(
                "cannot switch workbench root while a mutation is in progress for {}",
                inner.context.root.display()
            )));
        }
        let next_generation = inner.context.generation + 1;
        let next = build_context(&resolved_root, next_generation);
        inner.context = next.clone();
        Ok(next)
    }

    /// `select_bundle` (docs/port/port-mapping-e-web-cli-contract.md §4.1). Validates
    /// against a FRESH `list_bundle_dirs` call, per E §3.2: "`select_bundle` must
    /// validate against a fresh `list_bundle_dirs`... rather than panicking on `None`".
    pub fn select_bundle(&self, key: &str) -> Result<WorkbenchContext> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.context.mode == WorkbenchMode::Degraded {
            return Err(SopkbError::Conflict("workbench root is in a degraded state; pick a different folder first".to_string()));
        }
        let root = inner.context.root.clone();
        let dirs = bundles::list_bundle_dirs(&root);
        let matched = dirs.iter().any(|d| d.file_name().and_then(|n| n.to_str()) == Some(key));
        if !matched {
            return Err(SopkbError::NotFound(format!("unknown bundle: {key}")));
        }
        inner.context.selected_bundle = Some(key.to_string());
        Ok(inner.context.clone())
    }

    pub fn deselect_bundle(&self) -> WorkbenchContext {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.context.selected_bundle = None;
        inner.context.clone()
    }

    /// Begins a bundle-mutating operation under the context's CURRENT root. Hold the
    /// returned guard for the duration of the operation.
    pub fn begin_mutation(&self) -> MutationGuard<'_> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.in_flight += 1;
        MutationGuard { handle: self }
    }

    /// Clears any stale cancellation request from a PREVIOUS run before a new one
    /// starts -- without this, cancelling one ingest run would leave every future run
    /// dead-on-arrival (every `is_cancel_requested()` check would still see `true`).
    /// Call once, right at the start of a run, before any step/section/chunk work.
    pub fn clear_cancel_request(&self) {
        self.cancel_requested.store(false, Ordering::SeqCst);
    }

    /// Sets the cooperative cancellation flag -- see the field's own doc comment for
    /// exactly what this does and does not guarantee. Idempotent; safe to call even if
    /// nothing is running (a no-op the next run clears via `clear_cancel_request`).
    pub fn request_cancel(&self) {
        self.cancel_requested.store(true, Ordering::SeqCst);
    }

    pub fn is_cancel_requested(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    /// [`crate::ingest::run_ingest_pipeline`] wrapped in an in-flight guard, so a
    /// concurrent `set_workbench_root` is refused for its duration. This is the shape a
    /// future thin Tauri command would call: resolve `bundle_dir` from
    /// `context().selected_bundle`, then call this.
    pub fn run_ingest_pipeline_guarded(&self, bundle_dir: &Path, request: &crate::ingest::IngestRequest) -> Result<crate::ingest::IngestResult> {
        let _guard = self.begin_mutation();
        crate::ingest::run_ingest_pipeline(bundle_dir, request)
    }

    /// [`crate::upload::stage_uploaded_files`] wrapped in the same in-flight guard.
    pub fn stage_uploaded_files_guarded(&self, bundle_dir: &Path, uploads: &[crate::upload::UploadSource], replace: bool) -> Result<crate::upload::StagedUpload> {
        let _guard = self.begin_mutation();
        crate::upload::stage_uploaded_files(bundle_dir, uploads, replace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn resolve_launch_target_falls_back_to_default_for_missing_path() {
        let bogus = std::env::temp_dir().join("sopkb-this-path-should-not-exist-xyz-123");
        let target = resolve_launch_target(Some(&bogus));
        assert!(target.created_default);
        assert!(target.root.is_dir());
    }

    #[test]
    fn resolve_launch_target_uses_existing_dir_and_probes_mode() {
        let dir = tempfile::tempdir().unwrap();
        let target = resolve_launch_target(Some(dir.path()));
        assert!(!target.created_default);
        assert_eq!(target.mode, WorkbenchMode::WorkbenchRoot);

        sopkb_core::store::create_bundle(dir.path(), Some("Root Bundle")).unwrap();
        let target = resolve_launch_target(Some(dir.path()));
        assert_eq!(target.mode, WorkbenchMode::SingleBundle);
    }

    #[test]
    fn build_context_degraded_for_nonexistent_root_p_w30() {
        let bogus = std::env::temp_dir().join("sopkb-definitely-not-here-abc-999");
        let ctx = build_context(&bogus, 0);
        assert_eq!(ctx.mode, WorkbenchMode::Degraded);
        assert!(ctx.error.is_some());
    }

    #[test]
    fn build_context_degraded_for_malformed_manifest_p_w30() {
        let dir = tempfile::tempdir().unwrap();
        // sopkb-fmt's hand-rolled YAML parser never actually returns a parse error
        // (deliberately narrow but also deliberately lenient -- any text loads as SOME
        // mapping), so `load_manifest` can't be made to fail via YAML content alone.
        // Making `manifest.yaml` a DIRECTORY instead of a file keeps `is_bundle_dir`
        // true (`Path::exists()` is true for directories too) while making
        // `fs::read_to_string(&manifest_path)` fail with a real I/O error -- a
        // portable, reliable way to reach `load_manifest`'s error path without relying
        // on OS file permissions.
        std::fs::create_dir_all(dir.path().join("manifest.yaml")).unwrap();
        let ctx = build_context(dir.path(), 0);
        assert_eq!(ctx.mode, WorkbenchMode::Degraded);
        assert!(ctx.error.unwrap().contains("malformed manifest.yaml"));
    }

    #[test]
    fn build_context_single_bundle_self_selects_p_w5_mermaid() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_dir = dir.path().join("my-bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("My Bundle")).unwrap();
        let ctx = build_context(&bundle_dir, 0);
        assert_eq!(ctx.mode, WorkbenchMode::SingleBundle);
        assert_eq!(ctx.selected_bundle.as_deref(), Some("my-bundle"));
    }

    #[test]
    fn select_bundle_validates_against_fresh_listing() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("knowledge-bundles");
        sopkb_core::store::create_bundle(&root.join("a"), Some("A")).unwrap();
        let handle = WorkbenchHandle::new(build_context(dir.path(), 0));

        assert!(handle.select_bundle("missing").is_err());
        let ctx = handle.select_bundle("a").unwrap();
        assert_eq!(ctx.selected_bundle.as_deref(), Some("a"));

        // Selection disappears from disk -> a later select of the same key fails.
        std::fs::remove_dir_all(root.join("a")).unwrap();
        assert!(handle.select_bundle("a").is_err());
    }

    #[test]
    fn set_workbench_root_bumps_generation_and_clears_selection() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let handle = WorkbenchHandle::new(build_context(dir_a.path(), 0));
        assert_eq!(handle.context().generation, 0);

        let next = handle.set_workbench_root(dir_b.path()).unwrap();
        assert_eq!(next.generation, 1);
        // Compare against the SAME best-effort resolution `set_workbench_root` itself
        // applies -- e.g. under a temp dir whose path contains an 8.3 short name
        // component, resolving legitimately changes the string form (long name),
        // exactly as Python's own `Path.resolve()` would.
        assert_eq!(next.root, resolve_best_effort(dir_b.path()));
        assert!(next.selected_bundle.is_none());
    }

    #[test]
    fn set_workbench_root_refused_while_mutation_in_flight_and_leaves_context_unchanged() {
        // Real concurrency test, same spirit as sopkb-review::lock's
        // same_bundle_serializes_concurrent_writers: hold the mutation guard on one
        // thread (with a short sleep) while another thread attempts a switch.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let handle = Arc::new(WorkbenchHandle::new(build_context(dir_a.path(), 0)));
        let started = Arc::new(std::sync::Barrier::new(2));

        let worker = {
            let handle = handle.clone();
            let started = started.clone();
            thread::spawn(move || {
                let _guard = handle.begin_mutation();
                started.wait();
                thread::sleep(std::time::Duration::from_millis(60));
            })
        };

        started.wait();
        // The guard is held on the worker thread right now.
        let refused = handle.set_workbench_root(dir_b.path());
        assert!(matches!(refused, Err(SopkbError::Conflict(_))), "switch must be refused while a mutation is in flight");
        let ctx_after_refusal = handle.context();
        assert_eq!(ctx_after_refusal.root, dir_a.path());
        assert_eq!(ctx_after_refusal.generation, 0, "a refused switch must not bump the generation");

        worker.join().unwrap();
        assert_eq!(handle.in_flight_count(), 0);

        // Now that the mutation has ended, the identical switch succeeds.
        let committed = handle.set_workbench_root(dir_b.path()).unwrap();
        assert_eq!(committed.root, resolve_best_effort(dir_b.path()));
        assert_eq!(committed.generation, 1);
    }

    #[test]
    fn run_ingest_pipeline_guarded_releases_the_guard_on_completion() {
        let dir = tempfile::tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        sopkb_core::store::create_bundle(&bundle_dir, Some("Bundle")).unwrap();
        let handle = WorkbenchHandle::new(build_context(dir.path(), 0));

        // An empty source dir with everything off is a trivial, fast no-op pipeline
        // run; what matters here is only that the guard is released afterward.
        let source_dir = dir.path().join("empty-src");
        std::fs::create_dir_all(&source_dir).unwrap();
        let request = crate::ingest::IngestRequest {
            source: crate::ingest::IngestSource::Folder(source_dir),
            scan: false,
            normalize: false,
            mine: false,
            validate: false,
            export: false,
            mine_provider: "fixture".to_string(),
            profile_id: None,
            uploaded_file_count: None,
        };
        handle.run_ingest_pipeline_guarded(&bundle_dir, &request).unwrap();
        assert_eq!(handle.in_flight_count(), 0);

        // And a switch right after succeeds -- proving the guard was actually released,
        // not merely that no error was returned.
        let other = tempfile::tempdir().unwrap();
        assert!(handle.set_workbench_root(other.path()).is_ok());
    }
}
