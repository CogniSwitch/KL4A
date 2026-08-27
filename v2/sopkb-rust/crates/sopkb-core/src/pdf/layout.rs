//! Port of `integration/oss-launch`'s `extract_pdf_page_text` pipeline and its
//! helpers from `tools/sopkb/sopkb/normalize.py` (D1: current oss-launch
//! behavior, not the pre-fork spec).
//!
//! Every constant and threshold here is copied from that file rather than
//! re-derived; the Python carries long comments explaining which real document
//! each one was tuned against, and changing any of them changes output.
//!
//! Table detection itself (`page.find_tables()`, pdfplumber's ruling-line/
//! intersection algorithm) lives in [`super::table_finder`]; this module wires
//! its output into the same 7-point pipeline `normalize.py`'s
//! `extract_pdf_page_text` docstring describes (table-aware rendering,
//! 2-column-with-table-in-one-column handling, cross-column orphan-word
//! recovery, rotated-text exclusion from table bboxes).

use super::graphics::GraphicsObj;
use super::table_finder::{self, Table};
use super::words::{self, PdfChar, PdfWord, WordExtractor};

// --- constants, copied verbatim from normalize.py ------------------------

const DEDUPE_SIZE_DECIMALS: u32 = 1;
const DEDUPE_POSITION_TOLERANCE: f64 = 1.0;
const STALE_DIGIT_POSITION_TOLERANCE: u32 = 1;

const NON_LATIN_SCRIPT_THRESHOLD: f64 = 0.05;
const LATIN_SCRIPT_X_TOLERANCE: f64 = 2.0;
const DEFAULT_X_TOLERANCE: f64 = 3.0;

const GUTTER_BINS: usize = 300;
const GUTTER_SEARCH_LO: f64 = 0.25;
const GUTTER_SEARCH_HI: f64 = 0.75;
const GUTTER_REL_THRESHOLD: f64 = 0.25;
const GUTTER_MIN_PTS: f64 = 8.0;
const COLUMN_LINE_TOL: f64 = 3.0;

const SPAN_GAP_MIN_PTS: f64 = 10.0;
const HEADING_MARKER_WINDOW_PTS: f64 = 20.0;

const BOILERPLATE_EDGE_LINES: usize = 5;
const BOILERPLATE_MIN_PAGES: usize = 4;
const BOILERPLATE_THRESHOLD: f64 = 0.6;

const MIN_PLAUSIBLE_TABLE_COLS: usize = 2;
const TABLE_ORPHAN_X_PADDING: f64 = 1.0;
const TABLE_COLUMN_MARGIN_PTS: f64 = 20.0;
const TABLE_BBOX_PADDING: f64 = 10.0;

// --- character-level cleanups --------------------------------------------

/// `_dedupe_chars`: pdfplumber's dedup with a size-tolerant grouping key.
pub fn dedupe_chars(chars: &[PdfChar]) -> Vec<PdfChar> {
    let keep = words::dedupe_chars_size_tolerant(chars, DEDUPE_POSITION_TOLERANCE, DEDUPE_SIZE_DECIMALS);
    keep.into_iter().map(|i| chars[i].clone()).collect()
}

/// `_drop_stale_overprinted_digits`: where two or more characters sit at the
/// same rounded (x0, top) and *every* one of them is a single digit, keep only
/// the last-drawn. Deliberately digit-only -- the general "same position" form
/// was found unsafe against Indic combining vowel signs, which legitimately
/// share an anchor point with their base consonant.
pub fn drop_stale_overprinted_digits(chars: &[PdfChar]) -> Vec<PdfChar> {
    use std::collections::HashMap;
    let mut groups: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    let mut order: Vec<(u64, u64)> = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        let key = (
            words::python_round(c.x0, STALE_DIGIT_POSITION_TOLERANCE).to_bits(),
            words::python_round(c.top, STALE_DIGIT_POSITION_TOLERANCE).to_bits(),
        );
        if !groups.contains_key(&key) {
            order.push(key);
        }
        groups.entry(key).or_default().push(i);
    }

    let mut drop: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for key in order {
        let idxs = &groups[&key];
        if idxs.len() > 1
            && idxs.iter().all(|&i| {
                let t = chars[i].text.trim();
                t.len() == 1 && t.chars().all(|c| c.is_ascii_digit())
            })
        {
            for &i in &idxs[..idxs.len() - 1] {
                drop.insert(i);
            }
        }
    }
    if drop.is_empty() {
        return chars.to_vec();
    }
    chars.iter().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, c)| c.clone()).collect()
}

/// `_is_non_latin_char_group`: true if any *alphabetic* codepoint in the char's
/// text is outside Latin script.
///
/// pdfminer's Python asks `unicodedata.name(ch).startswith(("LATIN", "DIGIT"))`.
/// Rust's std has no Unicode name database, so this uses the Latin script
/// blocks instead. The "DIGIT" half of the Python test is unreachable in
/// practice -- it is guarded by `ch.isalpha()`, and no digit is alphabetic --
/// so only the Latin half is modelled.
///
/// Deviation: an alphabetic character whose Unicode name begins with "LATIN"
/// but which sits outside the blocks listed below would be classified
/// non-Latin here. The only consequence is which `x_tolerance` a page gets
/// (2 vs 3), and pages are scored by *fraction*, so a handful of stragglers
/// cannot flip a page on their own.
pub fn is_non_latin_char_group(text: &str) -> bool {
    text.chars().filter(|c| c.is_alphabetic()).any(|c| !is_latin_letter(c))
}

fn is_latin_letter(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x0041..=0x005A            // Basic Latin uppercase
        | 0x0061..=0x007A          // Basic Latin lowercase
        | 0x00AA | 0x00BA          // ordinal indicators (LATIN ... ORDINAL INDICATOR)
        | 0x00C0..=0x00FF          // Latin-1 Supplement letters
        | 0x0100..=0x017F          // Latin Extended-A
        | 0x0180..=0x024F          // Latin Extended-B
        | 0x0250..=0x02AF          // IPA Extensions (named LATIN ...)
        | 0x1D00..=0x1D7F          // Phonetic Extensions
        | 0x1E00..=0x1EFF          // Latin Extended Additional
        | 0x2C60..=0x2C7F          // Latin Extended-C
        | 0xA720..=0xA7FF          // Latin Extended-D
        | 0xAB30..=0xAB6F          // Latin Extended-E
        | 0xFB00..=0xFB06          // Latin ligatures
        | 0xFF21..=0xFF3A          // Fullwidth uppercase
        | 0xFF41..=0xFF5A          // Fullwidth lowercase
    )
}

