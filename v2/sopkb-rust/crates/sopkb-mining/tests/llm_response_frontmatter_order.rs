//! End-to-end proof that an authored document's `frontmatter` key order survives from
//! the raw LLM JSON response text all the way into the emitted YAML frontmatter,
//! unsorted -- the specific landmine called out in this crate's task brief
//! (`serde_json::Value`/`Map` in this workspace is BTreeMap-backed and silently
//! re-sorts object keys at parse time; only `sopkb_mining::ordered_json::OrderedJson`,
//! parsed directly from raw response text, avoids that).
//!
//! Uses the hand-authored fixture at `fixtures/llm-responses/mining-sample.json`
//! (NOT a captured real Azure response -- see that file's `_fixture_note`), whose
//! `documents[0].frontmatter` is deliberately ordered `title, type, sources, sopkb`
//! (not alphabetical) specifically to make an accidental re-sort visible.

use sopkb_mining::okf_author::parse_author_response;
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

#[test]
fn frontmatter_key_order_from_raw_response_text_survives_into_emitted_yaml() {
    let fixture_path = repo_root().join("v2/sopkb-rust/fixtures/llm-responses/mining-sample.json");
    let raw_text = std::fs::read_to_string(&fixture_path).unwrap();

    let response = parse_author_response(&raw_text).unwrap();
    let documents = response.get("documents").unwrap().as_array().unwrap();
    let frontmatter = documents[0].as_object().unwrap().get("frontmatter").unwrap().as_object().unwrap();

    let keys: Vec<&str> = frontmatter.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, vec!["title", "type", "sources", "sopkb"], "OrderedJson must preserve the raw response's original key order");

    // Confirm the SAME order reaches the actual serialized YAML output (not just the
    // parsed value), via the real write path.
    let prepared = sopkb_mining::okf_writer::prepare_concept_doc("rules/route-uncertain-case", frontmatter, "# Body\n", "test/actor").unwrap();
    let frontmatter_block = prepared.text.split("---").nth(1).unwrap();
    // Top-level mapping keys start at column 0 and are not sequence-item markers
    // (nested "- id: ..." lines from the `sources` list also start at column 0 in
    // this emitter's block style, so they must be excluded explicitly).
    let top_level_keys: Vec<&str> = frontmatter_block
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with('-') && !l.trim().is_empty())
        .map(|l| l.split(':').next().unwrap())
        .collect();
    // "generated" is appended by prepare_concept_doc (setdefault semantics), landing
    // after the original keys -- everything BEFORE it must match the original order.
    assert_eq!(&top_level_keys[..4], &["title", "type", "sources", "sopkb"]);
    assert_eq!(top_level_keys.last(), Some(&"generated"));

    // Sanity: this is NOT the alphabetically-sorted order that a `serde_json::Value`
    // round-trip would have silently produced.
    let mut sorted = keys.clone();
    sorted.sort();
    assert_ne!(keys, sorted, "fixture must use a non-alphabetical key order for this test to mean anything");

    // And confirm a JSON boolean nested in the frontmatter (sopkb.rule.otherwise.
    // action_required: false) renders as the bare YAML `false`, not the quoted string
    // `'false'` -- the specific bug this crate's OrderedJson->YamlValue conversion
    // must avoid (see sopkb-fmt's `YamlValue::Bool` doc comment).
    assert!(prepared.text.contains("action_required: false"), "{}", prepared.text);
    assert!(!prepared.text.contains("action_required: 'false'"), "{}", prepared.text);
}

#[test]
fn fixture_response_round_trips_through_mine_with_author_via_mock_transport() {
    // Broader smoke test: the same fixture, driven through the full
    // build-request/call/parse/apply pipeline with MockTransport, produces a
    // schema-valid authored document AND knowledge item (already covered in detail by
    // crates/sopkb-mining/src/okf_author.rs's own tests; this test only re-confirms
    // the fixture file itself is valid input for that pipeline, independent of this
    // crate's other tests reading it).
    let fixture_path = repo_root().join("v2/sopkb-rust/fixtures/llm-responses/mining-sample.json");
    let raw_text = std::fs::read_to_string(&fixture_path).unwrap();
    let response = parse_author_response(&raw_text).unwrap();
    let knowledge_items = response.get("knowledge_items").unwrap().as_array().unwrap();
    assert_eq!(knowledge_items.len(), 1);
    let candidate = knowledge_items[0].as_object().unwrap();
    assert_eq!(candidate.get("subject").unwrap().as_str(), Some("Procedure"));
}
