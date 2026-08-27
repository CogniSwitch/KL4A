//! `agent_tools.py` pure reads. docs/port/port-mapping-d-agent-mcp.md §4.

use sopkb_core::error::{Result, SopkbError};
use sopkb_core::store;
use std::collections::BTreeMap;
use std::path::Path;

pub fn bundle_describe(bundle_dir: &Path) -> Result<serde_json::Value> {
    let manifest = store::load_manifest(bundle_dir)?;
    let get = |k: &str| manifest.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let inventory = store::read_state_json(bundle_dir, "inventory.json", serde_json::json!({"sources": []}))?;
    let items = store::read_state_json(bundle_dir, "items.json", serde_json::json!([]))?;
    let source_count = inventory["sources"].as_array().map(|a| a.len()).unwrap_or(0);
    let knowledge_item_count = items.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "id": get("id"),
        "title": get("title"),
        "profile": get("profile"),
        "status": get("status"),
        "source_count": source_count,
        "knowledge_item_count": knowledge_item_count,
    }))
}

pub fn sources_list(bundle_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let inventory = store::read_state_json(bundle_dir, "inventory.json", serde_json::json!({"sources": []}))?;
    Ok(inventory["sources"].as_array().cloned().unwrap_or_default())
}

pub fn sources_get(bundle_dir: &Path, source_id: &str) -> Result<serde_json::Value> {
    sources_list(bundle_dir)?
        .into_iter()
        .find(|s| s["id"] == source_id)
        .ok_or_else(|| SopkbError::Value(format!("source not found: {source_id}")))
}

/// Every section, enriched with `heading_level`/`ancestor_headings` -- see
/// `enrich_with_heading_ancestry`'s own doc comment for why this is safe to add here
/// without touching `sections.json` itself. `sections_get` calls this (not the raw
/// file read) so both commands get the enrichment from one place, not duplicated.
pub fn sections_list(bundle_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let sections = store::read_state_json(bundle_dir, "sections.json", serde_json::json!([]))?;
    let mut sections: Vec<serde_json::Value> = sections.as_array().cloned().unwrap_or_default();
    enrich_with_heading_ancestry(bundle_dir, &mut sections);
    Ok(sections)
}

pub fn sections_get(bundle_dir: &Path, section_id: &str) -> Result<serde_json::Value> {
    sections_list(bundle_dir)?
        .into_iter()
        .find(|s| s["id"] == section_id)
        .ok_or_else(|| SopkbError::Value(format!("section not found: {section_id}")))
}

/// Attaches `heading_level` (1-6) and `ancestor_headings` (outermost first, NOT
/// including the section's own heading) to every section that has them -- purely
/// additive agent/UI-facing metadata, NOT part of the stored `sections.json` shape
/// (which stays byte-for-byte pinned against Python's own CLI output,
/// `phase2_v1_diff.rs`; Python's own `SectionRecord`/`agent_tools.py` has no
/// equivalent field). This is the single shared place both `sopkb-mcp`'s tools and
/// the desktop app's Tauri read commands pick this up, since both call through
/// `sections_list`/`sections_get` above -- neither reimplements it.
///
/// Computed once per DISTINCT `normalized_path` among the given sections (typically
/// one per source, however many sections that source has), not once per section,
/// since `sopkb_core::normalize::heading_ancestors` re-scans the whole file each call.
/// A source whose normalized file can't be read (already gone, a mid-run race) is
/// left unenriched rather than failing the whole list -- this metadata is a nice-to-
/// have on top of the section data that already loaded successfully, not something
/// that should make an otherwise-fine read fail.
fn enrich_with_heading_ancestry(bundle_dir: &Path, sections: &mut [serde_json::Value]) {
    let mut cache: std::collections::HashMap<String, BTreeMap<usize, (u8, Vec<String>)>> = std::collections::HashMap::new();
    for section in sections.iter_mut() {
        let Some(rel_path) = section.get("normalized_path").and_then(|v| v.as_str()).map(str::to_string) else { continue };
        let Some(start_pos) = section.get("start_pos").and_then(|v| v.as_u64()).map(|v| v as usize) else { continue };
        let ancestry = cache.entry(rel_path.clone()).or_insert_with(|| {
            std::fs::read_to_string(bundle_dir.join(&rel_path)).map(|content| sopkb_core::normalize::heading_ancestors(&content)).unwrap_or_default()
        });
        if let Some((level, ancestors)) = ancestry.get(&start_pos) {
            if let Some(obj) = section.as_object_mut() {
                obj.insert("heading_level".to_string(), serde_json::json!(level));
                obj.insert("ancestor_headings".to_string(), serde_json::json!(ancestors));
            }
        }
    }
}

