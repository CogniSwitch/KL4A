//! OKF frontmatter builder: `okf_metadata`, `okf_source_ref`, `okf_status_for_review`,
//! `frontmatter`/`write_okf_doc`, and the JSON->YAML value converters used to embed
//! `sopkb.rule`/`sopkb.relation` verbatim. docs/port/port-mapping-c-review-export.md §3.6.
//!
//! **Key insertion order is the file format** (§3.6, G13): every metadata map here is
//! built with `OrderedMap`, never a plain hash map, and fields are inserted in the
//! exact order the Python source declares them.
//!
//! Two JSON->YAML conversion strategies coexist, and mixing them up silently produces
//! alphabetically-reordered frontmatter that only shows up as a diff against a real
//! bundle:
//!
//! - **Fixed-order builders** (`relation_to_yaml`, the synthetic-rule path of
//!   `decision_rule_to_yaml`, `condition_to_yaml`, `obligation_to_yaml`,
//!   `otherwise_to_yaml`) for structures Python builds FRESH in memory with an
//!   explicit dict literal order every call -- that literal order is NOT recoverable
//!   from a `serde_json::Value` (this workspace's `serde_json::Map` is BTreeMap-backed,
//!   see `sopkb_fmt::json_canonical`'s module docs, so iterating one always yields
//!   alphabetical order regardless of what order the `json!{}` macro listed fields in).
//! - **Generic `json_to_yaml`** (alphabetical object iteration) for structures Python
//!   reads verbatim from a `.sopkb/*.json` file and never rebuilds -- e.g. an authored
//!   rule's pre-existing keys. This is safe specifically because EVERY state file is
//!   always written by `write_json(sort_keys=True)`, so by the time Python reads it
//!   back, the dict's "insertion order" (which `json.load` preserves) already equals
//!   alphabetical order. `serde_json`'s BTreeMap iteration reproduces that same
//!   alphabetical order independently, so the two agree without any special handling.

use sopkb_core::store;
use sopkb_fmt::{emit_yaml, OrderedMap, YamlValue};
use std::path::Path;

pub const OKF_VERSION: &str = "0.2";
pub const GENERATOR_ACTOR: &str = "sopkb/0.1.0";

fn scalar(v: &serde_json::Value, key: &str) -> YamlValue {
    YamlValue::Scalar(v[key].as_str().unwrap_or("").to_string())
}

/// Recursive JSON->YAML conversion for values read verbatim from a `.sopkb/*.json`
/// state file (never for values Python constructs fresh with an explicit field order
/// -- see module docs). Object key order is alphabetical (this crate's `serde_json`
/// build has no `preserve_order` feature), which matches Python's real observed order
/// for anything round-tripped through `write_json(sort_keys=True)`.
pub fn json_to_yaml(v: &serde_json::Value) -> YamlValue {
    match v {
        serde_json::Value::Null => YamlValue::Raw("null".to_string()),
        serde_json::Value::Bool(b) => YamlValue::Raw(if *b { "true" } else { "false" }.to_string()),
        serde_json::Value::Number(n) => YamlValue::Raw(format_json_number(n)),
        serde_json::Value::String(s) => YamlValue::Scalar(s.clone()),
        serde_json::Value::Array(a) => YamlValue::Sequence(a.iter().map(json_to_yaml).collect()),
        serde_json::Value::Object(o) => {
            let mut m = OrderedMap::new();
            for (k, val) in o.iter() {
                m.insert(k.clone(), json_to_yaml(val));
            }
            YamlValue::Mapping(m)
        }
    }
}

fn format_json_number(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else {
        sopkb_fmt::py_float(n.as_f64().unwrap_or(0.0))
    }
}

/// `okf_metadata(doc_type, title, description, resource, tags, sources) -> ordered map`
/// (§3.6). `sources` is inserted only when non-empty (Python's `if sources:` truthy
/// check) -- callers that never pass sources (source/concept/task/reference docs)
/// simply pass `None`.
pub fn okf_metadata(
    doc_type: &str,
    title: &str,
    description: &str,
    resource: &str,
    tags: &[&str],
    sources: Option<Vec<YamlValue>>,
) -> OrderedMap<YamlValue> {
    let mut m = OrderedMap::new();
    m.insert("type", YamlValue::Scalar(doc_type.to_string()));
    m.insert("title", YamlValue::Scalar(title.to_string()));
    m.insert("description", YamlValue::Scalar(description.to_string()));
    m.insert("resource", YamlValue::Scalar(resource.to_string()));
    m.insert("tags", YamlValue::Sequence(tags.iter().map(|t| YamlValue::Scalar((*t).to_string())).collect()));
    m.insert("status", YamlValue::Scalar("stable".to_string()));
    let mut generated = OrderedMap::new();
    generated.insert("actor", YamlValue::Scalar(GENERATOR_ACTOR.to_string()));
    generated.insert("date", YamlValue::Scalar(store::today()));
    m.insert("generated", YamlValue::Mapping(generated));
    if let Some(sources) = sources {
        if !sources.is_empty() {
            m.insert("sources", YamlValue::Sequence(sources));
        }
    }
    m
}

