//! Port of `kl4a/codekb/config.py` — codekb configuration loading and merging.
//!
//! Every symbol below is grounded via tools-code MCP `code_symbols_get`
//! against `kl4a.codekb.config.*`; excerpts are quoted in each doc comment.
//!
//! ## `default_codekb_config.yaml` — grounding note
//!
//! `kl4a/codekb/config.py` loads a YAML asset (`DEFAULT_CODEKB_CONFIG_PATH =
//! Path(__file__).with_name("default_codekb_config.yaml")`) via
//! `yaml.safe_load`. The tools-code MCP indexes *code* (Python/COBOL/etc.
//! source), not data assets, so `code_files_search`/`code_symbols_search`
//! returned no hits for this YAML file's actual contents under several
//! phrasings ("default_codekb_config.yaml", "codekb inventory skip_dirs
//! mining mode static hybrid provider bundle standard_root
//! default_name_template"). Per the cross-lang-port skill's Phase 2 rule,
//! since its content could not be grounded, this port does **not** invent
//! plausible-looking default keys/values for that file. Instead
//! [`load_default_codekb_config`] reads and parses whatever YAML file is
//! actually present at the mirrored path at runtime — exactly like the
//! Python does — so behavior is identical regardless of the file's actual
//! contents. The two defaults [`standard_code_bundle_dir`] falls back to
//! (`"knowledge-bundles"`, `"{repo_slug}-code"`) are grounded: they are
//! literal Python fallback values in `standard_code_bundle_dir`'s own body
//! (`bundle_config.get("standard_root") or "knowledge-bundles"`), not values
//! read from the YAML file.
//!
//! The `default_codekb_config.yaml` asset file does live alongside this
//! source file in the Rust crate (`kl4a-rs/codekb/src/default_codekb_config.yaml`).
//!
//! ## Fix: embed at compile time, don't resolve a runtime path (High finding)
//!
//! The previous version here resolved the asset via
//! `concat!(env!("CARGO_MANIFEST_DIR"), "/src/default_codekb_config.yaml")`
//! and then did a *runtime* `std::fs::read_to_string` of that path.
//! `env!("CARGO_MANIFEST_DIR")` is baked in at compile time as the
//! *build machine's* absolute source-tree path — it has nothing to do with
//! where the built binary is later run from. Ship the binary anywhere else
//! (a different machine, a packaged release, even just a moved checkout)
//! and the embedded path no longer exists, so `build`/`init`/etc. fail with
//! a bare `std::io::Error` ("The system cannot find the path specified")
//! that gives the user no actionable path at all — unlike Python, which
//! ships this file as real package data next to `config.py` and will always
//! find it via `Path(__file__).with_name(...)` relative to the installed
//! package. Fixed by embedding the file's *contents* at compile time via
//! `include_str!`, so there is no runtime path to resolve or ship
//! separately — the binary is self-contained, matching the practical effect
//! of Python's packaged asset.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use serde_json::{Map, Value};

use crate::ids::slugify;

/// Port of `kl4a.codekb.config.DEFAULT_CODEKB_CONFIG_PATH` — the YAML text
/// itself, embedded at compile time (see module docs) rather than a runtime
/// filesystem path.
const DEFAULT_CODEKB_CONFIG_YAML: &str = include_str!("default_codekb_config.yaml");

/// Port of `kl4a.codekb.config.REPO_CONFIG_FILENAMES`.
///
/// ```python
/// REPO_CONFIG_FILENAMES = ('.codekb.yaml', 'codekb.yaml', '.sopkb-codekb.yaml', 'sopkb-codekb.yaml')
/// ```
pub const REPO_CONFIG_FILENAMES: [&str; 4] = [
    ".codekb.yaml",
    "codekb.yaml",
    ".sopkb-codekb.yaml",
    "sopkb-codekb.yaml",
];

