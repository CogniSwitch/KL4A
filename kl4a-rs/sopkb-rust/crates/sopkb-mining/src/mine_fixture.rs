//! Port of `mine_fixture_bundle`/`obligation_sentences`/`predicate_for`/`subject_for`
//! from `tools/sopkb/sopkb/mine.py`. docs/port/port-mapping-b-mining-settings.md §3.1.
//!
//! Every quirk here is a documented PRESERVE (P-M1 through P-M8): the wider-than-text
//! "exact" spans (P-M2), naive substring obligation-term matching (P-M4), the
//! should/route/record/default predicate priority (P-M5), `rstrip(".:")`-only subject
//! cleanup (P-M6), and the hardcoded confidence/derivation/span_status/review_status
//! (P-M8). None of that is "fixed" here -- see DEVIATIONS.md for what actually changed
//! in this crate.

use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use sopkb_core::error::Result;
use sopkb_core::models::KnowledgeItem;
use sopkb_fmt::{CharIndex, OrderedMap};
use std::path::Path;

/// Exact order, 8 entries -- checked as lowercase substring containment, not
/// word-boundary matching (P-M4).
pub const OBLIGATION_TERMS: &[&str] = &["must", "shall", "should", "required", "requires", "confirm", "record", "route"];

static SENTENCE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^.!?\n]+[.!?]").unwrap());

/// `content[start:end]` in Python `str` units (Unicode code points), clamped the same
/// way Python slicing silently clamps out-of-range indices instead of raising (G1).
pub fn char_slice(content: &str, start: usize, end: usize) -> String {
    let idx = CharIndex::new(content);
    let len = idx.char_len();
    let start = start.min(len);
    let end = end.min(len).max(start);
    let start_b = idx.byte_offset_at_char(start);
    let end_b = idx.byte_offset_at_char(end);
    content[start_b..end_b].to_string()
}

/// `obligation_sentences`: scans `text` for regex matches of `[^.!?\n]+[.!?]`
/// (greedy, no DOTALL/MULTILINE/IGNORECASE), collapses each match's internal
/// whitespace, and keeps only sentences containing at least one obligation term as a
/// lowercase substring. Returned offsets are the RAW match's char offsets (P-M2: wider
/// than the stored, whitespace-collapsed sentence -- this is intentional, preserved
/// from Python exactly).
pub fn obligation_sentences(text: &str) -> Vec<(String, usize, usize)> {
    let char_index = CharIndex::new(text);
    let mut results = Vec::new();
    for m in SENTENCE_RE.find_iter(text) {
        let raw = m.as_str();
        let sentence: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        let lowered = sentence.to_lowercase();
        if OBLIGATION_TERMS.iter().any(|term| lowered.contains(term)) {
            let start_char = char_index.char_offset_at_byte(m.start());
            let end_char = char_index.char_offset_at_byte(m.end());
            results.push((sentence, start_char, end_char));
        }
    }
    results
}

/// `predicate_for`: "should" checked first (wins even if "route"/"record" also
/// present), then "route", then "record", default "requires" (covers
/// must/shall/required/confirm) -- P-M5, order is significant.
pub fn predicate_for(sentence: &str) -> &'static str {
    let lowered = sentence.to_lowercase();
    if lowered.contains("should") {
        "should"
    } else if lowered.contains("route") {
        "routes"
    } else if lowered.contains("record") {
        "records"
    } else {
        "requires"
    }
}

/// `subject_for`: strips trailing `.`/`:` repeatedly (Python `str.rstrip(".:")`).
/// Leading/internal text and trailing whitespace are untouched (P-M6).
pub fn subject_for(heading: &str) -> String {
    heading.trim_end_matches(['.', ':']).to_string()
}

