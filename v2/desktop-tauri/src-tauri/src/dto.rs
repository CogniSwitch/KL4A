//! Serializable wire-shape DTOs for the `sopkb-workbench` types that don't derive
//! `serde::Serialize` themselves (they're a plain orchestration library, deliberately
//! not Tauri/serde-aware -- see that crate's lib.rs doc comment). Each `From` impl here
//! is the ONLY place a backend struct's field order/naming is translated to the wire
//! shape in PORT_PLAN.md §4.0/§4.1 -- keep these one-call thin, no logic.

use serde::Serialize;
use sopkb_workbench::{
    BundleCard, BundleSummary, ConceptDetail, ConceptSummary, CreateProjectResult, Graph, GraphEdge, GraphNode, IngestResult,
    ResolvedLaunchTarget, StagedUpload, ValidationCounts, WorkbenchContext, WorkbenchMode,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WireMode {
    SingleBundle,
    WorkbenchRoot,
    Degraded,
}

impl From<WorkbenchMode> for WireMode {
    fn from(m: WorkbenchMode) -> Self {
        match m {
            WorkbenchMode::SingleBundle => WireMode::SingleBundle,
            WorkbenchMode::WorkbenchRoot => WireMode::WorkbenchRoot,
            WorkbenchMode::Degraded => WireMode::Degraded,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireWorkbenchContext {
    pub root: String,
    pub mode: WireMode,
    pub bundles_root: String,
    pub selected_bundle: Option<String>,
    pub generation: u64,
    pub settings_path: String,
    pub error: Option<String>,
}

impl From<WorkbenchContext> for WireWorkbenchContext {
    fn from(c: WorkbenchContext) -> Self {
        Self {
            root: c.root.display().to_string(),
            mode: c.mode.into(),
            bundles_root: c.bundles_root.display().to_string(),
            selected_bundle: c.selected_bundle,
            generation: c.generation,
            settings_path: c.settings_path.display().to_string(),
            error: c.error,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireResolvedLaunchTarget {
    pub root: String,
    pub mode: WireMode,
    pub bundles_root: String,
    pub created_default: bool,
}

impl From<ResolvedLaunchTarget> for WireResolvedLaunchTarget {
    fn from(t: ResolvedLaunchTarget) -> Self {
        Self { root: t.root.display().to_string(), mode: t.mode.into(), bundles_root: t.bundles_root.display().to_string(), created_default: t.created_default }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireBundleSummary {
    pub key: String,
    pub id: String,
    pub title: String,
    pub profile: String,
    pub status: String,
    pub source_count: usize,
    pub knowledge_item_count: usize,
    pub created_at: String,
}

impl From<BundleSummary> for WireBundleSummary {
    fn from(s: BundleSummary) -> Self {
        Self {
            key: s.key,
            id: s.id,
            title: s.title,
            profile: s.profile,
            status: s.status,
            source_count: s.source_count,
            knowledge_item_count: s.knowledge_item_count,
            created_at: s.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireBundleCard {
    pub key: String,
    pub bundle_path: String,
    #[serde(flatten)]
    pub summary: Option<WireBundleSummary>,
    pub export_path: Option<String>,
    pub load_error: Option<String>,
}

impl From<BundleCard> for WireBundleCard {
    fn from(c: BundleCard) -> Self {
        Self {
            key: c.key,
            bundle_path: c.bundle_path.display().to_string(),
            summary: c.summary.map(Into::into),
            export_path: c.export_path.map(|p| p.display().to_string()),
            load_error: c.load_error,
        }
    }
}

/// §4.2 `create_project`: `{ bundle: BundleCard, already_existed: bool, okf_synced: bool }`.
/// `okf_synced` is always `true` on a successful call -- `bundles::create_project`
/// unconditionally runs `sync_okf_bundle` via `?` before returning, so an `Ok` result
/// can only mean the sync also succeeded (P-W20). Kept as an explicit field rather
/// than dropped, per PORT_PLAN's instruction, so the wire shape says so rather than
/// leaving the frontend to assume it.
#[derive(Debug, Clone, Serialize)]
pub struct WireCreateProjectResult {
    pub bundle: WireBundleCard,
    pub already_existed: bool,
    pub okf_synced: bool,
}

impl From<CreateProjectResult> for WireCreateProjectResult {
    fn from(r: CreateProjectResult) -> Self {
        Self { bundle: r.card.into(), already_existed: r.already_existed, okf_synced: true }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireConceptSummary {
    pub id: String,
    pub label: String,
    pub item_count: usize,
    pub rule_count: usize,
    pub statuses: std::collections::BTreeMap<String, i64>,
}

impl From<ConceptSummary> for WireConceptSummary {
    fn from(c: ConceptSummary) -> Self {
        Self { id: c.id, label: c.label, item_count: c.item_count, rule_count: c.rule_count, statuses: c.statuses }
    }
}

/// §4.4 `get_concept_detail`: `{ concept, items: [KnowledgeItem], rules: [DecisionRule] }`.
/// `items`/`rules` are already `serde_json::Value` on the `sopkb-workbench` side (raw
/// bundle JSON, uncapped per P-W24) -- passed through unchanged rather than reshaped
/// into a narrower typed struct, which is also how `extra{}` round-tripping (§4.0,
/// P-D1) is satisfied for these two fields without an explicit catch-all.
#[derive(Debug, Clone, Serialize)]
pub struct WireConceptDetail {
    pub concept: WireConceptSummary,
    pub items: Vec<serde_json::Value>,
    pub rules: Vec<serde_json::Value>,
}

impl From<ConceptDetail> for WireConceptDetail {
    fn from(d: ConceptDetail) -> Self {
        Self { concept: d.concept.into(), items: d.items, rules: d.rules }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireGraphNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub label: String,
}

impl From<GraphNode> for WireGraphNode {
    fn from(n: GraphNode) -> Self {
        Self { id: n.id, node_type: n.node_type, label: n.label }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WireGraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}

impl From<GraphEdge> for WireGraphEdge {
    fn from(e: GraphEdge) -> Self {
        Self { source: e.source, target: e.target, edge_type: e.edge_type }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WireGraph {
    pub nodes: Vec<WireGraphNode>,
    pub edges: Vec<WireGraphEdge>,
}

impl From<Graph> for WireGraph {
    fn from(g: Graph) -> Self {
        Self { nodes: g.nodes.into_iter().map(Into::into).collect(), edges: g.edges.into_iter().map(Into::into).collect() }
    }
}

/// §4.3 `IngestRequest`/`IngestResult`. `ValidationCounts`/`IngestResult` (like the
/// other `sopkb-workbench` types above) don't derive `Serialize` themselves.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WireValidationCounts {
    pub errors: usize,
    pub warnings: usize,
}

impl From<ValidationCounts> for WireValidationCounts {
    fn from(v: ValidationCounts) -> Self {
        Self { errors: v.errors, warnings: v.warnings }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WireIngestResult {
    pub uploaded_files: Option<usize>,
    pub sources: Option<usize>,
    pub sections: Option<usize>,
    pub items: Option<usize>,
    pub mine_provider: Option<String>,
    pub validation: Option<WireValidationCounts>,
    pub okf_bundle: Option<serde_json::Value>,
    pub exports: Option<Vec<serde_json::Value>>,
}

impl From<IngestResult> for WireIngestResult {
    fn from(r: IngestResult) -> Self {
        Self {
            uploaded_files: r.uploaded_files,
            sources: r.sources,
            sections: r.sections,
            items: r.items,
            mine_provider: r.mine_provider,
            validation: r.validation.map(Into::into),
            okf_bundle: r.okf_bundle,
            exports: r.exports,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SkippedUpload {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WireStagedUpload {
    pub staging_dir: String,
    pub file_count: usize,
    pub skipped: Vec<SkippedUpload>,
    /// Relative paths (forward-slash-joined, so a `"folder"`-mode stage's
    /// subdirectory structure is still legible) of every currently staged file --
    /// lets the frontend render an itemized, individually-removable list instead
    /// of only a count. Empty for the `From<StagedUpload>` impl below (the
    /// immediate result of a stage call), since the caller already knows exactly
    /// which files it just staged; populated by `peek_staged_sources`, which
    /// re-reads the directory from scratch.
    pub files: Vec<String>,
}

impl From<StagedUpload> for WireStagedUpload {
    fn from(s: StagedUpload) -> Self {
        Self { staging_dir: s.staging_dir.display().to_string(), file_count: s.file_count, skipped: Vec::new(), files: Vec::new() }
    }
}
