//! Phase 2 acceptance gate (docs/port/PORT_PLAN.md §6.3 Done-when): V1 passes for
//! `ascii-md`, `unicode-md`, `crlf-md`, `no-heading-md`, `empty-md`, `preamble-md`,
//! `weird-headings-md`, `colliding-stems`, `unsupported-types` -- i.e. running
//! `init -> scan -> normalize` with THIS crate produces `.sopkb/inventory.json`,
//! `.sopkb/sections.json` and `manifest.yaml` byte-identical to the real Python CLI's
//! recorded output in `v2/sopkb-rust/fixtures/cases/<name>/expected-python/`, after the
//! same two normalizations the harness itself applies (timestamps -> `<TS>`,
//! host-absolute paths -> `<ABS>`). `bom-md` is deliberately excluded: P-N5 predicts
//! (and DECISIONS.md commits to) a diff there until the BOM-stripping fix lands.

use regex::Regex;
use sopkb_core::{inventory, normalize, store};
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

fn normalize_ts_abs(text: &str, workdir: &Path) -> String {
    // `\r?` before `$` matters: these files are CRLF-terminated on Windows, and
    // MULTILINE `$` only matches immediately before `\n`, not before a `\r` that
    // precedes it -- the same anchoring gap found (and fixed) in the Phase 0 harness
    // and in sopkb-fmt's own normalization rules.
    let re_yaml_ts = Regex::new(r#"(?m)^(?P<pre>[ \t]*(?:created_at|updated_at):[ \t]*).*\r?$"#).unwrap();
    let re_json_ts = Regex::new(
        r#"(?m)^(?P<pre>[ \t]*"(?:modified_time|timestamp)":[ \t]*)"(?:[^"\\]|\\.)*"(?P<suf>,?)[ \t]*\r?$"#,
    )
    .unwrap();
    let re_warn_path =
        Regex::new(r#"(?m)^(?P<pre>[ \t]*"path":[ \t]*)"(?:[^"\\]|\\.)*"(?P<suf>,?)[ \t]*\r?$"#).unwrap();
    let mut out = re_yaml_ts.replace_all(text, "${pre}<TS>").to_string();
    out = re_json_ts.replace_all(&out, "${pre}\"<TS>\"${suf}").to_string();
    out = re_warn_path.replace_all(&out, "${pre}\"<ABS>\"${suf}").to_string();
    // The recorded fixtures also replace the harness's own ephemeral workdir path
    // with <WORKDIR> inside _transcript.txt only; state files never contain it except
    // via the path-in-warnings case already covered above. Nothing else to strip here.
    let _ = workdir;
    out
}

/// The fixture corpus's `expected-python/` was recorded from running each case's FULL
/// script (init -> scan -> normalize -> mine -> validate -> export), so its
/// `manifest.yaml` has a populated `exports:` list (written by the `export` command --
/// Phase 5, not yet implemented). A Phase-2-only run legitimately leaves `exports: []`.
/// Blank out the exports block on both sides before comparing, since only Phase 5 owns
/// that field; everything else `init`/`scan` write is still asserted byte-exact.
fn strip_exports_block(text: &str) -> String {
    let re_populated = Regex::new(r"(?m)^exports:\r?\n(?:- .*\r?\n(?:  .*\r?\n)*)+").unwrap();
    let re_empty = Regex::new(r"(?m)^exports: \[\]\r?\n").unwrap();
    let out = re_populated.replace(text, "exports: <EXPORTS>\n");
    re_empty.replace(&out, "exports: <EXPORTS>\n").to_string()
}

fn run_case(case_name: &str, title: &str) {
    let case_dir = repo_root().join("v2/sopkb-rust/fixtures/cases").join(case_name);
    let input_sources = case_dir.join("input/sources");
    let expected_bundle = case_dir.join("expected-python/bundle");
    assert!(input_sources.exists(), "missing input/sources for {case_name}");
    assert!(expected_bundle.exists(), "missing expected-python/bundle for {case_name}");

    let workdir = std::env::temp_dir().join(format!("sopkb-phase2-{case_name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workdir);
    fs::create_dir_all(&workdir).unwrap();
    let bundle_dir = workdir.join("bundle");

    store::create_bundle(&bundle_dir, Some(title)).unwrap();
    inventory::scan_sources(&input_sources, &bundle_dir).unwrap();
    normalize::normalize_sources(&bundle_dir, None, None).unwrap();

    for rel in ["manifest.yaml", ".sopkb/inventory.json", ".sopkb/sections.json"] {
        let actual_path = bundle_dir.join(rel);
        let expected_path = expected_bundle.join(rel);
        let actual_raw = fs::read(&actual_path).unwrap_or_else(|e| panic!("read actual {rel} for {case_name}: {e}"));
        let expected_raw =
            fs::read(&expected_path).unwrap_or_else(|e| panic!("read expected {rel} for {case_name}: {e}"));
        let actual_text = String::from_utf8_lossy(&actual_raw).to_string();
        let expected_text = String::from_utf8_lossy(&expected_raw).to_string();
        let mut actual_norm = normalize_ts_abs(&actual_text, &bundle_dir);
        let mut expected_norm = normalize_ts_abs(&expected_text, &bundle_dir);
        if rel == "manifest.yaml" {
            actual_norm = strip_exports_block(&actual_norm);
            expected_norm = strip_exports_block(&expected_norm);
        }
        assert_eq!(actual_norm, expected_norm, "{case_name}: {rel} does not match Python's output");
    }

    let _ = fs::remove_dir_all(&workdir);
}

macro_rules! v1_case {
    ($test_name:ident, $case:literal, $title:literal) => {
        #[test]
        fn $test_name() {
            run_case($case, $title);
        }
    };
}

v1_case!(v1_ascii_md, "ascii-md", "ASCII Markdown Case");
v1_case!(v1_unicode_md, "unicode-md", "Unicode Markdown Case");
v1_case!(v1_crlf_md, "crlf-md", "CRLF Markdown Case");
v1_case!(v1_no_heading_md, "no-heading-md", "No Heading Case");
v1_case!(v1_empty_md, "empty-md", "Empty Markdown Case");
v1_case!(v1_preamble_md, "preamble-md", "Preamble Case");
v1_case!(v1_weird_headings_md, "weird-headings-md", "Weird Headings Case");
v1_case!(v1_colliding_stems, "colliding-stems", "Colliding Stems Case");
v1_case!(v1_unsupported_types, "unsupported-types", "Unsupported Types Case");

// P-N5 / DECISIONS.md: Phase 2 deliberately reproduces today's (unfixed) BOM-loses-
// the-heading behavior -- this must match Python's current output exactly, same as
// every other case. It's ONLY the future BOM-stripping fix that would make this case
// diverge by design.
v1_case!(v1_bom_md, "bom-md", "BOM Markdown Case");
