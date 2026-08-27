//! Profile persistence, legacy migration, and the `resolve`/`require`/`auth_headers`
//! layered lookup. docs/port/port-mapping-b-mining-settings.md §3.4.

use crate::profile::ModelProfile;
use sopkb_core::error::{Result, SopkbError};
use sopkb_core::ids::slugify;
use std::collections::HashMap;
use std::path::PathBuf;

/// Checked in this order per field; first non-blank (after trim) wins.
const ENV_OVERRIDES: &[(&str, &[&str])] = &[
    ("base_url", &["AZURE_OPENAI_BASE_URL", "AZURE_OPENAI_ENDPOINT", "OPENAI_BASE_URL"]),
    ("api_key", &["AZURE_OPENAI_API_KEY", "OPENAI_API_KEY"]),
    ("model", &["AZURE_OPENAI_DEPLOYMENT", "OPENAI_MODEL"]),
    ("auth_style", &["SOPKB_LLM_AUTH_STYLE"]),
    ("max_output_tokens", &["AZURE_OPENAI_MAX_OUTPUT_TOKENS"]),
    ("timeout_seconds", &["AZURE_OPENAI_TIMEOUT_SECONDS"]),
    ("reasoning_effort", &["AZURE_OPENAI_REASONING_EFFORT"]),
    // id, name, mining_prompt, chat_prompt: no entry -> env layer skipped entirely.
];

/// old flat settings key -> profile field, iteration/declaration order matters for migration.
const LEGACY_FIELD_MAP: &[(&str, &str)] = &[
    ("llm_base_url", "base_url"),
    ("llm_api_key", "api_key"),
    ("llm_auth_style", "auth_style"),
    ("llm_model", "model"),
    ("llm_max_output_tokens", "max_output_tokens"),
    ("llm_timeout_seconds", "timeout_seconds"),
    ("llm_reasoning_effort", "reasoning_effort"),
];

fn env_overrides_for(field: &str) -> &'static [&'static str] {
    ENV_OVERRIDES.iter().find(|(f, _)| *f == field).map(|(_, e)| *e).unwrap_or(&[])
}

/// Only 3 fields have built-in defaults; everything else defaults to `""`.
fn default_for(field: &str) -> &'static str {
    match field {
        "auth_style" => "azure",
        "max_output_tokens" => "4096",
        "timeout_seconds" => "60",
        _ => "",
    }
}

/// No Python equivalent -- Python hardcodes its own `_MAX_PARALLEL_*` constants at
/// 6 in each of `normalize.py`/`okf_author.py`, matching this port's own previous
/// hardcoded default. Configurable here so a user whose LLM endpoint throttles hard
/// under 6 concurrent requests can turn it down, without needing a source change.
pub const DEFAULT_MAX_PARALLEL_WORKERS: usize = 6;

#[derive(Debug, Clone)]
pub struct SettingsConfig {
    pub profiles: Vec<ModelProfile>,
    pub default_profile_id: String,
    pub reviewer_name: String,
    pub max_parallel_workers: usize,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self { profiles: Vec::new(), default_profile_id: String::new(), reviewer_name: String::new(), max_parallel_workers: DEFAULT_MAX_PARALLEL_WORKERS }
    }
}

