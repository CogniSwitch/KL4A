//! Source versioning, the two lifecycle state files, and the multi-file transaction
//! primitive the retire operation is built on.
//!
//! Port of the source-versioning half of `tools/sopkb/sopkb/bundle_store.py`
//! (`migrate_source_version_state`) plus the state-machine core of
//! `tools/sopkb/sopkb/source_lifecycle.py` (`retire_source`). See
//! `docs/port/CATCHUP_PLAN.md` workstream 2.
//!
//! # The two state files
//!
//! - `.sopkb/source_versions.json` -- `{"versions": [...]}`, a flattened registry of
//!   every version of every source. It is always a *derived view* over
//!   `inventory.json`'s per-source `versions` arrays, never an independent list, so
//!   the two cannot drift out of sync.
//! - `.sopkb/source_events.json` -- a flat, append-only array of ingestion/lifecycle
//!   events (`source_added`, `source_version_added`, `source_unchanged`,
//!   `source_retired`). Nothing ever rewrites or removes an existing element.
//!
//! # Why the split between planning and writing
//!
//! Every mutation here is computed as a complete new state *in memory* first
//! ([`plan_retire_source`] returns values, it does no I/O at all), and only then
//! handed to a [`FileTransaction`] that writes it. That is the fix for the
//! non-transactional `retire_source` called out in CATCHUP_PLAN.md: the reference
//! implementation writes four files sequentially and validates afterwards, so a
//! failure anywhere in that sequence leaves a permanently half-mutated bundle with no
//! way back.

use crate::error::Result;
use crate::ids::source_version_id_for;
use crate::store;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const SOURCE_VERSIONS_FILE: &str = "source_versions.json";
pub const SOURCE_EVENTS_FILE: &str = "source_events.json";

/// `{"versions": []}` -- the empty shape of `.sopkb/source_versions.json`, used as the
/// read default so callers never have to distinguish "absent" from "empty".
pub fn empty_source_versions() -> Value {
    json!({"versions": []})
}

/// Reads `.sopkb/source_events.json`. A file whose top level is not an array is
/// treated as an empty log rather than an error -- matching the reference
/// implementation's `if not isinstance(events, list): events = []`, which exists
/// because an append-only log is recoverable by appending, and refusing to ingest at
/// all because the *audit trail* is malformed would be a worse failure mode.
pub fn read_source_events(bundle_dir: &Path) -> Result<Vec<Value>> {
    let raw = store::read_state_json(bundle_dir, SOURCE_EVENTS_FILE, json!([]))?;
    Ok(raw.as_array().cloned().unwrap_or_default())
}

/// Reads `.sopkb/source_versions.json`'s `"versions"` array, tolerating both an absent
/// file and a malformed top level.
pub fn read_source_versions(bundle_dir: &Path) -> Result<Vec<Value>> {
    let raw = store::read_state_json(bundle_dir, SOURCE_VERSIONS_FILE, empty_source_versions())?;
    Ok(raw.get("versions").and_then(|v| v.as_array()).cloned().unwrap_or_default())
}

/// Flattens every source's `versions` array into the registry's element order:
/// sorted by `(source_id, version_number)`, both ascending. A missing/unparseable
/// `version_number` sorts as 0.
pub fn source_version_entries(sources: &[Value]) -> Vec<Value> {
    let mut entries: Vec<Value> = Vec::new();
    for source in sources {
        for version in source.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
            entries.push(version.clone());
        }
    }
    entries.sort_by(|a, b| {
        let key = |v: &Value| {
            (
                v.get("source_id").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                v.get("version_number").and_then(|n| n.as_i64()).unwrap_or(0),
            )
        };
        key(a).cmp(&key(b))
    });
    entries
}

/// Builds the registry document from a source list (the `{"versions": [...]}` wrapper
/// plus [`source_version_entries`]).
pub fn source_versions_document(sources: &[Value]) -> Value {
    json!({"versions": source_version_entries(sources)})
}

fn str_of(value: Option<&Value>) -> Option<&str> {
    value.and_then(|v| v.as_str())
}

/// `int(x or 1)` over a JSON value that may be a number, a numeric string, `null`, or
/// absent -- all of which real bundles contain, since `version_number` has been written
/// by more than one generation of the tool.
fn version_number_of(value: Option<&Value>) -> u32 {
    match value {
        Some(Value::Number(n)) => n.as_i64().filter(|n| *n > 0).unwrap_or(1) as u32,
        Some(Value::String(s)) => s.parse::<u32>().ok().filter(|n| *n > 0).unwrap_or(1),
        _ => 1,
    }
}

/// Highest `version_number` across a source's `versions` array AND its own top-level
/// `version_number` (the latter matters for a source that predates the array).
pub fn latest_version_number(source: &Value) -> u32 {
    let mut max = version_number_of(source.get("version_number"));
    for version in source.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
        max = max.max(version_number_of(version.get("version_number")));
    }
    max
}

