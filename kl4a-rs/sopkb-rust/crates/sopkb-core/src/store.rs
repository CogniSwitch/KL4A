//! Port of `tools/sopkb/sopkb/bundle_store.py`. See docs/port/port-mapping-a-core-data.md §3.3.

use crate::error::{Result, SopkbError};
use crate::models::Manifest;
use chrono::{SecondsFormat, Utc};
use sopkb_fmt::{emit_yaml, parse_yaml, py_title_case, OrderedMap, YamlValue};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Created by `create_bundle` in this order; checked by `validate_bundle` in this
/// order too, so error ordering is deterministic. 16 entries; `sources`/`.sopkb`
/// appear both bare and as parents of children -- harmless, `mkdir -p` dedupes it.
pub const REQUIRED_DIRS: &[&str] = &[
    "sources",
    "sources/originals",
    "sources/normalized",
    "sections",
    "concepts",
    "knowledge",
    "relations",
    "rules",
    "evidence",
    "tasks",
    "references",
    "authored",
    "reports",
    ".sopkb",
    ".sopkb/uploads",
    ".sopkb/cache",
];

/// `(bare state filename, legacy bundle-relative path)`, in this exact insertion
/// order (both `migrate_legacy_state`'s iteration order and, incidentally, the
/// table order in the port-mapping doc). The two `source_*` entries came in with the
/// source-versioning subsystem (CATCHUP_PLAN.md workstream 2) and sit between
/// `reviews.json` and `document_contexts.json`, matching the reference dict's
/// insertion order -- which is observable, since `migrate_legacy_state` iterates it.
pub const STATE_FILE_ALIASES: &[(&str, &str)] = &[
    ("inventory.json", "sources/inventory.json"),
    ("sections.json", "knowledge/sections.json"),
    ("items.json", "knowledge/items.json"),
    ("entities.json", "knowledge/entities.json"),
    ("triples.json", "knowledge/triples.json"),
    ("reviews.json", "knowledge/reviews.json"),
    ("source_versions.json", "sources/source_versions.json"),
    ("source_events.json", "sources/source_events.json"),
    ("document_contexts.json", "knowledge/document_contexts.json"),
    ("llm_authoring.json", "knowledge/llm_authoring.json"),
    ("agent_chat.json", "reports/agent_chat.json"),
];

pub const REQUIRED_MANIFEST_FIELDS: &[&str] =
    &["id", "version", "title", "profile", "profile_version", "status", "created_at", "updated_at", "sources", "exports"];

fn legacy_relative_for(filename: &str) -> Option<&'static str> {
    STATE_FILE_ALIASES.iter().find(|(name, _)| *name == filename).map(|(_, legacy)| *legacy)
}

/// Writes `text` the way Python's `Path.write_text(text, encoding="utf-8")` does on
/// this platform: on Windows, default text-mode naively replaces every `\n` with
/// `\r\n` (confirmed against the Phase 0 fixture corpus -- every real JSON/YAML/
/// Markdown file the reference bundle produces on Windows is CRLF-terminated). `text`
/// must not already contain `\r`, or this would double it -- matching Python's own
/// behavior in that situation (see the `_transcript.txt` double-CRLF bug found while
/// building the harness).
pub fn write_text_native(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let out = if cfg!(windows) { text.replace('\n', "\r\n") } else { text.to_string() };
    write_atomic(path, out.as_bytes())
}