/// `okf_source_ref(from_rel_path, source) = {id: "src-"+source.id, title: source.title,
/// resource: relative_link(from_rel_path, source_doc_path(source))}` (§3.6).
pub fn okf_source_ref(from_rel_path: &str, source: &serde_json::Value) -> YamlValue {
    let source_id = source["id"].as_str().unwrap_or("");
    let mut m = OrderedMap::new();
    m.insert("id", YamlValue::Scalar(format!("src-{source_id}")));
    m.insert("title", scalar(source, "title"));
    m.insert("resource", YamlValue::Scalar(crate::okf_paths::relative_link(from_rel_path, &crate::okf_paths::source_doc_path(source))));
    YamlValue::Mapping(m)
}

/// `"deprecated"` for rejected, `"draft"` for proposed/deferred, `"stable"` otherwise
/// (approved, edited, or any unrecognized status) (§3.6).
pub fn okf_status_for_review(review_status: &str) -> &'static str {
    match review_status {
        "rejected" => "deprecated",
        "proposed" | "deferred" => "draft",
        _ => "stable",
    }
}

/// `"---\n" + yaml.safe_dump(metadata, sort_keys=False, allow_unicode=False) + "---\n" + body`.
pub fn frontmatter(metadata: &OrderedMap<YamlValue>, body: &str) -> String {
    format!("---\n{}---\n{}", emit_yaml(&YamlValue::Mapping(metadata.clone())), body)
}

/// `write_okf_doc(export_dir, rel_path, metadata, body)`: mkdir -p the parent, write
/// through `write_text_native` (CRLF translation on Windows, matching Python's default
/// text-mode `Path.write_text`).
pub fn write_okf_doc(export_dir: &Path, rel_path: &str, metadata: &OrderedMap<YamlValue>, body: &str) -> sopkb_core::error::Result<()> {
    store::write_text_native(&export_dir.join(rel_path), &frontmatter(metadata, body))
}

// --- fixed-order builders for freshly-constructed (never disk-read) structures --------

/// `knowledge_relation_for_item(item)` re-assembled in Python's exact declared field
/// order (id, type, subject{id,label,text,okf_path}, predicate{id,text},
/// object{id,text,label}, knowledge_piece_id, evidence_id, review_status, confidence,
/// rdf_compatible) -- port-mapping-c §0.6. The VALUES come from
/// `sopkb_derive::relations::knowledge_relation_for_item`, which is correct; only its
/// `serde_json::Value` iteration order is untrustworthy (see module docs), so this
/// rebuilds the map field-by-field rather than iterating it.
pub fn relation_to_yaml(item: &serde_json::Value) -> YamlValue {
    let relation = sopkb_derive::relations::knowledge_relation_for_item(item);
    let mut m = OrderedMap::new();
    m.insert("id", scalar(&relation, "id"));
    m.insert("type", scalar(&relation, "type"));
    m.insert("subject", concept_to_yaml(&relation["subject"]));
    let mut predicate = OrderedMap::new();
    predicate.insert("id", scalar(&relation["predicate"], "id"));
    predicate.insert("text", scalar(&relation["predicate"], "text"));
    m.insert("predicate", YamlValue::Mapping(predicate));
    let mut object = OrderedMap::new();
    object.insert("id", scalar(&relation["object"], "id"));
    object.insert("text", scalar(&relation["object"], "text"));
    object.insert("label", scalar(&relation["object"], "label"));
    m.insert("object", YamlValue::Mapping(object));
    m.insert("knowledge_piece_id", scalar(&relation, "knowledge_piece_id"));
    m.insert("evidence_id", scalar(&relation, "evidence_id"));
    m.insert("review_status", scalar(&relation, "review_status"));
    m.insert("confidence", json_to_yaml(&relation["confidence"]));
    m.insert("rdf_compatible", YamlValue::Raw("true".to_string()));
    YamlValue::Mapping(m)
}

/// `concept_for_label(...)` in its declared order: id, label, text, okf_path.
fn concept_to_yaml(v: &serde_json::Value) -> YamlValue {
    let mut m = OrderedMap::new();
    m.insert("id", scalar(v, "id"));
    m.insert("label", scalar(v, "label"));
    m.insert("text", scalar(v, "text"));
    m.insert("okf_path", scalar(v, "okf_path"));
    YamlValue::Mapping(m)
}

