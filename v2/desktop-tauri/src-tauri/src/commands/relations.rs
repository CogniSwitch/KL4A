//! §4.9 Relations.

use serde_json::Value;
use tauri::State;

use crate::error::{AppError, CmdResult};
use crate::state::{resolve_bundle_dir, AppState};

#[tauri::command(rename_all = "snake_case")]
pub fn search_relations(
    state: State<AppState>,
    subject: Option<String>,
    predicate: Option<String>,
    object: Option<String>,
    key: Option<String>,
) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::relations::relations_search(
        &bundle_dir,
        subject.as_deref().unwrap_or(""),
        predicate.as_deref().unwrap_or(""),
        object.as_deref().unwrap_or(""),
    )
    .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_relation_neighborhood(state: State<AppState>, node_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::relations::relations_neighborhood(&bundle_dir, &node_id).map_err(AppError::from)
}
