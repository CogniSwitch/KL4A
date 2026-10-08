//! ID / slug generation. docs/port/port-mapping-a-core-data.md §3.1.

/// Unicode-strip, full-Unicode-lowercase, replace every maximal run of non-`[A-Za-z0-9]`
/// with a single `-`, strip leading/trailing `-`, and fall back to `"item"` if empty.
/// Non-ASCII letters are NOT transliterated -- they are simply replaced, same as any
/// other non-alphanumeric run (`"café"` -> `"caf"`, `"中文"` -> `"item"`).
pub fn slugify(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut in_run = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            in_run = false;
        } else if !in_run {
            out.push('-');
            in_run = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The source id is the SLUGIFIED FILE STEM, with no content-hash suffix.
///
/// This changed with the source-versioning subsystem (CATCHUP_PLAN.md workstream 2):
/// the old scheme was `<slug>-<first 12 hex chars of checksum>`, which made a source's
/// identity change every time its content changed -- the exact opposite of what a
/// version registry needs. Identity is now stable across edits and the *content* is
/// tracked by [`source_version_id_for`] instead, so `report.md` stays `report` through
/// `report:v1`, `report:v2`, ... . `stem` is the filename without its last suffix
/// (`Path::stem` equivalent -- callers pass this in rather than a `Path`, since it's
/// a pure string op).
///
/// Consequence, inherited deliberately from the reference implementation: two files
/// whose stems slugify identically (`a-b.md` and `a_b.md`, or `Report.md` in two
/// subdirectories) now collide onto one source id and are treated as versions of the
/// same source rather than as two sources. See the `colliding-stems` fixture case.
pub fn source_id_for(stem: &str) -> String {
    slugify(stem)
}

/// `"<source_id>:v<n>"`. The `:` is deliberate and load-bearing: it is what makes a
/// version id lexically distinguishable from a source id everywhere it appears, and
/// [`crate::knowledge_lifecycle::item_source_key_for`] maps it to `-` when embedding
/// it in a knowledge-item id (`weird-headings:v1` -> `ki-weird-headings-v1-000001`).
pub fn source_version_id_for(source_id: &str, version_number: u32) -> String {
    format!("{source_id}:v{version_number}")
}

/// Ordinal is 1-based and restarts at 1 for each source. Zero-padded to width 3,
/// never truncated (ordinal 1000 -> "1000").
pub fn section_id_for(source_id: &str, ordinal: u32) -> String {
    format!("section-{source_id}-{ordinal:03}")
}

/// Ordinal is a GLOBAL counter across all sources (unlike section ordinals). Zero-padded
/// to width 6, never truncated.
pub fn knowledge_item_id_for(source_id: &str, ordinal: u32) -> String {
    format!("ki-{source_id}-{ordinal:06}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_p_i1_p_i2_p_i3_cases() {
        assert_eq!(slugify("café"), "caf");
        assert_eq!(slugify("日本語"), "item");
        assert_eq!(slugify("İ"), "i");
        assert_eq!(slugify("GPT-4o mini"), "gpt-4o-mini");
        assert_eq!(slugify("Über/Modell"), "ber-modell");
        assert_eq!(slugify("v1.2.3"), "v1-2-3");
        assert_eq!(slugify("A  B__C"), "a-b-c");
        assert_eq!(slugify(""), "item");
        assert_eq!(slugify("---"), "item");
    }

    /// Ground truth: `fixtures/cases/weird-headings-md/expected-python/bundle/manifest.yaml`
    /// records `id: weird-headings` for `weird-headings.md` -- a bare stem, no hash.
    #[test]
    fn source_id_for_is_the_bare_slugified_stem() {
        assert_eq!(source_id_for("primary_care_glp1_sop"), "primary-care-glp1-sop");
        assert_eq!(source_id_for("weird-headings"), "weird-headings");
    }

    /// The whole point of dropping the hash suffix: the id survives a content edit,
    /// so the version registry has something stable to hang versions off.
    #[test]
    fn source_id_for_is_stable_across_content_changes() {
        assert_eq!(source_id_for("report"), source_id_for("report"));
    }

    #[test]
    fn source_version_id_for_matches_fixture_shape() {
        assert_eq!(source_version_id_for("weird-headings", 1), "weird-headings:v1");
        assert_eq!(source_version_id_for("glp1-intake", 12), "glp1-intake:v12");
    }

    #[test]
    fn section_and_knowledge_item_ids_zero_pad_without_truncating() {
        assert_eq!(section_id_for("src", 1), "section-src-001");
        assert_eq!(section_id_for("src", 1000), "section-src-1000");
        assert_eq!(knowledge_item_id_for("src", 7), "ki-src-000007");
        assert_eq!(knowledge_item_id_for("src", 1_000_000), "ki-src-1000000");
    }
}
