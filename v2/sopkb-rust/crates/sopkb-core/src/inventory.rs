//! Source-tree scanning + checksumming. docs/port/port-mapping-a-core-data.md §3.4.
//!
//! Scan ordering: per docs/port/DECISIONS.md Q3, this port uses case-insensitive
//! ordering UNIFORMLY on every platform (not Python's platform-native `sorted()`,
//! which is case-sensitive on POSIX and case-insensitive on Windows) -- eliminating
//! the cross-platform id-divergence problem (P-I8) rather than reproducing it.

use crate::error::{Result, SopkbError};
use crate::ids::{source_id_for, source_version_id_for};
use crate::lifecycle::{
    self, latest_version_number, source_checksums, source_versions_document, update_manifest_sources,
    SOURCE_EVENTS_FILE, SOURCE_VERSIONS_FILE,
};
use crate::models::{SourceRecord, SourceVersion};
use crate::store::{self, relative_to_bundle};
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// lowercased file suffix -> source `type` string.
pub fn supported_type_for_suffix(suffix: &str) -> Option<&'static str> {
    match suffix.to_lowercase().as_str() {
        ".md" => Some("markdown"),
        ".txt" => Some("text"),
        ".pdf" => Some("pdf"),
        ".docx" => Some("docx"),
        _ => None,
    }
}

/// `"sha256:" + lowercase hex`, streamed in 1 MiB chunks (bounded memory use). Hash is
/// over the raw bytes of the file, so unaffected by line-ending/encoding handling.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Ok(format!("sha256:{}", hex_lower(&digest)))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Same second-precision-UTC-with-Z format as [`crate::store::utc_now`], but derived
/// from the file's mtime rather than the current time. Sub-second precision discarded
/// (truncated, not rounded).
pub fn iso_mtime(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: chrono::DateTime<Utc> = modified.into();
    let truncated = Utc
        .with_ymd_and_hms(
            datetime.format("%Y").to_string().parse().unwrap(),
            datetime.format("%m").to_string().parse().unwrap(),
            datetime.format("%d").to_string().parse().unwrap(),
            datetime.format("%H").to_string().parse().unwrap(),
            datetime.format("%M").to_string().parse().unwrap(),
            datetime.format("%S").to_string().parse().unwrap(),
        )
        .single()
        .unwrap();
    Ok(truncated.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// `mkdir -p`, then deletes every direct child (one level; subdirectories removed
/// recursively via `remove_dir_all`, files via `remove_file`). Identical duplicate of
/// `normalize::reset_directory` in Python -- kept as one function here since Rust has
/// no import-cycle reason to duplicate it.
pub fn reset_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            fs::remove_dir_all(&p)?;
        } else {
            fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// Recursive walk of every file AND directory under `root` (matching Python
/// `rglob("*")`, which includes dotfiles/dot-directories and does not follow
/// symlinked directories), sorted case-insensitively on the full path string
/// uniformly on every platform (DECISIONS.md Q3).
fn walk_sorted(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(root, &mut out);
    out.sort_by(|a, b| {
        a.to_string_lossy().to_lowercase().cmp(&b.to_string_lossy().to_lowercase())
    });
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_symlink_dir = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) && path.is_dir();
        out.push(path.clone());
        if path.is_dir() && !is_symlink_dir {
            collect(&path, out);
        }
    }
}