/// `_page_word_x_tolerance`: the tighter tolerance recovers real word fusion in
/// justified Latin text, but breaks several Indic scripts whose normal
/// within-word spacing is tighter than Latin's -- so it is only applied to
/// pages that are confidently majority-Latin.
pub fn page_word_x_tolerance(chars: &[PdfChar]) -> f64 {
    let non_blank: Vec<&PdfChar> = chars.iter().filter(|c| !c.text.trim().is_empty()).collect();
    if non_blank.is_empty() {
        return DEFAULT_X_TOLERANCE;
    }
    let non_latin = non_blank.iter().filter(|c| is_non_latin_char_group(&c.text)).count();
    let fraction = non_latin as f64 / non_blank.len() as f64;
    if fraction > NON_LATIN_SCRIPT_THRESHOLD {
        DEFAULT_X_TOLERANCE
    } else {
        LATIN_SCRIPT_X_TOLERANCE
    }
}

/// `_char_in_any_table`: true if `char`'s midpoint falls within `padding`
/// points of any table's bbox. A rotated table row/column header is
/// legitimate content `table.extract()` already handles on its own, so it
/// must be excluded from the ordinary rotated-text block -- see
/// `_TABLE_BBOX_PADDING` in the Python original for why the padding is wide
/// (10pt): a table's own bbox is drawn from its ruling lines' centers, but a
/// header glyph's own bounding box can sit slightly outside that line.
fn char_in_any_table(c: &PdfChar, table_bboxes: &[(f64, f64, f64, f64)], padding: f64) -> bool {
    let cx = (c.x0 + c.x1) / 2.0;
    let cy = (c.top + c.bottom) / 2.0;
    table_bboxes.iter().any(|&(x0, top, x1, bottom)| {
        (x0 - padding) <= cx && cx <= (x1 + padding) && (top - padding) <= cy && cy <= (bottom + padding)
    })
}

