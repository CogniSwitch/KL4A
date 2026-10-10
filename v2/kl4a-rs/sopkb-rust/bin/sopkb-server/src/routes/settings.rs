//! Mirrors `desktop-tauri/src-tauri/src/commands/settings.rs` (§4.8) -- read-only
//! subset only for this pass (profile CRUD, reviewer name, worker count are NOT yet
//! wired to HTTP, disclosed as a gap). Global, not bundle-scoped, same as the Tauri
//! original.

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::dto::profile_json;
use crate::state::AppState;

/// Mirrors `desktop-tauri/src-tauri/src/commands/settings.rs`'s
/// `ENV_OVERRIDE_CANDIDATES`/`compute_env_overrides` field-for-field (that table is
/// private to `sopkb-config`, so both callers hand-keep their own copy -- see that
/// file's own doc comment on the gap). `SettingsScreen.tsx` reads `settings.data
/// .env_overrides.map(...)` unconditionally once `settings.data` is non-null; leaving
/// this field out entirely (as this route previously did) crashes the whole screen
/// the instant a bundle -- or no bundle at all -- loads Settings.
const ENV_OVERRIDE_CANDIDATES: &[(&str, &[&str])] = &[
    ("base_url", &["AZURE_OPENAI_BASE_URL", "AZURE_OPENAI_ENDPOINT", "OPENAI_BASE_URL"]),
    ("api_key", &["AZURE_OPENAI_API_KEY", "OPENAI_API_KEY"]),
    ("model", &["AZURE_OPENAI_DEPLOYMENT", "OPENAI_MODEL"]),
    ("auth_style", &["SOPKB_LLM_AUTH_STYLE"]),
    ("max_output_tokens", &["AZURE_OPENAI_MAX_OUTPUT_TOKENS"]),
    ("timeout_seconds", &["AZURE_OPENAI_TIMEOUT_SECONDS"]),
    ("reasoning_effort", &["AZURE_OPENAI_REASONING_EFFORT"]),
    // id, name, mining_prompt, chat_prompt have no env override at all.
];

fn compute_env_overrides() -> Vec<Value> {
    ENV_OVERRIDE_CANDIDATES
        .iter()
        .map(|(field, vars)| {
            let active = vars.iter().find(|v| std::env::var(v).map(|s| !s.trim().is_empty()).unwrap_or(false));
            match active {
                Some(v) => json!({ "field": field, "env_var": v, "active_value_present": true }),
                None => json!({ "field": field, "env_var": vars[0], "active_value_present": false }),
            }
        })
        .collect()
}

pub async fn get_settings(State(_state): State<AppState>) -> Json<Value> {
    let config = sopkb_config::load_config();
    let profiles: Vec<Value> = config.profiles.iter().map(|p| profile_json(p, &config.default_profile_id)).collect();
    Json(json!({
        "profiles": profiles,
        "default_profile_id": config.default_profile_id,
        "reviewer_name": config.reviewer_name,
        "settings_path": sopkb_config::settings_path().display().to_string(),
        "max_parallel_workers": config.max_parallel_workers,
        "env_overrides": compute_env_overrides(),
    }))
}

pub async fn get_default_prompts() -> Json<Value> {
    Json(json!({
        "mining_prompt": sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT,
        "chat_prompt": sopkb_agent::SYSTEM_PROMPT,
        "heading_index_prompt": sopkb_workbench::HEADING_INDEX_SYSTEM_PROMPT,
        "heading_relevel_prompt": sopkb_workbench::HEADING_RELEVEL_SYSTEM_PROMPT,
    }))
}