/// Scans `source_dir` and folds it into the bundle's inventory, creating a new source
/// or a new *version* of an existing one per file, and returns the resulting record
/// list (as JSON, matching what the CLI/web caller receives: `inventory["sources"]`,
/// i.e. dicts, not typed structs).
///
/// # Versioning behaviour (CATCHUP_PLAN.md workstream 2)
///
/// A source is identified by its slugified file stem alone ([`source_id_for`]), so
/// re-scanning an edited file finds the *same* source and appends a version to it
/// rather than minting a second one. For each file:
///
/// | existing source? | checksum already seen? | outcome | event |
/// |---|---|---|---|
/// | no | -- | v1 created | `source_added` |
/// | yes | yes | nothing changes | `source_unchanged` |
/// | yes | no | v(n+1) added, v(n) marked `superseded` | `source_version_added` |
///
/// Matching is by source id first, then by `metadata.source_filename` -- the latter
/// catches a file whose stem slugifies differently than it used to but which is
/// recognisably the same document.
///
/// # Two behaviours that changed with versioning, deliberately
///
/// - **`sources/originals/` is no longer wiped at the start of a scan.** Older versions'
///   originals must survive, or the version registry would point at files that no
///   longer exist. Copies are `<source_id>__v<n><ext>`, so versions never collide.
/// - **Removing a file from `source_dir` no longer removes the source from the
///   inventory.** Pre-existing records are carried forward, because a source's history
///   outliving its input file is the entire point of a version registry. Retiring a
///   source is now an explicit operation (`sopkb_review::retire_source`), not a
///   side effect of a scan.
pub fn scan_sources(source_dir: &Path, bundle_dir: &Path) -> Result<Vec<Value>> {
    if !source_dir.exists() {
        return Err(SopkbError::NotFound(format!(
            "Source directory does not exist: {}",
            source_dir.display()
        )));
    }

    lifecycle::migrate_source_version_state(bundle_dir)?;
    let inventory = store::read_state_json(bundle_dir, "inventory.json", json!({"sources": [], "warnings": []}))?;
    let existing: Vec<Value> = inventory.get("sources").and_then(|s| s.as_array()).cloned().unwrap_or_default();

    // Insertion-ordered so the final `records` list is stable; keyed by source id.
    let mut records_by_id: BTreeMap<String, Value> = BTreeMap::new();
    let mut by_filename: BTreeMap<String, String> = BTreeMap::new();
    for source in &existing {
        let Some(id) = source.get("id").and_then(|v| v.as_str()) else { continue };
        records_by_id.insert(id.to_string(), source.clone());
        if let Some(filename) =
            source.get("metadata").and_then(|m| m.get("source_filename")).and_then(|v| v.as_str())
        {
            by_filename.insert(filename.to_string(), id.to_string());
        }
    }

    let mut warnings: Vec<Value> = Vec::new();
    let mut events = lifecycle::read_source_events(bundle_dir)?;

    for path in walk_sorted(source_dir) {
        if !path.is_file() {
            continue;
        }
        let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        if supported_type_for_suffix(&suffix).is_none() {
            warnings.push(json!({"path": path.display().to_string(), "warning": "unsupported file type"}));
            continue;
        }

        let checksum = sha256_file(&path)?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let proposed_id = source_id_for(&stem);
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        let matched_id = if records_by_id.contains_key(&proposed_id) {
            Some(proposed_id.clone())
        } else {
            by_filename.get(&file_name).cloned()
        };

        let (record, event) = match matched_id.and_then(|id| records_by_id.get(&id).cloned()) {
            Some(existing_record) => update_existing_source(bundle_dir, &existing_record, &path, &checksum)?,
            None => create_new_source(bundle_dir, &path, &checksum)?,
        };

        let id = record.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        by_filename.insert(file_name, id.clone());
        records_by_id.insert(id, record);
        events.push(event);
    }

    // `sorted(records_by_id.values(), key=id)` -- a `BTreeMap` is already in that order.
    let records: Vec<Value> = records_by_id.into_values().collect();
    let inventory = json!({"sources": records, "warnings": warnings});
    let sources = inventory["sources"].as_array().cloned().unwrap_or_default();

    store::write_state_json(bundle_dir, "inventory.json", &inventory)?;
    store::write_state_json(bundle_dir, SOURCE_VERSIONS_FILE, &source_versions_document(&sources))?;
    store::write_state_json(bundle_dir, SOURCE_EVENTS_FILE, &json!(events))?;
    update_manifest_sources(bundle_dir, &sources, None)?;

    Ok(sources)
}

