//! `decision_rules_for_item`, `normalize_authored_rule`. docs/port/port-mapping-d-agent-mcp.md §3.14-3.15.

use serde_json::json;
use sopkb_core::ids::slugify;

/// Authored rules (`item.metadata.decision_rules`, if a non-empty list) win entirely;
/// otherwise a fixed keyword scan of the lowercased object text, with a generic
/// fallback only when none of the three keyword rules matched.
pub fn decision_rules_for_item(item: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(authored) = item.get("metadata").and_then(|m| m.get("decision_rules")).and_then(|d| d.as_array()) {
        if !authored.is_empty() {
            return authored.iter().filter(|r| r.is_object()).map(|r| normalize_authored_rule(item, r)).collect();
        }
    }

    let text = item["object"].as_str().unwrap_or("").to_lowercase();
    let mut rules = Vec::new();

    if text.contains("confirm patient identity") {
        rules.push(decision_rule(
            item,
            "confirm-patient-identity",
            "Confirm patient identity",
            json!({"fact": "patient_identity_confirmed", "action": "confirm", "label": "Patient identity confirmed"}),
            None,
            None,
        ));
    }
    // second condition is a redundant superset of the first (kept for source fidelity).
    if text.contains("record contraindication") || text.contains("record contraindications") {
        rules.push(decision_rule(
            item,
            "record-contraindications",
            "Record contraindications",
            json!({"fact": "contraindications_recorded", "action": "record", "label": "Contraindications recorded"}),
            None,
            None,
        ));
    }
    if text.contains("uncertain") && text.contains("clinical review") {
        rules.push(decision_rule(
            item,
            "route-uncertain-case",
            "Route uncertain cases for clinical review",
            json!({"fact": "route_clinical_review", "action": "route", "label": "Route case for clinical review"}),
            Some(json!({"fact": "case_uncertainty", "operator": "is_true", "label": "Case is uncertain"})),
            Some(json!({
                "action_required": false,
                "label": "Clinical review route is not required by this rule when the case is not uncertain."
            })),
        ));
    }
    if rules.is_empty() {
        let predicate = item["predicate"].as_str().unwrap_or("");
        let subject = item["subject"].as_str().unwrap_or("");
        let object = item["object"].as_str().unwrap_or("");
        let fact = format!("scenario_mentions_{}", slugify(subject).replace('-', "_"));
        rules.push(decision_rule(
            item,
            &slugify(predicate),
            subject,
            json!({"fact": fact, "action": predicate, "label": object}),
            None,
            None,
        ));
    }
    rules
}

fn decision_rule(
    item: &serde_json::Value,
    suffix: &str,
    title: &str,
    obligation: serde_json::Value,
    condition: Option<serde_json::Value>,
    otherwise: Option<serde_json::Value>,
) -> serde_json::Value {
    let item_id = item["id"].as_str().unwrap_or("");
    let rule_id = format!("rule-{item_id}-{}", slugify(suffix));
    let mut rule = json!({
        "id": rule_id,
        "type": "SOP Decision Rule",
        "title": title,
        "knowledge_item_id": item["id"],
        "source_id": item["source_id"],
        "section_id": item["section_id"],
        "review_status": item["review_status"],
        "confidence": item["confidence"],
        "condition": condition.unwrap_or(serde_json::Value::Null),
        "obligation": obligation,
        "evidence_id": format!("evidence-{item_id}"),
        "relation_id": format!("kr-{item_id}"),
        "okf_path": format!("rules/{rule_id}.md"),
    });
    if let Some(o) = otherwise {
        rule.as_object_mut().unwrap().insert("otherwise".to_string(), o);
    }
    rule
}

