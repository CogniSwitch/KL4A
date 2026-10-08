//! Direct tests for the P-X8/P-X9/P-G12 crash-avoidance fixes (see DEVIATIONS.md):
//! each proves the Rust port completes without error on upstream data that would make
//! the real Python `sync_okf_bundle`/`export_rdf` raise an uncaught `KeyError` and
//! abort entirely.

use serde_json::json;
use std::path::PathBuf;

fn fresh_bundle(title: &str) -> PathBuf {
    let workdir = std::env::temp_dir().join(format!("sopkb-phase5-gotcha-{title}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workdir);
    let bundle_dir = workdir.join("bundle");
    sopkb_core::store::create_bundle(&bundle_dir, Some(title)).unwrap();
    bundle_dir
}

/// P-X8 [FIX]: a section whose `source_id` has no matching entry in `inventory.json`
/// does not crash `sync_okf_bundle` -- it falls back to a synthetic
/// `{id: source_id, title: source_id}` source record, same as the other four writers.
#[test]
fn section_with_unknown_source_id_does_not_crash_sync() {
    let bundle_dir = fresh_bundle("px8");
    // Deliberately empty inventory -- the section below references a source that
    // simply isn't there.
    sopkb_core::store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": []})).unwrap();
    let sections = json!([{
        "id": "section-ghost-source-001", "source_id": "ghost-source", "heading": "Orphan Section",
        "semantic_role": "body", "start_pos": 0, "end_pos": 10, "normalized_path": ""
    }]);
    sopkb_core::store::write_state_json(&bundle_dir, "sections.json", &sections).unwrap();
    sopkb_core::store::write_state_json(&bundle_dir, "items.json", &json!([])).unwrap();

    let result = sopkb_export::sync_okf_bundle(&bundle_dir);
    assert!(result.is_ok(), "sync_okf_bundle must not crash on an unknown section source_id: {result:?}");

    let doc = std::fs::read_to_string(bundle_dir.join("sections/ghost-source/section-ghost-source-001.md")).unwrap();
    assert!(doc.contains("title: Orphan Section") || doc.contains("Orphan Section"));
    // The synthesized source's title falls back to the bare source id.
    assert!(doc.contains("Normalized section from ghost-source."));

    let _ = std::fs::remove_dir_all(bundle_dir.parent().unwrap());
}

/// P-X9 [FIX]: an item missing `section_id` does not crash `sync_okf_bundle` -- it is
/// simply absent from the `items_by_section` grouping (so no section links to it),
/// while its OWN knowledge/evidence/relation/rule docs (which iterate `items`
/// directly, not through any grouping) are still written normally.
#[test]
fn item_missing_section_id_does_not_crash_sync() {
    let bundle_dir = fresh_bundle("px9");
    let source = json!({
        "id": "src-1", "title": "Src One", "type": "markdown", "original_path": "sources/originals/src-1.md",
        "normalized_path": "sources/normalized/src-1.md", "checksum": "sha256:abc", "size": 10,
        "modified_time": "2026-01-01T00:00:00Z", "parse_status": "normalized", "warnings": [], "metadata": {}
    });
    sopkb_core::store::write_state_json(&bundle_dir, "inventory.json", &json!({"sources": [source]})).unwrap();
    sopkb_core::store::write_state_json(&bundle_dir, "sections.json", &json!([])).unwrap();
    let items = json!([{
        "id": "ki-src-1-000001", "item_type": "claim", "subject": "Orphan Item", "predicate": "requires",
        "object": "Some object text.", "source_id": "src-1",
        // "section_id" deliberately absent
        "source_text": "Some object text.", "start_pos": 0, "end_pos": 5, "span_status": "exact",
        "derivation": "direct_text", "confidence": 0.9, "review_status": "proposed", "metadata": {}
    }]);
    sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

    let result = sopkb_export::sync_okf_bundle(&bundle_dir);
    assert!(result.is_ok(), "sync_okf_bundle must not crash on an item missing section_id: {result:?}");

    // The item's own knowledge doc IS still written (write_knowledge_docs iterates
    // `items` directly, never through the section grouping).
    assert!(bundle_dir.join("knowledge/ki-src-1-000001.md").exists());

    let _ = std::fs::remove_dir_all(bundle_dir.parent().unwrap());
}

/// P-G12 [FIX]: an authored decision rule with no `obligation` key at all (legal --
/// `normalize_authored_rule` never adds a default `obligation`) is skipped by
/// `export_rdf` instead of crashing the whole export on `rule["obligation"]["fact"]`.
#[test]
fn decision_rule_with_no_obligation_is_skipped_not_crashed() {
    let bundle_dir = fresh_bundle("pg12");
    let items = json!([{
        "id": "ki-src-1-000001", "item_type": "claim", "subject": "S", "predicate": "requires",
        "object": "o", "source_id": "src-1", "section_id": "sec-1", "source_text": "o",
        "start_pos": 0, "end_pos": 1, "span_status": "exact", "derivation": "direct_text",
        "confidence": 0.9, "review_status": "proposed",
        "metadata": {"decision_rules": [{"title": "No obligation here"}]}
    }]);
    sopkb_core::store::write_state_json(&bundle_dir, "items.json", &items).unwrap();

    let export_dir = sopkb_export::default_export_dir(&bundle_dir).unwrap();
    let result = sopkb_export::export_rdf(&bundle_dir, &export_dir);
    assert!(result.is_ok(), "export_rdf must not crash on a rule with no obligation: {result:?}");

    let ttl = std::fs::read_to_string(export_dir.join("graph/triples.ttl")).unwrap();
    assert!(!ttl.contains("DecisionRule"), "the obligation-less rule must be skipped entirely, not partially emitted");
    // The item's own KnowledgeRelation is unaffected by the skipped rule.
    assert!(ttl.contains("KnowledgeRelation"));

    let _ = std::fs::remove_dir_all(bundle_dir.parent().unwrap());
}
