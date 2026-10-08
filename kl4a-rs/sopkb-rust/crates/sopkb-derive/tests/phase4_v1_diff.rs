//! Phase 4 acceptance gate (docs/port/PORT_PLAN.md §6.5 Done-when): "for the `reference`
//! ... bundle, a Rust dump of each derivation (as canonical JSON) equals a Python dump
//! of the same call." `phase4_python_dump.json` was captured by directly invoking
//! `agent_consumption`/`agent_tools` against the real `examples/glp1-healthcare/bundle`
//! (see the command in git history / session notes) -- this test re-derives the same
//! calls in Rust and diffs the canonical-JSON output.

use sopkb_derive::context::{agent_context, agent_guide, agent_tasks};
use sopkb_derive::reads::knowledge_items;
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

fn python_dump() -> serde_json::Value {
    let path = repo_root().join("v2/sopkb-rust/fixtures/phase4_python_dump.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn canonicalize(v: &serde_json::Value) -> String {
    sopkb_fmt::to_canonical_json(v)
}

#[test]
fn bundle_describe_matches_python() {
    let bundle_dir = repo_root().join("examples/glp1-healthcare/bundle");
    let actual = sopkb_derive::reads::bundle_describe(&bundle_dir).unwrap();
    let expected = &python_dump()["bundle_describe"];
    assert_eq!(canonicalize(&actual), canonicalize(expected));
}

#[test]
fn agent_tasks_matches_python() {
    let bundle_dir = repo_root().join("examples/glp1-healthcare/bundle");
    let actual = serde_json::to_value(agent_tasks(&bundle_dir).unwrap()).unwrap();
    let expected = &python_dump()["agent_tasks"];
    assert_eq!(canonicalize(&actual), canonicalize(expected));
}

#[test]
fn agent_guide_matches_python() {
    let bundle_dir = repo_root().join("examples/glp1-healthcare/bundle");
    let actual = agent_guide(&bundle_dir);
    let expected = python_dump()["agent_guide"].as_str().unwrap().to_string();
    assert_eq!(actual, expected);
}

#[test]
fn detect_scenario_concepts_matches_python() {
    let bundle_dir = repo_root().join("examples/glp1-healthcare/bundle");
    let items = knowledge_items(&bundle_dir).unwrap();
    let scenario = "A patient calls asking whether they are eligible for GLP-1 therapy and staff must confirm identity before proceeding.";
    let detected = sopkb_derive::concepts::detect_scenario_concepts(&items, scenario, 8);
    let actual: Vec<serde_json::Value> = detected
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.concept.id, "label": d.concept.label, "score": d.score,
                "match_reasons": d.match_reasons, "matched_knowledge_item_ids": d.matched_knowledge_item_ids,
            })
        })
        .collect();
    let expected = &python_dump()["detected_concepts"];
    assert_eq!(canonicalize(&serde_json::Value::Array(actual)), canonicalize(expected));
}

#[test]
fn agent_context_eligibility_check_matches_python() {
    let bundle_dir = repo_root().join("examples/glp1-healthcare/bundle");
    let actual = agent_context(&bundle_dir, "eligibility-check", false).unwrap();
    let expected = &python_dump()["agent_context_eligibility_check"];
    assert_eq!(canonicalize(&actual), canonicalize(expected));
}
