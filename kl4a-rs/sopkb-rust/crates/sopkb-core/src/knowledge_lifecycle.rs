//! Port of `tools/sopkb/sopkb/knowledge_lifecycle.py`.
//!
//! When a source gets a new version, the items mined from the *previous* version must
//! not simply vanish -- a reviewer may have approved them, and an auditor may need to
//! see what the bundle asserted last month. [`merge_mined_items`] is the rule that
//! decides, for each pre-existing item, whether it is replaced, superseded, or left
//! alone, given the sections the current mining pass actually saw.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The string embedded in a knowledge item's id to tie it to a specific source
/// *version*: the `source_version_id` with `:` mapped to `-`, so `weird-headings:v1`
/// becomes `weird-headings-v1` and the item id reads `ki-weird-headings-v1-000001`
/// (verified against `fixtures/cases/weird-headings-md/expected-python/bundle/.sopkb/items.json`).
///
/// Falls back to the bare `source_id` for a section that predates versioning, which is
/// what makes an unmigrated bundle still minable.
pub fn item_source_key_for(section: &Value) -> String {
    match section.get("source_version_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.replace(':', "-"),
        _ => section.get("source_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    }
}

/// Merges a fresh mining pass's `new_items` into the bundle's `existing_items`, using
/// `sections` (what this pass actually mined) to decide each existing item's fate:
///
/// - **Dropped** if its `source_version_id` is one of the versions just mined -- the
///   new items for that exact version replace it outright, so keeping it would double
///   every claim.
/// - **Superseded** if its source was mined but at a *different* version, and it is
///   still `active`. It stays in `items.json` with `lifecycle_status: "superseded"` and
///   a `superseded_by` pointing at the new items from the same source.
/// - **Untouched** otherwise (a source this pass did not look at at all).
///
/// The new items get the reciprocal `supersedes` edges.
///
/// # Deviation from the reference implementation (deliberate)
///
/// The Python original requires `item_version` to be truthy before it will supersede,
/// so a pre-versioning item -- one with `source_version_id` absent or `null`, from a
/// bundle written before this subsystem existed -- was silently left `active` forever
/// while its replacement was added alongside it, double-counting the same claim.
/// CATCHUP_PLAN.md flags this as the third bug to fix rather than carry over.
///
/// It is fixed **structurally**: `None` and `Some("")` are normalized to the same
/// "no version" sentinel, which by construction is never a member of `current_versions`
/// (that set only ever contains non-empty ids), so a versionless item whose source was
/// mined takes the supersede branch instead of falling through it. In a fully migrated
/// bundle the case is unreachable anyway, because
/// [`crate::lifecycle::migrate_source_version_state`] back-fills `source_version_id` on
/// every item before mining runs -- so this is defence in depth for bundles that reach
/// mining by some path that skipped migration, not the primary guard.
pub fn merge_mined_items(existing_items: &[Value], new_items: &[Value], sections: &[Value]) -> Vec<Value> {
    let current_versions: BTreeSet<String> = sections
        .iter()
        .filter_map(|s| s.get("source_version_id").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let current_sources: BTreeSet<String> = sections
        .iter()
        .filter_map(|s| s.get("source_id").and_then(|v| v.as_str()))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut new_items: Vec<Value> = new_items.to_vec();
    let mut new_ids_by_source: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for item in new_items.iter_mut() {
        let Some(object) = item.as_object_mut() else { continue };
        object.entry("supersedes").or_insert_with(|| json!([]));
        object.entry("superseded_by").or_insert_with(|| json!([]));
        let source_id = object.get("source_id").and_then(|v| v.as_str()).unwrap_or("");
        let id = object.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if !source_id.is_empty() && !id.is_empty() {
            new_ids_by_source.entry(source_id.to_string()).or_default().push(json!(id));
        }
    }

    let mut merged: Vec<Value> = Vec::new();
    let mut superseded_ids_by_source: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for item in existing_items {
        let mut item = item.clone();
        let item_version =
            item.get("source_version_id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
        let item_source = item.get("source_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        // Same source AND same version: the fresh pass regenerated this exact item.
        if item_version.as_deref().is_some_and(|v| current_versions.contains(v)) {
            continue;
        }

        let source_was_mined = current_sources.contains(&item_source);
        let is_active = item.get("lifecycle_status").and_then(|v| v.as_str()).unwrap_or("active") == "active";
        if source_was_mined && is_active {
            if let Some(object) = item.as_object_mut() {
                object.insert("lifecycle_status".into(), json!("superseded"));
                object.insert(
                    "superseded_by".into(),
                    json!(new_ids_by_source.get(&item_source).cloned().unwrap_or_default()),
                );
            }
            if let Some(id) = item.get("id").cloned() {
                superseded_ids_by_source.entry(item_source).or_default().push(id);
            }
        }
        merged.push(item);
    }

    for item in new_items.iter_mut() {
        let Some(object) = item.as_object_mut() else { continue };
        let source_id = object.get("source_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut supersedes: Vec<Value> =
            object.get("supersedes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for id in superseded_ids_by_source.get(&source_id).into_iter().flatten() {
            if !supersedes.contains(id) {
                supersedes.push(id.clone());
            }
        }
        object.insert("supersedes".into(), json!(supersedes));
    }

    merged.extend(new_items);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(source_id: &str, version: Option<&str>) -> Value {
        match version {
            Some(v) => json!({"source_id": source_id, "source_version_id": v}),
            None => json!({"source_id": source_id}),
        }
    }

    fn item(id: &str, source_id: &str, version: Option<&str>, status: &str) -> Value {
        let mut object = serde_json::Map::new();
        object.insert("id".into(), json!(id));
        object.insert("source_id".into(), json!(source_id));
        object.insert("lifecycle_status".into(), json!(status));
        if let Some(v) = version {
            object.insert("source_version_id".into(), json!(v));
        }
        Value::Object(object)
    }

    #[test]
    fn item_source_key_maps_colon_to_hyphen_matching_fixture_item_ids() {
        assert_eq!(item_source_key_for(&section("weird-headings", Some("weird-headings:v1"))), "weird-headings-v1");
    }

    #[test]
    fn item_source_key_falls_back_to_source_id_without_a_version() {
        assert_eq!(item_source_key_for(&section("legacy", None)), "legacy");
        assert_eq!(item_source_key_for(&section("legacy", Some(""))), "legacy");
    }

    #[test]
    fn remining_the_same_version_replaces_rather_than_accumulates() {
        let sections = vec![section("policy", Some("policy:v1"))];
        let existing = vec![item("ki-old", "policy", Some("policy:v1"), "active")];
        let new = vec![item("ki-new", "policy", Some("policy:v1"), "active")];
        let merged = merge_mined_items(&existing, &new, &sections);
        let ids: Vec<&str> = merged.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["ki-new"]);
    }

    #[test]
    fn a_new_source_version_supersedes_the_previous_versions_items_both_ways() {
        let sections = vec![section("policy", Some("policy:v2"))];
        let existing = vec![item("ki-v1", "policy", Some("policy:v1"), "active")];
        let new = vec![item("ki-v2", "policy", Some("policy:v2"), "active")];
        let merged = merge_mined_items(&existing, &new, &sections);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0]["id"], "ki-v1");
        assert_eq!(merged[0]["lifecycle_status"], "superseded");
        assert_eq!(merged[0]["superseded_by"], json!(["ki-v2"]));
        assert_eq!(merged[1]["id"], "ki-v2");
        assert_eq!(merged[1]["supersedes"], json!(["ki-v1"]));
    }

    #[test]
    fn items_from_a_source_this_pass_never_looked_at_are_untouched() {
        let sections = vec![section("policy", Some("policy:v2"))];
        let existing = vec![item("ki-other", "handbook", Some("handbook:v1"), "active")];
        let merged = merge_mined_items(&existing, &[], &sections);
        assert_eq!(merged[0]["lifecycle_status"], "active");
        assert!(merged[0].get("superseded_by").is_none(), "an untouched item is copied verbatim");
    }

    #[test]
    fn a_retired_item_is_not_resurrected_into_superseded() {
        let sections = vec![section("policy", Some("policy:v2"))];
        let existing = vec![item("ki-retired", "policy", Some("policy:v1"), "retired")];
        let merged = merge_mined_items(&existing, &[], &sections);
        assert_eq!(merged[0]["lifecycle_status"], "retired");
    }

    /// The third CATCHUP_PLAN.md bug: under the reference implementation this item
    /// stayed `active` alongside its replacement, because its falsy
    /// `source_version_id` short-circuited the supersede check.
    #[test]
    fn a_pre_versioning_item_is_superseded_rather_than_left_active() {
        let sections = vec![section("policy", Some("policy:v1"))];
        let existing = vec![item("ki-legacy", "policy", None, "active")];
        let new = vec![item("ki-new", "policy", Some("policy:v1"), "active")];
        let merged = merge_mined_items(&existing, &new, &sections);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0]["id"], "ki-legacy");
        assert_eq!(merged[0]["lifecycle_status"], "superseded", "must not stay active next to its replacement");
        assert_eq!(merged[0]["superseded_by"], json!(["ki-new"]));
        assert_eq!(merged[1]["supersedes"], json!(["ki-legacy"]));
    }

    #[test]
    fn new_items_always_gain_both_lifecycle_edge_arrays() {
        let merged = merge_mined_items(&[], &[json!({"id": "ki-1", "source_id": "policy"})], &[]);
        assert_eq!(merged[0]["supersedes"], json!([]));
        assert_eq!(merged[0]["superseded_by"], json!([]));
    }
}