/// `$SOPKB_SETTINGS_PATH` if set and non-blank, else `~/.sopkb/settings.json`. Home
/// directory resolution is a best-effort match of Python's `Path.home()` (checks
/// `$HOME` then `%USERPROFILE%`) -- this function is inherently host-environment
/// dependent and not something the differential-test methodology used elsewhere in
/// this port can meaningfully pin.
pub fn settings_path() -> PathBuf {
    if let Ok(over) = std::env::var("SOPKB_SETTINGS_PATH") {
        let trimmed = over.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home_dir().join(".sopkb").join("settings.json")
}

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

fn to_string_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "None".to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Missing file -> defaults (`{}`). Invalid JSON PROPAGATES as a parse error in Python;
/// here it's swallowed to defaults instead, since `load_config` has no `Result` in the
/// original signature and every call site treats it as infallible -- this is the one
/// spot this port intentionally diverges for ergonomics (a corrupt settings.json
/// degrading to "no profiles" rather than crashing every settings-touching call).
pub fn load_config() -> SettingsConfig {
    let path = settings_path();
    let data: serde_json::Value =
        std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_else(|| serde_json::json!({}));
    let data = if data.is_object() { data } else { serde_json::json!({}) };

    let profiles_list: Vec<serde_json::Value> = match data.get("profiles").and_then(|p| p.as_array()) {
        Some(arr) => arr.clone(),
        None => {
            // Migration triggers only when `profiles` is absent/null/non-list; an
            // existing `"profiles": []` blocks migration entirely.
            let mut legacy = serde_json::Map::new();
            for (old_key, new_key) in LEGACY_FIELD_MAP {
                if let Some(raw) = data.get(*old_key) {
                    let s = to_string_value(raw);
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        legacy.insert(new_key.to_string(), serde_json::json!(trimmed));
                    }
                }
            }
            if legacy.is_empty() {
                vec![]
            } else {
                let mut merged = serde_json::Map::new();
                merged.insert("id".to_string(), serde_json::json!("default"));
                merged.insert("name".to_string(), serde_json::json!("Default"));
                for (k, v) in legacy {
                    merged.insert(k, v);
                }
                vec![serde_json::Value::Object(merged)]
            }
        }
    };

    let mut cleaned_profiles: Vec<ModelProfile> = Vec::new();
    for p in &profiles_list {
        if !p.is_object() {
            continue;
        }
        let id_str = to_string_value(p.get("id").unwrap_or(&serde_json::Value::String(String::new())));
        if id_str.trim().is_empty() {
            continue;
        }
        cleaned_profiles.push(ModelProfile::from_json_normalized(p));
    }

    let mut default_profile_id = to_string_value(data.get("default_profile_id").unwrap_or(&serde_json::Value::String(String::new()))).trim().to_string();
    if !cleaned_profiles.iter().any(|p| p.id == default_profile_id) {
        default_profile_id = cleaned_profiles.first().map(|p| p.id.clone()).unwrap_or_default();
    }

    let reviewer_name = to_string_value(data.get("reviewer_name").unwrap_or(&serde_json::Value::String(String::new()))).trim().to_string();
    let max_parallel_workers =
        data.get("max_parallel_workers").and_then(|v| v.as_u64()).map(|v| v as usize).filter(|&v| v >= 1).unwrap_or(DEFAULT_MAX_PARALLEL_WORKERS);

    SettingsConfig { profiles: cleaned_profiles, default_profile_id, reviewer_name, max_parallel_workers }
}

