//! Per-chat memory for the desktop Agent screen (round 6, item 15) -- additive
//! alongside `run::run_agent_chat` and `react::run_react_chat`, exactly like
//! `react.rs` itself is additive alongside `run.rs` (see that module's own doc
//! comment): neither of those two functions is modified here, and P-A35's
//! "single-shot, no cross-turn memory" invariant on `run_agent_chat` still
//! holds for every existing caller (`sopkb-cli`, `sopkb-server`, and the
//! Python-parity fixture test in `tests/phase8_reference_diff.rs`).
//!
//! A `chat_id` (generated client-side by the Agent screen, one per chat) is
//! threaded through every turn and stored on that turn's transcript entry.
//! Before answering a NEW turn, this module reads every PRIOR entry sharing
//! that `chat_id` out of the same `agent_chat.json` transcript everything
//! else already writes to, and folds them into the outgoing request as a
//! plain-text history block (`history_block` below) -- `flatten_messages`
//! (`sopkb-llm/src/message.rs`) has no assistant-role channel, only "system"
//! and "user" get joined, so "the model remembers the last answer" has to
//! mean "the last answer's text is folded into this turn's user content",
//! the same trick `react.rs` already uses to feed a tool result back in.
//! A chat's first turn has no prior entries, so `history_block` returns "",
//! and the outbound request is byte-identical to the no-memory path -- memory
//! is purely additive, never a behavior change for a brand-new chat.

use crate::context_answer::render_context_answer;
use crate::prompt::build_agent_messages;
use crate::transcript::append_agent_chat_entry_capped;
use serde_json::{json, Value};
use sopkb_core::error::{Result, SopkbError};
use sopkb_llm::Transport;
use std::path::Path;

/// A real chat list (round 6, item 15) needs every chat's own turns to
/// survive, not just the most recent 25 turns across EVERY chat combined
/// (`transcript::DEFAULT_TRANSCRIPT_CAP`, kept at 25 elsewhere for exact
/// Python parity -- see that constant's own doc comment). This is the
/// "future caller... choose a larger value instead of silently losing chat
/// history" the parity-preserving default already anticipated. 1000 turns is
/// generous for a local desktop bundle (each entry is a few KB: an answer
/// plus slim concept/summary counts, not the full retrieved context) while
/// still bounding the file.
pub const CHAT_TRANSCRIPT_CAP: usize = 1000;

/// One prior (question, answer) pair from this chat, oldest matched first.
pub struct ChatTurn {
    pub scenario: String,
    pub answer: String,
}

/// Every transcript entry sharing `chat_id`, oldest first (`read_agent_transcript`
/// already returns oldest-first). Entries with no `chat_id` field (written before
/// this feature existed) or a missing `scenario`/`answer` never match/parse and are
/// silently skipped -- they still surface in the frontend's own chat list as an
/// "unfiled" bucket grouped client-side, but there is nothing to key them by here.
pub fn chat_turns_for(bundle_dir: &Path, chat_id: &str) -> Result<Vec<ChatTurn>> {
    let transcript = crate::transcript::read_agent_transcript(bundle_dir)?;
    Ok(transcript
        .into_iter()
        .filter(|e| e.get("chat_id").and_then(|v| v.as_str()) == Some(chat_id))
        .filter_map(|e| {
            let scenario = e.get("scenario")?.as_str()?.to_string();
            let answer = e.get("answer")?.as_str()?.to_string();
            Some(ChatTurn { scenario, answer })
        })
        .collect())
}

/// "" when there is no history yet -- see this module's own doc comment on why that
/// makes a chat's first turn identical to the no-memory path.
pub fn history_block(turns: &[ChatTurn]) -> String {
    if turns.is_empty() {
        return String::new();
    }
    let mut out = String::from("Earlier turns in this conversation, oldest first:\n\n");
    for (i, turn) in turns.iter().enumerate() {
        out.push_str(&format!("[Turn {}]\nUser: {}\nAssistant: {}\n\n", i + 1, turn.scenario, turn.answer));
    }
    out.push_str("---\nRespond to the NEW scenario below, using the earlier turns above as context.\n\n");
    out
}

/// Same slim 4-field `detected_concepts` projection `run.rs`/`react.rs` each keep
/// their own private copy of (P-A34) -- duplicated a third time here rather than
/// exported from either, matching this codebase's existing choice to keep the two
/// modules independently readable instead of sharing a helper across them.
fn slim_concept(concept: &Value) -> Value {
    json!({
        "id": concept.get("id").cloned().unwrap_or(Value::Null),
        "label": concept.get("label").cloned().unwrap_or(Value::Null),
        "score": concept.get("score").cloned().unwrap_or(Value::Null),
        "match_reasons": concept.get("match_reasons").cloned().unwrap_or_else(|| json!([])),
    })
}