pub fn knowledge_items(bundle_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let items = store::read_state_json(bundle_dir, "items.json", serde_json::json!([]))?;
    Ok(items.as_array().cloned().unwrap_or_default())
}

pub fn knowledge_get(bundle_dir: &Path, item_id: &str) -> Result<serde_json::Value> {
    knowledge_items(bundle_dir)?
        .into_iter()
        .find(|i| i["id"] == item_id)
        .ok_or_else(|| SopkbError::Value(format!("knowledge item not found: {item_id}")))
}

/// `rule_ids` on a `knowledge_search` result: ids of *authored* decision rules only
/// (`item.metadata.decision_rules`), mirroring oss-launch's `agent_tools.py` list
/// comprehension exactly. Deliberately narrower than `decision_rules_for_item`
/// (`rules.rs`), which also synthesizes keyword-inferred rules for agent-context
/// evaluation -- a search result should only flag an item as rule-governed when a
/// human actually authored a rule, not when the keyword fallback would infer one.
fn authored_rule_ids(item: &serde_json::Value) -> Vec<serde_json::Value> {
    item.get("metadata")
        .and_then(|m| m.get("decision_rules"))
        .and_then(|d| d.as_array())
        .map(|rules| {
            rules
                .iter()
                .filter(|r| r.is_object())
                .filter_map(|r| r.get("id"))
                .filter(|id| id.as_str().map(|s| !s.is_empty()).unwrap_or(false))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// AND-of-substrings over a concatenated lowercased haystack (subject/predicate/object/
/// source_text). Empty query -> every item matches. Result key is `"evidence"`, NOT
/// `"source_text"` (a deliberate wire-shape quirk, not a typo).
pub fn knowledge_search(bundle_dir: &Path, query: &str) -> Result<Vec<serde_json::Value>> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).filter(|t| !t.is_empty()).collect();
    let mut results = Vec::new();
    for item in knowledge_items(bundle_dir)? {
        let haystack = format!(
            "{} {} {} {}",
            item["subject"].as_str().unwrap_or(""),
            item["predicate"].as_str().unwrap_or(""),
            item["object"].as_str().unwrap_or(""),
            item["source_text"].as_str().unwrap_or("")
        )
        .to_lowercase();
        if terms.iter().all(|t| haystack.contains(t.as_str())) {
            results.push(serde_json::json!({
                "id": item["id"],
                "subject": item["subject"],
                "predicate": item["predicate"],
                "object": item["object"],
                "review_status": item["review_status"],
                "source_id": item["source_id"],
                "section_id": item["section_id"],
                "evidence": item["source_text"],
                "rule_ids": authored_rule_ids(&item),
            }));
        }
    }
    Ok(results)
}

pub fn evidence_get(bundle_dir: &Path, item_id: &str) -> Result<serde_json::Value> {
    let item = knowledge_get(bundle_dir, item_id)?;
    Ok(serde_json::json!({
        "knowledge_item_id": item["id"],
        "source_id": item["source_id"],
        "section_id": item["section_id"],
        "source_text": item["source_text"],
        "span_status": item["span_status"],
        "start_pos": item["start_pos"],
        "end_pos": item["end_pos"],
    }))
}

pub fn conflicts_list(bundle_dir: &Path) -> String {
    let p = bundle_dir.join("reports").join("conflicts.md");
    store::read_text_universal_newlines(&p).unwrap_or_else(|_| "# Conflict Report\n\n- Not generated.\n".to_string())
}

pub fn freshness_check(bundle_dir: &Path) -> String {
    let p = bundle_dir.join("reports").join("freshness.md");
    store::read_text_universal_newlines(&p).unwrap_or_else(|_| "# Freshness Report\n\n- Not generated.\n".to_string())
}

/// Strips only a single leading `"evidence-"` prefix (so `"evidence-evidence-ki-x"` ->
/// `"evidence-ki-x"`, which then fails lookup -- not "fixed" to strip repeatedly).
pub fn citations_resolve(bundle_dir: &Path, citation_id: &str) -> Result<serde_json::Value> {
    let stripped = citation_id.strip_prefix("evidence-").unwrap_or(citation_id);
    evidence_get(bundle_dir, stripped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn conflicts_and_freshness_fall_back_to_not_generated() {
        let dir = tempdir().unwrap();
        assert_eq!(conflicts_list(dir.path()), "# Conflict Report\n\n- Not generated.\n");
        assert_eq!(freshness_check(dir.path()), "# Freshness Report\n\n- Not generated.\n");
    }

    #[test]
    fn knowledge_search_empty_query_matches_all() {
        let dir = tempdir().unwrap();
        store::create_bundle(&dir.path().join("b"), None).unwrap();
        let bundle_dir = dir.path().join("b");
        let items = serde_json::json!([{"id": "ki-1", "subject": "A", "predicate": "requires", "object": "B", "source_text": "C"}]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        let results = knowledge_search(&bundle_dir, "  ").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["evidence"], "C");
        assert_eq!(results[0]["rule_ids"], serde_json::json!([]), "no metadata.decision_rules -> empty, not the keyword-inferred fallback");
    }

    #[test]
    fn knowledge_search_rule_ids_only_reflects_authored_rules() {
        let dir = tempdir().unwrap();
        store::create_bundle(&dir.path().join("b"), None).unwrap();
        let bundle_dir = dir.path().join("b");
        let items = serde_json::json!([
            {
                "id": "ki-1", "subject": "A", "predicate": "requires", "object": "confirm patient identity",
                "source_text": "C",
            },
            {
                "id": "ki-2", "subject": "A", "predicate": "requires", "object": "authored case",
                "source_text": "D",
                "metadata": {"decision_rules": [{"id": "r-1", "label": "Authored rule"}, {"id": "r-2"}, {"not_an_object": true}]},
            },
        ]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        let results = knowledge_search(&bundle_dir, "").unwrap();
        // ki-1's object text would trip decision_rules_for_item's keyword fallback
        // ("confirm patient identity"), but knowledge_search must NOT surface that as
        // rule_ids -- only genuinely authored rules count here (see authored_rule_ids doc).
        let ki1 = results.iter().find(|r| r["id"] == "ki-1").unwrap();
        assert_eq!(ki1["rule_ids"], serde_json::json!([]));
        let ki2 = results.iter().find(|r| r["id"] == "ki-2").unwrap();
        assert_eq!(ki2["rule_ids"], serde_json::json!(["r-1", "r-2"]));
    }

    #[test]
    fn citations_resolve_strips_single_prefix_only() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let items = serde_json::json!([{"id": "ki-1", "subject": "A", "predicate": "P", "object": "O", "source_id": "s", "section_id": "sec", "source_text": "T", "span_status": "exact", "start_pos": 0, "end_pos": 1}]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        let resolved = citations_resolve(&bundle_dir, "evidence-ki-1").unwrap();
        assert_eq!(resolved["knowledge_item_id"], "ki-1");
        let err = citations_resolve(&bundle_dir, "evidence-evidence-ki-1").unwrap_err();
        assert!(err.to_string().contains("knowledge item not found"));
    }
}
