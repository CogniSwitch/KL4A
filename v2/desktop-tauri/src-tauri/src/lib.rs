// `pub` (not just crate-visible): this crate's `[lib] crate-type` includes `rlib`,
// and `v2/desktop-ui2` depends on it directly to re-register the same underlying
// commands/state/DTOs rather than reimplementing ~80 handlers a second time --
// see that crate's own doc comment. No behavior change, visibility only.
pub mod commands;
pub mod dialogs;
pub mod dto;
pub mod error;
pub mod events;
mod startup_log;
pub mod state;

use std::path::PathBuf;

use commands::mcp::McpDetectionCache;
use state::AppState;

/// Optional starting directory: first CLI argument, else `SOPKB_BUNDLE_DIR`. `None`
/// defers to `WorkbenchHandle::launch`'s own silent-fallback default (`~/SOP
/// Knowledge Workbench`).
fn initial_bundle_dir() -> Option<PathBuf> {
    std::env::args_os().nth(1).filter(|arg| !arg.is_empty()).or_else(|| std::env::var_os("SOPKB_BUNDLE_DIR")).map(PathBuf::from)
}

/// Diagnostic-only ping: the frontend calls this once, fire-and-forget, right after
/// mounting (see `frontend/src/main.tsx`). Not part of the PORT_PLAN.md §4 command
/// surface -- it exists solely as startup-log checkpoint 3 (see `startup_log.rs`),
/// proving the webview didn't just get created but actually finished bootstrapping
/// its embedded JS.
#[tauri::command(rename_all = "snake_case")]
fn frontend_ready() {
    startup_log::log("frontend_ready received (webview finished bootstrapping JS)");
}

/// Mobile entry point (Android JNI / iOS): the `#[cfg_attr(mobile, ...)]` attribute is
/// a no-op on desktop builds, where `main.rs` just calls this directly instead.
/// `tauri-build` (via `build.rs`'s `tauri_build::try_build`) sets the `desktop`/`mobile`
/// cfg flags automatically based on the compile target -- no extra wiring needed here.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup_log::log("main() start");
    let bundle_dir = initial_bundle_dir();
    startup_log::log(&format!("initial_bundle_dir = {bundle_dir:?}"));

    startup_log::log("AppState::launch() starting");
    let state = AppState::launch(bundle_dir.as_deref());
    startup_log::log("AppState::launch() done");

    startup_log::log("building tauri::Builder, about to call .run()");
    let mut builder = tauri::Builder::default();

    // `tauri-plugin-dialog`'s folder/file pickers and `tauri-plugin-opener`'s
    // reveal-in-file-manager have no mobile equivalent (no general directory-chooser
    // on Android/iOS, no Finder/Explorer to reveal into) -- registered on desktop
    // only. The 4 commands that depend on them (pick_workbench_folder,
    // pick_source_files, pick_source_folder, reveal_path) stay in the handler list
    // below unconditionally for now; on mobile they'll runtime-error if invoked,
    // an accepted gap for this feasibility spike (see docs/port/MOBILE_FEASIBILITY.md)
    // rather than a real mobile UX, which is a product decision, not this spike's job.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init());
    }

    builder
        .manage(state)
        .manage(McpDetectionCache::default())
        .setup(|app| {
            startup_log::log("setup() hook invoked (event loop started, window/webview being created)");
            // Fire-and-forget: detection runs in the background and populates
            // McpDetectionCache once, so Settings never re-probes it on mount
            // (see mcp.rs's own doc comment on McpDetectionCache). A failure here
            // is logged by the task itself and never propagates -- it must not be
            // allowed to block or fail the rest of startup.
            commands::mcp::spawn_mcp_detection_startup_task(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            frontend_ready,
            // §4.1 Workbench context and lifecycle
            commands::context::get_workbench_context,
            commands::context::resolve_launch_target,
            commands::context::set_workbench_root,
            commands::context::pick_workbench_folder,
            commands::context::select_bundle,
            commands::context::deselect_bundle,
            // §4.2 Bundle management
            commands::bundles::list_bundles,
            commands::bundles::create_project,
            commands::bundles::describe_bundle,
            commands::bundles::init_bundle,
            commands::bundles::delete_bundle,
            commands::bundles::repair_bundle_dirs,
            commands::bundles::get_legacy_state_status,
            commands::bundles::get_bundle_prompt_overrides,
            commands::bundles::set_bundle_prompt_overrides,
            // §4.3 Ingest pipeline
            commands::ingest::pick_source_files,
            commands::ingest::pick_source_folder,
            commands::ingest::stage_source_files,
            commands::ingest::get_staged_sources,
            commands::ingest::clear_staged_sources,
            commands::ingest::remove_staged_source,
            commands::ingest::run_ingest_pipeline,
            commands::ingest::cancel_ingest,
            commands::ingest::preview_ingest_pipeline,
            commands::ingest::scan_sources,
            commands::ingest::normalize_sources,
            commands::ingest::mine_knowledge,
            commands::ingest::validate_bundle,
            commands::ingest::get_last_ingest_run,
            commands::ingest::retire_source,
            // §4.4 Read / query
            commands::reads::list_sources,
            commands::reads::get_source,
            commands::reads::get_normalized_text,
            commands::reads::get_source_stats,
            commands::reads::list_sections,
            commands::reads::get_section,
            commands::reads::list_knowledge_items,
            commands::reads::get_knowledge_item,
            commands::reads::search_knowledge,
            commands::reads::get_evidence,
            commands::reads::resolve_citation,
            commands::reads::get_concept_index,
            commands::reads::get_concept_detail,
            commands::reads::get_review_detail,
            commands::reads::get_graph,
            commands::reads::get_reports,
            commands::reads::get_validation_summary,
            commands::reads::get_conflicts_report,
            commands::reads::get_freshness_report,
            commands::reads::get_agent_guide,
            commands::reads::list_authored_drafts,
            commands::reads::get_authored_draft,
            // §4.5 Review
            commands::review::approve_item,
            commands::review::reject_item,
            commands::review::defer_item,
            commands::review::comment_item,
            commands::review::edit_item,
            commands::review::list_review_events,
            // §4.6 Agent
            commands::agent::list_agent_tasks,
            commands::agent::get_agent_transcript,
            commands::agent::run_agent_chat,
            commands::agent::get_task_context,
            commands::agent::get_scenario_context,
            commands::agent::clear_agent_transcript,
            commands::agent::delete_agent_chat,
            // §4.7 Export
            commands::export::sync_okf_documents,
            commands::export::get_export_dir,
            commands::export::reveal_path,
            // §4.8 Settings
            commands::settings::get_settings,
            commands::settings::get_default_prompts,
            commands::settings::save_profile,
            commands::settings::delete_profile,
            commands::settings::set_default_profile,
            commands::settings::set_reviewer_name,
            commands::settings::set_max_parallel_workers,
            commands::settings::test_profile_connection,
            // §4.9 Relations
            commands::relations::search_relations,
            commands::relations::get_relation_neighborhood,
            // §4.10 MCP
            commands::mcp::get_mcp_invocation,
            // One-click MCP client configuration -- no Python equivalent, see mcp.rs's own doc comment.
            commands::mcp::list_mcp_client_targets,
            commands::mcp::rescan_mcp_client_targets,
            commands::mcp::configure_mcp_client,
            // Task #38: diagnostics export -- no Python equivalent, see the module's own doc comment.
            commands::diagnostics::export_diagnostics_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the SOP Knowledge Workbench shell");
}
