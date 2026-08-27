//! §4.1 Workbench context and lifecycle.

use tauri::{AppHandle, Emitter, State};

use crate::dialogs::pick_folder;
use crate::dto::{WireBundleSummary, WireResolvedLaunchTarget, WireWorkbenchContext};
use crate::error::{AppError, CmdResult};
use crate::state::AppState;

#[tauri::command(rename_all = "snake_case")]
pub fn get_workbench_context(state: State<AppState>) -> WireWorkbenchContext {
    state.context().into()
}

#[tauri::command(rename_all = "snake_case")]
pub fn resolve_launch_target(path: Option<String>) -> WireResolvedLaunchTarget {
    sopkb_workbench::resolve_launch_target(path.as_deref().map(std::path::Path::new)).into()
}

#[tauri::command(rename_all = "snake_case")]
pub fn set_workbench_root(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<WireWorkbenchContext> {
    let ctx = state.set_workbench_root(std::path::Path::new(&path)).map_err(AppError::from)?;
    let wire: WireWorkbenchContext = ctx.into();
    let _ = app.emit("workbench://context-changed", &wire);
    Ok(wire)
}

/// Native folder picker. `None` = the user cancelled, a normal outcome (PORT_PLAN.md
/// §4.1) -- never surfaced as an error.
#[tauri::command(rename_all = "snake_case")]
pub async fn pick_workbench_folder(app: AppHandle) -> CmdResult<Option<String>> {
    pick_folder(&app).await
}

#[tauri::command(rename_all = "snake_case")]
pub fn select_bundle(app: AppHandle, state: State<AppState>, key: String) -> CmdResult<WireBundleSummary> {
    let ctx = state.select_bundle(&key).map_err(AppError::from)?;
    let _ = app.emit("workbench://context-changed", &WireWorkbenchContext::from(ctx.clone()));
    let bundle_dir = sopkb_workbench::bundle_dir_for_key(&ctx.root, &key).map_err(AppError::from)?;
    let summary = sopkb_workbench::describe_bundle(&bundle_dir).map_err(AppError::from)?;
    Ok(summary.into())
}

#[tauri::command(rename_all = "snake_case")]
pub fn deselect_bundle(app: AppHandle, state: State<AppState>) -> WireWorkbenchContext {
    let ctx = state.deselect_bundle();
    let wire: WireWorkbenchContext = ctx.into();
    let _ = app.emit("workbench://context-changed", &wire);
    wire
}