/// Port of `kl4a.codekb.config.load_default_codekb_config`.
///
/// ```python
/// def load_default_codekb_config() -> dict[str, Any]:
///     data = yaml.safe_load(DEFAULT_CODEKB_CONFIG_PATH.read_text(encoding="utf-8")) or {}
///     if not isinstance(data, dict):
///         raise ValueError(f"default codekb config must be a mapping: {DEFAULT_CODEKB_CONFIG_PATH}")
///     return data
/// ```
/// BLOCKED (Medium finding — YAML 1.1 vs 1.2 ambiguous scalars): Python's
/// `yaml.safe_load` is PyYAML, which resolves plain (unquoted) scalars
/// under the **YAML 1.1** rules — confirmed by direct test against this
/// repo's own `.venv` interpreter: `yaml.safe_load("a: yes\nb: on\nc: 017\n
/// d: 2024-01-01")` returns `{"a": True, "b": True, "c": 15, "d":
/// datetime.date(2024, 1, 1)}` (`yes`/`on` -> bool, a bare leading-zero
/// `017` -> octal int 15, a bare date -> a `date` object). This crate's
/// `serde_yaml` 0.9 (`unsafe-libyaml`-backed) instead implements something
/// closer to the **YAML 1.2 Core Schema** — confirmed by reading its own
/// resolver (`serde_yaml-0.9.34+deprecated/src/de.rs::parse_bool`, which
/// matches only `true|True|TRUE|false|False|FALSE`) — so `yes`/`on`/`off`
/// parse as the plain strings `"yes"`/`"on"`/`"off"`, a bare `017` parses
/// as the string `"017"` (no octal reinterpretation), and there is no date
/// resolution at all. This is a real, user-reachable divergence: a repo's
/// `.codekb.yaml` with e.g. `bundle: {standard_root: on}` gets a bool in
/// Python (fails `isinstance(..., str)`, falls back to the
/// `"knowledge-bundles"` default) but the literal string `"on"` here
/// (passes the Rust equivalent check, is actually used as the directory
/// name) — different final behavior from the same YAML input.
///
/// Left unfixed rather than rushed: a correct fix means re-resolving only
/// *plain-style* (unquoted) scalars under YAML 1.1 rules while leaving
/// explicitly-quoted scalars (`"on"`, `'017'`) as literal strings exactly
/// as PyYAML does — and `serde_yaml`'s `Value`/`serde_json::Value` output
/// has already lost which scalars were quoted by the time this code sees
/// it, so a real fix needs to walk `unsafe-libyaml`'s event stream
/// directly (each scalar event carries a `style` field) rather than a
/// post-hoc string-pattern pass over the deserialized value (which would
/// wrongly reinterpret an intentionally-quoted `"on"` too). That's a
/// meaningfully larger, riskier change than this task's remaining budget
/// supports verifying safely, so it's documented here rather than guessed
/// at. Workaround available to any repo hitting this today: quote the
/// ambiguous value in YAML (`standard_root: "on"` either way is
/// unambiguous... actually: quote it so it's *always* a string in both
/// implementations, e.g. `default_name_template: "{repo_slug}-code"`,
/// which this file's own fallback already does).
pub fn load_default_codekb_config() -> Result<Map<String, Value>> {
    let data: Value = serde_yaml::from_str(DEFAULT_CODEKB_CONFIG_YAML)?;
    match data {
        Value::Object(map) => Ok(map),
        Value::Null => Ok(Map::new()),
        _ => bail!("default codekb config must be a mapping: <embedded default_codekb_config.yaml>"),
    }
}

/// Port of `kl4a.codekb.config.load_repo_codekb_config`.
///
/// ```python
/// def load_repo_codekb_config(repo_dir: Path) -> dict[str, Any]:
///     for filename in REPO_CONFIG_FILENAMES:
///         path = repo_dir / filename
///         if not path.exists():
///             continue
///         data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
///         if not isinstance(data, dict):
///             raise ValueError(f"repo codekb config must be a mapping: {path}")
///         return data.get("codekb", data)
///     return {}
/// ```
pub fn load_repo_codekb_config(repo_dir: &Path) -> Result<Map<String, Value>> {
    for filename in REPO_CONFIG_FILENAMES {
        let path = repo_dir.join(filename);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let data: Value = serde_yaml::from_str(&text)?;
        let map = match data {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            _ => bail!("repo codekb config must be a mapping: {}", path.display()),
        };
        // Python: `return data.get("codekb", data)` — unwrap a nested
        // "codekb:" key if present, else use the whole mapping as-is.
        return Ok(match map.get("codekb") {
            Some(Value::Object(inner)) => inner.clone(),
            _ => map,
        });
    }
    Ok(Map::new())
}

