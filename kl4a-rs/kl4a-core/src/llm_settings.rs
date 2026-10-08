//! Port of `kl4a/kl4a/llm_settings.py`. Grounded via `tools-code` MCP
//! `code_symbols_get` against `source-kl4a-kl4a-llm-settings-py`.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::llm_provider::{self, Message, Provider, ProviderError};

/// Port of `SETTINGS_ENV_VAR = "SOPKB_LLM_SETTINGS"` (`kl4a/kl4a/llm_settings.py:37`).
pub const SETTINGS_ENV_VAR: &str = "SOPKB_LLM_SETTINGS";
/// Port of `DEFAULT_SETTINGS_DIR = ".sopkb"` (`kl4a/kl4a/llm_settings.py:38`).
pub const DEFAULT_SETTINGS_DIR: &str = ".sopkb";
/// Port of `DEFAULT_SETTINGS_NAME = "llm_settings.json"` (`kl4a/kl4a/llm_settings.py:39`).
pub const DEFAULT_SETTINGS_NAME: &str = "llm_settings.json";
/// Port of `DOTENV_ENV_VAR = "SOPKB_ENV_FILE"` (`kl4a/kl4a/llm_settings.py:45`).
pub const DOTENV_ENV_VAR: &str = "SOPKB_ENV_FILE";
/// Port of `DEFAULT_DOTENV_NAME = ".env"` (`kl4a/kl4a/llm_settings.py:46`).
pub const DEFAULT_DOTENV_NAME: &str = ".env";

/// Port of `kl4a.kl4a.llm_settings.FIELDS` (`kl4a/kl4a/llm_settings.py:49-57`).
/// Kept as an ordered list (Python dict preserves insertion order, used by
/// `resolved_settings`'s iteration order).
pub const FIELDS: [&str; 7] = [
    "provider",
    "base_url",
    "api_key",
    "deployment",
    "max_output_tokens",
    "timeout_seconds",
    "reasoning_effort",
];

/// Port of `kl4a.kl4a.llm_settings.SECRET_FIELDS = {"api_key"}` (`kl4a/kl4a/llm_settings.py:59`).
pub const SECRET_FIELDS: [&str; 1] = ["api_key"];

fn settings_path() -> PathBuf {
    let configured = env::var(SETTINGS_ENV_VAR).unwrap_or_default();
    let configured = configured.trim();
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }
    dirs_home()
        .join(DEFAULT_SETTINGS_DIR)
        .join(DEFAULT_SETTINGS_NAME)
}

fn dotenv_path() -> PathBuf {
    let configured = env::var(DOTENV_ENV_VAR).unwrap_or_default();
    let configured = configured.trim();
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(DEFAULT_DOTENV_NAME)
}

/// No `dirs`/`home` crate is in the approved dependency list, so `HOME`
/// (unix) / `USERPROFILE` (windows) is read directly, matching what
/// `Path.home()` (= `Path("~").expanduser()`) resolves to on each platform.
///
/// Fix (Low finding — settings directory resolution): the previous version
/// here checked `HOME` first on *every* platform, including Windows. That's
/// backwards for Windows: CPython's `Path.home()` goes through
/// `ntpath.expanduser`, confirmed by reading
/// `Lib/ntpath.py::expanduser` directly (the function this crate has no
/// access to at runtime, so it's grounded here instead) — on Windows it
/// checks `USERPROFILE` first, falling back to `HOMEDRIVE`+`HOMEPATH`, and
/// **never consults `HOME` at all**, no matter what it's set to. So on a
/// Windows machine where `HOME` happens to be set to something different
/// from `USERPROFILE` (common in POSIX-emulation shells — Git Bash/MSYS
/// export `HOME` — or any CI container that sets both), the previous
/// version silently resolved to the wrong directory and read/wrote
/// `llm_settings.json` somewhere the user's real saved settings (written by
/// Python, or by the workbench UI, at `%USERPROFILE%\.sopkb\...`) are never
/// found — exactly the bug this finding describes ("Rust reportedly checks
/// `HOME` first ... silently causes Rust to ignore the user's saved
/// settings"). `posixpath.expanduser` (`Lib/posixpath.py`) confirms Unix
/// does check `HOME` first, so that half of the previous logic was correct
/// — only the platform split was missing.
fn dirs_home() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(profile) = env::var("USERPROFILE") {
            if !profile.is_empty() {
                return PathBuf::from(profile);
            }
        }
        if let Ok(homepath) = env::var("HOMEPATH") {
            if !homepath.is_empty() {
                let drive = env::var("HOMEDRIVE").unwrap_or_default();
                return PathBuf::from(format!("{drive}{homepath}"));
            }
        }
        return PathBuf::from(".");
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = env::var("HOME") {
            if !home.is_empty() {
                return PathBuf::from(home);
            }
        }
        PathBuf::from(".")
    }
}

