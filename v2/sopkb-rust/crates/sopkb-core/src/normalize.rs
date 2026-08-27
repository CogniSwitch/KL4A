//! Normalization (markdown/text; docx/pdf are feature-gated and not yet implemented --
//! none of Phase 2's required fixture cases need them) + section splitting.
//! docs/port/port-mapping-a-core-data.md §3.5.

use crate::ids::section_id_for;
use crate::models::SectionRecord;
use crate::store::{self, relative_to_bundle};
use serde_json::json;
use std::path::Path;

/// CRLF -> LF first (so CRLF never becomes "\n\n"), then lone CR -> LF, then a full
/// Unicode strip (removes all leading/trailing whitespace, does NOT strip a UTF-8 BOM,
/// U+FEFF), then exactly one trailing `\n` appended. Empty/whitespace-only input -> `"\n"`.
pub fn normalize_markdown(content: &str) -> String {
    let s = content.replace("\r\n", "\n").replace('\r', "\n");
    let s = unicode_strip(&s);
    format!("{s}\n")
}

/// Same CRLF/strip handling as markdown, but always prefixed with a synthetic
/// `"# Document\n\n"` heading -- a `.txt` source therefore always has at least one
/// heading and never takes the synthetic-section branch.
pub fn normalize_text(content: &str) -> String {
    let s = content.replace("\r\n", "\n").replace('\r', "\n");
    let s = unicode_strip(&s);
    format!("# Document\n\n{s}\n")
}

/// Python `str.strip()`: removes leading/trailing whitespace per Unicode's White_Space
/// property (superset of ASCII whitespace) but does NOT strip U+FEFF (BOM), which is
/// category Cf (format), not whitespace.
pub(crate) fn unicode_strip(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace())
}

/// ATX heading regex `(?m)^(#{1,6})\s+(.+?)\s*$`, replicated by hand since Rust's
/// `regex` crate's `\s` in a Markdown-derived string behaves the same way for our
/// purposes; `\s+` matching newlines (so a lone `#` line can pull in a later text line
/// as its heading) is the one genuinely easy-to-miss detail, handled explicitly below.
///
/// `source_version_id` is carried through onto every emitted section so that a later
/// mining pass can tell which *version* of the source each section came from -- that is
/// what [`crate::knowledge_lifecycle::merge_mined_items`] keys its supersede decisions
/// off. `None` is legal and means "pre-versioning bundle"; section ids themselves are
/// keyed on the bare `source_id`, unversioned, matching the fixture corpus
/// (`section-weird-headings-001`, not `section-weird-headings-v1-001`).
pub fn extract_sections(
    source_id: &str,
    content: &str,
    normalized_path: &str,
    source_version_id: Option<&str>,
) -> Vec<SectionRecord> {
    let version = || source_version_id.map(|s| s.to_string());
    let headings = find_headings(content);
    if headings.is_empty() {
        return vec![SectionRecord {
            id: section_id_for(source_id, 1),
            source_id: source_id.to_string(),
            heading: "Document".to_string(),
            semantic_role: "section".to_string(),
            start_pos: 0,
            end_pos: content.chars().count(),
            normalized_path: normalized_path.to_string(),
            source_version_id: version(),
        }];
    }

    let char_index = sopkb_fmt::CharIndex::new(content);
    let mut sections = Vec::with_capacity(headings.len());
    for (i, h) in headings.iter().enumerate() {
        let end_byte = if i + 1 < headings.len() { headings[i + 1].start_byte } else { content.len() };
        sections.push(SectionRecord {
            id: section_id_for(source_id, (i + 1) as u32),
            source_id: source_id.to_string(),
            heading: h.text.trim().to_string(),
            semantic_role: semantic_role_for(&h.text),
            start_pos: char_index.char_offset_at_byte(h.start_byte),
            end_pos: char_index.char_offset_at_byte(end_byte),
            normalized_path: normalized_path.to_string(),
            source_version_id: version(),
        });
    }
    sections
}

struct HeadingMatch {
    start_byte: usize,
    text: String,
    /// Number of leading `#` characters (1-6) -- Python's own `extract_sections` never
    /// looks at this (see its own regex, group 1 is captured but never read), so it was
    /// never carried on `SectionRecord`. Added here to support `heading_ancestors`
    /// below, purely additive agent/UI-facing metadata computed OUTSIDE the
    /// byte-for-byte-pinned `sections.json` contract.
    level: u8,
}