/// Process-wide counter appended to the temp-file name below, so two writes to the
/// same `path` from different threads in the same process never race on the same
/// temp name (a bare pid isn't enough for that case).
static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to `path` atomically: write to a sibling temp file in the same
/// directory (so the final rename is same-filesystem, never cross-device), fsync it,
/// then rename over the destination. A crash, power loss, or forced-quit mid-write
/// leaves either the old content or the new content on disk, never a truncated file
/// -- unlike a direct `fs::write`, which truncates-then-writes and can leave a
/// half-written `.sopkb/*.json` if interrupted. Every multi-file bundle mutation in
/// `sopkb-workbench` goes through `write_text_native`/`write_json`/`write_state_json`,
/// which all bottom out here, so fixing it once fixes all of them (code review
/// finding, 2026-08-21: this was the single largest correctness gap in the Rust core).
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        SopkbError::Value(format!("write_atomic: path has no parent: {}", path.display()))
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        SopkbError::Value(format!("write_atomic: path has no file name: {}", path.display()))
    })?;
    let n = ATOMIC_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".{}.tmp.{}.{}", file_name.to_string_lossy(), std::process::id(), n);
    let tmp_path = parent.join(tmp_name);

    let write_result = (|| -> Result<()> {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        // Best-effort cleanup; the write error itself is what the caller needs to see.
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

/// Reads `path` as UTF-8 text with universal-newline translation (`\r\n`/`\r`/`\n` all
/// normalized to `\n`), matching Python's default text-mode `Path.read_text()`.
/// Required for any function that returns raw file content VERBATIM (`agent_guide`,
/// `conflicts_list`, `freshness_check`, and similarly-shaped future OKF-doc readers) --
/// JSON/YAML parsing already tolerates embedded `\r\n` transparently (`str::lines()`
/// and `serde_json::from_str` both do), so `read_json`/`load_manifest` don't need this,
/// but a function handing back the literal string does, or a Rust build on Windows
/// returns CRLF where Python's equivalent returns LF (found via the Phase 4 real-bundle
/// differential test against `conflicts_list`/`freshness_check`/`agent_guide`).
pub fn read_text_universal_newlines(path: &Path) -> Result<String> {
    let raw = fs::read_to_string(path)?;
    Ok(normalize_newlines(&raw))
}

fn normalize_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// `now.replace(microsecond=0).isoformat().replace("+00:00", "Z")` -- always exactly
/// `YYYY-MM-DDTHH:MM:SSZ` (19 chars + `Z`).
pub fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// `datetime.now(UTC).date().isoformat()` -- always exactly `YYYY-MM-DD`. Used by
/// `okf_metadata`'s `generated.date` and `write_log`'s dated heading (port-mapping-c
/// §0.4, §3.6, §3.9).
pub fn today() -> String {
    Utc::now().date_naive().to_string()
}

/// mkdir the bundle dir and all 16 `REQUIRED_DIRS` (in order); if `manifest.yaml`
/// already exists this is an IDEMPOTENT re-init that does NOT touch the manifest and
/// returns the raw loaded dict (not a fresh `Manifest`) -- see G-A1 in the port
/// mapping doc. Otherwise builds and writes a fresh `Manifest`, returning it.
/// Called once per `create_bundle` invocation (not a hot path or a bulk-stored type),
/// so `Manifest`'s ~264 bytes doesn't justify boxing it just to shrink the other variant.
#[allow(clippy::large_enum_variant)]
pub enum CreateBundleResult {
    Existing(OrderedMap<YamlValue>),
    Created(Manifest),
}

pub fn create_bundle(bundle_dir: &Path, title: Option<&str>) -> Result<CreateBundleResult> {
    fs::create_dir_all(bundle_dir)?;
    for relative in REQUIRED_DIRS {
        fs::create_dir_all(bundle_dir.join(relative))?;
    }

    let manifest_path = bundle_dir.join("manifest.yaml");
    if manifest_path.exists() {
        return Ok(CreateBundleResult::Existing(load_manifest(bundle_dir)?));
    }

    let bundle_title = match title {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            let dir_name = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let spaced = dir_name.replace(['-', '_'], " ");
            py_title_case(&spaced)
        }
    };

    let timestamp = utc_now();
    let dir_name = bundle_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let manifest = Manifest {
        id: sopkb_fmt_slugify_bridge(dir_name),
        version: "0.1.0".to_string(),
        title: bundle_title,
        profile: "sop-knowledge-bundle".to_string(),
        profile_version: "0.2.0".to_string(),
        status: "draft".to_string(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
        sources: Vec::new(),
        exports: Vec::new(),
        okf_version: "0.2".to_string(),
    };
    save_manifest_value(bundle_dir, &manifest.to_yaml_value())?;
    Ok(CreateBundleResult::Created(manifest))
}

// `ids::slugify` lives in this crate, not sopkb-fmt; small local bridge so store.rs
// doesn't need a direct `crate::ids` import cycle concern -- kept as a thin wrapper
// for readability at the call site above.
fn sopkb_fmt_slugify_bridge(name: &str) -> String {
    crate::ids::slugify(name)
}

/// Raises the exact `"Missing manifest: <path>"` / `"manifest.yaml must contain a
/// mapping"` messages `validate_bundle` surfaces verbatim. Returns the RAW loaded
/// mapping -- no schema check, no defaults injected (Gotcha G-A14: nothing
/// downstream ever wants a strictly-typed `Manifest` back from disk).
pub fn load_manifest(bundle_dir: &Path) -> Result<OrderedMap<YamlValue>> {
    let manifest_path = bundle_dir.join("manifest.yaml");
    if !manifest_path.exists() {
        return Err(SopkbError::NotFound(format!("Missing manifest: {}", manifest_path.display())));
    }
    let text = fs::read_to_string(&manifest_path)?;
    if text.trim().is_empty() {
        return Ok(OrderedMap::new());
    }
    let value = parse_yaml(&text).map_err(SopkbError::Parse)?;
    match value {
        YamlValue::Mapping(map) => Ok(map),
        _ => Err(SopkbError::Value("manifest.yaml must contain a mapping".to_string())),
    }
}

fn save_manifest_value(bundle_dir: &Path, value: &YamlValue) -> Result<()> {
    let text = emit_yaml(value);
    write_text_native(&bundle_dir.join("manifest.yaml"), &text)
}

/// Writes an already-mutated raw manifest dict back out (the `scan`/`export`-command
/// path: load, mutate a couple of keys, save -- never round-trips through a typed
/// `Manifest`).
pub fn save_manifest_raw(bundle_dir: &Path, manifest: &OrderedMap<YamlValue>) -> Result<()> {
    save_manifest_value(bundle_dir, &YamlValue::Mapping(manifest.clone()))
}

/// `default` is returned as-is (no I/O) when `path` doesn't exist. A malformed-JSON
/// file's parse error PROPAGATES -- never silently swallowed, never falls back to
/// `default`, matching Python's `json.loads`.
pub fn read_json(path: &Path, default: serde_json::Value) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(default);
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|e| SopkbError::Parse(e.to_string()))
}

