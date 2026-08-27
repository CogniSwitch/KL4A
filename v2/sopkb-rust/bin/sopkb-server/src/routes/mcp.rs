//! Mirrors `desktop-tauri/src-tauri/src/commands/mcp.rs::get_mcp_invocation` only
//! (§4.10). The one-click client-configuration commands
//! (`list_mcp_client_targets`/`configure_mcp_client`) are NOT ported here, disclosed
//! as a gap: they configure MCP hosts installed on whatever machine runs the
//! desktop app, which is a meaningfully different question when the caller is a
//! browser talking to a possibly-remote server -- worth a deliberate design pass,
//! not a mechanical port.

use axum::extract::{Query, State};
use axum::Json;
use serde::Serialize;
use serde_json::Value;

use crate::error::ApiResult;
use crate::routes::bundles::KeyQuery;
use crate::state::resolve_bundle_dir;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct McpInvocation {
    pub command: String,
    pub args: Vec<String>,
    pub enable_review_notes_flag: String,
}

pub async fn get_mcp_invocation(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let invocation = McpInvocation {
        command: format!("sopkb-mcp{}", std::env::consts::EXE_SUFFIX),
        args: vec![bundle_dir.display().to_string()],
        enable_review_notes_flag: "--enable-review-notes".to_string(),
    };
    Ok(Json(serde_json::to_value(invocation).unwrap()))
}