/// Hand-rolled equivalent of `re.finditer(r"(?m)^(#{1,6})\s+(.+?)\s*$", content)`:
/// scans line-by-line (byte offsets, since this feeds `CharIndex` conversion), a line
/// matches only if it starts with 1-6 `#` immediately followed by whitespace (so
/// `#Heading` and 7+ hashes never match); `\s+` after the hashes may consume
/// subsequent blank lines, pulling a LATER non-blank line in as the heading text (the
/// "lone `#`" case) -- the match's start stays at the `#`, matching Python's
/// `heading.start()`.
fn find_headings(content: &str) -> Vec<HeadingMatch> {
    let bytes = content.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < bytes.len() {
        let line_start = pos;
        let line_end = memchr_newline(bytes, pos);
        // Count leading '#' bytes, bounded to this line ('#' can't span lines).
        let mut hash_end = line_start;
        while hash_end < line_end && bytes[hash_end] == b'#' {
            hash_end += 1;
        }
        let hashes = hash_end - line_start;
        if (1..=6).contains(&hashes) {
            // The char immediately after the hashes may be on THIS line, or it may be
            // the newline ending this very line (a lone "#" with nothing else on it) --
            // either way, Python's `\s` matches it (it matches `\n` too), so this must
            // be checked against the byte stream directly, not a line slice that
            // already excluded the trailing newline.
            let next_is_ws = char_at(content, hash_end).is_some_and(|c| c.is_whitespace());
            if next_is_ws {
                let mut cursor = hash_end;
                while let Some(c) = char_at(content, cursor) {
                    if !c.is_whitespace() {
                        break;
                    }
                    cursor += c.len_utf8();
                }
                if cursor < bytes.len() {
                    let text_line_end = memchr_newline(bytes, cursor);
                    let text = content[cursor..text_line_end].to_string();
                    if !text.trim().is_empty() {
                        out.push(HeadingMatch { start_byte: line_start, text, level: hashes as u8 });
                        pos = if text_line_end < bytes.len() { text_line_end + 1 } else { text_line_end };
                        continue;
                    }
                }
            }
        }
        pos = if line_end < bytes.len() { line_end + 1 } else { line_end };
    }
    out
}

/// For every heading in `content`, its nesting `level` (1-6, from `#` count) and the
/// full chain of ANCESTOR heading texts above it (outermost first, NOT including the
/// heading's own text) -- keyed by the heading's own char offset, which is exactly
/// `SectionRecord::start_pos` for the section `extract_sections` builds from that same
/// heading (both are derived from the identical `find_headings` scan).
///
/// Deliberately NOT part of `SectionRecord`/`sections.json`: that file is pinned
/// byte-for-byte against the real Python CLI's own output (`phase2_v1_diff.rs`), which
/// has no equivalent field. This is purely additive, request-time metadata for
/// agent/UI consumers that want "what section is this nested under" -- computed once
/// by `sopkb_derive::reads::sections_list`/`sections_get` (the single shared layer both
/// the desktop app's Tauri commands and `sopkb-mcp`'s tools already call through) and
/// merged into their response, never duplicated per-consumer.
///
/// Standard "flat heading list -> outline tree" algorithm: a stack of `(level, text)`
/// pairs, popped down to the nearest ancestor (strictly shallower level) before each
/// heading is recorded, then pushed as the new potential parent for whatever follows.
pub fn heading_ancestors(content: &str) -> std::collections::BTreeMap<usize, (u8, Vec<String>)> {
    let headings = find_headings(content);
    let char_index = sopkb_fmt::CharIndex::new(content);
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut result = std::collections::BTreeMap::new();
    for h in &headings {
        while let Some((top_level, _)) = stack.last() {
            if *top_level >= h.level {
                stack.pop();
            } else {
                break;
            }
        }
        let char_pos = char_index.char_offset_at_byte(h.start_byte);
        let ancestors = stack.iter().map(|(_, t)| t.clone()).collect();
        result.insert(char_pos, (h.level, ancestors));
        stack.push((h.level, h.text.trim().to_string()));
    }
    result
}

fn char_at(content: &str, byte_pos: usize) -> Option<char> {
    content.get(byte_pos..)?.chars().next()
}

fn memchr_newline(bytes: &[u8], from: usize) -> usize {
    bytes[from..].iter().position(|&b| b == b'\n').map(|i| from + i).unwrap_or(bytes.len())
}

/// Plain substring containment on the lowercased heading, checked in this order:
/// procedure/workflow/steps -> "procedure"; else policy/requirement/control ->
/// "policy"; else "section". No word boundaries, no stemming.
pub fn semantic_role_for(heading: &str) -> String {
    let lowered = heading.to_lowercase();
    if ["procedure", "workflow", "steps"].iter().any(|kw| lowered.contains(kw)) {
        "procedure".to_string()
    } else if ["policy", "requirement", "control"].iter().any(|kw| lowered.contains(kw)) {
        "policy".to_string()
    } else {
        "section".to_string()
    }
}

fn normalize_source_file(path: &Path, source_type: &str) -> Result<String, String> {
    match source_type {
        "markdown" => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            Ok(normalize_markdown(&text))
        }
        "text" => {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            Ok(normalize_text(&text))
        }
        "docx" => normalize_docx_dispatch(path),
        // The Python raises RuntimeError("pdfplumber is required for PDF
        // normalization") only when the `import pdfplumber` itself fails. The
        // Rust analogue of "the library isn't available" is "the `pdf` feature
        // wasn't compiled in", so that message stays on this arm verbatim and
        // the real extraction lives behind the feature. Once the feature is on,
        // failures come from the extraction itself and carry their own message
        // (e.g. "PDF text extraction produced no content"), exactly as they do
        // in Python once the import succeeds.
        #[cfg(not(feature = "pdf"))]
        "pdf" => Err("pdfplumber is required for PDF normalization (not yet ported)".to_string()),
        #[cfg(feature = "pdf")]
        "pdf" => crate::pdf::normalize_pdf(path),
        other => Err(format!("unsupported source type: {other}")),
    }
}

/// Dispatches to the real OOXML-parsing implementation in `crate::docx` when the `docx` feature is
/// enabled; otherwise falls back to the same "not yet ported" stub message the crate always used, so
/// a build without the feature behaves exactly as it did before this workstream.
#[cfg(feature = "docx")]
fn normalize_docx_dispatch(path: &Path) -> Result<String, String> {
    crate::docx::normalize_docx(path)
}

