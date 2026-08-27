//! Workstream 1 (docs/port/CATCHUP_PLAN.md), DOCX half: extraction-correctness fixture
//! verification. Per the CATCHUP_PLAN's file-boundary finding, this crate owns
//! `normalize.rs`/`docx.rs` extraction correctness only (NOT `inventory.rs`/`scan_sources`'s
//! id-generation, which is workstream 2's territory) -- so these tests compare `docx::normalize_docx`'s
//! output directly against the *content* fields recorded in each fixture case's
//! `expected-python/` (the normalized markdown body, and independently, `sections.json`'s
//! heading list), not the full bundle tree and not any id.
//!
//! `expected-python/` for these four cases was recorded via
//! `v2/sopkb-rust/fixtures/harness/harness.py record <case> --engine python`, with
//! `SOPKB_HARNESS_PYTHON_CWD` pointed at a throwaway checkout of
//! `origin/integration/oss-launch:tools/sopkb` (with `python-docx` installed in a throwaway
//! venv) -- this worktree's own `tools/sopkb` was never touched, per the task brief. Each case
//! was verified `PASS` running `harness.py run <case> --engine python` immediately after
//! recording (Python-against-Python), confirming the recorded fixture reproduces.

#![cfg(feature = "docx")]

use sopkb_core::docx::normalize_docx;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let mut dir = Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        dir = dir.parent().expect("repo_root: reached filesystem root without finding a .git entry").to_path_buf();
    }
}

fn case_dir(name: &str) -> PathBuf {
    repo_root().join("v2/sopkb-rust/fixtures/cases").join(name)
}

/// The recorded fixture's normalized `.md` was written by the reference Python CLI's
/// `store.write_text`, which -- like this crate's own `store::write_text_native` -- translates
/// `\n` -> `\r\n` on Windows at the file-write layer (unrelated to `normalize_docx` itself,
/// which never touches line endings for DOCX -- see the task brief's "No CRLF handling applied
/// to DOCX output specifically"). Undo that write-layer translation before comparing against
/// `normalize_docx`'s in-memory return value, which always uses bare `\n`.
fn read_expected_normalized(case: &str, normalized_filename: &str) -> String {
    let path = case_dir(case).join("expected-python/bundle/sources/normalized").join(normalized_filename);
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    raw.replace("\r\n", "\n")
}

fn sections_json(case: &str) -> serde_json::Value {
    let path = case_dir(case).join("expected-python/bundle/.sopkb/sections.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap()
}

fn headings(sections: &serde_json::Value) -> Vec<String> {
    sections.as_array().unwrap().iter().map(|s| s["heading"].as_str().unwrap().to_string()).collect()
}

/// Plain paragraphs with Heading 1 / Heading 2 styles, a hard line break inside a run (-> a
/// literal `\n` inside the block, not a new block), and a tab character (-> literal `\t`).
#[test]
fn headings_docx_matches_recorded_python_output() {
    let docx_path = case_dir("headings-docx").join("input/sources/vendor-onboarding.docx");
    let actual = normalize_docx(&docx_path).expect("normalize_docx should succeed");
    let expected = read_expected_normalized("headings-docx", "vendor-onboarding__v1.md");
    assert_eq!(actual, expected);

    let sections = sections_json("headings-docx");
    assert_eq!(
        headings(&sections),
        vec!["Vendor Onboarding Procedure", "Eligibility Requirements", "Approval Steps"]
    );
}

/// A table sitting in the MIDDLE of the visual document (between "Quarterly Review" prose and
/// "Closing Notes") must be relocated to the very end of the normalized output, after every
/// paragraph -- G-A17, the deliberately-preserved "bug".
#[test]
fn table_docx_is_relocated_to_the_end() {
    let docx_path = case_dir("table-docx").join("input/sources/quarterly-review.docx");
    let actual = normalize_docx(&docx_path).expect("normalize_docx should succeed");
    let expected = read_expected_normalized("table-docx", "quarterly-review__v1.md");
    assert_eq!(actual, expected);

    // Belt-and-braces: the table's markdown block must come after BOTH headings' text.
    let table_pos = actual.find("| Metric | Value |").expect("table markdown present");
    let closing_notes_pos = actual.find("## Closing Notes").expect("Closing Notes heading present");
    assert!(table_pos > closing_notes_pos, "table must be relocated after all prose, including text that visually followed it");
}

/// A horizontally-merged cell (`w:gridSpan`) must have its (combined) text duplicated across
/// every spanned column, not deduplicated -- G-A12/G-A17.
#[test]
fn merged_cells_docx_duplicates_across_spanned_columns() {
    let docx_path = case_dir("merged-cells-docx").join("input/sources/coverage-matrix.docx");
    let actual = normalize_docx(&docx_path).expect("normalize_docx should succeed");
    let expected = read_expected_normalized("merged-cells-docx", "coverage-matrix__v1.md");
    assert_eq!(actual, expected);
    assert_eq!(
        actual,
        "# Coverage Matrix\n\n\
         | Region | Plan | Status |\n\
         | --- | --- | --- |\n\
         | All Regions | Standard Coverage | Standard Coverage |\n"
    );
}

/// A custom "Titre 1" (French) paragraph style must NOT be detected as a heading --
/// `heading_level_for_style` is deliberately English-only and case-sensitive (G-A18). The
/// whole document collapses to plain paragraph blocks; the styled paragraph gets no `#` prefix.
#[test]
fn non_english_heading_style_is_not_detected() {
    let docx_path = case_dir("non-english-heading-docx").join("input/sources/politique.docx");
    let actual = normalize_docx(&docx_path).expect("normalize_docx should succeed");
    let expected = read_expected_normalized("non-english-heading-docx", "politique__v1.md");
    assert_eq!(actual, expected);
    assert!(!actual.starts_with('#'), "a non-English heading style must not produce a Markdown heading");

    let sections = sections_json("non-english-heading-docx");
    // No real heading anywhere -> extract_sections falls back to a single synthetic "Document"
    // section, exactly like a heading-less Markdown source would.
    assert_eq!(headings(&sections), vec!["Document"]);
}
