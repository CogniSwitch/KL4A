//! §4.6 Agent.

use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, State};

use crate::error::{AppError, CmdResult};
use crate::events::emit_agent_progress;
use crate::state::{resolve_bundle_dir, AppState};

#[tauri::command(rename_all = "snake_case")]
pub fn list_agent_tasks(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::context::agent_tasks(&bundle_dir).map_err(AppError::from)
}

/// The transcript is stored oldest-first, capped at the last 25 entries on write
/// (§4.6: "destroyed, not hidden"). `limit`, when given, keeps only the most recent
/// `limit` entries -- still oldest-first within that window -- mirroring how the
/// write-side cap itself behaves. Pure/testable independent of the file read.
pub(crate) fn apply_transcript_limit(entries: Vec<Value>, limit: Option<usize>) -> Vec<Value> {
    match limit {
        Some(n) if n < entries.len() => entries[entries.len() - n..].to_vec(),
        _ => entries,
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_agent_transcript(state: State<AppState>, limit: Option<usize>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let entries = sopkb_agent::read_agent_transcript(&bundle_dir).map_err(AppError::from)?;
    Ok(apply_transcript_limit(entries, limit))
}

fn default_task_id() -> String {
    "auto".to_string()
}

/// PORT_PLAN.md §4.6 `AgentRequest`. `allow_proposed_advisory` deliberately renamed
/// from the plan's `allow_proposed` per the plan's own instruction: it filters
/// nothing server-side (F6, D 1) -- embedded in the LLM payload, echoed back, and
/// referenced by one sentence of the built-in system prompt. The only hard
/// exclusion anywhere is `review_status == "rejected"`, gated by `include_rejected`,
/// which this path never forwards (`sopkb_agent::run_agent_chat` always passes
/// `false` for it internally) -- do not build a UI affordance implying this filters.
///
/// `chat_id` (round 6, item 15): generated client-side (`crypto.randomUUID()`) by
/// the Agent screen once per chat and sent on every turn in that chat -- this is
/// what lets `sopkb_agent::handle_chat_turn` fold that chat's own prior turns into
/// the outgoing request as real memory, and what a "list of chats" groups by. Web
/// mode's `sopkb-server` deliberately does NOT get this (its `AgentChatBody` has no
/// `chat_id` field, and Rust's serde ignores unknown JSON fields by default) --
/// scoped to the desktop app only, matching this round's other desktop-only changes
/// (e.g. the Export screen removal); `sopkb-cli`/`sopkb-server` keep calling the
/// original single-shot `handle_agent_chat`/`handle_react_chat` unchanged.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentRequest {
    pub scenario: String,
    #[serde(default = "default_task_id")]
    pub task_id: String,
    /// `"azure-llm"` | `"context"` | `"azure-llm-tools"`. `"context"` is zero-
    /// configuration -- no settings touched, no socket opened -- and should be the
    /// default in a fresh install. `"azure-llm-tools"` (`sopkb_agent::handle_react_chat`,
    /// routed here rather than inside `sopkb_agent::handle_agent_chat` itself -- see
    /// that function's own doc comment on why it stays untouched) is the same model
    /// call with a bounded tool-use back-and-forth first.
    pub provider: String,
    pub profile_id: Option<String>,
    pub allow_proposed_advisory: bool,
    pub chat_id: String,
    pub key: Option<String>,
}

/// The one non-trivial step here is resolving the bundle directory (needs `state`)
/// before handing off to a blocking task -- `sopkb_agent::handle_chat_turn` makes a
/// real (potentially slow, un-timeout-bounded on the `azure-llm`/`azure-llm-tools`
/// path) network call, so it runs on `spawn_blocking` rather than the async
/// command's own task, exactly like `run_ingest_pipeline` must for the same reason
/// (PORT_PLAN §4.3's rationale applies equally here: a synchronous command would
/// freeze the single-window app).
#[tauri::command(rename_all = "snake_case")]
pub async fn run_agent_chat(app: AppHandle, state: State<'_, AppState>, request: AgentRequest) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, request.key.as_deref())?;
    let AgentRequest { scenario, task_id, provider, profile_id, allow_proposed_advisory, chat_id, .. } = request;

    emit_agent_progress(&app, "started", format!("scenario={scenario} task={task_id} provider={provider}"));

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        sopkb_agent::handle_chat_turn(&bundle_dir, &chat_id, &task_id, &scenario, allow_proposed_advisory, &provider, profile_id.as_deref())
    })
    .await
    .map_err(|err| AppError::new("Io", format!("agent chat task did not complete: {err}")))?;

    match outcome {
        Ok(entry) => {
            emit_agent_progress(&app, "done", "agent chat completed");
            Ok(entry)
        }
        Err(err) => {
            let app_err = AppError::from(err);
            emit_agent_progress(&app, "failed", app_err.message.clone());
            Err(app_err)
        }
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_task_context(state: State<AppState>, task_id: String, include_rejected: bool, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::context::agent_context(&bundle_dir, &task_id, include_rejected).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_scenario_context(
    state: State<AppState>,
    scenario: String,
    task_id: Option<String>,
    include_rejected: bool,
    item_limit: Option<usize>,
    key: Option<String>,
) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    // 32 matches the chat path's own hard-wired limit (`sopkb_agent::run_agent_chat`
    // always calls `scenario_agent_context` with `item_limit = 32`) -- used here only
    // as this read command's own default when the caller doesn't specify one.
    let limit = item_limit.unwrap_or(32);
    sopkb_derive::context::scenario_agent_context(&bundle_dir, &scenario, task_id.as_deref(), include_rejected, limit)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn clear_agent_transcript(state: State<AppState>, key: Option<String>) -> CmdResult<()> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_core::store::write_state_json(&bundle_dir, "agent_chat.json", &serde_json::json!([])).map_err(AppError::from)
}

/// Deletes one chat's turns (round 6, item 15's per-chat "Delete" action) without
/// touching any other chat -- `clear_agent_transcript` above stays the separate,
/// more clearly-dangerous "wipe every chat" action. Returns the number of turns
/// removed so the UI can show e.g. "Deleted a chat with 3 turns" without a second
/// read; 0 for an unknown or already-empty `chat_id` is not an error.
#[tauri::command(rename_all = "snake_case")]
pub fn delete_agent_chat(state: State<AppState>, chat_id: String, key: Option<String>) -> CmdResult<usize> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_agent::delete_chat(&bundle_dir, &chat_id).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_transcript_limit_none_returns_everything() {
        let entries = vec![serde_json::json!(1), serde_json::json!(2), serde_json::json!(3)];
        assert_eq!(apply_transcript_limit(entries.clone(), None), entries);
    }

    #[test]
    fn apply_transcript_limit_keeps_last_n_oldest_first_within_window() {
        let entries = vec![serde_json::json!(1), serde_json::json!(2), serde_json::json!(3), serde_json::json!(4)];
        let limited = apply_transcript_limit(entries, Some(2));
        assert_eq!(limited, vec![serde_json::json!(3), serde_json::json!(4)]);
    }

    #[test]
    fn apply_transcript_limit_larger_than_len_returns_everything() {
        let entries = vec![serde_json::json!(1), serde_json::json!(2)];
        assert_eq!(apply_transcript_limit(entries.clone(), Some(10)), entries);
    }

    #[test]
    fn apply_transcript_limit_zero_returns_empty() {
        let entries = vec![serde_json::json!(1), serde_json::json!(2)];
        assert!(apply_transcript_limit(entries, Some(0)).is_empty());
    }

    #[test]
    fn agent_request_defaults_task_id_to_auto_when_absent() {
        let request: AgentRequest = serde_json::from_value(serde_json::json!({
            "scenario": "s",
            "provider": "context",
            "allow_proposed_advisory": false,
            "chat_id": "c1",
        }))
        .unwrap();
        assert_eq!(request.task_id, "auto");
    }
}
