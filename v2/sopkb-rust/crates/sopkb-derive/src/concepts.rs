//! `concept_for_label` / `detect_scenario_concepts`. docs/port/port-mapping-d-agent-mcp.md §3.4, §3.12.

use crate::tokens::{knowledge_haystack, meaningful_tokens};
use sopkb_core::ids::slugify;

#[derive(Debug, Clone)]
pub struct Concept {
    pub id: String,
    pub label: String,
    pub text: String,
    pub okf_path: String,
}

impl Concept {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"id": self.id, "label": self.label, "text": self.text, "okf_path": self.okf_path})
    }
}

/// `label`/`text` are both the raw, un-slugified subject string.
pub fn concept_for_label(label: &str) -> Concept {
    let concept_id = format!("concept-{}", slugify(label));
    Concept { id: concept_id.clone(), label: label.to_string(), text: label.to_string(), okf_path: format!("concepts/{concept_id}.md") }
}

#[derive(Debug, Clone)]
pub struct DetectedConcept {
    pub concept: Concept,
    pub score: f64,
    pub match_reasons: Vec<String>,
    pub matched_knowledge_item_ids: Vec<String>,
}

impl DetectedConcept {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.concept.id, "label": self.concept.label, "text": self.concept.text, "okf_path": self.concept.okf_path,
            "score": self.score, "match_reasons": self.match_reasons,
            "matched_knowledge_item_ids": self.matched_knowledge_item_ids,
        })
    }
}

fn add_concept_score(entry: &mut DetectedConcept, score: f64, reason: String, item_id: &str) {
    entry.score += score; // ALWAYS accumulates, even for a duplicate reason/item
    if !entry.match_reasons.contains(&reason) {
        entry.match_reasons.push(reason);
    }
    if !entry.matched_knowledge_item_ids.contains(&item_id.to_string()) {
        entry.matched_knowledge_item_ids.push(item_id.to_string());
    }
}

