//! `export_graph_json` + `dedupe_graph`. docs/port/port-mapping-c-review-export.md §3.21.

use crate::bundle_export::export_path_for_manifest;
use sopkb_core::error::Result;
use sopkb_core::store;
use sopkb_derive::concepts::concept_for_label;
use sopkb_derive::relations::{evidence_id_for_item, knowledge_relation_for_item};
use sopkb_derive::rules::decision_rules_for_item;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// `export_graph_json(bundle_dir, export_dir) -> dict` (§3.21). Nodes/edges have NO
/// referential integrity by design (G3): `relation_subject` edges point at
/// `concept-<slugify(item.subject)>`, which is only a real node if some SECTION
/// HEADING happens to equal that subject (concept nodes here come from headings, not
/// item subjects -- G20 covers the opposite population in `write_concept_docs`).
pub fn export_graph_json(bundle_dir: &Path, export_dir: &Path) -> Result<Value> {
    let inventory = store::read_state_json(bundle_dir, "inventory.json", json!({"sources": []}))?;
    let sections: Vec<Value> = store::read_state_json(bundle_dir, "sections.json", json!([]))?.as_array().cloned().unwrap_or_default();
    let items: Vec<Value> = store::read_state_json(bundle_dir, "items.json", json!([]))?.as_array().cloned().unwrap_or_default();
    let reviews: Vec<Value> = store::read_state_json(bundle_dir, "reviews.json", json!([]))?.as_array().cloned().unwrap_or_default();
    let sources: Vec<Value> = inventory.get("sources").and_then(|v| v.as_array()).cloned().unwrap_or_default();

    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();

    // Source, section and knowledge-item nodes all carry `source_version_id` so a graph
    // consumer can tell which version of a document a node belongs to; knowledge items
    // additionally carry `lifecycle_status`, so superseded/retired knowledge is
    // identifiable in the graph rather than silently indistinguishable from live
    // knowledge. Verified byte-exact against
    // `fixtures/cases/reference/expected-python/exports/bundle/graph/graph.json`.
    for source in &sources {
        nodes.push(json!({
            "id": source["id"],
            "type": "source",
            "label": source["title"],
            "source_version_id": source.get("source_version_id").cloned().unwrap_or(Value::Null),
        }));
    }
    for section in &sections {
        nodes.push(json!({
            "id": section["id"],
            "type": "section",
            "label": section["heading"],
            "source_version_id": section.get("source_version_id").cloned().unwrap_or(Value::Null),
        }));
        let concept = concept_for_label(section["heading"].as_str().unwrap_or(""));
        nodes.push(json!({"id": concept.id, "type": "concept", "label": concept.label}));
        edges.push(json!({"source": section["source_id"], "target": section["id"], "type": "has_section"}));
        edges.push(json!({"source": section["id"], "target": concept.id, "type": "mentions_concept"}));
    }
    for item in &items {
        let relation = knowledge_relation_for_item(item);
        let evidence_id = evidence_id_for_item(item);
        nodes.push(json!({
            "id": item["id"],
            "type": "knowledge_item",
            "label": item["subject"],
            "source_version_id": item.get("source_version_id").cloned().unwrap_or(Value::Null),
            "lifecycle_status": item.get("lifecycle_status").and_then(|v| v.as_str()).unwrap_or("active"),
        }));
        nodes.push(json!({"id": evidence_id, "type": "evidence", "label": item["source_text"]}));
        nodes.push(json!({"id": relation["id"], "type": "knowledge_relation", "label": item["predicate"]}));
        edges.push(json!({"source": item["section_id"], "target": item["id"], "type": "has_knowledge"}));
        edges.push(json!({"source": item["id"], "target": evidence_id, "type": "supported_by"}));
        edges.push(json!({"source": item["id"], "target": relation["id"], "type": "expressed_as"}));
        edges.push(json!({"source": relation["subject"]["id"], "target": relation["id"], "type": "relation_subject"}));
        edges.push(json!({"source": relation["id"], "target": evidence_id, "type": "supported_by"}));
        for rule in decision_rules_for_item(item) {
            nodes.push(json!({"id": rule["id"], "type": "decision_rule", "label": rule["title"]}));
            edges.push(json!({"source": item["id"], "target": rule["id"], "type": "has_decision_rule"}));
            edges.push(json!({"source": relation["id"], "target": rule["id"], "type": "operationalized_by"}));
            edges.push(json!({"source": rule["id"], "target": evidence_id, "type": "supported_by"}));
        }
    }
    for review in &reviews {
        nodes.push(json!({"id": review["id"], "type": "review", "label": review["action"]}));
        edges.push(json!({"source": review["knowledge_item_id"], "target": review["id"], "type": "has_review"}));
    }

    let graph = dedupe_graph(nodes, edges);
    let path = export_dir.join("graph").join("graph.json");
    store::write_json(&path, &graph)?;
    Ok(json!({"type": "graph_json", "path": export_path_for_manifest(bundle_dir, &path)}))
}

/// `dedupe_graph` (§3.21 detail, G4): node dedupe keeps **first insertion position**
/// but **last** value (`{n["id"]: n for n in nodes}` semantics); edge dedupe keeps the
/// **first** occurrence of each `(source, target, type)` triple. These are opposite
/// rules -- inverting either one changes output.
pub fn dedupe_graph(nodes: Vec<Value>, edges: Vec<Value>) -> Value {
    let mut node_order: Vec<String> = Vec::new();
    let mut node_by_id: HashMap<String, Value> = HashMap::new();
    for node in nodes {
        let id = node["id"].as_str().unwrap_or("").to_string();
        if !node_by_id.contains_key(&id) {
            node_order.push(id.clone());
        }
        node_by_id.insert(id, node); // last value wins
    }
    let out_nodes: Vec<Value> = node_order.iter().map(|id| node_by_id[id].clone()).collect();

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut out_edges = Vec::new();
    for edge in edges {
        let key = (
            edge["source"].as_str().unwrap_or("").to_string(),
            edge["target"].as_str().unwrap_or("").to_string(),
            edge["type"].as_str().unwrap_or("").to_string(),
        );
        if seen.insert(key) {
            out_edges.push(edge); // first occurrence wins
        }
    }
    json!({"nodes": out_nodes, "edges": out_edges})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_dedupe_keeps_first_position_last_value() {
        let nodes = vec![json!({"id": "a", "type": "x", "label": "first"}), json!({"id": "b", "type": "x", "label": "b"}), json!({"id": "a", "type": "x", "label": "second"})];
        let graph = dedupe_graph(nodes, vec![]);
        let out_nodes = graph["nodes"].as_array().unwrap();
        assert_eq!(out_nodes.len(), 2);
        assert_eq!(out_nodes[0]["id"], "a");
        assert_eq!(out_nodes[0]["label"], "second"); // last value wins
        assert_eq!(out_nodes[1]["id"], "b");
    }

    #[test]
    fn edge_dedupe_keeps_first_occurrence() {
        let edges = vec![
            json!({"source": "a", "target": "b", "type": "t", "extra": 1}),
            json!({"source": "a", "target": "b", "type": "t", "extra": 2}),
        ];
        let graph = dedupe_graph(vec![], edges);
        let out_edges = graph["edges"].as_array().unwrap();
        assert_eq!(out_edges.len(), 1);
        assert_eq!(out_edges[0]["extra"], 1); // first occurrence wins
    }
}