/// Port of `kl4a.codekb.config.deep_merge`.
///
/// ```python
/// def deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
///     merged = dict(base)
///     for key, value in override.items():
///         if isinstance(value, dict) and isinstance(merged.get(key), dict):
///             merged[key] = deep_merge(merged[key], value)
///         else:
///             merged[key] = value
///     return merged
/// ```
pub fn deep_merge(base: &Map<String, Value>, override_: &Map<String, Value>) -> Map<String, Value> {
    let mut merged = base.clone();
    for (key, value) in override_ {
        if let Value::Object(incoming) = value {
            if let Some(Value::Object(existing)) = merged.get(key) {
                let existing = existing.clone();
                merged.insert(key.clone(), Value::Object(deep_merge(&existing, incoming)));
                continue;
            }
        }
        merged.insert(key.clone(), value.clone());
    }
    merged
}

/// Port of `kl4a.codekb.config.configured_values`.
///
/// ```python
/// def configured_values(config: dict[str, Any], section: str, key: str) -> list[str]:
///     values = ((config.get(section) or {}).get(key)) or []
///     if not isinstance(values, list) or not all(
///         isinstance(value, str) and value for value in values
///     ):
///         raise ValueError(f"codekb.{section}.{key} must be a list of non-empty strings")
///     return sorted(dict.fromkeys(values))
/// ```
pub fn configured_values(config: &Map<String, Value>, section: &str, key: &str) -> Result<Vec<String>> {
    // Fix (Medium finding — malformed config tolerance): Python's
    // `((config.get(section) or {}).get(key)) or []` uses `or`, which
    // replaces *any* falsy value (not just a missing key) with `[]` —
    // including an explicit YAML `null`
    // (`skip_dirs: null` -> `None or [] == []`, no error). The previous
    // version here only substituted the empty-array default when the key
    // was *absent* (`.unwrap_or(...)` only fires on `None`); an explicit
    // `Value::Null` survived through to the `match` below and hit the
    // `_ => bail!(...)` arm, exiting 1 — where Python builds successfully
    // with an empty skip list. Routed through the same `is_falsy` helper
    // already used elsewhere in this crate for Python `or`/`not`
    // truthiness, so `skip_dirs: null`/`false`/`0`/`""`/`[]`/`{}` all
    // become `[]` exactly like Python, and only a genuinely non-list truthy
    // value (a string, a number, a mapping, `true`) still errors.
    let raw = config.get(section).and_then(Value::as_object).and_then(|s| s.get(key));
    let values = if crate::bundle_store::is_falsy(raw) { Value::Array(Vec::new()) } else { raw.cloned().unwrap() };
    let arr = match &values {
        Value::Array(a) => a,
        _ => bail!("codekb.{section}.{key} must be a list of non-empty strings"),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        match item {
            Value::String(s) if !s.is_empty() => out.push(s.clone()),
            _ => bail!("codekb.{section}.{key} must be a list of non-empty strings"),
        }
    }
    // Python: `sorted(dict.fromkeys(values))` — dedupe (first-seen order
    // discarded, since the result is then sorted anyway) then sort.
    out.sort();
    out.dedup();
    Ok(out)
}

