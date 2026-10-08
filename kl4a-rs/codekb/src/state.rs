//! Port of `kl4a/codekb/state.py`.
//!
//! Atomic, torn-write-tolerant JSON/YAML state helpers for a Code Knowledge
//! Bundle's `.codekb/` directory. Ported symbol-for-symbol from the Python
//! source (verified via tools-code MCP `code_symbols_get`); the sibling
//! `kl4a/apikb/state.py` is the byte-for-byte-identical counterpart already
//! ported at `kl4a-rs/apikb/src/state.rs` — this file mirrors that one.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value;

/// `kl4a.codekb.state.STATE_DIR` — the bundle-relative directory that holds
/// this module's state documents.
pub const STATE_DIR: &str = ".codekb";

/// Port of `kl4a.codekb.state.write_text_atomic`.
///
/// Replaces a file's contents in one step, so a reader never sees a torn
/// write. On Windows the replace fails with a permission error while another
/// process has the destination open for reading, which a polling page does
/// routinely. The retry rides out that contention; the write itself stays
/// atomic.
///
/// `attempts` (default 20) and `delay` (default 20ms) mirror the Python
/// defaults (`attempts: int = 20, delay: float = 0.02`).
pub fn write_text_atomic(path: &Path, payload: &str) -> Result<()> {
    write_text_atomic_with(path, payload, 20, Duration::from_millis(20))
}

/// Same as [`write_text_atomic`] but with explicit `attempts`/`delay`,
/// matching the Python function's keyword-only overrides.
pub fn write_text_atomic_with(
    path: &Path,
    payload: &str,
    attempts: u32,
    delay: Duration,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Bug fix: canonicalize the parent directory (it now exists, having
    // just been created above, so this cannot fail for *that* reason) and
    // rejoin the file name onto it before doing any raw `fs::write`/
    // `fs::rename` below. On Windows, `std::fs::canonicalize()` is what
    // opts a path into the `\\?\`-prefixed extended-length form that lifts
    // the ~260-char MAX_PATH limit; without it, a bundle nested deep enough
    // (common under a test runner's temp dir, or a long repo/bundle name)
    // hits MAX_PATH on this write even when the caller's own path
    // construction was otherwise correct. Python has no such limit
    // (`pathlib`/the OS layer handles long paths transparently), so this
    // makes the atomic writer long-path-safe regardless of whether the
    // caller already canonicalized `bundle_dir` itself. Falls back to the
    // original `path` if canonicalization fails for any other reason (e.g.
    // a relative path with no real parent component).
    let path_buf;
    let path: &Path = match path.parent().filter(|p| !p.as_os_str().is_empty()) {
        Some(parent) => match parent.canonicalize() {
            Ok(canonical_parent) => {
                path_buf = canonical_parent.join(
                    path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("")),
                );
                &path_buf
            }
            Err(_) => path,
        },
        None => path,
    };

    // Python: f"{path.name}.{os.getpid()}.{threading.get_ident()}.tmp"
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let temp_name = format!(
        "{}.{}.{:?}.tmp",
        file_name,
        std::process::id(),
        thread::current().id()
    );
    let temp_path: PathBuf = path.with_file_name(temp_name);

    let write_result = (|| -> Result<()> {
        fs::write(&temp_path, payload.as_bytes())?;
        let mut last_err = None;
        for attempt in 0..attempts {
            match fs::rename(&temp_path, path) {
                Ok(()) => return Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    last_err = Some(err);
                    if attempt + 1 == attempts {
                        break;
                    }
                    thread::sleep(delay);
                }
                Err(err) => return Err(err.into()),
            }
        }
        Err(last_err.unwrap().into())
    })();

    // Python's `finally: if temp_path.exists(): temp_path.unlink()`.
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

/// Port of `kl4a.codekb.state.read_json`.
///
/// Tolerates a torn or empty file rather than raising. Belt and braces
/// alongside the atomic write: a file left empty by an older build, or by a
/// crash mid-write, should read as "no state yet" instead of taking down the
/// page that polls it.
pub fn read_json(path: &Path, default: Value) -> Value {
    if !path.exists() {
        return default;
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => return default,
    };
    if text.trim().is_empty() {
        return default;
    }
    serde_json::from_str(&text).unwrap_or(default)
}

