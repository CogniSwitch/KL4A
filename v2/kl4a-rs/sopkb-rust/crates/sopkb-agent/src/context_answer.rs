//! `render_context_answer` -- the no-LLM `provider: "context"` fallback
//! (`agent_chat.py` lines 73-105). docs/port/port-mapping-d-agent-mcp.md §5.3.
//!
//! P-A32: PRESERVE. This path touches no settings and opens no socket, so it always
//! succeeds if retrieval succeeded. Note the deliberately different key names vs. the
//! outer `run_agent_chat` response (`concepts` here, not `detected_concepts`, plus a
//! `retrieval_mode` key) -- preserved exactly, not "fixed" for consistency.

use serde_json::{json, Value};

fn array_len(context: &Value, key: &str) -> usize {
    context.get(key).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
}

/// Slim `{id, label, score, match_reasons}` projection of a fat detected-concept
/// object (shared shape between `render_context_answer`'s `concepts` field and
/// `run_agent_chat`'s `detected_concepts` field -- same 4 fields, different key name
/// at the call site, per P-A34).
fn slim_concept(concept: &Value) -> Value {
    json!({
        "id": concept.get("id").cloned().unwrap_or(Value::Null),
        "label": concept.get("label").cloned().unwrap_or(Value::Null),
        "score": concept.get("score").cloned().unwrap_or(Value::Null),
        "match_reasons": concept.get("match_reasons").cloned().unwrap_or_else(|| json!([])),
    })
}

fn detected_concepts_of(context: &Value) -> Vec<Value> {
    context.get("retrieval").and_then(|r| r.get("detected_concepts")).and_then(|v| v.as_array()).cloned().unwrap_or_default()
}

pub fn render_context_answer(context: &Value, scenario: &str, allow_proposed: bool) -> String {
    let concepts: Vec<Value> = detected_concepts_of(context).iter().map(slim_concept).collect();
    let retrieval_mode = context.get("retrieval").and_then(|r| r.get("mode")).cloned().unwrap_or(Value::Null);

    let summary = json!({
        "scenario": scenario,
        "allow_proposed": allow_proposed,
        "task": context.get("task").cloned().unwrap_or(Value::Null),
        "usable_knowledge_count": array_len(context, "usable_knowledge"),
        "decision_rule_count": array_len(context, "decision_rules"),
        "relation_count": array_len(context, "knowledge_relations"),
        "evidence_count": array_len(context, "evidence"),
        "concepts": concepts,
        "retrieval_mode": retrieval_mode,
    });

    [
        "## Context Ready",
        "",
        "The OKF task context was assembled for agent use. Select the Azure LLM provider to evaluate the scenario.",
        "",
        "```json",
        &sopkb_fmt::to_canonical_json(&summary),
        "```",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> Value {
        json!({
            "task": {"id": "t1", "title": "T", "description": "d", "query_terms": ["a"]},
            "usable_knowledge": [1, 2],
            "decision_rules": [1],
            "knowledge_relations": [1, 2, 3],
            "evidence": [],
            "concepts": [{"id": "concept-a"}],
            "retrieval": {
                "mode": "scenario-concepts",
                "detected_concepts": [
                    {"id": "concept-a", "label": "A", "text": "A", "okf_path": "concepts/concept-a.md", "score": 1.5, "match_reasons": ["r1"], "matched_knowledge_item_ids": ["ki-1"]},
                ],
            },
        })
    }

    #[test]
    fn structure_and_counts_match() {
        let answer = render_context_answer(&sample_context(), "scenario text", true);
        assert!(answer.starts_with("## Context Ready\n\n"));
        assert!(answer.contains("Select the Azure LLM provider to evaluate the scenario."));
        assert!(answer.trim_end().ends_with("```"));
        assert!(answer.contains("\"usable_knowledge_count\": 2"));
        assert!(answer.contains("\"decision_rule_count\": 1"));
        assert!(answer.contains("\"relation_count\": 3"));
        assert!(answer.contains("\"evidence_count\": 0"));
    }

    #[test]
    fn uses_concepts_key_not_detected_concepts_and_includes_retrieval_mode() {
        let answer = render_context_answer(&sample_context(), "s", false);
        assert!(answer.contains("\"concepts\""));
        assert!(!answer.contains("\"detected_concepts\""));
        assert!(answer.contains("\"retrieval_mode\": \"scenario-concepts\""));
    }

    #[test]
    fn slim_projection_drops_text_okf_path_and_matched_ids() {
        let answer = render_context_answer(&sample_context(), "s", false);
        assert!(!answer.contains("okf_path"));
        assert!(!answer.contains("matched_knowledge_item_ids"));
        assert!(answer.contains("\"score\": 1.5"));
        assert!(answer.contains("\"match_reasons\""));
    }

    #[test]
    fn no_detected_concepts_yields_empty_concepts_array() {
        let mut context = sample_context();
        context["retrieval"]["detected_concepts"] = json!([]);
        let answer = render_context_answer(&context, "s", false);
        assert!(answer.contains("\"concepts\": []"));
    }

    #[test]
    fn is_deterministic_for_same_input() {
        let context = sample_context();
        let a = render_context_answer(&context, "s", true);
        let b = render_context_answer(&context, "s", true);
        assert_eq!(a, b);
    }
}
