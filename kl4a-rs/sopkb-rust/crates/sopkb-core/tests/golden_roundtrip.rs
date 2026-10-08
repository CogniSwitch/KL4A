//! Phase 1 "Done when" criteria (docs/port/PORT_PLAN.md §6.2), checked against real
//! data rather than hand-written fixtures:
//!
//! 1. Reading and re-emitting `examples/glp1-healthcare/bundle/manifest.yaml` and
//!    every `.sopkb/*.json` in it produces byte-identical output.
//! 2. A property test asserts `emit(parse(x)) == x` for every YAML and JSON file in
//!    the golden corpus (here: every recorded Phase 0 fixture case's
//!    `expected-python/` tree, plus the real reference bundle).

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // Walk up from this crate's own directory until a top-level `.git` entry is
    // found (a directory in a normal checkout, a file in a git worktree) --
    // depth-agnostic, unlike a fixed relative hop count.
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        dir = dir.parent().expect("repo_root: reached filesystem root without finding a .git entry").to_path_buf();
    }
}

fn assert_json_round_trips(path: &Path) {
    let text = fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let mut re_emitted = sopkb_fmt::to_canonical_json(&value);
    re_emitted.push('\n');
    let re_emitted = if cfg!(windows) { re_emitted.replace('\n', "\r\n") } else { re_emitted };
    assert_eq!(re_emitted, text, "JSON round-trip mismatch: {path:?}");
}

fn assert_yaml_round_trips(path: &Path) {
    let text = fs::read_to_string(path).unwrap();
    let value = sopkb_fmt::parse_yaml(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let re_emitted = sopkb_fmt::emit_yaml(&value);
    let re_emitted = if cfg!(windows) { re_emitted.replace('\n', "\r\n") } else { re_emitted };
    assert_eq!(re_emitted, text, "YAML round-trip mismatch: {path:?}");
}

#[test]
fn reference_bundle_manifest_round_trips_byte_identical() {
    let path = repo_root().join("examples/glp1-healthcare/bundle/manifest.yaml");
    assert!(path.exists(), "reference bundle manifest.yaml not found at {path:?}");
    assert_yaml_round_trips(&path);
}

#[test]
fn reference_bundle_sopkb_json_files_round_trip_byte_identical() {
    let dir = repo_root().join("examples/glp1-healthcare/bundle/.sopkb");
    assert!(dir.exists(), "reference bundle .sopkb dir not found at {dir:?}");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            assert_json_round_trips(&path);
            checked += 1;
        }
    }
    assert!(checked > 0, "no .sopkb/*.json files found to check under {dir:?}");
}

/// Every recorded Phase 0 `expected-python/` tree: every `manifest.yaml` and every
/// `.json` file under `.sopkb/` and `reports/` must round-trip byte-identical. This
/// is the corpus PORT_PLAN.md §6.2's "golden corpus" property test refers to.
#[test]
fn every_fixture_case_manifest_and_state_json_round_trips() {
    let cases_dir = repo_root().join("v2/sopkb-rust/fixtures/cases");
    assert!(cases_dir.exists(), "fixtures/cases not found at {cases_dir:?}");

    let mut yaml_checked = 0;
    let mut json_checked = 0;

    for case_entry in fs::read_dir(&cases_dir).unwrap() {
        let case_dir = case_entry.unwrap().path();
        let case_name = case_dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let expected = case_dir.join("expected-python");
        if !expected.is_dir() {
            continue;
        }
        // `legacy-layout` and the five `malformed-*` cases pre-stage a fully-formed
        // `.sopkb/` (or legacy-path) state by hand rather than deriving it from a
        // `scan`/`normalize`/`mine` run, and their `script` is just `validate` (+
        // `export` for legacy-layout) -- neither of which rewrites
        // document_contexts.json/entities.json/triples.json/reviews.json. So those
        // fixtures' `.sopkb/*.json` files are exactly the Phase-0-hand-authored
        // input bytes (some deliberately compact, not `write_json`-formatted), never
        // actually produced by `write_json`. Asserting canonical-round-trip on them
        // would test the fixture author's formatting choice, not the writer's
        // correctness -- so only the pipeline-driven cases (which DO freshly write
        // every state file through real Python `write_json`) are checked here.
        let hand_authored_json_state_cases = [
            "legacy-layout",
            "malformed-null-manifest-sources",
            "malformed-null-confidence",
            "malformed-items-object",
            "malformed-inventory-array",
            "malformed-warnings-strings",
        ];
        // Unlike its .sopkb/*.json siblings, `legacy-layout`'s manifest.yaml IS
        // freshly regenerated -- its script runs `export`, whose manifest-exports
        // merge re-saves manifest.yaml (populating `exports` with real paths) through
        // the real `save_manifest`, so it genuinely is canonical writer output. The
        // five `malformed-*` cases run only `validate`, which never calls
        // `save_manifest`, so their manifest.yaml stays exactly as hand-authored.
        let hand_authored_manifest_cases = [
            "malformed-null-manifest-sources",
            "malformed-null-confidence",
            "malformed-items-object",
            "malformed-inventory-array",
            "malformed-warnings-strings",
        ];
        let skip_json = hand_authored_json_state_cases.contains(&case_name.as_str());
        let skip_manifest = hand_authored_manifest_cases.contains(&case_name.as_str());
        walk(&expected, &mut |path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "manifest.yaml" {
                if skip_manifest {
                    return;
                }
                assert_yaml_round_trips(path);
                yaml_checked += 1;
            } else if !skip_json && path.extension().and_then(|e| e.to_str()) == Some("json") {
                // validation.json / inventory.json / items.json / etc -- every
                // .sopkb/ and reports/ state file the harness recorded.
                assert_json_round_trips(path);
                json_checked += 1;
            }
        });
    }

    assert!(yaml_checked > 0, "no manifest.yaml files found under {cases_dir:?}");
    assert!(json_checked > 0, "no .json state files found under {cases_dir:?}");
}

fn walk(dir: &Path, f: &mut impl FnMut(&Path)) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else {
            f(&path);
        }
    }
}
