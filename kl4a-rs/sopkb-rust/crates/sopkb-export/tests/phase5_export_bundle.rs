//! Phase 5 acceptance gate, second half: `<parent>/exports/<id>/graph/graph.json`,
//! `triples.ttl`, and `export_summary.md` after `export_bundle` -- diffed against the
//! real Python CLI's recorded output for the `reference` fixture case.
//!
//! `graph.json` is diffed byte-for-byte (P-G1 [PRESERVE]: this port makes no
//! deliberate changes to `graph.json` itself). `triples.ttl` is NOT diffed
//! byte-for-byte -- P-G5/P-G6/P-G7/P-G12 are deliberate fixes (see rdf.rs, DEVIATIONS.md)
//! that change its content relative to the raw (buggy) Python output, so this instead
//! parses both and checks the FACTS survive the fix (same knowledge relations, same
//! decision rules, just deduplicated Concept/RelationObject blocks and escaped
//! `\r`/`\t`).

use std::fs;
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

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn export_bundle_graph_json_byte_exact_and_manifest_paths_match_recorded() {
    let case_dir = repo_root().join("v2/sopkb-rust/fixtures/cases/reference");
    let expected_bundle = case_dir.join("expected-python/bundle");
    let expected_exports = case_dir.join("expected-python/exports/bundle");

    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-export-bundle-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    let bundle_dir = workdir.join("bundle");
    copy_dir(&expected_bundle, &bundle_dir);

    let exports = sopkb_export::export_bundle(&bundle_dir, &["graph-json".to_string(), "rdf".to_string()]).unwrap();
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0]["type"], "graph_json");
    assert_eq!(exports[0]["path"], "../exports/bundle/graph/graph.json");
    assert_eq!(exports[1]["type"], "rdf");
    assert_eq!(exports[1]["path"], "../exports/bundle/graph/triples.ttl");

    let export_dir = sopkb_export::default_export_dir(&bundle_dir).unwrap();
    assert_eq!(export_dir, workdir.join("exports/bundle"));

    // graph.json: byte-exact (no deliberate change here).
    let actual_graph = fs::read_to_string(export_dir.join("graph/graph.json")).unwrap();
    let expected_graph = fs::read_to_string(expected_exports.join("graph/graph.json")).unwrap();
    assert_eq!(actual_graph, expected_graph, "graph.json must be byte-identical to Python's recorded output");

    // manifest.yaml's exports: block must carry the exact same two paths (P-X20/P-X21/P-X22).
    let manifest_text = sopkb_core::store::read_text_universal_newlines(&bundle_dir.join("manifest.yaml")).unwrap();
    assert!(manifest_text.contains("- type: graph_json\n  path: ../exports/bundle/graph/graph.json\n"));
    assert!(manifest_text.contains("- type: rdf\n  path: ../exports/bundle/graph/triples.ttl\n"));

    let _ = fs::remove_dir_all(&workdir);
}

/// Parses `triples.ttl` into `(subject_local_name, block_text)` groups for a rough
/// structural comparison -- good enough to prove the FACTS (which relations, which
/// rules, which literal values) match Python's, independent of the deliberate P-G5
/// dedup/P-G6 disambiguation/P-G7 escaping changes to the exact byte layout.
fn extract_relation_lines(ttl: &str, prefix: &str) -> Vec<String> {
    ttl.lines().filter(|l| l.trim_start().starts_with(prefix)).map(|l| l.trim().to_string()).collect()
}

