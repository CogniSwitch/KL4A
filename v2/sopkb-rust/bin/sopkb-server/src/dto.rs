//! Manual JSON shaping for `sopkb-workbench`/`sopkb-config` types that don't derive
//! `Serialize` (they're internal to those crates; `desktop-tauri` has its own
//! `dto.rs` doing the same job for its Tauri wire contract -- this is this crate's
//! independent equivalent, not a shared dependency, for the same reason noted in
//! `error.rs`/`events.rs`).

use serde_json::{json, Value};
use sopkb_config::ModelProfile;
use sopkb_workbench::{BundleCard, BundleSummary, WorkbenchContext, WorkbenchMode};

pub fn bundle_summary_json(s: &BundleSummary) -> Value {
    json!({
        "key": s.key,
        "id": s.id,
        "title": s.title,
        "profile": s.profile,
        "status": s.status,
        "source_count": s.source_count,
        "knowledge_item_count": s.knowledge_item_count,
        "created_at": s.created_at,
    })
}

pub fn bundle_card_json(c: &BundleCard) -> Value {
    json!({
        "key": c.key,
        "bundle_path": c.bundle_path.display().to_string(),
        "summary": c.summary.as_ref().map(bundle_summary_json),
        "export_path": c.export_path.as_ref().map(|p| p.display().to_string()),
        "load_error": c.load_error,
    })
}

fn mode_str(mode: WorkbenchMode) -> &'static str {
    match mode {
        WorkbenchMode::SingleBundle => "single_bundle",
        WorkbenchMode::WorkbenchRoot => "workbench_root",
        WorkbenchMode::Degraded => "degraded",
    }
}

pub fn workbench_context_json(c: &WorkbenchContext) -> Value {
    json!({
        "root": c.root.display().to_string(),
        "mode": mode_str(c.mode),
        "bundles_root": c.bundles_root.display().to_string(),
        "selected_bundle": c.selected_bundle,
        "generation": c.generation,
        "settings_path": c.settings_path.display().to_string(),
        "error": c.error,
    })
}

/// Never the raw `api_key` -- same redaction convention as `desktop-tauri`'s
/// `ProfileView::from_profile` (only `has_api_key: bool` crosses this boundary).
pub fn profile_json(p: &ModelProfile, default_id: &str) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "base_url": p.base_url,
        "auth_style": p.auth_style,
        "model": p.model,
        "max_output_tokens": p.max_output_tokens,
        "timeout_seconds": p.timeout_seconds,
        "reasoning_effort": p.reasoning_effort,
        "mining_prompt": p.mining_prompt,
        "chat_prompt": p.chat_prompt,
        "has_api_key": !p.api_key.trim().is_empty(),
        "is_default": !default_id.is_empty() && p.id == default_id,
    })
}
