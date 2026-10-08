//! Record structs. docs/port/port-mapping-a-core-data.md §3.2.
//!
//! `Manifest`/`ManifestSourceEntry`/`ManifestExportEntry` are YAML-only (field order
//! matters: `yaml.safe_dump(sort_keys=False)` emits insertion/declaration order) and
//! carry their own `to_yaml_value()`. The other four types are JSON-only
//! (`write_json` uses `sort_keys=True`, so output order is always alphabetical
//! regardless of struct field order) and derive `Serialize` -- convert with
//! `serde_json::to_value` and hand the result to `sopkb_fmt::to_canonical_json`.
//!
//! There is deliberately no `Deserialize` on any of these: nothing in the real
//! pipeline ever constructs one of these from JSON. Every reader works on untyped
//! `serde_json::Value` with `.get()`-style tolerance (see Gotcha G-A14 in the port
//! mapping doc) -- a Rust port that added strict typed deserialization would reject
//! bundles the Python code tolerates.

use serde::Serialize;
use sopkb_fmt::{OrderedMap, YamlValue};

#[derive(Debug, Clone)]
pub struct ManifestSourceEntry {
    pub id: String,
    pub source_type: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ManifestExportEntry {
    pub export_type: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub id: String,
    pub version: String,
    pub title: String,
    pub profile: String,
    pub profile_version: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub sources: Vec<ManifestSourceEntry>,
    pub exports: Vec<ManifestExportEntry>,
    pub okf_version: String,
}

impl Manifest {
    /// Field-declaration order, matching Python `dataclasses.asdict` insertion order,
    /// which survives into `yaml.safe_dump(sort_keys=False)` output -- `okf_version`
    /// lands LAST, after `exports`.
    pub fn to_yaml_value(&self) -> YamlValue {
        let mut map = OrderedMap::new();
        map.insert("id", YamlValue::Scalar(self.id.clone()));
        map.insert("version", YamlValue::Scalar(self.version.clone()));
        map.insert("title", YamlValue::Scalar(self.title.clone()));
        map.insert("profile", YamlValue::Scalar(self.profile.clone()));
        map.insert("profile_version", YamlValue::Scalar(self.profile_version.clone()));
        map.insert("status", YamlValue::Scalar(self.status.clone()));
        map.insert("created_at", YamlValue::Scalar(self.created_at.clone()));
        map.insert("updated_at", YamlValue::Scalar(self.updated_at.clone()));
        map.insert(
            "sources",
            YamlValue::Sequence(
                self.sources
                    .iter()
                    .map(|s| {
                        let mut m = OrderedMap::new();
                        m.insert("id", YamlValue::Scalar(s.id.clone()));
                        m.insert("type", YamlValue::Scalar(s.source_type.clone()));
                        m.insert("path", YamlValue::Scalar(s.path.clone()));
                        YamlValue::Mapping(m)
                    })
                    .collect(),
            ),
        );
        map.insert(
            "exports",
            YamlValue::Sequence(
                self.exports
                    .iter()
                    .map(|e| {
                        let mut m = OrderedMap::new();
                        m.insert("type", YamlValue::Scalar(e.export_type.clone()));
                        m.insert("path", YamlValue::Scalar(e.path.clone()));
                        YamlValue::Mapping(m)
                    })
                    .collect(),
            ),
        );
        map.insert("okf_version", YamlValue::Scalar(self.okf_version.clone()));
        YamlValue::Mapping(map)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRecord {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub original_path: String,
    /// `Optional[str]` in the Python type annotation but `scan_sources` always sets
    /// it (before the normalized file exists).
    pub normalized_path: Option<String>,
    pub checksum: String,
    pub size: u64,
    pub modified_time: String,
    pub parse_status: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    /// Source-versioning fields (CATCHUP_PLAN.md workstream 2). All five are emitted
    /// unconditionally -- `None` serializes as JSON `null`, matching the Python
    /// dataclass's `asdict()`, which never omits a field just because it is `None`.
    pub source_version_id: Option<String>,
    pub version_number: u32,
    /// `"active"` | `"retired"`. Source-level; individual *versions* additionally use
    /// `"superseded"` (see [`SourceVersion::status`]).
    pub status: String,
    pub active_version_id: Option<String>,
    pub versions: Vec<SourceVersion>,
}

/// One entry in a source's version history. Also the element type of
/// `.sopkb/source_versions.json`'s `"versions"` array -- the registry is a flattened,
/// re-sorted view over exactly these, never an independently-maintained list, so the
/// two can't drift.
///
/// Note `size_bytes` here vs. [`SourceRecord::size`] on the record: two different key
/// names for the same number, which is a quirk of the reference implementation
/// reproduced verbatim because both names are load-bearing in on-disk fixture data.
#[derive(Debug, Clone, Serialize)]
pub struct SourceVersion {
    pub source_id: String,
    pub source_version_id: String,
    pub version_number: u32,
    pub checksum: String,
    /// `"active"` | `"superseded"` | `"retired"`.
    pub status: String,
    pub original_path: String,
    pub normalized_path: String,
    pub size_bytes: u64,
    pub modified_time: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SectionRecord {
    pub id: String,
    pub source_id: String,
    pub heading: String,
    pub semantic_role: String,
    pub start_pos: usize,
    pub end_pos: usize,
    pub normalized_path: String,
    /// Which *version* of `source_id` this section was segmented out of. `None` only
    /// for sections written by a pre-versioning engine and not yet migrated.
    pub source_version_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeItem {
    pub id: String,
    pub item_type: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source_id: String,
    pub section_id: String,
    pub source_text: String,
    pub start_pos: Option<usize>,
    pub end_pos: Option<usize>,
    pub span_status: String,
    pub derivation: String,
    pub confidence: f64,
    pub review_status: String,
    #[serde(default)]
    pub metadata: serde_json::Map<String, serde_json::Value>,
    /// Knowledge-lifecycle fields (CATCHUP_PLAN.md workstream 2).
    pub source_version_id: Option<String>,
    /// `"active"` | `"superseded"` | `"retired"` | `"conflicted"`. Orthogonal to
    /// `review_status`: an item can be `approved` *and* `superseded`, which is exactly
    /// what happens when a reviewed source gets a new version.
    pub lifecycle_status: String,
    /// Ids of items THIS item replaces (populated when a new source version's mining
    /// output supersedes the previous version's items).
    pub supersedes: Vec<String>,
    /// The inverse edge: ids of the items that replaced this one.
    pub superseded_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewEvent {
    pub id: String,
    pub knowledge_item_id: String,
    pub action: String,
    pub reviewer: String,
    pub timestamp: String,
    pub rationale: String,
    #[serde(default)]
    pub previous_value: serde_json::Value,
    #[serde(default)]
    pub new_value: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_yaml_field_order_puts_okf_version_last() {
        let m = Manifest {
            id: "bundle".into(),
            version: "0.1.0".into(),
            title: "Demo".into(),
            profile: "sop-knowledge-bundle".into(),
            profile_version: "0.2.0".into(),
            status: "draft".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            sources: vec![],
            exports: vec![],
            okf_version: "0.2".into(),
        };
        let yaml_value = m.to_yaml_value();
        let keys: Vec<&str> = yaml_value.as_mapping().unwrap().iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            vec![
                "id",
                "version",
                "title",
                "profile",
                "profile_version",
                "status",
                "created_at",
                "updated_at",
                "sources",
                "exports",
                "okf_version",
            ]
        );
    }

    fn sample_record() -> SourceRecord {
        SourceRecord {
            id: "foo".into(),
            title: "Foo".into(),
            source_type: "markdown".into(),
            original_path: "sources/originals/foo__v1.md".into(),
            normalized_path: Some("sources/normalized/foo__v1.md".into()),
            checksum: "sha256:abc123".into(),
            size: 42,
            modified_time: "2026-01-01T00:00:00Z".into(),
            parse_status: "pending".into(),
            warnings: vec![],
            metadata: serde_json::Map::new(),
            source_version_id: Some("foo:v1".into()),
            version_number: 1,
            status: "active".into(),
            active_version_id: Some("foo:v1".into()),
            versions: vec![SourceVersion {
                source_id: "foo".into(),
                source_version_id: "foo:v1".into(),
                version_number: 1,
                checksum: "sha256:abc123".into(),
                status: "active".into(),
                original_path: "sources/originals/foo__v1.md".into(),
                normalized_path: "sources/normalized/foo__v1.md".into(),
                size_bytes: 42,
                modified_time: "2026-01-01T00:00:00Z".into(),
            }],
        }
    }

    #[test]
    fn source_record_serializes_type_field_correctly() {
        let value = serde_json::to_value(sample_record()).unwrap();
        assert_eq!(value["type"], "markdown");
        assert!(value.get("source_type").is_none());
    }

    /// Locks the exact key set `.sopkb/inventory.json` entries carry, against real
    /// regenerated fixture data
    /// (`fixtures/cases/weird-headings-md/expected-python/bundle/.sopkb/inventory.json`).
    #[test]
    fn source_record_key_set_matches_regenerated_fixture_inventory() {
        let value = serde_json::to_value(sample_record()).unwrap();
        let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "active_version_id",
                "checksum",
                "id",
                "metadata",
                "modified_time",
                "normalized_path",
                "original_path",
                "parse_status",
                "size",
                "source_version_id",
                "status",
                "title",
                "type",
                "version_number",
                "versions",
                "warnings",
            ]
        );
    }

    /// Same, for `.sopkb/source_versions.json`'s `"versions"` elements.
    #[test]
    fn source_version_key_set_matches_regenerated_fixture_registry() {
        let value = serde_json::to_value(&sample_record().versions[0]).unwrap();
        let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "checksum",
                "modified_time",
                "normalized_path",
                "original_path",
                "size_bytes",
                "source_id",
                "source_version_id",
                "status",
                "version_number",
            ]
        );
    }
}