fn condition_to_yaml(v: &serde_json::Value) -> YamlValue {
    if v.is_null() {
        return YamlValue::Raw("null".to_string());
    }
    let mut m = OrderedMap::new();
    m.insert("fact", scalar(v, "fact"));
    m.insert("operator", scalar(v, "operator"));
    m.insert("label", scalar(v, "label"));
    YamlValue::Mapping(m)
}

fn obligation_to_yaml(v: &serde_json::Value) -> YamlValue {
    let mut m = OrderedMap::new();
    m.insert("fact", scalar(v, "fact"));
    m.insert("action", scalar(v, "action"));
    m.insert("label", scalar(v, "label"));
    YamlValue::Mapping(m)
}

fn otherwise_to_yaml(v: &serde_json::Value) -> YamlValue {
    let action_required = v.get("action_required").and_then(|b| b.as_bool()).unwrap_or(true);
    let mut m = OrderedMap::new();
    m.insert("action_required", YamlValue::Raw(if action_required { "true" } else { "false" }.to_string()));
    m.insert("label", scalar(v, "label"));
    YamlValue::Mapping(m)
}

/// The declared field order for a SYNTHETIC (non-authored) decision rule (§0.6): id,
/// type, title, knowledge_item_id, source_id, section_id, review_status, confidence,
/// condition, obligation, evidence_id, relation_id, okf_path[, otherwise].
fn synthetic_rule_to_yaml(rule: &serde_json::Value) -> YamlValue {
    let mut m = OrderedMap::new();
    m.insert("id", scalar(rule, "id"));
    m.insert("type", scalar(rule, "type"));
    m.insert("title", scalar(rule, "title"));
    m.insert("knowledge_item_id", scalar(rule, "knowledge_item_id"));
    m.insert("source_id", scalar(rule, "source_id"));
    m.insert("section_id", scalar(rule, "section_id"));
    m.insert("review_status", scalar(rule, "review_status"));
    m.insert("confidence", json_to_yaml(&rule["confidence"]));
    m.insert("condition", condition_to_yaml(&rule["condition"]));
    m.insert("obligation", obligation_to_yaml(&rule["obligation"]));
    m.insert("evidence_id", scalar(rule, "evidence_id"));
    m.insert("relation_id", scalar(rule, "relation_id"));
    m.insert("okf_path", scalar(rule, "okf_path"));
    if let Some(otherwise) = rule.get("otherwise") {
        m.insert("otherwise", otherwise_to_yaml(otherwise));
    }
    YamlValue::Mapping(m)
}

/// Fixed set of fields `normalize_authored_rule` fills in ONLY when absent, in the
/// exact order it checks them (§0.6) -- NOT alphabetical, so it must stay a literal
/// list rather than being derived. `condition`/`obligation` are deliberately absent:
/// authored rules never get those defaulted.
const AUTHORED_DEFAULT_FILL_ORDER: &[&str] = &[
    "id",
    "type",
    "title",
    "knowledge_item_id",
    "source_id",
    "section_id",
    "review_status",
    "confidence",
    "evidence_id",
    "relation_id",
    "okf_path",
];

/// An AUTHORED rule (`item.metadata.decision_rules`, non-dict entries already
/// filtered out by the caller): starts from the raw pre-existing keys in their
/// on-disk (alphabetical) order, then appends any of `AUTHORED_DEFAULT_FILL_ORDER`
/// that were absent, pulling the now-defaulted value from `normalized` (the already
/// -computed `decision_rules_for_item` output, which has the correct default VALUES
/// even though its own key order is untrustworthy).
fn authored_rule_to_yaml(normalized: &serde_json::Value, raw: &serde_json::Value) -> YamlValue {
    let mut m = OrderedMap::new();
    if let Some(obj) = raw.as_object() {
        for (k, v) in obj.iter() {
            m.insert(k.clone(), json_to_yaml(v));
        }
    }
    for key in AUTHORED_DEFAULT_FILL_ORDER {
        if !m.contains_key(key) {
            m.insert(*key, json_to_yaml(&normalized[*key]));
        }
    }
    YamlValue::Mapping(m)
}

/// Dispatches to the synthetic or authored builder. `raw_authored` is `Some(&raw
/// dict)` when this rule came from `item.metadata.decision_rules` (the caller is
/// responsible for pairing each `decision_rules_for_item` output entry with its
/// matching raw dict-typed source entry, in order -- see `decision_rules_for_item`'s
/// `filter(is_object).map(normalize)` in the same pass).
pub fn decision_rule_to_yaml(rule: &serde_json::Value, raw_authored: Option<&serde_json::Value>) -> YamlValue {
    match raw_authored {
        Some(raw) => authored_rule_to_yaml(rule, raw),
        None => synthetic_rule_to_yaml(rule),
    }
}