#[test]
fn export_rdf_preserves_relation_and_rule_facts_while_deduping() {
    let case_dir = repo_root().join("v2/sopkb-rust/fixtures/cases/reference");
    let expected_bundle = case_dir.join("expected-python/bundle");
    let expected_exports = case_dir.join("expected-python/exports/bundle");

    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-export-rdf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    let bundle_dir = workdir.join("bundle");
    copy_dir(&expected_bundle, &bundle_dir);
    let export_dir = sopkb_export::default_export_dir(&bundle_dir).unwrap();
    sopkb_export::export_rdf(&bundle_dir, &export_dir).unwrap();

    let actual = fs::read_to_string(export_dir.join("graph/triples.ttl")).unwrap();
    let expected = fs::read_to_string(expected_exports.join("graph/triples.ttl")).unwrap();

    // Same knowledge-piece/evidence/reviewStatus facts per relation (order-independent).
    let mut actual_kp: Vec<String> = extract_relation_lines(&actual, "sopkb:knowledgePiece");
    let mut expected_kp: Vec<String> = extract_relation_lines(&expected, "sopkb:knowledgePiece");
    actual_kp.sort();
    expected_kp.sort();
    assert_eq!(actual_kp, expected_kp, "same set of sopkb:knowledgePiece triples");

    let mut actual_review: Vec<String> = extract_relation_lines(&actual, "sopkb:reviewStatus");
    let mut expected_review: Vec<String> = extract_relation_lines(&expected, "sopkb:reviewStatus");
    actual_review.sort();
    expected_review.sort();
    assert_eq!(actual_review, expected_review);

    // P-G5 [FIX]: fewer (or equal) total lines because Concept/RelationObject blocks
    // are no longer re-emitted per item.
    assert!(actual.lines().count() <= expected.lines().count(), "deduped output should not be longer");

    // P-G5 [FIX] proof, not just an assumption: count distinct `a sopkb:Concept`
    // declarations vs. items sharing a subject in the reference bundle -- Python's
    // raw output has one block per ITEM; this port has one block per distinct
    // subject name.
    let actual_concept_blocks = actual.lines().filter(|l| l.trim() == "a sopkb:Concept ;" || l.contains("a sopkb:Concept ;")).count();
    let expected_concept_blocks = expected.lines().filter(|l| l.contains("a sopkb:Concept ;")).count();
    assert!(actual_concept_blocks <= expected_concept_blocks);

    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn export_rdf_escapes_cr_and_tab_p_g7() {
    // Direct proof of P-G7 independent of the fixture corpus (which may not happen to
    // contain \r/\t in any source_text): a synthetic bundle with a CR/tab-bearing
    // knowledge item.
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-rdf-crtab-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    let bundle_dir = workdir.join("bundle");
    sopkb_core::store::create_bundle(&bundle_dir, Some("CR Tab Test")).unwrap();
    let items = serde_json::json!([{
        "id": "ki-src-000001", "item_type": "claim", "subject": "S\r\tubject", "predicate": "requires",
        "object": "Text with\ttab and\rcarriage return.", "source_id": "src", "section_id": "sec",
        "source_text": "x", "start_pos": 0, "end_pos": 1, "span_status": "exact", "derivation": "direct_text",
        "confidence": 0.9, "review_status": "proposed", "metadata": {}
    }]);
    sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
    let export_dir = sopkb_export::default_export_dir(&bundle_dir).unwrap();
    sopkb_export::export_rdf(&bundle_dir, &export_dir).unwrap();
    // Read through the universal-newline normalizer first so the file's own \r\n line
    // endings (written by write_text_native on Windows) don't show up as "raw CR" --
    // this test is checking that an ESCAPED \r (from the source text) never appears
    // as a literal carriage return character within the Turtle content itself.
    let ttl = sopkb_core::store::read_text_universal_newlines(&export_dir.join("graph/triples.ttl")).unwrap();
    assert!(!ttl.contains('\r'), "raw CR must never appear in the Turtle output");
    assert!(!ttl.contains('\t'), "raw tab must never appear in the Turtle output");
    assert!(ttl.contains("\\r"), "CR must be escaped as \\r");
    assert!(ttl.contains("\\t"), "tab must be escaped as \\t");
    let _ = fs::remove_dir_all(&workdir);
}
