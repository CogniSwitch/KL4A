//! §4.8 Settings -- global, never bundle-scoped.

use serde::{Deserialize, Serialize};
use sopkb_config::ModelProfile;
use tauri::AppHandle;

use crate::error::{AppError, CmdResult};
use crate::events::emit_settings_changed;

/// PORT_PLAN.md §4.8 `ProfileView`. **Never carries the raw API key** -- only
/// `has_api_key: bool` (a pinned test upstream asserts the raw key never appears in
/// a rendered page; this is the same non-negotiable at the IPC boundary).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub auth_style: String,
    pub model: String,
    pub max_output_tokens: String,
    pub timeout_seconds: String,
    pub reasoning_effort: String,
    pub mining_prompt: String,
    pub chat_prompt: String,
    pub has_api_key: bool,
    pub is_default: bool,
}

impl ProfileView {
    fn from_profile(p: &ModelProfile, default_id: &str) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            base_url: p.base_url.clone(),
            auth_style: p.auth_style.clone(),
            model: p.model.clone(),
            max_output_tokens: p.max_output_tokens.clone(),
            timeout_seconds: p.timeout_seconds.clone(),
            reasoning_effort: p.reasoning_effort.clone(),
            mining_prompt: p.mining_prompt.clone(),
            chat_prompt: p.chat_prompt.clone(),
            has_api_key: !p.api_key.trim().is_empty(),
            is_default: !default_id.is_empty() && p.id == default_id,
        }
    }
}

/// PORT_PLAN.md §4.8 `ProfileInput`. `api_key` absent or blank means "keep the
/// existing key" -- it must never be interpreted as "clear the key".
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub auth_style: String,
    pub model: String,
    pub max_output_tokens: String,
    pub timeout_seconds: String,
    pub reasoning_effort: String,
    pub mining_prompt: String,
    pub chat_prompt: String,
    pub api_key: Option<String>,
}

