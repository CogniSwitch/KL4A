//! `agent_guide`, `agent_tasks`, `build_context`, `agent_context`, `scenario_agent_context`.
//! docs/port/port-mapping-d-agent-mcp.md §3.2-3.3, §3.9-3.11.

use crate::concepts::{concept_for_label, detect_scenario_concepts};
use crate::constants::{task_definition, AUTO_TASK, TASK_DEFINITIONS};
use crate::matching::{matching_items, matching_items_by_text};
use crate::reads::{conflicts_list, evidence_get, freshness_check, knowledge_items, sections_list, sources_list};
use crate::relations::knowledge_relation_for_item;
use crate::rules::decision_rules_for_item;
use serde_json::json;
use sopkb_core::error::Result;
use std::path::Path;

const AGENT_RULES: [&str; 6] = [
    "Use only usable_knowledge items as task rules.",
    "Treat rejected knowledge as excluded unless include_rejected is true.",
    "Resolve evidence before making a claim to a downstream user or system.",
    "Use Knowledge Relations for graph traversal and RDF-compatible assertions.",
    "Use decision_rules for conditional task handling; do not infer conditions from prose when structured rules exist.",
    "Check freshness and conflict reports before finalizing a decision.",
];

pub fn agent_guide(bundle_dir: &Path) -> String {
    let primary = bundle_dir.join("references").join("agent-guide.md");
    let fallback_path = bundle_dir.join("exports").join("okf").join("references").join("agent-guide.md");
    let path = if primary.exists() { Some(primary) } else if fallback_path.exists() { Some(fallback_path) } else { None };
    match path {
        Some(p) => sopkb_core::store::read_text_universal_newlines(&p).unwrap_or_else(|_| default_agent_guide()),
        None => default_agent_guide(),
    }
}

fn default_agent_guide() -> String {
    [
        "# SOP KB Agent Guide",
        "",
        "Use `agent tasks` to choose a task context, then use `agent context` to retrieve usable knowledge, evidence, and Knowledge Relations.",
        "Rejected knowledge is excluded by default. Always check freshness and conflict reports before using a rule operationally.",
        "",
    ]
    .join("\n")
}

pub fn agent_tasks(bundle_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let items = knowledge_items(bundle_dir)?;
    Ok(TASK_DEFINITIONS
        .iter()
        .map(|t| {
            let count = matching_items(&items, t.query_terms).len();
            json!({"id": t.id, "title": t.title, "description": t.description, "matching_knowledge_count": count})
        })
        .collect())
}

