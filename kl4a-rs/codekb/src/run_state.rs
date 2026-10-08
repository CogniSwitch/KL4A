//! Port of `kl4a/codekb/run_state.py`.
//!
//! Tracks a code-pipeline build run's stage-by-stage progress on disk so a
//! polling web page can show live status, and guards against two concurrent
//! runs stepping on each other for the same bundle directory.
//!
//! Grounded (via tools-code MCP) tested invariants this port preserves
//! exactly:
//! - every stage is recorded in order (`begin_stage` / `end_stage`)
//! - a disabled ("skipped") stage is marked `skipped`, not left `pending`
//! - `run_progress` counts exclude skipped stages from both `total` and
//!   `completed`
//! - a second run against the same bundle directory is rejected while one is
//!   already active (`start_background_run` raises rather than interleaving)
//! - a failed run records which stage was running and the error, rather than
//!   leaving the run state stuck at "running" forever

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use serde_json::{Map, Value};

use crate::state::{write_code_state, STATE_DIR};

// WIRING: `utc_now` lives on the Python side in `kl4a.kl4a.bundle_store` (a
// shared package above codekb/apikb), not inside `kl4a/codekb`. This batch
// was scoped to render.py/run_state.py/server.py/state.py only, so it was not
// ported here — same gap `model.rs`'s `load_manifest` WIRING note already
// flags. Until a `crate::bundle_store` (or `kl4a-core`) module lands with a
// real `utc_now() -> String`, this reference will not compile; the
// coordinator should wire the real path in during lib.rs assembly.
use crate::bundle_store::utc_now;

/// Mirrors `kl4a.codekb.run_state.RUN_STATE_FILENAME`.
pub const RUN_STATE_FILENAME: &str = "code_pipeline_run.json";

/// Mirrors `kl4a.codekb.run_state.STAGES`: `(key, label)` pairs, in the exact
/// order a run executes them.
pub const STAGES: &[(&str, &str)] = &[
    ("create", "Create bundle"),
    ("scan", "Scan repository"),
    ("parse", "Parse sources"),
    ("relations", "Resolve relations"),
    ("architecture", "Detect architecture"),
    ("mine", "Mine knowledge"),
    ("render", "Render human layer"),
    ("validate", "Validate bundle"),
];

/// Mirrors `kl4a.codekb.run_state.STAGE_LABELS = dict(STAGES)`.
pub fn stage_labels() -> HashMap<&'static str, &'static str> {
    STAGES.iter().copied().collect()
}

/// Mirrors `kl4a.codekb.run_state.ACTIVE_STATUSES`.
pub fn active_statuses() -> [&'static str; 2] {
    ["queued", "running"]
}

fn is_active_status(status: &str) -> bool {
    active_statuses().contains(&status)
}

/// Shared stage-tracking shape a recorder implements. `kl4a.codekb.pipeline`
/// separately defines `_NullRecorder`, a no-op that implements the same
/// `begin_stage`/`end_stage` shape as `RunRecorder` here (per the Python
/// source, both are passed interchangeably as "the recorder" to pipeline
/// work). If that batch also introduces a Rust `Recorder` trait, the
/// coordinator should reconcile to one definition — this one, or theirs —
/// rather than keep two incompatible traits with the same shape.
pub trait Recorder {
    fn begin_stage(&mut self, key: &str);
    fn end_stage(&mut self, key: &str, detail: Option<&str>);
}

/// Mirrors `kl4a.codekb.run_state._LOCKS` / `_LOCKS_GUARD`: a process-wide,
/// per-bundle-directory mutual-exclusion flag.
///
/// Python uses a `threading.Lock` per key and `lock.acquire(blocking=False)`
/// / `lock.release()`. A `std::sync::Mutex`'s guard cannot be held across a
/// `thread::spawn` boundary without a self-referential struct, so this uses
/// an `Arc<AtomicBool>` per key instead: `compare_exchange(false, true, ...)`
/// is the non-blocking "acquire" (fails exactly when another holder has it
/// set), and `store(false, ...)` is "release". This reproduces the same
/// observable behavior (reject a second concurrent run for the same bundle
/// directory) with a `Send + 'static` flag that the spawned thread can own.
static LOCKS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Mirrors `kl4a.codekb.run_state._lock_for`.
fn lock_for(bundle_dir: &Path) -> Arc<AtomicBool> {
    let key = bundle_dir
        .canonicalize()
        .unwrap_or_else(|_| bundle_dir.to_path_buf())
        .to_string_lossy()
        .to_string();
    let mut locks = LOCKS.lock().expect("run_state LOCKS mutex poisoned");
    locks
        .entry(key)
        .or_insert_with(|| Arc::new(AtomicBool::new(false)))
        .clone()
}