fn array_len(context: &Value, key: &str) -> usize {
    context.get(key).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
}

/// The `"context"`/`"azure-llm"` half of `run_chat_turn_with_transport` --
/// `"azure-llm-tools"` is handled separately via `react::run_react_chat_with_history_and_transport`,
/// which already owns its own response shape (including `trace`). Shaped to match
/// `run::run_agent_chat_with_transport`'s response exactly (same fields, same
/// meaning) since the Agent screen renders both through one shared `AgentEntry`
/// type -- the only difference is the history-aware `azure-llm` branch below.
fn run_single_shot_with_history(
    bundle_dir: &Path,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
    history: &str,
    transport: &dyn Transport,
) -> Result<Value> {
    let context = sopkb_derive::context::scenario_agent_context(bundle_dir, scenario, Some(task_id), false, 32)?;

    let answer = match provider {
        "context" => render_context_answer(&context, scenario, allow_proposed),
        "azure-llm" => {
            let overrides = sopkb_core::prompt_overrides::read_bundle_prompt_overrides(bundle_dir);
            let chat_override = if overrides.chat_prompt.trim().is_empty() { None } else { Some(overrides.chat_prompt.as_str()) };
            let mut messages = build_agent_messages(&context, scenario, allow_proposed, profile_id, chat_override);
            if !history.is_empty() {
                if let Some(user_msg) = messages.iter_mut().find(|m| m.role == "user") {
                    user_msg.content = format!("{history}{}", user_msg.content);
                }
            }
            sopkb_llm::chat_call_with_transport(&messages, profile_id, transport)?
        }
        other => return Err(SopkbError::Value(format!("unsupported agent provider: {other}"))),
    };

    let detected_concepts: Vec<Value> = context
        .get("retrieval")
        .and_then(|r| r.get("detected_concepts"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(slim_concept)
        .collect();

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
        "task_id": task_id,
        "task_title": context["task"]["title"],
        "scenario": scenario,
        "allow_proposed": allow_proposed,
        "detected_concepts": detected_concepts,
        "answer": answer,
        "context_summary": context_summary,
    }))
}

/// Runs one turn of a real, remembered conversation: reads this chat's own prior
/// turns, folds them into the outgoing request (see this module's own doc comment),
/// dispatches to the matching provider, and tags the response with `chat_id` so the
/// caller can persist it against the same chat. Does not persist by itself -- see
/// `run_chat_turn`/`handle_chat_turn` below, mirroring `run::run_agent_chat_with_transport`
/// vs `transcript::handle_agent_chat`'s own run/persist split.
pub fn run_chat_turn_with_transport(
    bundle_dir: &Path,
    chat_id: &str,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
    transport: &dyn Transport,
) -> Result<Value> {
    let history = history_block(&chat_turns_for(bundle_dir, chat_id)?);

    let mut response = if provider == "azure-llm-tools" {
        crate::react::run_react_chat_with_history_and_transport(bundle_dir, task_id, scenario, allow_proposed, profile_id, &history, transport)?
    } else {
        run_single_shot_with_history(bundle_dir, task_id, scenario, allow_proposed, provider, profile_id, &history, transport)?
    };

    if let Value::Object(ref mut map) = response {
        map.insert("chat_id".to_string(), json!(chat_id));
    }
    Ok(response)
}

pub fn run_chat_turn(
    bundle_dir: &Path,
    chat_id: &str,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
) -> Result<Value> {
    run_chat_turn_with_transport(bundle_dir, chat_id, task_id, scenario, allow_proposed, provider, profile_id, &sopkb_llm::UreqTransport)
}