/// `task` is `(id, title, description, query_terms)`.
pub fn build_context(
    bundle_dir: &Path,
    task_id: &str,
    task_title: &str,
    task_description: &str,
    task_query_terms: &[&str],
    matched_items: &[serde_json::Value],
    include_rejected: bool,
) -> Result<serde_json::Value> {
    let usable_items: Vec<&serde_json::Value> = matched_items
        .iter()
        .filter(|item| include_rejected || item["review_status"].as_str() != Some("rejected"))
        .collect();

    let sources = sources_list(bundle_dir)?;
    let sections = sections_list(bundle_dir)?;
    let source_by_id = |id: &str| sources.iter().rev().find(|s| s["id"] == id).cloned();
    let section_by_id = |id: &str| sections.iter().rev().find(|s| s["id"] == id).cloned();

    let mut knowledge_payload = Vec::new();
    let mut evidence_payload = Vec::new();
    let mut relation_payload = Vec::new();
    let mut decision_rule_payload = Vec::new();
    let mut concept_order: Vec<String> = Vec::new();
    let mut concept_payload: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();

    for item in &usable_items {
        let item_id = item["id"].as_str().unwrap_or("");
        let source_id = item["source_id"].as_str().unwrap_or("");
        let section_id = item["section_id"].as_str().unwrap_or("");
        let source = source_by_id(source_id);
        let section = section_by_id(section_id);
        let relation = knowledge_relation_for_item(item);
        let rules = decision_rules_for_item(item);
        let evidence = evidence_get(bundle_dir, item_id)?;
        let subject_concept = concept_for_label(item["subject"].as_str().unwrap_or(""));

        if !concept_payload.contains_key(&subject_concept.id) {
            concept_order.push(subject_concept.id.clone());
        }
        concept_payload.insert(
            subject_concept.id.clone(),
            json!({
                "id": subject_concept.id, "label": subject_concept.label, "text": subject_concept.text, "okf_path": subject_concept.okf_path,
                "links": {"knowledge_piece_id": item_id, "relation_id": relation["id"], "section_id": item["section_id"]},
            }),
        );

        let rule_ids: Vec<serde_json::Value> = rules.iter().map(|r| r["id"].clone()).collect();
        knowledge_payload.push(json!({
            "id": item["id"], "subject": item["subject"], "predicate": item["predicate"], "object": item["object"],
            "review_status": item["review_status"], "confidence": item["confidence"],
            "source_id": item["source_id"], "source_title": source.as_ref().and_then(|s| s.get("title").cloned()).unwrap_or(serde_json::Value::Null),
            "section_id": item["section_id"], "section_heading": section.as_ref().and_then(|s| s.get("heading").cloned()).unwrap_or(serde_json::Value::Null),
            "evidence_id": format!("evidence-{item_id}"), "relation_id": relation["id"],
            "rule_ids": rule_ids, "concept_ids": [subject_concept.id.clone()],
            "okf_path": format!("knowledge/{item_id}.md"),
        }));
        evidence_payload.push(evidence);
        relation_payload.push(relation);
        decision_rule_payload.extend(rules);
    }

    let mut concepts: Vec<serde_json::Value> = concept_order.into_iter().map(|id| concept_payload.remove(&id).unwrap()).collect();
    concepts.sort_by(|a, b| a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or("")));

    Ok(json!({
        "task": {"id": task_id, "title": task_title, "description": task_description, "query_terms": task_query_terms},
        "agent_rules": AGENT_RULES,
        "usable_knowledge": knowledge_payload,
        "knowledge_relations": relation_payload,
        "decision_rules": decision_rule_payload,
        "concepts": concepts,
        "evidence": evidence_payload,
        "reports": {"freshness": freshness_check(bundle_dir), "conflicts": conflicts_list(bundle_dir)},
        "excluded_knowledge_count": matched_items.len() - usable_items.len(),
    }))
}

pub fn agent_context(bundle_dir: &Path, task_id: &str, include_rejected: bool) -> Result<serde_json::Value> {
    let task = task_definition(task_id).map_err(sopkb_core::error::SopkbError::Value)?;
    let all_items = knowledge_items(bundle_dir)?;
    let mut matched = matching_items(&all_items, task.query_terms);
    if matched.is_empty() {
        matched = all_items;
    }
    build_context(bundle_dir, task.id, task.title, task.description, task.query_terms, &matched, include_rejected)
}

