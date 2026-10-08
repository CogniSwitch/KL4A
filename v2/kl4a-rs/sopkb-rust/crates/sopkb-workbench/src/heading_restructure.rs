//! Port of `restructure_headings_llm` and its supporting functions from
//! `origin/integration/oss-launch`'s `tools/sopkb/sopkb/normalize.py` (NOT this
//! branch's own `tools/sopkb/sopkb/normalize.py`, which is 1538 lines shorter and
//! predates this whole pipeline entirely -- confirmed via `git diff HEAD
//! origin/integration/oss-launch -- tools/sopkb/sopkb/normalize.py`).
//!
//! **Why this lives here, not in `sopkb-core`:** `sopkb-core::normalize` has no
//! `sopkb-llm` dependency by design (`pdf/mod.rs`'s own doc comment already noted
//! this). `sopkb-workbench` depends on both, so this is where the seam
//! `sopkb_core::normalize::normalize_sources`'s `restructure` closure parameter
//! gets its real (non-`None`) implementation -- see `provider_hook` below, and
//! `ingest.rs`'s use of it.
//!
//! **Why this exists at all**: source PDFs and plain-text SOPs are almost never
//! authored with Markdown headings. Without this, every such source normalizes to
//! exactly one section (`sopkb-core`'s `extract_sections` has nothing to split on),
//! and mining has nothing to focus on per call. Confirmed directly against two real
//! documents this session: a real Cigna EOC PDF produced 1 section in Rust where a
//! real `oss-launch` run (given by the user) produced 89 Markdown headings on
//! otherwise byte-identical extracted text -- same content, same line positions,
//! the only difference being the headings this module inserts.
//!
//! **Deliberate deviation from Python, disclosed here rather than silently**:
//! Python's `normalize_pdf` calls `restructure_headings_llm` PER PAGE, independently,
//! before joining pages together -- a real limitation (no heading can be recognized
//! as spanning two pages, since each page's LLM call sees only that page's text).
//! This Rust port instead calls `restructure_headings_llm` ONCE on `pdf`/`markdown`/
//! `text` sources' fully-assembled text (`sopkb_core::normalize::normalize_sources`'s
//! injection point is after the whole source is normalized, not per-page), which is
//! simpler to wire (one call site for every restructurable type, rather than a
//! separate one inside PDF's own per-page loop) and produces more coherent section
//! boundaries for exactly the case Python's per-page limitation drops. `docx` is
//! never restructured (see `sopkb_core::normalize::LLM_RESTRUCTURABLE_TYPES`), same
//! as Python.
//!
//! **Updated 2026-08-24: the disclosed sequential-chunk-indexing simplification above
//! is now closed**, together with the matching gap in `sopkb_mining::okf_author`
//! (mining's own `ThreadPoolExecutor` fan-out) -- both were the same underlying real
//! oss-launch pattern (`ThreadPoolExecutor(max_workers=6).map(...)` over independent
//! per-item LLM calls) and both are now ported via `sopkb_core::parallel::parallel_map`.
//! Real motivation, not just parity for its own sake: a real user hit a mining run
//! that looked hung for over an hour on a document heading-restructuring had split
//! into 200+ sections -- sequential processing at that scale (a scale this exact
//! heading-restructuring feature is what newly makes common) made "slow" and "hung"
//! indistinguishable with no progress signal in between. See `build_heading_index`'s
//! own doc comment for the fix.

use regex::Regex;
use serde::Deserialize;
use sopkb_core::error::Result;
use sopkb_llm::{chat_call, Message};
use std::sync::OnceLock;

const CHUNK_TARGET_CHARS: usize = 10_000;
const CHUNK_CONTEXT_CHARS: usize = 800;
const RELEVEL_MAX_ATTEMPTS: u32 = 3;
/// Same retry budget/shape as `RELEVEL_MAX_ATTEMPTS` (see `relevel_heading_index_llm`),
/// now also applied to `index_chunk_llm` -- see that function's own doc comment for
/// why a bare single attempt was a real, user-visible correctness gap.
const INDEX_MAX_ATTEMPTS: u32 = 3;