/// `json.dumps(data, indent=2, sort_keys=True, ensure_ascii=True) + "\n"`, mkdir -p'ing
/// the parent first.
pub fn write_json(path: &Path, data: &serde_json::Value) -> Result<()> {
    let mut text = sopkb_fmt::to_canonical_json(data);
    text.push('\n');
    write_text_native(path, &text)
}

/// `filename` may be a nested relative path or a directory name (callers use both a
/// bare state filename and things like `"uploads"`/`"authored_okf"`). No sanitization.
pub fn state_path(bundle_dir: &Path, filename: &str) -> PathBuf {
    bundle_dir.join(".sopkb").join(filename)
}

/// Canonical file always wins if present (even over a newer legacy file); otherwise
/// falls back to the legacy alias path IF `filename` is a known alias, else returns
/// `default` with no I/O at all. A CORRUPT canonical file raises rather than falling
/// back to legacy.
pub fn read_state_json(bundle_dir: &Path, filename: &str, default: serde_json::Value) -> Result<serde_json::Value> {
    let canonical = state_path(bundle_dir, filename);
    if canonical.exists() {
        return read_json(&canonical, default);
    }
    match legacy_relative_for(filename) {
        Some(legacy_relative) => read_json(&bundle_dir.join(legacy_relative), default),
        None => Ok(default),
    }
}

/// ALWAYS writes canonical (`.sopkb/<filename>`); never touches the legacy location.
pub fn write_state_json(bundle_dir: &Path, filename: &str, data: &serde_json::Value) -> Result<()> {
    write_json(&state_path(bundle_dir, filename), data)
}