/// Mirrors `kl4a.codekb.run_state.read_run_state`.
pub fn read_run_state(bundle_dir: &Path) -> Value {
    crate::state::read_json(
        &bundle_dir.join(STATE_DIR).join(RUN_STATE_FILENAME),
        Value::Object(Map::new()),
    )
}

/// Mirrors `kl4a.codekb.run_state.write_run_state`.
pub fn write_run_state(bundle_dir: &Path, state: &Value) -> Result<()> {
    write_code_state(bundle_dir, RUN_STATE_FILENAME, state)
}

/// Mirrors `kl4a.codekb.run_state.is_running`.
pub fn is_running(bundle_dir: &Path) -> bool {
    let state = read_run_state(bundle_dir);
    let status = state.get("status").and_then(Value::as_str).unwrap_or("");
    is_active_status(status)
}

/// Mirrors `kl4a.codekb.run_state.new_run_state`.
///
/// `skipped` names stage keys that are disabled for this run; those stages
/// start (and stay, per `begin_stage`/`end_stage`) in the `"skipped"` status
/// rather than `"pending"`, and `run_progress` excludes them from its counts.
pub fn new_run_state(
    repo_dir: &str,
    mining_mode: &str,
    provider: &str,
    skipped: Option<&[&str]>,
) -> Value {
    let skipped: std::collections::HashSet<&str> = skipped.unwrap_or(&[]).iter().copied().collect();
    let stages: Vec<Value> = STAGES
        .iter()
        .map(|(key, label)| {
            let mut stage = Map::new();
            stage.insert("key".into(), Value::String((*key).into()));
            stage.insert("label".into(), Value::String((*label).into()));
            let status = if skipped.contains(key) { "skipped" } else { "pending" };
            stage.insert("status".into(), Value::String(status.into()));
            stage.insert("started_at".into(), Value::Null);
            stage.insert("finished_at".into(), Value::Null);
            stage.insert("detail".into(), Value::Null);
            Value::Object(stage)
        })
        .collect();

    let mut state = Map::new();
    state.insert("status".into(), Value::String("queued".into()));
    state.insert("repo_dir".into(), Value::String(repo_dir.into()));
    state.insert("mining_mode".into(), Value::String(mining_mode.into()));
    state.insert("provider".into(), Value::String(provider.into()));
    state.insert("started_at".into(), Value::String(utc_now()));
    state.insert("finished_at".into(), Value::Null);
    state.insert("current_stage".into(), Value::Null);
    state.insert("error".into(), Value::Null);
    state.insert("summary".into(), Value::Null);
    state.insert("stages".into(), Value::Array(stages));
    Value::Object(state)
}