/// Byte-exact copy of `_HEADING_INDEX_SYSTEM_PROMPT_TEXT`
/// (`origin/integration/oss-launch:tools/sopkb/sopkb/normalize.py`).
pub const HEADING_INDEX_SYSTEM_PROMPT: &str = "You are a document indexing engine. You will be given a fragment of a larger document.\n\nYour task: find every line that functions as a genuine section or subsection TITLE organizing the document's structure, at ANY nesting depth, and produce one index entry per title.\n\nRules:\n- A heading/title organizes the content that follows it under a name of its own. Different levels of the same document often use completely different numbering conventions - e.g. a top level might use \"SECTION A.\", a level below that \"3.\", below that Roman numerals \"I.\", \"II.\", below that letters \"a.\", \"b.\", and below that a dotted-decimal style like \"3.4.1.\" A numbering style SWITCH partway down the outline does not mean a line stopped being a heading - it's normal for outlines to mix conventions across depths.\n- CRITICAL - apply this same test at EVERY level of nesting, no matter how deep, and do not stop drilling into a subsection's own internal structure just because you already found headings above it. Two different things can look alike; tell them apart by CONTENT, not numbering style:\n  1. A GLOSSARY/DEFINITION-STYLE list, where every entry follows the same repetitive template regardless of topic (e.g. dozens of entries all shaped like \"<Term> means <definition>\" for unrelated terms, such as \"Widget means a tool used for X\" followed by \"Gadget means a device used for Y\"). These are POINTS, not headings - index only the ONE heading introducing the whole list, never the individual entries, even when one entry's own definition happens to contain a nested sub-list of its own criteria.\n  2. A list of DISTINCTLY-TITLED CLAUSES OR STEPS, where each entry names a different, substantial topic or step of its own rather than filling in the same template (e.g. \"I. Notice Period\", \"II. Confidentiality\", \"III. Indemnification\" - three different legal topics, not a repeated pattern). These ARE genuine subsections and must each be indexed individually, however deep they are nested.\n  - Worked example of depth: suppose a document has \"B. Payment Procedure\" as a subsection, and inside it, \"i. Method of Payment\" as a further subsection, and inside THAT, \"3.4.1. By Bank Transfer:\" and \"3.4.2. By Cheque:\" as two distinctly-named payment methods. All four of these are genuine headings at increasing depth (say levels 3, 4, 5) and must all be indexed - finding \"B. Payment Procedure\" and \"i. Method of Payment\" is not a reason to stop before also finding \"3.4.1. By Bank Transfer:\" and \"3.4.2. By Cheque:\" nested inside them, even though the numbering style changed twice along the way.\n- Do NOT treat an ordinary numbered/lettered list item as a heading unless it is clearly a section/subsection/step title rather than a plain list entry (the glossary rule above still applies at every depth, however deep).\n- The \"heading\" field you return MUST be an EXACT substring of the fragment, copied character-for-character (same spelling, punctuation, spacing, case). If you cannot quote it exactly, omit it.\n- If a candidate title is already preceded by literal Markdown heading syntax in the fragment (one or more `#` characters), copy only the title text itself as the \"heading\" field — do not include those leading `#` characters.\n- \"level_guess\": nesting depth as an integer, 2 = top-level section, 3 = next level down, 4, 5, 6... for each further level of nesting actually present. Do not cap yourself at 4 - go as deep as the real structure goes. This is a first guess only, based on this fragment alone, and may be corrected later with full-document context.\n- \"summary\": one or two plain sentences describing what the content under this heading actually covers, based on the text that follows it in this fragment. Do not just restate the heading text.\n- The fragment may open mid-list or mid-section with no heading in sight, continuing something from an earlier fragment — that's expected, not an error. Do not invent a heading to cover the opening lines; just start your index at the first genuine title you find (if any).\n- Do not include duplicate entries for the same title.\n\nIf the input is wrapped in `<CONTEXT_ONLY>...</CONTEXT_ONLY>` followed by `<FRAGMENT_TO_INDEX>...</FRAGMENT_TO_INDEX>` tags:\n- `<CONTEXT_ONLY>` is the tail end of the previous fragment. It is given only so you can tell whether an item at the very start of `<FRAGMENT_TO_INDEX>` continues a list/pattern that began before this fragment (e.g. the fragment opens with \"V. Termination\" and the context tail ends with \"...III. Confidentiality\\nIV. Indemnification\" — that tells you V is one more entry in that same list of distinctly-titled clauses, at the same level as III and IV, OR — if the context tail instead ends with a run of repetitive \"<Term> means <definition>\" glossary entries — that V continues that same glossary and is a point, not a heading).\n- Only index headings found within `<FRAGMENT_TO_INDEX>`. Never produce an entry for anything inside `<CONTEXT_ONLY>`.\n\nReturn ONLY a JSON array, no commentary, no markdown code fences. Each element: {\"heading\": \"...\", \"level_guess\": N, \"summary\": \"...\"}. Return [] if no genuine section/subsection headings are found (e.g. a fragment that is entirely a continuation of a list of points).\n";