/// Port of `kl4a.kl4a.llm_settings.load_settings` (`kl4a/kl4a/llm_settings.py:87-101`).
pub fn load_settings() -> HashMap<String, String> {
    let path = settings_path();
    if !path.exists() {
        return HashMap::new();
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return HashMap::new(),
    };
    let data: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let Some(obj) = data.as_object() else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (key, value) in obj {
        if !FIELDS.contains(&key.as_str()) {
            continue;
        }
        let is_blank = matches!(value, serde_json::Value::Null)
            || value.as_str().map(|s| s.is_empty()).unwrap_or(false);
        if is_blank {
            continue;
        }
        let as_string = match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        out.insert(key.clone(), as_string);
    }
    out
}

/// Port of `kl4a.kl4a.llm_settings.write_settings` (`kl4a/kl4a/llm_settings.py:175-179`
/// referenced by `save_settings`/`clear_setting`/`clear_all`). Also applies
/// `restrict_permissions` (unix-only `0o600`, a no-op elsewhere, matching
/// Python's own platform guard).
fn write_settings(settings: &HashMap<String, String>) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let ordered: serde_json::Map<String, serde_json::Value> = FIELDS
        .iter()
        .filter_map(|f| settings.get(*f).map(|v| (f.to_string(), serde_json::Value::String(v.clone()))))
        .collect();
    if let Ok(text) = serde_json::to_string_pretty(&serde_json::Value::Object(ordered)) {
        let _ = fs::write(&path, text);
    }
    restrict_permissions(&path);
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = fs::metadata(path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let _ = fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) {}

/// Port of `kl4a.kl4a.llm_settings.save_settings` (`kl4a/kl4a/llm_settings.py:145-160`).
pub fn save_settings(updates: &HashMap<String, String>) -> HashMap<String, String> {
    let mut settings = load_settings();
    for (key, value) in updates {
        if !FIELDS.contains(&key.as_str()) {
            continue;
        }
        let value = value.trim();
        if !value.is_empty() {
            settings.insert(key.clone(), value.to_string());
        }
    }
    write_settings(&settings);
    settings
}

/// Port of `kl4a.kl4a.llm_settings.clear_setting` (`kl4a/kl4a/llm_settings.py:163-167`).
pub fn clear_setting(field: &str) -> HashMap<String, String> {
    let mut settings = load_settings();
    settings.remove(field);
    write_settings(&settings);
    settings
}

/// Port of `kl4a.kl4a.llm_settings.clear_all` (`kl4a/kl4a/llm_settings.py:170-172`).
pub fn clear_all() -> HashMap<String, String> {
    let settings = HashMap::new();
    write_settings(&settings);
    settings
}

/// Port of `kl4a.kl4a.llm_settings.load_dotenv` (`kl4a/kl4a/llm_settings.py:111-142`).
pub fn load_dotenv() -> HashMap<String, String> {
    let path = dotenv_path();
    if !path.exists() {
        return HashMap::new();
    }
    let Ok(text) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    let mut values = HashMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || !line.contains('=') {
            continue;
        }
        let (key, value) = line.split_once('=').unwrap();
        let key = key.trim();
        let mut value = value.trim();
        if value.len() >= 2 {
            let bytes = value.as_bytes();
            let first = bytes[0];
            let last = bytes[bytes.len() - 1];
            if first == last && (first == b'"' || first == b'\'') {
                value = &value[1..value.len() - 1];
            }
        }
        if !key.is_empty() {
            values.insert(key.to_string(), value.to_string());
        }
    }
    values
}

/// Port of `kl4a.kl4a.llm_settings.env_names` (`kl4a/kl4a/llm_settings.py:194-203`).
pub fn env_names(field: &str) -> Vec<&'static str> {
    if field == "provider" {
        return vec!["SOPKB_LLM_PROVIDER"];
    }
    let provider_id = active_provider_id();
    let meta = provider_meta_for(&provider_id);
    meta.env_names_for(field).to_vec()
}

fn provider_meta_for(id: &str) -> &'static llm_provider::ProviderMeta {
    match id {
        "openai-compatible" => &llm_provider::OPENAI_COMPATIBLE_META,
        "anthropic" => &llm_provider::ANTHROPIC_META,
        _ => &llm_provider::AZURE_RESPONSES_META,
    }
}

/// Port of `kl4a.kl4a.llm_settings.resolve` (`kl4a/kl4a/llm_settings.py:206-228`).
/// Returns `(value, source)`, `source` one of `"saved"`, `"environment"`,
/// `"dotenv"`, `"unset"`.
pub fn resolve(field: &str) -> (String, &'static str) {
    let names = env_names(field);
    let saved = load_settings();
    if let Some(value) = saved.get(field) {
        let value = value.trim();
        if !value.is_empty() {
            return (value.to_string(), "saved");
        }
    }
    for name in &names {
        if let Ok(value) = env::var(name) {
            let value = value.trim();
            if !value.is_empty() {
                return (value.to_string(), "environment");
            }
        }
    }
    let dotenv_values = load_dotenv();
    for name in &names {
        if let Some(value) = dotenv_values.get(*name) {
            let value = value.trim();
            if !value.is_empty() {
                return (value.to_string(), "dotenv");
            }
        }
    }
    (String::new(), "unset")
}

