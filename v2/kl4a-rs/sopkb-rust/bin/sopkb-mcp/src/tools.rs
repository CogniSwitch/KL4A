//! Tool descriptors (§6.1) and dispatch (§6.2, §6.3) for the 16 read-only MCP tools
//! plus the gated `review.note` mutation. Mirrors `tools/sopkb/sopkb/mcp_server.py`'s
//! `object_schema`/`READ_ONLY_TOOLS`/`REVIEW_NOTE_TOOL`/`list_mcp_tools`/
//! `call_mcp_tool`/`require_arg`. See docs/port/port-mapping-d-agent-mcp.md §6.1-§6.3.

use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use std::path::Path;

fn object_schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let mut props = serde_json::Map::new();
    for (name, type_name) in properties {
        props.insert((*name).to_string(), json!({ "type": type_name }));
    }
    json!({
        "type": "object",
        "properties": Value::Object(props),
        "required": required,
        "additionalProperties": false,
    })
}

fn tool(name: &str, description: &str, properties: &[(&str, &str)], required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": object_schema(properties, required),
    })
}

/// The 16 read-only tool descriptors. Exact list order is part of the wire contract
/// (P-P13) -- `tools/list` preserves it verbatim.
pub fn read_only_tools() -> Vec<Value> {
    vec![
        tool("bundle.describe", "Describe the SOP Knowledge Bundle.", &[], &[]),
        tool("sources.list", "List bundle sources.", &[], &[]),
        tool("sources.get", "Get one source by id.", &[("source_id", "string")], &["source_id"]),
        tool("sections.list", "List normalized sections.", &[], &[]),
        tool("sections.get", "Get one section by id.", &[("section_id", "string")], &["section_id"]),
        tool(
            "knowledge.search",
            "Search knowledge items. Each result includes its evidence (source_text) inline \
             -- quote it verbatim in your answer alongside any summary, don't just paraphrase \
             it. A non-empty rule_ids means the item is governed by a structured decision rule \
             (condition/obligation/otherwise) -- call knowledge.get on the item to retrieve and \
             apply it; don't rely on the prose alone when a rule exists.",
            &[("query", "string")],
            &["query"],
        ),
        tool(
            "knowledge.get",
            "Get one knowledge item by id.",
            &[("knowledge_item_id", "string")],
            &["knowledge_item_id"],
        ),
        tool(
            "evidence.get",
            "Get the verbatim source_text evidence for a knowledge item. Quote this text \
             directly in your answer, don't just paraphrase it.",
            &[("knowledge_item_id", "string")],
            &["knowledge_item_id"],
        ),
        tool("conflicts.list", "Read conflict report.", &[], &[]),
        tool("freshness.check", "Read freshness report.", &[], &[]),
        tool("citations.resolve", "Resolve a citation id.", &[("citation_id", "string")], &["citation_id"]),
        tool("agent.guide", "Read the OKF bundle agent guide.", &[], &[]),
        tool("agent.tasks", "List task contexts agents can request.", &[], &[]),
        tool(
            "agent.context",
            "Get task-scoped knowledge, evidence, concepts, and Knowledge Relations.",
            &[("task", "string")],
            &["task"],
        ),
        tool(
            "relations.search",
            "Search RDF-compatible Knowledge Relations.",
            &[("subject", "string"), ("predicate", "string"), ("object", "string")],
            &[],
        ),
        tool(
            "relations.neighborhood",
            "Get relations around a concept, relation, or knowledge id.",
            &[("node_id", "string")],
            &["node_id"],
        ),
    ]
}

/// Appended at index 16 only when `enable_review_notes` is true (P-P7).
pub fn review_note_tool() -> Value {
    tool(
        "review.note",
        "Write a reviewer note. Disabled unless explicitly enabled.",
        &[("knowledge_item_id", "string"), ("reviewer", "string"), ("rationale", "string")],
        &["knowledge_item_id", "rationale"],
    )
}

pub fn list_mcp_tools(enable_review_notes: bool) -> Vec<Value> {
    let mut tools = read_only_tools();
    if enable_review_notes {
        tools.push(review_note_tool());
    }
    tools
}

/// `require_arg` (§6.2): the RAW string value is returned UNTRIMMED after validating
/// the *trimmed* value is non-empty (P-P9, decided PRESERVE -- trimming would be
/// friendlier but changes what e.g. `knowledge.search` receives).
pub fn require_arg(arguments: &Value, name: &str) -> Result<String> {
    match arguments.get(name) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.clone()),
        _ => Err(SopkbError::Value(format!("missing required argument: {name}"))),
    }
}

