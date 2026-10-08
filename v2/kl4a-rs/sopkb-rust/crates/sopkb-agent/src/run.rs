//! `run_agent_chat` (`agent_chat.py` lines 22-70). docs/port/port-mapping-d-agent-mcp.md §5.2.

use crate::context_answer::render_context_answer;
use crate::prompt::build_agent_messages;
use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use std::path::Path;

fn array_len(context: &Value, key: &str) -> usize {
    context.get(key).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
}

/// Slim `{id, label, score, match_reasons}` projection -- same shape as
/// `context_answer::slim_concept`, duplicated here to keep the two modules
/// independently readable (mirrors the Python source, which repeats the same list
/// comprehension in both `run_agent_chat` and `render_context_answer`).
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

/// `run_agent_chat(bundle_dir, task_id, scenario, allow_proposed, provider, profile_id)`.
///
/// `scenario_agent_context` is always called with `include_rejected = false` and
/// `item_limit = 32` -- neither is forwarded from here (P-A30: no token budgeting
/// beyond that hard-wired limit).
///
/// Delegates to `run_agent_chat_with_transport` with the real `UreqTransport` -- see
/// that function for the injectable-transport variant used by tests (mirrors
/// `sopkb_llm::chat_call`/`chat_call_with_transport`'s own split).
pub fn run_agent_chat(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
) -> Result<Value> {
    run_agent_chat_with_transport(bundle_dir, task_id, scenario, allow_proposed, provider, profile_id, &sopkb_llm::UreqTransport)
}