/// Port of `kl4a.codekb.state.write_json`.
///
/// Atomic counterpart to `kl4a.bundle_store.write_json`.
///
/// NOTE: Python serializes with `json.dumps(data, indent=2, sort_keys=True)`.
/// `serde_json::Value`'s object map is a `BTreeMap` (this workspace does not
/// enable serde_json's `preserve_order` feature), so keys are already emitted
/// in sorted order — `to_string_pretty` reproduces `sort_keys=True` for free.
pub fn write_json(path: &Path, data: &Value) -> Result<()> {
    let payload = format!("{}\n", serde_json::to_string_pretty(data)?);
    write_text_atomic(path, &payload)
}

/// Port of `kl4a.codekb.state.save_manifest`.
///
/// Atomic counterpart to `kl4a.bundle_store.save_manifest`. The bytes are
/// meant to be identical to the core's (same dumper, same options, so key
/// order and formatting are unchanged) — only the way they reach disk
/// differs: a reader sees either the whole old manifest or the whole new
/// one.
///
/// NOTE: Python dumps with `yaml.safe_dump(manifest, sort_keys=False,
/// allow_unicode=False)`. `serde_yaml::to_string` preserves map insertion
/// order (matching `sort_keys=False`), but has no `allow_unicode=False`
/// equivalent — non-ASCII scalars are emitted as literal UTF-8 rather than
/// escaped. UNCONFIRMED / not bit-for-bit: this is a known divergence, not an
/// invented default (same divergence already accepted in the apikb port).
pub fn save_manifest(bundle_dir: &Path, manifest: &Value) -> Result<()> {
    let payload = serde_yaml::to_string(manifest)?;
    let payload = quote_manifest_timestamps(&payload);
    write_text_atomic(&bundle_dir.join("manifest.yaml"), &payload)
}

/// Bug fix: PyYAML's `yaml.safe_dump` recognizes ISO-8601-looking scalars
/// via its implicit timestamp resolver and force-quotes them so they stay
/// strings on the next `safe_load` (that's why Python's `manifest.yaml` has
/// `created_at: '2026-09-28T12:13:22Z'`, quoted). `serde_yaml` has no such
/// rule -- an unquoted `created_at: 2026-09-28T12:13:23Z` round-trips
/// through a standard YAML parser (including `yaml.safe_load`, per an
/// inline check) as a native `datetime`, a real type-level divergence from
/// Python's `str` for any downstream consumer of this bundle. `serde_yaml`
/// doesn't expose per-scalar quote-style control on a `Value` (there is no
/// tagged "force string style" wrapper in this dependency version), so this
/// is a small manual text pass over the already-serialized YAML rather than
/// a serializer-level fix -- more robust than hand-rolling a `serde_yaml`
/// emitter override, and scoped to exactly the two fields known to need it.
/// Only `created_at`/`updated_at` are touched; every other manifest field
/// keeps whatever style `serde_yaml` chose.
fn quote_manifest_timestamps(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + 8);
    for (index, line) in yaml.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&quote_timestamp_line(line));
    }
    out
}

/// Quotes the scalar value of a `created_at:`/`updated_at:` line (at any
/// indentation) unless it is already quoted or is an explicit YAML null
/// (`null`/`~`/empty), in which case it is left alone.
fn quote_timestamp_line(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, content) = line.split_at(indent_len);
    for key in ["created_at", "updated_at"] {
        let prefix = format!("{key}: ");
        if let Some(rest) = content.strip_prefix(prefix.as_str()) {
            let value = rest.trim_end_matches('\r');
            let already_quoted = value.starts_with('\'') || value.starts_with('"');
            if !already_quoted && !value.is_empty() && value != "null" && value != "~" {
                let escaped = value.replace('\'', "''");
                let trailer = &rest[value.len()..]; // preserves a trailing \r, if any
                return format!("{indent}{key}: '{escaped}'{trailer}");
            }
        }
    }
    line.to_string()
}

/// Mirrors `kl4a.codekb.bundle.write_code_state`, which is a one-line
/// `write_json(bundle_dir / STATE_DIR / filename, data)` wrapper living in
/// `kl4a/codekb/bundle.py` (not in this batch's scope). Reproduced here,
/// inline, because `run_state.write_run_state` depends on exactly this
/// behavior; if `bundle.py`'s batch also ports `write_code_state`, the
/// coordinator should reconcile to a single definition (this one, or a
/// `crate::bundle` re-export of it — the bytes are identical either way).
pub fn write_code_state(bundle_dir: &Path, filename: &str, data: &Value) -> Result<()> {
    write_json(&bundle_dir.join(STATE_DIR).join(filename), data)
}