/// An optional string argument with a default for "falsy" (absent/null) values.
///
/// P-P10, decided FIX: Python's `args.get(name) or default` would silently use a
/// non-string *truthy* JSON value (a number, bool, non-empty array/object) as-is, then
/// crash downstream the moment it's `.lower()`-ed (this only actually happens for
/// `relations.search`'s subject/predicate/object). This port rejects any present,
/// non-null, non-string value outright with a clear typed error instead of either
/// silently misusing it or crashing -- applied uniformly to every optional string
/// argument (`relations.search`'s three filters AND `review.note`'s `reviewer`) for
/// internal consistency, since every one of these arguments is declared `"string"` in
/// its own `inputSchema` already. See DEVIATIONS.md.
fn optional_string_arg(arguments: &Value, name: &str, default: &str) -> Result<String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(default.to_string()),
        Some(Value::String(s)) if s.is_empty() => Ok(default.to_string()),
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(SopkbError::Value(format!("argument '{name}' must be a string"))),
    }
}

/// `call_mcp_tool` (§6.3): the linear dispatch chain on `name`.
pub fn call_mcp_tool(bundle_dir: &Path, name: &str, arguments: &Value, enable_review_notes: bool) -> Result<Value> {
    match name {
        "bundle.describe" => sopkb_derive::reads::bundle_describe(bundle_dir),
        "sources.list" => sopkb_derive::reads::sources_list(bundle_dir).map(Value::Array),
        "sources.get" => sopkb_derive::reads::sources_get(bundle_dir, &require_arg(arguments, "source_id")?),
        "sections.list" => sopkb_derive::reads::sections_list(bundle_dir).map(Value::Array),
        "sections.get" => sopkb_derive::reads::sections_get(bundle_dir, &require_arg(arguments, "section_id")?),
        "knowledge.search" => {
            sopkb_derive::reads::knowledge_search(bundle_dir, &require_arg(arguments, "query")?).map(Value::Array)
        }
        "knowledge.get" => sopkb_derive::reads::knowledge_get(bundle_dir, &require_arg(arguments, "knowledge_item_id")?),
        "evidence.get" => sopkb_derive::reads::evidence_get(bundle_dir, &require_arg(arguments, "knowledge_item_id")?),
        "conflicts.list" => Ok(json!({ "markdown": sopkb_derive::reads::conflicts_list(bundle_dir) })),
        "freshness.check" => Ok(json!({ "markdown": sopkb_derive::reads::freshness_check(bundle_dir) })),
        "citations.resolve" => sopkb_derive::reads::citations_resolve(bundle_dir, &require_arg(arguments, "citation_id")?),
        "agent.guide" => Ok(json!({ "markdown": sopkb_derive::context::agent_guide(bundle_dir) })),
        "agent.tasks" => sopkb_derive::context::agent_tasks(bundle_dir).map(Value::Array),
        // include_rejected is NOT exposed via MCP -- always false (P-P8).
        "agent.context" => sopkb_derive::context::agent_context(bundle_dir, &require_arg(arguments, "task")?, false),
        "relations.search" => {
            // NOTE wire arg name is "object", not "object_text" (P-P11, PRESERVE).
            let subject = optional_string_arg(arguments, "subject", "")?;
            let predicate = optional_string_arg(arguments, "predicate", "")?;
            let object_text = optional_string_arg(arguments, "object", "")?;
            sopkb_derive::relations::relations_search(bundle_dir, &subject, &predicate, &object_text).map(Value::Array)
        }
        "relations.neighborhood" => sopkb_derive::relations::relations_neighborhood(bundle_dir, &require_arg(arguments, "node_id")?),
        "review.note" => {
            if !enable_review_notes {
                // P-P7, decided PRESERVE both messages: this specific message, not
                // "unknown tool" -- review.note is discoverable-by-calling even when
                // hidden from tools/list.
                return Err(SopkbError::Value(
                    "review.note is disabled; start with --enable-review-notes".to_string(),
                ));
            }
            let knowledge_item_id = require_arg(arguments, "knowledge_item_id")?;
            let rationale = require_arg(arguments, "rationale")?;
            let reviewer = optional_string_arg(arguments, "reviewer", "mcp:user")?;
            sopkb_review::comment_item(bundle_dir, &knowledge_item_id, &reviewer, &rationale)
        }
        _ => Err(SopkbError::Value(format!("unknown MCP tool: {name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sopkb_core::store;
    use tempfile::tempdir;

    #[test]
    fn golden_16_descriptor_list_matches_table_exactly() {
        let tools = list_mcp_tools(false);
        assert_eq!(tools.len(), 16, "read-only listing must have exactly 16 tools");

        let expected: Vec<(&str, &str)> = vec![
            ("bundle.describe", "Describe the SOP Knowledge Bundle."),
            ("sources.list", "List bundle sources."),
            ("sources.get", "Get one source by id."),
            ("sections.list", "List normalized sections."),
            ("sections.get", "Get one section by id."),
            (
                "knowledge.search",
                "Search knowledge items. Each result includes its evidence (source_text) inline \
                 -- quote it verbatim in your answer alongside any summary, don't just paraphrase \
                 it. A non-empty rule_ids means the item is governed by a structured decision rule \
                 (condition/obligation/otherwise) -- call knowledge.get on the item to retrieve and \
                 apply it; don't rely on the prose alone when a rule exists.",
            ),
            ("knowledge.get", "Get one knowledge item by id."),
            (
                "evidence.get",
                "Get the verbatim source_text evidence for a knowledge item. Quote this text \
                 directly in your answer, don't just paraphrase it.",
            ),
            ("conflicts.list", "Read conflict report."),
            ("freshness.check", "Read freshness report."),
            ("citations.resolve", "Resolve a citation id."),
            ("agent.guide", "Read the OKF bundle agent guide."),
            ("agent.tasks", "List task contexts agents can request."),
            (
                "agent.context",
                "Get task-scoped knowledge, evidence, concepts, and Knowledge Relations.",
            ),
            ("relations.search", "Search RDF-compatible Knowledge Relations."),
            (
                "relations.neighborhood",
                "Get relations around a concept, relation, or knowledge id.",
            ),
        ];
        for (i, (name, description)) in expected.iter().enumerate() {
            assert_eq!(tools[i]["name"], *name, "tool at index {i}");
            assert_eq!(tools[i]["description"], *description, "description at index {i}");
        }

        // No-argument tools declare the exact empty-schema shape.
        assert_eq!(
            tools[0]["inputSchema"],
            json!({"type": "object", "properties": {}, "required": [], "additionalProperties": false})
        );
        // Single required-arg tool.
        assert_eq!(
            tools[2]["inputSchema"],
            json!({"type": "object", "properties": {"source_id": {"type": "string"}}, "required": ["source_id"], "additionalProperties": false})
        );
        // relations.search: 3 optional args, none required.
        assert_eq!(
            tools[14]["inputSchema"],
            json!({
                "type": "object",
                "properties": {"subject": {"type": "string"}, "predicate": {"type": "string"}, "object": {"type": "string"}},
                "required": [],
                "additionalProperties": false,
            })
        );
    }

    #[test]
    fn review_note_appended_at_index_16_only_when_enabled() {
        let disabled = list_mcp_tools(false);
        assert_eq!(disabled.len(), 16);
        assert!(disabled.iter().all(|t| t["name"] != "review.note"));

        let enabled = list_mcp_tools(true);
        assert_eq!(enabled.len(), 17);
        assert_eq!(enabled[16]["name"], "review.note");
        assert_eq!(enabled[16]["description"], "Write a reviewer note. Disabled unless explicitly enabled.");
        assert_eq!(
            enabled[16]["inputSchema"],
            json!({
                "type": "object",
                "properties": {
                    "knowledge_item_id": {"type": "string"},
                    "reviewer": {"type": "string"},
                    "rationale": {"type": "string"},
                },
                "required": ["knowledge_item_id", "rationale"],
                "additionalProperties": false,
            })
        );
    }

    fn bundle_with_one_item() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("bundle");
        store::create_bundle(&bundle_dir, Some("Test Bundle")).unwrap();
        let inventory = json!({"sources": [{"id": "src-1", "title": "Source One"}]});
        store::write_state_json(&bundle_dir, "inventory.json", &inventory).unwrap();
        let sections = json!([{"id": "sec-1", "source_id": "src-1", "heading": "Intro"}]);
        store::write_state_json(&bundle_dir, "sections.json", &sections).unwrap();
        let items = json!([{
            "id": "ki-a-000001", "item_type": "claim", "subject": "Staff", "predicate": "must",
            "object": "confirm patient identity before dosing", "source_id": "src-1", "section_id": "sec-1",
            "source_text": "Staff must confirm patient identity.", "start_pos": 0, "end_pos": 10,
            "span_status": "exact", "derivation": "direct_text", "confidence": 0.82,
            "review_status": "proposed", "metadata": {},
        }]);
        store::write_state_json(&bundle_dir, "items.json", &items).unwrap();
        dir
    }

    #[test]
    fn dispatch_bundle_describe() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let result = call_mcp_tool(&bundle_dir, "bundle.describe", &json!({}), false).unwrap();
        assert_eq!(result["title"], "Test Bundle");
        assert_eq!(result["source_count"], 1);
        assert_eq!(result["knowledge_item_count"], 1);
    }

    #[test]
    fn dispatch_sources_list_and_get() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let list = call_mcp_tool(&bundle_dir, "sources.list", &json!({}), false).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);

        let one = call_mcp_tool(&bundle_dir, "sources.get", &json!({"source_id": "src-1"}), false).unwrap();
        assert_eq!(one["id"], "src-1");

        let err = call_mcp_tool(&bundle_dir, "sources.get", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: source_id");

        let err_blank = call_mcp_tool(&bundle_dir, "sources.get", &json!({"source_id": "   "}), false).unwrap_err();
        assert_eq!(err_blank.to_string(), "missing required argument: source_id");
    }

    #[test]
    fn dispatch_sections_list_and_get() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let list = call_mcp_tool(&bundle_dir, "sections.list", &json!({}), false).unwrap();
        assert_eq!(list.as_array().unwrap().len(), 1);
        let one = call_mcp_tool(&bundle_dir, "sections.get", &json!({"section_id": "sec-1"}), false).unwrap();
        assert_eq!(one["id"], "sec-1");
        let err = call_mcp_tool(&bundle_dir, "sections.get", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: section_id");
    }

    #[test]
    fn dispatch_knowledge_search_and_get() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let results = call_mcp_tool(&bundle_dir, "knowledge.search", &json!({"query": "staff"}), false).unwrap();
        assert_eq!(results.as_array().unwrap().len(), 1);
        let err = call_mcp_tool(&bundle_dir, "knowledge.search", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: query");

        let one = call_mcp_tool(&bundle_dir, "knowledge.get", &json!({"knowledge_item_id": "ki-a-000001"}), false).unwrap();
        assert_eq!(one["id"], "ki-a-000001");
        let err2 = call_mcp_tool(&bundle_dir, "knowledge.get", &json!({}), false).unwrap_err();
        assert_eq!(err2.to_string(), "missing required argument: knowledge_item_id");
    }

    #[test]
    fn dispatch_evidence_get() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let evidence = call_mcp_tool(&bundle_dir, "evidence.get", &json!({"knowledge_item_id": "ki-a-000001"}), false).unwrap();
        assert_eq!(evidence["knowledge_item_id"], "ki-a-000001");
        let err = call_mcp_tool(&bundle_dir, "evidence.get", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: knowledge_item_id");
    }

    #[test]
    fn dispatch_conflicts_and_freshness_wrap_markdown() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let conflicts = call_mcp_tool(&bundle_dir, "conflicts.list", &json!({}), false).unwrap();
        assert!(conflicts["markdown"].as_str().unwrap().contains("Not generated"));
        let freshness = call_mcp_tool(&bundle_dir, "freshness.check", &json!({}), false).unwrap();
        assert!(freshness["markdown"].as_str().unwrap().contains("Not generated"));
    }

    #[test]
    fn dispatch_citations_resolve() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let resolved = call_mcp_tool(&bundle_dir, "citations.resolve", &json!({"citation_id": "evidence-ki-a-000001"}), false).unwrap();
        assert_eq!(resolved["knowledge_item_id"], "ki-a-000001");
        let err = call_mcp_tool(&bundle_dir, "citations.resolve", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: citation_id");
    }

    #[test]
    fn dispatch_agent_guide_wraps_markdown() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let result = call_mcp_tool(&bundle_dir, "agent.guide", &json!({}), false).unwrap();
        assert!(result["markdown"].as_str().unwrap().starts_with("# SOP KB Agent Guide"));
    }

    #[test]
    fn dispatch_agent_tasks_returns_three() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let tasks = call_mcp_tool(&bundle_dir, "agent.tasks", &json!({}), false).unwrap();
        assert_eq!(tasks.as_array().unwrap().len(), 3);
    }

    #[test]
    fn dispatch_agent_context_always_excludes_rejected() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let context = call_mcp_tool(&bundle_dir, "agent.context", &json!({"task": "eligibility-check"}), false).unwrap();
        assert_eq!(context["task"]["id"], "eligibility-check");
        let err = call_mcp_tool(&bundle_dir, "agent.context", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: task");
    }

    #[test]
    fn dispatch_relations_search_coerces_missing_and_null_and_empty_to_no_filter() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let all = call_mcp_tool(&bundle_dir, "relations.search", &json!({}), false).unwrap();
        assert_eq!(all.as_array().unwrap().len(), 1);
        let with_null = call_mcp_tool(&bundle_dir, "relations.search", &json!({"subject": null}), false).unwrap();
        assert_eq!(with_null.as_array().unwrap().len(), 1);
        let with_empty = call_mcp_tool(&bundle_dir, "relations.search", &json!({"subject": ""}), false).unwrap();
        assert_eq!(with_empty.as_array().unwrap().len(), 1);
    }

    #[test]
    fn dispatch_relations_search_rejects_non_string_filter_p_p10() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let err = call_mcp_tool(&bundle_dir, "relations.search", &json!({"subject": 5}), false).unwrap_err();
        assert_eq!(err.to_string(), "argument 'subject' must be a string");
        let err2 = call_mcp_tool(&bundle_dir, "relations.search", &json!({"predicate": true}), false).unwrap_err();
        assert_eq!(err2.to_string(), "argument 'predicate' must be a string");
        let err3 = call_mcp_tool(&bundle_dir, "relations.search", &json!({"object": [1, 2]}), false).unwrap_err();
        assert_eq!(err3.to_string(), "argument 'object' must be a string");
    }

    #[test]
    fn dispatch_relations_neighborhood() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let result = call_mcp_tool(&bundle_dir, "relations.neighborhood", &json!({"node_id": "staff"}), false).unwrap();
        assert_eq!(result["node_id"], "staff");
        let err = call_mcp_tool(&bundle_dir, "relations.neighborhood", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "missing required argument: node_id");
    }

    #[test]
    fn dispatch_unknown_tool() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let err = call_mcp_tool(&bundle_dir, "review.approve", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "unknown MCP tool: review.approve");
    }

    #[test]
    fn review_note_disabled_by_default_yields_specific_permission_message_not_unknown_tool() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let err = call_mcp_tool(
            &bundle_dir,
            "review.note",
            &json!({"knowledge_item_id": "ki-a-000001", "rationale": "looks fine"}),
            false,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "review.note is disabled; start with --enable-review-notes");
    }

    #[test]
    fn review_note_enabled_writes_a_real_review_event_and_syncs() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let result = call_mcp_tool(
            &bundle_dir,
            "review.note",
            &json!({"knowledge_item_id": "ki-a-000001", "rationale": "confirmed with legal", "reviewer": "alice"}),
            true,
        )
        .unwrap();
        assert_eq!(result["action"], "commented");
        assert_eq!(result["reviewer"], "alice");
        assert_eq!(result["rationale"], "confirmed with legal");

        let reviews = store::read_state_json(&bundle_dir, "reviews.json", json!([])).unwrap();
        assert_eq!(reviews.as_array().unwrap().len(), 1);
        assert_eq!(reviews[0]["action"], "commented");

        // items.json must be untouched by a note.
        let items = store::read_state_json(&bundle_dir, "items.json", json!([])).unwrap();
        assert_eq!(items[0]["review_status"], "proposed");

        // sync_okf_bundle side effect: knowledge OKF doc regenerated.
        assert!(bundle_dir.join("knowledge/ki-a-000001.md").exists());
    }

    #[test]
    fn review_note_defaults_reviewer_to_mcp_user_when_absent_null_or_empty() {
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let result = call_mcp_tool(
            &bundle_dir,
            "review.note",
            &json!({"knowledge_item_id": "ki-a-000001", "rationale": "r"}),
            true,
        )
        .unwrap();
        assert_eq!(result["reviewer"], "mcp:user");

        let result2 = call_mcp_tool(
            &bundle_dir,
            "review.note",
            &json!({"knowledge_item_id": "ki-a-000001", "rationale": "r", "reviewer": ""}),
            true,
        )
        .unwrap();
        assert_eq!(result2["reviewer"], "mcp:user");
    }

    #[test]
    fn review_note_missing_required_args_reported_before_permission_check_order_matches_python() {
        // Python checks enable_review_notes FIRST (before require_arg is ever called),
        // so a disabled server always yields the PermissionError even with missing args.
        let dir = bundle_with_one_item();
        let bundle_dir = dir.path().join("bundle");
        let err = call_mcp_tool(&bundle_dir, "review.note", &json!({}), false).unwrap_err();
        assert_eq!(err.to_string(), "review.note is disabled; start with --enable-review-notes");

        let err2 = call_mcp_tool(&bundle_dir, "review.note", &json!({}), true).unwrap_err();
        assert_eq!(err2.to_string(), "missing required argument: knowledge_item_id");
    }
}