/// Same as `run_agent_chat`, with the HTTP transport injectable so the `azure-llm`
/// path is testable with `sopkb_llm::MockTransport` and no real network call.
pub fn run_agent_chat_with_transport(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
    transport: &dyn sopkb_llm::Transport,
) -> Result<Value> {
    let context = sopkb_derive::context::scenario_agent_context(bundle_dir, scenario, Some(task_id), false, 32)?;

    let answer = match provider {
        "context" => render_context_answer(&context, scenario, allow_proposed),
        "azure-llm" => {
            // See sopkb_core::prompt_overrides's own doc comment -- a non-blank
            // bundle-level override wins over even a non-blank per-profile one.
            let overrides = sopkb_core::prompt_overrides::read_bundle_prompt_overrides(bundle_dir);
            let chat_override = if overrides.chat_prompt.trim().is_empty() { None } else { Some(overrides.chat_prompt.as_str()) };
            let messages = build_agent_messages(&context, scenario, allow_proposed, profile_id, chat_override);
            sopkb_llm::chat_call_with_transport(&messages, profile_id, transport)?
        }
        other => return Err(SopkbError::Value(format!("unsupported agent provider: {other}"))),
    };

    // P-A34: slim 4-field projection here; the fat version stays inside
    // context.retrieval.detected_concepts and is never returned or persisted.
    let detected_concepts: Vec<Value> = detected_concepts_of(&context).iter().map(slim_concept).collect();

    let context_summary = json!({
        "usable_knowledge_count": array_len(&context, "usable_knowledge"),
        "decision_rule_count": array_len(&context, "decision_rules"),
        "relation_count": array_len(&context, "knowledge_relations"),
        "evidence_count": array_len(&context, "evidence"),
        "concept_count": array_len(&context, "concepts"),
        "detected_concept_count": detected_concepts.len(),
        "excluded_knowledge_count": context.get("excluded_knowledge_count").cloned().unwrap_or(json!(0)),
    });

    Ok(json!({
        "provider": provider,
        "profile_id": profile_id,
        // P-A33: echoed AS PASSED IN (e.g. "auto"), distinct from the resolved title below.
        "task_id": task_id,
        "task_title": context["task"]["title"],
        "scenario": scenario,
        "allow_proposed": allow_proposed,
        "detected_concepts": detected_concepts,
        "answer": answer,
        "context_summary": context_summary,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use sopkb_core::store;
    use tempfile::tempdir;

    fn with_settings_path<F: FnOnce()>(f: F) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        unsafe { std::env::set_var("SOPKB_SETTINGS_PATH", &path) };
        f();
        unsafe { std::env::remove_var("SOPKB_SETTINGS_PATH") };
    }

    fn bundle_with_item(dir: &Path) {
        store::create_bundle(dir, None).unwrap();
        let items = serde_json::json!([
            {
                "id": "ki-1", "subject": "Staff", "predicate": "must confirm", "object": "patient identity",
                "source_id": "s", "section_id": "sec", "review_status": "accepted", "confidence": 0.9,
                "source_text": "Staff must confirm patient identity.", "span_status": "exact", "start_pos": 0, "end_pos": 10
            }
        ]);
        store::write_state_json(dir, "items.json", &items).unwrap();
    }

    #[test]
    fn unsupported_provider_errors() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let err = run_agent_chat(&bundle_dir, "auto", "a scenario", false, "not-a-real-provider", None).unwrap_err();
        assert_eq!(err.to_string(), "unsupported agent provider: not-a-real-provider");
    }

    #[test]
    fn context_provider_echoes_task_id_raw_and_resolves_title() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let response = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        assert_eq!(response["task_id"], json!("auto"));
        assert_eq!(response["task_title"], json!("Scenario Analysis"));
    }

    #[test]
    fn allow_proposed_never_filters_knowledge() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let with_false = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        let with_true = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", true, "context", None).unwrap();
        assert_eq!(with_false["context_summary"]["usable_knowledge_count"], with_true["context_summary"]["usable_knowledge_count"]);
        assert_eq!(with_false["allow_proposed"], json!(false));
        assert_eq!(with_true["allow_proposed"], json!(true));
    }

    #[test]
    fn context_summary_field_counts_match_context_object() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let response = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        let summary = &response["context_summary"];
        assert_eq!(summary["usable_knowledge_count"], json!(1));
        assert_eq!(summary["relation_count"], json!(1));
        assert_eq!(summary["evidence_count"], json!(1));
        assert_eq!(summary["concept_count"], json!(1));
        assert_eq!(summary["excluded_knowledge_count"], json!(0));
    }

    #[test]
    fn detected_concepts_is_slim_four_field_projection() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let response = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        let concepts = response["detected_concepts"].as_array().unwrap();
        assert!(!concepts.is_empty());
        for c in concepts {
            let obj = c.as_object().unwrap();
            let mut keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
            keys.sort();
            assert_eq!(keys, vec!["id", "label", "match_reasons", "score"]);
        }
    }

    #[test]
    fn profile_id_null_is_echoed_as_json_null() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let response = run_agent_chat(&bundle_dir, "auto", "confirm patient identity", false, "context", None).unwrap();
        assert!(response["profile_id"].is_null());
    }

    /// Recorded-response harness for the `azure-llm` path: no real network call, a
    /// hand-authored (NOT captured -- no live Azure endpoint is reachable in this
    /// environment) plausible Responses-API success body via `MockTransport`, with a
    /// byte-comparison of the outbound request against independently-recomputed
    /// `build_agent_messages` output.
    #[test]
    #[serial]
    fn azure_llm_path_sends_expected_outbound_request_and_returns_model_text() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile {
                id: "p1".into(),
                base_url: "https://example.test".into(),
                api_key: "secret-key".into(),
                model: "gpt-5".into(),
                ..Default::default()
            };
            sopkb_config::save_profile(&profile).unwrap();

            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let hand_authored_response = b"{\"output_text\": \"## Decision\\n\\nEligible pending identity confirmation.\"}".to_vec();
            let transport = sopkb_llm::MockTransport::ok(200, hand_authored_response);

            let scenario = "confirm patient identity";
            let response = run_agent_chat_with_transport(&bundle_dir, "auto", scenario, true, "azure-llm", Some("p1"), &transport).unwrap();
            assert_eq!(response["answer"], json!("## Decision\n\nEligible pending identity confirmation."));

            let request = transport.last_request().unwrap();
            assert_eq!(request.url, "https://example.test/responses");
            assert!(request.headers.iter().any(|(k, v)| k == "api-key" && v == "secret-key"));

            let body: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["model"], json!("gpt-5"));
            assert_eq!(body["instructions"], json!(crate::prompt::SYSTEM_PROMPT));

            // Independently recompute the exact expected `input` via the same pure
            // functions this crate exposes, and assert byte equality against what was
            // actually sent over the wire.
            let context = sopkb_derive::context::scenario_agent_context(&bundle_dir, scenario, Some("auto"), false, 32).unwrap();
            let expected_messages = build_agent_messages(&context, scenario, true, Some("p1"), None);
            assert_eq!(body["input"], json!(expected_messages[1].content));
        });
    }

    #[test]
    #[serial]
    fn azure_llm_path_uses_a_bundle_level_chat_prompt_override_when_present() {
        with_settings_path(|| {
            let profile = sopkb_config::ModelProfile {
                id: "p1".into(),
                base_url: "https://example.test".into(),
                api_key: "secret-key".into(),
                model: "gpt-5".into(),
                chat_prompt: "Profile-level prompt that should lose.".into(),
                ..Default::default()
            };
            sopkb_config::save_profile(&profile).unwrap();

            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);
            sopkb_core::prompt_overrides::write_bundle_prompt_overrides(
                &bundle_dir,
                &sopkb_core::prompt_overrides::BundlePromptOverrides {
                    mining_prompt: String::new(),
                    chat_prompt: "Bundle-level prompt that should win.".into(),
                },
            )
            .unwrap();

            let hand_authored_response = b"{\"output_text\": \"ok\"}".to_vec();
            let transport = sopkb_llm::MockTransport::ok(200, hand_authored_response);
            run_agent_chat_with_transport(&bundle_dir, "auto", "confirm patient identity", true, "azure-llm", Some("p1"), &transport).unwrap();

            let request = transport.last_request().unwrap();
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["instructions"], json!("Bundle-level prompt that should win."));
        });
    }
}
