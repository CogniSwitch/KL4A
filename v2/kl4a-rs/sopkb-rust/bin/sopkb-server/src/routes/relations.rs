//! Mirrors `desktop-tauri/src-tauri/src/commands/relations.rs` (§4.9).

use axum::extract::{Path as AxPath, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiResult;
use crate::routes::bundles::KeyQuery;
use crate::state::{resolve_bundle_dir, AppState};

#[derive(Debug, Deserialize, Default)]
pub struct RelationsSearchQuery {
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub predicate: String,
    #[serde(default)]
    pub object: String,
    pub key: Option<String>,
}

pub async fn search_relations(State(state): State<AppState>, Query(q): Query<RelationsSearchQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let results = sopkb_derive::relations::relations_search(&bundle_dir, &q.subject, &q.predicate, &q.object)?;
    Ok(Json(json!(results)))
}

pub async fn get_relation_neighborhood(State(state): State<AppState>, AxPath(node_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::relations::relations_neighborhood(&bundle_dir, &node_id)?))
}