/// Appends one `[<UTC timestamp>] <message>` line to `.sopkb/ingest.log`, creating
/// the file if needed. Best-effort and silent on I/O failure -- same "never let a
/// logging failure affect the real operation" rule `desktop-tauri::startup_log`
/// already follows, but bundle-scoped (not one global file) and reachable from every
/// binary (`sopkb-cli`, `sopkb-server`, `desktop-tauri`, `desktop-ui2`) that calls
/// into `sopkb-mining`/`sopkb-workbench`, not just the desktop app.
///
/// Exists specifically so a per-chunk/per-section LLM-call failure during
/// normalize/mine -- previously only `eprintln!`, which reaches nowhere in a
/// GUI-subsystem release build's stderr -- has somewhere a real user can actually
/// find it. Callers MUST NOT pass raw document/chunk text, prompts, model
/// responses, or API keys/credentials in `message` -- only structural facts (step
/// name, chunk/section index, attempt number, error type/short message) that are
/// safe to leave sitting in a bundle's own directory indefinitely.
pub fn append_ingest_log(bundle_dir: &Path, message: &str) {
    let path = state_path(bundle_dir, "ingest.log");
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{}] {message}", utc_now());
    }
}

/// For each of the 9 alias entries, in order: if the canonical file already exists,
/// skip (canonical wins); if the legacy file doesn't exist either, skip; otherwise
/// COPY (not move -- the legacy file is never unlinked) the legacy file's TEXT
/// content to the canonical location. Idempotent (a second call is a no-op because
/// the canonical file now exists). The only caller is `sync_okf_bundle` (Phase 5),
/// which must call this BEFORE `reset_okf_documents` or the 7 legacy files under
/// `knowledge/` are destroyed by that reset before ever being migrated (F21).
pub fn migrate_legacy_state(bundle_dir: &Path) -> Result<()> {
    for (filename, legacy_relative) in STATE_FILE_ALIASES {
        let target = bundle_dir.join(".sopkb").join(filename);
        let legacy = bundle_dir.join(legacy_relative);
        if target.exists() || !legacy.exists() {
            continue;
        }
        let text = fs::read_to_string(&legacy)?;
        write_text_native(&target, &text)?;
    }
    Ok(())
}

/// Purely lexical (no realpath/symlink resolution); always POSIX `/`-separated
/// output regardless of host platform, which is why bundle JSON/YAML paths use `/`
/// even on Windows.
pub fn relative_to_bundle(bundle_dir: &Path, path: &Path) -> Result<String> {
    let rel = path.strip_prefix(bundle_dir).map_err(|_| {
        SopkbError::Value(format!("'{}' is not in the subpath of '{}'", path.display(), bundle_dir.display()))
    })?;
    let posix: Vec<&str> = rel.components().map(|c| c.as_os_str().to_str().unwrap_or("")).collect();
    Ok(posix.join("/"))
}

const ALLOWED_STATUSES: &[&str] = &["draft", "in_review", "reviewed", "exported"];

