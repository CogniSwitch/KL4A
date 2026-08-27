//! Lightweight startup diagnostics for chasing a reported one-off launch hang (hung
//! once, opened fine on retry -- no reproduction, no console attached since release
//! builds are GUI-subsystem). Appends timestamped stage markers to a plain text file
//! so that IF it hangs again, there's a trail showing exactly which stage it got stuck
//! in, without needing devtools or a debugger attached.
//!
//! Three checkpoints bracket the entire possible hang window:
//!   1. Everything up to "calling tauri Builder.run()" -- if the log stops before
//!      this, the hang is in Rust-side startup (e.g. `AppState::launch`'s filesystem
//!      probing of the default workbench directory).
//!   2. "setup() hook invoked" -- fires once Tauri's event loop has started and the
//!      window/webview are being created. If the log reaches here but no further, the
//!      hang is in WebView2's own initialization (plausible for a genuine first-run
//!      one-time setup cost on a machine that's never hosted a WebView2 app before).
//!   3. "frontend_ready received" -- a fire-and-forget ping the React app sends once
//!      after mounting (see `commands::diagnostics::frontend_ready` and
//!      `frontend/src/main.tsx`). If setup() logged but this never arrives, the
//!      webview created successfully but the embedded JS never finished bootstrapping.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();
static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Prefer a file next to the executable (visible, obvious, works for the portable
/// build with no install step); fall back to the OS temp dir if that location isn't
/// writable (e.g. a future installed/Program-Files deployment).
fn compute_log_path() -> Option<PathBuf> {
    let beside_exe = std::env::current_exe().ok()?.parent()?.join("sopkb-startup.log");
    if OpenOptions::new().create(true).append(true).open(&beside_exe).is_ok() {
        return Some(beside_exe);
    }
    let fallback = std::env::temp_dir().join("sopkb-startup.log");
    OpenOptions::new().create(true).append(true).open(&fallback).ok().map(|_| fallback)
}

/// Appends one `[+12.345s] stage` line. Best-effort: a logging failure must never be
/// allowed to affect startup itself, so every I/O error here is silently swallowed.
pub fn log(stage: &str) {
    let start = START.get_or_init(Instant::now);
    let elapsed = start.elapsed().as_secs_f64();
    let path = match LOG_PATH.get_or_init(compute_log_path) {
        Some(p) => p,
        None => return,
    };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[+{elapsed:>8.3}s] {stage}");
        let _ = f.flush();
    }
}
