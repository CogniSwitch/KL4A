//! PDF normalization (CATCHUP_PLAN.md workstream 1, decisions D1/D2).
//!
//! Target behavior is `integration/oss-launch`'s *current*
//! `tools/sopkb/sopkb/normalize.py` per decision D1 -- **not** the pre-fork
//! behavior specified in `PORT_PLAN.md` / `port-mapping-a-core-data.md`. Those
//! two disagree materially about this function's output; see the
//! `## Page N` note on `normalize_pdf` below and `DEVIATIONS.md`.
//!
//! Structure parsing uses `lopdf`; every layout decision (character -> word ->
//! line) is hand-built to mirror pdfminer.six/pdfplumber rather than delegated
//! to a high-level extraction crate, per D2.

pub mod content;
pub mod fonts;
pub mod graphics;
pub mod layout;
pub mod table_finder;
pub mod tables;
pub mod words;

/// `normalize_pdf(path)`.
///
/// Errors are returned as the message text `normalize_sources` will splice into
/// `"normalization failed: {}"`, matching how Python's `str(exc)` is used.
///
/// Output shape, per current oss-launch:
///
/// ```text
/// # <promoted title>
///
/// <!-- page 1 -->
///
/// ...page 1 text...
///
/// <!-- page 3 -->
///
/// ...page 3 text...
/// ```
///
/// Two behaviors worth calling out because they look like bugs and are not:
///
/// * **Page numbering has gaps.** A page whose extracted text is empty is
///   skipped entirely -- there is no OCR and no placeholder -- but the number
///   printed is the TRUE 1-based page index, so `<!-- page 1 -->` may be
///   followed by `<!-- page 3 -->` (G-A19 / P-N15).
/// * **The marker is an HTML comment, not a heading.** The pre-fork spec this
///   repo's `PORT_PLAN.md` still describes emitted `## Page N`, which made
///   `extract_sections` split a spurious section at every page boundary (50 of
///   175 sections across 3 real PDFs turned out to be nothing else). Current
///   oss-launch emits a comment so page breaks stay greppable without any
///   heading-driven consumer seeing them. D1 selects this newer behavior.
#[cfg(feature = "pdf")]
pub fn normalize_pdf(path: &std::path::Path) -> Result<String, String> {
    // An encrypted or corrupt file fails here, exactly where pdfplumber.open()
    // does, and the message is surfaced as "normalization failed: <message>".
    let doc = lopdf::Document::load(path).map_err(|e| e.to_string())?;
    if doc.is_encrypted() {
        return Err("PDF is encrypted".to_string());
    }

    let extracted = content::extract_pages(&doc);
    let raw_pages: Vec<String> = extracted
        .iter()
        .map(|page| {
            let text = layout::extract_pdf_page_text(&page.chars, &page.graphics, page.width, page.height);
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            text.trim_matches(|c: char| c.is_whitespace()).to_string()
        })
        .collect();
    let raw_pages = layout::strip_repeating_boilerplate(&raw_pages);

    // The `provider in {"azure-llm", "llm"}` per-page LLM heading restructure
    // is deliberately not ported here: `sopkb-core` has no LLM dependency (that
    // lives in `sopkb-llm`), and the fixture corpus runs the "fixture" provider,
    // which skips that branch entirely.
    let mut pages: Vec<String> = Vec::new();
    for (i, text) in raw_pages.iter().enumerate() {
        if !text.is_empty() {
            pages.push(format!("<!-- page {} -->\n\n{}", i + 1, text));
        }
    }

    if pages.is_empty() {
        return Err("PDF text extraction produced no content".to_string());
    }

    // Promote the document's own first line into the H1, rather than emitting a
    // generic "Document" placeholder, and remove it from the body so it does
    // not appear twice. Skipped when that line already starts with `#` (it is
    // a heading of its own) or `|` (a misdetected table row).
    let mut title = "Document".to_string();
    if let Some((prefix_len, first_line, consumed)) = first_line_of(&pages[0]) {
        let lstripped = first_line.trim_start_matches(|c: char| c.is_whitespace());
        if !lstripped.starts_with('#') && !lstripped.starts_with('|') {
            title = first_line.trim_matches(|c: char| c.is_whitespace()).to_string();
            let page0 = pages[0].clone();
            pages[0] = format!("{}{}", &page0[..prefix_len], &page0[consumed..]);
        }
    }

    let body = pages.join("\n\n");
    let body = body.trim_matches(|c: char| c.is_whitespace());
    Ok(format!("# {title}\n\n{body}\n"))
}