/// How `classify_source_updates` labels one file, without touching the bundle.
pub const CLASSIFICATION_NEW_SOURCE: &str = "new_source";
pub const CLASSIFICATION_NEW_VERSION: &str = "new_version";
pub const CLASSIFICATION_UNCHANGED: &str = "unchanged_version";
pub const CLASSIFICATION_UNSUPPORTED: &str = "unsupported";

/// A read-only dry run of [`scan_sources`]: reports what each file in `source_dir`
/// *would* do without copying, writing, or versioning anything. Backs the "preview
/// before ingest" affordance.
///
/// Writes nothing itself, but does call migration first (as the reference
/// implementation does) so that the classification it reports is the one the real scan
/// would produce on a legacy bundle rather than one based on unmigrated state.
pub fn classify_source_updates(source_dir: &Path, bundle_dir: &Path) -> Result<Value> {
    if !source_dir.exists() {
        return Err(SopkbError::NotFound(format!(
            "Source directory does not exist: {}",
            source_dir.display()
        )));
    }
    lifecycle::migrate_source_version_state(bundle_dir)?;
    let inventory = store::read_state_json(bundle_dir, "inventory.json", json!({"sources": [], "warnings": []}))?;
    let existing: Vec<Value> = inventory.get("sources").and_then(|s| s.as_array()).cloned().unwrap_or_default();

    let mut by_id: BTreeMap<String, Value> = BTreeMap::new();
    let mut by_filename: BTreeMap<String, Value> = BTreeMap::new();
    for source in &existing {
        if let Some(id) = source.get("id").and_then(|v| v.as_str()) {
            by_id.insert(id.to_string(), source.clone());
        }
        if let Some(filename) = source.get("metadata").and_then(|m| m.get("source_filename")).and_then(|v| v.as_str())
        {
            by_filename.insert(filename.to_string(), source.clone());
        }
    }

    let mut files: Vec<Value> = Vec::new();
    let mut warnings: Vec<Value> = Vec::new();
    for path in walk_sorted(source_dir) {
        if !path.is_file() {
            continue;
        }
        let display = path.display().to_string();
        let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        if supported_type_for_suffix(&suffix).is_none() {
            warnings.push(json!({"path": display, "warning": "unsupported file type"}));
            files.push(json!({
                "path": display, "classification": CLASSIFICATION_UNSUPPORTED, "warning": "unsupported file type"
            }));
            continue;
        }

        let checksum = sha256_file(&path)?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let proposed_id = source_id_for(&stem);
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let existing_record = by_id.get(&proposed_id).or_else(|| by_filename.get(&file_name));

        let Some(existing_record) = existing_record else {
            files.push(json!({
                "path": display,
                "source_id": proposed_id,
                "classification": CLASSIFICATION_NEW_SOURCE,
                "checksum": checksum,
                "version_number": 1,
            }));
            continue;
        };

        let unchanged = source_checksums(existing_record).contains(&checksum);
        let classification = if unchanged { CLASSIFICATION_UNCHANGED } else { CLASSIFICATION_NEW_VERSION };
        let current = latest_version_number(existing_record);
        files.push(json!({
            "path": display,
            "source_id": existing_record.get("id").cloned().unwrap_or(Value::Null),
            "classification": classification,
            "checksum": checksum,
            "version_number": if unchanged { current } else { current + 1 },
            "active_version_id": existing_record.get("active_version_id").cloned().unwrap_or(Value::Null),
        }));
    }

    Ok(json!({"files": files, "warnings": warnings}))
}

