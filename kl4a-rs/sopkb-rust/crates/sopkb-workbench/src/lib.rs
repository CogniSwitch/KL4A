//! Non-presentation half of web_app.py: WorkbenchContext, bundle listing, upload staging, ingest pipeline orchestration, concept index.
//! Preserves the workbench-root/single-bundle dual mode internally (docs/port/DECISIONS.md Q2).
//! Upload staging: replace=true default, folder picks skip staging (docs/port/DECISIONS.md Q8).
//! See docs/port/PORT_PLAN.md §3.1 (sopkb-workbench) and §6.10 (Phase 9).
//!
//! **Scope note (Phase 9 split):** this crate is the pure-Rust orchestration library
//! only. It deliberately exposes plain functions over plain types (paths, strings, the
//! structs defined here) and nothing Tauri-specific, per PORT_PLAN.md §3.1's own
//! instruction for the future command layer: "Thin: each command body is argument
//! marshalling + one call into sopkb-workbench + error mapping. NO domain logic here."
//! Wiring an actual `desktop-tauri/src-tauri` command layer on top of this crate is the
//! natural next step but is explicitly out of scope here.

pub mod bundles;
pub mod concepts;
pub mod context;
pub mod heading_restructure;
pub mod ingest;
pub mod paths;
pub mod upload;

pub use bundles::{bundle_dir_for_key, create_project, delete_bundle, describe_bundle, init_bundle, is_bundle_dir, knowledge_bundles_root, list_bundle_dirs, list_bundles, BundleCard, BundleKey, BundleSummary, CreateProjectResult};
pub use concepts::{concept_index, default_graph_concept, focused_graph_for_concept, get_concept_detail, ConceptDetail, ConceptSummary, Graph, GraphEdge, GraphNode};
pub use context::{build_context, default_workbench_dir, resolve_launch_target, MutationGuard, ResolvedLaunchTarget, WorkbenchContext, WorkbenchHandle, WorkbenchMode};
pub use heading_restructure::{provider_hook, restructure_headings_llm, HEADING_INDEX_SYSTEM_PROMPT, HEADING_RELEVEL_SYSTEM_PROMPT};
pub use ingest::{
    preview_ingest_sources, retire_bundle_source, run_ingest_pipeline, IngestRequest, IngestResult, IngestSource,
    ValidationCounts, DEFAULT_EXPORT_FORMATS,
};
pub use upload::{reset_upload_directory, safe_upload_relative_path, sanitize_upload_segment, stage_uploaded_files, StagedUpload, UploadSource};
