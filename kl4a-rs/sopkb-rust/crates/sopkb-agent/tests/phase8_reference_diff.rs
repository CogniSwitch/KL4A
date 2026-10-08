//! Phase 8 acceptance gate (docs/port/PORT_PLAN.md §6.9 Done-when): "the `provider:
//! "context"` path produces a byte-identical `## Context Ready` answer and a
//! byte-identical `.sopkb/agent_chat.json` entry (modulo the timestamp token) for a
//! fixed scenario against the `reference` bundle."
//!
//! `phase8_agent_chat_dump.json` was captured by running the REAL Python
//! `run_agent_chat`/`handle_agent_form` (see `v2/sopkb-rust/fixtures/gen_phase8_agent_chat_dump.py`)
//! against a temp copy of `fixtures/cases/reference/expected-python/bundle`, with a
//! fixed scenario and `utc_now` monkeypatched to a constant -- same precedent as
//! `phase4_python_dump.json` / `crates/sopkb-derive/tests/phase4_v1_diff.rs`. This
//! test re-derives the same calls in Rust and diffs canonical-JSON output, with
//! `created_at` normalized to the literal token `<TS>` on both sides (the harness's
//! own timestamp-normalization convention -- see `crates/sopkb-core/tests/phase2_v1_diff.rs`).

use serde_json::Value;
use std::path::{Path, PathBuf};

const SCENARIO: &str = "A patient calls asking whether they are eligible for GLP-1 therapy and staff must confirm identity before proceeding.";
const TASK_ID: &str = "auto";

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
    let path = repo_root().join("v2/sopkb-rust/fixtures/phase8_agent_chat_dump.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn canonicalize(v: &Value) -> String {
    sopkb_fmt::to_canonical_json(v)
}

/// Replace a top-level (or, for an array of entries, per-element) `created_at` field
/// with the literal token `<TS>`, mirroring the harness's `<TS>` normalization
/// convention used elsewhere in this port for timestamp fields.
fn normalize_created_at(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.contains_key("created_at") {
                map.insert("created_at".to_string(), Value::String("<TS>".to_string()));
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_created_at(item);
            }
        }
        _ => {}
    }
}

/// Copies the checked-in reference fixture bundle to a fresh temp dir so this test
/// (which writes `.sopkb/agent_chat.json`) never mutates the shared fixture corpus.
fn temp_reference_bundle() -> tempfile::TempDir {
    let source = repo_root().join("v2/sopkb-rust/fixtures/cases/reference/expected-python/bundle");
    let dir = tempfile::tempdir().unwrap();
    copy_dir_recursive(&source, &dir.path().join("bundle"));
    dir
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_type = entry.file_type().unwrap();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path);
        } else {
            std::fs::copy(entry.path(), &dst_path).unwrap();
        }
    }
}

#[test]
fn run_agent_chat_context_provider_matches_python() {
    let temp = temp_reference_bundle();
    let bundle_dir = temp.path().join("bundle");

    let actual = sopkb_agent::run_agent_chat(&bundle_dir, TASK_ID, SCENARIO, true, "context", None).unwrap();
    let expected = &python_dump()["run_agent_chat_context"];
    assert_eq!(canonicalize(&actual), canonicalize(expected));
}

#[test]
fn context_answer_markdown_is_byte_identical_to_python() {
    let temp = temp_reference_bundle();
    let bundle_dir = temp.path().join("bundle");

    let actual = sopkb_agent::run_agent_chat(&bundle_dir, TASK_ID, SCENARIO, true, "context", None).unwrap();
    let expected_answer = python_dump()["run_agent_chat_context"]["answer"].as_str().unwrap().to_string();
    assert_eq!(actual["answer"].as_str().unwrap(), expected_answer);
}

#[test]
fn handle_agent_chat_transcript_entry_matches_python_modulo_timestamp() {
    let temp = temp_reference_bundle();
    let bundle_dir = temp.path().join("bundle");

    let mut actual_entry = sopkb_agent::handle_agent_chat(&bundle_dir, TASK_ID, SCENARIO, true, "context", None).unwrap();
    normalize_created_at(&mut actual_entry);

    let mut expected_entry = python_dump()["handle_agent_form_entry"].clone();
    normalize_created_at(&mut expected_entry);

    assert_eq!(canonicalize(&actual_entry), canonicalize(&expected_entry));
}

#[test]
fn agent_chat_json_on_disk_matches_python_modulo_timestamp() {
    let temp = temp_reference_bundle();
    let bundle_dir = temp.path().join("bundle");

    sopkb_agent::handle_agent_chat(&bundle_dir, TASK_ID, SCENARIO, true, "context", None).unwrap();
    let on_disk_path = bundle_dir.join(".sopkb").join("agent_chat.json");
    let mut on_disk: Value = serde_json::from_str(&std::fs::read_to_string(&on_disk_path).unwrap()).unwrap();
    normalize_created_at(&mut on_disk);

    let mut expected = python_dump()["agent_chat_json_on_disk"].clone();
    normalize_created_at(&mut expected);

    assert_eq!(canonicalize(&on_disk), canonicalize(&expected));
}
