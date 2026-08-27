//! Phase 3 acceptance gate (docs/port/PORT_PLAN.md §6.4 Done-when): V1 passes for
//! `reference` and `missing-empty-dirs` on `validate` alone. Since mining isn't ported
//! yet (Phase 7), these tests replay Python's OWN post-full-pipeline bundle state
//! (`expected-python/bundle/`, which already has real mined `items.json` etc.) through
//! THIS crate's `validate_bundle`, and assert the resulting reports are byte-identical
//! to what Python itself produced from that same state -- a genuine self-consistency
//! check against real data, not a hand-built fixture.

use regex::Regex;
use sopkb_review::validate::validate_bundle;
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

fn normalize(text: &str) -> String {
    let re_json_ts = Regex::new(
        r#"(?m)^(?P<pre>[ \t]*"(?:modified_time|timestamp)":[ \t]*)"(?:[^"\\]|\\.)*"(?P<suf>,?)[ \t]*\r?$"#,
    )
    .unwrap();
    re_json_ts.replace_all(text, "${pre}\"<TS>\"${suf}").to_string()
}

fn run_case(case_name: &str) {
    let expected_bundle = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name).join("expected-python/bundle");
    assert!(expected_bundle.exists(), "missing expected-python/bundle for {case_name}");

    let workdir = std::env::temp_dir().join(format!("sopkb-phase3-v1-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    copy_dir(&expected_bundle, &workdir);

    validate_bundle(&workdir).unwrap_or_else(|e| panic!("{case_name}: validate_bundle failed: {e}"));

    for rel in [
        "reports/validation.json",
        "reports/validation.md",
        "reports/freshness.md",
        "reports/conflicts.md",
        "reports/extraction_summary.md",
        "reports/review_summary.md",
    ] {
        let actual = fs::read_to_string(workdir.join(rel)).unwrap_or_else(|e| panic!("{case_name}: read actual {rel}: {e}"));
        let expected =
            fs::read_to_string(expected_bundle.join(rel)).unwrap_or_else(|e| panic!("{case_name}: read expected {rel}: {e}"));
        assert_eq!(normalize(&actual), normalize(&expected), "{case_name}: {rel} mismatch");
    }

    let _ = fs::remove_dir_all(&workdir);
}

#[test]
fn v1_missing_empty_dirs() {
    run_case("missing-empty-dirs");
}

#[test]
fn v1_reference() {
    run_case("reference");
}