/// First sighting of a file: mints `<id>:v1` and copies the original to
/// `sources/originals/<id>__v1<ext>`.
fn create_new_source(bundle_dir: &Path, path: &Path, checksum: &str) -> Result<(Value, Value)> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let source_id = source_id_for(&stem);
    let version_number = 1;
    let source_version_id = source_version_id_for(&source_id, version_number);
    let destination = copy_versioned_original(bundle_dir, path, &source_id, version_number)?;
    let normalized_path = format!("sources/normalized/{source_id}__v{version_number}.md");
    let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let source_type = supported_type_for_suffix(&suffix).unwrap_or("markdown");
    let size = fs::metadata(path)?.len();
    let modified_time = iso_mtime(path)?;
    let original_path = relative_to_bundle(bundle_dir, &destination)?;

    let version = SourceVersion {
        source_id: source_id.clone(),
        source_version_id: source_version_id.clone(),
        version_number,
        checksum: checksum.to_string(),
        status: "active".to_string(),
        original_path: original_path.clone(),
        normalized_path: normalized_path.clone(),
        size_bytes: size,
        modified_time: modified_time.clone(),
    };
    let record = SourceRecord {
        id: source_id.clone(),
        title: stem.replace(['_', '-'], " ").trim().to_string(),
        source_type: source_type.to_string(),
        original_path,
        normalized_path: Some(normalized_path),
        checksum: checksum.to_string(),
        size,
        modified_time,
        parse_status: "pending".to_string(),
        warnings: Vec::new(),
        metadata: {
            let mut m = serde_json::Map::new();
            m.insert(
                "source_filename".to_string(),
                json!(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            );
            m
        },
        source_version_id: Some(source_version_id.clone()),
        version_number,
        status: "active".to_string(),
        active_version_id: Some(source_version_id.clone()),
        versions: vec![version],
    };
    let event = lifecycle::source_event("source_added", &source_id, &source_version_id, path, Utc::now());
    Ok((serde_json::to_value(record)?, event))
}

