//! Phase 9 differential-verification gate for F12 (`concept_index`) and F13
//! (`focused_graph_for_concept`) -- the two functions with no pseudocode anywhere in
//! the port-mapping docs. `phase9_concept_graph_dump.json` was captured by running the
//! REAL Python `concept_index`/`default_graph_concept`/`focused_graph_for_concept`
//! (see `v2/sopkb-rust/fixtures/gen_phase9_concept_graph_dump.py`) against the read-only
//! `fixtures/cases/reference/expected-python/bundle` fixture -- same precedent as
//! `phase8_agent_chat_dump.json` / `crates/sopkb-agent/tests/phase8_reference_diff.rs`.

use serde_json::Value;
use sopkb_workbench::{concept_index, default_graph_concept, focused_graph_for_concept, ConceptSummary, Graph, GraphEdge, GraphNode};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // Walk up from this crate's own directory until a top-level `.git` entry is
    // found (a directory in a normal checkout, a file in a git worktree) --
    // depth-agnostic, unlike a fixed `ancestors().nth(N)` hop count.
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        dir = dir.parent().expect("repo_root: reached filesystem root without finding a .git entry").to_path_buf();
    }
}

fn python_dump() -> Value {
    let path = repo_root().join("v2/sopkb-rust/fixtures/phase9_concept_graph_dump.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn reference_bundle_dir() -> PathBuf {
    repo_root().join("v2/sopkb-rust/fixtures/cases/reference/expected-python/bundle")
}

fn expected_summary(entry: &Value) -> ConceptSummary {
    let statuses: BTreeMap<String, i64> = entry["statuses"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap())).collect();
    ConceptSummary {
        id: entry["id"].as_str().unwrap().to_string(),
        label: entry["label"].as_str().unwrap().to_string(),
        item_count: entry["items"].as_array().unwrap().len(),
        rule_count: entry["rule_count"].as_u64().unwrap() as usize,
        statuses,
    }
}

fn expected_graph(entry: &Value) -> Graph {
    let nodes = entry["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| GraphNode { id: n["id"].as_str().unwrap().to_string(), node_type: n["type"].as_str().unwrap().to_string(), label: n["label"].as_str().unwrap().to_string() })
        .collect();
    let edges = entry["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| GraphEdge { source: e["source"].as_str().unwrap().to_string(), target: e["target"].as_str().unwrap().to_string(), edge_type: e["type"].as_str().unwrap().to_string() })
        .collect();
    Graph { nodes, edges }
}

#[test]
fn concept_index_matches_python_exactly() {
    let dump = python_dump();
    let expected_concepts: Vec<ConceptSummary> = dump["concept_index"].as_array().unwrap().iter().map(expected_summary).collect();

    let actual = concept_index(&reference_bundle_dir()).unwrap();

    assert_eq!(actual.len(), expected_concepts.len(), "concept count must match Python");
    for (a, e) in actual.iter().zip(expected_concepts.iter()) {
        assert_eq!(a.id, e.id);
        assert_eq!(a.label, e.label);
        assert_eq!(a.item_count, e.item_count, "item_count mismatch for concept {}", a.id);
        assert_eq!(a.rule_count, e.rule_count, "rule_count mismatch for concept {}", a.id);
        assert_eq!(a.statuses, e.statuses, "statuses mismatch for concept {}", a.id);
    }
}

#[test]
fn default_graph_concept_matches_python_tiebreak() {
    let dump = python_dump();
    let expected_id = dump["default_graph_concept_id"].as_str().unwrap();

    let concepts = concept_index(&reference_bundle_dir()).unwrap();
    let winner = default_graph_concept(&concepts).unwrap();
    assert_eq!(winner.id, expected_id);
}

#[test]
fn focused_graph_for_first_concept_matches_python() {
    let dump = python_dump();
    let expected = expected_graph(&dump["focused_graph_first"]);

    let concepts = concept_index(&reference_bundle_dir()).unwrap();
    let first = &concepts[0];
    let actual = focused_graph_for_concept(&reference_bundle_dir(), &first.id).unwrap();

    assert_eq!(actual.nodes.len(), expected.nodes.len());
    assert_eq!(actual.edges.len(), expected.edges.len());
    assert_eq!(actual, expected, "focused_graph_for_concept output must match Python node-for-node, edge-for-edge, in order");
}

#[test]
fn focused_graph_for_default_concept_matches_python() {
    let dump = python_dump();
    let expected = expected_graph(&dump["focused_graph_default"]);

    let concepts = concept_index(&reference_bundle_dir()).unwrap();
    let default_concept = default_graph_concept(&concepts).unwrap();
    let actual = focused_graph_for_concept(&reference_bundle_dir(), &default_concept.id).unwrap();

    assert_eq!(actual, expected);
}
