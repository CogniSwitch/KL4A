//! Mirrors `desktop-tauri/src-tauri/src/commands/review.rs` (§4.5). Each action
//! (approve/reject/defer/comment/edit) runs on `spawn_blocking` -- the Tauri sibling
//! converted these from sync to async+spawn_blocking specifically because a full
//! `sync_okf_bundle` resync running on the request thread was observed to hang the
//! UI (see `docs/port/CATCHUP_PLAN.md`); the same blocking-I/O concern applies here.

use axum::extract::{Path as AxPath, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::routes::bundles::KeyQuery;
use crate::state::{bundle_key_of, resolve_bundle_dir, AppState};

const REVIEW_SCOPE: &[&str] = &["items", "reviews", "okf"];

#[derive(Debug, Deserialize)]
pub struct ReviewActionBody {
    pub reviewer: String,
    pub rationale: String,
    pub key: Option<String>,
}

async fn run_action(
    state: AppState,
    item_id: String,
    key: Option<String>,
    action: impl FnOnce(&std::path::Path, &str, &str, &str) -> Result<Value, sopkb_core::error::SopkbError> + Send + 'static,
    reviewer: String,
    rationale: String,
) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();

    let result = tokio::task::spawn_blocking(move || action(&bundle_dir, &item_id, &reviewer, &rationale))
        .await
        .map_err(|err| ApiError::new("Io", format!("review action task did not complete: {err}")))??;

    state.events.bundle_state_changed(&bundle_key, REVIEW_SCOPE);
    Ok(Json(result))
}

pub async fn approve_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Json(body): Json<ReviewActionBody>) -> ApiResult<Json<Value>> {
    run_action(state, item_id, body.key, sopkb_review::approve_item, body.reviewer, body.rationale).await
}

pub async fn reject_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Json(body): Json<ReviewActionBody>) -> ApiResult<Json<Value>> {
    run_action(state, item_id, body.key, sopkb_review::reject_item, body.reviewer, body.rationale).await
}

pub async fn defer_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Json(body): Json<ReviewActionBody>) -> ApiResult<Json<Value>> {
    run_action(state, item_id, body.key, sopkb_review::defer_item, body.reviewer, body.rationale).await
}

pub async fn comment_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Json(body): Json<ReviewActionBody>) -> ApiResult<Json<Value>> {
    run_action(state, item_id, body.key, sopkb_review::comment_item, body.reviewer, body.rationale).await
}

#[derive(Debug, Deserialize)]
pub struct EditItemBody {
    pub field: String,
    pub value: String,
    pub reviewer: String,
    pub rationale: String,
    pub key: Option<String>,
}

pub async fn edit_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Json(body): Json<EditItemBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();

    let EditItemBody { field, value, reviewer, rationale, .. } = body;
    let result = tokio::task::spawn_blocking(move || sopkb_review::edit_item(&bundle_dir, &item_id, &field, &value, &reviewer, &rationale))
        .await
        .map_err(|err| ApiError::new("Io", format!("edit task did not complete: {err}")))??;

    state.events.bundle_state_changed(&bundle_key, REVIEW_SCOPE);
    Ok(Json(result))
}

pub async fn list_review_events(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let events = sopkb_core::store::read_state_json(&bundle_dir, "reviews.json", serde_json::json!([]))?;
    let filtered: Vec<Value> = events
        .as_array()
        .map(|arr| arr.iter().filter(|e| e.get("knowledge_item_id").and_then(|v| v.as_str()) == Some(item_id.as_str())).cloned().collect())
        .unwrap_or_default();
    Ok(Json(serde_json::json!(filtered)))
}

pub async fn validate_bundle(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();

    state.events.ingest_progress("validate", "started", "");
    let bundle_dir_for_task = bundle_dir.clone();
    let outcome = tokio::task::spawn_blocking(move || sopkb_review::with_bundle_lock(&bundle_dir_for_task, || sopkb_review::validate_bundle(&bundle_dir_for_task)))
        .await
        .map_err(|err| ApiError::new("Io", format!("validate task did not complete: {err}")))?;

    let (errors, warnings) = match outcome {
        Ok(pair) => pair,
        Err(e) => {
            state.events.ingest_progress("validate", "failed", e.to_string());
            return Err(ApiError::from(e));
        }
    };
    state.events.ingest_progress("validate", "done", format!("{} errors, {} warnings", errors.len(), warnings.len()));
    state.events.bundle_state_changed(&bundle_key, &["okf"]);
    Ok(Json(serde_json::json!({ "errors": errors, "warnings": warnings })))
}