/// Shallow-copies the authored object, filling only ABSENT keys (key-presence check,
/// not truthiness -- a key explicitly `null` is preserved). No `condition`/`obligation`
/// defaults are ever added here.
fn normalize_authored_rule(item: &serde_json::Value, rule: &serde_json::Value) -> serde_json::Value {
    let mut normalized = rule.clone();
    let item_id = item["id"].as_str().unwrap_or("");
    let predicate = item["predicate"].as_str().unwrap_or("");
    let subject = item["subject"].as_str().unwrap_or("");
    let obj = normalized.as_object_mut().expect("caller filters to objects only");

    if !obj.contains_key("id") {
        // Computed from the ORIGINAL rule.title, before the separate title default
        // below -- `or` here is truthiness-based, so a null/"" title falls back to
        // item.predicate (NOT item.subject, unlike the title default itself).
        let title_for_id = rule.get("title").and_then(|t| t.as_str()).filter(|s| !s.is_empty());
        let base = title_for_id.unwrap_or(predicate);
        obj.insert("id".to_string(), json!(format!("rule-{item_id}-{}", slugify(base))));
    }
    if !obj.contains_key("type") {
        obj.insert("type".to_string(), json!("SOP Decision Rule"));
    }
    if !obj.contains_key("title") {
        let title_val = rule.get("title").and_then(|t| t.as_str()).filter(|s| !s.is_empty());
        obj.insert("title".to_string(), json!(title_val.unwrap_or(subject)));
    }
    if !obj.contains_key("knowledge_item_id") {
        obj.insert("knowledge_item_id".to_string(), item["id"].clone());
    }
    if !obj.contains_key("source_id") {
        obj.insert("source_id".to_string(), item["source_id"].clone());
    }
    if !obj.contains_key("section_id") {
        obj.insert("section_id".to_string(), item["section_id"].clone());
    }
    if !obj.contains_key("review_status") {
        obj.insert("review_status".to_string(), item["review_status"].clone());
    }
    if !obj.contains_key("confidence") {
        obj.insert("confidence".to_string(), item["confidence"].clone());
    }
    if !obj.contains_key("evidence_id") {
        obj.insert("evidence_id".to_string(), json!(format!("evidence-{item_id}")));
    }
    if !obj.contains_key("relation_id") {
        obj.insert("relation_id".to_string(), json!(format!("kr-{item_id}")));
    }
    if !obj.contains_key("okf_path") {
        let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        obj.insert("okf_path".to_string(), json!(format!("rules/{id}.md")));
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_item() -> serde_json::Value {
        json!({
            "id": "ki-1", "subject": "Escalation", "predicate": "requires",
            "object": "Staff must confirm patient identity before proceeding.",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8
        })
    }

    #[test]
    fn keyword_rule_confirm_identity() {
        let rules = decision_rules_for_item(&base_item());
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["id"], "rule-ki-1-confirm-patient-identity");
        assert_eq!(rules[0]["condition"], serde_json::Value::Null);
        assert!(rules[0].get("otherwise").is_none());
    }

    #[test]
    fn multiple_keyword_rules_can_co_occur() {
        let mut item = base_item();
        item["object"] = json!(
            "Staff must confirm patient identity, record contraindications, and route uncertain cases for clinical review."
        );
        let rules = decision_rules_for_item(&item);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[2]["id"], "rule-ki-1-route-uncertain-case");
        assert_eq!(rules[2]["otherwise"]["action_required"], false);
    }

    #[test]
    fn generic_fallback_only_when_no_keyword_matched() {
        let mut item = base_item();
        item["object"] = json!("Nothing keyword-related here.");
        let rules = decision_rules_for_item(&item);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["title"], "Escalation"); // item.subject
        assert_eq!(rules[0]["obligation"]["fact"], "scenario_mentions_escalation");
        assert_eq!(rules[0]["obligation"]["action"], "requires"); // raw predicate, not slugified
    }

    #[test]
    fn authored_rule_id_falls_back_to_predicate_not_subject() {
        let item = base_item();
        let authored = json!([{}]); // no title at all
        let normalized = decision_rules_for_item(&json!({
            "id": "ki-1", "subject": "Escalation", "predicate": "requires", "object": "x",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8,
            "metadata": {"decision_rules": authored}
        }));
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0]["id"], "rule-ki-1-requires"); // from predicate, not subject
        assert_eq!(normalized[0]["title"], "Escalation"); // title default IS subject
        let _ = item;
    }

    #[test]
    fn authored_rule_preserves_explicit_null_and_extra_keys() {
        let authored = json!([{"title": "Custom", "condition": null, "extra_field": "kept"}]);
        let normalized = decision_rules_for_item(&json!({
            "id": "ki-1", "subject": "S", "predicate": "P", "object": "x",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8,
            "metadata": {"decision_rules": authored}
        }));
        assert_eq!(normalized[0]["id"], "rule-ki-1-custom"); // from title, not predicate
        assert_eq!(normalized[0]["condition"], serde_json::Value::Null);
        assert_eq!(normalized[0]["extra_field"], "kept");
        assert!(normalized[0].get("obligation").is_none(), "no obligation default for authored rules");
    }

    #[test]
    fn authored_rule_non_object_entries_dropped() {
        let authored = json!(["not an object", 5, {"title": "Real"}]);
        let normalized = decision_rules_for_item(&json!({
            "id": "ki-1", "subject": "S", "predicate": "P", "object": "x",
            "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.8,
            "metadata": {"decision_rules": authored}
        }));
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0]["title"], "Real");
    }
}