/// Byte-exact copy of `_HEADING_RELEVEL_SYSTEM_PROMPT_TEXT` (same source file).
pub const HEADING_RELEVEL_SYSTEM_PROMPT: &str = "You are given an ordered index of section/subsection headings extracted from a single document, each with a short summary of what that section covers and an independent first-guess nesting level (guessed by analyzing only a small fragment of the document, without seeing the rest).\n\nBecause each heading's first guess was made without seeing the other headings, levels may be inconsistent — e.g. two headings that are really siblings might have been given different levels, or a heading that should nest under an earlier one doesn't (e.g. a numbered list that continues under the same parent section across a fragment boundary, like items 10-16 and 17-23 both belonging under the same \"GENERAL CONDITIONS\" section, should get the same level even if they were guessed differently).\n\nYour task: reassign the \"level\" for EVERY entry so the whole sequence forms one consistent, correctly nested hierarchy, based on the heading text, its summary, and its position relative to the other headings in the list.\n\nRules (strict):\n- Return a JSON array with EXACTLY the same number of entries, in the EXACT same order, as the input.\n- Each output entry's \"heading\" must be identical, character for character, to the corresponding input entry's \"heading\". Never reword, correct, or paraphrase it.\n- You may ONLY assign a \"level\" number (integer, 2 = top-level section, 3 = subsection, 4 = deeper, etc.). Do not add, remove, merge, split, or reorder entries.\n- Do not include \"summary\" or \"level_guess\" in your output.\n\nReturn ONLY the JSON array of {\"heading\": \"...\", \"level\": N}, no commentary, no code fences.\n";

fn strip_code_fence(raw: &str) -> String {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    let re = FENCE_RE.get_or_init(|| Regex::new(r"^```(?:json)?\s*|\s*```$").unwrap());
    re.replace_all(raw.trim(), "").into_owned()
}

/// `_split_into_chunks`: split on blank-line paragraph boundaries (never
/// mid-sentence) into pieces close to but not exceeding `target_size` -- measured
/// in bytes here, not Python's `len()` characters, a deliberate simplification: this
/// is a heuristic threshold for LLM context sizing, not a correctness boundary, so a
/// slightly-smaller-than-target chunk on multi-byte-heavy (non-Latin-script) text
/// only ever means marginally more chunks, never a wrong result.
fn split_into_chunks(content: &str, target_size: usize) -> Vec<String> {
    if content.len() <= target_size {
        return vec![content.to_string()];
    }
    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut current_len = 0usize;
    for paragraph in paragraphs {
        let paragraph_len = paragraph.len() + 2;
        if !current.is_empty() && current_len + paragraph_len > target_size {
            chunks.push(current.join("\n\n"));
            current.clear();
            current_len = 0;
        }
        current.push(paragraph);
        current_len += paragraph_len;
    }
    if !current.is_empty() {
        chunks.push(current.join("\n\n"));
    }
    chunks
}

/// `_find_original_span`: locate `candidate` within `text`, tolerating
/// whitespace-only drift -- a different AMOUNT or KIND of whitespace at a position
/// both sides genuinely have some (e.g. a heading's embedded line-wrap newline,
/// which the model may normalize to a single space when it echoes the heading
/// back), via a `\s+`-based fallback pattern. This does NOT bridge whitespace's
/// complete absence on one side (`\s+` cannot match zero characters), only a
/// mismatch in how much/what kind of whitespace is present on both. Returns byte
/// offsets using `text`'s own bytes at that span, never the model's copy of them.
fn find_original_span(text: &str, candidate: &str, search_from: usize) -> Option<(usize, usize, String)> {
    if let Some(idx) = text.get(search_from..).and_then(|s| s.find(candidate)) {
        let start = search_from + idx;
        return Some((start, start + candidate.len(), candidate.to_string()));
    }
    if let Some(idx) = text.find(candidate) {
        return Some((idx, idx + candidate.len(), candidate.to_string()));
    }

    // Whitespace-fuzzy fallback: split candidate into whitespace/non-whitespace
    // tokens, build a pattern where a whitespace token becomes `\s+` and every
    // other token is regex-escaped literal text.
    let pattern = fuzzy_whitespace_pattern(candidate);
    let re = Regex::new(&pattern).ok()?;
    for start_at in [search_from, 0] {
        if let Some(hay) = text.get(start_at..) {
            if let Some(m) = re.find(hay) {
                return Some((start_at + m.start(), start_at + m.end(), m.as_str().to_string()));
            }
        }
    }
    None
}

fn fuzzy_whitespace_pattern(candidate: &str) -> String {
    static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    let token_re = TOKEN_RE.get_or_init(|| Regex::new(r"(\s+)").unwrap());
    let mut pattern = String::new();
    let mut last = 0;
    for m in token_re.find_iter(candidate) {
        if m.start() > last {
            pattern.push_str(&regex::escape(&candidate[last..m.start()]));
        }
        pattern.push_str(r"\s+");
        last = m.end();
    }
    if last < candidate.len() {
        pattern.push_str(&regex::escape(&candidate[last..]));
    }
    pattern
}