/// Full scoring algorithm: rule 1 (exact phrase containment, +8), rule 2 (label token
/// overlap, +3/token uncapped), rule 3 (item-text token overlap, +1/token capped at 6).
/// Score accumulates across every item sharing the same subject even though reason
/// strings de-duplicate. Sorted by (score DESC, raw subject label ASC).
pub fn detect_scenario_concepts(items: &[serde_json::Value], scenario: &str, limit: usize) -> Vec<DetectedConcept> {
    let scenario_text = scenario.to_lowercase();
    let scenario_tokens = meaningful_tokens(scenario);
    let mut order: Vec<String> = Vec::new();
    let mut concepts: std::collections::HashMap<String, DetectedConcept> = std::collections::HashMap::new();

    for item in items {
        let subject = item["subject"].as_str().unwrap_or("");
        let concept = concept_for_label(subject);
        let concept_id = concept.id.clone();
        concepts.entry(concept_id.clone()).or_insert_with(|| {
            order.push(concept_id.clone());
            DetectedConcept { concept: concept.clone(), score: 0.0, match_reasons: Vec::new(), matched_knowledge_item_ids: Vec::new() }
        });

        let item_id = item["id"].as_str().unwrap_or("").to_string();
        let label = concept.label.clone();
        let label_tokens = meaningful_tokens(&label);
        let item_tokens = meaningful_tokens(&knowledge_haystack(item));
        let label_overlap: std::collections::HashSet<&String> = scenario_tokens.intersection(&label_tokens).collect();
        let item_overlap: std::collections::HashSet<&String> = scenario_tokens.intersection(&item_tokens).collect();

        let entry = concepts.get_mut(&concept_id).unwrap();

        if scenario_text.contains(&label.to_lowercase()) {
            add_concept_score(entry, 8.0, "label phrase".to_string(), &item_id);
        }
        if !label_overlap.is_empty() {
            let mut sorted: Vec<&String> = label_overlap.iter().copied().collect();
            sorted.sort();
            let reason = format!("label terms: {}", sorted.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            add_concept_score(entry, 3.0 * label_overlap.len() as f64, reason, &item_id);
        }
        if !item_overlap.is_empty() {
            let mut sorted: Vec<&String> = item_overlap.iter().copied().collect();
            sorted.sort();
            let capped: Vec<&str> = sorted.iter().take(6).map(|s| s.as_str()).collect();
            let reason = format!("linked knowledge terms: {}", capped.join(", "));
            add_concept_score(entry, (item_overlap.len() as f64).min(6.0), reason, &item_id);
        }
    }

    let mut ranked: Vec<DetectedConcept> = order
        .into_iter()
        .map(|id| concepts.remove(&id).unwrap())
        .filter(|e| e.score > 0.0 && !e.matched_knowledge_item_ids.is_empty())
        .collect();
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap().then_with(|| a.concept.label.cmp(&b.concept.label)));
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn concept_for_label_uses_raw_label_as_text() {
        let c = concept_for_label("Café Checklist");
        assert_eq!(c.id, "concept-caf-checklist");
        assert_eq!(c.label, "Café Checklist");
        assert_eq!(c.text, "Café Checklist");
        assert_eq!(c.okf_path, "concepts/concept-caf-checklist.md");
    }

    #[test]
    fn score_accumulates_across_items_sharing_a_subject() {
        // The haystack always includes the subject itself, so rules 2/3 unavoidably
        // co-fire alongside rule 1 here -- rather than hand-computing that combined
        // total, prove the *accumulation* property directly: two items sharing a
        // subject must score exactly double one item alone.
        let one = vec![json!({"id": "ki-1", "subject": "Eligibility Requirements", "predicate": "p", "object": "must confirm eligibility", "source_text": ""})];
        let two = vec![
            json!({"id": "ki-1", "subject": "Eligibility Requirements", "predicate": "p", "object": "must confirm eligibility", "source_text": ""}),
            json!({"id": "ki-2", "subject": "Eligibility Requirements", "predicate": "p", "object": "must confirm eligibility", "source_text": ""}),
        ];
        let one_detected = detect_scenario_concepts(&one, "eligibility requirements", 8);
        let two_detected = detect_scenario_concepts(&two, "eligibility requirements", 8);
        assert_eq!(two_detected.len(), 1);
        assert_eq!(two_detected[0].score, one_detected[0].score * 2.0, "score must accumulate additively across items sharing a subject");
        assert_eq!(two_detected[0].match_reasons.len(), one_detected[0].match_reasons.len(), "reason strings de-duplicate even though score doesn't");
        assert_eq!(two_detected[0].matched_knowledge_item_ids, vec!["ki-1", "ki-2"]);
    }

    #[test]
    fn sorted_by_score_desc_then_label_asc() {
        let items = vec![
            json!({"id": "ki-1", "subject": "Bravo", "predicate": "p", "object": "bravo term", "source_text": ""}),
            json!({"id": "ki-2", "subject": "Alpha", "predicate": "p", "object": "alpha term", "source_text": ""}),
        ];
        let detected = detect_scenario_concepts(&items, "alpha bravo", 8);
        // both score equally (label-phrase match, +8 each) -> tie-break by label ASC
        assert_eq!(detected.len(), 2);
        assert_eq!(detected[0].concept.label, "Alpha");
        assert_eq!(detected[1].concept.label, "Bravo");
    }

    #[test]
    fn item_overlap_reason_capped_at_six_terms_but_score_uses_full_count() {
        // "Quokka" deliberately shares no characters with any of the 8 scenario words
        // below (unlike e.g. "X", which collides with the 'x' inside "foxtrot" and
        // would spuriously trigger rule 1's phrase match) -- isolating rule 3 alone.
        let items = vec![json!({
            "id": "ki-1", "subject": "Quokka", "predicate": "p",
            "object": "alpha bravo charlie delta echo foxtrot golf hotel",
            "source_text": ""
        })];
        let detected = detect_scenario_concepts(&items, "alpha bravo charlie delta echo foxtrot golf hotel", 8);
        assert_eq!(detected.len(), 1);
        // item_overlap count = 8 tokens -> score capped at 6.0 for rule 3
        assert_eq!(detected[0].score, 6.0);
        let reason = detected[0].match_reasons.iter().find(|r| r.starts_with("linked knowledge terms")).unwrap();
        assert_eq!(reason.matches(',').count(), 5, "reason text should list only 6 terms (5 commas)");
    }
}
