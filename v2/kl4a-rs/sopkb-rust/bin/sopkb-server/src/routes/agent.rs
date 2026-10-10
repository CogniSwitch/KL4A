//! Mirrors `desktop-tauri/src-tauri/src/commands/agent.rs` (§4.6). Runs on
//! `tokio::task::spawn_blocking` for the same reason the Tauri command does: the
//! `azure-llm`/`azure-llm-tools` paths make a real, potentially slow, un-timeout-
//! bounded network call, and axum's own worker threads must not block on it.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::events::EventBus;
use crate::routes::bundles::KeyQuery;
use crate::state::{resolve_bundle_dir, AppState};

pub async fn list_agent_tasks(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(serde_json::json!(sopkb_derive::context::agent_tasks(&bundle_dir)?)))
}

#[derive(Debug, Deserialize)]
pub struct TranscriptQuery {
    pub limit: Option<usize>,
    pub key: Option<String>,
}

/// Same oldest-first-window slicing as `desktop-tauri`'s `apply_transcript_limit`.
fn apply_transcript_limit(entries: Vec<Value>, limit: Option<usize>) -> Vec<Value> {
    match limit {
        Some(n) if n < entries.len() => entries[entries.len() - n..].to_vec(),
        _ => entries,
    }
}

pub async fn get_agent_transcript(State(state): State<AppState>, Query(q): Query<TranscriptQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let entries = sopkb_agent::read_agent_transcript(&bundle_dir)?;
    Ok(Json(serde_json::json!(apply_transcript_limit(entries, q.limit))))
}

pub async fn clear_agent_transcript(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    sopkb_core::store::write_state_json(&bundle_dir, "agent_chat.json", &serde_json::json!([]))?;
    Ok(Json(serde_json::json!({"ok": true})))
}

fn default_task_id() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize)]
pub struct AgentChatBody {
    pub scenario: String,
    #[serde(default = "default_task_id")]
    pub task_id: String,
    pub provider: String,
    pub profile_id: Option<String>,
    pub allow_proposed_advisory: bool,
    pub key: Option<String>,
}

pub async fn run_agent_chat(State(state): State<AppState>, Json(body): Json<AgentChatBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let AgentChatBody { scenario, task_id, provider, profile_id, allow_proposed_advisory, .. } = body;
    let events = state.events.clone();
    events.agent_progress("started", format!("scenario={scenario} task={task_id} provider={provider}"));

    let outcome = tokio::task::spawn_blocking(move || {
        if provider == "azure-llm-tools" {
            sopkb_agent::handle_react_chat(&bundle_dir, &task_id, &scenario, allow_proposed_advisory, profile_id.as_deref())
        } else {
            sopkb_agent::handle_agent_chat(&bundle_dir, &task_id, &scenario, allow_proposed_advisory, &provider, profile_id.as_deref())
        }
    })
    .await
    .map_err(|err| ApiError::new("Io", format!("agent chat task did not complete: {err}")))?;

    finish(&events, outcome)
}

fn finish(events: &EventBus, outcome: Result<Value, sopkb_core::error::SopkbError>) -> ApiResult<Json<Value>> {
    match outcome {
        Ok(entry) => {
            events.agent_progress("done", "agent chat completed");
            Ok(Json(entry))
        }
        Err(err) => {
            let api_err = ApiError::from(err);
            events.agent_progress("failed", api_err.message.clone());
            Err(api_err)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskContextQuery {
    pub task_id: String,
    #[serde(default)]
    pub include_rejected: bool,
    pub key: Option<String>,
}

pub async fn get_task_context(State(state): State<AppState>, Query(q): Query<TaskContextQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::context::agent_context(&bundle_dir, &q.task_id, q.include_rejected)?))
}

#[derive(Debug, Deserialize)]
pub struct ScenarioContextQuery {
    pub scenario: String,
    pub task_id: Option<String>,
    #[serde(default)]
    pub include_rejected: bool,
    pub item_limit: Option<usize>,
    pub key: Option<String>,
}

pub async fn get_scenario_context(State(state): State<AppState>, Query(q): Query<ScenarioContextQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let limit = q.item_limit.unwrap_or(32);
    Ok(Json(sopkb_derive::context::scenario_agent_context(&bundle_dir, &q.scenario, q.task_id.as_deref(), q.include_rejected, limit)?))
}
