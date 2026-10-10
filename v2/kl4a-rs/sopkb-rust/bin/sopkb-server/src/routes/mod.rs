pub mod agent;
pub mod bundles;
pub mod export;
pub mod health;
pub mod ingest;
pub mod mcp;
pub mod reads;
pub mod relations;
pub mod review;
pub mod settings;

use axum::middleware;
use axum::routing::{get, patch, post};
use axum::Router;

use crate::auth::require_bearer_token;
use crate::events::events_handler;
use crate::state::AppState;

/// The authenticated API surface, mounted under `/api`. Everything except
/// `/health` (mounted separately, see `build_router`) requires a bearer token.
fn api_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/context", get(bundles::get_context))
        .route("/workbench-root", post(bundles::set_workbench_root))
        .route("/bundles", get(bundles::list_bundles).post(bundles::create_project))
        .route("/bundles/describe", get(bundles::describe_bundle))
        .route("/bundles/:key/select", post(bundles::select_bundle))
        .route("/bundles/:key", axum::routing::delete(bundles::delete_bundle))
        .route("/bundles/deselect", post(bundles::deselect_bundle))
        .route("/sources", get(reads::list_sources))
        .route("/sources/upload", post(bundles::upload_sources))
        .route("/sources/staged", get(ingest::get_staged_sources).delete(ingest::clear_staged_sources))
        .route("/sources/:source_id", get(reads::get_source))
        .route("/sources/:source_id/normalized-text", get(reads::get_normalized_text))
        .route("/source-stats", get(reads::get_source_stats))
        .route("/sections", get(reads::list_sections))
        .route("/sections/:section_id", get(reads::get_section))
        .route("/knowledge", get(reads::list_knowledge_items))
        .route("/knowledge/search", get(reads::search_knowledge))
        .route("/knowledge/:item_id", get(reads::get_knowledge_item))
        .route("/evidence/:item_id", get(reads::get_evidence))
        .route("/citations/:citation_id", get(reads::resolve_citation))
        .route("/conflicts-report", get(reads::get_conflicts_report))
        .route("/freshness-report", get(reads::get_freshness_report))
        .route("/agent-guide", get(reads::get_agent_guide))
        .route("/validation-summary", get(reads::get_validation_summary))
        .route("/concepts", get(reads::get_concept_index))
        .route("/concepts/:concept_id", get(reads::get_concept_detail))
        .route("/reports", get(reads::get_reports))
        .route("/review/:item_id", get(reads::get_review_detail))
        .route("/review/:item_id/events", get(review::list_review_events))
        .route("/review/:item_id/approve", post(review::approve_item))
        .route("/review/:item_id/reject", post(review::reject_item))
        .route("/review/:item_id/defer", post(review::defer_item))
        .route("/review/:item_id/comment", post(review::comment_item))
        .route("/review/:item_id/edit", patch(review::edit_item))
        .route("/relations/search", get(relations::search_relations))
        .route("/relations/:node_id/neighborhood", get(relations::get_relation_neighborhood))
        .route("/agent/tasks", get(agent::list_agent_tasks))
        .route("/agent/transcript", get(agent::get_agent_transcript).delete(agent::clear_agent_transcript))
        .route("/agent/chat", post(agent::run_agent_chat))
        .route("/agent/task-context", get(agent::get_task_context))
        .route("/agent/scenario-context", get(agent::get_scenario_context))
        .route("/ingest/scan", post(ingest::scan_sources))
        .route("/ingest/normalize", post(ingest::normalize_sources))
        .route("/ingest/mine", post(ingest::mine_knowledge))
        .route("/ingest/validate", post(review::validate_bundle))
        .route("/ingest/preview", post(ingest::preview_ingest_pipeline))
        .route("/ingest/run", post(ingest::run_ingest_pipeline))
        .route("/export", post(export::export_bundle))
        .route("/export/sync", post(export::sync_okf_documents))
        .route("/export/dir", get(export::get_export_dir))
        .route("/settings", get(settings::get_settings))
        .route("/settings/default-prompts", get(settings::get_default_prompts))
        .route("/mcp/invocation", get(mcp::get_mcp_invocation))
        .route("/events", get(events_handler))
        .route_layer(middleware::from_fn_with_state(state, require_bearer_token))
}

pub fn build_router(state: AppState) -> Router {
    Router::new().route("/health", get(health::health)).nest("/api", api_router(state.clone())).with_state(state)
}