/// The non-trivial part of `save_profile`: id assignment (`unique_profile_id` for a
/// fresh profile, the given id for an update) and the "blank/absent api_key means
/// keep existing" rule (§4.8's one non-negotiable besides never returning the key).
/// Pure and unit-tested independent of the actual `sopkb_config::save_profile` disk
/// write.
pub(crate) fn build_profile_to_save(input: ProfileInput, existing: &[ModelProfile]) -> ModelProfile {
    let id = match &input.id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => sopkb_config::unique_profile_id(&input.name, existing),
    };
    let api_key = match input.api_key.as_deref().map(str::trim) {
        Some(key) if !key.is_empty() => key.to_string(),
        _ => existing.iter().find(|p| p.id == id).map(|p| p.api_key.clone()).unwrap_or_default(),
    };
    ModelProfile {
        id,
        name: input.name,
        base_url: input.base_url,
        api_key,
        auth_style: input.auth_style,
        model: input.model,
        max_output_tokens: input.max_output_tokens,
        timeout_seconds: input.timeout_seconds,
        reasoning_effort: input.reasoning_effort,
        mining_prompt: input.mining_prompt,
        chat_prompt: input.chat_prompt,
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_profile(app: AppHandle, input: ProfileInput) -> CmdResult<ProfileView> {
    let existing = sopkb_config::list_profiles();
    let profile = build_profile_to_save(input, &existing);
    sopkb_config::save_profile(&profile).map_err(AppError::from)?;
    emit_settings_changed(&app);
    let default_id = sopkb_config::get_default_profile_id();
    Ok(ProfileView::from_profile(&profile, &default_id))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EnvOverrideStatus {
    pub field: String,
    pub env_var: String,
    pub active_value_present: bool,
}

/// Mirrors `sopkb_config::settings::ENV_OVERRIDES` field-by-field. That table is
/// private to `sopkb-config` (not `pub`), so it cannot be imported directly -- this
/// is a hand-kept duplicate. Flagged in the final integration report as a real gap:
/// `sopkb-config` exporting either the table or a
/// `fn active_env_override(field) -> Option<(&str, &str)>` helper would let this stay
/// in sync automatically instead of by hand. `resolve`'s own doc comment confirms
/// the precedence this mirrors: "first non-blank in the field's ordered list".
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

pub(crate) fn compute_env_overrides() -> Vec<EnvOverrideStatus> {
    ENV_OVERRIDE_CANDIDATES
        .iter()
        .map(|(field, vars)| {
            let active = vars.iter().find(|v| std::env::var(v).map(|s| !s.trim().is_empty()).unwrap_or(false));
            match active {
                Some(v) => EnvOverrideStatus { field: (*field).to_string(), env_var: (*v).to_string(), active_value_present: true },
                None => EnvOverrideStatus { field: (*field).to_string(), env_var: vars[0].to_string(), active_value_present: false },
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub profiles: Vec<ProfileView>,
    pub default_profile_id: String,
    pub reviewer_name: String,
    pub settings_path: String,
    pub env_overrides: Vec<EnvOverrideStatus>,
    pub max_parallel_workers: usize,
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_settings() -> SettingsView {
    let profiles = sopkb_config::list_profiles();
    let default_id = sopkb_config::get_default_profile_id();
    SettingsView {
        profiles: profiles.iter().map(|p| ProfileView::from_profile(p, &default_id)).collect(),
        default_profile_id: default_id,
        reviewer_name: sopkb_config::reviewer_name(),
        settings_path: sopkb_config::settings_path().display().to_string(),
        env_overrides: compute_env_overrides(),
        max_parallel_workers: sopkb_config::max_parallel_workers(),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_profile(app: AppHandle, id: String) -> CmdResult<()> {
    sopkb_config::delete_profile(&id).map_err(AppError::from)?;
    emit_settings_changed(&app);
    Ok(())
}

/// [FIX] per §4.8: the underlying `sopkb_config::set_default_profile` silently
/// no-ops (file not rewritten) on an unknown id; this command checks existence
/// first and returns `NotFound` instead, since a silent no-op through Tauri IPC
/// would look identical to success on the frontend.
#[tauri::command(rename_all = "snake_case")]
pub fn set_default_profile(app: AppHandle, id: String) -> CmdResult<()> {
    let exists = sopkb_config::list_profiles().iter().any(|p| p.id == id);
    if !exists {
        return Err(AppError::not_found(format!("unknown profile: {id}")));
    }
    sopkb_config::set_default_profile(&id).map_err(AppError::from)?;
    emit_settings_changed(&app);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_reviewer_name(app: AppHandle, name: String) -> CmdResult<()> {
    sopkb_config::set_reviewer_name(&name).map_err(AppError::from)?;
    emit_settings_changed(&app);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_max_parallel_workers(app: AppHandle, value: usize) -> CmdResult<()> {
    sopkb_config::set_max_parallel_workers(value).map_err(AppError::from)?;
    emit_settings_changed(&app);
    Ok(())
}

/// The built-in prompts a blank `mining_prompt`/`chat_prompt` falls back to (see
/// `sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT`, `sopkb_agent::prompt::SYSTEM_PROMPT`).
/// Surfaced read-only so the Settings screen can show what a non-blank override actually
/// replaces -- a user writing a custom prompt with no visibility into the default they're
/// discarding is exactly how the fields got silently poisoned with a UI placeholder before
/// (see docs/port/CATCHUP_PLAN.md's "Settings UI silently poisoned every mining/chat
/// system prompt").
#[derive(Debug, Clone, Serialize)]
pub struct DefaultPrompts {
    pub mining_prompt: String,
    pub chat_prompt: String,
    /// Not user-overridable (unlike `mining_prompt`/`chat_prompt` above) -- these two
    /// are surfaced purely for visibility into what the heading-restructuring step
    /// (Normalize, when an LLM provider is selected) actually sends. Read-only in
    /// the UI for the same reason there's no `heading_index_prompt`/
    /// `heading_relevel_prompt` field on `ProfileView`: overriding them safely would
    /// need matching changes to `build_heading_index`'s/`relevel_heading_index_llm`'s
    /// own JSON-shape parsing of the response, not just swapping the prompt text.
    pub heading_index_prompt: String,
    pub heading_relevel_prompt: String,
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_default_prompts() -> DefaultPrompts {
    DefaultPrompts {
        mining_prompt: sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT.to_string(),
        chat_prompt: sopkb_agent::SYSTEM_PROMPT.to_string(),
        heading_index_prompt: sopkb_workbench::HEADING_INDEX_SYSTEM_PROMPT.to_string(),
        heading_relevel_prompt: sopkb_workbench::HEADING_RELEVEL_SYSTEM_PROMPT.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionTestResult {
    pub ok: bool,
    pub detail: String,
}

/// This command's entire purpose is "tell me if this profile works" -- per §4.8 it
/// must never itself return a hard Tauri error for a bad connection, so failures are
/// caught and reported in `detail` rather than propagated with `?`. Runs on
/// `spawn_blocking` since `sopkb_llm::chat_call` is a real, synchronous, potentially
/// slow network call.
#[tauri::command(rename_all = "snake_case")]
pub async fn test_profile_connection(id: Option<String>) -> ConnectionTestResult {
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        sopkb_llm::chat_call(&[sopkb_llm::Message::user("Connection test. Reply with the single word OK.")], id.as_deref())
    })
    .await;
    match outcome {
        Ok(Ok(text)) => ConnectionTestResult { ok: true, detail: text },
        Ok(Err(err)) => ConnectionTestResult { ok: false, detail: err.to_string() },
        Err(join_err) => ConnectionTestResult { ok: false, detail: format!("connection test task did not complete: {join_err}") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn profile(id: &str, api_key: &str) -> ModelProfile {
        ModelProfile { id: id.to_string(), name: id.to_string(), api_key: api_key.to_string(), ..Default::default() }
    }

    #[test]
    fn build_profile_to_save_generates_a_fresh_id_when_absent() {
        let input = ProfileInput {
            id: None,
            name: "My Profile".to_string(),
            base_url: String::new(),
            auth_style: String::new(),
            model: String::new(),
            max_output_tokens: String::new(),
            timeout_seconds: String::new(),
            reasoning_effort: String::new(),
            mining_prompt: String::new(),
            chat_prompt: String::new(),
            api_key: Some("secret".to_string()),
        };
        let saved = build_profile_to_save(input, &[]);
        assert_eq!(saved.id, "my-profile");
        assert_eq!(saved.api_key, "secret");
    }

    #[test]
    fn build_profile_to_save_keeps_existing_key_when_input_key_is_blank() {
        let existing = vec![profile("p1", "existing-secret")];
        let input = ProfileInput {
            id: Some("p1".to_string()),
            name: "P1".to_string(),
            base_url: String::new(),
            auth_style: String::new(),
            model: String::new(),
            max_output_tokens: String::new(),
            timeout_seconds: String::new(),
            reasoning_effort: String::new(),
            mining_prompt: String::new(),
            chat_prompt: String::new(),
            api_key: None,
        };
        let saved = build_profile_to_save(input.clone(), &existing);
        assert_eq!(saved.api_key, "existing-secret", "absent api_key must keep the existing one");

        let mut blank_input = input;
        blank_input.api_key = Some("   ".to_string());
        let saved_blank = build_profile_to_save(blank_input, &existing);
        assert_eq!(saved_blank.api_key, "existing-secret", "blank api_key must also keep the existing one");
    }

    #[test]
    fn build_profile_to_save_overwrites_key_when_a_real_value_is_given() {
        let existing = vec![profile("p1", "old-secret")];
        let input = ProfileInput {
            id: Some("p1".to_string()),
            name: "P1".to_string(),
            base_url: String::new(),
            auth_style: String::new(),
            model: String::new(),
            max_output_tokens: String::new(),
            timeout_seconds: String::new(),
            reasoning_effort: String::new(),
            mining_prompt: String::new(),
            chat_prompt: String::new(),
            api_key: Some("new-secret".to_string()),
        };
        let saved = build_profile_to_save(input, &existing);
        assert_eq!(saved.api_key, "new-secret");
    }

    #[test]
    fn profile_view_never_carries_the_raw_key() {
        let p = profile("p1", "super-secret");
        let view = ProfileView::from_profile(&p, "p1");
        assert!(view.has_api_key);
        assert!(view.is_default);
        // Compile-time guarantee, not just a runtime check: ProfileView has no
        // `api_key`-shaped field at all, so there is nothing to serialize even by
        // accident. This assertion documents that the serialized form is exactly
        // the 12 non-secret fields.
        let value = serde_json::to_value(&view).unwrap();
        assert!(value.get("api_key").is_none());
        assert!(value.get("has_api_key").is_some());
    }

    #[test]
    #[serial(sopkb_llm_env_overrides)]
    fn compute_env_overrides_reports_inactive_when_no_env_var_set() {
        for (_, vars) in ENV_OVERRIDE_CANDIDATES {
            for v in *vars {
                unsafe { std::env::remove_var(v) };
            }
        }
        let overrides = compute_env_overrides();
        assert_eq!(overrides.len(), 7);
        for status in &overrides {
            assert!(!status.active_value_present, "{} should be inactive", status.field);
        }
    }

    #[test]
    #[serial(sopkb_llm_env_overrides)]
    fn compute_env_overrides_detects_first_non_blank_candidate_in_order() {
        for (_, vars) in ENV_OVERRIDE_CANDIDATES {
            for v in *vars {
                unsafe { std::env::remove_var(v) };
            }
        }
        unsafe { std::env::set_var("OPENAI_API_KEY", "sk-test") };
        let overrides = compute_env_overrides();
        let api_key_status = overrides.iter().find(|o| o.field == "api_key").unwrap();
        assert!(api_key_status.active_value_present);
        assert_eq!(api_key_status.env_var, "OPENAI_API_KEY");
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
    }
}