/// Every checksum this source has ever had: its current one plus one per recorded
/// version. Used to decide "is this file content we have already seen?" -- which is
/// what makes re-scanning an unchanged tree a no-op instead of a version bump.
pub fn source_checksums(source: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(c) = str_of(source.get("checksum")) {
        if !c.is_empty() {
            out.push(c.to_string());
        }
    }
    for version in source.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
        if let Some(c) = str_of(version.get("checksum")) {
            if !c.is_empty() && !out.iter().any(|e| e == c) {
                out.push(c.to_string());
            }
        }
    }
    out
}

/// Brings a bundle written by a pre-versioning engine up to the current scheme, in
/// place: fills in `source_version_id` / `version_number` / `status` /
/// `active_version_id` / `versions` on every inventory source, writes the derived
/// `source_versions.json` registry, and back-fills `source_version_id` on sections and
/// `source_version_id` / `lifecycle_status` / `supersedes` / `superseded_by` on items.
///
/// Idempotent: a second call on an already-migrated bundle computes the same values,
/// finds nothing changed, and writes nothing.
///
/// **This does not rename anything.** A legacy source keeps its old hash-suffixed id
/// and its old `sources/normalized/<stem>-<hash>.md` path; it just gains the version
/// metadata that hangs off it. Renaming would invalidate every section, item, evidence
/// and rule id in the bundle, all of which embed the source id. Verified against the
/// regenerated `malformed-null-confidence` fixture, whose migrated source is still
/// `intake-checklist-malformed01` at `sources/normalized/intake-checklist-malformed01.md`
/// while carrying a brand-new `intake-checklist-malformed01:v1` version entry.
///
/// # Deviation from the reference implementation (deliberate)
///
/// The Python original adds missing `supersedes`/`superseded_by` keys to items with
/// `dict.setdefault`, but never sets its `item_changed` flag when it does -- so if
/// those were the *only* things missing, the mutation happened in memory and was then
/// dropped on the floor, and the next call had to redo it. Here every mutation,
/// including those two, flags the write-back (CATCHUP_PLAN.md's second named bug fix).
pub fn migrate_source_version_state(bundle_dir: &Path) -> Result<()> {
    let mut inventory = store::read_state_json(bundle_dir, "inventory.json", Value::Null)?;
    let Some(sources) = inventory.get_mut("sources").and_then(|s| s.as_array_mut()) else {
        return Ok(());
    };

    let mut changed = false;
    let mut registry: Vec<Value> = Vec::new();
    // `source_id -> source_version_id`, for the section/item back-fill below.
    let mut version_by_source: BTreeMap<String, String> = BTreeMap::new();

    for source in sources.iter_mut() {
        let Some(object) = source.as_object_mut() else { continue };
        let Some(source_id) = object.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) else { continue };
        if source_id.is_empty() {
            continue;
        }

        let version_number = version_number_of(object.get("version_number"));
        let source_version_id = match object.get("source_version_id").and_then(|v| v.as_str()) {
            Some(existing) if !existing.is_empty() => existing.to_string(),
            _ => source_version_id_for(&source_id, version_number),
        };
        version_by_source.insert(source_id.clone(), source_version_id.clone());

        if object.get("source_version_id") != Some(&json!(source_version_id)) {
            object.insert("source_version_id".into(), json!(source_version_id));
            changed = true;
        }
        if object.get("version_number") != Some(&json!(version_number)) {
            object.insert("version_number".into(), json!(version_number));
            changed = true;
        }
        // Note the asymmetry with the fields above, and keep it: `status` is only
        // filled in when ABSENT/empty, never corrected to a computed value. A source
        // that has been retired must survive migration still retired.
        if str_of(object.get("status")).unwrap_or("").is_empty() {
            object.insert("status".into(), json!("active"));
            changed = true;
        }
        if object.get("active_version_id") != Some(&json!(source_version_id)) {
            object.insert("active_version_id".into(), json!(source_version_id));
            changed = true;
        }

        let has_versions =
            object.get("versions").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
        if !has_versions {
            let synthesized = json!({
                "source_id": source_id,
                "source_version_id": source_version_id,
                "version_number": version_number,
                "checksum": object.get("checksum").cloned().unwrap_or(Value::Null),
                "status": "active",
                "original_path": object.get("original_path").cloned().unwrap_or(Value::Null),
                "normalized_path": object.get("normalized_path").cloned().unwrap_or(Value::Null),
                // Reference quirk: the record's key is `size`, the version's is
                // `size_bytes`; a record that already has `size_bytes` wins.
                "size_bytes": object
                    .get("size_bytes")
                    .or_else(|| object.get("size"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "modified_time": object.get("modified_time").cloned().unwrap_or(Value::Null),
            });
            object.insert("versions".into(), json!([synthesized]));
            changed = true;
        }
        for version in object.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
            if version.is_object() {
                registry.push(version.clone());
            }
        }
    }

    if changed {
        store::write_state_json(bundle_dir, "inventory.json", &inventory)?;
    }
    if !registry.is_empty() {
        // Document order, NOT `source_version_entries`' sort: this mirrors the
        // reference implementation, and `scan_sources` rewrites the registry sorted
        // on the very next ingest anyway.
        store::write_state_json(bundle_dir, SOURCE_VERSIONS_FILE, &json!({"versions": registry}))?;
    }

    migrate_sections(bundle_dir, &version_by_source)?;
    migrate_items(bundle_dir, &version_by_source)?;
    Ok(())
}

