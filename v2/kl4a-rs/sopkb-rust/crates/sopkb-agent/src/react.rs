//! A tool-use "back and forth" loop for the Agent screen, additive alongside
//! `run::run_agent_chat` -- NOT a modification of it. `run.rs`'s own doc comment
//! (P-A35) explicitly preserves that function as a single-shot evaluation with no
//! Python equivalent for anything else, and this module does not touch it.
//!
//! No Python source exists for this at all: it's a genuinely new capability, built
//! in response to a direct request for something "a lot like MCP which simply
//! delegates to the model" but with "a back & forth... a thought process" before
//! settling on an answer, so it helps even a non-"thinking" model the same way a
//! reasoning model's own extended thinking does -- by giving it a chance to look
//! something up before committing to a final answer, not by making it deliberate
//! silently.
//!
//! ## Why a text protocol, not native function-calling
//!
//! `sopkb_llm::chat_call` talks to Azure's Responses API via `flatten_messages`,
//! which collapses every message into exactly one system string and one user
//! string (see `sopkb-llm/src/message.rs`) -- there is no wire-level notion of an
//! assistant turn or a tool-result turn today, and giving it one (the Responses
//! API's own `function_call`/`function_call_output` item types) would be a much
//! larger, riskier change to a request-building path this session has no way to
//! verify against a real endpoint. A back-and-forth built entirely in TEXT --
//! instructing the model to emit a recognizable line to request a tool, then
//! feeding the tool's result back in as another plain user-role message before
//! asking again -- gets the same real, multi-round effect (the model can decide it
//! needs more evidence, ask for it, and change its answer once it has it) using
//! nothing but the single-shot call this crate already has, and it works
//! identically regardless of whether the underlying model has native tool-calling
//! or extended thinking at all. This is the "least tokens, most straightforward"
//! path: no new wire format, no provider-specific capability detection.
//!
//! ## Tool catalog
//!
//! A small, curated subset of `sopkb-mcp`'s own read-only tools -- enough to let
//! the model dig for a SPECIFIC piece of evidence beyond what `scenario_agent_context`
//! already assembled up front, not a full second copy of every MCP tool. Each tool
//! dispatches to the exact same `sopkb_derive::{reads, relations}` functions
//! `bin/sopkb-mcp/src/tools.rs` calls -- the dispatch MATCH TABLE below is not
//! literally shared with that binary crate (it has no library target to import
//! from), but the underlying logic is: neither this module nor `sopkb-mcp` re-
//! implements what a tool actually does, both just call the one shared read layer.

use crate::transcript::append_agent_chat_entry;
use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use sopkb_llm::{Message, Transport};
use std::path::Path;

/// Bounded so a model that never emits `FINAL_ANSWER:` (ignores the protocol
/// entirely, or gets stuck re-requesting the same tool) can't loop indefinitely --
/// each iteration is a real network round trip with real token cost, and this
/// harness is meant to be a SHORT back-and-forth, not an open-ended agent run.
pub const MAX_REACT_ITERATIONS: usize = 4;