/// `mine_fixture_bundle`. `provider` is always the literal string `"fixture"` when
/// reached via `mine_bundle`'s dispatch, but kept as a parameter (as in Python) since
/// it's stamped into every item's `metadata.provider`.
pub fn mine_fixture_bundle(bundle_dir: &Path, provider: &str) -> Result<Vec<Value>> {
    sopkb_core::lifecycle::migrate_source_version_state(bundle_dir)?;
    let sections_value = sopkb_core::store::read_state_json(bundle_dir, "sections.json", json!([]))?;
    let sections: Vec<Value> = sections_value.as_array().cloned().unwrap_or_default();
    let existing_items_value = sopkb_core::store::read_state_json(bundle_dir, "items.json", json!([]))?;
    let existing_items: Vec<Value> = existing_items_value.as_array().cloned().unwrap_or_default();

    let mut items: Vec<KnowledgeItem> = Vec::new();
    let mut document_contexts: Vec<Value> = Vec::new();
    let mut entities: OrderedMap<Value> = OrderedMap::new();
    let mut ordinal: u32 = 1; // GLOBAL across all sections, never resets per source (P-M8/G16).

    for section in &sections {
        let normalized_path = section.get("normalized_path").and_then(|v| v.as_str()).unwrap_or_default();
        let content = sopkb_core::store::read_text_universal_newlines(&bundle_dir.join(normalized_path))?;
        let start_pos = section.get("start_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let end_pos = section.get("end_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let section_text = char_slice(&content, start_pos, end_pos);

        let source_id = section.get("source_id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let section_id = section.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let heading = section.get("heading").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let semantic_role = section.get("semantic_role").and_then(|v| v.as_str()).unwrap_or_default().to_string();

        // Key renamed: sections.json's "id" -> document_contexts.json's "section_id".
        document_contexts.push(json!({
            "source_id": source_id,
            "section_id": section_id,
            "heading": heading,
            "semantic_role": semantic_role,
        }));

        for (sentence, rel_start, rel_end) in obligation_sentences(&section_text) {
            let absolute_start = start_pos + rel_start;
            let absolute_end = start_pos + rel_end;
            let predicate = predicate_for(&sentence).to_string();
            let subject = subject_for(&heading);

            let mut metadata = serde_json::Map::new();
            metadata.insert("provider".to_string(), json!(provider));

            // Keyed on the source VERSION, not the bare source: `weird-headings:v1`
            // becomes `ki-weird-headings-v1-000001`, so items mined from two versions
            // of one source can coexist in `items.json` without id collisions -- which
            // is precisely what supersede-rather-than-delete requires.
            let item_source_key = sopkb_core::knowledge_lifecycle::item_source_key_for(section);
            let item = KnowledgeItem {
                id: sopkb_core::ids::knowledge_item_id_for(&item_source_key, ordinal),
                item_type: "claim".to_string(),
                subject: subject.clone(),
                predicate,
                object: sentence.clone(),
                source_id: source_id.clone(),
                section_id: section_id.clone(),
                source_text: sentence, // same collapsed string as `object` (P-M8).
                start_pos: Some(absolute_start),
                end_pos: Some(absolute_end),
                span_status: "exact".to_string(),
                derivation: "direct_text".to_string(),
                confidence: 0.82,
                review_status: "proposed".to_string(),
                metadata,
                source_version_id: section
                    .get("source_version_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                lifecycle_status: "active".to_string(),
                supersedes: Vec::new(),
                superseded_by: Vec::new(),
            };

            if !entities.contains_key(&subject) {
                entities.insert(
                    subject.clone(),
                    json!({ "id": format!("entity-{}", sopkb_core::ids::slugify(&subject)), "label": subject }),
                );
            }

            items.push(item);
            ordinal += 1;
        }
    }

    let mined: Vec<Value> = items.iter().map(|it| serde_json::to_value(it).unwrap()).collect();
    // Mining is no longer a full overwrite of `items.json`: items from source versions
    // this pass did not re-mine are carried forward, and items from a version that HAS
    // been superseded are kept with `lifecycle_status: "superseded"` rather than
    // dropped. `document_contexts`/`entities`/`triples` are still regenerated wholesale
    // -- they are pure derivations of the current pass plus (for triples) the merged
    // item list, so they carry no history of their own.
    let item_dicts: Vec<Value> = sopkb_core::knowledge_lifecycle::merge_mined_items(&existing_items, &mined, &sections);

    // Writes always happen, even with zero items (produces `[]` files).
    sopkb_core::store::write_state_json(bundle_dir, "document_contexts.json", &Value::Array(document_contexts))?;
    sopkb_core::store::write_state_json(bundle_dir, "items.json", &Value::Array(item_dicts.clone()))?;
    let entities_list: Vec<Value> = entities.iter().map(|(_, v)| v.clone()).collect();
    sopkb_core::store::write_state_json(bundle_dir, "entities.json", &Value::Array(entities_list))?;

    let triples: Vec<Value> = item_dicts
        .iter()
        .map(|it| {
            json!({
                "id": format!("triple-{}", it["id"].as_str().unwrap_or_default()),
                "knowledge_item_id": it["id"],
                "subject": it["subject"],
                "predicate": it["predicate"],
                "object": it["object"],
            })
        })
        .collect();
    sopkb_core::store::write_state_json(bundle_dir, "triples.json", &Value::Array(triples))?;

    Ok(item_dicts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obligation_sentences_matches_multiple_terms_and_skips_non_matching() {
        let text = "This sentence has nothing notable. Staff must confirm identity. A plain statement here.";
        let found = obligation_sentences(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "Staff must confirm identity.");
    }

    #[test]
    fn obligation_sentences_substring_matching_is_not_word_boundary() {
        // "mustard" contains "must" as a substring -- must match per P-M4, no word
        // boundaries.
        let found = obligation_sentences("The mustard sauce was recorded on the menu.");
        assert_eq!(found.len(), 1);
        assert!(found[0].0.to_lowercase().contains("mustard"));
    }

    #[test]
    fn obligation_sentences_never_spans_a_newline() {
        let text = "Staff must\nconfirm identity.";
        let found = obligation_sentences(text);
        // The regex cannot cross the "\n", so no single match spans "must" through
        // "confirm identity." -- only the second fragment (containing "confirm")
        // matches as its own sentence.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "confirm identity.");
    }

    #[test]
    fn obligation_sentences_span_is_wider_than_stored_text_p_m2() {
        let text = "Intro text. Staff must confirm identity.";
        let found = obligation_sentences(text);
        assert_eq!(found.len(), 1);
        let (sentence, start, end) = &found[0];
        // The match starts right after the previous match's end -- i.e. at the space
        // before "Staff", not at "S" -- so end-start > len(sentence) even though the
        // span is still called "exact".
        assert!(end - start > sentence.chars().count(), "span must be wider than the stored sentence");
    }

    #[test]
    fn obligation_sentences_embedded_cr_is_absorbed_into_the_match_then_collapsed() {
        // \r is not excluded from the sentence character class -- a stray \r with no
        // accompanying \n (e.g. old-Mac-style endings, or raw text this function sees
        // before any newline-normalization upstream has run) is absorbed into the
        // match like any other non-terminator character, then collapsed away by the
        // whitespace-normalization step that builds the stored sentence.
        let text = "Staff must\rconfirm identity.";
        let found = obligation_sentences(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "Staff must confirm identity.");
    }

    #[test]
    fn obligation_sentences_crlf_terminated_line_still_matches_before_the_period() {
        let text = "Staff must confirm identity.\r\nNext line stays separate.";
        let found = obligation_sentences(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "Staff must confirm identity.");
    }

    #[test]
    fn obligation_sentences_unicode_text() {
        let text = "Le personnel doit confirmer l'identité du café patient.";
        // "confirm" is not a substring of "confirmer" in a word-boundary sense, but IS
        // a substring per P-M4's naive matching.
        let found = obligation_sentences(text);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn predicate_for_should_wins_over_route_and_record() {
        assert_eq!(predicate_for("Staff should record contraindications and route uncertain cases."), "should");
        assert_eq!(predicate_for("Route the case for review."), "routes");
        assert_eq!(predicate_for("Record the outcome."), "records");
        assert_eq!(predicate_for("Clinicians must confirm identity."), "requires");
    }

    #[test]
    fn subject_for_strips_trailing_dot_colon_repeatedly_not_whitespace() {
        assert_eq!(subject_for("Step 1.:."), "Step 1");
        assert_eq!(subject_for("Procedure"), "Procedure");
        assert_eq!(subject_for("Trailing space  "), "Trailing space  ");
    }
}