#[derive(Debug, Clone, Deserialize)]
struct RawIndexEntry {
    heading: Option<String>,
    #[serde(default = "default_level_guess")]
    level_guess: i64,
    #[serde(default)]
    summary: String,
}

fn default_level_guess() -> i64 {
    2
}

#[derive(Debug, Clone)]
struct VerifiedEntry {
    heading: String,
    summary: String,
    level_guess: i64,
    start: usize,
}

/// `_index_chunk_llm`: ask the LLM for a short list of (heading, level_guess,
/// summary) entries found in this chunk. `context_tail`, when non-empty, is sent as
/// read-only `<CONTEXT_ONLY>` so a chunk opening mid-list has the sibling context
/// needed to classify its own first item correctly.
///
/// Retries up to `INDEX_MAX_ATTEMPTS` times (same shape as
/// `relevel_heading_index_llm`'s own retry loop) before giving up and treating this
/// chunk as headingless -- a bare single attempt meant ANY transient network/API
/// error (rate limit, timeout, one bad response) for even one chunk silently
/// dropped that chunk's real headings, and if it happened to hit most/all chunks in
/// a burst (e.g. several parallel chunk requests landing on a rate-limited or
/// momentarily slow endpoint at once), the WHOLE document could come back with zero
/// headings inserted -- collapsing to one giant section with no error surfaced
/// anywhere a GUI build's user could see it -- `eprintln!` alone reaches nowhere in
/// a GUI-subsystem release build, so `log`, when given, ALSO receives every warning
/// this function emits (see `provider_hook`'s own doc comment for where that
/// callback actually writes to). Retrying makes a transient failure much less
/// likely to cost real headings; it does not change the fallback behavior when
/// every attempt still fails.
fn index_chunk_llm(
    chunk_text: &str,
    context_tail: &str,
    profile_id: Option<&str>,
    log: Option<&(dyn Fn(&str) + Sync)>,
) -> Vec<VerifiedEntry> {
    let user_content = if context_tail.is_empty() {
        chunk_text.to_string()
    } else {
        format!("<CONTEXT_ONLY>\n{context_tail}\n</CONTEXT_ONLY>\n\n<FRAGMENT_TO_INDEX>\n{chunk_text}\n</FRAGMENT_TO_INDEX>")
    };
    let messages = [Message::system(HEADING_INDEX_SYSTEM_PROMPT), Message::user(user_content)];

    let mut entries: Vec<RawIndexEntry> = Vec::new();
    for attempt in 1..=INDEX_MAX_ATTEMPTS {
        let last_attempt = attempt == INDEX_MAX_ATTEMPTS;
        let raw = match chat_call(&messages, profile_id) {
            Ok(text) => text,
            // Matches Python's own JSON-parse-failure handling below (log and treat
            // as headingless once attempts are exhausted): a network/API error
            // mid-chunk is not meaningfully different from a parse failure for this
            // step's purposes, and one bad chunk should not discard every other
            // chunk's genuinely-found headings. Logged (unlike a plain silent drop)
            // because this failure mode is otherwise indistinguishable from "this
            // chunk genuinely has no headings" -- confirmed necessary after a real
            // run silently lost most of a document's headings with nothing in the
            // logs to explain why.
            Err(e) => {
                let action = if last_attempt { "treating as headingless" } else { "retrying" };
                let message = format!("[sopkb.normalize] chunk index request failed (attempt {attempt}/{INDEX_MAX_ATTEMPTS}), {action}: {e}");
                eprintln!("{message}");
                if let Some(log) = log {
                    log(&message);
                }
                continue;
            }
        };
        let stripped = strip_code_fence(&raw);
        match serde_json::from_str(&stripped) {
            Ok(parsed) => {
                entries = parsed;
                break;
            }
            Err(e) => {
                let action = if last_attempt { "treating as headingless" } else { "retrying" };
                let message = format!("[sopkb.normalize] chunk index JSON parse failed (attempt {attempt}/{INDEX_MAX_ATTEMPTS}), {action}: {e}");
                eprintln!("{message}");
                if let Some(log) = log {
                    log(&message);
                }
            }
        }
    }

    let mut verified = Vec::new();
    let mut cursor = 0usize;
    for entry in entries {
        let Some(candidate) = entry.heading.filter(|h| !h.is_empty()) else { continue };
        let Some((start, end, exact_text)) = find_original_span(chunk_text, &candidate, cursor) else { continue };
        cursor = cursor.max(end);
        verified.push(VerifiedEntry { heading: exact_text, summary: entry.summary, level_guess: entry.level_guess, start });
    }
    verified
}