fn migrate_sections(bundle_dir: &Path, version_by_source: &BTreeMap<String, String>) -> Result<()> {
    let mut sections = store::read_state_json(bundle_dir, "sections.json", json!([]))?;
    let Some(list) = sections.as_array_mut() else { return Ok(()) };
    let mut changed = false;
    for section in list.iter_mut() {
        let Some(object) = section.as_object_mut() else { continue };
        let Some(source_id) = object.get("source_id").and_then(|v| v.as_str()) else { continue };
        let Some(version_id) = version_by_source.get(source_id) else { continue };
        if object.get("source_version_id") != Some(&json!(version_id)) {
            object.insert("source_version_id".into(), json!(version_id));
            changed = true;
        }
    }
    if changed {
        store::write_state_json(bundle_dir, "sections.json", &sections)?;
    }
    Ok(())
}

fn migrate_items(bundle_dir: &Path, version_by_source: &BTreeMap<String, String>) -> Result<()> {
    let mut items = store::read_state_json(bundle_dir, "items.json", json!([]))?;
    let Some(list) = items.as_array_mut() else { return Ok(()) };
    let mut changed = false;
    for item in list.iter_mut() {
        let Some(object) = item.as_object_mut() else { continue };
        // Unlike sections, an item's existing `source_version_id` is NEVER corrected:
        // it records which version the item was mined from, which may legitimately be
        // an older one than the source's current version.
        if str_of(object.get("source_version_id")).unwrap_or("").is_empty() {
            if let Some(version_id) =
                object.get("source_id").and_then(|v| v.as_str()).and_then(|s| version_by_source.get(s))
            {
                object.insert("source_version_id".into(), json!(version_id));
                changed = true;
            }
        }
        if str_of(object.get("lifecycle_status")).unwrap_or("").is_empty() {
            object.insert("lifecycle_status".into(), json!("active"));
            changed = true;
        }
        // The two `setdefault`s whose write-back the reference implementation forgot
        // to flag. Both flag it here.
        for key in ["supersedes", "superseded_by"] {
            if !object.contains_key(key) {
                object.insert(key.into(), json!([]));
                changed = true;
            }
        }
    }
    if changed {
        store::write_state_json(bundle_dir, "items.json", &items)?;
    }
    Ok(())
}

/// An all-or-nothing group of file writes.
///
/// Before the first write to a given path, the path's current bytes (or its
/// non-existence) are captured in memory. [`Self::rollback`] puts every captured path
/// back exactly as it was, in reverse write order; [`Self::commit`] discards the
/// captures. Dropping an uncommitted transaction rolls it back on a best-effort basis,
/// so an early `?` return anywhere in the middle of a multi-file mutation cannot leave
/// a half-written bundle behind.
///
/// Each individual write still goes through [`store::write_state_json`] and therefore
/// through the atomic temp-file-plus-rename in [`store`], so a *single* file is never
/// observed truncated either.
///
/// # What this does and does not guarantee
///
/// It makes the group atomic with respect to *errors*: any failure, at any point,
/// including one raised by a validation pass that runs after the writes, ends with the
/// bundle byte-identical to how it started. It is not atomic with respect to a *crash*:
/// the undo log lives in memory, so a process killed mid-transaction leaves whatever
/// subset of files it had already written. Making that case recoverable needs an
/// on-disk journal, which is a bigger change than this workstream, and the failure
/// mode CATCHUP_PLAN.md actually names -- "validates only after, no rollback on
/// failure" -- is fully covered.
pub struct FileTransaction {
    entries: Vec<Snapshot>,
    finalized: bool,
}

struct Snapshot {
    path: PathBuf,
    /// `None` means "did not exist", so rollback deletes rather than restores.
    original: Option<Vec<u8>>,
}

impl FileTransaction {
    pub fn new() -> Self {
        Self { entries: Vec::new(), finalized: false }
    }

    /// Captures `path`'s current contents without writing anything, so that a later
    /// mutation made by code outside this transaction (a validation pass that
    /// regenerates reports, say) can still be undone by [`Self::rollback`]. A no-op if
    /// `path` is already captured.
    pub fn snapshot(&mut self, path: &Path) -> Result<()> {
        if self.entries.iter().any(|e| e.path == path) {
            return Ok(());
        }
        let original = if path.exists() { Some(fs::read(path)?) } else { None };
        self.entries.push(Snapshot { path: path.to_path_buf(), original });
        Ok(())
    }