/// Combines `run_chat_turn` with transcript persistence, using `CHAT_TRANSCRIPT_CAP`
/// (not the 25-entry `DEFAULT_TRANSCRIPT_CAP`) since a real multi-chat history needs
/// far more room -- see that constant's own doc comment.
pub fn handle_chat_turn(
    bundle_dir: &Path,
    chat_id: &str,
    task_id: &str,
    scenario: &str,
    allow_proposed: bool,
    provider: &str,
    profile_id: Option<&str>,
) -> Result<Value> {
    let response = run_chat_turn(bundle_dir, chat_id, task_id, scenario, allow_proposed, provider, profile_id)?;
    append_agent_chat_entry_capped(bundle_dir, response, CHAT_TRANSCRIPT_CAP)
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
        let items = json!([{
            "id": "ki-1", "subject": "Staff", "predicate": "must confirm", "object": "patient identity",
            "source_id": "s", "section_id": "sec", "review_status": "accepted", "confidence": 0.9,
            "source_text": "Staff must confirm patient identity.", "span_status": "exact", "start_pos": 0, "end_pos": 10
        }]);
        store::write_state_json(dir, "items.json", &items).unwrap();
    }

    fn profile() {
        let profile = sopkb_config::ModelProfile { id: "p1".into(), base_url: "https://example.test".into(), api_key: "k".into(), model: "m".into(), ..Default::default() };
        sopkb_config::save_profile(&profile).unwrap();
    }

    #[test]
    fn history_block_is_empty_for_no_turns() {
        assert_eq!(history_block(&[]), "");
    }

    #[test]
    fn history_block_numbers_turns_and_labels_user_assistant() {
        let turns = vec![
            ChatTurn { scenario: "q1".into(), answer: "a1".into() },
            ChatTurn { scenario: "q2".into(), answer: "a2".into() },
        ];
        let block = history_block(&turns);
        assert!(block.contains("[Turn 1]\nUser: q1\nAssistant: a1"));
        assert!(block.contains("[Turn 2]\nUser: q2\nAssistant: a2"));
    }

    #[test]
    fn chat_turns_for_filters_by_chat_id_and_skips_legacy_entries() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        store::create_bundle(&bundle_dir, None).unwrap();
        crate::transcript::append_agent_chat_entry(&bundle_dir, json!({"chat_id": "c1", "scenario": "q1", "answer": "a1"})).unwrap();
        crate::transcript::append_agent_chat_entry(&bundle_dir, json!({"chat_id": "c2", "scenario": "other", "answer": "other-a"})).unwrap();
        crate::transcript::append_agent_chat_entry(&bundle_dir, json!({"scenario": "legacy, no chat_id", "answer": "legacy-a"})).unwrap();
        crate::transcript::append_agent_chat_entry(&bundle_dir, json!({"chat_id": "c1", "scenario": "q2", "answer": "a2"})).unwrap();

        let turns = chat_turns_for(&bundle_dir, "c1").unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].scenario, "q1");
        assert_eq!(turns[1].scenario, "q2");
    }

    #[test]
    fn a_chats_first_turn_has_no_history_and_matches_run_agent_chat() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);

        let via_chat = run_chat_turn_with_transport(&bundle_dir, "c1", "auto", "confirm patient identity", false, "context", None, &sopkb_llm::UreqTransport).unwrap();
        let via_plain = sopkb_llm::UreqTransport;
        let direct = crate::run::run_agent_chat_with_transport(&bundle_dir, "auto", "confirm patient identity", false, "context", None, &via_plain).unwrap();

        assert_eq!(via_chat["answer"], direct["answer"]);
        assert_eq!(via_chat["chat_id"], json!("c1"));
    }

    #[test]
    #[serial]
    fn a_second_turn_in_the_same_chat_includes_the_first_turns_answer_in_the_outbound_request() {
        with_settings_path(|| {
            profile();
            let dir = tempdir().unwrap();
            let bundle_dir = dir.path().join("b");
            bundle_with_item(&bundle_dir);

            let first_transport = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "Confirm identity before proceeding."}"#.to_vec());
            let first = run_chat_turn_with_transport(&bundle_dir, "c1", "auto", "can staff proceed", false, "azure-llm", Some("p1"), &first_transport).unwrap();
            crate::transcript::append_agent_chat_entry_capped(&bundle_dir, first, CHAT_TRANSCRIPT_CAP).unwrap();

            let second_transport = sopkb_llm::MockTransport::ok(200, br#"{"output_text": "Yes, now proceed."}"#.to_vec());
            run_chat_turn_with_transport(&bundle_dir, "c1", "auto", "and after that?", false, "azure-llm", Some("p1"), &second_transport).unwrap();

            let request = second_transport.last_request().unwrap();
            let body: Value = serde_json::from_slice(&request.body).unwrap();
            let input = body["input"].as_str().unwrap();
            assert!(input.contains("Confirm identity before proceeding."), "the first turn's real answer must appear in the second turn's request: {input}");
            assert!(input.contains("can staff proceed"), "the first turn's own scenario text must appear too: {input}");
        });
    }

    #[test]
    fn unsupported_provider_still_errors() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);
        let err = run_chat_turn_with_transport(&bundle_dir, "c1", "auto", "s", false, "not-a-real-provider", None, &sopkb_llm::UreqTransport).unwrap_err();
        assert_eq!(err.to_string(), "unsupported agent provider: not-a-real-provider");
    }

    #[test]
    fn handle_chat_turn_persists_the_entry_with_its_chat_id() {
        let dir = tempdir().unwrap();
        let bundle_dir = dir.path().join("b");
        bundle_with_item(&bundle_dir);

        let entry = handle_chat_turn(&bundle_dir, "c1", "auto", "confirm patient identity", false, "context", None).unwrap();
        assert_eq!(entry["chat_id"], json!("c1"));

        let transcript = crate::transcript::read_agent_transcript(&bundle_dir).unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0]["chat_id"], json!("c1"));
    }
}