const TOOL_CATALOG: &[(&str, &str, &str)] = &[
    ("knowledge.search", "Search knowledge items by keyword.", r#"{"query": "<string>"}"#),
    ("knowledge.get", "Get one knowledge item by id.", r#"{"knowledge_item_id": "<string>"}"#),
    ("evidence.get", "Get the verbatim source-text evidence for a knowledge item.", r#"{"knowledge_item_id": "<string>"}"#),
    ("sections.get", "Get one normalized section by id.", r#"{"section_id": "<string>"}"#),
    (
        "relations.search",
        "Search structured subject/predicate/object relations (any field optional).",
        r#"{"subject": "<string?>", "predicate": "<string?>", "object": "<string?>"}"#,
    ),
];

fn build_react_system_prompt(base_system_prompt: &str) -> String {
    let tools = TOOL_CATALOG.iter().map(|(name, desc, args)| format!("- {name}({args}): {desc}")).collect::<Vec<_>>().join("\n");
    format!(
        "{base_system_prompt}\n\n\
        You may look up additional evidence before answering. Available tools:\n{tools}\n\n\
        To use a tool, respond with EXACTLY one line and nothing else:\n\
        TOOL_CALL: <tool_name> <json_arguments>\n\
        You will be given the tool's result and may call another tool, or answer.\n\
        Once you have enough information, respond with:\n\
        FINAL_ANSWER: <your complete answer>\n\
        Never mix a tool call and a final answer in the same response. If you don't need \
        to look anything up, just give a FINAL_ANSWER right away."
    )
}

#[derive(Debug, Clone, PartialEq)]
enum ReactAction {
    ToolCall { name: String, args: Value },
    FinalAnswer(String),
    /// The model ignored the protocol and just answered directly -- treated as the
    /// final answer rather than an error, so a model that doesn't follow the exact
    /// format still produces a usable result instead of a hard failure.
    PlainText(String),
}

fn parse_react_response(text: &str) -> ReactAction {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("FINAL_ANSWER:") {
        return ReactAction::FinalAnswer(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("TOOL_CALL:") {
        let rest = rest.trim();
        if let Some((name, args_str)) = rest.split_once(char::is_whitespace) {
            if let Ok(args) = serde_json::from_str::<Value>(args_str.trim()) {
                return ReactAction::ToolCall { name: name.trim().to_string(), args };
            }
        }
        // A TOOL_CALL line that doesn't parse (missing/malformed JSON args) is not
        // a final answer either -- surfacing the raw line lets the loop's caller
        // see what actually went wrong instead of silently treating garbage as an
        // answer.
        return ReactAction::PlainText(format!("(malformed tool call, treated as final answer): {trimmed}"));
    }
    ReactAction::PlainText(trimmed.to_string())
}

/// Dispatches one tool call to the exact same read functions `bin/sopkb-mcp`'s own
/// tool dispatch calls -- see this module's own doc comment for why the match table
/// itself isn't literally the same function. A tool error (unknown name, missing
/// argument, not-found id) becomes `{"error": "..."}` rather than aborting the
/// whole loop -- the MODEL sees the failure and can adapt (try a different id,
/// rephrase a query, or just answer with what it already has), matching how a real
/// tool-use failure is normally handled.
fn dispatch_tool(bundle_dir: &Path, name: &str, args: &Value) -> Value {
    fn str_arg(args: &Value, key: &str) -> Option<String> {
        args.get(key).and_then(|v| v.as_str()).map(str::to_string)
    }
    let result: Result<Value> = match name {
        "knowledge.search" => match str_arg(args, "query") {
            Some(q) => sopkb_derive::reads::knowledge_search(bundle_dir, &q).map(Value::Array),
            None => Err(SopkbError::Value("missing required argument: query".to_string())),
        },
        "knowledge.get" => match str_arg(args, "knowledge_item_id") {
            Some(id) => sopkb_derive::reads::knowledge_get(bundle_dir, &id),
            None => Err(SopkbError::Value("missing required argument: knowledge_item_id".to_string())),
        },
        "evidence.get" => match str_arg(args, "knowledge_item_id") {
            Some(id) => sopkb_derive::reads::evidence_get(bundle_dir, &id),
            None => Err(SopkbError::Value("missing required argument: knowledge_item_id".to_string())),
        },
        "sections.get" => match str_arg(args, "section_id") {
            Some(id) => sopkb_derive::reads::sections_get(bundle_dir, &id),
            None => Err(SopkbError::Value("missing required argument: section_id".to_string())),
        },
        "relations.search" => {
            let subject = str_arg(args, "subject").unwrap_or_default();
            let predicate = str_arg(args, "predicate").unwrap_or_default();
            let object = str_arg(args, "object").unwrap_or_default();
            sopkb_derive::relations::relations_search(bundle_dir, &subject, &predicate, &object).map(Value::Array)
        }
        other => Err(SopkbError::Value(format!("unknown tool: {other}"))),
    };
    match result {
        Ok(value) => value,
        Err(err) => json!({ "error": err.to_string() }),
    }
}

/// The main loop. Builds on `sopkb_llm::chat_call_with_transport` (each call is
/// still, individually, the exact same single-shot HTTP request `run_agent_chat`
/// makes) by growing the message list with a tool-result turn after every
/// `TOOL_CALL` and calling again -- `flatten_messages` joins all `user`-role
/// messages in order, so each successive call replays the whole transcript so far
/// as one growing prompt rather than needing any server-side conversation state.
///
/// Returns a response shape deliberately compatible with `run_agent_chat`'s own
/// (`provider`/`task_title`/`scenario`/`answer`/`context_summary`/...), so it can
/// be appended to the SAME `.sopkb/agent_chat.json` transcript and rendered by the
/// existing Agent screen without that screen needing to know two different entry
/// shapes exist -- plus a `trace` field (the tool calls actually made) that today's
/// UI simply won't render, matching this codebase's own "never drop an unknown
/// key" convention rather than needing a schema bump to add later.
pub fn run_react_chat_with_transport(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    profile_id: Option<&str>,
    transport: &dyn Transport,
) -> Result<Value> {
    run_react_chat_with_history_and_transport(bundle_dir, task_id, scenario, allow_proposed, profile_id, "", transport)
}

/// Same as `run_react_chat_with_transport`, plus an optional `history` block
/// (see `chat::history_block`) prepended to the first user turn -- an empty
/// `history` produces a byte-identical outbound request to the no-history
/// path above, which is exactly how that function is implemented (a thin
/// wrapper calling this one with `history: ""`). This is what lets
/// `chat::run_chat_turn_with_transport` give the `azure-llm-tools` provider
/// real per-chat memory without duplicating the whole ReAct loop.
pub fn run_react_chat_with_history_and_transport(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    profile_id: Option<&str>,
    history: &str,
    transport: &dyn Transport,
) -> Result<Value> {
    let context = sopkb_derive::context::scenario_agent_context(bundle_dir, scenario, Some(task_id), false, 32)?;
    // See sopkb_core::prompt_overrides's own doc comment -- a non-blank bundle-level
    // override wins over even a non-blank per-profile one (mirrors run_agent_chat's
    // own azure-llm branch).
    let overrides = sopkb_core::prompt_overrides::read_bundle_prompt_overrides(bundle_dir);
    let chat_override = if overrides.chat_prompt.trim().is_empty() { None } else { Some(overrides.chat_prompt.as_str()) };
    let base_messages = crate::prompt::build_agent_messages(&context, scenario, allow_proposed, profile_id, chat_override);
    let base_system_prompt = base_messages.iter().find(|m| m.role == "system").map(|m| m.content.clone()).unwrap_or_default();

    let mut messages = vec![Message::system(build_react_system_prompt(&base_system_prompt))];
    messages.extend(base_messages.into_iter().filter(|m| m.role != "system").map(|mut m| {
        if !history.is_empty() {
            m.content = format!("{history}{}", m.content);
        }
        m
    }));

    let mut trace: Vec<Value> = Vec::new();
    let mut last_text = String::new();

    for _ in 0..MAX_REACT_ITERATIONS {
        let response_text = sopkb_llm::chat_call_with_transport(&messages, profile_id, transport)?;
        last_text = response_text.clone();
        match parse_react_response(&response_text) {
            ReactAction::FinalAnswer(answer) => {
                return Ok(build_response(&context, scenario, allow_proposed, task_id, profile_id, answer, trace));
            }
            ReactAction::PlainText(text) => {
                return Ok(build_response(&context, scenario, allow_proposed, task_id, profile_id, text, trace));
            }
            ReactAction::ToolCall { name, args } => {
                let result = dispatch_tool(bundle_dir, &name, &args);
                trace.push(json!({ "tool": name, "args": args, "result": &result }));
                messages.push(Message::user(format!(
                    "Tool result for {name}({args}): {}",
                    serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string())
                )));
            }
        }
    }

    // Iteration cap hit without a FINAL_ANSWER -- return the last response as a
    // disclosed best-effort answer rather than an error; the caller still gets
    // something, and `trace` shows exactly how much looking-around happened first.
    let disclosed = format!("(stopped after {MAX_REACT_ITERATIONS} tool round-trips without a final answer)\n\n{last_text}");
    Ok(build_response(&context, scenario, allow_proposed, task_id, profile_id, disclosed, trace))
}

pub fn run_react_chat(bundle_dir: &Path, task_id: &str, scenario: &str, allow_proposed: bool, profile_id: Option<&str>) -> Result<Value> {
    run_react_chat_with_transport(bundle_dir, task_id, scenario, allow_proposed, profile_id, &sopkb_llm::UreqTransport)
}

/// Same combination `transcript::handle_agent_chat` does for the single-shot path:
/// run, then persist to the one shared transcript.
pub fn handle_react_chat(bundle_dir: &Path, task_id: &str, scenario: &str, allow_proposed: bool, profile_id: Option<&str>) -> Result<Value> {
    let response = run_react_chat(bundle_dir, task_id, scenario, allow_proposed, profile_id)?;
    append_agent_chat_entry(bundle_dir, response)
}

fn build_response(
    context: &Value,
    scenario: &str,
    allow_proposed: bool,
    task_id: &str,
    profile_id: Option<&str>,
    answer: String,
    trace: Vec<Value>,
) -> Value {
    let array_len = |key: &str| context.get(key).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let detected_concepts: Vec<Value> = context
        .get("retrieval")
        .and_then(|r| r.get("detected_concepts"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|c| {
            json!({
                "id": c.get("id").cloned().unwrap_or(Value::Null),
                "label": c.get("label").cloned().unwrap_or(Value::Null),
                "score": c.get("score").cloned().unwrap_or(Value::Null),
                "match_reasons": c.get("match_reasons").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect();

    json!({
        "provider": "azure-llm-tools",
        "profile_id": profile_id,
        "task_id": task_id,
        "task_title": context["task"]["title"],
        "scenario": scenario,
        "allow_proposed": allow_proposed,
        "detected_concepts": detected_concepts,
        "answer": answer,
        "context_summary": {
            "usable_knowledge_count": array_len("usable_knowledge"),
            "decision_rule_count": array_len("decision_rules"),
            "relation_count": array_len("knowledge_relations"),
            "evidence_count": array_len("evidence"),
            "concept_count": array_len("concepts"),
            "detected_concept_count": detected_concepts.len(),
            "excluded_knowledge_count": context.get("excluded_knowledge_count").cloned().unwrap_or(json!(0)),
        },
        "trace": trace,
        "tool_iterations": trace.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let items = json!([{
            "id": "ki-1", "subject": "Staff", "predicate": "must confirm", "object": "patient identity",
            "source_id": "s", "section_id": "sec", "review_status": "accepted", "confidence": 0.9,
            "source_text": "Staff must confirm patient identity.", "span_status": "exact", "start_pos": 0, "end_pos": 10
        }]);
        store::write_state_json(dir, "items.json", &items).unwrap();
    }

    fn profile() {
        let profile = sopkb_config::ModelProfile {
            id: "p1".into(),
            base_url: "https://example.test".into(),
            api_key: "k".into(),
            model: "m".into(),
            ..Default::default()
        };
        sopkb_config::save_profile(&profile).unwrap();
    }

    #[test]
    fn parses_final_answer() {
        assert_eq!(parse_react_response("FINAL_ANSWER: Confirm identity first."), ReactAction::FinalAnswer("Confirm identity first.".to_string()));
    }

    #[test]
    fn parses_tool_call_with_json_args() {
        assert_eq!(
            parse_react_response(r#"TOOL_CALL: knowledge.search {"query": "identity"}"#),
            ReactAction::ToolCall { name: "knowledge.search".to_string(), args: json!({"query": "identity"}) }
        );
    }

    #[test]
    fn treats_a_response_ignoring_the_protocol_as_a_plain_final_answer() {
        assert_eq!(parse_react_response("The answer is: yes, confirm first."), ReactAction::PlainText("The answer is: yes, confirm first.".to_string()));
    }

    #[test]
    fn malformed_tool_call_json_does_not_panic_and_is_surfaced_not_silently_answered() {
        match parse_react_response("TOOL_CALL: knowledge.search {not valid json}") {
            ReactAction::PlainText(text) => assert!(text.contains("malformed tool call")),
            other => panic!("expected PlainText, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_tool_unknown_name_returns_error_payload_not_a_crash() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        let result = dispatch_tool(&bundle_dir, "not.a.real.tool", &json!({}));
        assert!(result["error"].as_str().unwrap().contains("unknown tool"));
    }

    #[test]
    fn dispatch_tool_knowledge_search_finds_the_real_item() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let result = dispatch_tool(&bundle_dir, "knowledge.search", &json!({"query": "identity"}));
        let items = result.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], json!("ki-1"));
    }

    /// The key proof test: an actual multi-round trip, not just "the types compile".
    /// First call returns a TOOL_CALL; this asserts the tool's REAL result (not a
    /// stub) gets folded into the second request's body, and the second call's
    /// FINAL_ANSWER becomes the loop's own returned answer.
    #[test]
    #[serial_test::serial]
    fn round_trips_a_real_tool_call_into_a_final_answer() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let tool_call_response = br#"{"output_text": "TOOL_CALL: knowledge.search {\"query\": \"identity\"}"}"#.to_vec();
            let final_response = br#"{"output_text": "FINAL_ANSWER: Staff must confirm patient identity before proceeding."}"#.to_vec();
            let transport = sopkb_llm::MockTransport::sequence(vec![
                Ok(sopkb_llm::HttpResponse { status: 200, body: tool_call_response }),
                Ok(sopkb_llm::HttpResponse { status: 200, body: final_response }),
            ]);

            let response = run_react_chat_with_transport(&bundle_dir, "auto", "can staff proceed", false, Some("p1"), &transport).unwrap();

            assert_eq!(response["answer"], json!("Staff must confirm patient identity before proceeding."));
            assert_eq!(response["provider"], json!("azure-llm-tools"));
            assert_eq!(response["tool_iterations"], json!(1));
            let trace = response["trace"].as_array().unwrap();
            assert_eq!(trace.len(), 1);
            assert_eq!(trace[0]["tool"], json!("knowledge.search"));
            assert_eq!(trace[0]["result"][0]["id"], json!("ki-1"), "the tool's REAL search result, not a stub, must appear in the trace");

            let requests = transport.all_requests();
            assert_eq!(requests.len(), 2, "must make exactly two HTTP calls: the tool-call turn and the final-answer turn");
            let second_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
            let second_input = second_body["input"].as_str().unwrap();
            assert!(
                second_input.contains("ki-1") && second_input.contains("Staff must confirm patient identity"),
                "the second request must actually include the first tool call's real result, not just a placeholder: {second_input}"
            );
        });
    }

    #[test]
    #[serial_test::serial]
    fn a_model_that_never_calls_a_tool_answers_in_one_round_trip() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let transport = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "FINAL_ANSWER: Yes, proceed."}"#.to_vec());
            let response = run_react_chat_with_transport(&bundle_dir, "auto", "can staff proceed", false, Some("p1"), &transport).unwrap();
            assert_eq!(response["answer"], json!("Yes, proceed."));
            assert_eq!(response["tool_iterations"], json!(0));
        });
    }

    #[test]
    #[serial_test::serial]
    fn history_and_no_history_produce_a_byte_identical_first_request_when_history_is_empty() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let transport_a = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "FINAL_ANSWER: ok"}"#.to_vec());
            run_react_chat_with_transport(&bundle_dir, "auto", "can staff proceed", false, Some("p1"), &transport_a).unwrap();

            let transport_b = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "FINAL_ANSWER: ok"}"#.to_vec());
            run_react_chat_with_history_and_transport(&bundle_dir, "auto", "can staff proceed", false, Some("p1"), "", &transport_b).unwrap();

            assert_eq!(transport_a.last_request().unwrap().body, transport_b.last_request().unwrap().body);
        });
    }

    #[test]
    #[serial_test::serial]
    fn a_non_empty_history_is_prepended_to_the_first_outbound_user_message() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let transport = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "FINAL_ANSWER: ok"}"#.to_vec());
            run_react_chat_with_history_and_transport(
                &bundle_dir,
                "auto",
                "can staff proceed",
                false,
                Some("p1"),
                "Earlier turns in this conversation, oldest first:\n\n[Turn 1]\nUser: q1\nAssistant: a1\n\n",
                &transport,
            )
            .unwrap();

            let request = transport.last_request().unwrap();
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let input = body["input"].as_str().unwrap();
            assert!(input.starts_with("Earlier turns in this conversation"), "history must be prepended, not appended or dropped: {input}");
            assert!(input.contains("Assistant: a1"));
        });
    }

    #[test]
    #[serial_test::serial]
    fn stops_after_the_iteration_cap_and_discloses_it_rather_than_looping_forever() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            // Always asks for another tool call, never answers -- queue exactly
            // MAX_REACT_ITERATIONS responses, proving the loop stops there rather
            // than calling a MockTransport with nothing left queued (which panics).
            let responses = (0..MAX_REACT_ITERATIONS)
                .map(|_| Ok(sopkb_llm::HttpResponse { status: 200, body: br#"{"output_text": "TOOL_CALL: knowledge.search {\"query\": \"x\"}"}"#.to_vec() }))
                .collect();
            let transport = sopkb_llm::MockTransport::sequence(responses);

            let response = run_react_chat_with_transport(&bundle_dir, "auto", "can staff proceed", false, Some("p1"), &transport).unwrap();
            assert_eq!(response["tool_iterations"], json!(MAX_REACT_ITERATIONS));
            assert!(response["answer"].as_str().unwrap().contains("stopped after"));
        });
    }
}