/// `_build_rotated_block`: reconstruct non-upright text in natural stream order
/// as its own block, instead of letting it bleed into unrelated prose. Reads the
/// chars as a plain list -- no filtering or sorting -- because pdfplumber's
/// own line clustering was observed to shift for *untouched* upright text
/// elsewhere on the page when the char list was filtered first. Excludes a
/// rotated char that belongs to a detected table (see `char_in_any_table`).
pub fn build_rotated_block(chars: &[PdfChar], table_bboxes: &[(f64, f64, f64, f64)]) -> Option<String> {
    let text: String = chars
        .iter()
        .filter(|c| !c.upright && !char_in_any_table(c, table_bboxes, TABLE_BBOX_PADDING))
        .map(|c| c.text.as_str())
        .collect();
    let trimmed = text.trim_matches(|c: char| c.is_whitespace());
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn word_overlaps_any_rotated_char(word: &PdfWord, rotated: &[&PdfChar]) -> bool {
    rotated
        .iter()
        .any(|c| c.x1 > word.x0 && c.x0 < word.x1 && c.bottom > word.top && c.top < word.bottom)
}

// --- column detection -----------------------------------------------------

/// `_detect_column_gutter`: histogram word x-coverage, find the widest
/// low-density run in the middle of the page, return its midpoint.
pub fn detect_column_gutter(words_in: &[PdfWord], page_width: f64) -> Option<f64> {
    let mut counts = vec![0i64; GUTTER_BINS];
    let bin_w = page_width / GUTTER_BINS as f64;
    if bin_w <= 0.0 {
        return None;
    }
    for w in words_in {
        let b0 = ((w.x0 / bin_w) as i64).max(0) as usize;
        let b1 = (((w.x1 / bin_w) as i64).min(GUTTER_BINS as i64 - 1)).max(0) as usize;
        let hi = b1.max(b0).min(GUTTER_BINS - 1);
        for slot in &mut counts[b0.min(hi)..=hi] {
            *slot += 1;
        }
    }

    let lo_bin = (GUTTER_BINS as f64 * GUTTER_SEARCH_LO) as usize;
    let hi_bin = (GUTTER_BINS as f64 * GUTTER_SEARCH_HI) as usize;
    let peak = counts.iter().copied().max().unwrap_or(0);
    // Python's `(max(counts) or 1)`: an all-zero histogram uses 1.
    let threshold = if peak == 0 { 1.0 } else { peak as f64 } * GUTTER_REL_THRESHOLD;

    let (mut best_len, mut best_start, mut best_end) = (0usize, 0usize, 0usize);
    let mut run_start: Option<usize> = None;
    for (b, &count) in counts.iter().enumerate().take(hi_bin).skip(lo_bin) {
        if count as f64 <= threshold {
            if run_start.is_none() {
                run_start = Some(b);
            }
        } else if let Some(s) = run_start.take() {
            if b - s > best_len {
                best_len = b - s;
                best_start = s;
                best_end = b;
            }
        }
    }
    if let Some(s) = run_start {
        if hi_bin.saturating_sub(s) > best_len {
            best_len = hi_bin - s;
            best_start = s;
            best_end = hi_bin;
        }
    }

    if best_len as f64 * bin_w < GUTTER_MIN_PTS {
        return None;
    }
    Some((best_start + best_end) as f64 / 2.0 * bin_w)
}

/// The row grouping used by `_columns_from_words` and `_words_to_lines`.
///
/// Note this is NOT pdfplumber's `cluster_objects`: the anchor (`current_top`)
/// stays pinned to the first word of the group rather than advancing, so it is
/// a fixed-window grouping with no single-linkage chaining.
fn group_rows<'a>(words_in: &[&'a PdfWord], line_tol: f64) -> Vec<Vec<&'a PdfWord>> {
    let mut ordered: Vec<&'a PdfWord> = words_in.to_vec();
    // Python's `sorted` is stable, so ties keep their input order.
    ordered.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap_or(std::cmp::Ordering::Equal));

    let mut rows: Vec<Vec<&'a PdfWord>> = Vec::new();
    let mut current: Vec<&'a PdfWord> = Vec::new();
    let mut current_top: Option<f64> = None;
    for w in ordered {
        match current_top {
            None => {
                current.push(w);
                current_top = Some(w.top);
            }
            Some(t) if (w.top - t).abs() <= line_tol => current.push(w),
            _ => {
                rows.push(std::mem::replace(&mut current, vec![w]));
                current_top = Some(w.top);
            }
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// `_words_to_lines`: group into lines by `top`, sort each line by `x0`, join.
pub fn words_to_lines(words_in: &[&PdfWord], line_tol: f64) -> String {
    group_rows(words_in, line_tol)
        .into_iter()
        .map(|mut line| {
            line.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
            line.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `_HEADING_MARKER_RE`: `^(\d{1,3}\.|[ivxlcdm]{1,7}\.|[IVXLCDM]{1,7}\.|[a-zA-Z]\)|[a-zA-Z]\.)$`
///
/// Every character class in that regex is ASCII-only, so a non-ASCII `s` can never match
/// regardless of shape -- checked first, both to short-circuit correctly and because it's
/// what makes the byte-index slice below safe: `s.is_ascii()` guarantees every byte is a
/// single-byte char, so `s.len()` equals the char count and `s.len() - 1` is always a valid
/// char boundary. Without this check, a word starting with any multi-byte UTF-8 character
/// (e.g. an en-dash "–", 3 bytes) panics here with "byte index N is not a char boundary" --
/// found via a real-world PDF containing en-dash bullet points (2026-08-22).
fn is_heading_marker(s: &str) -> bool {
    if !s.is_ascii() {
        return false;
    }
    let b = s.as_bytes();
    if b.len() < 2 {
        return false;
    }
    let (body, last) = (&s[..s.len() - 1], b[b.len() - 1]);
    if last == b'.' {
        if !body.is_empty() && body.len() <= 3 && body.bytes().all(|c| c.is_ascii_digit()) {
            return true;
        }
        if !body.is_empty() && body.len() <= 7 && body.bytes().all(|c| b"ivxlcdm".contains(&c)) {
            return true;
        }
        if !body.is_empty() && body.len() <= 7 && body.bytes().all(|c| b"IVXLCDM".contains(&c)) {
            return true;
        }
        if body.len() == 1 && body.bytes().all(|c| c.is_ascii_alphabetic()) {
            return true;
        }
        return false;
    }
    if last == b')' {
        return body.len() == 1 && body.bytes().all(|c| c.is_ascii_alphabetic());
    }
    false
}

/// `_columns_from_words`: split into left/right columns at the detected gutter
/// and emit left-then-right, keeping genuinely page-spanning lines whole.
/// Returns `None` when there is no gutter or the split leaves one side empty.
pub fn columns_from_words(words_in: &[&PdfWord], page_width: f64, line_tol: f64) -> Option<String> {
    let owned: Vec<PdfWord> = words_in.iter().map(|w| (*w).clone()).collect();
    let split_x = detect_column_gutter(&owned, page_width)?;

    let rows = group_rows(words_in, line_tol);
    let mut parts: Vec<String> = Vec::new();
    let mut left_bin: Vec<&PdfWord> = Vec::new();
    let mut right_bin: Vec<&PdfWord> = Vec::new();
    let (mut saw_left, mut saw_right) = (false, false);

    fn flush(parts: &mut Vec<String>, left: &mut Vec<&PdfWord>, right: &mut Vec<&PdfWord>, line_tol: f64) {
        let mut chunk: Vec<String> = Vec::new();
        for bin in [&*left, &*right] {
            let text = words_to_lines(bin, line_tol);
            if !text.is_empty() {
                chunk.push(text);
            }
        }
        if !chunk.is_empty() {
            parts.push(chunk.join("\n\n"));
        }
        left.clear();
        right.clear();
    }

    for row in rows {
        let mut row_sorted: Vec<&PdfWord> = row.clone();
        row_sorted.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));

        // A standalone heading marker sitting right at the gutter is treated as
        // the hard start of the right column, rather than asking whether the
        // whole line spans -- see the Python docstring for the real document
        // whose tightly-indented numbered headings this exists for.
        let marker_idx = row_sorted
            .iter()
            .position(|w| is_heading_marker(&w.text) && (w.x0 - split_x).abs() <= HEADING_MARKER_WINDOW_PTS);
        if let Some(i) = marker_idx {
            left_bin.extend_from_slice(&row_sorted[..i]);
            right_bin.extend_from_slice(&row_sorted[i..]);
            saw_left = saw_left || i > 0;
            saw_right = true;
            continue;
        }

        let mut crossing_gap: Option<f64> = None;
        for pair in row_sorted.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.x1 <= split_x && split_x <= b.x0 {
                crossing_gap = Some(b.x0 - a.x1);
                break;
            }
        }
        let has_left = row.iter().any(|w| (w.x0 + w.x1) / 2.0 < split_x);
        let has_right = row.iter().any(|w| (w.x0 + w.x1) / 2.0 >= split_x);
        let is_spanning =
            has_left && has_right && !matches!(crossing_gap, Some(g) if g >= SPAN_GAP_MIN_PTS);
        if is_spanning {
            flush(&mut parts, &mut left_bin, &mut right_bin, line_tol);
            parts.push(row_sorted.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "));
            continue;
        }
        saw_left = saw_left || has_left;
        saw_right = saw_right || has_right;
        // Note: iterates `row`, not `row_sorted` -- the bins are re-sorted by
        // `words_to_lines` anyway, so this only affects tie order.
        for w in &row {
            if (w.x0 + w.x1) / 2.0 < split_x {
                left_bin.push(w);
            } else {
                right_bin.push(w);
            }
        }
    }
    flush(&mut parts, &mut left_bin, &mut right_bin, line_tol);

    if !(saw_left && saw_right) {
        return None;
    }
    Some(parts.join("\n\n"))
}

// --- table rendering -------------------------------------------------------

/// `_cleaned_table_rows`: whitespace-normalize every cell (`" ".join(cell.split())`
/// -- collapses any run of whitespace, including newlines, to a single space
/// and trims the ends), drop fully-blank rows. `None`/empty cell -> `""`.
/// Shared by [`is_plausible_table`] and every table-rendering call site so
/// both judge precisely the same content.
pub fn cleaned_table_rows(table: &Table, chars: &[PdfChar]) -> Vec<Vec<String>> {
    let rows = table.extract(chars);
    rows.into_iter()
        .map(|row| row.into_iter().map(|cell| cell.unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<String>>())
        .filter(|row: &Vec<String>| row.iter().any(|c| !c.is_empty()))
        .collect()
}

/// `_is_plausible_table`: reject a `find_tables()` detection shaped more like
/// misdetected prose/infographic layout than genuine tabular data -- every
/// real false positive found while tuning this against a real document corpus
/// collapses to exactly one populated column per row, however many rows,
/// unlike every genuine table sampled (at least [`MIN_PLAUSIBLE_TABLE_COLS`]
/// populated columns somewhere). A row-count floor was tried first and
/// rejected: several genuine tables in that corpus are single-row (one
/// fee-line or directory entry per bordered box).
pub fn is_plausible_table(cleaned_rows: &[Vec<String>]) -> bool {
    let max_populated_cols = cleaned_rows.iter().map(|row| row.iter().filter(|c| !c.is_empty()).count()).max().unwrap_or(0);
    max_populated_cols >= MIN_PLAUSIBLE_TABLE_COLS
}

/// `markdown_table`: pad every row to the widest row's length, render as a
/// GitHub-flavored Markdown table with a `---` separator under the header row.
pub fn markdown_table(rows: &[Vec<String>]) -> String {
    let width = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let pad = |row: &[String]| -> Vec<String> {
        let mut r = row.to_vec();
        r.resize(width, String::new());
        r
    };
    let padded: Vec<Vec<String>> = rows.iter().map(|r| pad(r)).collect();
    let header = format!("| {} |", padded[0].join(" | "));
    let separator = format!("| {} |", vec!["---"; width].join(" | "));
    let mut lines = vec![header, separator];
    lines.extend(padded[1..].iter().map(|row| format!("| {} |", row.join(" | "))));
    lines.join("\n")
}

/// `_render_table_aware_column`: render one column's own words and tables as a
/// single, independently Y-ordered stream -- a table embedded at its rightful
/// position within THIS column's flow, never treated as a page-wide
/// horizontal band that forces the other column to redo its own local
/// left/right split.
pub fn render_table_aware_column(column_words: &[&PdfWord], column_tables: &[&Table], chars: &[PdfChar], line_tol: f64) -> String {
    enum Entry<'a> {
        Words(Vec<&'a PdfWord>),
        Table(&'a Table),
    }
    let mut sorted_tables: Vec<&Table> = column_tables.to_vec();
    sorted_tables.sort_by(|a, b| a.bbox().1.partial_cmp(&b.bbox().1).unwrap());

    let mut entries: Vec<Entry> = Vec::new();
    let mut cursor = 0.0_f64;
    for table in sorted_tables {
        let (t_x0, t_top, t_x1, t_bottom) = table.bbox();
        let band_words: Vec<&PdfWord> = column_words.iter().copied().filter(|w| cursor <= (w.top + w.bottom) / 2.0 && (w.top + w.bottom) / 2.0 < t_top).collect();
        if !band_words.is_empty() {
            entries.push(Entry::Words(band_words));
        }
        entries.push(Entry::Table(table));
        let same_height_orphans: Vec<&PdfWord> = column_words
            .iter()
            .copied()
            .filter(|w| {
                let mid = (w.top + w.bottom) / 2.0;
                t_top <= mid && mid < t_bottom && (w.x1 < t_x0 - TABLE_ORPHAN_X_PADDING || w.x0 > t_x1 + TABLE_ORPHAN_X_PADDING)
            })
            .collect();
        if !same_height_orphans.is_empty() {
            entries.push(Entry::Words(same_height_orphans));
        }
        cursor = cursor.max(t_bottom);
    }
    let tail_words: Vec<&PdfWord> = column_words.iter().copied().filter(|w| (w.top + w.bottom) / 2.0 >= cursor).collect();
    if !tail_words.is_empty() {
        entries.push(Entry::Words(tail_words));
    }

    let mut parts: Vec<String> = Vec::new();
    for entry in entries {
        match entry {
            Entry::Table(table) => {
                let cleaned = cleaned_table_rows(table, chars);
                if !cleaned.is_empty() {
                    parts.push(markdown_table(&cleaned));
                }
            }
            Entry::Words(ws) => {
                let text = words_to_lines(&ws, line_tol);
                if !text.trim().is_empty() {
                    parts.push(text);
                }
            }
        }
    }
    parts.join("\n\n")
}

// --- page pipeline --------------------------------------------------------

/// `extract_pdf_page_text`: the full pipeline, including table detection and
/// table-aware rendering. See `normalize.py`'s own docstring (quoted in full
/// in `docs/port/CATCHUP_PLAN.md`'s table-aware-rendering section) for the
/// 7-point rationale; this is a direct line-for-line port of that function.
pub fn extract_pdf_page_text(chars_in: &[PdfChar], graphics: &[GraphicsObj], page_width: f64, page_height: f64) -> String {
    let chars = dedupe_chars(chars_in);
    let chars = drop_stale_overprinted_digits(&chars);

    let detected = table_finder::find_tables(graphics);
    // Reject implausible detections before they're treated as "table
    // territory" anywhere below -- see `is_plausible_table`'s docstring.
    let tables: Vec<Table> = detected.into_iter().filter(|t| is_plausible_table(&cleaned_table_rows(t, &chars))).collect();
    let table_bboxes: Vec<(f64, f64, f64, f64)> = tables.iter().map(|t| t.bbox()).collect();

    let has_rotated = chars.iter().any(|c| !c.upright);
    let rotated_block = if has_rotated { build_rotated_block(&chars, &table_bboxes) } else { None };

    let extractor = WordExtractor { x_tolerance: page_word_x_tolerance(&chars), ..Default::default() };

    if tables.is_empty() {
        if !has_rotated {
            // `_extract_columns_or_flow(page) or page.extract_text() or ""`
            let all_words = extractor.extract_words(&chars);
            let refs: Vec<&PdfWord> = all_words.iter().collect();
            let columns = if all_words.is_empty() { None } else { columns_from_words(&refs, page_width, COLUMN_LINE_TOL) };
            return match columns {
                Some(t) if !t.is_empty() => t,
                _ => words::extract_text(&chars, &WordExtractor::default()),
            };
        }

        let all_words = extractor.extract_words(&chars);
        let rotated_chars: Vec<&PdfChar> = chars.iter().filter(|c| !c.upright).collect();
        let upright_words: Vec<&PdfWord> = all_words.iter().filter(|w| !word_overlaps_any_rotated_char(w, &rotated_chars)).collect();

        let mut text = if upright_words.is_empty() { None } else { columns_from_words(&upright_words, page_width, COLUMN_LINE_TOL) }
            .unwrap_or_else(|| words::extract_text(&chars, &WordExtractor::default()));

        if let Some(block) = rotated_block {
            text = if !text.trim_matches(|c: char| c.is_whitespace()).is_empty() { format!("{text}\n\n{block}") } else { block };
        }
        return text;
    }

    let all_words = extractor.extract_words(&chars);
    let words_vec: Vec<PdfWord> = if has_rotated {
        let rotated_chars: Vec<&PdfChar> = chars.iter().filter(|c| !c.upright).collect();
        all_words.into_iter().filter(|w| !word_overlaps_any_rotated_char(w, &rotated_chars)).collect()
    } else {
        all_words
    };
    let words_refs: Vec<&PdfWord> = words_vec.iter().collect();

    // A genuine 2-column page with a table confined to one column needs each
    // column rendered as its own independent stream -- only when no table
    // straddles both columns.
    if let Some(split_x) = detect_column_gutter(&words_vec, page_width) {
        let (mut left_tables, mut right_tables, mut spanning) = (Vec::new(), Vec::new(), false);
        for table in &tables {
            let (t_x0, _, t_x1, _) = table.bbox();
            if t_x1 <= split_x + TABLE_COLUMN_MARGIN_PTS {
                left_tables.push(table);
            } else if t_x0 >= split_x - TABLE_COLUMN_MARGIN_PTS {
                right_tables.push(table);
            } else {
                spanning = true;
                break;
            }
        }
        if !spanning {
            let left_words: Vec<&PdfWord> = words_refs.iter().copied().filter(|w| (w.x0 + w.x1) / 2.0 < split_x).collect();
            let right_words: Vec<&PdfWord> = words_refs.iter().copied().filter(|w| (w.x0 + w.x1) / 2.0 >= split_x).collect();
            let left_text = render_table_aware_column(&left_words, &left_tables, &chars, COLUMN_LINE_TOL);
            let right_text = render_table_aware_column(&right_words, &right_tables, &chars, COLUMN_LINE_TOL);
            let result = [left_text, right_text].into_iter().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n\n");
            if !result.trim().is_empty() {
                return match &rotated_block {
                    Some(block) => format!("{result}\n\n{block}"),
                    None => result,
                };
            }
            // both columns empty is suspicious -- fall through to the Y-band path below
        }
    }

    enum Segment<'a> {
        Prose(f64, f64),
        Table(&'a Table),
    }
    let mut sorted_tables: Vec<&Table> = tables.iter().collect();
    sorted_tables.sort_by(|a, b| a.bbox().1.partial_cmp(&b.bbox().1).unwrap());

    let mut segments: Vec<Segment> = Vec::new();
    let mut cursor = 0.0_f64;
    for table in sorted_tables {
        let (_, t_top, _, t_bottom) = table.bbox();
        if t_top > cursor {
            segments.push(Segment::Prose(cursor, t_top));
        }
        segments.push(Segment::Table(table));
        cursor = cursor.max(t_bottom);
    }
    if cursor < page_height {
        segments.push(Segment::Prose(cursor, page_height));
    }

    let mut parts: Vec<String> = Vec::new();
    for segment in segments {
        match segment {
            Segment::Table(table) => {
                let (t_x0, t_top, t_x1, t_bottom) = table.bbox();
                let cleaned = cleaned_table_rows(table, &chars);
                if !cleaned.is_empty() {
                    parts.push(markdown_table(&cleaned));
                }
                let band_words: Vec<&PdfWord> =
                    words_refs.iter().copied().filter(|w| t_top <= (w.top + w.bottom) / 2.0 && (w.top + w.bottom) / 2.0 < t_bottom).collect();
                let orphaned: Vec<&PdfWord> =
                    band_words.into_iter().filter(|w| w.x1 < t_x0 - TABLE_ORPHAN_X_PADDING || w.x0 > t_x1 + TABLE_ORPHAN_X_PADDING).collect();
                if !orphaned.is_empty() {
                    let text = columns_from_words(&orphaned, page_width, COLUMN_LINE_TOL);
                    parts.push(text.unwrap_or_else(|| words_to_lines(&orphaned, COLUMN_LINE_TOL)));
                }
            }
            Segment::Prose(top, bottom) => {
                let band_words: Vec<&PdfWord> =
                    words_refs.iter().copied().filter(|w| top <= (w.top + w.bottom) / 2.0 && (w.top + w.bottom) / 2.0 < bottom).collect();
                if band_words.is_empty() {
                    continue;
                }
                let text = columns_from_words(&band_words, page_width, COLUMN_LINE_TOL);
                parts.push(text.unwrap_or_else(|| words_to_lines(&band_words, COLUMN_LINE_TOL)));
            }
        }
    }

    let result = parts.iter().filter(|p| !p.trim().is_empty()).cloned().collect::<Vec<_>>().join("\n\n");
    match &rotated_block {
        Some(block) if !result.trim().is_empty() => format!("{result}\n\n{block}"),
        Some(block) => block.clone(),
        None => result,
    }
}

// --- running header/footer removal ---------------------------------------

/// `_boilerplate_key`: collapse every run of digits to `#`, so "Page 5 of 20"
/// and "Page 6 of 20" count as the same recurring line.
pub fn boilerplate_key(line: &str) -> String {
    let stripped = line.trim_matches(|c: char| c.is_whitespace());
    let mut out = String::with_capacity(stripped.len());
    let mut in_digits = false;
    for c in stripped.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
                in_digits = true;
            }
        } else {
            out.push(c);
            in_digits = false;
        }
    }
    out
}

/// `_identify_boilerplate_lines`: which normalized keys recur at a page edge on
/// at least `BOILERPLATE_THRESHOLD` of pages. Looks only; changes nothing.
pub fn identify_boilerplate_lines(pages: &[String]) -> (Vec<String>, Vec<String>) {
    use std::collections::HashMap;
    if pages.len() < BOILERPLATE_MIN_PAGES {
        return (Vec::new(), Vec::new());
    }
    let mut header: HashMap<String, usize> = HashMap::new();
    let mut footer: HashMap<String, usize> = HashMap::new();
    for text in pages {
        let lines: Vec<&str> =
            text.split('\n').filter(|l| !l.trim_matches(|c: char| c.is_whitespace()).is_empty()).collect();
        for line in lines.iter().take(BOILERPLATE_EDGE_LINES) {
            *header.entry(boilerplate_key(line)).or_insert(0) += 1;
        }
        let tail_start = lines.len().saturating_sub(BOILERPLATE_EDGE_LINES);
        for line in &lines[tail_start..] {
            *footer.entry(boilerplate_key(line)).or_insert(0) += 1;
        }
    }
    let total = pages.len() as f64;
    let pick = |m: HashMap<String, usize>| -> Vec<String> {
        let mut v: Vec<String> =
            m.into_iter().filter(|(_, c)| *c as f64 / total >= BOILERPLATE_THRESHOLD).map(|(k, _)| k).collect();
        v.sort();
        v
    };
    (pick(header), pick(footer))
}

/// `_remove_boilerplate_lines`: pop matching lines inward from each edge only,
/// stopping at the first line that is neither blank nor boilerplate.
///
/// The interior is never touched. An earlier version of the Python filtered
/// blank lines out of the whole page before this loop, which silently deleted
/// every blank line in the body of any document that had a running footer.
pub fn remove_boilerplate_lines(pages: &[String], header_keys: &[String], footer_keys: &[String]) -> Vec<String> {
    if header_keys.is_empty() && footer_keys.is_empty() {
        return pages.to_vec();
    }
    let is_blank = |s: &str| s.trim_matches(|c: char| c.is_whitespace()).is_empty();
    pages
        .iter()
        .map(|text| {
            let mut lines: Vec<&str> = text.split('\n').collect();
            while let Some(last) = lines.last() {
                if is_blank(last) || footer_keys.contains(&boilerplate_key(last)) {
                    lines.pop();
                } else {
                    break;
                }
            }
            while let Some(first) = lines.first() {
                if is_blank(first) || header_keys.contains(&boilerplate_key(first)) {
                    lines.remove(0);
                } else {
                    break;
                }
            }
            lines.join("\n")
        })
        .collect()
}

/// `_strip_repeating_boilerplate`.
pub fn strip_repeating_boilerplate(pages: &[String]) -> Vec<String> {
    let (h, f) = identify_boilerplate_lines(pages);
    remove_boilerplate_lines(pages, &h, &f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, x0: f64, x1: f64, top: f64) -> PdfWord {
        PdfWord { text: text.into(), x0, x1, top, bottom: top + 10.0, doctop: top, upright: true }
    }

    fn c(text: &str, x0: f64, x1: f64, top: f64) -> PdfChar {
        PdfChar {
            text: text.into(),
            x0,
            x1,
            top,
            bottom: top + 10.0,
            doctop: top,
            upright: true,
            size: 10.0,
            fontname: "F1".into(),
        }
    }

    #[test]
    fn boilerplate_key_collapses_digit_runs() {
        assert_eq!(boilerplate_key("Page 5 of 20"), "Page # of #");
        assert_eq!(boilerplate_key("  Page 12 of 20  "), "Page # of #");
        assert_eq!(boilerplate_key("Page 5 of 20"), boilerplate_key("Page 6 of 20"));
        assert_eq!(boilerplate_key("no digits"), "no digits");
    }

    #[test]
    fn boilerplate_needs_min_pages() {
        // Below BOILERPLATE_MIN_PAGES the "repeats across the document" signal
        // is not trusted at all, however often a line recurs.
        let pages: Vec<String> = vec!["Footer".into(), "Footer".into(), "Footer".into()];
        assert_eq!(identify_boilerplate_lines(&pages), (Vec::new(), Vec::new()));
    }

    /// Pages with enough distinct body lines that only the genuine running
    /// header/footer sit inside the `BOILERPLATE_EDGE_LINES` windows on every
    /// page. Body lines deliberately vary by a LETTER, not a digit --
    /// `boilerplate_key` collapses digit runs, so "body A1"/"body A2" would
    /// normalize to the same key and be misread as recurring boilerplate.
    fn boilerplate_pages() -> Vec<String> {
        (1..=5)
            .map(|i| {
                let s = (b'a' + (i as u8) - 1) as char;
                format!(
                    "ACME Corp\nbody A{s}\nbody B{s}\n\nbody C{s}\nbody D{s}\nbody E{s}\nbody F{s}\nPage {i} of 5"
                )
            })
            .collect()
    }

    #[test]
    fn boilerplate_identified_and_removed_from_edges_only() {
        // Cross-checked against oss-launch's own `_identify_boilerplate_lines`
        // / `_strip_repeating_boilerplate` on this exact input.
        let pages = boilerplate_pages();
        let (h, f) = identify_boilerplate_lines(&pages);
        assert_eq!(h, vec!["ACME Corp".to_string()]);
        assert_eq!(f, vec!["Page # of #".to_string()]);

        let cleaned = strip_repeating_boilerplate(&pages);
        assert_eq!(cleaned[0], "body Aa\nbody Ba\n\nbody Ca\nbody Da\nbody Ea\nbody Fa");
        for page in &cleaned {
            assert!(!page.contains("ACME Corp"), "running header must be removed");
            assert!(!page.contains("Page "), "running footer must be removed");
        }
    }

    #[test]
    fn boilerplate_removal_does_not_touch_interior_blank_lines() {
        // The regression the Python docstring calls out: an earlier version
        // filtered blank lines out of the WHOLE page before the edge loops ran,
        // silently deleting every blank line in the body of any document with a
        // running footer. The blank line between "body Ba" and "body Ca" is the
        // guard.
        let cleaned = strip_repeating_boilerplate(&boilerplate_pages());
        assert!(cleaned[0].contains("body Ba\n\nbody Ca"), "interior blank line must survive: {:?}", cleaned[0]);
    }

    #[test]
    fn short_near_identical_pages_are_stripped_entirely() {
        // Not a bug, and verified against the real Python: with
        // BOILERPLATE_EDGE_LINES = 5, every line of a page with <= 5 non-empty
        // lines is BOTH a header and a footer candidate, so on 4+ near-identical
        // pages all of them clear the recurrence threshold and the whole page is
        // removed. Pinned so a future "fix" to the edge windows is a deliberate
        // deviation rather than an accident.
        let pages: Vec<String> = (1..=4).map(|i| format!("HDR\npara one\n\npara two\nFTR {i}")).collect();
        assert_eq!(strip_repeating_boilerplate(&pages), vec!["", "", "", ""]);
    }

    #[test]
    fn stale_overprinted_digits_keep_the_last_draw() {
        // "73" where the 7 is a stale ghost under the real 3.
        let chars = vec![c("7", 10.0, 16.0, 700.0), c("3", 10.0, 16.0, 700.0)];
        let out = drop_stale_overprinted_digits(&chars);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "3", "the last-drawn digit is the visible one");
    }

    #[test]
    fn stale_digit_drop_is_digit_only() {
        // A combining mark over a base letter shares an anchor point and must
        // never be dropped -- the reason this is scoped to digits.
        let chars = vec![c("\u{0A30}", 10.0, 16.0, 700.0), c("\u{0A4C}", 10.0, 16.0, 700.0)];
        assert_eq!(drop_stale_overprinted_digits(&chars).len(), 2);
        // Two different non-digit characters at one spot are also left alone.
        let chars = vec![c("a", 10.0, 16.0, 700.0), c("b", 10.0, 16.0, 700.0)];
        assert_eq!(drop_stale_overprinted_digits(&chars).len(), 2);
    }

    #[test]
    fn non_latin_detection_matches_the_python_intent() {
        assert!(!is_non_latin_char_group("a"));
        assert!(!is_non_latin_char_group("é"));       // accented Latin
        assert!(!is_non_latin_char_group("\u{FB01}")); // fi ligature
        assert!(!is_non_latin_char_group("5"));        // digits are script-neutral
        assert!(!is_non_latin_char_group("."));        // non-alphabetic
        assert!(is_non_latin_char_group("क"));         // Devanagari
        assert!(is_non_latin_char_group("中"));        // Han
        assert!(is_non_latin_char_group("Ω"));         // Greek
        assert!(is_non_latin_char_group("д"));         // Cyrillic
    }

    #[test]
    fn x_tolerance_tightens_only_for_latin_majority_pages() {
        let latin: Vec<PdfChar> = "hello world".chars().map(|ch| c(&ch.to_string(), 0.0, 5.0, 0.0)).collect();
        assert_eq!(page_word_x_tolerance(&latin), LATIN_SCRIPT_X_TOLERANCE);

        let indic: Vec<PdfChar> = "कखगघङचछजझ".chars().map(|ch| c(&ch.to_string(), 0.0, 5.0, 0.0)).collect();
        assert_eq!(page_word_x_tolerance(&indic), DEFAULT_X_TOLERANCE);

        assert_eq!(page_word_x_tolerance(&[]), DEFAULT_X_TOLERANCE);
    }

    #[test]
    fn heading_marker_regex_shape() {
        for good in ["1.", "17.", "999.", "iv.", "IV.", "a)", "Z)", "a.", "A."] {
            assert!(is_heading_marker(good), "{good} should match");
        }
        for bad in ["1234.", "hello.", "1", ".", "(a)", "iv", "abcdefghi."] {
            assert!(!is_heading_marker(bad), "{bad} should not match");
        }
    }

    /// Regression: a word starting with any multi-byte UTF-8 character used to panic with
    /// "byte index N is not a char boundary" instead of returning false -- found via a real
    /// PDF containing en-dash ("–") bullet points (2026-08-22). None of these can match the
    /// regex's ASCII-only character classes regardless of shape, so `false` is also the
    /// semantically correct answer, not just the crash-safe one.
    #[test]
    fn heading_marker_non_ascii_does_not_panic() {
        for s in ["–", "– ", "—.", "café.", "日本語)", "\u{feff}."] {
            assert!(!is_heading_marker(s), "{s:?} should not match and must not panic");
        }
    }

    #[test]
    fn words_to_lines_groups_by_top_and_sorts_by_x() {
        let ws = [w("world", 60.0, 90.0, 100.0), w("hello", 10.0, 50.0, 100.0), w("next", 10.0, 40.0, 130.0)];
        let refs: Vec<&PdfWord> = ws.iter().collect();
        assert_eq!(words_to_lines(&refs, COLUMN_LINE_TOL), "hello world\nnext");
    }

    #[test]
    fn row_grouping_anchor_does_not_chain() {
        // Unlike pdfplumber's cluster_objects, the anchor stays at the first
        // word's top, so 0/3/6 is TWO rows, not one chained row.
        let ws = [w("a", 0.0, 5.0, 0.0), w("b", 10.0, 15.0, 3.0), w("c", 20.0, 25.0, 6.0)];
        let refs: Vec<&PdfWord> = ws.iter().collect();
        assert_eq!(words_to_lines(&refs, 3.0), "a b\nc");
    }

    #[test]
    fn gutter_detected_between_two_columns() {
        let mut ws = Vec::new();
        for i in 0..10 {
            ws.push(w("left", 50.0, 250.0, i as f64 * 20.0));
            ws.push(w("right", 350.0, 550.0, i as f64 * 20.0));
        }
        let gutter = detect_column_gutter(&ws, 612.0).expect("a 100pt gap should be found");
        assert!((250.0..=350.0).contains(&gutter), "gutter {gutter} should sit in the gap");
    }

    #[test]
    fn no_gutter_on_single_column_text() {
        let ws: Vec<PdfWord> = (0..10).map(|i| w("line", 50.0, 550.0, i as f64 * 20.0)).collect();
        assert_eq!(detect_column_gutter(&ws, 612.0), None);
    }

    #[test]
    fn columns_emit_left_then_right() {
        let mut ws = Vec::new();
        for i in 0..6 {
            ws.push(w(&format!("L{i}"), 50.0, 250.0, i as f64 * 20.0));
            ws.push(w(&format!("R{i}"), 350.0, 550.0, i as f64 * 20.0));
        }
        let refs: Vec<&PdfWord> = ws.iter().collect();
        let out = columns_from_words(&refs, 612.0, COLUMN_LINE_TOL).expect("two columns");
        let l0 = out.find("L0").unwrap();
        let l5 = out.find("L5").unwrap();
        let r0 = out.find("R0").unwrap();
        assert!(l0 < l5, "left column stays in order");
        assert!(l5 < r0, "the whole left column precedes the right one");
    }

    #[test]
    fn columns_returns_none_when_one_side_is_empty() {
        // A false-positive gutter on genuinely single-column content.
        let ws: Vec<PdfWord> = (0..6).map(|i| w("L", 50.0, 250.0, i as f64 * 20.0)).collect();
        let refs: Vec<&PdfWord> = ws.iter().collect();
        assert_eq!(columns_from_words(&refs, 612.0, COLUMN_LINE_TOL), None);
    }

    #[test]
    fn spanning_line_is_kept_whole() {
        let mut ws = Vec::new();
        for i in 0..6 {
            ws.push(w(&format!("L{i}"), 50.0, 250.0, i as f64 * 20.0));
            ws.push(w(&format!("R{i}"), 350.0, 550.0, i as f64 * 20.0));
        }
        // A full-width line whose words flow continuously through the gutter
        // with only ordinary spacing.
        for (k, x) in (50..550).step_by(50).enumerate() {
            ws.push(w(&format!("S{k}"), x as f64, x as f64 + 48.0, 200.0));
        }
        let refs: Vec<&PdfWord> = ws.iter().collect();
        let out = columns_from_words(&refs, 612.0, COLUMN_LINE_TOL).expect("two columns");
        assert!(out.contains("S0 S1 S2"), "spanning line must stay on one line: {out}");
    }

    #[test]
    fn rotated_block_uses_natural_stream_order() {
        let chars = vec![
            PdfChar { upright: false, ..c("2", 5.0, 10.0, 100.0) },
            PdfChar { upright: false, ..c("0", 5.0, 10.0, 110.0) },
            c("body", 100.0, 200.0, 50.0),
        ];
        assert_eq!(build_rotated_block(&chars, &[]).as_deref(), Some("20"));
        assert_eq!(build_rotated_block(&[c("only upright", 0.0, 10.0, 0.0)], &[]), None);
    }

    #[test]
    fn rotated_block_excludes_chars_inside_a_table_bbox() {
        let chars = vec![
            PdfChar { upright: false, ..c("2", 5.0, 10.0, 100.0) },
            PdfChar { upright: false, ..c("0", 5.0, 10.0, 110.0) },
        ];
        // Both rotated chars' midpoints fall inside this bbox.
        assert_eq!(build_rotated_block(&chars, &[(0.0, 95.0, 15.0, 115.0)]), None);
    }

    #[test]
    fn page_text_dedupes_a_fake_bold_title() {
        // The same run drawn twice at the same spot must not come out doubled.
        let mut chars = Vec::new();
        for (i, ch) in "Title".chars().enumerate() {
            let x = 72.0 + i as f64 * 6.0;
            chars.push(c(&ch.to_string(), x, x + 6.0, 100.0));
        }
        let doubled: Vec<PdfChar> = chars.iter().cloned().chain(chars.iter().cloned()).collect();
        assert_eq!(extract_pdf_page_text(&doubled, &[], 612.0, 792.0), "Title");
    }

    #[test]
    fn page_text_renders_a_detected_table_as_markdown_and_leaves_prose_around_it() {
        // A 2x2 ruled table (4 rects) sitting between two prose lines.
        let table_objs = vec![
            GraphicsObj::Rect { x0: 0.0, top: 100.0, x1: 50.0, bottom: 120.0 },
            GraphicsObj::Rect { x0: 50.0, top: 100.0, x1: 100.0, bottom: 120.0 },
            GraphicsObj::Rect { x0: 0.0, top: 120.0, x1: 50.0, bottom: 140.0 },
            GraphicsObj::Rect { x0: 50.0, top: 120.0, x1: 100.0, bottom: 140.0 },
        ];
        let chars = vec![
            c("Above", 0.0, 40.0, 50.0),
            c("A1", 5.0, 15.0, 105.0),
            c("B1", 60.0, 70.0, 105.0),
            c("A2", 5.0, 15.0, 125.0),
            c("B2", 60.0, 70.0, 125.0),
            c("Below", 0.0, 40.0, 200.0),
        ];
        let text = extract_pdf_page_text(&chars, &table_objs, 612.0, 792.0);
        assert!(text.contains("Above"), "{text}");
        assert!(text.contains("Below"), "{text}");
        assert!(text.contains("| A1 | B1 |"), "{text}");
        assert!(text.contains("| --- | --- |"), "{text}");
        assert!(text.contains("| A2 | B2 |"), "{text}");
    }

    #[test]
    fn page_text_ignores_an_implausible_single_column_table_detection() {
        // A single-column "table" (2 stacked rects with no shared vertical
        // divider) collapses to 1 populated column per row and must be
        // rejected by `is_plausible_table`, falling back to ordinary flow.
        let objs = vec![
            GraphicsObj::Rect { x0: 0.0, top: 100.0, x1: 100.0, bottom: 120.0 },
            GraphicsObj::Rect { x0: 0.0, top: 120.0, x1: 100.0, bottom: 140.0 },
        ];
        let chars = vec![c("Row one text", 5.0, 90.0, 105.0), c("Row two text", 5.0, 90.0, 125.0)];
        let text = extract_pdf_page_text(&chars, &objs, 612.0, 792.0);
        assert!(!text.contains('|'), "implausible table must not render as markdown: {text}");
        assert!(text.contains("Row"), "{text}");
    }
}