#[derive(Debug, Clone)]
struct MergedEntry {
    heading: String,
    summary: String,
    level_guess: i64,
    full_offset: usize,
}

/// `build_heading_index`: run `index_chunk_llm` over every chunk -- fanned out
/// across up to [`MAX_PARALLEL_CHUNKS`] worker threads via
/// [`sopkb_core::parallel::parallel_map`], matching real oss-launch's own
/// `ThreadPoolExecutor(max_workers=6).map(...)` fan-out (see the module doc) -- then
/// merge into one document-order list with each heading's position expressed as a
/// full-document byte offset.
///
/// `on_progress`, when given, is called `(chunks_completed, total_chunks)` as each
/// chunk's LLM call returns -- during the fan-out itself, which is where all the
/// wall-clock time is spent, not after the fact once every chunk is already done.
///
/// `is_cancelled`, when given, is checked once per chunk, right before that chunk's
/// own LLM call would start (never mid-call -- see `mine_with_author`'s identical
/// caveat, the same "cooperative, not a hard abort" tradeoff applies here). A
/// cancelled chunk contributes no headings (as if it were headingless), same as any
/// other chunk whose LLM call failed or returned unusable JSON -- `index_chunk_llm`
/// already treats those uniformly, so cancellation needs no new outcome shape here.
fn build_heading_index(
    content: &str,
    chunks: &[String],
    profile_id: Option<&str>,
    on_progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    is_cancelled: Option<&(dyn Fn() -> bool + Sync)>,
    log: Option<&(dyn Fn(&str) + Sync)>,
) -> Vec<MergedEntry> {
    let mut chunk_starts = Vec::with_capacity(chunks.len());
    let mut cursor = 0usize;
    for chunk in chunks {
        let start = content[cursor..].find(chunk.as_str()).map(|i| cursor + i).unwrap_or(cursor);
        chunk_starts.push(start);
        cursor = start;
    }

    let total = chunks.len();
    let completed = std::sync::atomic::AtomicUsize::new(0);
    // Configurable (Settings) rather than hardcoded, no Python equivalent -- see
    // sopkb_config::settings::DEFAULT_MAX_PARALLEL_WORKERS's own doc comment.
    let max_workers = sopkb_config::max_parallel_workers();
    let per_chunk_entries: Vec<Vec<VerifiedEntry>> =
        sopkb_core::parallel::parallel_map(chunks, max_workers, |chunk_index, chunk| {
            let entries = if is_cancelled.is_some_and(|f| f()) {
                Vec::new()
            } else {
                let context_tail = if chunk_index == 0 {
                    String::new()
                } else {
                    let prev = &chunks[chunk_index - 1];
                    let tail_start = prev.len().saturating_sub(CHUNK_CONTEXT_CHARS);
                    // Byte-safe: walk forward to the nearest char boundary rather than
                    // slicing at an arbitrary byte offset, which can land mid-codepoint
                    // on non-Latin-script text.
                    let mut safe_start = tail_start;
                    while safe_start < prev.len() && !prev.is_char_boundary(safe_start) {
                        safe_start += 1;
                    }
                    prev[safe_start..].to_string()
                };
                index_chunk_llm(chunk, &context_tail, profile_id, log)
            };
            let done = completed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if let Some(cb) = on_progress {
                cb(done, total);
            }
            entries
        });

    let mut merged = Vec::new();
    for (chunk_index, entries) in per_chunk_entries.into_iter().enumerate() {
        for entry in entries {
            merged.push(MergedEntry {
                heading: entry.heading,
                summary: entry.summary,
                level_guess: entry.level_guess,
                full_offset: chunk_starts[chunk_index] + entry.start,
            });
        }
    }
    merged.sort_by_key(|e| e.full_offset);
    merged
}

fn table_block_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)(?:^\|.*\|[ \t]*\n?)+").unwrap())
}

/// `_filter_out_table_headings`: drop any heading candidate whose position falls
/// inside a Markdown table block (a contiguous run of `| cell | cell |` lines,
/// exactly what `markdown_table()`-shaped output looks like) -- the heading-index
/// prompt has no notion of table syntax and occasionally mistakes a table's
/// row-label cell for a genuine section title.
fn filter_out_table_headings(index: Vec<MergedEntry>, content: &str) -> Vec<MergedEntry> {
    let ranges: Vec<(usize, usize)> = table_block_re().find_iter(content).map(|m| (m.start(), m.end())).collect();
    if ranges.is_empty() {
        return index;
    }
    index.into_iter().filter(|entry| !ranges.iter().any(|&(s, e)| s <= entry.full_offset && entry.full_offset < e)).collect()
}

#[derive(Debug, Clone, Deserialize)]
struct RawRelevelEntry {
    heading: String,
    level: i64,
}