#[cfg(not(feature = "docx"))]
fn normalize_docx_dispatch(_path: &Path) -> Result<String, String> {
    Err("python-docx is required for DOCX normalization (not yet ported)".to_string())
}

/// Source types Python's `normalize_markdown`/`normalize_text`/`normalize_pdf` each
/// call `restructure_headings_llm` on (see `restructure` param below) -- `docx` is
/// deliberately excluded, since `normalize_docx` already gets real heading levels
/// straight from Word's own "Heading N" paragraph styles and never calls it.
const LLM_RESTRUCTURABLE_TYPES: &[&str] = &["markdown", "text", "pdf"];

/// A conservative, regex-free, LLM-free heading detector for the offline/fixture
/// provider. There is no Python equivalent -- Python's fixture mode has NO
/// heading-discovery mechanism at all (verified directly against
/// `origin/integration/oss-launch:tools/sopkb/sopkb/normalize.py`: `extract_sections`
/// only ever splits on LITERAL Markdown `#` headings, and `restructure_headings_llm`,
/// the only thing that ever inserts one, never runs without an LLM provider) -- so a
/// heading-less source (almost every PDF, most plain text) always collapsed to exactly
/// one section offline, regardless of how clearly it was actually structured. This is
/// a genuinely new capability, not a port.
///
/// Calibrated against a real document (an HDFC group health insurance policy PDF)
/// rather than invented rules: its raw pdfplumber-extracted text has NO blank-line
/// paragraph breaks between what were visually distinct blocks in the original PDF
/// (page/paragraph spacing collapses to plain newlines), so "is this line isolated by
/// blank lines" -- an obvious first idea -- turned out to be useless as a signal on
/// real extracted text. What DOES hold up on that document:
///
/// - **ALL-CAPS lines** (`"SECTION A. GOLD PLAN"`, `"SECTION B. PLATINUM PLAN"`): body
///   prose is essentially never fully capitalized for an entire line, so this is a very
///   low-false-positive signal. Inserted as a level-2 heading (`##`).
/// - **Short numbered clause markers** (`"1. In-Patient Hospitalization"`,
///   `"6. Road Ambulance Cover"`): on this document (and plausibly many
///   insurance/legal-style documents), arabic-numbered lines are consistently the
///   document's own named top-level clauses, while roman-numeral (`i.`, `ii.`) and
///   single-letter (`a.`, `b.`) markers are used for dense sub-enumeration -- some of
///   which ARE real sub-headings (`"a. Special Conditions"`) and some of which are
///   plainly list items (`"i. Room Rent and boarding charges..."`), a distinction that
///   needs real semantic judgment (exactly what the LLM path already does; see its own
///   system prompt's "glossary list vs. distinctly-titled clauses" rule). A regex
///   cannot reliably tell those apart, so roman/lettered markers are deliberately left
///   alone here -- under-detecting is the safe failure mode (same one section as
///   today, never worse); over-detecting would fragment real list content into noisy,
///   misleading section boundaries. Inserted as a level-3 heading (`###`).
///
/// Neither rule fires on a genuine list item or a wrapped mid-sentence fragment in
/// practice, because both also require the candidate text to be short and free of
/// terminal sentence punctuation (`.`/`,`/`;`/`:`/`!`/`?`) -- a real heading like
/// "Road Ambulance Cover" has neither; a real sentence almost always has one or the
/// other by the time it's this short, or simply isn't this short at all.
mod heuristic_headings {
    const MAX_SECTION_HEADING_CHARS: usize = 80;
    const MAX_CLAUSE_HEADING_CHARS: usize = 70;
    const MIN_ALPHA_CHARS: usize = 4;

    pub(super) fn insert(content: &str) -> String {
        let trailing_newline = content.ends_with('\n');
        let rewritten: Vec<String> = content
            .lines()
            .map(|line| {
                let trimmed = line.trim();
                if let Some(heading) = detect_all_caps_section(trimmed) {
                    format!("## {heading}")
                } else if let Some(heading) = detect_numbered_clause(trimmed) {
                    format!("### {heading}")
                } else {
                    line.to_string()
                }
            })
            .collect();
        let mut out = rewritten.join("\n");
        if trailing_newline {
            out.push('\n');
        }
        out
    }

    /// A short, fully-uppercase (ignoring digits/punctuation/whitespace) standalone
    /// line with at least `MIN_ALPHA_CHARS` letters -- see the module doc comment for
    /// why this is a safe signal on real extracted document text.
    ///
    /// Requires genuine evidence of UPPERCASE letters (`char::is_uppercase`), not
    /// merely an absence of lowercase ones: a script with no case distinction at all
    /// (CJK ideographs, for one -- caught by a real golden-fixture regression while
    /// building this) has neither, so "no lowercase" alone would misfire on every
    /// short line of ordinary Chinese/Japanese/Korean text.
    fn detect_all_caps_section(trimmed: &str) -> Option<&str> {
        if trimmed.is_empty() || trimmed.chars().count() > MAX_SECTION_HEADING_CHARS {
            return None;
        }
        let uppercase_count = trimmed.chars().filter(|c| c.is_uppercase()).count();
        if uppercase_count < MIN_ALPHA_CHARS {
            return None;
        }
        if trimmed.chars().any(|c| c.is_lowercase()) {
            return None;
        }
        Some(trimmed)
    }