/// Equivalent of `re.match(r"(<!-- page \d+ -->\n\n)([^\n]+)\n", page)`.
///
/// Returns `(prefix_len, first_line, match_end)` so the caller can splice the
/// promoted line out exactly the way the Python slices `pages[0]`. The trailing
/// `\n` is required by the regex, so a page marker followed by a single
/// unterminated line does not match -- and no title is promoted.
#[cfg(feature = "pdf")]
fn first_line_of(page: &str) -> Option<(usize, &str, usize)> {
    let rest = page.strip_prefix("<!-- page ")?;
    let digits_len = rest.find(|c: char| !c.is_ascii_digit())?;
    if digits_len == 0 {
        return None;
    }
    let after_digits = &rest[digits_len..];
    let body = after_digits.strip_prefix(" -->\n\n")?;
    let prefix_len = page.len() - body.len();

    let nl = body.find('\n')?;
    if nl == 0 {
        return None; // `[^\n]+` needs at least one character
    }
    Some((prefix_len, &body[..nl], prefix_len + nl + 1))
}

#[cfg(all(test, feature = "pdf"))]
mod tests {
    use super::*;

    /// A minimal valid PDF with `n` pages, each carrying `content` as its
    /// content stream. Mirrors `fixtures/harness/make_pdf.py` closely enough for
    /// the error-path tests, which need a *structurally valid* PDF that simply
    /// has no text in it -- the case a scanned/image-only document produces.
    fn minimal_pdf(page_contents: &[&str]) -> Vec<u8> {
        let mut objects: Vec<String> = vec![String::new(), String::new()]; // catalog, pages
        let font = {
            objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
            objects.len()
        };
        let mut page_nums = Vec::new();
        for content in page_contents {
            objects.push(format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content));
            let content_num = objects.len();
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 {font} 0 R >> >> /Contents {content_num} 0 R >>"
            ));
            page_nums.push(objects.len());
        }
        let kids = page_nums.iter().map(|n| format!("{n} 0 R")).collect::<Vec<_>>().join(" ");
        objects[1] = format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", page_nums.len());
        objects[0] = "<< /Type /Catalog /Pages 2 0 R >>".to_string();

        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0usize; objects.len() + 1];
        for (i, body) in objects.iter().enumerate() {
            offsets[i + 1] = out.len();
            out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
        }
        let xref_pos = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for offset in &offsets[1..=objects.len()] {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objects.len() + 1, xref_pos)
                .as_bytes(),
        );
        out
    }

    fn write_temp(bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("t.pdf");
        std::fs::write(&path, bytes).expect("write pdf");
        (dir, path)
    }

    #[test]
    fn a_pdf_with_no_extractable_text_is_an_error_not_an_empty_document() {
        // Matches the Python exactly: `raise ValueError("PDF text extraction
        // produced no content")`, verified against oss-launch's normalize_pdf on
        // this same input. A scanned/image-only PDF becomes a `parse_status:
        // "failed"` source -- there is no OCR and no synthesized placeholder
        // (P-N9's empty-input asymmetry: empty Markdown normalizes to "\n", an
        // empty PDF raises).
        let (_dir, path) = write_temp(&minimal_pdf(&["", ""]));
        let err = normalize_pdf(&path).expect_err("a text-free PDF must fail");
        assert_eq!(err, "PDF text extraction produced no content");
    }

    #[test]
    fn a_corrupt_file_fails_at_open_with_a_message_not_a_panic() {
        // The Python fails inside `pdfplumber.open()`; the message text differs
        // between engines (pdfminer says "No /Root object!", lopdf has its own
        // wording), which is acceptable -- the malformed-* fixtures assert shape
        // rather than exact text for exactly this reason. What matters is that
        // it is a returned error, surfaced as "normalization failed: ...", and
        // never a panic.
        let (_dir, path) = write_temp(b"this is not a pdf at all");
        let err = normalize_pdf(&path).expect_err("a corrupt file must fail");
        assert!(!err.is_empty(), "the error must carry a message for the inventory warning");
    }

    #[test]
    fn a_missing_file_fails_rather_than_panicking() {
        let err = normalize_pdf(std::path::Path::new("does-not-exist-anywhere.pdf"))
            .expect_err("a missing file must fail");
        assert!(!err.is_empty());
    }

    #[test]
    fn first_line_matches_the_python_regex_shape() {
        let page = "<!-- page 1 -->\n\nACME Insurance Ltd\nbody text\n";
        let (prefix_len, line, end) = first_line_of(page).expect("should match");
        assert_eq!(line, "ACME Insurance Ltd");
        assert_eq!(&page[..prefix_len], "<!-- page 1 -->\n\n");
        assert_eq!(&page[end..], "body text\n");
    }

    #[test]
    fn first_line_requires_a_terminating_newline() {
        // `[^\n]+\n` -- an unterminated single line does not match, so no title
        // is promoted from it.
        assert!(first_line_of("<!-- page 1 -->\n\nonly line").is_none());
    }

    #[test]
    fn first_line_rejects_a_malformed_marker() {
        assert!(first_line_of("<!-- page -->\n\nx\n").is_none());
        assert!(first_line_of("## Page 1\n\nx\n").is_none());
        assert!(first_line_of("<!-- page 1 -->\nx\n").is_none(), "needs a blank line after the marker");
    }
}