/// `relevel_heading_index_llm`: give the model the small merged index (headings +
/// summaries + first-guess levels), never the document itself, and have it assign
/// final, cross-chunk-consistent levels. Verified narrowly (same count, same order,
/// identical heading text -- only levels may differ); falls back to the unmodified
/// `level_guess` values if every attempt fails that check.
fn relevel_heading_index_llm(merged_index: Vec<MergedEntry>, profile_id: Option<&str>, max_attempts: u32) -> Vec<(MergedEntry, i64)> {
    #[derive(serde::Serialize)]
    struct RelevelInputEntry<'a> {
        heading: &'a str,
        summary: &'a str,
        level_guess: i64,
    }
    let relevel_input: Vec<RelevelInputEntry> =
        merged_index.iter().map(|e| RelevelInputEntry { heading: &e.heading, summary: &e.summary, level_guess: e.level_guess }).collect();
    let Ok(input_json) = serde_json::to_string(&relevel_input) else {
        return merged_index.into_iter().map(|e| {
            let level = e.level_guess;
            (e, level)
        }).collect();
    };

    for _attempt in 1..=max_attempts {
        let messages = [Message::system(HEADING_RELEVEL_SYSTEM_PROMPT), Message::user(input_json.clone())];
        let Ok(raw) = chat_call(&messages, profile_id) else { continue };
        let stripped = strip_code_fence(&raw);
        let Ok(releveled) = serde_json::from_str::<Vec<RawRelevelEntry>>(&stripped) else { continue };
        if releveled.len() != merged_index.len() {
            continue;
        }
        if releveled.iter().zip(merged_index.iter()).any(|(new, orig)| new.heading != orig.heading) {
            continue;
        }
        return merged_index.into_iter().zip(releveled).map(|(entry, r)| (entry, r.level)).collect();
    }
    eprintln!(
        "[sopkb.normalize] relevel_heading_index_llm: giving up after {max_attempts} attempt(s), keeping per-chunk level guesses as-is"
    );
    merged_index.into_iter().map(|e| {
        let level = e.level_guess;
        (e, level)
    }).collect()
}

/// `assemble_document_from_index`: deterministically insert a Markdown heading
/// marker (`#`/`##`/.../ per each entry's final level) immediately before each
/// heading's already-known byte position in `content`. No LLM involved; every
/// character of `content` besides the inserted marker text is reproduced
/// untouched, so losslessness holds by construction.
fn assemble_document_from_index(content: &str, releveled_index: &[(MergedEntry, i64)]) -> String {
    let mut out = String::with_capacity(content.len() + releveled_index.len() * 4);
    let mut last_pos = 0usize;
    for (entry, level) in releveled_index {
        out.push_str(&content[last_pos..entry.full_offset]);
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        let level = (*level).clamp(1, 6) as usize;
        out.push_str(&"#".repeat(level));
        out.push(' ');
        last_pos = entry.full_offset; // the heading's own text is still ahead in `content`, untouched
    }
    out.push_str(&content[last_pos..]);
    out
}

/// `restructure_headings_llm`: insert Markdown headings into a flat document so
/// `sopkb_core::normalize::extract_sections` has real structure to split on,
/// instead of one giant section per document. See the module doc for the full
/// design and disclosed deviations from Python.
///
/// `on_progress`, when given, is forwarded to [`build_heading_index`] unchanged
/// (see its own doc comment) -- the chunk-indexing fan-out is the only part of this
/// pipeline slow enough, on a large document, to need a progress signal at all;
/// releveling is one single LLM call over a small already-summarized index,
/// regardless of document size.
///
/// `is_cancelled`, when given, is likewise forwarded to [`build_heading_index`]
/// unchanged -- NOT checked again before the releveling call, deliberately: once
/// chunk-indexing has produced a real (possibly cancellation-truncated) heading
/// index, running that one small releveling call to completion and returning a
/// consistent, if partial, restructured document is preferable to discarding
/// already-fetched work over a flag that could only have flipped in a single-call
/// window.
///
/// `log`, when given, receives every per-chunk indexing warning `build_heading_index`
/// produces (see `index_chunk_llm`'s own doc comment for why this exists) -- purely
/// additive, `None` reproduces this function's exact prior behavior.
pub fn restructure_headings_llm(
    content: &str,
    profile_id: Option<&str>,
    on_progress: Option<&(dyn Fn(usize, usize) + Sync)>,
    is_cancelled: Option<&(dyn Fn() -> bool + Sync)>,
    log: Option<&(dyn Fn(&str) + Sync)>,
) -> Result<String> {
    let chunks = split_into_chunks(content, CHUNK_TARGET_CHARS);
    let heading_index = build_heading_index(content, &chunks, profile_id, on_progress, is_cancelled, log);
    let heading_index = filter_out_table_headings(heading_index, content);
    if heading_index.is_empty() {
        return Ok(content.to_string());
    }
    let releveled = relevel_heading_index_llm(heading_index, profile_id, RELEVEL_MAX_ATTEMPTS);
    Ok(assemble_document_from_index(content, &releveled))
}