/// Port of `kl4a.kl4a.llm_settings.effective_settings` (`kl4a/kl4a/llm_settings.py:235-239`).
pub fn effective_settings() -> HashMap<String, String> {
    let mut settings: HashMap<String, String> =
        FIELDS.iter().map(|f| (f.to_string(), resolve(f).0)).collect();
    let provider = settings.get("provider").cloned();
    settings.insert(
        "provider".to_string(),
        llm_provider::resolve_provider_id(provider.as_deref()),
    );
    settings
}

/// Port of `kl4a.kl4a.llm_settings.active_provider_id` (`kl4a/kl4a/llm_settings.py:242-243`).
pub fn active_provider_id() -> String {
    llm_provider::resolve_provider_id(Some(&resolve("provider").0))
}

/// Port of `kl4a.kl4a.llm_settings.active_provider` (`kl4a/kl4a/llm_settings.py:246-247`).
pub fn active_provider() -> Box<dyn Provider> {
    llm_provider::build_provider(&effective_settings())
}

/// Port of `kl4a.kl4a.llm_settings.resolved_settings` (`kl4a/kl4a/llm_settings.py:231-232`).
pub fn resolved_settings() -> HashMap<String, (String, &'static str)> {
    FIELDS.iter().map(|f| (f.to_string(), resolve(f))).collect()
}

/// Port of `kl4a.kl4a.llm_settings.required_fields` (`kl4a/kl4a/llm_settings.py:314-316`).
pub fn required_fields() -> &'static [&'static str] {
    provider_meta_for(&active_provider_id()).required_fields
}

/// Port of `kl4a.kl4a.llm_settings.missing_fields` (`kl4a/kl4a/llm_settings.py:323-324`).
pub fn missing_fields() -> Vec<String> {
    required_fields()
        .iter()
        .filter(|f| resolve(f).0.is_empty())
        .map(|f| f.to_string())
        .collect()
}

/// Port of `kl4a.kl4a.llm_settings.is_configured` (`kl4a/kl4a/llm_settings.py:319-320`).
pub fn is_configured() -> bool {
    missing_fields().is_empty()
}

/// Port of `kl4a.kl4a.llm_settings.mask` (`kl4a/kl4a/llm_settings.py:327-333`).
pub fn mask(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    if value.len() <= 8 {
        return "*".repeat(value.len());
    }
    let chars: Vec<char> = value.chars().collect();
    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars.iter().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{prefix}{}{suffix}", "*".repeat(8))
}

/// Port of `kl4a.kl4a.llm_settings.display_value` (`kl4a/kl4a/llm_settings.py:336-338`).
pub fn display_value(field: &str) -> String {
    let (value, _source) = resolve(field);
    if SECRET_FIELDS.contains(&field) {
        mask(&value)
    } else {
        value
    }
}

/// Port of `kl4a.kl4a.llm_settings.apply_to_env` (`kl4a/kl4a/llm_settings.py:298-311`).
pub fn apply_to_env() -> Vec<String> {
    let mut applied = Vec::new();
    for (field, value) in load_settings() {
        let names = env_names(&field);
        if names.is_empty() {
            continue;
        }
        let already_set = names.iter().any(|n| {
            env::var(n).map(|v| !v.trim().is_empty()).unwrap_or(false)
        });
        if already_set {
            continue;
        }
        env::set_var(names[0], &value);
        applied.push(names[0].to_string());
    }
    applied
}

/// Port of `kl4a.kl4a.llm_settings.complete` (`kl4a/kl4a/llm_settings.py:256-281`).
/// `log`, if given, receives one line before the request and one on success
/// or failure, matching the Python original's optional progress callback.
pub fn complete(
    messages: &[Message],
    log: Option<&mut dyn FnMut(&str)>,
) -> Result<String, ProviderError> {
    let provider = active_provider();
    let mut log = log;
    if let Some(log) = log.as_deref_mut() {
        log(&format!(
            "requesting via {} ({} message(s))",
            provider.meta().label,
            messages.len()
        ));
    }
    let started = std::time::Instant::now();
    match provider.complete(messages) {
        Ok(text) => {
            if let Some(log) = log.as_deref_mut() {
                log(&format!(
                    "response received after {:.1}s",
                    started.elapsed().as_secs_f64()
                ));
            }
            Ok(text)
        }
        Err(exc) => {
            if let Some(log) = log.as_deref_mut() {
                log(&format!(
                    "failed after {:.1}s: {exc}",
                    started.elapsed().as_secs_f64()
                ));
            }
            Err(exc)
        }
    }
}
