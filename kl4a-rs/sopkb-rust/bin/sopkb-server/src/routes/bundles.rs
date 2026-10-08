//! Mirrors `desktop-tauri/src-tauri/src/commands/{bundles,context}.rs`'s bundle-
//! management surface (§4.1/§4.2).

use axum::extract::{Multipart, Path as AxPath, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::dto::{bundle_card_json, bundle_summary_json, workbench_context_json};
use crate::error::{ApiError, ApiResult};
use crate::state::{resolve_bundle_dir, AppState};

#[derive(Debug, Deserialize, Default)]
pub struct KeyQuery {
    pub key: Option<String>,
}

pub async fn get_context(State(state): State<AppState>) -> Json<Value> {
    Json(workbench_context_json(&state.workbench.context()))
}

pub async fn list_bundles(State(state): State<AppState>) -> Json<Value> {
    let ctx = state.workbench.context();
    let cards: Vec<Value> = sopkb_workbench::list_bundles(&ctx.root).iter().map(bundle_card_json).collect();
    Json(json!(cards))
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectBody {
    pub title: String,
}

pub async fn create_project(State(state): State<AppState>, Json(body): Json<CreateProjectBody>) -> ApiResult<Json<Value>> {
    let ctx = state.workbench.context();
    let _guard = state.workbench.begin_mutation();
    let result = sopkb_workbench::create_project(&ctx.root, &body.title)?;
    state.events.bundle_index_changed(&ctx.bundles_root.display().to_string());
    Ok(Json(json!({ "card": bundle_card_json(&result.card), "already_existed": result.already_existed })))
}

pub async fn describe_bundle(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let summary = sopkb_workbench::describe_bundle(&bundle_dir)?;
    Ok(Json(bundle_summary_json(&summary)))
}

pub async fn select_bundle(State(state): State<AppState>, AxPath(key): AxPath<String>) -> ApiResult<Json<Value>> {
    let ctx = state.workbench.select_bundle(&key)?;
    Ok(Json(workbench_context_json(&ctx)))
}

pub async fn deselect_bundle(State(state): State<AppState>) -> Json<Value> {
    Json(workbench_context_json(&state.workbench.deselect_bundle()))
}

/// Permanently deletes a bundle directory -- irreversible, the frontend must
/// confirm with the user before calling this (same convention as the Tauri
/// sibling command).
pub async fn delete_bundle(State(state): State<AppState>, AxPath(key): AxPath<String>) -> ApiResult<Json<Value>> {
    let ctx = state.workbench.context();
    let _guard = state.workbench.begin_mutation();
    sopkb_workbench::delete_bundle(&ctx.root, &key)?;
    if ctx.selected_bundle.as_deref() == Some(key.as_str()) {
        state.workbench.deselect_bundle();
    }
    state.events.bundle_index_changed(&ctx.bundles_root.display().to_string());
    Ok(Json(json!({"ok": true})))
}

#[derive(Debug, Deserialize)]
pub struct SetRootBody {
    pub path: String,
}

pub async fn set_workbench_root(State(state): State<AppState>, Json(body): Json<SetRootBody>) -> ApiResult<Json<Value>> {
    let ctx = state.workbench.set_workbench_root(std::path::Path::new(&body.path))?;
    Ok(Json(workbench_context_json(&ctx)))
}

/// Web-mode replacement for the native folder/file pickers (`pick_source_files`/
/// `pick_source_folder`/`init_bundle` from an arbitrary local path have no browser
/// equivalent -- see this crate's own coverage notes): `multipart/form-data` upload
/// into the bundle's staging area, then the same `stage_uploaded_files_guarded` path
/// the desktop app's `stage_source_files` command uses.
pub async fn upload_sources(State(state): State<AppState>, Query(q): Query<KeyQuery>, mut multipart: Multipart) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let mut uploads = Vec::new();
    let tmp_dir = std::env::temp_dir().join(format!("sopkb-upload-{}", uuid_like()));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| ApiError::new("Io", e.to_string()))?;

    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::invalid_input(e.to_string()))? {
        let relative_name = field.file_name().unwrap_or("upload.bin").to_string();
        let bytes = field.bytes().await.map_err(|e| ApiError::invalid_input(e.to_string()))?;
        let tmp_path = tmp_dir.join(sanitize_filename(&relative_name));
        std::fs::write(&tmp_path, &bytes).map_err(|e| ApiError::new("Io", e.to_string()))?;
        uploads.push(sopkb_workbench::UploadSource { path: tmp_path, relative_name });
    }

    let staged = state.workbench.stage_uploaded_files_guarded(&bundle_dir, &uploads, true)?;
    let _ = std::fs::remove_dir_all(&tmp_dir);
    Ok(Json(json!({ "staging_dir": staged.staging_dir.display().to_string(), "file_count": staged.file_count })))
}

fn sanitize_filename(name: &str) -> String {
    name.rsplit(['/', '\\']).next().unwrap_or(name).to_string()
}

fn uuid_like() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