pub fn scenario_agent_context(
    bundle_dir: &Path,
    scenario: &str,
    task_id: Option<&str>,
    include_rejected: bool,
    item_limit: usize,
) -> Result<serde_json::Value> {
    let all_items = knowledge_items(bundle_dir)?;
    let detected = detect_scenario_concepts(&all_items, scenario, 8);

    let mut matched_ids: Vec<String> = Vec::new();
    for concept in &detected {
        for item_id in &concept.matched_knowledge_item_ids {
            if !matched_ids.contains(item_id) {
                matched_ids.push(item_id.clone());
            }
        }
    }

    let is_real_task_id = task_id.is_some_and(|t| !t.is_empty() && t != "auto" && t != "scenario-analysis");
    let task = if is_real_task_id {
        let t = task_definition(task_id.unwrap()).map_err(sopkb_core::error::SopkbError::Value)?;
        let task_ids: std::collections::HashSet<String> =
            matching_items(&all_items, t.query_terms).iter().map(|i| i["id"].as_str().unwrap_or("").to_string()).collect();
        if !task_ids.is_empty() {
            let intersected: Vec<String> = matched_ids.iter().filter(|id| task_ids.contains(*id)).cloned().collect();
            if !intersected.is_empty() {
                matched_ids = intersected;
            } else {
                matched_ids = all_items.iter().map(|i| i["id"].as_str().unwrap_or("").to_string()).filter(|id| task_ids.contains(id)).collect();
            }
        }
        t
    } else {
        &AUTO_TASK
    };

    if matched_ids.is_empty() {
        matched_ids = matching_items_by_text(&all_items, scenario, 16).iter().map(|i| i["id"].as_str().unwrap_or("").to_string()).collect();
    }
    if matched_ids.is_empty() {
        matched_ids = all_items.iter().map(|i| i["id"].as_str().unwrap_or("").to_string()).collect();
    }

    let keep: std::collections::HashSet<&String> = matched_ids.iter().take(item_limit).collect();
    let matched_items: Vec<serde_json::Value> = all_items.into_iter().filter(|i| keep.contains(&i["id"].as_str().unwrap_or("").to_string())).collect();

    let mut context = build_context(bundle_dir, task.id, task.title, task.description, task.query_terms, &matched_items, include_rejected)?;

    let task_override = match task_id {
        Some(t) if !t.is_empty() && t != "auto" && t != "scenario-analysis" => json!(t),
        _ => serde_json::Value::Null,
    };
    context.as_object_mut().unwrap().insert(
        "retrieval".to_string(),
        json!({
            "mode": "scenario-concepts",
            "scenario": scenario,
            "detected_concepts": detected.iter().map(|d| d.to_json()).collect::<Vec<_>>(),
            "item_limit": item_limit,
            "task_override": task_override,
        }),
    );
    Ok(context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sopkb_core::store;
    use tempfile::tempdir;

    #[test]
    fn agent_guide_falls_back_to_default() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let guide = agent_guide(&bundle_dir);
        assert!(guide.starts_with("# SOP KB Agent Guide\n"));
        assert!(guide.ends_with("operationally.\n"));
        assert!(!guide.ends_with("operationally.\n\n"), "must end with exactly one trailing newline");
    }

    #[test]
    fn agent_tasks_returns_exactly_three_never_auto() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let tasks = agent_tasks(&bundle_dir).unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().all(|t| t["id"] != "scenario-analysis"));
    }

    #[test]
    fn build_context_excludes_rejected_by_default() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let items = vec![
            json!({"id": "ki-1", "subject": "A", "predicate": "requires", "object": "confirm patient identity", "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9, "source_text": "x", "span_status": "exact", "start_pos": 0, "end_pos": 1}),
            json!({"id": "ki-2", "subject": "B", "predicate": "requires", "object": "confirm patient identity", "source_id": "s", "section_id": "sec", "review_status": "rejected", "confidence": 0.9, "source_text": "x", "span_status": "exact", "start_pos": 0, "end_pos": 1}),
        ];
        store::write_state_json(&bundle_dir, "items.json", &json!(items)).unwrap();
        let context = build_context(&bundle_dir, "scenario-analysis", "Scenario Analysis", "d", &[], &items, false).unwrap();
        assert_eq!(context["usable_knowledge"].as_array().unwrap().len(), 1);
        assert_eq!(context["excluded_knowledge_count"], 1);
    }

    #[test]
    fn concept_ids_always_has_exactly_one_element() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let items = vec![json!({"id": "ki-1", "subject": "A", "predicate": "requires", "object": "x", "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9, "source_text": "x", "span_status": "exact", "start_pos": 0, "end_pos": 1})];
        store::write_state_json(&bundle_dir, "items.json", &json!(items)).unwrap();
        let context = build_context(&bundle_dir, "t", "T", "d", &[], &items, false).unwrap();
        assert_eq!(context["usable_knowledge"][0]["concept_ids"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn index_parallel_invariant_across_knowledge_relations_evidence() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let items = vec![
            json!({"id": "ki-1", "subject": "A", "predicate": "requires", "object": "x", "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9, "source_text": "x", "span_status": "exact", "start_pos": 0, "end_pos": 1}),
            json!({"id": "ki-2", "subject": "B", "predicate": "requires", "object": "y", "source_id": "s", "section_id": "sec", "review_status": "proposed", "confidence": 0.9, "source_text": "y", "span_status": "exact", "start_pos": 0, "end_pos": 1}),
        ];
        store::write_state_json(&bundle_dir, "items.json", &json!(items)).unwrap();
        let context = build_context(&bundle_dir, "t", "T", "d", &[], &items, false).unwrap();
        let uk = context["usable_knowledge"].as_array().unwrap();
        let kr = context["knowledge_relations"].as_array().unwrap();
        let ev = context["evidence"].as_array().unwrap();
        assert_eq!(uk.len(), kr.len());
        assert_eq!(uk.len(), ev.len());
        for i in 0..uk.len() {
            assert_eq!(uk[i]["id"], kr[i]["knowledge_piece_id"]);
            assert_eq!(uk[i]["id"], ev[i]["knowledge_item_id"]);
        }
    }
}