/// Presence check only (a key present with `None`/empty/wrong-type value passes);
/// plus the two content checks on `profile`/`status`. Note the DOUBLE REPORTING a
/// missing `profile` or `status` key triggers (both the presence error and the
/// content error fire) -- current observable Python behavior, reproduced as-is.
pub fn validate_manifest_fields(manifest: &OrderedMap<YamlValue>) -> Vec<String> {
    let mut errors = Vec::new();
    for field in REQUIRED_MANIFEST_FIELDS {
        if !manifest.contains_key(field) {
            errors.push(format!("manifest missing required field: {field}"));
        }
    }
    let profile_ok = manifest.get("profile").and_then(|v| v.as_str()) == Some("sop-knowledge-bundle");
    if !profile_ok {
        errors.push("manifest profile must be sop-knowledge-bundle".to_string());
    }
    let status_ok = manifest.get("status").and_then(|v| v.as_str()).is_some_and(|s| ALLOWED_STATUSES.contains(&s));
    if !status_ok {
        errors.push("manifest status must be draft, in_review, reviewed, or exported".to_string());
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn utc_now_format() {
        let s = utc_now();
        assert_eq!(s.len(), 20);
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn append_ingest_log_creates_the_file_and_appends_timestamped_lines() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        append_ingest_log(&bundle_dir, "first line");
        append_ingest_log(&bundle_dir, "second line");

        let contents = fs::read_to_string(state_path(&bundle_dir, "ingest.log")).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("] first line"), "unexpected line: {}", lines[0]);
        assert!(lines[1].ends_with("] second line"), "unexpected line: {}", lines[1]);
        assert!(lines[0].starts_with('['));
    }

    #[test]
    fn append_ingest_log_never_panics_when_the_bundle_dir_cannot_be_created() {
        // A path with a NUL byte is invalid on every platform and will fail
        // `create_dir_all` -- this must be silently swallowed, not panic or
        // propagate, matching startup_log::log's own "logging must never break
        // the real operation" rule.
        let bad_dir = PathBuf::from("b\0ad");
        append_ingest_log(&bad_dir, "should not panic");
    }

    #[test]
    fn create_bundle_makes_all_required_dirs() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("my-bundle");
        create_bundle(&bundle_dir, None).unwrap();
        for rel in REQUIRED_DIRS {
            assert!(bundle_dir.join(rel).is_dir(), "missing dir: {rel}");
        }
        assert!(bundle_dir.join("manifest.yaml").exists());
    }

    #[test]
    fn create_bundle_default_title_uses_py_title_case() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("glp1-healthcare");
        let result = create_bundle(&bundle_dir, None).unwrap();
        match result {
            CreateBundleResult::Created(m) => assert_eq!(m.title, "Glp1 Healthcare"),
            _ => panic!("expected Created"),
        }
    }

    #[test]
    fn create_bundle_is_idempotent_and_does_not_touch_existing_manifest() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        create_bundle(&bundle_dir, Some("Original Title")).unwrap();
        let result = create_bundle(&bundle_dir, Some("Different Title")).unwrap();
        match result {
            CreateBundleResult::Existing(m) => {
                assert_eq!(m.get("title").and_then(|v| v.as_str()), Some("Original Title"));
            }
            _ => panic!("expected Existing"),
        }
    }

    #[test]
    fn load_manifest_missing_file_error_message() {
        let dir = tempdir().unwrap();
        let err = load_manifest(dir.path()).unwrap_err();
        assert!(err.to_string().starts_with("Missing manifest: "));
    }

    #[test]
    fn read_state_json_falls_back_to_legacy_alias() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("knowledge")).unwrap();
        fs::write(dir.path().join("knowledge/items.json"), "[1,2,3]").unwrap();
        let v = read_state_json(dir.path(), "items.json", serde_json::json!([])).unwrap();
        assert_eq!(v, serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn read_state_json_canonical_wins_over_legacy() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("knowledge")).unwrap();
        fs::write(dir.path().join("knowledge/items.json"), "[1]").unwrap();
        fs::create_dir_all(dir.path().join(".sopkb")).unwrap();
        fs::write(dir.path().join(".sopkb/items.json"), "[9,9]").unwrap();
        let v = read_state_json(dir.path(), "items.json", serde_json::json!([])).unwrap();
        assert_eq!(v, serde_json::json!([9, 9]));
    }

    #[test]
    fn migrate_legacy_state_is_idempotent_and_non_deleting() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("knowledge")).unwrap();
        fs::write(dir.path().join("knowledge/items.json"), "[1,2,3]").unwrap();
        migrate_legacy_state(dir.path()).unwrap();
        assert!(dir.path().join(".sopkb/items.json").exists());
        assert!(dir.path().join("knowledge/items.json").exists(), "legacy must survive");
        let first = fs::read_to_string(dir.path().join(".sopkb/items.json")).unwrap();
        migrate_legacy_state(dir.path()).unwrap(); // second call: no-op
        let second = fs::read_to_string(dir.path().join(".sopkb/items.json")).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn validate_manifest_fields_double_reports_missing_profile() {
        let manifest = OrderedMap::new();
        let errors = validate_manifest_fields(&manifest);
        assert!(errors.contains(&"manifest missing required field: profile".to_string()));
        assert!(errors.contains(&"manifest profile must be sop-knowledge-bundle".to_string()));
    }
}