    /// `^\d{1,3}\.\s+(.+)$` at the start of the line, where the captured remainder is
    /// short, starts with a capital letter, has no trailing sentence punctuation, and
    /// has at least a few letters -- see the module doc comment for the reasoning (and
    /// why roman/lettered markers are deliberately NOT matched here). The
    /// whitespace-after-the-dot requirement also rules out a decimal number like
    /// "3.14" or "3.5% of Base Sum Insured", which has no space right after the dot.
    ///
    /// Rejects a LEADING ZERO followed by another digit ("020.", "007.") -- a real
    /// clause is never numbered that way, but a postal/PIN code wrapped across two
    /// lines by the PDF's own layout very much can be (a real false positive found on
    /// the calibration document: "...Mumbai – 400" / "020. Trade Logo displayed..." --
    /// the second half of "400 020" alone looks exactly like a clause marker).
    ///
    /// Rejects a remainder that starts with a LOWERCASE letter: a real heading in this
    /// style consistently starts with a capital ("In-Patient Hospitalization", "Road
    /// Ambulance Cover"); a lowercase start is a strong signal of a wrapped
    /// mid-sentence fragment instead (another real false positive found on the
    /// calibration document, inside a numbered list of criteria under one glossary
    /// definition: "it needs on-going or long-term", "it continues indefinitely").
    fn detect_numbered_clause(trimmed: &str) -> Option<String> {
        let bytes = trimmed.as_bytes();
        let mut i = 0;
        while i < bytes.len() && i < 3 && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 || i >= bytes.len() || bytes[i] != b'.' {
            return None;
        }
        if i > 1 && bytes[0] == b'0' {
            return None;
        }
        let after_dot = &trimmed[i + 1..];
        if !after_dot.starts_with([' ', '\t']) {
            return None;
        }
        let rest = after_dot.trim();
        if rest.is_empty() || rest.chars().count() > MAX_CLAUSE_HEADING_CHARS {
            return None;
        }
        if !rest.chars().next().is_some_and(|c| c.is_uppercase()) {
            return None;
        }
        if rest.ends_with(['.', ',', ';', ':', '!', '?']) {
            return None;
        }
        if rest.chars().filter(|c| c.is_alphabetic()).count() < 3 {
            return None;
        }
        Some(rest.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Positive cases lifted verbatim from a real HDFC group health insurance
        // policy PDF's raw, un-restructured pdfplumber output -- not invented text.
        #[test]
        fn detects_real_all_caps_section_headings() {
            assert_eq!(detect_all_caps_section("SECTION A. GOLD PLAN"), Some("SECTION A. GOLD PLAN"));
            assert_eq!(detect_all_caps_section("SECTION B. PLATINUM PLAN"), Some("SECTION B. PLATINUM PLAN"));
        }

        #[test]
        fn detects_real_numbered_clause_headings() {
            assert_eq!(detect_numbered_clause("1. In-Patient Hospitalization"), Some("In-Patient Hospitalization".to_string()));
            assert_eq!(
                detect_numbered_clause("2. Pre-Hospitalization Medical Expenses Cover"),
                Some("Pre-Hospitalization Medical Expenses Cover".to_string())
            );
            assert_eq!(detect_numbered_clause("4. Day Care Procedures"), Some("Day Care Procedures".to_string()));
            assert_eq!(detect_numbered_clause("6. Road Ambulance Cover"), Some("Road Ambulance Cover".to_string()));
            assert_eq!(detect_numbered_clause("7. Organ Donor Expenses"), Some("Organ Donor Expenses".to_string()));
        }

        #[test]
        fn does_not_flag_ordinary_prose() {
            assert_eq!(detect_all_caps_section("We will pay under below listed Covers on Medically"), None);
            assert_eq!(detect_numbered_clause("We will pay under below listed Covers on Medically"), None);
        }

        #[test]
        fn does_not_flag_roman_or_lettered_sub_enumeration() {
            // Deliberately excluded per the module doc comment -- some of these ARE
            // real headings ("a. Special Conditions") and some are plainly list items
            // ("i. Room Rent..."), a distinction a regex can't reliably make.
            assert_eq!(detect_all_caps_section("i. Room Rent and boarding chargesup to 1% of"), None);
            assert_eq!(detect_numbered_clause("i. Room Rent and boarding chargesup to 1% of"), None);
            assert_eq!(detect_numbered_clause("a. Special Conditions"), None);
        }

        #[test]
        fn does_not_flag_title_case_prose_as_all_caps() {
            assert_eq!(detect_all_caps_section("HDFC Group Health Insurance"), None);
        }

        #[test]
        fn does_not_flag_case_less_scripts_as_all_caps() {
            // A real regression caught by phase2_v1_diff.rs's Chinese-language golden
            // fixture: CJK ideographs are neither uppercase nor lowercase, so "no
            // lowercase characters present" alone incorrectly matched ordinary Chinese
            // prose that has no case distinction at all. Requiring genuine evidence of
            // UPPERCASE characters (not just an absence of lowercase ones) fixes this.
            assert_eq!(detect_all_caps_section("北京事务所"), None);
            assert_eq!(detect_all_caps_section("概述"), None);
        }

        #[test]
        fn does_not_flag_a_decimal_number_as_a_numbered_clause() {
            assert_eq!(detect_numbered_clause("3.14 is not a clause marker"), None);
            assert_eq!(detect_numbered_clause("3.5% of Base Sum Insured"), None);
        }

        #[test]
        fn does_not_flag_a_long_numbered_sentence_as_a_heading() {
            // A real wrapped fragment from the same document -- long, and (in its full
            // form) sentence-like, even though this particular line happens not to end
            // in terminal punctuation itself.
            let long = "In case of admission to a room at rates exceeding the aforesaid limits the reimbursement";
            assert_eq!(detect_numbered_clause(&format!("1. {long}")), None, "too long to be a heading");
        }

        #[test]
        fn does_not_flag_a_wrapped_postal_code_as_a_numbered_clause() {
            // Real false positive found on the calibration document: "Mumbai – 400"
            // wraps onto the next line as "020. Trade Logo displayed above belongs to
            // HDFC Ltd and" -- the pin code's second half alone looks exactly like a
            // clause marker. Real clause numbering never has a leading zero.
            assert_eq!(detect_numbered_clause("020. Trade Logo displayed above belongs to HDFC Ltd"), None);
            assert_eq!(detect_numbered_clause("007. James Bond Street"), None);
        }

        #[test]
        fn does_not_flag_a_lowercase_starting_fragment_as_a_numbered_clause() {
            // Real false positives found on the calibration document, inside a
            // numbered list of criteria under ONE glossary definition (not three
            // separate real headings): "1. it needs on-going or long-term",
            // "2. it continues indefinitely", "3. it recurs or is likely to recur".
            assert_eq!(detect_numbered_clause("1. it needs on-going or long-term"), None);
            assert_eq!(detect_numbered_clause("2. it continues indefinitely"), None);
            assert_eq!(detect_numbered_clause("3. it recurs or is likely to recur"), None);
        }

        #[test]
        fn does_not_flag_a_short_numbered_sentence_ending_in_punctuation() {
            assert_eq!(detect_numbered_clause("1. Payment is due within 30 days."), None);
            assert_eq!(detect_numbered_clause("2. See Section F for definitions:"), None);
        }

        #[test]
        fn does_not_flag_empty_or_whitespace_lines() {
            assert_eq!(detect_all_caps_section(""), None);
            assert_eq!(detect_numbered_clause(""), None);
        }

        #[test]
        fn insert_rewrites_only_matching_lines_and_preserves_everything_else() {
            let content = "HDFC Group Health Insurance\nSECTION A. GOLD PLAN\nWe will pay under below listed Covers.\n1. In-Patient Hospitalization\ni. Room Rent and boarding charges\n";
            let out = insert(content);
            assert_eq!(
                out,
                "HDFC Group Health Insurance\n## SECTION A. GOLD PLAN\nWe will pay under below listed Covers.\n### In-Patient Hospitalization\ni. Room Rent and boarding charges\n"
            );
        }

        #[test]
        fn insert_preserves_absence_of_trailing_newline() {
            assert!(!insert("SECTION A. GOLD PLAN").ends_with("\n\n"));
            assert_eq!(insert("SECTION A. GOLD PLAN"), "## SECTION A. GOLD PLAN");
        }
    }
}

/// Reads `.sopkb/inventory.json`, normalizes every source in inventory order,
/// accumulating a flat `sections.json`. A per-source normalization failure is a SOFT
/// failure (`parse_status = "failed"`, a warning appended) -- it does not abort the run
/// for other sources.
///
/// Migration runs first, so a pre-versioning bundle gains its `source_version_id`s
/// before sections are rebuilt against them.
///
/// **`sources/normalized/` is deliberately NOT reset here** (it was, before source
/// versioning). Normalized files are now per-version (`<id>__v<n>.md`), and the version
/// registry references every one of them; wiping the directory on each run would leave
/// the registry pointing at files that no longer exist, and `validate_bundle` would
/// correctly start reporting `missing normalized source:` for every superseded version.
/// The cost is that a normalized file for a source since removed from the inventory is
/// no longer garbage-collected -- an accepted trade, and the reference implementation's
/// behaviour too.
///
/// `restructure`, when given, is called on every `markdown`/`text`/`pdf` source's
/// already-normalized text (never `docx`, see `LLM_RESTRUCTURABLE_TYPES`) before it is
/// written to disk and split into sections -- the seam this crate uses to stay free of
/// an `sopkb-llm` dependency while still supporting Python's
/// `provider in {"azure-llm", "llm"}: content = restructure_headings_llm(content)`
/// branch, which lives in a higher crate that can see both `sopkb-core` and
/// `sopkb-llm` (see `sopkb_workbench::heading_restructure`). `None` (the fixture-provider
/// case) reproduces this crate's pre-existing behavior exactly: every source normalizes
/// with whatever real Markdown headings it already had, or none, unchanged.
///
/// An `Err` returned by `restructure` is treated exactly like a `normalize_source_file`
/// failure -- a soft, per-source failure (`parse_status = "failed"`, a warning appended),
/// matching Python's own control flow: `restructure_headings_llm` raising propagates
/// out through the very same per-source `except Exception` in `normalize_one` that
/// catches a `normalize_source_file`-equivalent failure, with no special casing for
/// which of the two actually raised.
///
/// Step 1 (below) fans every qualifying source's `normalize_source_file` +
/// `restructure` call out across up to `MAX_PARALLEL_SOURCES` threads -- each source is
/// its own file and (for an LLM provider) its own independent chain of requests, same
/// reasoning as `mine_with_author`'s per-section fan-out. Step 2 applies results and
/// writes files sequentially, in original inventory order (`parallel_map` preserves
/// input order in its results regardless of which worker finished first): not safe to
/// parallelize, since `sections` is one shared `Vec` that a concurrent `.extend()` from
/// multiple threads could interleave. Mirrors `normalize.py`'s own identical two-step
/// split (`_MAX_PARALLEL_SOURCES = 6`).
/// `max_workers`: `None` uses `DEFAULT_MAX_PARALLEL_SOURCES` (6, matching Python's
/// own hardcoded `_MAX_PARALLEL_SOURCES`). `sopkb-core` has no dependency on
/// `sopkb-config` by design (see this module's own normalize_source_file/pdf
/// doc comments on that boundary), so a caller that DOES have config access
/// (`sopkb-workbench`, the desktop app, the CLI) passes the configured value in
/// explicitly rather than this crate reading it itself.
pub const DEFAULT_MAX_PARALLEL_SOURCES: usize = 6;

pub fn normalize_sources(
    bundle_dir: &Path,
    restructure: Option<&(dyn Fn(&str) -> Result<String, String> + Sync)>,
    max_workers: Option<usize>,
) -> crate::error::Result<Vec<serde_json::Value>> {
    let max_workers = max_workers.unwrap_or(DEFAULT_MAX_PARALLEL_SOURCES);

    crate::lifecycle::migrate_source_version_state(bundle_dir)?;
    let mut inventory = store::read_state_json(bundle_dir, "inventory.json", json!({"sources": [], "warnings": []}))?;

    let sources = inventory["sources"].as_array().cloned().unwrap_or_default();
    let mut new_sources: Vec<serde_json::Value> = Vec::with_capacity(sources.len());
    let mut normalizable: Vec<(usize, std::path::PathBuf, String)> = Vec::new();

    for mut source in sources {
        let source_type = source["type"].as_str().unwrap_or("").to_string();
        if !["markdown", "text", "docx", "pdf"].contains(&source_type.as_str()) {
            source["parse_status"] = json!("skipped");
            push_warning(&mut source, "unsupported normalization source type");
        } else {
            let original_path = bundle_dir.join(source["original_path"].as_str().unwrap_or(""));
            normalizable.push((new_sources.len(), original_path, source_type));
        }
        new_sources.push(source);
    }

    let outcomes: Vec<Result<String, String>> = crate::parallel::parallel_map(&normalizable, max_workers, |_, (_, original_path, source_type)| {
        normalize_source_file(original_path, source_type).and_then(|normalized| {
            if LLM_RESTRUCTURABLE_TYPES.contains(&source_type.as_str()) {
                if let Some(f) = restructure {
                    return f(&normalized);
                }
                // No LLM provider configured: fall back to the regex-based heuristic
                // heading detector (see `heuristic_headings`'s own doc comment) rather
                // than leaving a heading-less source to collapse into one section, as
                // Python's own fixture mode always does.
                return Ok(heuristic_headings::insert(&normalized));
            }
            Ok(normalized)
        })
    });

    let mut sections: Vec<SectionRecord> = Vec::new();
    for ((index, _original_path, _source_type), outcome) in normalizable.into_iter().zip(outcomes) {
        let source = &mut new_sources[index];
        match outcome {
            Ok(normalized) => {
                let normalized_rel = source["normalized_path"].as_str().unwrap_or("").to_string();
                let normalized_path = bundle_dir.join(&normalized_rel);
                if let Some(parent) = normalized_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                store::write_text_native(&normalized_path, &normalized)?;
                source["parse_status"] = json!("normalized");
                let source_id = source["id"].as_str().unwrap_or("").to_string();
                let source_version_id = source["source_version_id"].as_str().map(|s| s.to_string());
                let rel = relative_to_bundle(bundle_dir, &normalized_path)?;
                sections.extend(extract_sections(&source_id, &normalized, &rel, source_version_id.as_deref()));
            }
            Err(exc) => {
                source["parse_status"] = json!("failed");
                push_warning(source, &format!("normalization failed: {exc}"));
            }
        }
    }

    inventory["sources"] = json!(new_sources);
    store::write_state_json(bundle_dir, "inventory.json", &inventory)?;

    let section_dicts: Vec<serde_json::Value> = sections.iter().map(|s| serde_json::to_value(s).unwrap()).collect();
    store::write_state_json(bundle_dir, "sections.json", &json!(section_dicts))?;
    Ok(section_dicts)
}

fn push_warning(source: &mut serde_json::Value, warning: &str) {
    let entry = source.as_object_mut().unwrap().entry("warnings").or_insert_with(|| json!([]));
    if let Some(arr) = entry.as_array_mut() {
        arr.push(json!(warning));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn bundle_with_markdown_source(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("policy.md"), text).unwrap();
        store::create_bundle(&bundle_dir, Some("Restructure Hook Test")).unwrap();
        crate::inventory::scan_sources(&source_dir, &bundle_dir).unwrap();
        (dir, bundle_dir)
    }

    fn bundle_with_markdown_sources(count: usize) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        for i in 0..count {
            fs::write(source_dir.join(format!("doc-{i:02}.md")), format!("# Heading {i}\n\nBody {i}.\n")).unwrap();
        }
        store::create_bundle(&bundle_dir, Some("Parallel Sources Test")).unwrap();
        crate::inventory::scan_sources(&source_dir, &bundle_dir).unwrap();
        (dir, bundle_dir)
    }

