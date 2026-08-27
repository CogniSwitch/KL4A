//! `matching_items` / `matching_items_by_text`. docs/port/port-mapping-d-agent-mcp.md §3.7-3.8.

use crate::tokens::{knowledge_haystack, meaningful_tokens};

/// Substring matching (not token matching): ANY term a substring of the (already
/// lowercase) haystack keeps the item. `terms == []` matches nothing (`any` over
/// empty is false).
pub fn matching_items(items: &[serde_json::Value], terms: &[&str]) -> Vec<serde_json::Value> {
    items
        .iter()
        .filter(|item| {
            let haystack = knowledge_haystack(item);
            terms.iter().any(|t| haystack.contains(&t.to_lowercase()))
        })
        .cloned()
        .collect()
}

/// Token-overlap ranking, sorted by (overlap count DESC, item id ASC), truncated to `limit`.
pub fn matching_items_by_text(items: &[serde_json::Value], scenario: &str, limit: usize) -> Vec<serde_json::Value> {
    let scenario_tokens = meaningful_tokens(scenario);
    let mut scored: Vec<(usize, &serde_json::Value)> = items
        .iter()
        .filter_map(|item| {
            let item_tokens = meaningful_tokens(&knowledge_haystack(item));
            let overlap = scenario_tokens.intersection(&item_tokens).count();
            if overlap > 0 {
                Some((overlap, item))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0).then_with(|| {
            let id_a = a.1["id"].as_str().unwrap_or("");
            let id_b = b.1["id"].as_str().unwrap_or("");
            id_a.cmp(id_b)
        })
    });
    scored.into_iter().take(limit).map(|(_, item)| item.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matching_items_substring_not_token() {
        // "contraindication" is a true substring of "contraindications" (simple +s
        // pluralization, unlike the doc's own "identity"/"identities" example, which
        // is actually NOT a substring relationship -- identity/identit-IES diverges
        // after "identit", so that specific doc example does not hold literally).
        let items = vec![json!({"id": "1", "subject": "Contraindications noted", "predicate": "p", "object": "o", "source_text": ""})];
        let results = matching_items(&items, &["contraindication"]);
        assert_eq!(results.len(), 1, "substring match should hit 'Contraindications'");
    }

    #[test]
    fn matching_items_empty_terms_matches_nothing() {
        let items = vec![json!({"id": "1", "subject": "A", "predicate": "p", "object": "o", "source_text": ""})];
        assert!(matching_items(&items, &[]).is_empty());
    }

    #[test]
    fn matching_items_by_text_sorts_by_overlap_then_id() {
        let items = vec![
            json!({"id": "ki-2", "subject": "identity confirmed", "predicate": "p", "object": "o", "source_text": ""}),
            json!({"id": "ki-1", "subject": "identity confirmed", "predicate": "p", "object": "o", "source_text": ""}),
            json!({"id": "ki-3", "subject": "identity", "predicate": "p", "object": "o", "source_text": ""}),
        ];
        let results = matching_items_by_text(&items, "identity confirmed", 16);
        let ids: Vec<&str> = results.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["ki-1", "ki-2", "ki-3"]);
    }
}