/// A file we have seen before. Content we have already recorded (under ANY version) is
/// a no-op beyond an audit event; new content becomes the next version and demotes the
/// current one to `superseded`.
fn update_existing_source(bundle_dir: &Path, source: &Value, path: &Path, checksum: &str) -> Result<(Value, Value)> {
    let source_id = source.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();

    if source_checksums(source).iter().any(|c| c == checksum) {
        let active = source.get("active_version_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let event = lifecycle::source_event("source_unchanged", &source_id, &active, path, Utc::now());
        return Ok((source.clone(), event));
    }

    let mut record = source.clone();
    let version_number = latest_version_number(source) + 1;
    let source_version_id = source_version_id_for(&source_id, version_number);
    let destination = copy_versioned_original(bundle_dir, path, &source_id, version_number)?;
    let normalized_path = format!("sources/normalized/{source_id}__v{version_number}.md");
    let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let source_type = supported_type_for_suffix(&suffix).unwrap_or("markdown");
    let size = fs::metadata(path)?.len();
    let modified_time = iso_mtime(path)?;
    let original_path = relative_to_bundle(bundle_dir, &destination)?;

    let object = record.as_object_mut().ok_or_else(|| SopkbError::Value("inventory source is not a mapping".into()))?;

    let mut versions: Vec<Value> = object.get("versions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for version in versions.iter_mut() {
        if version.get("status").and_then(|v| v.as_str()) == Some("active") {
            if let Some(vo) = version.as_object_mut() {
                vo.insert("status".into(), json!("superseded"));
            }
        }
    }
    versions.push(serde_json::to_value(SourceVersion {
        source_id: source_id.clone(),
        source_version_id: source_version_id.clone(),
        version_number,
        checksum: checksum.to_string(),
        status: "active".to_string(),
        original_path: original_path.clone(),
        normalized_path: normalized_path.clone(),
        size_bytes: size,
        modified_time: modified_time.clone(),
    })?);

    let mut metadata = object.get("metadata").and_then(|m| m.as_object()).cloned().unwrap_or_default();
    metadata.insert(
        "source_filename".into(),
        json!(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
    );

    object.insert("type".into(), json!(source_type));
    object.insert("original_path".into(), json!(original_path));
    object.insert("normalized_path".into(), json!(normalized_path));
    object.insert("checksum".into(), json!(checksum));
    object.insert("size".into(), json!(size));
    object.insert("modified_time".into(), json!(modified_time));
    object.insert("parse_status".into(), json!("pending"));
    object.insert("warnings".into(), json!([]));
    object.insert("metadata".into(), Value::Object(metadata));
    object.insert("source_version_id".into(), json!(source_version_id));
    object.insert("version_number".into(), json!(version_number));
    // A new version REACTIVATES a retired source: someone put the file back.
    object.insert("status".into(), json!("active"));
    object.insert("active_version_id".into(), json!(source_version_id));
    object.insert("versions".into(), json!(versions));

    let event = lifecycle::source_event("source_version_added", &source_id, &source_version_id, path, Utc::now());
    Ok((record, event))
}

/// Copies `path` to `sources/originals/<source_id>__v<n><ext>`. If that destination
/// already exists with IDENTICAL content the copy is skipped (idempotent re-scan);
/// if it exists with *different* content that is a hard error rather than a silent
/// overwrite -- an already-published version's bytes are immutable, and overwriting
/// them would invalidate every span offset recorded against it.
fn copy_versioned_original(bundle_dir: &Path, path: &Path, source_id: &str, version_number: u32) -> Result<PathBuf> {
    let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default().to_lowercase();
    let destination =
        bundle_dir.join("sources").join("originals").join(format!("{source_id}__v{version_number}{suffix}"));
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    if destination.exists() {
        if sha256_file(&destination)? != sha256_file(path)? {
            return Err(SopkbError::Value(format!(
                "versioned source already exists with different content: {}",
                destination.display()
            )));
        }
        return Ok(destination);
    }
    fs::copy(path, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scan_sources_missing_dir_errors_with_exact_message() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let missing = dir.path().join("nope");
        let err = scan_sources(&missing, &bundle_dir).unwrap_err();
        assert!(err.to_string().starts_with("Source directory does not exist: "));
    }

    #[test]
    fn scan_sources_skips_unsupported_and_records_supported() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("a.md"), "# Hello\n\nWorld.\n").unwrap();
        fs::write(source_dir.join("b.png"), "not a real png").unwrap();

        let records = scan_sources(&source_dir, &bundle_dir).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["type"], "markdown");

        let inventory: serde_json::Value =
            store::read_json(&bundle_dir.join(".sopkb/inventory.json"), serde_json::Value::Null).unwrap();
        assert_eq!(inventory["warnings"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["warnings"][0]["warning"], "unsupported file type");
    }

    #[test]
    fn source_ids_are_stable_across_repeated_scans() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("a.md"), "# Hello\n").unwrap();
        fs::write(source_dir.join("b.md"), "# World\n").unwrap();

        let first = scan_sources(&source_dir, &bundle_dir).unwrap();
        let second = scan_sources(&source_dir, &bundle_dir).unwrap();
        let first_ids: Vec<&str> = first.iter().map(|r| r["id"].as_str().unwrap()).collect();
        let second_ids: Vec<&str> = second.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(first_ids, second_ids);
    }

    fn one_source_bundle(content: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, Some("Versioning Bundle")).unwrap();
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("weird-headings.md"), content).unwrap();
        (dir, bundle_dir, source_dir)
    }

    /// Ground truth: `fixtures/cases/weird-headings-md/expected-python/bundle/`, whose
    /// regenerated `inventory.json` / `manifest.yaml` / `source_versions.json` /
    /// `source_events.json` this reproduces field for field.
    #[test]
    fn a_first_scan_produces_the_bare_stem_id_and_v1_paths_from_the_fixture_corpus() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n\nStaff must confirm.\n");
        let records = scan_sources(&source_dir, &bundle_dir).unwrap();

        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record["id"], "weird-headings", "bare stem, no content-hash suffix");
        assert_eq!(record["source_version_id"], "weird-headings:v1");
        assert_eq!(record["active_version_id"], "weird-headings:v1");
        assert_eq!(record["version_number"], 1);
        assert_eq!(record["status"], "active");
        assert_eq!(record["original_path"], "sources/originals/weird-headings__v1.md");
        assert_eq!(record["normalized_path"], "sources/normalized/weird-headings__v1.md");
        assert_eq!(record["versions"][0]["status"], "active");
        assert_eq!(record["versions"][0]["source_version_id"], "weird-headings:v1");
        assert!(bundle_dir.join("sources/originals/weird-headings__v1.md").exists());

        let registry = store::read_state_json(&bundle_dir, SOURCE_VERSIONS_FILE, Value::Null).unwrap();
        assert_eq!(registry["versions"][0]["source_version_id"], "weird-headings:v1");
        assert_eq!(registry["versions"][0]["status"], "active");

        let events = store::read_state_json(&bundle_dir, SOURCE_EVENTS_FILE, Value::Null).unwrap();
        assert_eq!(events.as_array().unwrap().len(), 1);
        assert_eq!(events[0]["action"], "source_added");
        assert_eq!(events[0]["actor"], "sopkb/inventory");
        assert_eq!(events[0]["source_version_id"], "weird-headings:v1");

        let manifest = store::load_manifest(&bundle_dir).unwrap();
        let entry = &manifest.get("sources").unwrap().as_sequence().unwrap()[0];
        let entry = entry.as_mapping().unwrap();
        let keys: Vec<&str> = entry.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["id", "type", "path", "source_version_id", "status"]);
        assert_eq!(entry.get("source_version_id").unwrap().as_str(), Some("weird-headings:v1"));
        assert_eq!(entry.get("status").unwrap().as_str(), Some("active"));
    }

    #[test]
    fn rescanning_unchanged_content_adds_no_version_but_does_log_an_event() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n\nStaff must confirm.\n");
        scan_sources(&source_dir, &bundle_dir).unwrap();
        let records = scan_sources(&source_dir, &bundle_dir).unwrap();

        assert_eq!(records[0]["version_number"], 1);
        assert_eq!(records[0]["versions"].as_array().unwrap().len(), 1);
        let events = store::read_state_json(&bundle_dir, SOURCE_EVENTS_FILE, Value::Null).unwrap();
        let actions: Vec<&str> = events.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["source_added", "source_unchanged"]);
    }

    #[test]
    fn editing_a_source_adds_v2_and_supersedes_v1_keeping_both_originals() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n\nStaff must confirm.\n");
        scan_sources(&source_dir, &bundle_dir).unwrap();
        fs::write(source_dir.join("weird-headings.md"), "# Purpose\n\nStaff must now escalate.\n").unwrap();
        let records = scan_sources(&source_dir, &bundle_dir).unwrap();

        assert_eq!(records.len(), 1, "an edit is a new VERSION, never a second source");
        let record = &records[0];
        assert_eq!(record["id"], "weird-headings");
        assert_eq!(record["version_number"], 2);
        assert_eq!(record["source_version_id"], "weird-headings:v2");
        assert_eq!(record["original_path"], "sources/originals/weird-headings__v2.md");
        let versions = record["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0]["status"], "superseded");
        assert_eq!(versions[1]["status"], "active");

        assert!(bundle_dir.join("sources/originals/weird-headings__v1.md").exists(), "v1's bytes must survive");
        assert!(bundle_dir.join("sources/originals/weird-headings__v2.md").exists());

        let events = store::read_state_json(&bundle_dir, SOURCE_EVENTS_FILE, Value::Null).unwrap();
        let actions: Vec<&str> = events.as_array().unwrap().iter().map(|e| e["action"].as_str().unwrap()).collect();
        assert_eq!(actions, vec!["source_added", "source_version_added"]);
    }

    /// Reverting a file to content the bundle already recorded under v1 is treated as
    /// unchanged, not as a v3 -- checksum matching spans the whole version history.
    #[test]
    fn reverting_to_a_previously_seen_checksum_is_unchanged_not_a_new_version() {
        let original = "# Purpose\n\nStaff must confirm.\n";
        let (_dir, bundle_dir, source_dir) = one_source_bundle(original);
        scan_sources(&source_dir, &bundle_dir).unwrap();
        fs::write(source_dir.join("weird-headings.md"), "# Purpose\n\nEdited.\n").unwrap();
        scan_sources(&source_dir, &bundle_dir).unwrap();
        fs::write(source_dir.join("weird-headings.md"), original).unwrap();
        let records = scan_sources(&source_dir, &bundle_dir).unwrap();
        assert_eq!(records[0]["versions"].as_array().unwrap().len(), 2);
    }

    /// A source whose file has been deleted from the input tree stays in the inventory
    /// with its history intact -- removal is not retirement.
    #[test]
    fn a_removed_input_file_leaves_its_source_record_in_place() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n\nBody.\n");
        scan_sources(&source_dir, &bundle_dir).unwrap();
        fs::remove_file(source_dir.join("weird-headings.md")).unwrap();
        fs::write(source_dir.join("other.md"), "# Other\n").unwrap();
        let records = scan_sources(&source_dir, &bundle_dir).unwrap();
        let ids: Vec<&str> = records.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["other", "weird-headings"]);
    }

    #[test]
    fn classify_source_updates_previews_without_writing_anything() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n\nBody.\n");
        let preview = classify_source_updates(&source_dir, &bundle_dir).unwrap();
        assert_eq!(preview["files"][0]["classification"], CLASSIFICATION_NEW_SOURCE);
        assert_eq!(preview["files"][0]["source_id"], "weird-headings");
        assert_eq!(preview["files"][0]["version_number"], 1);
        assert!(!bundle_dir.join("sources/originals/weird-headings__v1.md").exists(), "a preview copies nothing");

        scan_sources(&source_dir, &bundle_dir).unwrap();
        let unchanged = classify_source_updates(&source_dir, &bundle_dir).unwrap();
        assert_eq!(unchanged["files"][0]["classification"], CLASSIFICATION_UNCHANGED);
        assert_eq!(unchanged["files"][0]["version_number"], 1);

        fs::write(source_dir.join("weird-headings.md"), "# Purpose\n\nChanged.\n").unwrap();
        let changed = classify_source_updates(&source_dir, &bundle_dir).unwrap();
        assert_eq!(changed["files"][0]["classification"], CLASSIFICATION_NEW_VERSION);
        assert_eq!(changed["files"][0]["version_number"], 2);
        assert_eq!(changed["files"][0]["active_version_id"], "weird-headings:v1");
    }

    #[test]
    fn classify_source_updates_reports_unsupported_types_in_both_lists() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n");
        fs::write(source_dir.join("image.png"), "not a png").unwrap();
        let preview = classify_source_updates(&source_dir, &bundle_dir).unwrap();
        let classifications: Vec<&str> =
            preview["files"].as_array().unwrap().iter().map(|f| f["classification"].as_str().unwrap()).collect();
        assert!(classifications.contains(&CLASSIFICATION_UNSUPPORTED));
        assert_eq!(preview["warnings"].as_array().unwrap().len(), 1);
    }

    /// Re-copying the same version is fine; a *different* payload under an existing
    /// version filename is refused rather than silently overwritten.
    #[test]
    fn copy_versioned_original_refuses_to_overwrite_a_version_with_different_bytes() {
        let (_dir, bundle_dir, source_dir) = one_source_bundle("# Purpose\n");
        scan_sources(&source_dir, &bundle_dir).unwrap();
        // Corrupt the stored v1 so the next attempt to write v1 disagrees with it.
        fs::write(bundle_dir.join("sources/originals/weird-headings__v1.md"), "tampered").unwrap();
        let err = copy_versioned_original(&bundle_dir, &source_dir.join("weird-headings.md"), "weird-headings", 1)
            .unwrap_err();
        assert!(err.to_string().starts_with("versioned source already exists with different content: "));
    }

    #[test]
    fn reset_directory_removes_pre_placed_stale_file() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("originals");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.md"), "old").unwrap();
        reset_directory(&target).unwrap();
        assert!(!target.join("stale.md").exists());
    }

    #[test]
    fn classify_source_updates_missing_dir_errors_with_exact_message() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, None).unwrap();
        let missing = dir.path().join("nope");
        let err = classify_source_updates(&missing, &bundle_dir).unwrap_err();
        assert!(err.to_string().starts_with("Source directory does not exist: "));
    }

}
