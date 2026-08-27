//! Differential test for the PDF fixture cases, at the level workstream 1
//! actually owns.
//!
//! Per `docs/port/CATCHUP_PLAN.md`'s "Finding, 2026-08-21", workstream 1 owns
//! *extraction correctness* only -- what `normalize_pdf` produces as content --
//! while the id scheme, `inventory.rs`/`scan_sources` and the source-versioning
//! state files belong to workstream 2. A full `harness.py run --engine rust`
//! would therefore fail these cases on ids no matter how correct the extraction
//! is, and that failure would say nothing about this workstream.
//!
//! So this test compares exactly the field that IS in scope: the normalized
//! Markdown body recorded in each case's
//! `expected-python/bundle/sources/normalized/*.md`, produced by the real
//! oss-launch Python CLI. Byte-exact, no normalization applied -- the content
//! has no timestamps or absolute paths in it.
//!
//! Run with: `cargo test -p sopkb-core --features pdf`

#![cfg(feature = "pdf")]

use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is .../v2/sopkb-rust/crates/sopkb-core
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").canonicalize().expect("fixtures dir")
}

/// `(case name, input pdf, recorded normalized markdown)`
const CASES: &[(&str, &str, &str)] = &[
    ("simple-pdf", "acme_intake_policy.pdf", "acme-intake-policy__v1.md"),
    ("multipage-gap-pdf", "benefits_handbook.pdf", "benefits-handbook__v1.md"),
    ("two-column-pdf", "two_column_notice.pdf", "two-column-notice__v1.md"),
];

fn check(case: &str, pdf_name: &str, md_name: &str) -> String {
    let root = fixtures_root().join("cases").join(case);
    let pdf = root.join("input").join("sources").join(pdf_name);
    let expected_path =
        root.join("expected-python").join("bundle").join("sources").join("normalized").join(md_name);

    let expected_raw = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", expected_path.display()));
    // The reference Python CLI writes in text mode, so on Windows the recorded
    // file is CRLF-terminated. normalize_pdf itself returns LF; the CRLF is an
    // artifact of how the fixture was written to disk, not of the extraction.
    let expected = expected_raw.replace("\r\n", "\n");

    let actual = sopkb_core::pdf::normalize_pdf(&pdf)
        .unwrap_or_else(|e| panic!("{case}: normalize_pdf failed: {e}"));

    assert_eq!(actual, expected, "{case}: normalized markdown differs from the recorded Python output");
    actual
}

#[test]
fn simple_pdf_matches_recorded_python_output() {
    let out = check("simple-pdf", "acme_intake_policy.pdf", "acme-intake-policy__v1.md");
    // The document's own first line is promoted into the H1 and removed from
    // the body, rather than a generic "# Document" placeholder being emitted.
    assert!(out.starts_with("# Acme Clinical Intake Policy\n"), "got: {out:?}");
    assert_eq!(out.matches("Acme Clinical Intake Policy").count(), 1, "promoted title must not be duplicated");
}

#[test]
fn multipage_pdf_skips_the_empty_page_and_keeps_true_page_numbers() {
    let out = check("multipage-gap-pdf", "benefits_handbook.pdf", "benefits-handbook__v1.md");
    // G-A19 / P-N15: the empty middle page is skipped entirely -- no OCR, no
    // placeholder -- and the numbering is the TRUE 1-based page index, so the
    // gap is expected output rather than a dropped page.
    assert!(out.contains("<!-- page 1 -->"), "got: {out:?}");
    assert!(!out.contains("<!-- page 2 -->"), "the empty page must be skipped, not emitted");
    assert!(out.contains("<!-- page 3 -->"), "page 3 keeps its true index: {out:?}");
    let p1 = out.find("<!-- page 1 -->").unwrap();
    let p3 = out.find("<!-- page 3 -->").unwrap();
    assert!(p1 < p3, "pages stay in order");
}

#[test]
fn two_column_pdf_emits_columns_in_reading_order() {
    let out = check("two-column-pdf", "two_column_notice.pdf", "two-column-notice__v1.md");
    // The whole left column must precede the whole right column, rather than
    // the two being interleaved by height (the defect the column pipeline
    // exists to fix).
    let left_last = out.find("for several lines total").expect("left column tail");
    let right_first = out.find("Right column is separate").expect("right column head");
    assert!(left_last < right_first, "left column must be emitted before the right: {out:?}");
    // A genuinely page-spanning line is kept whole, not torn at the gutter.
    assert!(
        out.contains("This line spans the full width of the page body"),
        "spanning line must survive intact: {out:?}"
    );
}

#[test]
fn every_pdf_case_has_its_input_and_recording_present() {
    // Guards against a case being added to CASES without its fixture files, or
    // a recording being deleted -- either would otherwise show up as a
    // confusing panic inside one of the tests above.
    for (case, pdf_name, md_name) in CASES {
        let root = fixtures_root().join("cases").join(case);
        let pdf = root.join("input").join("sources").join(pdf_name);
        let md = root
            .join("expected-python")
            .join("bundle")
            .join("sources")
            .join("normalized")
            .join(md_name);
        assert!(pdf.is_file(), "missing fixture input {}", pdf.display());
        assert!(md.is_file(), "missing recorded output {}", md.display());
    }
}
