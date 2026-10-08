//! Mirrors `desktop-tauri/src-tauri/src/commands/reads.rs`'s directly-`Value`-
//! returning subset (§4.4). NOT covered here, disclosed as a gap in
//! `docs/port/CATCHUP_PLAN.md`: `get_concept_index`/`get_concept_detail`/`get_graph`
//! (need the same Wire-DTO shaping `desktop-tauri/src-tauri/src/dto.rs` does, which
//! this pass didn't have time to duplicate) and `list_authored_drafts`/
//! `get_authored_draft`/`get_reports`.

use axum::extract::{Path as AxPath, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::error::ApiResult;
use crate::routes::bundles::KeyQuery;
use crate::state::{resolve_bundle_dir, AppState};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
    pub key: Option<String>,
}

macro_rules! simple_read {
    ($name:ident, $target:path) => {
        pub async fn $name(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
            let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
            Ok(Json(json!($target(&bundle_dir)?)))
        }
    };
}

simple_read!(list_sources, sopkb_derive::reads::sources_list);
simple_read!(list_sections, sopkb_derive::reads::sections_list);
simple_read!(list_knowledge_items, sopkb_derive::reads::knowledge_items);

pub async fn get_source(State(state): State<AppState>, AxPath(source_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::reads::sources_get(&bundle_dir, &source_id)?))
}

pub async fn get_section(State(state): State<AppState>, AxPath(section_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::reads::sections_get(&bundle_dir, &section_id)?))
}

pub async fn get_knowledge_item(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::reads::knowledge_get(&bundle_dir, &item_id)?))
}

pub async fn search_knowledge(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(json!(sopkb_derive::reads::knowledge_search(&bundle_dir, &q.q)?)))
}

pub async fn get_evidence(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::reads::evidence_get(&bundle_dir, &item_id)?))
}

pub async fn resolve_citation(State(state): State<AppState>, AxPath(citation_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(Json(sopkb_derive::reads::citations_resolve(&bundle_dir, &citation_id)?))
}

pub async fn get_conflicts_report(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<String> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(sopkb_derive::reads::conflicts_list(&bundle_dir))
}

pub async fn get_freshness_report(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<String> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(sopkb_derive::reads::freshness_check(&bundle_dir))
}

pub async fn get_agent_guide(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<String> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    Ok(sopkb_derive::context::agent_guide(&bundle_dir))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct SourceStats {
    pub total: usize,
    pub by_type: BTreeMap<String, usize>,
    pub by_parse_status: BTreeMap<String, usize>,
}

/// Byte-identical logic to `desktop-tauri`'s own `compute_source_stats` (pure
/// aggregation, small enough to duplicate rather than depend on a standalone crate).
pub(crate) fn compute_source_stats(sources: &[Value]) -> SourceStats {
    let mut stats = SourceStats { total: sources.len(), ..Default::default() };
    for s in sources {
        if let Some(t) = s.get("type").and_then(|v| v.as_str()) {
            *stats.by_type.entry(t.to_string()).or_insert(0) += 1;
        }
        if let Some(p) = s.get("parse_status").and_then(|v| v.as_str()) {
            *stats.by_parse_status.entry(p.to_string()).or_insert(0) += 1;
        }
    }
    stats
}

pub async fn get_source_stats(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<SourceStats>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let sources = sopkb_derive::reads::sources_list(&bundle_dir)?;
    Ok(Json(compute_source_stats(&sources)))
}

pub async fn get_validation_summary(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let validation = sopkb_core::store::read_json(&bundle_dir.join("reports").join("validation.json"), json!({"errors": [], "warnings": []}))?;
    let export_dir = sopkb_export::default_export_dir(&bundle_dir)?;
    Ok(Json(json!({
        "errors": validation.get("errors").cloned().unwrap_or_else(|| json!([])),
        "warnings": validation.get("warnings").cloned().unwrap_or_else(|| json!([])),
        "export_dir": export_dir.display().to_string(),
    })))
}

/// Byte-identical logic to `desktop-tauri`'s own `build_review_detail`. Round 7
/// (item 4): `edited` is the only action still gated on `mutable` -- see
/// `sopkb-review::review`'s own doc comments on `TERMINAL_STATUSES`/`ensure_mutable`.
pub(crate) fn build_review_detail(item: Value) -> Value {
    let status = item.get("review_status").and_then(|v| v.as_str()).unwrap_or("");
    let mutable = !matches!(status, "approved" | "rejected");
    let relation = sopkb_derive::relations::knowledge_relation_for_item(&item);
    let rules = sopkb_derive::rules::decision_rules_for_item(&item);
    let evidence_id = sopkb_derive::relations::evidence_id_for_item(&item);
    let allowed_actions: Vec<&str> = sopkb_review::REVIEW_ACTIONS.iter().copied().filter(|a| *a != "edited" || mutable).collect();
    json!({
        "item": item, "relation": relation, "rules": rules, "evidence_id": evidence_id,
        "mutable": mutable, "allowed_actions": allowed_actions,
    })
}

pub async fn get_review_detail(State(state): State<AppState>, AxPath(item_id): AxPath<String>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Value>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let item = sopkb_derive::reads::knowledge_get(&bundle_dir, &item_id)?;
    Ok(Json(build_review_detail(item)))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NormalizedTextResult {
    pub text: String,
    pub truncated: bool,
    pub present: bool,
}

#[derive(Debug, Deserialize)]
pub struct NormalizedTextQuery {
    pub max_chars: Option<usize>,
    pub key: Option<String>,
}

pub(crate) fn truncate_text(text: String, max_chars: Option<usize>) -> (String, bool) {
    match max_chars {
        Some(max) if text.chars().count() > max => (text.chars().take(max).collect(), true),
        _ => (text, false),
    }
}

pub async fn get_normalized_text(
    State(state): State<AppState>,
    AxPath(source_id): AxPath<String>,
    Query(q): Query<NormalizedTextQuery>,
) -> ApiResult<Json<NormalizedTextResult>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let source = sopkb_derive::reads::sources_get(&bundle_dir, &source_id)?;
    let Some(normalized_path) = source.get("normalized_path").and_then(|v| v.as_str()) else {
        return Ok(Json(NormalizedTextResult { text: String::new(), truncated: false, present: false }));
    };
    match std::fs::read_to_string(bundle_dir.join(normalized_path)) {
        Ok(raw) => {
            let (text, truncated) = truncate_text(raw, q.max_chars);
            Ok(Json(NormalizedTextResult { text, truncated, present: true }))
        }
        Err(_) => Ok(Json(NormalizedTextResult { text: String::new(), truncated: false, present: false })),
    }
}

/// Mirrors `desktop-tauri/src-tauri/src/dto.rs`'s `WireConceptSummary` --
/// `sopkb_workbench::ConceptSummary` itself isn't `Serialize` (workspace crates
/// stay wire-format-agnostic on purpose), so this thin duplicate is the same
/// necessarily-duplicated pattern the rest of this module already uses (see this
/// file's own top-of-file doc comment).
#[derive(Debug, Clone, Serialize)]
pub struct WireConceptSummary {
    pub id: String,
    pub label: String,
    pub item_count: usize,
    pub rule_count: usize,
    pub statuses: BTreeMap<String, i64>,
}

impl From<sopkb_workbench::ConceptSummary> for WireConceptSummary {
    fn from(c: sopkb_workbench::ConceptSummary) -> Self {
        Self { id: c.id, label: c.label, item_count: c.item_count, rule_count: c.rule_count, statuses: c.statuses }
    }
}

pub async fn get_concept_index(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Vec<WireConceptSummary>>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let concepts = sopkb_workbench::concept_index(&bundle_dir)?;
    Ok(Json(concepts.into_iter().map(Into::into).collect()))
}

#[derive(Debug, Clone, Serialize)]
pub struct WireConceptDetail {
    pub concept: WireConceptSummary,
    pub items: Vec<Value>,
    pub rules: Vec<Value>,
}

impl From<sopkb_workbench::ConceptDetail> for WireConceptDetail {
    fn from(d: sopkb_workbench::ConceptDetail) -> Self {
        Self { concept: d.concept.into(), items: d.items, rules: d.rules }
    }
}

pub async fn get_concept_detail(
    State(state): State<AppState>,
    AxPath(concept_id): AxPath<String>,
    Query(q): Query<KeyQuery>,
) -> ApiResult<Json<WireConceptDetail>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let detail = sopkb_workbench::get_concept_detail(&bundle_dir, &concept_id)?;
    Ok(Json(detail.into()))
}

/// Mirrors `desktop-tauri/src-tauri/src/commands/reads.rs`'s `ReportEntry`/
/// `BUNDLE_REPORT_NAMES`/`get_reports` exactly, including `export_summary`'s
/// special case (lives in the *export* directory, not `reports/`, and a missing/
/// unreadable manifest reports it absent rather than failing the other five).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportEntry {
    pub name: String,
    pub path: String,
    pub present: bool,
    pub text: String,
}