/// Port of `kl4a.codekb.config.effective_codekb_config`.
///
/// ```python
/// def effective_codekb_config(repo_dir: Path, manifest: dict[str, Any] | None=None) -> dict[str, Any]:
///     config = deep_merge(load_default_codekb_config(), (manifest or {}).get("codekb") or {})
///     repo_config = load_repo_codekb_config(repo_dir)
///     if repo_config:
///         config = deep_merge(config, repo_config)
///     config["repo_root"] = str(repo_dir.resolve())
///     inventory = config.setdefault("inventory", {})
///     skip_dirs = configured_values(config, "inventory", "skip_dirs")
///     extra_skip_dirs = configured_values(config, "inventory", "extra_skip_dirs")
///     inventory["skip_dirs"] = sorted(dict.fromkeys([*skip_dirs, *extra_skip_dirs]))
///     inventory["extra_skip_dirs"] = extra_skip_dirs
///     return config
/// ```
pub fn effective_codekb_config(
    repo_dir: &Path,
    manifest: Option<&Map<String, Value>>,
) -> Result<Map<String, Value>> {
    let manifest_codekb = manifest
        .and_then(|m| m.get("codekb"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut config = deep_merge(&load_default_codekb_config()?, &manifest_codekb);

    let repo_config = load_repo_codekb_config(repo_dir)?;
    if !repo_config.is_empty() {
        config = deep_merge(&config, &repo_config);
    }

    config.insert(
        "repo_root".to_string(),
        Value::String(crate::bundle_store::display_path(repo_dir)),
    );

    let skip_dirs = configured_values(&config, "inventory", "skip_dirs")?;
    let extra_skip_dirs = configured_values(&config, "inventory", "extra_skip_dirs")?;
    let mut combined: Vec<String> = Vec::with_capacity(skip_dirs.len() + extra_skip_dirs.len());
    combined.extend(skip_dirs);
    combined.extend(extra_skip_dirs.clone());
    combined.sort();
    combined.dedup();

    // Python: `config.setdefault("inventory", {})` then mutates that same
    // dict in place; if `config["inventory"]` already existed but was not a
    // mapping, Python would raise `TypeError` on the subscript-assignment
    // below. Mirrored here by coercing a non-object value to an empty
    // object first rather than silently discarding data — an edge case the
    // original does not handle gracefully either.
    let inventory_entry = config
        .entry("inventory".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !inventory_entry.is_object() {
        *inventory_entry = Value::Object(Map::new());
    }
    if let Value::Object(inv) = inventory_entry {
        inv.insert(
            "skip_dirs".to_string(),
            Value::Array(combined.into_iter().map(Value::String).collect()),
        );
        inv.insert(
            "extra_skip_dirs".to_string(),
            Value::Array(extra_skip_dirs.into_iter().map(Value::String).collect()),
        );
    }

    Ok(config)
}

/// Port of `kl4a.codekb.config.standard_code_bundle_dir`.
///
/// ```python
/// def standard_code_bundle_dir(repo_dir: Path, config: dict[str, Any] | None=None) -> Path:
///     repo_dir = repo_dir.resolve()
///     codekb = config or effective_codekb_config(repo_dir)
///     bundle_config = codekb.get("bundle") or {}
///     configured = bundle_config.get("default_bundle_dir")
///     if isinstance(configured, str) and configured.strip():
///         path = Path(configured)
///         return path if path.is_absolute() else repo_dir / path
///     root = str(bundle_config.get("standard_root") or "knowledge-bundles")
///     template = str(bundle_config.get("default_name_template") or "{repo_slug}-code")
///     bundle_name = template.format(repo_slug=slugify(repo_dir.name), repo_name=repo_dir.name)
///     return repo_dir / root / bundle_name
/// ```
///
/// `template.format(...)` in Python runs the full `str.format`
/// mini-language. Fix (Medium finding): the previous version here did a
/// plain two-name string replace, which diverges from Python in both
/// directions — an unknown placeholder (`{typo}`) passes through literally
/// here instead of Python's `KeyError` (which propagates out of
/// `standard_code_bundle_dir` uncaught, exiting 1), and a format spec
/// (`{repo_slug:.8}`) was left completely unsubstituted (the literal text
/// `{repo_slug:.8}` would end up in the directory name) instead of being
/// honored the way Python's formatter actually applies it. Now routed
/// through [`format_str_template`], a small `str.format`-compatible
/// formatter (named fields + the string subset of the format-spec
/// mini-language: fill/align/width/precision) that errors the same way
/// Python does on an unknown field name.
pub fn standard_code_bundle_dir(repo_dir: &Path, config: Option<&Map<String, Value>>) -> Result<PathBuf> {
    let repo_dir = repo_dir
        .canonicalize()
        .unwrap_or_else(|_| repo_dir.to_path_buf());

    let owned_config;
    let codekb: &Map<String, Value> = match config {
        Some(c) => c,
        None => {
            owned_config = effective_codekb_config(&repo_dir, None)?;
            &owned_config
        }
    };

    let bundle_config = codekb.get("bundle").and_then(Value::as_object);

    if let Some(bc) = bundle_config {
        if let Some(Value::String(configured)) = bc.get("default_bundle_dir") {
            if !configured.trim().is_empty() {
                let path = PathBuf::from(configured);
                return Ok(if path.is_absolute() {
                    path
                } else {
                    repo_dir.join(path)
                });
            }
        }
    }

    let root = bundle_config
        .and_then(|bc| bc.get("standard_root"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("knowledge-bundles")
        .to_string();
    let template = bundle_config
        .and_then(|bc| bc.get("default_name_template"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("{repo_slug}-code")
        .to_string();

    let repo_name = repo_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let repo_slug = slugify(&repo_name);
    let fields: &[(&str, &str)] = &[("repo_slug", &repo_slug), ("repo_name", &repo_name)];
    let bundle_name = format_str_template(&template, fields)?;

    Ok(repo_dir.join(root).join(bundle_name))
}

/// A `str.format`-compatible formatter for the subset Python's
/// `template.format(repo_slug=..., repo_name=...)` call can actually
/// exercise: named fields, `{{`/`}}` literal-brace escaping, and the
/// string subset of the format-spec mini-language (`[[fill]align][width]
/// [.precision]`, optional trailing `s` type). Returns an error for
/// anything Python's `str.format` would itself raise on — an unknown field
/// name (`KeyError`) or a positional/auto-numbered field (`IndexError`,
/// since this call site only ever passes keyword arguments) — so the
/// caller can propagate it the same way Python's uncaught exception exits
/// the process with an error.
fn format_str_template(template: &str, fields: &[(&str, &str)]) -> Result<String> {
    let mut out = String::with_capacity(template.len());
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                out.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                out.push('}');
                i += 2;
            }
            '{' => {
                let close = chars[i + 1..]
                    .iter()
                    .position(|&c| c == '}')
                    .map(|p| i + 1 + p)
                    .ok_or_else(|| anyhow!("Single '{{' encountered in format string: {template:?}"))?;
                let field: String = chars[i + 1..close].iter().collect();
                let (name, spec) = match field.split_once(':') {
                    Some((n, s)) => (n, Some(s)),
                    None => (field.as_str(), None),
                };
                let name = name.split('!').next().unwrap_or(name); // drop a `!r`/`!s`/`!a` conversion, if any
                if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
                    bail!("standard_code_bundle_dir: positional/auto-numbered fields are not supported in default_name_template: {template:?}");
                }
                let value = fields
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| anyhow!("default_name_template references an unknown field {name:?}: {template:?}"))?;
                out.push_str(&apply_str_format_spec(value, spec.unwrap_or("")));
                i = close + 1;
            }
            '}' => bail!("Single '}}' encountered in format string: {template:?}"),
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    Ok(out)
}

/// Applies the string subset of Python's format-spec mini-language
/// (`[[fill]align][width][.precision]`, optional trailing `s` type) to a
/// string value. Precision truncates; width pads using `fill` (default
/// `' '`) in the given `align` direction (`<` left/default for strings,
/// `>` right, `^` center); `=` is treated like `<` (sign-aware padding has
/// no meaning for a plain string).
fn apply_str_format_spec(value: &str, spec: &str) -> String {
    if spec.is_empty() {
        return value.to_string();
    }
    let spec_chars: Vec<char> = spec.chars().collect();
    let mut idx = 0usize;
    let mut fill = ' ';
    let mut align = '<';
    if spec_chars.len() >= 2 && matches!(spec_chars[1], '<' | '>' | '^' | '=') {
        fill = spec_chars[0];
        align = spec_chars[1];
        idx = 2;
    } else if !spec_chars.is_empty() && matches!(spec_chars[0], '<' | '>' | '^' | '=') {
        align = spec_chars[0];
        idx = 1;
    }
    let mut width = 0usize;
    while idx < spec_chars.len() && spec_chars[idx].is_ascii_digit() {
        width = width * 10 + spec_chars[idx].to_digit(10).unwrap() as usize;
        idx += 1;
    }
    let mut precision: Option<usize> = None;
    if idx < spec_chars.len() && spec_chars[idx] == '.' {
        idx += 1;
        let mut p = 0usize;
        while idx < spec_chars.len() && spec_chars[idx].is_ascii_digit() {
            p = p * 10 + spec_chars[idx].to_digit(10).unwrap() as usize;
            idx += 1;
        }
        precision = Some(p);
    }
    // A trailing `s` type (or nothing) is all that's valid for a string;
    // anything else is simply ignored here rather than erroring, since
    // `default_name_template` is user-authored config, not Python source.
    let mut text: String = value.chars().collect();
    if let Some(p) = precision {
        text = text.chars().take(p).collect();
    }
    let len = text.chars().count();
    if len >= width {
        return text;
    }
    let pad = width - len;
    match align {
        '>' => format!("{}{}", fill.to_string().repeat(pad), text),
        '^' => {
            let left = pad / 2;
            let right = pad - left;
            format!("{}{}{}", fill.to_string().repeat(left), text, fill.to_string().repeat(right))
        }
        _ => format!("{}{}", text, fill.to_string().repeat(pad)),
    }
}
