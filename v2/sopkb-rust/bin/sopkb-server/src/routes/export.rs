//! Mirrors `desktop-tauri/src-tauri/src/commands/export.rs` (§4.7). `reveal_path`
//! has no meaning on a server (there is no local file manager to reveal into on
//! whatever machine is running the browser) -- deliberately NOT implemented; the
//! web frontend hides that button entirely in web mode instead (see its own notes).

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::error::ApiResult;
use crate::routes::bundles::KeyQuery;
use crate::state::{bundle_key_of, resolve_bundle_dir, AppState};

#[derive(Debug, Deserialize)]
pub struct ExportBody {
    pub formats: Vec<String>,
    pub key: Option<String>,
}

pub async fn export_bundle(State(state): State<AppState>, Json(body): Json<ExportBody>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, body.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();
    let artifacts = sopkb_export::export_bundle(&bundle_dir, &body.formats)?;
    state.events.bundle_state_changed(&bundle_key, &["exports", "okf"]);
    Ok(Json(serde_json::json!(artifacts)))
}

pub async fn sync_okf_documents(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let bundle_key = bundle_key_of(&bundle_dir);
    let _guard = state.workbench.begin_mutation();
    let result = sopkb_export::sync_okf_bundle(&bundle_dir)?;
    state.events.bundle_state_changed(&bundle_key, &["okf"]);
    Ok(Json(result))
}

pub async fn get_export_dir(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<String> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(sopkb_export::default_export_dir(&bundle_dir)?.display().to_string())
}