    #[test]
    fn normalize_sources_actually_runs_sources_concurrently_not_just_compiles() {
        // Same proof shape as `mine_with_author_actually_runs_sections_concurrently_not_just_compiles`
        // and `build_heading_index`'s own concurrency test: a hook that blocks until it
        // observes more than one call in flight at once can only ever return if sources
        // are genuinely dispatched in parallel, not one at a time.
        let (_dir, bundle_dir) = bundle_with_markdown_sources(4);
        let in_flight = std::sync::atomic::AtomicUsize::new(0);
        let max_observed = std::sync::atomic::AtomicUsize::new(0);
        let hook = |text: &str| -> Result<String, String> {
            let now = in_flight.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            max_observed.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(30));
            in_flight.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Ok(text.to_string())
        };
        let sections = normalize_sources(&bundle_dir, Some(&hook), None).unwrap();
        assert_eq!(sections.len(), 4);
        assert!(
            max_observed.load(std::sync::atomic::Ordering::SeqCst) > 1,
            "expected multiple sources' restructure calls in flight at once, saw max {}",
            max_observed.load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    #[test]
    fn normalize_sources_preserves_original_inventory_order_despite_parallel_dispatch() {
        // Sources finish restructuring in whatever order the thread pool happens to
        // schedule them, but `sections.json` must come out in the SAME order as
        // `inventory["sources"]` regardless -- this deliberately delays sources
        // inversely to their inventory position (the first source finishes LAST) so a
        // completion-order bug can't hide behind "it happened to finish in order".
        let (_dir, bundle_dir) = bundle_with_markdown_sources(4);
        let hook = |text: &str| -> Result<String, String> {
            let heading_num: u64 = text.strip_prefix("# Heading ").and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(0);
            std::thread::sleep(std::time::Duration::from_millis((3 - heading_num.min(3)) * 15));
            Ok(text.to_string())
        };
        let sections = normalize_sources(&bundle_dir, Some(&hook), None).unwrap();
        let headings: Vec<String> = sections.iter().map(|s| s["heading"].as_str().unwrap().to_string()).collect();
        assert_eq!(headings, vec!["Heading 0", "Heading 1", "Heading 2", "Heading 3"]);
    }

    #[test]
    fn restructure_hook_none_reproduces_pre_hook_behavior() {
        let (_dir, bundle_dir) = bundle_with_markdown_source("Flat paragraph with no heading.\n");
        let sections = normalize_sources(&bundle_dir, None, None).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["heading"], json!("Document"));
    }