    /// Snapshots every regular file directly inside `dir` (one level, no recursion).
    /// A missing directory is not an error. Used to bring derived artifacts that a
    /// post-write validation pass rewrites -- `reports/` -- inside the transaction.
    pub fn snapshot_dir_files(&mut self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        let mut paths: Vec<PathBuf> =
            fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        paths.sort();
        for path in paths {
            self.snapshot(&path)?;
        }
        Ok(())
    }

    /// Snapshot-then-write one `.sopkb/<filename>` state file.
    pub fn write_state_json(&mut self, bundle_dir: &Path, filename: &str, data: &Value) -> Result<()> {
        let path = store::state_path(bundle_dir, filename);
        self.snapshot(&path)?;
        store::write_state_json(bundle_dir, filename, data)
    }

    /// Snapshot-then-write `manifest.yaml`.
    pub fn save_manifest(&mut self, bundle_dir: &Path, manifest: &sopkb_fmt::OrderedMap<sopkb_fmt::YamlValue>) -> Result<()> {
        self.snapshot(&bundle_dir.join("manifest.yaml"))?;
        store::save_manifest_raw(bundle_dir, manifest)
    }

    /// Accepts the writes and throws away the undo log.
    pub fn commit(mut self) {
        self.finalized = true;
        self.entries.clear();
    }

    /// Restores every captured path, reporting the first failure. Later entries are
    /// still attempted after a failure -- restoring three of four files beats
    /// restoring none.
    pub fn rollback(mut self) -> Result<()> {
        self.finalized = true;
        let entries = std::mem::take(&mut self.entries);
        restore_all(entries)
    }
}

impl Default for FileTransaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FileTransaction {
    fn drop(&mut self) {
        if self.finalized {
            return;
        }
        // An uncommitted transaction going out of scope means an error path took a
        // `?` somewhere in the middle. Undo, best effort -- a panic in a destructor
        // would abort the process and is never the right escalation here.
        let _ = restore_all(std::mem::take(&mut self.entries));
    }
}

