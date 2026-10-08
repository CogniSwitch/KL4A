//! Port of `kl4a/kl4a/ids.py` — the *shared* id-helper module, distinct from
//! `kl4a/codekb/ids.py` (already ported as `codekb::ids`, with its own
//! `as_posix`/`code_*_id_for` functions scoped to code-bundle ids). This
//! module holds `slugify`/`bounded_id`, used across bundle profiles
//! (codekb symbols/relations, apikb capability cards/evidence) for
//! filename-safe, length-bounded identifiers.
//!
//! Grounded via `tools-code` MCP `code_symbols_get` against
//! `kl4a/kl4a/ids.py` (`source-kl4a-kl4a-ids-py`).

use sha2::{Digest, Sha256};

/// Port of `kl4a.kl4a.ids.slugify` (`kl4a/kl4a/ids.py:8-10`).
///
/// ```python
/// def slugify(value: str) -> str:
///     slug = re.sub(r"[^a-zA-Z0-9]+", "-", value.strip().lower()).strip("-")
///     return slug or "item"
/// ```
pub fn slugify(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    let mut slug = String::with_capacity(lowered.len());
    let mut last_was_sep = false;
    for ch in lowered.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_sep = false;
        } else if !last_was_sep {
            slug.push('-');
            last_was_sep = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Port of `kl4a.kl4a.ids.bounded_id` (`kl4a/kl4a/ids.py:29-46`).
///
/// ```python
/// def bounded_id(prefix: str, value: str, *, max_length: int) -> str:
///     body = slugify(value)
///     candidate = f"{prefix}-{body}"
///     if len(candidate) <= max_length:
///         return candidate
///     digest = hashlib.sha256(value.encode("utf-8")).hexdigest()[:12]
///     keep = max_length - len(prefix) - len(digest) - 2
///     return f"{prefix}-{body[:keep].rstrip('-')}-{digest}"
/// ```
///
/// Slug-based identifier that never exceeds `max_length`. A slug built from
/// a long qualified name, path or JSON pointer can outgrow what a filename
/// or link fragment can carry, so an over-long candidate is truncated and
/// disambiguated with a digest of the *full* value — two inputs that share
/// a truncated prefix still get distinct ids.
///
/// `keep` indexes into `body` by byte offset like Python's `body[:keep]`
/// indexes by codepoint; since `body` is already ASCII-only (produced by
/// [`slugify`]), byte and codepoint indexing coincide here.
pub fn bounded_id(prefix: &str, value: &str, max_length: usize) -> String {
    let body = slugify(value);
    let candidate = format!("{prefix}-{body}");
    if candidate.len() <= max_length {
        return candidate;
    }
    let digest_full = format!("{:x}", Sha256::digest(value.as_bytes()));
    let digest: String = digest_full.chars().take(12).collect();
    let keep = max_length
        .saturating_sub(prefix.len())
        .saturating_sub(digest.len())
        .saturating_sub(2);
    let truncated = body.get(..keep.min(body.len())).unwrap_or("");
    let truncated = truncated.trim_end_matches('-');
    format!("{prefix}-{truncated}-{digest}")
}