/// Mirrors `kl4a.codekb.run_state.run_progress`: "Summarise a run for
/// display."
pub fn run_progress(state: &Value) -> Value {
    let stages = state
        .get("stages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let countable: Vec<&Value> = stages
        .iter()
        .filter(|s| s.get("status").and_then(Value::as_str) != Some("skipped"))
        .collect();
    let done: Vec<&&Value> = countable
        .iter()
        .filter(|s| {
            matches!(
                s.get("status").and_then(Value::as_str),
                Some("done") | Some("failed")
            )
        })
        .collect();
    let current = stages
        .iter()
        .find(|s| s.get("status").and_then(Value::as_str) == Some("running"));
    let status = state
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_string();
    let active = is_active_status(&status);

    let mut out = Map::new();
    out.insert("status".into(), Value::String(status));
    out.insert("total".into(), Value::from(countable.len()));
    out.insert("completed".into(), Value::from(done.len()));
    let percent = if countable.is_empty() {
        0
    } else {
        (100 * done.len() / countable.len()) as i64
    };
    out.insert("percent".into(), Value::from(percent));
    out.insert(
        "current_label".into(),
        current
            .and_then(|s| s.get("label").cloned())
            .unwrap_or(Value::Null),
    );
    out.insert("active".into(), Value::Bool(active));
    Value::Object(out)
}

/// Mirrors `kl4a.codekb.run_state.RunRecorder`.
///
/// "Writes stage transitions to bundle state as the pipeline advances."
pub struct RunRecorder {
    bundle_dir: PathBuf,
    state: Value,
}

impl RunRecorder {
    /// Mirrors `RunRecorder.__init__`: flushes the initial state to disk
    /// immediately (matches the Python constructor, which calls `_flush()`).
    pub fn new(bundle_dir: PathBuf, state: Value) -> Self {
        let recorder = RunRecorder { bundle_dir, state };
        recorder.flush();
        recorder
    }

    /// Mirrors `RunRecorder._stage`.
    fn stage_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.state
            .get_mut("stages")
            .and_then(Value::as_array_mut)
            .and_then(|stages| {
                stages
                    .iter_mut()
                    .find(|s| s.get("key").and_then(Value::as_str) == Some(key))
            })
    }

    /// Mirrors `RunRecorder._flush`. Python lets a write failure propagate;
    /// this logs to stderr instead of panicking a background worker thread,
    /// since a lost status write should not crash the pipeline it is only
    /// reporting on. UNCONFIRMED: Python has no such fallback (it simply
    /// raises), so a caller relying on `_flush` raising on disk failure will
    /// observe different behavior here.
    fn flush(&self) {
        if let Err(err) = write_run_state(&self.bundle_dir, &self.state) {
            eprintln!("run_state: failed to write run state: {err:#}");
        }
    }

    /// Mirrors `RunRecorder.start`.
    pub fn start(&mut self) {
        set_str(&mut self.state, "status", "running");
        self.flush();
    }

    /// Mirrors `RunRecorder.succeed`.
    pub fn succeed(&mut self, summary: Value) {
        set_str(&mut self.state, "status", "succeeded");
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert("summary".into(), summary);
            obj.insert("finished_at".into(), Value::String(utc_now()));
            obj.insert("current_stage".into(), Value::Null);
        }
        self.flush();
    }

    /// Mirrors `RunRecorder.fail`: "Record a failure against the stage that
    /// was running when it happened."
    ///
    /// Python builds `error` as `f"{type(exc).__name__}: {exc}"` and
    /// `traceback` as the full formatted traceback, truncated to its last
    /// 4000 characters. `anyhow::Error` does not carry a reflective type
    /// name the way a Python exception does, so `error` here is
    /// `format!("{err}")` alone — UNCONFIRMED: this drops the
    /// `TypeName: ` prefix Python's message always has. `traceback` uses
    /// `anyhow::Error`'s `{:?}` rendering (the full source chain, plus a
    /// backtrace if one was captured), truncated the same way.
    pub fn fail(&mut self, err: &anyhow::Error) {
        let current = self
            .state
            .get("current_stage")
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(current) = current {
            if let Some(stage) = self.stage_mut(&current) {
                set_str(stage, "status", "failed");
                if let Some(obj) = stage.as_object_mut() {
                    obj.insert("finished_at".into(), Value::String(utc_now()));
                    obj.insert("detail".into(), Value::String(err.to_string()));
                }
            }
        }
        let full_traceback = format!("{err:?}");
        let truncated = tail_chars(&full_traceback, 4000);
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert("status".into(), Value::String("failed".into()));
            obj.insert("error".into(), Value::String(err.to_string()));
            obj.insert("traceback".into(), Value::String(truncated));
            obj.insert("finished_at".into(), Value::String(utc_now()));
        }
        self.flush();
    }
}

impl Recorder for RunRecorder {
    /// Mirrors `RunRecorder.begin_stage`: a stage that is missing, or already
    /// `"skipped"`, is left untouched (this is the invariant a disabled
    /// stage must stay `skipped`, never regress to `running`/`pending`).
    fn begin_stage(&mut self, key: &str) {
        let should_flush = {
            let Some(stage) = self.stage_mut(key) else { return };
            if stage.get("status").and_then(Value::as_str) == Some("skipped") {
                return;
            }
            set_str(stage, "status", "running");
            if let Some(obj) = stage.as_object_mut() {
                obj.insert("started_at".into(), Value::String(utc_now()));
            }
            true
        };
        if should_flush {
            set_str(&mut self.state, "current_stage", key);
            self.flush();
        }
    }

