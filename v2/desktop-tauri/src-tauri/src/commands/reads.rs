//! §4.4 Read/query commands. Thin, cacheable reads. Most backend functions here
//! already return raw `serde_json::Value`/`Vec<Value>` (see `sopkb-derive/src/reads.rs`'s
//! own doc comment: nothing in the pipeline constructs typed structs from bundle
//! JSON), so those commands pass the value straight through unchanged -- which is
//! also how `extra{}` round-tripping (§4.0) is satisfied without an explicit
//! catch-all field: nothing is ever reshaped into a narrower type that could drop an
//! unknown key.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::dto::{WireConceptDetail, WireConceptSummary, WireGraph};
use crate::error::{AppError, CmdResult};
use crate::state::{resolve_bundle_dir, AppState};

#[tauri::command(rename_all = "snake_case")]
pub fn list_sources(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::sources_list(&bundle_dir).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_source(state: State<AppState>, source_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::sources_get(&bundle_dir, &source_id).map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NormalizedTextResult {
    pub text: String,
    pub truncated: bool,
    pub present: bool,
}

/// `CharPos`/truncation is Unicode-scalar-offset based per §4.0, not a byte offset --
/// `str::chars().take(n)` is the char-boundary-safe way to do that. Pure/testable
/// independent of the file read.
pub(crate) fn truncate_text(text: String, max_chars: Option<usize>) -> (String, bool) {
    match max_chars {
        Some(max) if text.chars().count() > max => (text.chars().take(max).collect(), true),
        _ => (text, false),
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_normalized_text(state: State<AppState>, source_id: String, max_chars: Option<usize>, key: Option<String>) -> CmdResult<NormalizedTextResult> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let source = sopkb_derive::reads::sources_get(&bundle_dir, &source_id).map_err(AppError::from)?;
    let Some(normalized_path) = source.get("normalized_path").and_then(|v| v.as_str()) else {
        return Ok(NormalizedTextResult { text: String::new(), truncated: false, present: false });
    };
    match std::fs::read_to_string(bundle_dir.join(normalized_path)) {
        Ok(raw) => {
            let (text, truncated) = truncate_text(raw, max_chars);
            Ok(NormalizedTextResult { text, truncated, present: true })
        }
        Err(_) => Ok(NormalizedTextResult { text: String::new(), truncated: false, present: false }),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct SourceStats {
    pub total: usize,
    pub by_type: BTreeMap<String, usize>,
    pub by_parse_status: BTreeMap<String, usize>,
}

/// Pure aggregation over an already-fetched source list, unit-tested with synthetic
/// JSON rather than a real bundle on disk.
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

#[tauri::command(rename_all = "snake_case")]
pub fn get_source_stats(state: State<AppState>, key: Option<String>) -> CmdResult<SourceStats> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let sources = sopkb_derive::reads::sources_list(&bundle_dir).map_err(AppError::from)?;
    Ok(compute_source_stats(&sources))
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_sections(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::sections_list(&bundle_dir).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_section(state: State<AppState>, section_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::sections_get(&bundle_dir, &section_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_knowledge_items(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::knowledge_items(&bundle_dir).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_knowledge_item(state: State<AppState>, item_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::knowledge_get(&bundle_dir, &item_id).map_err(AppError::from)
}

/// D §4: empty/whitespace-only query returns the entire bundle, no limit -- kept
/// as-is (the frontend, not this command, is responsible for not firing this on an
/// empty input).
#[tauri::command(rename_all = "snake_case")]
pub fn search_knowledge(state: State<AppState>, query: String, key: Option<String>) -> CmdResult<Vec<Value>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::knowledge_search(&bundle_dir, &query).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_evidence(state: State<AppState>, item_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::evidence_get(&bundle_dir, &item_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn resolve_citation(state: State<AppState>, citation_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    sopkb_derive::reads::citations_resolve(&bundle_dir, &citation_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_concept_index(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<WireConceptSummary>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let concepts = sopkb_workbench::concept_index(&bundle_dir).map_err(AppError::from)?;
    Ok(concepts.into_iter().map(Into::into).collect())
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_concept_detail(state: State<AppState>, concept_id: String, key: Option<String>) -> CmdResult<WireConceptDetail> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let detail = sopkb_workbench::get_concept_detail(&bundle_dir, &concept_id).map_err(AppError::from)?;
    Ok(detail.into())
}

/// §4.5's `allowed_actions`: comment is always legal; the other four
/// (`sopkb_review::REVIEW_ACTIONS`, in its own declared order) only when `mutable`.
/// Reuses the canonical action vocabulary from `sopkb-review` rather than
/// duplicating the list of action names here.
pub(crate) fn allowed_actions(mutable: bool) -> Vec<&'static str> {
    sopkb_review::REVIEW_ACTIONS.iter().copied().filter(|a| *a == "commented" || mutable).collect()
}

/// §4.4 `get_review_detail`: combine `knowledge_get` + `knowledge_relation_for_item`
/// + `decision_rules_for_item`, plus `mutable`/`allowed_actions` computed from the
/// item's `review_status` (mutable = not in `{approved, rejected}`, matching
/// `sopkb-review::review`'s own `ensure_mutable`, which treats a missing/null/
/// unrecognized status as mutable too). Pure once the item JSON is in hand, so
/// unit-tested directly with synthetic items rather than a real bundle.
pub(crate) fn build_review_detail(item: Value) -> Value {
    let status = item.get("review_status").and_then(|v| v.as_str()).unwrap_or("");
    let mutable = !matches!(status, "approved" | "rejected");
    let relation = sopkb_derive::relations::knowledge_relation_for_item(&item);
    let rules = sopkb_derive::rules::decision_rules_for_item(&item);
    let evidence_id = sopkb_derive::relations::evidence_id_for_item(&item);
    serde_json::json!({
        "item": item,
        "relation": relation,
        "rules": rules,
        "evidence_id": evidence_id,
        "mutable": mutable,
        "allowed_actions": allowed_actions(mutable),
    })
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_review_detail(state: State<AppState>, item_id: String, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let item = sopkb_derive::reads::knowledge_get(&bundle_dir, &item_id).map_err(AppError::from)?;
    Ok(build_review_detail(item))
}

/// §4.4 `get_graph`'s tagged shape for the `concept_id`-given ("live") branch. Pure
/// shaping over an already-computed `Graph`, so testable without touching disk.
pub(crate) fn shape_live_graph(graph: sopkb_workbench::Graph) -> Value {
    let wire: WireGraph = graph.into();
    serde_json::json!({ "origin": "live", "nodes": wire.nodes, "edges": wire.edges })
}

/// Same for the export-file ("export") branch: `graph.json`'s own `nodes`/`edges`
/// keys are passed through unchanged (F3: the export vocabulary is deliberately left
/// byte-identical, never "fixed" to match the live vocabulary), plus `origin` and
/// `export_path` added.
pub(crate) fn shape_export_graph(raw: Value, export_path: String) -> Value {
    serde_json::json!({
        "origin": "export",
        "nodes": raw.get("nodes").cloned().unwrap_or_else(|| serde_json::json!([])),
        "edges": raw.get("edges").cloned().unwrap_or_else(|| serde_json::json!([])),
        "export_path": export_path,
    })
}

/// `limits` is accepted for wire-shape compatibility with PORT_PLAN.md §4.4 but is
/// currently a no-op: `sopkb_workbench::focused_graph_for_concept`'s caps (18 items,
/// 4 rules per item) are hardcoded, not parameterized (its own doc comment: "caps...
/// PRESERVED bit-identically"), and re-implementing the traversal here to honor a
/// custom limit would put domain logic in this thin command layer, which PORT_PLAN
/// explicitly disallows. Flagged in the integration report as a real gap.
#[tauri::command(rename_all = "snake_case")]
pub fn get_graph(state: State<AppState>, concept_id: Option<String>, limits: Option<Value>, key: Option<String>) -> CmdResult<Value> {
    let _ = limits;
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    match concept_id.filter(|id| !id.is_empty()) {
        Some(id) => {
            let graph = sopkb_workbench::focused_graph_for_concept(&bundle_dir, &id).map_err(AppError::from)?;
            Ok(shape_live_graph(graph))
        }
        None => {
            let path = sopkb_export::export_artifact_path(&bundle_dir, "graph/graph.json").map_err(AppError::from)?;
            let raw = sopkb_core::store::read_json(&path, serde_json::json!({"nodes": [], "edges": []})).map_err(AppError::from)?;
            Ok(shape_export_graph(raw, path.display().to_string()))
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReportEntry {
    pub name: String,
    pub path: String,
    pub present: bool,
    pub text: String,
}

fn report_entry(name: &str, path: PathBuf) -> ReportEntry {
    match std::fs::read_to_string(&path) {
        Ok(text) => ReportEntry { name: name.to_string(), path: path.display().to_string(), present: true, text },
        Err(_) => ReportEntry { name: name.to_string(), path: path.display().to_string(), present: false, text: String::new() },
    }
}

/// Six fixed names, in display order (§4.4): the five `write_standard_reports`/
/// `validate_bundle` write into `bundle_dir/reports/*.md`, plus `export_summary`
/// which -- unlike the other five -- lives in the *export* directory, written only
/// by `export_bundle` (`sopkb-export/src/bundle_export.rs::write_export_summary`).
pub(crate) const BUNDLE_REPORT_NAMES: &[&str] = &["freshness", "conflicts", "extraction_summary", "review_summary", "validation"];

#[tauri::command(rename_all = "snake_case")]
pub fn get_reports(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<ReportEntry>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let mut entries: Vec<ReportEntry> =
        BUNDLE_REPORT_NAMES.iter().map(|name| report_entry(name, bundle_dir.join("reports").join(format!("{name}.md")))).collect();
    let export_entry = match sopkb_export::default_export_dir(&bundle_dir) {
        Ok(export_dir) => report_entry("export_summary", export_dir.join("export_summary.md")),
        // No readable manifest / export not yet run: report absent rather than
        // failing the whole six-report screen over one entry (this command's whole
        // point is "show me what's there"; other five entries are still valid).
        Err(_) => ReportEntry { name: "export_summary".to_string(), path: String::new(), present: false, text: String::new() },
    };
    entries.push(export_entry);
    Ok(entries)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_validation_summary(state: State<AppState>, key: Option<String>) -> CmdResult<Value> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let validation = sopkb_core::store::read_json(&bundle_dir.join("reports").join("validation.json"), serde_json::json!({"errors": [], "warnings": []}))
        .map_err(AppError::from)?;
    let export_dir = sopkb_export::default_export_dir(&bundle_dir).map_err(AppError::from)?;
    Ok(serde_json::json!({
        "errors": validation.get("errors").cloned().unwrap_or_else(|| serde_json::json!([])),
        "warnings": validation.get("warnings").cloned().unwrap_or_else(|| serde_json::json!([])),
        "export_dir": export_dir.display().to_string(),
    }))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_conflicts_report(state: State<AppState>, key: Option<String>) -> CmdResult<String> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(sopkb_derive::reads::conflicts_list(&bundle_dir))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_freshness_report(state: State<AppState>, key: Option<String>) -> CmdResult<String> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(sopkb_derive::reads::freshness_check(&bundle_dir))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_agent_guide(state: State<AppState>, key: Option<String>) -> CmdResult<String> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    Ok(sopkb_derive::context::agent_guide(&bundle_dir))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AuthoredDraftSummary {
    pub rel_path: String,
    pub bytes: u64,
}

/// No existing backend function lists `.sopkb/authored_okf/**/*.md` (§4.4 notes this
/// explicitly) -- small local walk, sorted for determinism. `rel_path` is always
/// `/`-separated regardless of host platform, matching the rest of this codebase's
/// convention for bundle-relative paths (`sopkb_core::store::relative_to_bundle`).
fn walk_markdown_files(root: &Path, dir: &Path, out: &mut Vec<AuthoredDraftSummary>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            walk_markdown_files(root, &path, out);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel_path = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push(AuthoredDraftSummary { rel_path, bytes });
        }
    }
}

pub(crate) fn list_authored_drafts_at(authored_root: &Path) -> Vec<AuthoredDraftSummary> {
    let mut out = Vec::new();
    walk_markdown_files(authored_root, authored_root, &mut out);
    out
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_authored_drafts(state: State<AppState>, key: Option<String>) -> CmdResult<Vec<AuthoredDraftSummary>> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let authored_root = sopkb_core::store::state_path(&bundle_dir, "authored_okf");
    Ok(list_authored_drafts_at(&authored_root))
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthoredDraftText {
    pub text: String,
}

/// Same belt-and-braces traversal guard `sopkb-workbench::upload` uses for uploaded
/// file names, reused here via its own public `resolve_best_effort` rather than
/// duplicated, since `rel_path` is caller-supplied and a crafted `../../` should not
/// be able to read outside `.sopkb/authored_okf/`.
pub(crate) fn read_authored_draft_at(authored_root: &Path, rel_path: &str) -> Result<String, AppError> {
    let candidate = authored_root.join(rel_path);
    let resolved_root = sopkb_workbench::paths::resolve_best_effort(authored_root);
    let resolved_candidate = sopkb_workbench::paths::resolve_best_effort(&candidate);
    if !resolved_candidate.starts_with(&resolved_root) {
        return Err(AppError::invalid_input(format!("unsafe authored draft path: {rel_path}")));
    }
    std::fs::read_to_string(&candidate).map_err(|e| AppError::from(sopkb_core::error::SopkbError::from(e)))
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_authored_draft(state: State<AppState>, rel_path: String, key: Option<String>) -> CmdResult<AuthoredDraftText> {
    let bundle_dir = resolve_bundle_dir(&state, key.as_deref())?;
    let authored_root = sopkb_core::store::state_path(&bundle_dir, "authored_okf");
    let text = read_authored_draft_at(&authored_root, &rel_path)?;
    Ok(AuthoredDraftText { text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn truncate_text_no_limit_returns_everything() {
        let (text, truncated) = truncate_text("hello world".to_string(), None);
        assert_eq!(text, "hello world");
        assert!(!truncated);
    }

    #[test]
    fn truncate_text_under_limit_is_unchanged() {
        let (text, truncated) = truncate_text("hi".to_string(), Some(10));
        assert_eq!(text, "hi");
        assert!(!truncated);
    }

    #[test]
    fn truncate_text_respects_unicode_scalar_boundaries() {
        // 5 multi-byte characters; a byte-offset truncation at "3 bytes" would panic
        // or split a codepoint. Char-based truncation must not.
        let text = "héllo".to_string();
        let (truncated, was_truncated) = truncate_text(text, Some(3));
        assert_eq!(truncated, "hél");
        assert!(was_truncated);
    }

    #[test]
    fn truncate_text_exact_length_is_not_marked_truncated() {
        let (text, truncated) = truncate_text("abc".to_string(), Some(3));
        assert_eq!(text, "abc");
        assert!(!truncated);
    }

    #[test]
    fn compute_source_stats_counts_by_type_and_parse_status() {
        let sources = vec![
            serde_json::json!({"type": "markdown", "parse_status": "normalized"}),
            serde_json::json!({"type": "markdown", "parse_status": "failed"}),
            serde_json::json!({"type": "pdf", "parse_status": "normalized"}),
        ];
        let stats = compute_source_stats(&sources);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.by_type.get("markdown"), Some(&2));
        assert_eq!(stats.by_type.get("pdf"), Some(&1));
        assert_eq!(stats.by_parse_status.get("normalized"), Some(&2));
        assert_eq!(stats.by_parse_status.get("failed"), Some(&1));
    }

    #[test]
    fn compute_source_stats_empty_list() {
        let stats = compute_source_stats(&[]);
        assert_eq!(stats.total, 0);
        assert!(stats.by_type.is_empty());
    }

    #[test]
    fn allowed_actions_mutable_includes_all_five_in_review_actions_order() {
        assert_eq!(allowed_actions(true), vec!["approved", "rejected", "deferred", "commented", "edited"]);
    }

    #[test]
    fn allowed_actions_immutable_only_comment() {
        assert_eq!(allowed_actions(false), vec!["commented"]);
    }

    fn sample_item(review_status: &str) -> Value {
        serde_json::json!({
            "id": "ki-1", "item_type": "requirement", "subject": "Patient", "predicate": "requires",
            "object": "identity confirmation", "source_id": "s1", "section_id": "sec1",
            "source_text": "the patient must confirm identity", "start_pos": 0, "end_pos": 10,
            "span_status": "exact", "derivation": "extracted", "confidence": 0.9,
            "review_status": review_status,
        })
    }

    #[test]
    fn build_review_detail_mutable_for_proposed_item() {
        let detail = build_review_detail(sample_item("proposed"));
        assert_eq!(detail["mutable"], serde_json::json!(true));
        assert_eq!(detail["allowed_actions"].as_array().unwrap().len(), 5);
        assert!(detail["relation"].is_object());
        assert!(detail["rules"].is_array());
        assert_eq!(detail["evidence_id"], serde_json::json!("evidence-ki-1"));
    }

    #[test]
    fn build_review_detail_immutable_for_approved_item() {
        let detail = build_review_detail(sample_item("approved"));
        assert_eq!(detail["mutable"], serde_json::json!(false));
        assert_eq!(detail["allowed_actions"], serde_json::json!(["commented"]));
    }

    #[test]
    fn build_review_detail_missing_status_is_mutable() {
        // ensure_mutable in sopkb-review only excludes {approved, rejected} --
        // missing/null/unrecognized falls through as mutable (C §1.2).
        let mut item = sample_item("some-unrecognized-value");
        item["review_status"] = serde_json::Value::Null;
        let detail = build_review_detail(item);
        assert_eq!(detail["mutable"], serde_json::json!(true));
    }

    #[test]
    fn shape_live_graph_tags_origin_live() {
        let graph = sopkb_workbench::Graph {
            nodes: vec![sopkb_workbench::GraphNode { id: "c1".into(), node_type: "concept".into(), label: "Alpha".into() }],
            edges: vec![],
        };
        let value = shape_live_graph(graph);
        assert_eq!(value["origin"], serde_json::json!("live"));
        assert_eq!(value["nodes"].as_array().unwrap().len(), 1);
        assert!(value.get("export_path").is_none());
    }

    #[test]
    fn shape_export_graph_tags_origin_export_and_preserves_export_vocabulary() {
        let raw = serde_json::json!({"nodes": [{"id": "n1", "type": "relation"}], "edges": [{"source": "a", "target": "b", "type": "has_review"}]});
        let value = shape_export_graph(raw, "/exports/bundle/graph/graph.json".to_string());
        assert_eq!(value["origin"], serde_json::json!("export"));
        assert_eq!(value["export_path"], serde_json::json!("/exports/bundle/graph/graph.json"));
        assert_eq!(value["nodes"][0]["type"], serde_json::json!("relation"));
        assert_eq!(value["edges"][0]["type"], serde_json::json!("has_review"));
    }

    #[test]
    fn shape_export_graph_missing_keys_default_to_empty_arrays() {
        let value = shape_export_graph(serde_json::json!({}), "p".to_string());
        assert_eq!(value["nodes"], serde_json::json!([]));
        assert_eq!(value["edges"], serde_json::json!([]));
    }

    #[test]
    fn list_authored_drafts_at_finds_nested_md_files_sorted_and_ignores_other_extensions() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("authored_okf");
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::create_dir_all(root.join("overrides")).unwrap();
        std::fs::write(root.join("rules").join("b-rule.md"), "content").unwrap();
        std::fs::write(root.join("overrides").join("a-note.md"), "hi").unwrap();
        std::fs::write(root.join("overrides").join("notes.txt"), "not markdown").unwrap();

        let drafts = list_authored_drafts_at(&root);
        assert_eq!(drafts.len(), 2, "the .txt file must be excluded");
        let rel_paths: Vec<&str> = drafts.iter().map(|d| d.rel_path.as_str()).collect();
        assert_eq!(rel_paths, vec!["overrides/a-note.md", "rules/b-rule.md"]);
        assert!(drafts.iter().all(|d| d.bytes > 0));
    }

    #[test]
    fn list_authored_drafts_at_missing_directory_is_empty_not_an_error() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(list_authored_drafts_at(&missing).is_empty());
    }

    #[test]
    fn read_authored_draft_at_reads_a_real_file() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("authored_okf");
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::write(root.join("rules").join("x.md"), "# hello").unwrap();
        let text = read_authored_draft_at(&root, "rules/x.md").unwrap();
        assert_eq!(text, "# hello");
    }

    #[test]
    fn read_authored_draft_at_rejects_traversal_outside_the_authored_root() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("authored_okf");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "nope").unwrap();
        let err = read_authored_draft_at(&root, "../secret.txt").unwrap_err();
        assert_eq!(err.kind, "InvalidInput");
    }
}