/// Only these three keys ever survive a save; any other on-disk key (legacy flat
/// `llm_*`, hand-added fields) is discarded. `profiles` is written exactly as passed --
/// normalization to `PROFILE_FIELDS` happens on load, and in `save_profile`, not here.
pub fn save_config(config: &SettingsConfig) -> Result<()> {
    let path = settings_path();
    let data = serde_json::json!({
        "profiles": config.profiles.iter().map(|p| p.to_json()).collect::<Vec<_>>(),
        "default_profile_id": config.default_profile_id,
        "reviewer_name": config.reviewer_name,
        "max_parallel_workers": config.max_parallel_workers,
    });
    sopkb_core::store::write_json(&path, &data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
    Ok(())
}

pub fn list_profiles() -> Vec<ModelProfile> {
    load_config().profiles
}

/// `profile_id or default_profile_id` -- both `None` and `""` fall back to the default.
/// An unknown id returns `None` (no fallback to default for an unrecognized id).
pub fn get_profile(profile_id: Option<&str>) -> Option<ModelProfile> {
    let config = load_config();
    let pid = match profile_id {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => config.default_profile_id.clone(),
    };
    config.profiles.into_iter().find(|p| p.id == pid)
}

pub fn get_default_profile_id() -> String {
    load_config().default_profile_id
}

pub fn unique_profile_id(name: &str, existing: &[ModelProfile]) -> String {
    let base = slugify(name);
    let taken: std::collections::HashSet<&str> = existing.iter().map(|p| p.id.as_str()).collect();
    if !taken.contains(base.as_str()) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Upsert by id (remove any existing entry with the same id, then append at the end).
/// The first ever saved profile becomes the default; subsequent saves do not steal it.
pub fn save_profile(profile: &ModelProfile) -> Result<()> {
    let mut config = load_config();
    config.profiles.retain(|p| p.id != profile.id);
    config.profiles.push(ModelProfile::from_json_normalized(&profile.to_json()));
    if config.default_profile_id.is_empty() {
        config.default_profile_id = profile.id.clone();
    }
    save_config(&config)
}

/// Always writes, even if `profile_id` matched nothing. Reassigns the default to the
/// first remaining profile, or clears it to `""`.
pub fn delete_profile(profile_id: &str) -> Result<()> {
    let mut config = load_config();
    config.profiles.retain(|p| p.id != profile_id);
    if config.default_profile_id == profile_id {
        config.default_profile_id = config.profiles.first().map(|p| p.id.clone()).unwrap_or_default();
    }
    save_config(&config)
}

/// A no-op (file NOT rewritten) if `profile_id` doesn't match any existing profile.
pub fn set_default_profile(profile_id: &str) -> Result<()> {
    let mut config = load_config();
    if config.profiles.iter().any(|p| p.id == profile_id) {
        config.default_profile_id = profile_id.to_string();
        save_config(&config)?;
    }
    Ok(())
}

pub fn reviewer_name() -> String {
    load_config().reviewer_name
}

pub fn set_reviewer_name(value: &str) -> Result<()> {
    let mut config = load_config();
    config.reviewer_name = value.trim().to_string();
    save_config(&config)
}

pub fn max_parallel_workers() -> usize {
    load_config().max_parallel_workers
}

/// Clamped to at least 1 -- a caller passing 0 almost certainly means "don't limit
/// me to nothing", not "never run anything", and 0 would deadlock every one of this
/// port's `parallel_map` call sites (`max_workers.min(total).max(1)` already floors
/// at 1 there too, so this just keeps the STORED value honest about what it means).
pub fn set_max_parallel_workers(value: usize) -> Result<()> {
    let mut config = load_config();
    config.max_parallel_workers = value.max(1);
    save_config(&config)
}

/// Precedence: env var (first non-blank in the field's ordered list) > stored profile
/// value (stripped) > `DEFAULTS` > `""`. Env vars apply globally, overriding every
/// profile. Fields with no `ENV_OVERRIDES` entry skip layer 1 entirely.
pub fn resolve(field: &str, profile_id: Option<&str>) -> String {
    for env_name in env_overrides_for(field) {
        if let Ok(value) = std::env::var(env_name) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(profile) = get_profile(profile_id) {
        if let Some(stored) = profile.get(field) {
            let trimmed = stored.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    default_for(field).to_string()
}

pub fn require(field: &str, label: &str, profile_id: Option<&str>) -> Result<String> {
    let value = resolve(field, profile_id);
    if !value.is_empty() {
        return Ok(value);
    }
    let env_names = env_overrides_for(field);
    let via_env = if env_names.is_empty() { String::new() } else { format!(" or {}", env_names.join(" / ")) };
    Err(SopkbError::Value(format!("Missing {label}. Configure a model profile on the Settings screen{via_env}.")))
}

/// Exact-match, case-sensitive on `"openai"`; every other value (including `"azure"`,
/// `""`, and any unrecognized string) falls into the `api-key` branch. No validation.
pub fn auth_headers(auth_style: &str, api_key: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if auth_style == "openai" {
        headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));
    } else {
        headers.insert("api-key".to_string(), api_key.to_string());
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    fn with_settings_path<F: FnOnce()>(f: F) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        // SAFETY: tests using this helper are marked #[serial] to avoid concurrent
        // env mutation across threads (Rust's default test runner shares one process).
        unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &path) };
        f();
        unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
    }

    #[test]
    #[serial]
    fn fresh_settings_file_yields_empty_defaults() {
        with_settings_path(|| {
            assert!(list_profiles().is_empty());
            assert_eq!(reviewer_name(), "");
            assert_eq!(get_default_profile_id(), "");
        });
    }

    #[test]
    #[serial]
    fn first_save_becomes_default_second_does_not_steal_it() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            let p2 = ModelProfile { id: "p2".into(), name: "Two".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            save_profile(&p2).unwrap();
            assert_eq!(get_default_profile_id(), "p1");
        });
    }

    #[test]
    #[serial]
    fn upsert_does_not_duplicate() {
        with_settings_path(|| {
            let mut p = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            save_profile(&p).unwrap();
            p.name = "One Updated".into();
            save_profile(&p).unwrap();
            let profiles = list_profiles();
            assert_eq!(profiles.len(), 1);
            assert_eq!(profiles[0].name, "One Updated");
        });
    }

    #[test]
    #[serial]
    fn get_profile_none_is_default_unknown_is_none() {
        with_settings_path(|| {
            let p = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            save_profile(&p).unwrap();
            assert_eq!(get_profile(None).unwrap().id, "p1");
            assert!(get_profile(Some("nope")).is_none());
        });
    }

    #[test]
    #[serial]
    fn delete_reassigns_default_or_clears_it() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            let p2 = ModelProfile { id: "p2".into(), name: "Two".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            save_profile(&p2).unwrap();
            delete_profile("p1").unwrap();
            assert_eq!(get_default_profile_id(), "p2");
            delete_profile("p2").unwrap();
            assert_eq!(get_default_profile_id(), "");
        });
    }

    #[test]
    #[serial]
    fn set_default_profile_unknown_is_noop() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            set_default_profile("does-not-exist").unwrap();
            assert_eq!(get_default_profile_id(), "p1");
        });
    }

    #[test]
    #[serial]
    fn set_reviewer_name_trims_and_leaves_profiles_untouched() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            set_reviewer_name("  Jane Doe  ").unwrap();
            assert_eq!(reviewer_name(), "Jane Doe");
            assert_eq!(list_profiles().len(), 1);
        });
    }

    #[test]
    #[serial]
    fn max_parallel_workers_defaults_to_six_with_no_settings_file() {
        with_settings_path(|| {
            assert_eq!(max_parallel_workers(), DEFAULT_MAX_PARALLEL_WORKERS);
            assert_eq!(DEFAULT_MAX_PARALLEL_WORKERS, 6);
        });
    }

    #[test]
    #[serial]
    fn set_max_parallel_workers_round_trips_and_leaves_profiles_untouched() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            set_max_parallel_workers(2).unwrap();
            assert_eq!(max_parallel_workers(), 2);
            assert_eq!(list_profiles().len(), 1);
        });
    }

    #[test]
    #[serial]
    fn set_max_parallel_workers_clamps_zero_to_one() {
        with_settings_path(|| {
            set_max_parallel_workers(0).unwrap();
            assert_eq!(max_parallel_workers(), 1);
        });
    }

    #[test]
    #[serial]
    fn legacy_flat_settings_migrate_to_default_profile() {
        with_settings_path(|| {
            let path = settings_path();
            std::fs::write(
                &path,
                r#"{"llm_base_url": "https://example.test", "llm_api_key": "secret", "llm_model": "gpt-x"}"#,
            )
            .unwrap();
            let config = load_config();
            assert_eq!(config.profiles.len(), 1);
            assert_eq!(config.profiles[0].id, "default");
            assert_eq!(config.profiles[0].name, "Default");
            assert_eq!(config.profiles[0].base_url, "https://example.test");
            assert_eq!(config.profiles[0].api_key, "secret");
            assert_eq!(config.profiles[0].model, "gpt-x");
            assert_eq!(config.default_profile_id, "default");
        });
    }

    #[test]
    #[serial]
    fn resolve_precedence_env_over_profile_over_default() {
        with_settings_path(|| {
            let p1 = ModelProfile { id: "p1".into(), name: "One".into(), base_url: "https://profile.example".into(), ..Default::default() };
            save_profile(&p1).unwrap();
            assert_eq!(resolve("base_url", Some("p1")), "https://profile.example");
            unsafe { std::env::set_var("AZURE_OPENAI_BASE_URL", "https://env.example") };
            assert_eq!(resolve("base_url", Some("p1")), "https://env.example");
            unsafe { std::env::remove_var("AZURE_OPENAI_BASE_URL") };
            assert_eq!(resolve("auth_style", Some("p1")), "azure"); // DEFAULTS fallback
        });
    }

    #[test]
    #[serial]
    fn require_missing_field_error_message() {
        with_settings_path(|| {
            let err = require("model", "a model/deployment name", None).unwrap_err();
            assert_eq!(
                err.to_string(),
                "Missing a model/deployment name. Configure a model profile on the Settings screen or AZURE_OPENAI_DEPLOYMENT / OPENAI_MODEL."
            );
        });
    }

    #[test]
    fn auth_headers_azure_openai_and_unrecognized() {
        assert_eq!(auth_headers("openai", "k").get("Authorization").unwrap(), "Bearer k");
        assert_eq!(auth_headers("azure", "k").get("api-key").unwrap(), "k");
        assert_eq!(auth_headers("", "k").get("api-key").unwrap(), "k");
        assert_eq!(auth_headers("OpenAI", "k").get("api-key").unwrap(), "k"); // case-sensitive
    }

    #[test]
    fn unique_profile_id_appends_numeric_suffix() {
        let existing = vec![
            ModelProfile { id: "my-model".into(), ..Default::default() },
            ModelProfile { id: "my-model-2".into(), ..Default::default() },
        ];
        assert_eq!(unique_profile_id("My Model", &existing), "my-model-3");
        assert_eq!(unique_profile_id("Brand New", &existing), "brand-new");
    }
}