    /// Mirrors `RunRecorder.end_stage`.
    fn end_stage(&mut self, key: &str, detail: Option<&str>) {
        let should_flush = {
            let Some(stage) = self.stage_mut(key) else { return };
            if stage.get("status").and_then(Value::as_str) == Some("skipped") {
                return;
            }
            set_str(stage, "status", "done");
            if let Some(obj) = stage.as_object_mut() {
                obj.insert("finished_at".into(), Value::String(utc_now()));
                obj.insert(
                    "detail".into(),
                    detail.map(Value::from).unwrap_or(Value::Null),
                );
            }
            true
        };
        if should_flush {
            if let Some(obj) = self.state.as_object_mut() {
                obj.insert("current_stage".into(), Value::Null);
            }
            self.flush();
        }
    }
}

fn set_str(value: &mut Value, key: &str, val: &str) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(key.to_string(), Value::String(val.to_string()));
    }
}

/// Python `s[-n:]` on a `str` truncates to the last `n` *characters*, not
/// bytes; mirrored here rather than a byte-slice truncation, which could
/// otherwise split a multi-byte UTF-8 traceback character.
fn tail_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_string();
    }
    s.chars().skip(count - n).collect()
}

/// Mirrors `kl4a.codekb.run_state.start_background_run`.
///
/// "Run `work(recorder)` on a worker thread, recording progress and
/// failures. The lock is held for the duration of the run so a second
/// submission is rejected rather than interleaved."
///
/// Returns the `JoinHandle` so a caller (tests, in particular) can join it —
/// mirrors the Python function returning the `threading.Thread`.
///
/// `work` returning `Err` is this port's equivalent of the Python callable
/// raising: it is passed to `RunRecorder::fail`, matching
/// `except BaseException as exc: recorder.fail(exc)`.
pub fn start_background_run(
    bundle_dir: PathBuf,
    state: Value,
    work: impl FnOnce(&mut dyn Recorder) -> Result<Value> + Send + 'static,
) -> Result<JoinHandle<()>> {
    let lock = lock_for(&bundle_dir);
    if lock.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(anyhow!(
            "a pipeline run is already in progress for this bundle"
        ));
    }

    let mut recorder = RunRecorder::new(bundle_dir, state);

    let handle = thread::Builder::new()
        .name(format!(
            "code-pipeline-{}",
            recorder
                .bundle_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        ))
        .spawn(move || {
            recorder.start();
            // Fix (Medium finding — pipeline worker failure / stuck
            // "running" state). The previous version here called
            // `work(&mut recorder)` directly: if it ever *panics* (not just
            // returns `Err`) — an `unwrap()`, slice index, arithmetic
            // overflow, etc. anywhere in the whole build/scan/parse/mine
            // pipeline — the panic unwinds this worker thread without ever
            // reaching `recorder.succeed`/`recorder.fail` *or*
            // `lock.store(false, ...)` below. That leaves the run's
            // recorded status stuck at `"running"` forever (nothing ever
            // marks it failed) AND leaves this bundle's run lock
            // permanently held, so `a pipeline run is already in progress
            // for this bundle` is returned for every future run attempt
            // until the process restarts — exactly the failure mode this
            // finding describes. Python's equivalent
            // (`except BaseException as exc: recorder.fail(exc)`) catches
            // everything, including a Python-level crash, so it never gets
            // stuck this way. `catch_unwind` is this port's equivalent
            // safety net for a Rust panic specifically (an `Err` return is
            // already handled normally below, panic or not).
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&mut recorder)));
            match outcome {
                Ok(Ok(summary)) => recorder.succeed(summary),
                Ok(Err(err)) => recorder.fail(&err),
                Err(panic_payload) => recorder.fail(&anyhow!("pipeline worker panicked: {}", panic_message(&panic_payload))),
            }
            lock.store(false, Ordering::SeqCst);
        })?;
    Ok(handle)
}

/// Best-effort text extraction from a caught panic's payload (the type
/// `std::panic::catch_unwind` hands back is `Box<dyn Any + Send>`, which
/// carries no guaranteed string — `panic!("...")`/`unwrap()` payloads are
/// almost always `&str` or `String`, covered here; anything else falls back
/// to a generic message rather than failing to report the failure at all).
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}
