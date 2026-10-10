//! HTTP wrapper around the same shared `sopkb-*` library crates
//! `desktop-tauri/src-tauri`'s Tauri commands call -- a thin marshalling layer, no
//! domain logic, same philosophy as that crate's own `commands/*.rs` (see its
//! module doc comment). See `docs/port/CATCHUP_PLAN.md` for exact endpoint coverage
//! vs. the Tauri command surface, and why certain commands (native file/folder
//! pickers, `reveal_path`, one-click MCP client configuration) are deliberately not
//! ported here.

pub mod auth;
pub mod dto;
pub mod error;
pub mod events;
pub mod routes;
pub mod state;
pub mod static_files;
pub mod token;

use std::sync::Arc;

use sopkb_workbench::WorkbenchHandle;

use crate::events::EventBus;
use crate::state::AppState;

pub fn build_state(bundle_dir: Option<&std::path::Path>, token: String) -> AppState {
    AppState { workbench: Arc::new(WorkbenchHandle::launch(bundle_dir)), events: EventBus::new(), token: Arc::from(token.as_str()) }
}