fn report_entry(name: &str, path: std::path::PathBuf) -> ReportEntry {
    match std::fs::read_to_string(&path) {
        Ok(text) => ReportEntry { name: name.to_string(), path: path.display().to_string(), present: true, text },
        Err(_) => ReportEntry { name: name.to_string(), path: path.display().to_string(), present: false, text: String::new() },
    }
}

const BUNDLE_REPORT_NAMES: &[&str] = &["freshness", "conflicts", "extraction_summary", "review_summary", "validation"];

pub async fn get_reports(State(state): State<AppState>, Query(q): Query<KeyQuery>) -> ApiResult<Json<Vec<ReportEntry>>> {
    let bundle_dir = resolve_bundle_dir(&state.workbench, q.key.as_deref())?;
    let mut entries: Vec<ReportEntry> =
        BUNDLE_REPORT_NAMES.iter().map(|name| report_entry(name, bundle_dir.join("reports").join(format!("{name}.md")))).collect();
    let export_entry = match sopkb_export::default_export_dir(&bundle_dir) {
        Ok(export_dir) => report_entry("export_summary", export_dir.join("export_summary.md")),
        Err(_) => ReportEntry { name: "export_summary".to_string(), path: String::new(), present: false, text: String::new() },
    };
    entries.push(export_entry);
    Ok(Json(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_source_stats_counts_by_type_and_status() {
        let sources = vec![json!({"type": "pdf", "parse_status": "normalized"}), json!({"type": "pdf", "parse_status": "failed"})];
        let stats = compute_source_stats(&sources);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_type.get("pdf"), Some(&2));
        assert_eq!(stats.by_parse_status.get("failed"), Some(&1));
    }

    #[test]
    fn truncate_text_respects_char_boundaries_not_bytes() {
        let (text, truncated) = truncate_text("hello world".to_string(), Some(5));
        assert_eq!(text, "hello");
        assert!(truncated);
    }

    #[test]
    fn build_review_detail_marks_approved_items_immutable() {
        let item = json!({"id": "i1", "review_status": "approved"});
        let detail = build_review_detail(item);
        assert_eq!(detail["mutable"], json!(false));
        assert_eq!(detail["allowed_actions"], json!(["approved", "rejected", "deferred", "commented"]));
    }
}