fn restore_all(entries: Vec<Snapshot>) -> Result<()> {
    let mut first_error: Option<crate::error::SopkbError> = None;
    for entry in entries.into_iter().rev() {
        let outcome = match &entry.original {
            Some(bytes) => restore_bytes(&entry.path, bytes),
            None => remove_if_present(&entry.path),
        };
        if let Err(e) = outcome {
            first_error.get_or_insert(e);
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn restore_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// What [`plan_retire_source`] decided, before anything is written.
#[derive(Debug)]
pub enum RetirePlan {
    /// The source was already retired. No files should be touched -- re-running a
    /// retire must not append a second, near-identical audit event (CATCHUP_PLAN.md's
    /// first named bug fix). Carries the existing `source_retired` event for this
    /// source, when the log has one, so the caller can return the same answer it
    /// returned the first time.
    AlreadyRetired { existing_event: Option<Value> },
    /// The full new state, ready to write.
    Retire(Box<RetireMutation>),
}

/// A complete post-retirement bundle state, computed entirely in memory.
#[derive(Debug)]
pub struct RetireMutation {
    pub inventory: Value,
    pub items: Value,
    pub source_versions: Value,
    pub events: Value,
    /// The single event appended to `events`, also returned to the caller.
    pub event: Value,
    /// Inventory sources in their new state, for the manifest rewrite.
    pub sources: Vec<Value>,
}

/// Computes the effect of retiring `source_id`, or reports that it is already retired.
/// Pure: no I/O, no clock read (`timestamp` is passed in), so it is exhaustively
/// unit-testable and so the caller controls exactly when the bundle is touched.
///
/// Retiring a source marks the source and all of its still-active versions `retired`,
/// and flips every *active* knowledge item mined from it to `lifecycle_status:
/// "retired"`. Items that are already `superseded` or `retired` are left alone -- their
/// existing status is more specific than "retired" would be, and rewriting it would
/// lose the reason they left the active set.
///
/// Nothing is deleted. Evidence and provenance survive retirement intact; retirement is
/// purely a visibility decision, which is what makes it reversible in principle and
/// what lets an agent still ask for retired knowledge explicitly.
pub fn plan_retire_source(
    inventory: &Value,
    items: &Value,
    events: &[Value],
    source_id: &str,
    actor: &str,
    rationale: &str,
    timestamp: &str,
) -> Result<RetirePlan> {
    let mut inventory = inventory.clone();
    let sources = inventory
        .get_mut("sources")
        .and_then(|s| s.as_array_mut())
        .ok_or_else(|| crate::error::SopkbError::NotFound(format!("source not found: {source_id}")))?;

    let index = sources
        .iter()
        .position(|s| s.get("id").and_then(|v| v.as_str()) == Some(source_id))
        .ok_or_else(|| crate::error::SopkbError::NotFound(format!("source not found: {source_id}")))?;

    let previous_status = str_of(sources[index].get("status")).unwrap_or("active").to_string();
    if previous_status == "retired" {
        let existing_event = events
            .iter()
            .rev()
            .find(|e| {
                e.get("action").and_then(|v| v.as_str()) == Some("source_retired")
                    && e.get("source_id").and_then(|v| v.as_str()) == Some(source_id)
            })
            .cloned();
        return Ok(RetirePlan::AlreadyRetired { existing_event });
    }

    let active_version_id = sources[index].get("active_version_id").cloned().unwrap_or(Value::Null);
    {
        let object = sources[index].as_object_mut().expect("inventory source must be an object");
        object.insert("status".into(), json!("retired"));
        if let Some(versions) = object.get_mut("versions").and_then(|v| v.as_array_mut()) {
            for version in versions.iter_mut() {
                if version.get("status").and_then(|v| v.as_str()) == Some("active") {
                    if let Some(vo) = version.as_object_mut() {
                        vo.insert("status".into(), json!("retired"));
                    }
                }
            }
        }
    }

    let mut items = items.clone();
    let mut retired_item_ids: Vec<Value> = Vec::new();
    for item in items.as_array_mut().into_iter().flatten() {
        let Some(object) = item.as_object_mut() else { continue };
        if object.get("source_id").and_then(|v| v.as_str()) != Some(source_id) {
            continue;
        }
        let status = object.get("lifecycle_status").and_then(|v| v.as_str()).unwrap_or("active");
        if status != "active" {
            continue;
        }
        object.insert("lifecycle_status".into(), json!("retired"));
        if let Some(id) = object.get("id").cloned() {
            retired_item_ids.push(id);
        }
    }

    // Reference quirk preserved verbatim: the id's prefix is hyphenated
    // (`source-retired-`) while the `action` field is underscored (`source_retired`).
    let event = json!({
        "id": format!("source-retired-{source_id}-{:06}", events.len() + 1),
        "action": "source_retired",
        "source_id": source_id,
        "source_version_id": active_version_id,
        "actor": actor,
        "timestamp": timestamp,
        "rationale": rationale,
        "previous_value": {"status": previous_status},
        "new_value": {"status": "retired", "retired_knowledge_item_ids": retired_item_ids},
    });

    let mut new_events: Vec<Value> = events.to_vec();
    new_events.push(event.clone());

    let sources_snapshot: Vec<Value> = sources.clone();
    Ok(RetirePlan::Retire(Box::new(RetireMutation {
        source_versions: json!({"versions": flatten_versions(&sources_snapshot)}),
        inventory,
        items,
        events: json!(new_events),
        event,
        sources: sources_snapshot,
    })))
}

/// Document-order flatten (no re-sort), matching what `retire_source` writes -- unlike
/// [`source_version_entries`], which `scan_sources` uses and which does sort.
fn flatten_versions(sources: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for source in sources {
        for version in source.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
            out.push(version.clone());
        }
    }
    out
}

/// Rewrites `manifest.yaml`'s `sources:` block from inventory records and stamps
/// `updated_at`. Each entry is `id` / `type` / `path` / `source_version_id` / `status`,
/// in that order -- the last two are new with source versioning (confirmed against
/// `fixtures/cases/*/expected-python/bundle/manifest.yaml`).
pub fn manifest_sources_from_records(records: &[Value]) -> Vec<sopkb_fmt::YamlValue> {
    records
        .iter()
        .map(|record| {
            let mut entry = sopkb_fmt::OrderedMap::new();
            let scalar = |v: Option<&Value>| sopkb_fmt::YamlValue::Scalar(v.and_then(|v| v.as_str()).unwrap_or("").to_string());
            entry.insert("id", scalar(record.get("id")));
            entry.insert("type", scalar(record.get("type")));
            entry.insert("path", scalar(record.get("original_path")));
            entry.insert("source_version_id", scalar(record.get("source_version_id")));
            entry.insert(
                "status",
                sopkb_fmt::YamlValue::Scalar(
                    record.get("status").and_then(|v| v.as_str()).unwrap_or("active").to_string(),
                ),
            );
            sopkb_fmt::YamlValue::Mapping(entry)
        })
        .collect()
}

/// Loads the manifest, replaces `sources:` and `updated_at`, and writes it back through
/// `transaction` when one is supplied (so the manifest participates in the same
/// all-or-nothing group as the state files) or directly when it is not.
pub fn update_manifest_sources(
    bundle_dir: &Path,
    records: &[Value],
    transaction: Option<&mut FileTransaction>,
) -> Result<()> {
    let mut manifest = store::load_manifest(bundle_dir)?;
    manifest.insert("sources", sopkb_fmt::YamlValue::Sequence(manifest_sources_from_records(records)));
    manifest.insert("updated_at", sopkb_fmt::YamlValue::Scalar(store::utc_now()));
    match transaction {
        Some(tx) => tx.save_manifest(bundle_dir, &manifest),
        None => store::save_manifest_raw(bundle_dir, &manifest),
    }
}

/// One `.sopkb/source_events.json` entry for an ingestion event, matching the shape in
/// `fixtures/cases/*/expected-python/bundle/.sopkb/source_events.json`.
///
/// The id embeds a 14-digit `YYYYMMDDhhmmss` stamp, so two events for the same source
/// and action within the same second collide on id. That is reference behavior and is
/// left as-is: the log is append-only and consumers key off position and `timestamp`,
/// not off id uniqueness. (The fixture harness normalizes this field for exactly that
/// reason -- see `fixtures/README.md`'s `RE_SOURCE_EVENT_ID`.)
pub fn source_event(action: &str, source_id: &str, source_version_id: &str, path: &Path, now: chrono::DateTime<chrono::Utc>) -> Value {
    let mut map = Map::new();
    map.insert("id".into(), json!(format!("{action}-{source_id}-{}", now.format("%Y%m%d%H%M%S"))));
    map.insert("action".into(), json!(action));
    map.insert("source_id".into(), json!(source_id));
    map.insert("source_version_id".into(), json!(source_version_id));
    map.insert("path".into(), json!(path.display().to_string()));
    map.insert("timestamp".into(), json!(now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)));
    map.insert("actor".into(), json!("sopkb/inventory"));
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn bundle() -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, Some("T")).unwrap();
        (dir, bundle_dir)
    }

    fn legacy_source() -> Value {
        json!({
            "id": "legacy-sop-abc123",
            "title": "Legacy SOP",
            "type": "markdown",
            "original_path": "sources/originals/legacy-sop-abc123.md",
            "normalized_path": "sources/normalized/legacy-sop-abc123.md",
            "checksum": "sha256:abc123",
            "size": 25,
            "modified_time": "2026-08-08T00:00:00Z",
            "parse_status": "normalized",
            "warnings": [],
            "metadata": {"source_filename": "legacy_sop.md"}
        })
    }

    /// Mirrors `tools/sopkb/tests/test_source_versioning_migration.py`'s single test,
    /// assertion for assertion.
    #[test]
    fn migration_adds_source_version_defaults_to_existing_bundle_state() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": [legacy_source()], "warnings": []}))
            .unwrap();
        store::write_state_json(
            &bundle_dir,
            "sections.json",
            &json!([{
                "id": "section-legacy-sop-abc123-001", "source_id": "legacy-sop-abc123",
                "heading": "Policy", "semantic_role": "policy", "start_pos": 0, "end_pos": 20,
                "normalized_path": "sources/normalized/legacy-sop-abc123.md"
            }]),
        )
        .unwrap();
        store::write_state_json(
            &bundle_dir,
            "items.json",
            &json!([{
                "id": "ki-legacy-sop-abc123-000001", "source_id": "legacy-sop-abc123",
                "section_id": "section-legacy-sop-abc123-001", "metadata": {}
            }]),
        )
        .unwrap();

        migrate_source_version_state(&bundle_dir).unwrap();

        let inventory = store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        let source = &inventory["sources"][0];
        assert_eq!(source["source_version_id"], "legacy-sop-abc123:v1");
        assert_eq!(source["active_version_id"], "legacy-sop-abc123:v1");
        assert_eq!(source["status"], "active");
        assert_eq!(source["versions"][0]["source_version_id"], "legacy-sop-abc123:v1");
        // Paths must NOT be rewritten to the new `__v1` scheme by a migration.
        assert_eq!(source["normalized_path"], "sources/normalized/legacy-sop-abc123.md");

        let sections = store::read_state_json(&bundle_dir, "sections.json", json!([])).unwrap();
        assert_eq!(sections[0]["source_version_id"], "legacy-sop-abc123:v1");

        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["source_version_id"], "legacy-sop-abc123:v1");
        assert_eq!(items[0]["lifecycle_status"], "active");

        let registry = store::read_state_json(&bundle_dir, SOURCE_VERSIONS_FILE, Value::Null).unwrap();
        assert_eq!(registry["versions"][0]["source_version_id"], "legacy-sop-abc123:v1");
        assert_eq!(registry["versions"][0]["size_bytes"], 25);
    }

    /// The reference implementation's `setdefault` write-back bug, as a regression
    /// test: an item that is complete EXCEPT for `supersedes`/`superseded_by` triggers
    /// no other mutation, so under the original code nothing set `item_changed` and the
    /// two added keys were silently discarded.
    #[test]
    fn migration_persists_setdefault_only_additions_to_items() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": [legacy_source()], "warnings": []}))
            .unwrap();
        store::write_state_json(
            &bundle_dir,
            "items.json",
            &json!([{
                "id": "ki-1",
                "source_id": "legacy-sop-abc123",
                // Already present, so neither of the two `if` branches above fires:
                "source_version_id": "legacy-sop-abc123:v1",
                "lifecycle_status": "active"
                // `supersedes` / `superseded_by` absent -- the only thing to add.
            }]),
        )
        .unwrap();

        migrate_source_version_state(&bundle_dir).unwrap();

        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["supersedes"], json!([]), "supersedes must be persisted, not dropped");
        assert_eq!(items[0]["superseded_by"], json!([]), "superseded_by must be persisted, not dropped");
    }

    #[test]
    fn migration_is_idempotent_and_leaves_a_migrated_bundle_byte_identical() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": [legacy_source()], "warnings": []}))
            .unwrap();
        migrate_source_version_state(&bundle_dir).unwrap();
        let after_first = fs::read(store::state_path(&bundle_dir, "inventory.json")).unwrap();
        migrate_source_version_state(&bundle_dir).unwrap();
        let after_second = fs::read(store::state_path(&bundle_dir, "inventory.json")).unwrap();
        assert_eq!(after_first, after_second);
    }

    #[test]
    fn migration_never_reactivates_a_retired_source() {
        let (_dir, bundle_dir) = bundle();
        let mut source = legacy_source();
        source["status"] = json!("retired");
        store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": [source], "warnings": []})).unwrap();
        migrate_source_version_state(&bundle_dir).unwrap();
        let inventory = store::read_state_json(&bundle_dir, "inventory.json", Value::Null).unwrap();
        assert_eq!(inventory["sources"][0]["status"], "retired");
    }

    #[test]
    fn migration_on_absent_or_malformed_inventory_is_a_no_op() {
        let (_dir, bundle_dir) = bundle();
        migrate_source_version_state(&bundle_dir).unwrap();
        assert!(!store::state_path(&bundle_dir, SOURCE_VERSIONS_FILE).exists());

        store::write_state_json(&bundle_dir, "inventory.json", &json!(["not", "a", "mapping"])).unwrap();
        migrate_source_version_state(&bundle_dir).unwrap();
        assert!(!store::state_path(&bundle_dir, SOURCE_VERSIONS_FILE).exists());
    }

    #[test]
    fn source_version_entries_sorts_by_source_then_version_number() {
        let sources = json!([
            {"versions": [{"source_id": "b", "version_number": 1}, {"source_id": "b", "version_number": 2}]},
            {"versions": [{"source_id": "a", "version_number": 2}, {"source_id": "a", "version_number": 1}]},
        ]);
        let entries = source_version_entries(sources.as_array().unwrap());
        let keys: Vec<(String, i64)> = entries
            .iter()
            .map(|e| (e["source_id"].as_str().unwrap().to_string(), e["version_number"].as_i64().unwrap()))
            .collect();
        assert_eq!(keys, vec![("a".into(), 1), ("a".into(), 2), ("b".into(), 1), ("b".into(), 2)]);
    }

    #[test]
    fn latest_version_number_considers_both_the_record_and_its_versions() {
        assert_eq!(latest_version_number(&json!({"version_number": 3})), 3);
        assert_eq!(latest_version_number(&json!({"versions": [{"version_number": 5}]})), 5);
        assert_eq!(latest_version_number(&json!({"version_number": 7, "versions": [{"version_number": 2}]})), 7);
        assert_eq!(latest_version_number(&json!({})), 1);
    }

    // --- FileTransaction -------------------------------------------------------

    #[test]
    fn rollback_restores_changed_files_and_deletes_created_ones() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "items.json", &json!([{"id": "before"}])).unwrap();
        let items_before = fs::read(store::state_path(&bundle_dir, "items.json")).unwrap();

        let mut tx = FileTransaction::new();
        tx.write_state_json(&bundle_dir, "items.json", &json!([{"id": "after"}])).unwrap();
        tx.write_state_json(&bundle_dir, SOURCE_EVENTS_FILE, &json!([{"id": "new"}])).unwrap();
        assert!(store::state_path(&bundle_dir, SOURCE_EVENTS_FILE).exists());

        tx.rollback().unwrap();

        assert_eq!(fs::read(store::state_path(&bundle_dir, "items.json")).unwrap(), items_before);
        assert!(
            !store::state_path(&bundle_dir, SOURCE_EVENTS_FILE).exists(),
            "a file the transaction created must be removed, not left empty"
        );
    }

    #[test]
    fn commit_keeps_writes() {
        let (_dir, bundle_dir) = bundle();
        let mut tx = FileTransaction::new();
        tx.write_state_json(&bundle_dir, "items.json", &json!([{"id": "kept"}])).unwrap();
        tx.commit();
        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["id"], "kept");
    }

    /// The RAII half: an error path that returns early without calling either
    /// `commit` or `rollback` still leaves nothing behind.
    #[test]
    fn dropping_an_uncommitted_transaction_rolls_back() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "items.json", &json!([{"id": "before"}])).unwrap();
        {
            let mut tx = FileTransaction::new();
            tx.write_state_json(&bundle_dir, "items.json", &json!([{"id": "after"}])).unwrap();
            // no commit, no rollback -- just drop
        }
        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["id"], "before");
    }

    #[test]
    fn snapshot_captures_only_the_first_state_of_a_repeatedly_written_path() {
        let (_dir, bundle_dir) = bundle();
        store::write_state_json(&bundle_dir, "items.json", &json!([{"id": "v0"}])).unwrap();
        let mut tx = FileTransaction::new();
        tx.write_state_json(&bundle_dir, "items.json", &json!([{"id": "v1"}])).unwrap();
        tx.write_state_json(&bundle_dir, "items.json", &json!([{"id": "v2"}])).unwrap();
        tx.rollback().unwrap();
        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["id"], "v0");
    }

    // --- plan_retire_source ----------------------------------------------------

    fn retirable_inventory() -> Value {
        json!({"sources": [{
            "id": "policy",
            "status": "active",
            "active_version_id": "policy:v1",
            "source_version_id": "policy:v1",
            "type": "markdown",
            "original_path": "sources/originals/policy__v1.md",
            "versions": [
                {"source_id": "policy", "source_version_id": "policy:v1", "version_number": 1, "status": "active"}
            ]
        }], "warnings": []})
    }

    #[test]
    fn plan_marks_source_versions_and_active_items_retired() {
        let items = json!([
            {"id": "ki-1", "source_id": "policy", "lifecycle_status": "active"},
            {"id": "ki-2", "source_id": "other", "lifecycle_status": "active"},
        ]);
        let plan =
            plan_retire_source(&retirable_inventory(), &items, &[], "policy", "test:user", "why", "2026-01-01T00:00:00Z")
                .unwrap();
        let RetirePlan::Retire(m) = plan else { panic!("expected a retire plan") };

        assert_eq!(m.inventory["sources"][0]["status"], "retired");
        assert_eq!(m.inventory["sources"][0]["versions"][0]["status"], "retired");
        assert_eq!(m.source_versions["versions"][0]["status"], "retired");
        assert_eq!(m.items[0]["lifecycle_status"], "retired");
        assert_eq!(m.items[1]["lifecycle_status"], "active", "another source's items are untouched");
        assert_eq!(m.event["action"], "source_retired");
        assert_eq!(m.event["id"], "source-retired-policy-000001");
        assert_eq!(m.event["previous_value"]["status"], "active");
        assert_eq!(m.event["new_value"]["retired_knowledge_item_ids"], json!(["ki-1"]));
        assert_eq!(m.events.as_array().unwrap().len(), 1);
    }

    #[test]
    fn plan_leaves_already_superseded_items_alone() {
        let items = json!([{"id": "ki-old", "source_id": "policy", "lifecycle_status": "superseded"}]);
        let plan =
            plan_retire_source(&retirable_inventory(), &items, &[], "policy", "a", "r", "2026-01-01T00:00:00Z").unwrap();
        let RetirePlan::Retire(m) = plan else { panic!("expected a retire plan") };
        assert_eq!(m.items[0]["lifecycle_status"], "superseded");
        assert_eq!(m.event["new_value"]["retired_knowledge_item_ids"], json!([]));
    }

    #[test]
    fn plan_on_an_already_retired_source_is_a_no_op_returning_the_original_event() {
        let mut inventory = retirable_inventory();
        inventory["sources"][0]["status"] = json!("retired");
        let existing = json!({"action": "source_retired", "source_id": "policy", "id": "source-retired-policy-000001"});
        let plan = plan_retire_source(
            &inventory,
            &json!([]),
            std::slice::from_ref(&existing),
            "policy",
            "a",
            "r",
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        match plan {
            RetirePlan::AlreadyRetired { existing_event } => assert_eq!(existing_event, Some(existing)),
            _ => panic!("expected AlreadyRetired"),
        }
    }

    #[test]
    fn plan_for_an_unknown_source_is_an_error_naming_it() {
        let err =
            plan_retire_source(&retirable_inventory(), &json!([]), &[], "nope", "a", "r", "2026-01-01T00:00:00Z")
                .unwrap_err();
        assert_eq!(err.to_string(), "source not found: nope");
    }

    #[test]
    fn plan_event_ordinal_follows_the_existing_log_length() {
        let events = vec![json!({"id": "source_added-policy-1"}), json!({"id": "source_unchanged-policy-2"})];
        let plan =
            plan_retire_source(&retirable_inventory(), &json!([]), &events, "policy", "a", "r", "2026-01-01T00:00:00Z")
                .unwrap();
        let RetirePlan::Retire(m) = plan else { panic!("expected a retire plan") };
        assert_eq!(m.event["id"], "source-retired-policy-000003");
        assert_eq!(m.events.as_array().unwrap().len(), 3, "the log is appended to, never rewritten");
    }
}