    #[test]
    fn restructure_hook_applies_to_markdown_and_can_add_headings() {
        let (_dir, bundle_dir) = bundle_with_markdown_source("Flat paragraph with no heading.\n");
        let hook: &(dyn Fn(&str) -> Result<String, String> + Sync) = &|text: &str| Ok(format!("# Injected Heading\n\n{text}"));
        let sections = normalize_sources(&bundle_dir, Some(hook), None).unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["heading"], json!("Injected Heading"));
    }

    #[test]
    fn restructure_hook_error_is_a_soft_per_source_failure() {
        let (_dir, bundle_dir) = bundle_with_markdown_source("Flat paragraph with no heading.\n");
        let hook: &(dyn Fn(&str) -> Result<String, String> + Sync) = &|_text: &str| Err("LLM call failed".to_string());
        let sections = normalize_sources(&bundle_dir, Some(hook), None).unwrap();
        assert!(sections.is_empty(), "a restructure failure must not produce any section");
        let inventory = store::read_state_json(&bundle_dir, "inventory.json", json!({})).unwrap();
        let source = &inventory["sources"][0];
        assert_eq!(source["parse_status"], json!("failed"));
        assert!(source["warnings"][0].as_str().unwrap().contains("LLM call failed"));
    }

    #[test]
    fn restructure_hook_is_never_called_for_docx() {
        // Word already carries real "Heading N" paragraph styles -- normalize_docx
        // reads those directly and Python's own normalize_docx never calls
        // restructure_headings_llm. A hook that always errors must therefore never
        // fire for a docx source: if this test fails, the hook started being called
        // for docx too and would need `docx` added to `LLM_RESTRUCTURABLE_TYPES`
        // deliberately, not by accident.
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        let source_dir = dir.path().join("sources");
        fs::create_dir_all(&source_dir).unwrap();
        // No real .docx fixture needed: an unsupported/failing extraction still
        // proves the hook wasn't reached, since normalize_source_file's own error
        // ("python-docx is required..." without the feature, or a real parse error
        // with it) would be indistinguishable from a hook error otherwise -- so
        // assert on the WARNING TEXT instead, which only the hook's error message
        // ("LLM call failed") could have produced.
        fs::write(source_dir.join("policy.docx"), b"not a real docx").unwrap();
        store::create_bundle(&bundle_dir, Some("Docx Hook Test")).unwrap();
        crate::inventory::scan_sources(&source_dir, &bundle_dir).unwrap();
        let hook: &(dyn Fn(&str) -> Result<String, String> + Sync) = &|_text: &str| Err("LLM call failed".to_string());
        normalize_sources(&bundle_dir, Some(hook), None).unwrap();
        let inventory = store::read_state_json(&bundle_dir, "inventory.json", json!({})).unwrap();
        let warning = inventory["sources"][0]["warnings"][0].as_str().unwrap().to_string();
        assert!(!warning.contains("LLM call failed"), "hook must not run for docx, got: {warning}");
    }

    #[test]
    fn normalize_markdown_p_n7_crlf_order() {
        assert_eq!(normalize_markdown("# Title\r\n\r\nBody\r\n"), "# Title\n\nBody\n");
    }

    #[test]
    fn normalize_markdown_empty_becomes_single_newline() {
        assert_eq!(normalize_markdown(""), "\n");
        assert_eq!(normalize_markdown("   \n\t "), "\n");
    }

    #[test]
    fn normalize_markdown_does_not_strip_bom() {
        let with_bom = "\u{FEFF}# Policy\n\nBody\n";
        let out = normalize_markdown(with_bom);
        assert!(out.starts_with('\u{FEFF}'));
    }

    #[test]
    fn extract_sections_no_heading_yields_synthetic_document() {
        let content = "Just a paragraph.\n";
        let sections = extract_sections("src", content, "sources/normalized/src.md", None);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "Document");
        assert_eq!(sections[0].start_pos, 0);
        assert_eq!(sections[0].end_pos, content.chars().count());
    }

    #[test]
    fn extract_sections_preamble_is_orphaned() {
        let content = "Preamble text.\n\n# Heading\n\nBody.\n";
        let sections = extract_sections("src", content, "p", None);
        assert_eq!(sections.len(), 1);
        let heading_char_offset = content.chars().take_while(|_| true).collect::<String>().find('#').map(|byte_off| content[..byte_off].chars().count()).unwrap();
        assert_eq!(sections[0].start_pos, heading_char_offset);
        assert!(sections[0].start_pos > 0, "preamble before the heading must not be covered");
    }

    #[test]
    fn extract_sections_no_space_after_hash_is_not_a_heading() {
        let content = "#NoSpace\n\nBody text.\n";
        let sections = extract_sections("src", content, "p", None);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "Document");
    }

    #[test]
    fn extract_sections_seven_hashes_is_not_a_heading() {
        let content = "####### Seven\n\nBody.\n";
        let sections = extract_sections("src", content, "p", None);
        assert_eq!(sections[0].heading, "Document");
    }

    #[test]
    fn extract_sections_closing_hashes_not_stripped() {
        let content = "## Purpose ##\n\nBody.\n";
        let sections = extract_sections("src", content, "p", None);
        assert_eq!(sections[0].heading, "Purpose ##");
    }

    #[test]
    fn extract_sections_lone_hash_adopts_next_line() {
        let content = "#\nLone Hash Heading\n\nBody.\n";
        let sections = extract_sections("src", content, "p", None);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "Lone Hash Heading");
        assert_eq!(sections[0].start_pos, 0);
    }

    #[test]
    fn heading_ancestors_builds_the_full_outline_chain() {
        let content = "# Title\n\n## Section A\n\nIntro.\n\n### 1. Clause One\n\nBody.\n\n### 2. Clause Two\n\nBody.\n\n## Section B\n\nBody.\n";
        let ancestors = heading_ancestors(content);

        let pos = |needle: &str| content.find(needle).unwrap();
        assert_eq!(ancestors[&pos("# Title")], (1, vec![]));
        assert_eq!(ancestors[&pos("## Section A")], (2, vec!["Title".to_string()]));
        assert_eq!(ancestors[&pos("### 1. Clause One")], (3, vec!["Title".to_string(), "Section A".to_string()]));
        // A sibling clause must NOT inherit the previous sibling as an ancestor.
        assert_eq!(ancestors[&pos("### 2. Clause Two")], (3, vec!["Title".to_string(), "Section A".to_string()]));
        // A later top-level-ish section resets the chain back to just the document title.
        assert_eq!(ancestors[&pos("## Section B")], (2, vec!["Title".to_string()]));
    }

    #[test]
    fn heading_ancestors_handles_a_level_jumping_back_up_multiple_steps() {
        // Section A -> 1. Clause -> a deeper level-4 heading, then straight back to a
        // level-2 heading: the stack must pop past BOTH the level-4 and level-3
        // entries, not just one.
        let content = "## Section A\n\n### 1. Clause\n\n#### Detail\n\nBody.\n\n## Section B\n\nBody.\n";
        let ancestors = heading_ancestors(content);
        let pos = |needle: &str| content.find(needle).unwrap();
        assert_eq!(ancestors[&pos("#### Detail")], (4, vec!["Section A".to_string(), "1. Clause".to_string()]));
        assert_eq!(ancestors[&pos("## Section B")], (2, Vec::<String>::new()));
    }

    #[test]
    fn semantic_role_for_matches_pinned_examples() {
        assert_eq!(semantic_role_for("Eligibility Requirements"), "policy");
        assert_eq!(semantic_role_for("Controlled Substances"), "policy");
        assert_eq!(semantic_role_for("Procedure"), "procedure");
        assert_eq!(semantic_role_for("Workflow Policy"), "procedure");
        assert_eq!(semantic_role_for("Step 1"), "section");
    }
}
