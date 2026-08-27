//! Guards P-M17 (`AUTHOR_SYSTEM_PROMPT` must be byte-exact, including the one real
//! embedded newline inside the last JSON example). Independently re-derives the
//! expected text directly from `tools/sopkb/sopkb/okf_author.py` at test time (rather
//! than trusting a static copy that could itself silently drift from the Python
//! source) and asserts byte equality with `sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT`.
//!
//! Handles the one subtlety that makes this "a subtle transcription trap" (per
//! PORT_PLAN.md): the .py file is git-tracked as LF but may be checked out as CRLF on
//! Windows (`core.autocrlf`), while CPython's tokenizer applies universal-newline
//! translation to source files at compile time regardless of the file's on-disk line
//! endings -- so the TRUE runtime value of the Python string is always LF-only. This
//! test normalizes CRLF -> LF before comparing, matching that runtime semantics rather
//! than whatever this checkout's line endings happen to be.

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

/// Extracts the exact text Python's `AUTHOR_SYSTEM_PROMPT = """...."""` triple-quoted
/// string evaluates to, from the raw `.py` source.
fn expected_prompt_from_python_source() -> String {
    let path = repo_root().join("tools/sopkb/sopkb/okf_author.py");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let start_marker = "AUTHOR_SYSTEM_PROMPT = \"\"\"";
    let start = text.find(start_marker).expect("AUTHOR_SYSTEM_PROMPT marker not found") + start_marker.len();
    let end = text[start..].find("\"\"\"").expect("closing triple-quote not found") + start;
    let raw = &text[start..end];
    // CPython's tokenizer applies universal-newline translation to the whole source
    // file before parsing string literals -- normalize CRLF (and lone CR) to LF first,
    // matching that, not this checkout's on-disk line-ending state.
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    // The ONE actual escape sequence in this block is the literal two-char `\n` inside
    // `"body":"# Record contraindications\n"` -- a real Python string-literal escape,
    // which becomes an actual newline character in the string's runtime value. Nothing
    // else in this block uses a backslash escape (verified: exactly one `\` byte is
    // present in the raw extracted text).
    assert_eq!(normalized.matches('\\').count(), 1, "expected exactly one backslash escape in the prompt source");
    normalized.replace("\\n", "\n")
}

#[test]
fn author_system_prompt_is_byte_exact_with_python_source() {
    let expected = expected_prompt_from_python_source();
    assert_eq!(sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT, expected);
}

#[test]
fn author_system_prompt_ends_with_trailing_newline_and_has_no_markdown_fence_mentioned_as_literal_backticks() {
    // Sanity checks independent of the Python-source comparison above: the prompt
    // must end with a newline (P-M17) and must literally instruct "no Markdown
    // fences" near the top (guards against an accidental truncation during any future
    // edit of this constant).
    assert!(sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT.ends_with('\n'));
    assert!(sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT.starts_with("You are an OKF v0.2 author for SOP knowledge.\n"));
    assert!(sopkb_mining::okf_author::AUTHOR_SYSTEM_PROMPT.contains("no Markdown fences"));
}
