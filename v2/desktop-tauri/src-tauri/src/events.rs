//! PORT_PLAN.md §4.11 event payloads and emit helpers, shared across command
//! modules so every mutation emits the exact same shapes. Emission is always
//! best-effort (`let _ = ...`): a dropped event means a screen goes stale until its
//! next manual refresh, which is far preferable to a mutation that already
//! succeeded on disk failing the whole command because no webview happened to be
//! listening.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
pub struct BundleStateChanged<'a> {
    pub key: String,
    pub scope: &'a [&'a str],
}

/// Every review mutation (§4.5) uses this exact scope set.
pub const REVIEW_SCOPE: &[&str] = &["items", "reviews", "okf"];

pub fn emit_bundle_state_changed(app: &AppHandle, key: &str, scope: &[&str]) {
    let _ = app.emit("bundle://state-changed", BundleStateChanged { key: key.to_string(), scope });
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleIndexChanged {
    pub bundles_root: String,
}

pub fn emit_bundle_index_changed(app: &AppHandle, bundles_root: &std::path::Path) {
    let _ = app.emit("bundle://index-changed", BundleIndexChanged { bundles_root: bundles_root.display().to_string() });
}

#[derive(Debug, Clone, Serialize)]
pub struct IngestProgress<'a> {
    pub step: &'a str,
    pub status: &'a str,
    pub detail: String,
}

pub fn emit_ingest_progress(app: &AppHandle, step: &str, status: &str, detail: impl Into<String>) {
    let _ = app.emit("ingest://progress", IngestProgress { step, status, detail: detail.into() });
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentProgress<'a> {
    pub phase: &'a str,
    pub detail: String,
}

pub fn emit_agent_progress(app: &AppHandle, phase: &str, detail: impl Into<String>) {
    let _ = app.emit("agent://progress", AgentProgress { phase, detail: detail.into() });
}

pub fn emit_settings_changed(app: &AppHandle) {
    let _ = app.emit("settings://changed", serde_json::json!({}));
}