/// Builds the `restructure` closure `sopkb_core::normalize::normalize_sources`
/// expects, or `None` for the fixture provider (reproducing pre-existing behavior
/// exactly). The one call site every ingest-pipeline entry point (desktop-tauri,
/// `sopkb-cli`, `sopkb-workbench::ingest`) should use, so "is this provider
/// LLM-backed" is decided in exactly one place.
///
/// `on_progress`, when given, is forwarded to [`restructure_headings_llm`] for every
/// restructurable source normalized through the returned closure -- there is
/// deliberately no per-source disambiguation in the callback signature itself
/// (`(chunks_done, chunks_total)` only, not which source): a caller wiring this to a
/// UI event already knows which source is currently normalizing from its own
/// surrounding context (e.g. desktop-tauri's ingest step is already scoped to one
/// step of one run), so threading a source id through here as well would be
/// redundant plumbing for no caller that actually needs it yet.
///
/// `log`, when given, receives every per-chunk indexing warning (network/API
/// failure, malformed JSON) as a plain `&str` -- structural facts only (step name,
/// chunk index, attempt number, error type), never document text or credentials.
/// Callers typically wire this to `sopkb_core::store::append_ingest_log(bundle_dir,
/// _)` so a GUI build's user has somewhere to actually find these warnings (see
/// that function's own doc comment); `None` reproduces this function's exact prior
/// (silent-except-`eprintln!`) behavior.
pub fn provider_hook<'a>(
    provider: &str,
    profile_id: Option<&str>,
    on_progress: Option<&'a (dyn Fn(usize, usize) + Sync)>,
    is_cancelled: Option<&'a (dyn Fn() -> bool + Sync)>,
    log: Option<&'a (dyn Fn(&str) + Sync)>,
) -> Option<Box<dyn Fn(&str) -> std::result::Result<String, String> + Sync + 'a>> {
    if provider != "azure-llm" && provider != "llm" {
        return None;
    }
    let profile_id = profile_id.map(|s| s.to_string());
    Some(Box::new(move |text: &str| restructure_headings_llm(text, profile_id.as_deref(), on_progress, is_cancelled, log).map_err(|e| e.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_into_chunks_single_chunk_when_under_target() {
        let content = "Short document.\n\nTwo paragraphs.";
        assert_eq!(split_into_chunks(content, 10_000), vec![content.to_string()]);
    }

    #[test]
    fn split_into_chunks_never_splits_mid_paragraph() {
        let content = format!("{}\n\n{}\n\n{}", "a".repeat(40), "b".repeat(40), "c".repeat(40));
        let chunks = split_into_chunks(&content, 50);
        assert_eq!(chunks.len(), 3, "each paragraph exceeds half the target alone, so each becomes its own chunk");
        for chunk in &chunks {
            assert!(chunk == &"a".repeat(40) || chunk == &"b".repeat(40) || chunk == &"c".repeat(40));
        }
    }

    #[test]
    fn find_original_span_exact_match() {
        let text = "one two three";
        assert_eq!(find_original_span(text, "two", 0), Some((4, 7, "two".to_string())));
    }

    #[test]
    fn find_original_span_tolerates_whitespace_drift() {
        // `\s+` tolerates a DIFFERENT AMOUNT/KIND of whitespace at a position both
        // sides genuinely have some at (e.g. a line-wrapped heading's embedded
        // newline, normalized to a single space by the model when it echoed the
        // heading back) -- not whitespace's complete ABSENCE on one side. A
        // heading split across a PDF's own line wrap is a realistic source of
        // exactly this kind of drift.
        let text = "Section 3.4.1\nBy Bank Transfer:\n\nBody text.";
        let candidate = "Section 3.4.1 By Bank Transfer:";
        let found = find_original_span(text, candidate, 0).unwrap();
        assert_eq!(&text[found.0..found.1], "Section 3.4.1\nBy Bank Transfer:");
    }

    #[test]
    fn find_original_span_none_when_truly_absent() {
        assert_eq!(find_original_span("one two three", "nowhere", 0), None);
    }

    #[test]
    fn assemble_document_from_index_is_lossless_outside_inserted_markers() {
        let content = "Intro text.\nFirst Heading\nBody one.\nSecond Heading\nBody two.\n";
        let first_offset = content.find("First Heading").unwrap();
        let second_offset = content.find("Second Heading").unwrap();
        let index = vec![
            (MergedEntry { heading: "First Heading".into(), summary: String::new(), level_guess: 2, full_offset: first_offset }, 2),
            (MergedEntry { heading: "Second Heading".into(), summary: String::new(), level_guess: 3, full_offset: second_offset }, 3),
        ];
        let out = assemble_document_from_index(content, &index);
        assert_eq!(out, "Intro text.\n## First Heading\nBody one.\n### Second Heading\nBody two.\n");
    }

    #[test]
    fn filter_out_table_headings_drops_offsets_inside_a_table_block() {
        let content = "Prose.\n\n| Process for X | On receipt of Y |\n| --- | --- |\n\nMore prose.\n";
        let inside_table = content.find("Process for X").unwrap();
        let outside_table = content.find("More prose").unwrap();
        let index = vec![
            MergedEntry { heading: "Process for X".into(), summary: String::new(), level_guess: 2, full_offset: inside_table },
            MergedEntry { heading: "More prose".into(), summary: String::new(), level_guess: 2, full_offset: outside_table },
        ];
        let filtered = filter_out_table_headings(index, content);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].heading, "More prose");
    }

    #[test]
    fn provider_hook_none_for_fixture() {
        assert!(provider_hook("fixture", None, None, None, None).is_none());
    }

    #[test]
    fn provider_hook_some_for_azure_llm() {
        assert!(provider_hook("azure-llm", None, None, None, None).is_some());
    }

    fn with_settings_path<F: FnOnce()>(f: F) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &path) };
        f();
        unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
    }

    fn save_full_profile() {
        let profile = sopkb_config::ModelProfile {
            id: "p1".into(),
            name: "One".into(),
            base_url: "https://example.test".into(),
            api_key: "secret-key".into(),
            model: "gpt-x".into(),
            ..Default::default()
        };
        sopkb_config::save_profile(&profile).unwrap();
    }

    // These exercise `restructure_headings_llm` end to end against a mocked
    // transport by way of a real `chat_call` -- possible only because `chat_call`
    // itself takes no transport parameter (it always uses `UreqTransport`), so
    // these tests validate everything EXCEPT the actual HTTP call by pointing
    // `SOPKB_SETTINGS_PATH` at a scratch profile and asserting on the pure
    // chunking/filtering/assembly logic directly instead. Direct-transport
    // mocking of `restructure_headings_llm`'s own LLM calls isn't possible without
    // a transport-injectable variant, which none of this module's callers need
    // today (`sopkb-mining`'s equivalent -- `azure_llm_author_with_transport` --
    // exists specifically because `sopkb-mining` has its own test harness needing
    // it; this module's tests instead validate the deterministic, non-LLM parts
    // directly, which is where all of this module's own logic actually lives).
    #[test]
    fn restructure_headings_llm_returns_input_unchanged_when_index_step_finds_nothing() {
        with_settings_path(|| {
            save_full_profile();
            // A profile with no reachable base_url's request will fail at the
            // network layer inside index_chunk_llm on every one of its
            // INDEX_MAX_ATTEMPTS retries, which is then treated as "no headings
            // found in this chunk" (see index_chunk_llm's doc comment) -- so the
            // whole document comes back unchanged, exactly like Python's
            // "no section/subsection headings found" branch.
            let content = "Flat content with no headings.\n";
            let result = restructure_headings_llm(content, None, None, None, None).unwrap();
            assert_eq!(result, content);
        });
    }

    #[test]
    fn build_heading_index_is_cancelled_true_never_calls_index_chunk_llm() {
        // `index_chunk_llm` has no transport-injection seam (unlike
        // `mine_with_author`'s `author` closure) -- there is no way to directly
        // count calls, so this proves the negative the same way this file's own
        // `restructure_headings_llm_returns_input_unchanged_when_index_step_finds_nothing`
        // test already does: without a reachable base_url, a REAL (uncancelled)
        // call would fail at the network layer, which `index_chunk_llm` treats
        // identically to "cancelled" (empty entries) -- so this test's only real
        // claim is "cancellation doesn't panic and produces the documented
        // graceful-fallback shape", not "the network was never touched". Multiple
        // chunks (a content long enough to split) exercise the `parallel_map`
        // fan-out itself, not just the single-chunk path.
        let content = format!("{}\n\n{}\n\n{}", "a".repeat(20_000), "b".repeat(20_000), "c".repeat(20_000));
        let chunks = split_into_chunks(&content, CHUNK_TARGET_CHARS);
        assert!(chunks.len() > 1, "test setup: need multiple chunks to exercise the fan-out");
        let is_cancelled = || true;
        let index = build_heading_index(&content, &chunks, None, None, Some(&is_cancelled), None);
        assert!(index.is_empty(), "a cancelled run must contribute no headings");
    }
}
