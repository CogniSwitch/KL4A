//! Port of the handful of `kl4a/kl4a/bundle_store.py` symbols that
//! `codekb`/`apikb` call directly across the workspace: `load_manifest`,
//! `utc_now`, `relative_to_bundle`, plus the `is_falsy` helper the earlier
//! `kl4a_shared.rs` vendoring added for Python truthiness on JSON values.
//!
//! Grounded via `tools-code` MCP `code_symbols_get` against
//! `kl4a/kl4a/bundle_store.py` (`source-kl4a-kl4a-bundle-store-py`).

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// Port of `kl4a.kl4a.bundle_store.utc_now` (`kl4a/kl4a/bundle_store.py:60-61`).
///
/// ```python
/// def utc_now() -> str:
///     return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")
/// ```
///
/// Rust has no `std` wall-clock-to-ISO8601 formatter and no `chrono`/`time`
/// crate is in the approved dependency list, so this hand-formats UTC time
/// from `SystemTime`. UNCONFIRMED: leap seconds and pre-1970 times are not
/// handled (Python's `datetime` wouldn't hit those in practice here either).
pub fn utc_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format_unix_utc(now.as_secs() as i64)
}

/// Same `...replace(microsecond=0).isoformat().replace("+00:00", "Z")`
/// formatting as [`utc_now`], for an arbitrary Unix timestamp (seconds).
pub fn format_unix_utc(unix_secs: i64) -> String {
    let (year, month, day, hour, minute, second) = civil_from_unix(unix_secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Civil (Gregorian) date/time from a Unix timestamp, UTC. Howard Hinnant's
/// well-known `civil_from_days` algorithm, adapted for seconds-of-day too.
fn civil_from_unix(unix_secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let hour = (secs_of_day / 3600) as u32;
    let minute = ((secs_of_day % 3600) / 60) as u32;
    let second = (secs_of_day % 60) as u32;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hour, minute, second)
}

/// Port of `kl4a.kl4a.bundle_store.relative_to_bundle`
/// (`kl4a/kl4a/bundle_store.py:251-252`).
///
/// ```python
/// def relative_to_bundle(bundle_dir: Path, path: Path) -> str:
///     return path.relative_to(bundle_dir).as_posix()
/// ```
pub fn relative_to_bundle(bundle_dir: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(bundle_dir)
        .with_context(|| format!("{} is not inside {}", path.display(), bundle_dir.display()))?;
    Ok(relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

/// Port of `kl4a.kl4a.bundle_store.load_manifest` (`kl4a/kl4a/bundle_store.py:89-96`).
///
/// ```python
/// def load_manifest(bundle_dir: Path) -> dict[str, Any]:
///     manifest_path = bundle_dir / "manifest.yaml"
///     if not manifest_path.exists():
///         raise FileNotFoundError(f"Missing manifest: {manifest_path}")
///     data = yaml.safe_load(manifest_path.read_text(encoding="utf-8")) or {}
///     if not isinstance(data, dict):
///         raise ValueError("manifest.yaml must contain a mapping")
///     return data
/// ```
pub fn load_manifest(bundle_dir: &Path) -> Result<Value> {
    let manifest_path = bundle_dir.join("manifest.yaml");
    if !manifest_path.exists() {
        bail!("Missing manifest: {}", manifest_path.display());
    }
    let text = fs::read_to_string(&manifest_path)?;
    // Bug fix: Python's `yaml.safe_load(...)` only maps a *successful* parse
    // of an empty/`null` document to `{}` via `... or {}`; a genuine syntax
    // error raises `yaml.YAMLError` uncaught, propagating the real parser
    // diagnostic to the caller (grounded via `tools-code` MCP against
    // `kl4a.kl4a.bundle_store.load_manifest`, `kl4a/kl4a/bundle_store.py:89-96`,
    // and confirmed the error reaches `codekb.bundle.validate_code_profile`
    // as the single reported validation error). The previous
    // `.unwrap_or(Value::Null)` here swallowed a parse error and silently
    // treated a corrupted manifest.yaml as an empty-but-valid one instead,
    // masking the real diagnostic behind unrelated generic "missing field"
    // errors downstream. Only a successful parse that yields `null` (an
    // empty file, or the literal `null`) still defaults to `{}`.
    // `bail!`, not `.with_context()`: `anyhow::Error::to_string()` (what
    // `codekb::bundle::validate_code_profile` calls to turn this into the
    // single reported validation error) only prints the *outermost*
    // context message, not the wrapped source -- context alone would show
    // "failed to parse manifest.yaml" without the real parser diagnostic.
    // Embedding `err` (serde_yaml's `Display`, e.g. "mapping values are not
    // allowed here at line 2 column 15") directly in the message text is
    // what actually reproduces Python's `f"{yaml.YAMLError}"` behavior.
    let data: Value = match serde_yaml::from_str(&text) {
        Ok(data) => data,
        Err(err) => bail!("failed to parse {}: {}", manifest_path.display(), err),
    };
    let data = if data.is_null() { Value::Object(Default::default()) } else { data };
    if !data.is_object() {
        bail!("manifest.yaml must contain a mapping");
    }
    Ok(data)
}

/// Canonicalizes `p` (so any actual filesystem operation on the result gets
/// Windows' long-path support — see [`load_manifest`]'s callers in
/// `codekb::inventory::scan_code_repo` and `codekb::state`/`apikb::state`'s
/// atomic writers) but strips the `\\?\` / `\\?\UNC\` extended-length-path
/// prefix that `std::fs::canonicalize()` adds on Windows before turning it
/// into a display/storage `String`.
///
/// Python never produces this prefix (`Path.resolve()` has no such concept),
/// so anywhere a canonicalized path is serialized into `manifest.yaml`, JSON
/// state, rendered docs, or printed to the user, it must go through this
/// helper instead of a bare `.to_string_lossy()` on the canonical `PathBuf`,
/// or Windows users see a leaked `\\?\C:\Users\...` instead of the plain
/// `C:\Users\...` Python shows. On non-Windows platforms `canonicalize()`
/// never adds this prefix, so this is a no-op beyond canonicalizing.
///
/// IMPORTANT: only use this for the *string* that gets displayed/stored.
/// Keep using the real (possibly `\\?\`-prefixed) `PathBuf` internally for
/// any actual filesystem call, since that prefix is exactly what makes
/// Windows' long-path support work.
pub fn display_path(p: &Path) -> String {
    let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let text = canonical.to_string_lossy();
    if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{stripped}")
    } else if let Some(stripped) = text.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        text.into_owned()
    }
}

/// Python truthiness for a JSON-ish [`Value`], used everywhere the original
/// relies on `if not x:` / `x or default` over a `dict.get(...)` result:
/// `null`/missing, `false`, `0`/`0.0`, `""`, `[]` and `{}` are all falsy;
/// everything else (including a non-empty string, non-zero number, or a
/// non-empty array/object) is truthy. Not itself a ported symbol — a shared
/// helper for call sites across the workspace that reproduce Python's
/// `or`/`not` short-circuiting on dict values.
pub fn is_falsy(value: Option<&Value>) -> bool {
    match value {
        None => true,
        Some(Value::Null) => true,
        Some(Value::Bool(b)) => !*b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f == 0.0).unwrap_or(false),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
    }
}
