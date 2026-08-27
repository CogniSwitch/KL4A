//! Shared native-dialog helpers. Every picker command in §4.1/§4.3
//! (`pick_workbench_folder`, `pick_source_files`, `pick_source_folder`) needs the
//! same "drive a non-blocking callback-based dialog from an async command, `None`/
//! empty on cancel is a normal outcome, never an error" shape -- factored once here
//! rather than duplicated per command.

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, CmdResult};

fn home_start_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().home_dir().ok()
}

/// Native single-folder picker. `None` = the user cancelled -- a normal outcome,
/// never surfaced as an error.
///
/// Desktop-only: `tauri_plugin_dialog::FileDialogBuilder::pick_folder` itself is
/// `#[cfg(desktop)]` in the plugin (confirmed by reading its source -- there is no
/// general arbitrary-directory-chooser on Android/iOS the way desktop platforms have
/// one), so calling it unconditionally fails to *compile* on mobile targets, not just
/// fails at runtime. That's a real difference from `reveal_path`'s
/// `tauri_plugin_opener::reveal_item_in_dir`, which compiles everywhere and only
/// returns `Err` at runtime on mobile -- don't assume every desktop-only plugin API
/// degrades the same way without checking.
#[cfg(desktop)]
pub async fn pick_folder(app: &AppHandle) -> CmdResult<Option<String>> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let mut dialog = app.dialog().file();
    if let Some(dir) = home_start_dir(app) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_folder(move |choice| {
        let _ = tx.try_send(choice);
    });
    let choice = rx.recv().await.ok_or_else(|| AppError::new("Io", "the folder picker closed unexpectedly"))?;
    let Some(selected) = choice else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|err| AppError::invalid_input(format!("that selection is not a local folder: {err}")))?;
    Ok(Some(path.display().to_string()))
}

/// Mobile stand-in: same signature as the desktop implementation above so
/// `pick_workbench_folder`/`pick_source_folder` don't need per-platform command
/// bodies, but always errors -- there is nothing to call.
#[cfg(mobile)]
pub async fn pick_folder(_app: &AppHandle) -> CmdResult<Option<String>> {
    Err(AppError::new("Io", "picking an arbitrary folder is not supported on mobile"))
}

/// Native "save as" picker, pre-filled with `default_name`. `None` = the user
/// cancelled -- a normal outcome, never surfaced as an error. Same channel-based
/// shape as `pick_folder`/`pick_files` above (the plugin's dialog callbacks are not
/// themselves `async`).
pub async fn pick_save_file(app: &AppHandle, default_name: &str, filter_name: &str, extensions: &[&str]) -> CmdResult<Option<String>> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let mut dialog = app.dialog().file().add_filter(filter_name, extensions).set_file_name(default_name);
    if let Some(dir) = home_start_dir(app) {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file(move |choice| {
        let _ = tx.try_send(choice);
    });
    let choice = rx.recv().await.ok_or_else(|| AppError::new("Io", "the save dialog closed unexpectedly"))?;
    let Some(selected) = choice else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|err| AppError::invalid_input(format!("that selection is not a local path: {err}")))?;
    Ok(Some(path.display().to_string()))
}

/// Native multi-file picker, filtered to `extensions`. An empty result means the
/// user cancelled or selected nothing -- also a normal outcome, never an error.
pub async fn pick_files(app: &AppHandle, filter_name: &str, extensions: &[&str]) -> CmdResult<Vec<String>> {
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let mut dialog = app.dialog().file().add_filter(filter_name, extensions);
    if let Some(dir) = home_start_dir(app) {
        dialog = dialog.set_directory(dir);
    }
    dialog.pick_files(move |choice| {
        let _ = tx.try_send(choice);
    });
    let choice = rx.recv().await.ok_or_else(|| AppError::new("Io", "the file picker closed unexpectedly"))?;
    let Some(selected) = choice else {
        return Ok(Vec::new());
    };
    let mut paths = Vec::with_capacity(selected.len());
    for file_path in selected {
        let path = file_path.into_path().map_err(|err| AppError::invalid_input(format!("that selection is not a local file: {err}")))?;
        paths.push(path.display().to_string());
    }
    Ok(paths)
}