/// Pairs each `decision_rules_for_item(item)` output with the raw authored dict it
/// came from, if any (mirrors the exact filter Phase 4's `decision_rules_for_item`
/// applies: `item.metadata.decision_rules` entries that are JSON objects, in order).
/// When the item has no (or an empty) authored list, every entry pairs with `None`
/// (synthetic keyword-derived rules).
pub fn decision_rules_with_raw(item: &serde_json::Value) -> Vec<(serde_json::Value, Option<serde_json::Value>)> {
    let normalized = sopkb_derive::rules::decision_rules_for_item(item);
    let authored_raw: Vec<serde_json::Value> = item
        .get("metadata")
        .and_then(|m| m.get("decision_rules"))
        .and_then(|d| d.as_array())
        .filter(|a| !a.is_empty())
        .map(|a| a.iter().filter(|r| r.is_object()).cloned().collect())
        .unwrap_or_default();
    if authored_raw.is_empty() {
        normalized.into_iter().map(|r| (r, None)).collect()
    } else {
        normalized.into_iter().zip(authored_raw).map(|(r, raw)| (r, Some(raw))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn okf_metadata_field_order() {
        let m = okf_metadata("SOP Source", "Title", "Desc", "res.md", &["a", "b"], None);
        let keys: Vec<&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["type", "title", "description", "resource", "tags", "status", "generated"]);
    }

    #[test]
    fn okf_metadata_sources_inserted_only_when_present() {
        let m = okf_metadata("X", "T", "D", "r", &[], Some(vec![YamlValue::Scalar("x".into())]));
        assert!(m.contains_key("sources"));
        let m2 = okf_metadata("X", "T", "D", "r", &[], None);
        assert!(!m2.contains_key("sources"));
    }

    #[test]
    fn status_override_preserves_position() {
        let mut m = okf_metadata("X", "T", "D", "r", &[], None);
        m.insert("status", YamlValue::Scalar("draft".to_string()));
        let keys: Vec<&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["type", "title", "description", "resource", "tags", "status", "generated"]);
        assert_eq!(m.get("status").unwrap().as_str(), Some("draft"));
    }

    #[test]
    fn synthetic_rule_field_order() {
        let item = json!({
            "id": "ki-1", "subject": "S", "predicate": "P",
            "object": "Staff must confirm patient identity before proceeding.",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8
        });
        let pairs = decision_rules_with_raw(&item);
        assert_eq!(pairs.len(), 1);
        let yaml = decision_rule_to_yaml(&pairs[0].0, pairs[0].1.as_ref());
        let keys: Vec<&str> = yaml.as_mapping().unwrap().iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            vec![
                "id",
                "type",
                "title",
                "knowledge_item_id",
                "source_id",
                "section_id",
                "review_status",
                "confidence",
                "condition",
                "obligation",
                "evidence_id",
                "relation_id",
                "okf_path",
            ]
        );
    }

    #[test]
    fn authored_rule_raw_keys_alphabetical_then_defaults_appended() {
        let item = json!({
            "id": "ki-1", "subject": "S", "predicate": "P", "object": "x",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8,
            "metadata": {"decision_rules": [{"title": "Custom", "zzz_extra": "kept"}]}
        });
        let pairs = decision_rules_with_raw(&item);
        assert_eq!(pairs.len(), 1);
        assert!(pairs[0].1.is_some());
        let yaml = decision_rule_to_yaml(&pairs[0].0, pairs[0].1.as_ref());
        let keys: Vec<&str> = yaml.as_mapping().unwrap().iter().map(|(k, _)| k).collect();
        // "title" and "zzz_extra" are the raw (alphabetical) pre-existing keys; the
        // rest are appended defaults in AUTHORED_DEFAULT_FILL_ORDER.
        assert_eq!(
            keys,
            vec![
                "title",
                "zzz_extra",
                "id",
                "type",
                "knowledge_item_id",
                "source_id",
                "section_id",
                "review_status",
                "confidence",
                "evidence_id",
                "relation_id",
                "okf_path",
            ]
        );
        assert_eq!(yaml.as_mapping().unwrap().get("title").unwrap().as_str(), Some("Custom"));
    }

    #[test]
    fn relation_field_order() {
        let item = json!({
            "id": "ki-1", "subject": "S", "predicate": "P", "object": "o",
            "review_status": "proposed", "confidence": 0.8
        });
        let yaml = relation_to_yaml(&item);
        let keys: Vec<&str> = yaml.as_mapping().unwrap().iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            vec![
                "id",
                "type",
                "subject",
                "predicate",
                "object",
                "knowledge_piece_id",
                "evidence_id",
                "review_status",
                "confidence",
                "rdf_compatible",
            ]
        );
    }
}
