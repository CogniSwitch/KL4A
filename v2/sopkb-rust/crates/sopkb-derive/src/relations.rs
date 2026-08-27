//! `knowledge_relation_for_item`, `object_label_for_item`, `relations_search`,
//! `relations_neighborhood`. docs/port/port-mapping-d-agent-mcp.md §3.13, §3.16-3.18.

use crate::concepts::concept_for_label;
use crate::reads::knowledge_items;
use sopkb_core::error::Result;
use sopkb_core::ids::slugify;
use std::path::Path;

/// <=9 words: full trimmed object, internal whitespace preserved. >9 words: first 9
/// words joined by a single space, trailing `. , ; :` stripped (possibly multiple).
pub fn object_label_for_item(item: &serde_json::Value) -> String {
    let object = item["object"].as_str().unwrap_or("").trim();
    let words: Vec<&str> = object.split_whitespace().collect();
    if words.len() <= 9 {
        object.to_string()
    } else {
        words[..9].join(" ").trim_end_matches(|c: char| ".,;:".contains(c)).to_string()
    }
}

pub fn relation_id_for_item(item: &serde_json::Value) -> String {
    format!("kr-{}", item["id"].as_str().unwrap_or(""))
}

pub fn evidence_id_for_item(item: &serde_json::Value) -> String {
    format!("evidence-{}", item["id"].as_str().unwrap_or(""))
}

/// `type` is the literal `"Knowledge Relation"` here -- OKF export files use `"SOP
/// Knowledge Relation"`, a different string; do not unify them.
pub fn knowledge_relation_for_item(item: &serde_json::Value) -> serde_json::Value {
    let subject = concept_for_label(item["subject"].as_str().unwrap_or(""));
    let predicate_text = item["predicate"].as_str().unwrap_or("");
    let object_label = object_label_for_item(item);
    let object_text = item["object"].as_str().unwrap_or("");
    let object_id_full = slugify(&object_label);
    let object_id: String = object_id_full.chars().take(64).collect();
    serde_json::json!({
        "id": relation_id_for_item(item),
        "type": "Knowledge Relation",
        "subject": subject.to_json(),
        "predicate": {"id": format!("predicate-{}", slugify(predicate_text)), "text": predicate_text},
        "object": {"id": format!("object-{object_id}"), "text": object_text, "label": object_label},
        "knowledge_piece_id": item["id"],
        "evidence_id": evidence_id_for_item(item),
        "review_status": item["review_status"],
        "confidence": item["confidence"],
        "rdf_compatible": true,
    })
}

/// Case-insensitive substring containment over subject/predicate/object.text; empty
/// filter = no constraint. No review-status filtering (rejected items' relations ARE
/// returned -- differs deliberately from `build_context`).
pub fn relations_search(bundle_dir: &Path, subject: &str, predicate: &str, object_text: &str) -> Result<Vec<serde_json::Value>> {
    let mut results = Vec::new();
    for item in knowledge_items(bundle_dir)? {
        let r = knowledge_relation_for_item(&item);
        let r_subject_text = r["subject"]["text"].as_str().unwrap_or("").to_lowercase();
        let r_predicate_text = r["predicate"]["text"].as_str().unwrap_or("").to_lowercase();
        let r_object_text = r["object"]["text"].as_str().unwrap_or("").to_lowercase();
        if !subject.is_empty() && !r_subject_text.contains(&subject.to_lowercase()) {
            continue;
        }
        if !predicate.is_empty() && !r_predicate_text.contains(&predicate.to_lowercase()) {
            continue;
        }
        if !object_text.is_empty() && !r_object_text.contains(&object_text.to_lowercase()) {
            continue;
        }
        results.push(r);
    }
    Ok(results)
}

/// Case-insensitive substring containment over exactly 6 candidate fields (id,
/// knowledge_piece_id, subject.id, subject.text, predicate.text, object.text) -- NOT
/// object.id, object.label, subject.okf_path, or evidence_id. An `evidence-<id>` node
/// id therefore matches nothing (it's not a substring of any of the 6 fields, and none
/// of them are a substring of it either).
pub fn relations_neighborhood(bundle_dir: &Path, node_id: &str) -> Result<serde_json::Value> {
    let lowered = node_id.to_lowercase();
    let mut matched = Vec::new();
    for item in knowledge_items(bundle_dir)? {
        let r = knowledge_relation_for_item(&item);
        let candidates = [
            r["id"].as_str().unwrap_or("").to_string(),
            r["knowledge_piece_id"].as_str().unwrap_or("").to_string(),
            r["subject"]["id"].as_str().unwrap_or("").to_string(),
            r["subject"]["text"].as_str().unwrap_or("").to_string(),
            r["predicate"]["text"].as_str().unwrap_or("").to_string(),
            r["object"]["text"].as_str().unwrap_or("").to_string(),
        ];
        if candidates.iter().any(|c| c.to_lowercase().contains(&lowered)) {
            matched.push(r);
        }
    }
    Ok(serde_json::json!({"node_id": node_id, "relations": matched}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_label_short_object_preserves_whitespace() {
        let item = json!({"object": "  must confirm  identity  "});
        assert_eq!(object_label_for_item(&item), "must confirm  identity");
    }

    #[test]
    fn object_label_truncates_at_9_words_and_strips_trailing_punct() {
        let item = json!({"object": "one two three four five six seven eight nine., ten eleven"});
        assert_eq!(object_label_for_item(&item), "one two three four five six seven eight nine");
    }

    #[test]
    fn object_id_truncated_to_64_chars_after_slugify() {
        let long_object = "a".repeat(100);
        let item = json!({"object": long_object, "predicate": "p", "subject": "s", "id": "ki-1", "review_status": "proposed", "confidence": 0.9});
        let r = knowledge_relation_for_item(&item);
        let object_id = r["object"]["id"].as_str().unwrap();
        assert!(object_id.len() <= 71); // "object-" (7) + 64 max
    }

    #[test]
    fn relation_type_is_knowledge_relation_not_sop_prefixed() {
        let item = json!({"object": "o", "predicate": "p", "subject": "s", "id": "ki-1", "review_status": "proposed", "confidence": 0.9});
        let r = knowledge_relation_for_item(&item);
        assert_eq!(r["type"], "Knowledge Relation");
    }
}
