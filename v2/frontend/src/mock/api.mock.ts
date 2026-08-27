/**
 * Mock implementations of every §4 command, operating on `src/mock/store.ts`.
 *
 * `src/lib/api.ts` is the only file that imports this module. Each function
 * here is named and typed identically to its real-backend counterpart so
 * that migrating a given command to a real `invoke()` call is a one-line
 * change in `src/lib/api.ts`, not a rewrite.
 */
import type {
  AgentContext,
  AgentEntry,
  AgentRequest,
  AgentTask,
  AuthoredDraft,
  AuthoredDraftListing,
  BundleCard,
  BundleKey,
  BundlePromptOverrides,
  BundleSummary,
  Concept,
  ConceptDetail,
  CreateProjectResult,
  DefaultPrompts,
  EditFieldValue,
  Evidence,
  GraphLimits,
  GraphResult,
  IngestPreviewFile,
  IngestPreviewResult,
  IngestRequest,
  IngestResult,
  IngestRunStatus,
  IngestSource,
  KnowledgeItem,
  KnowledgeSearchHit,
  LegacyStateStatusEntry,
  McpClientTarget,
  McpConfigureResult,
  McpInvocation,
  MineKnowledgeResult,
  NormalizeSourcesResult,
  NormalizedTextResult,
  ProfileInput,
  ProfileView,
  RelationNeighborhood,
  Relation,
  RepairBundleDirsResult,
  RetireSourceResult,
  ReportEntry,
  ResolveLaunchTargetResult,
  ReviewEvent,
  ScanSourcesResult,
  Section,
  SettingsView,
  Source,
  SourceStats,
  StagedUpload,
  SopkbError,
  TestConnectionResult,
  ValidateBundleResult,
  ValidationSummary,
  WorkbenchContext,
} from '../types/commands';
import { SopkbApiError } from '../types/commands';
import { publish } from '../lib/events';
import { deriveEvidence } from './derive';
import { invalidInput, missingConfiguration, notFound, store, type BundleRecord } from './store';

// Simulated IPC latency so loading states are visibly exercised in the UI
// rather than resolving instantly every time.
const LATENCY_MS = 120;
function delay<T>(value: T): Promise<T> {
  return new Promise((resolve) => setTimeout(() => resolve(value), LATENCY_MS));
}

async function guard<T>(fn: () => T): Promise<T> {
  try {
    return await delay(fn());
  } catch (err) {
    if (err && typeof err === 'object' && 'kind' in err && 'message' in err) {
      throw new SopkbApiError(err as SopkbError);
    }
    throw new SopkbApiError({ kind: 'Io', message: String(err) });
  }
}

const REPORT_DEFS: { key: string; name: string; path: string }[] = [
  { key: 'extraction_summary', name: 'Extraction Summary', path: 'reports/extraction_summary.md' },
  { key: 'validation', name: 'Validation', path: 'reports/validation.md' },
  { key: 'conflicts', name: 'Conflicts', path: 'reports/conflicts.md' },
  { key: 'freshness', name: 'Freshness', path: 'reports/freshness.md' },
  { key: 'review_summary', name: 'Review Summary', path: 'reports/review_summary.md' },
  { key: 'export_summary', name: 'Export Summary', path: 'export_summary.md' },
];

const REPORT_CHAR_CAP = 1800;
const SOURCE_PREVIEW_CAP = 900;

function toBundleSummary(bundle: BundleRecord): BundleSummary {
  return {
    key: bundle.key,
    id: bundle.id,
    title: bundle.title,
    profile: bundle.profile,
    status: bundle.status,
    source_count: bundle.sources.length,
    knowledge_item_count: bundle.items.length,
    created_at: bundle.created_at,
  };
}

// ---------------------------------------------------------------------------
// §4.1 Workbench context and lifecycle
// ---------------------------------------------------------------------------

export function get_workbench_context(): Promise<WorkbenchContext> {
  return guard(() => store.getContext());
}

export function resolve_launch_target(path?: string): Promise<ResolveLaunchTargetResult> {
  return guard(() => ({
    root: path ?? store.root,
    mode: store.mode,
    bundles_root: store.bundlesRoot,
    created_default: !path,
  }));
}

export function set_workbench_root(path: string): Promise<WorkbenchContext> {
  return guard(() => store.setWorkbenchRoot(path));
}

/** Mocked: no real OS dialog is available here, so this resolves to a canned path after a short delay. */
export function pick_workbench_folder(): Promise<string | null> {
  return guard(() => store.root);
}

export function select_bundle(key: BundleKey): Promise<BundleSummary> {
  return guard(() => toBundleSummary(store.requireBundle(key)));
}

export function deselect_bundle(): Promise<void> {
  return guard(() => store.deselectBundle());
}

// ---------------------------------------------------------------------------
// §4.2 Bundle management
// ---------------------------------------------------------------------------

export function list_bundles(): Promise<BundleCard[]> {
  return guard(() => store.listBundles());
}

export function create_project(title: string): Promise<CreateProjectResult> {
  return guard(() => store.createProject(title));
}

export function describe_bundle(key?: BundleKey): Promise<BundleSummary> {
  return guard(() => toBundleSummary(store.requireBundle(key)));
}

export function init_bundle(_path: string, title?: string): Promise<BundleSummary> {
  return guard(() => {
    const { bundle } = store.createProject(title ?? 'Untitled Bundle');
    return toBundleSummary(store.requireBundle(bundle.key));
  });
}

export function delete_bundle(key: BundleKey): Promise<void> {
  return guard(() => store.deleteBundle(key));
}

export function repair_bundle_dirs(key?: BundleKey): Promise<RepairBundleDirsResult> {
  return guard(() => {
    store.requireBundle(key);
    return { created: [] }; // mock bundles never have missing dirs
  });
}

export function get_legacy_state_status(key?: BundleKey): Promise<LegacyStateStatusEntry[]> {
  return guard(() => {
    store.requireBundle(key);
    return ['inventory', 'sections', 'items', 'reviews'].map((name) => ({
      name,
      canonical_present: true,
      legacy_present: false,
      legacy_path: '',
    }));
  });
}

export function get_bundle_prompt_overrides(key?: BundleKey): Promise<BundlePromptOverrides> {
  return guard(() => store.getBundlePromptOverrides(key));
}

export function set_bundle_prompt_overrides(overrides: BundlePromptOverrides, key?: BundleKey): Promise<void> {
  return guard(() => store.setBundlePromptOverrides(overrides, key));
}

// ---------------------------------------------------------------------------
// §4.3 Ingest pipeline
// ---------------------------------------------------------------------------

export function pick_source_files(): Promise<string[]> {
  return guard(() => [`${store.root}\\inbox\\new-procedure.md`]);
}

export function pick_source_folder(): Promise<string | null> {
  return guard(() => `${store.root}\\inbox`);
}

export function stage_source_files(
  paths: string[],
  _mode: 'files' | 'folder',
  replace: boolean,
): Promise<StagedUpload> {
  return guard(() => {
    const existing = replace ? { skipped: [], files: [] } : { skipped: store.stagedUpload?.skipped ?? [], files: store.stagedUpload?.files ?? [] };
    // Mock has no real filesystem behind this -- basenames only, "folder" mode's
    // subdirectory-preserving relative-path logic isn't replicated here.
    const newNames = paths.map((p) => p.split(/[\\/]/).pop() ?? p);
    const files = [...existing.files, ...newNames];
    const staged: StagedUpload = {
      staging_dir: `${store.root}\\.staging`,
      file_count: files.length,
      skipped: existing.skipped,
      files,
    };
    store.stagedUpload = staged;
    return staged;
  });
}

export function get_staged_sources(): Promise<StagedUpload | null> {
  return guard(() => store.stagedUpload);
}

export function get_last_ingest_run(): Promise<IngestRunStatus | null> {
  return guard(() => store.lastIngestRun);
}

export function clear_staged_sources(): Promise<void> {
  return guard(() => {
    store.stagedUpload = null;
  });
}

export function remove_staged_source(relative_path: string): Promise<StagedUpload | null> {
  return guard(() => {
    if (!store.stagedUpload) return null;
    const files = store.stagedUpload.files.filter((f) => f !== relative_path);
    store.stagedUpload = files.length === 0 ? null : { ...store.stagedUpload, files, file_count: files.length };
    return store.stagedUpload;
  });
}

async function emitIngestStep(step: string, work: () => void): Promise<void> {
  publish('ingest://progress', { step, status: 'started', detail: `starting ${step}` });
  try {
    work();
    await delay(undefined);
    publish('ingest://progress', { step, status: 'done', detail: `${step} complete` });
  } catch (err) {
    publish('ingest://progress', {
      step,
      status: 'failed',
      detail: err instanceof Error ? err.message : String(err),
    });
    throw err;
  }
}

/** See Rust's `cancel_ingest` command doc comment; this is the mock's mirror of it. */
export function cancel_ingest(): Promise<void> {
  return guard(() => {
    store.ingestCancelRequested = true;
  });
}

export async function run_ingest_pipeline(request: IngestRequest): Promise<IngestResult> {
  const bundle = store.requireBundle();
  if (request.source.kind === 'folder' && !request.source.path) {
    throw new SopkbApiError(invalidInput('source_dir is required unless staging is used'));
  }
  // A cancellation requested during a PREVIOUS run must not immediately kill this
  // new one -- mirrors the real `WorkbenchHandle::clear_cancel_request()` call at
  // the top of `run_ingest_pipeline`.
  store.ingestCancelRequested = false;
  const cancelledBeforeStep = (step: string): boolean => {
    if (!store.ingestCancelRequested) return false;
    publish('ingest://progress', { step, status: 'cancelled', detail: '' });
    return true;
  };

  const result: IngestResult = {};
  if (request.scan && cancelledBeforeStep('scan')) return result;
  if (request.scan) {
    await emitIngestStep('scan', () => {
      result.uploaded_files = store.stagedUpload?.file_count ?? bundle.sources.length;
      result.sources = bundle.sources.length;
    });
  }
  if (request.normalize && cancelledBeforeStep('normalize')) return result;
  if (request.normalize) {
    await emitIngestStep('normalize', () => {
      result.sections = bundle.sections.length;
    });
  }
  if (request.mine && cancelledBeforeStep('mine')) return result;
  if (request.mine) {
    await emitIngestStep('mine', () => {
      result.items = bundle.items.length;
      result.mine_provider = request.mine_provider;
    });
  }
  if (request.validate && cancelledBeforeStep('validate')) return result;
  if (request.validate) {
    await emitIngestStep('validate', () => {
      result.validation = { errors: 0, warnings: 0 };
    });
  }
  if (request.scan || request.normalize || request.mine || request.validate) {
    await emitIngestStep('sync_okf', () => {
      result.okf_bundle = { type: 'okf_native', path: '.' };
    });
    publish('bundle://state-changed', { key: bundle.key, scope: ['inventory', 'sections', 'items', 'okf'] });
  }
  if (request.export && cancelledBeforeStep('export')) return result;
  if (request.export) {
    await emitIngestStep('export', () => {
      result.exports = [
        { type: 'graph_json', path: `${bundle.export_path}\\graph\\graph.json` },
        { type: 'rdf', path: `${bundle.export_path}\\graph\\triples.ttl` },
      ];
    });
  }

  const status: Record<string, string> = {
    scan: request.scan ? 'done' : 'pending',
    normalize: request.normalize ? 'done' : 'pending',
    mine: request.mine ? 'done' : 'pending',
    validate: request.validate ? 'done' : 'pending',
    export: request.export ? 'done' : 'pending',
  };
  const detail: Record<string, string> = {};
  if (request.scan) detail.scan = `${result.sources ?? 0} sources`;
  if (request.normalize) detail.normalize = `${result.sections ?? 0} sections`;
  if (request.mine) detail.mine = `${result.items ?? 0} items`;
  if (request.validate) detail.validate = `${result.validation?.errors ?? 0} errors, ${result.validation?.warnings ?? 0} warnings`;
  if (request.export) detail.export = `${result.exports?.length ?? 0} artifacts`;
  store.lastIngestRun = { ok: true, finished_at: new Date().toISOString(), status, detail };

  return result;
}

/**
 * The mock has no real filesystem behind staged uploads —
 * `stage_source_files` above only tracks a count, not individual paths — so
 * this can't do a real directory diff the way the Rust
 * `classify_source_updates` does. It synthesizes plausible classification
 * data instead: every source already in the bundle reads back as
 * "unchanged" (a preview run against an already-ingested bundle with no new
 * files should show nothing pending), plus one "new" row per staged file.
 */
export function preview_ingest_pipeline(source: IngestSource): Promise<IngestPreviewResult> {
  return guard(() => {
    const bundle = store.requireBundle();
    if (source.kind === 'folder' && !source.path) {
      throw new SopkbApiError(invalidInput('source_dir is required unless staging is used'));
    }
    const sourceDir = source.kind === 'folder' ? source.path : `${bundle.bundle_path}\\.sopkb\\uploads\\current`;

    const files: IngestPreviewFile[] = bundle.sources.map((s) => ({
      path: s.original_path,
      classification: 'unchanged',
      source_id: s.id,
    }));
    const stagedCount = source.kind === 'staged' ? (store.stagedUpload?.file_count ?? 0) : 0;
    for (let i = 0; i < stagedCount; i += 1) {
      files.push({ path: `${sourceDir}\\staged-file-${i + 1}`, classification: 'new' });
    }
    return { source_dir: sourceDir, files };
  });
}

export function scan_sources(_source_dir: string, key?: BundleKey): Promise<ScanSourcesResult> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    publish('bundle://state-changed', { key: bundle.key, scope: ['inventory', 'okf'] });
    return { sources: bundle.sources.length, warnings: [] };
  });
}

export function normalize_sources(key?: BundleKey): Promise<NormalizeSourcesResult> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    publish('bundle://state-changed', { key: bundle.key, scope: ['sections', 'okf'] });
    return { sections: bundle.sections.length, failed: [] };
  });
}

export function mine_knowledge(provider: string, _profile_id: string | undefined, key?: BundleKey): Promise<MineKnowledgeResult> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    publish('bundle://state-changed', { key: bundle.key, scope: ['items', 'okf'] });
    return { items: bundle.items.length, provider };
  });
}

export function validate_bundle(key?: BundleKey): Promise<ValidateBundleResult> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    publish('bundle://state-changed', { key: bundle.key, scope: ['okf'] });
    return { errors: [], warnings: [] };
  });
}

// ---------------------------------------------------------------------------
// §4.4 Read / query
// ---------------------------------------------------------------------------

export function list_sources(): Promise<Source[]> {
  return guard(() => store.requireBundle().sources);
}

export function get_source(source_id: string): Promise<Source> {
  return guard(() => {
    const source = store.requireBundle().sources.find((s) => s.id === source_id);
    if (!source) throw notFound(`no source "${source_id}"`);
    return source;
  });
}

export function retire_source(source_id: string, rationale: string, reviewer?: string): Promise<RetireSourceResult> {
  return guard(() => {
    const source = store.requireBundle().sources.find((s) => s.id === source_id);
    if (!source) throw notFound(`no source "${source_id}"`);
    if (source.status === 'retired') return { retired: false, event: null };
    source.status = 'retired';
    return {
      retired: true,
      event: { action: 'source_retired', source_id, actor: reviewer || 'local:user', rationale },
    };
  });
}

export function get_normalized_text(source_id: string, max_chars?: number): Promise<NormalizedTextResult> {
  return guard(() => {
    const bundle = store.requireBundle();
    const source = bundle.sources.find((s) => s.id === source_id);
    if (!source) throw notFound(`no source "${source_id}"`);
    const sections = bundle.sections.filter((sec) => sec.source_id === source_id);
    const items = bundle.items.filter((i) => i.source_id === source_id);
    // Reconstruct a plausible normalized document from section headings + item source_text,
    // since the mock bundle carries no real markdown files on disk.
    const parts: string[] = [];
    for (const section of sections) {
      parts.push(`## ${section.heading}`);
      const sectionItems = items.filter((i) => i.section_id === section.id);
      for (const item of sectionItems) parts.push(item.source_text);
    }
    const full = parts.join('\n\n');
    const cap = max_chars ?? SOURCE_PREVIEW_CAP;
    return {
      text: full.length > cap ? full.slice(0, cap) : full,
      truncated: full.length > cap,
      present: true,
    };
  });
}

export function get_source_stats(): Promise<SourceStats> {
  return guard(() => {
    const sources = store.requireBundle().sources;
    const by_type: Record<string, number> = {};
    const by_parse_status: Record<string, number> = {};
    for (const s of sources) {
      by_type[s.type] = (by_type[s.type] ?? 0) + 1;
      by_parse_status[s.parse_status] = (by_parse_status[s.parse_status] ?? 0) + 1;
    }
    return { total: sources.length, by_type, by_parse_status };
  });
}

export function list_sections(): Promise<Section[]> {
  return guard(() => store.requireBundle().sections);
}

export function get_section(section_id: string): Promise<Section> {
  return guard(() => {
    const section = store.requireBundle().sections.find((s) => s.id === section_id);
    if (!section) throw notFound(`no section "${section_id}"`);
    return section;
  });
}

export function list_knowledge_items(): Promise<KnowledgeItem[]> {
  return guard(() => store.requireBundle().items);
}

export function get_knowledge_item(item_id: string): Promise<KnowledgeItem> {
  return guard(() => {
    const item = store.requireBundle().items.find((i) => i.id === item_id);
    if (!item) throw notFound(`no knowledge item "${item_id}"`);
    return item;
  });
}

/**
 * AND-of-substrings over lowercased `subject + predicate + object +
 * source_text`, matching D §4 exactly (order-independent, any field). An
 * empty/whitespace query returns the entire bundle with no limit — the
 * *command* preserves that; `KnowledgeScreen` is responsible for not
 * firing it on an empty input, per the same section.
 */
export function search_knowledge(query: string): Promise<KnowledgeSearchHit[]> {
  return guard(() => {
    const items = store.requireBundle().items;
    const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const hits = terms.length === 0
      ? items
      : items.filter((item) => {
          const haystack = `${item.subject} ${item.predicate} ${item.object} ${item.source_text}`.toLowerCase();
          return terms.every((t) => haystack.includes(t));
        });
    return hits.map((item) => ({
      id: item.id,
      subject: item.subject,
      predicate: item.predicate,
      object: item.object,
      review_status: item.review_status,
      source_id: item.source_id,
      section_id: item.section_id,
      evidence: item.source_text,
    }));
  });
}

export function get_evidence(item_id: string): Promise<Evidence> {
  return guard(() => {
    const item = store.requireBundle().items.find((i) => i.id === item_id);
    if (!item) throw notFound(`no knowledge item "${item_id}"`);
    return deriveEvidence(item);
  });
}

export function resolve_citation(citation_id: string): Promise<Evidence> {
  return guard(() => {
    const itemId = citation_id.replace(/^evidence-/, '');
    const item = store.requireBundle().items.find((i) => i.id === itemId);
    if (!item) throw notFound(`no evidence for citation "${citation_id}"`);
    return deriveEvidence(item);
  });
}

export function get_concept_index(): Promise<Concept[]> {
  return guard(() => store.concepts(store.requireBundle()).concepts);
}

const RULES_PER_CONCEPT_CAP = 24;

export function get_concept_detail(concept_id: string): Promise<ConceptDetail> {
  return guard(() => {
    const bundle = store.requireBundle();
    const { concepts, rules } = store.concepts(bundle);
    const concept = concepts.find((c) => c.id === concept_id);
    if (!concept) throw notFound(`no concept "${concept_id}"`);
    const items = bundle.items.filter((i) => `concept-${slug(i.subject)}` === concept_id);
    const conceptRules = rules.filter((r) => items.some((i) => i.id === r.knowledge_item_id)).slice(0, RULES_PER_CONCEPT_CAP);
    return { concept, items, rules: conceptRules };
  });
}

function slug(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
}

export function get_review_detail(item_id: string): Promise<import('../types/commands').ReviewDetail> {
  return guard(() => {
    const bundle = store.requireBundle();
    const item = bundle.items.find((i) => i.id === item_id);
    if (!item) throw notFound(`no knowledge item "${item_id}"`);
    const { relations, rules } = store.concepts(bundle);
    const relation = relations.find((r) => r.knowledge_piece_id === item_id);
    const itemRules = rules.filter((r) => r.knowledge_item_id === item_id);
    const allowed = store.allowedActions(item);
    return {
      item,
      relation,
      rules: itemRules,
      evidence_id: `evidence-${item.id}`,
      mutable: allowed.length > 1,
      allowed_actions: allowed,
    };
  });
}

const GRAPH_NODE_CAP = 90;
const GRAPH_ITEMS_PER_CONCEPT_CAP = 18;
const GRAPH_RULES_PER_CONCEPT_CAP = 4;

export function get_graph(concept_id?: string, _limits?: GraphLimits): Promise<GraphResult> {
  return guard(() => {
    const bundle = store.requireBundle();
    const { concepts, relations, rules } = store.concepts(bundle);
    const focusConcepts = concept_id ? concepts.filter((c) => c.id === concept_id) : concepts;

    const nodes: import('../types/commands').GraphNode[] = [];
    const edges: import('../types/commands').GraphEdge[] = [];

    for (const concept of focusConcepts) {
      nodes.push({ id: concept.id, type: 'concept', label: concept.label });
      const conceptItems = bundle.items
        .filter((i) => `concept-${slug(i.subject)}` === concept.id)
        .slice(0, GRAPH_ITEMS_PER_CONCEPT_CAP);
      const conceptRules = rules
        .filter((r) => conceptItems.some((i) => i.id === r.knowledge_item_id))
        .slice(0, GRAPH_RULES_PER_CONCEPT_CAP);
      for (const item of conceptItems) {
        nodes.push({ id: item.id, type: 'knowledge_item', label: item.object.slice(0, 60) });
        edges.push({ source: concept.id, target: item.id, type: 'has_knowledge' });
        const relation = relations.find((r) => r.knowledge_piece_id === item.id);
        if (relation) {
          edges.push({ source: item.id, target: concept.id, type: 'relation_subject' });
        }
      }
      for (const rule of conceptRules) {
        nodes.push({ id: rule.id, type: 'decision_rule', label: rule.title.slice(0, 60) });
        edges.push({ source: concept.id, target: rule.id, type: 'has_review' });
      }
    }

    const cappedNodes = nodes.slice(0, GRAPH_NODE_CAP);
    const nodeIds = new Set(cappedNodes.map((n) => n.id));
    // Real export graphs legitimately contain dangling edges by design (relation_subject
    // edges pointing at concept nodes that may not exist among section-derived concepts).
    // We don't "fix" this by dropping edges whose endpoints got capped out — we keep them,
    // same as the real export, so the renderer's dangling-edge tolerance actually gets exercised.
    void nodeIds;

    if (concept_id) {
      return { origin: 'live', nodes: cappedNodes, edges };
    }
    return {
      origin: 'export',
      nodes: cappedNodes,
      edges,
      export_path: `${bundle.export_path}\\graph\\graph.json`,
    };
  });
}

export function get_reports(): Promise<ReportEntry[]> {
  return guard(() => {
    const bundle = store.requireBundle();
    return REPORT_DEFS.map(({ key, name, path }) => {
      const text = bundle.reportsMarkdown[key] ?? '';
      return {
        name,
        path,
        present: text.length > 0,
        text: text.length > REPORT_CHAR_CAP ? text.slice(0, REPORT_CHAR_CAP) : text,
      };
    });
  });
}

export function get_validation_summary(): Promise<ValidationSummary> {
  return guard(() => {
    const bundle = store.requireBundle();
    return { errors: 0, warnings: 0, export_dir: bundle.export_path };
  });
}

export function get_conflicts_report(): Promise<string> {
  return guard(() => store.requireBundle().reportsMarkdown.conflicts ?? '');
}

export function get_freshness_report(): Promise<string> {
  return guard(() => store.requireBundle().reportsMarkdown.freshness ?? '');
}

export function get_agent_guide(): Promise<string> {
  return guard(() => store.requireBundle().reportsMarkdown.agent_guide ?? '');
}

export function list_authored_drafts(): Promise<AuthoredDraftListing[]> {
  return guard(() => []);
}

export function get_authored_draft(rel_path: string): Promise<AuthoredDraft> {
  return guard(() => {
    throw notFound(`no authored draft at "${rel_path}"`);
  });
}

// ---------------------------------------------------------------------------
// §4.5 Review
// ---------------------------------------------------------------------------

export function approve_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  return guard(() => store.setReviewStatus(undefined, item_id, 'approved', 'approved', rationale, reviewer));
}

export function reject_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  return guard(() => store.setReviewStatus(undefined, item_id, 'rejected', 'rejected', rationale, reviewer));
}

export function defer_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  return guard(() => store.setReviewStatus(undefined, item_id, 'deferred', 'deferred', rationale, reviewer));
}

export function comment_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  return guard(() => store.commentItem(undefined, item_id, rationale, reviewer));
}

export function edit_item(
  item_id: string,
  field: string,
  value: EditFieldValue,
  rationale: string,
  reviewer?: string,
): Promise<ReviewEvent> {
  return guard(() => store.editItem(undefined, item_id, field, value, rationale, reviewer));
}

export function list_review_events(item_id?: string): Promise<ReviewEvent[]> {
  return guard(() => store.listReviewEvents(undefined, item_id));
}

// ---------------------------------------------------------------------------
// §4.6 Agent
// ---------------------------------------------------------------------------

export function list_agent_tasks(): Promise<AgentTask[]> {
  return guard(() => store.requireBundle().agentTasks);
}

export function get_agent_transcript(limit?: number): Promise<AgentEntry[]> {
  return guard(() => {
    const transcript = store.requireBundle().agentTranscript;
    return limit ? transcript.slice(-limit) : transcript;
  });
}

export async function run_agent_chat(request: AgentRequest): Promise<AgentEntry> {
  const bundle = store.requireBundle();
  if (request.provider === 'azure-llm' || request.provider === 'azure-llm-tools') {
    const hasProfile = store.settingsProfiles.some((p) => p.id === (request.profile_id ?? store.defaultProfileId));
    if (!hasProfile) {
      throw new SopkbApiError(
        missingConfiguration('no LLM profile is configured — add one in Settings or use the "context" provider'),
      );
    }
  }
  publish('agent://progress', { phase: 'retrieval', detail: 'gathering matching knowledge and rules' });
  await delay(undefined);
  const { concepts } = store.concepts(bundle);
  const task = bundle.agentTasks.find((t) => t.id === request.task_id);
  const matched = concepts.slice(0, 3);
  publish('agent://progress', { phase: 'answering', detail: 'composing answer from retrieved context' });
  await delay(undefined);
  // Prior turns in this SAME chat -- mirrors the real backend's `chat_turns_for`:
  // folded into the mock answer text so a test (or a person poking at the mock
  // build) can actually observe that turn 2 "remembers" turn 1, not just that a
  // `chat_id` field round-trips.
  const priorTurns = store.requireBundle().agentTranscript.filter((e) => e.chat_id === request.chat_id).length;
  const memoryNote = priorTurns > 0 ? ` (turn ${priorTurns + 1} of this chat — remembers ${priorTurns} earlier turn(s))` : '';
  const entry: AgentEntry = {
    created_at: new Date().toISOString(),
    provider: request.provider,
    profile_id: request.profile_id,
    task_id: request.task_id,
    task_title: task?.title ?? 'Auto',
    scenario: request.scenario,
    allow_proposed: request.allow_proposed,
    chat_id: request.chat_id,
    detected_concepts: matched.map((c) => ({
      id: c.id,
      label: c.label,
      score: 1,
      match_reasons: [`label terms: ${c.label.toLowerCase()}`],
    })),
    context_summary: {
      usable_knowledge_count: bundle.items.length,
      decision_rule_count: bundle.items.length,
      relation_count: bundle.items.length,
      evidence_count: bundle.items.length,
      concept_count: concepts.length,
      detected_concept_count: matched.length,
      excluded_knowledge_count: 0,
    },
    answer:
      request.provider === 'context'
        ? `[context provider — no LLM call] Matched ${matched.length} concept(s) for scenario "${request.scenario}": ${matched.map((c) => c.label).join(', ') || 'none'}.`
        : request.provider === 'azure-llm-tools'
          ? `[mock azure-llm-tools response, after looking something up]${memoryNote} Based on ${bundle.items.length} knowledge items across ${concepts.length} concepts, here is guidance for: ${request.scenario}`
          : `[mock azure-llm response]${memoryNote} Based on ${bundle.items.length} knowledge items across ${concepts.length} concepts, here is guidance for: ${request.scenario}`,
    ...(request.provider === 'azure-llm-tools'
      ? {
          tool_iterations: 1,
          trace: [
            {
              tool: 'knowledge.search',
              args: { query: request.scenario.split(' ').slice(0, 3).join(' ') },
              result: bundle.items.slice(0, 1).map((i) => ({ id: i.id, subject: i.subject })),
            },
          ],
        }
      : {}),
  };
  publish('agent://progress', { phase: 'done', detail: 'answer ready' });
  return store.appendAgentEntry(undefined, entry);
}

export function get_task_context(task_id: string, include_rejected: boolean): Promise<AgentContext> {
  return guard(() => buildAgentContext(task_id, include_rejected));
}

export function get_scenario_context(
  _scenario: string,
  task_id?: string,
  include_rejected = false,
  item_limit?: number,
): Promise<AgentContext> {
  return guard(() => ({
    ...buildAgentContext(task_id ?? 'auto', include_rejected, item_limit),
    retrieval: { mode: 'scenario-concepts' },
  }));
}

function buildAgentContext(_task_id: string, include_rejected: boolean, item_limit?: number): AgentContext {
  const bundle = store.requireBundle();
  const { concepts, relations, rules } = store.concepts(bundle);
  const items = (include_rejected ? bundle.items : bundle.items.filter((i) => i.review_status !== 'rejected')).slice(
    0,
    item_limit ?? 32,
  );
  return {
    usable_knowledge: items,
    knowledge_relations: relations.filter((r) => items.some((i) => i.id === r.knowledge_piece_id)),
    evidence: items.map(deriveEvidence),
    decision_rules: rules.filter((r) => items.some((i) => i.id === r.knowledge_item_id)),
    concepts,
  };
}

export function clear_agent_transcript(): Promise<void> {
  return guard(() => store.clearAgentTranscript(undefined));
}

export function delete_agent_chat(chat_id: string): Promise<number> {
  return guard(() => store.deleteAgentChat(undefined, chat_id));
}

// ---------------------------------------------------------------------------
// §4.7 Export
// ---------------------------------------------------------------------------

export function sync_okf_documents(key?: BundleKey): Promise<{ type: 'okf_native'; path: '.' }> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    publish('bundle://state-changed', { key: bundle.key, scope: ['okf'] });
    return { type: 'okf_native' as const, path: '.' as const };
  });
}

export function get_export_dir(): Promise<string> {
  return guard(() => store.requireBundle().export_path);
}

export function reveal_path(_path: string): Promise<void> {
  // Mocked: no OS shell to hand off to in this environment.
  return guard(() => undefined);
}

// ---------------------------------------------------------------------------
// §4.8 Settings
// ---------------------------------------------------------------------------

export function get_settings(): Promise<SettingsView> {
  return guard(() => ({
    profiles: store.settingsProfiles,
    default_profile_id: store.defaultProfileId,
    reviewer_name: store.reviewerName,
    settings_path: `${store.root}\\settings.json`,
    env_overrides: store.envOverrides,
    max_parallel_workers: store.maxParallelWorkers,
  }));
}

export function get_default_prompts(): Promise<DefaultPrompts> {
  return guard(() => ({
    mining_prompt: 'You are an OKF v0.2 author for SOP knowledge. (mock default -- see AUTHOR_SYSTEM_PROMPT in the real backend)',
    chat_prompt: 'You are an SOP Knowledge Bundle agent. (mock default -- see SYSTEM_PROMPT in the real backend)',
    heading_index_prompt: 'You are a document indexing engine. (mock default -- see HEADING_INDEX_SYSTEM_PROMPT in the real backend)',
    heading_relevel_prompt: 'You are given an ordered index of section/subsection headings. (mock default -- see HEADING_RELEVEL_SYSTEM_PROMPT in the real backend)',
  }));
}

export function set_max_parallel_workers(value: number): Promise<void> {
  return guard(() => {
    store.maxParallelWorkers = Math.max(1, Math.trunc(value));
  });
}

export function save_profile(input: ProfileInput): Promise<ProfileView> {
  return guard(() => store.saveProfile(input));
}

export function delete_profile(id: string): Promise<void> {
  return guard(() => store.deleteProfile(id));
}

export function set_default_profile(id: string): Promise<void> {
  return guard(() => store.setDefaultProfile(id));
}

export function set_reviewer_name(name: string): Promise<void> {
  return guard(() => store.setReviewerName(name));
}

export function test_profile_connection(id?: string): Promise<TestConnectionResult> {
  return guard(() => {
    const profile = store.settingsProfiles.find((p) => p.id === (id ?? store.defaultProfileId));
    if (!profile) throw notFound('no profile to test');
    if (!profile.has_api_key) {
      return { ok: false, detail: 'no API key configured for this profile' };
    }
    return { ok: true, detail: `mock connection to ${profile.base_url} succeeded` };
  });
}

// ---------------------------------------------------------------------------
// §4.9 Relations
// ---------------------------------------------------------------------------

export function search_relations(subject?: string, predicate?: string, object?: string): Promise<Relation[]> {
  return guard(() => {
    const bundle = store.requireBundle();
    const { relations } = store.concepts(bundle);
    const s = subject?.toLowerCase();
    const p = predicate?.toLowerCase();
    const o = object?.toLowerCase();
    return relations.filter(
      (r) =>
        (!s || r.subject.text.toLowerCase().includes(s)) &&
        (!p || r.predicate.text.toLowerCase().includes(p)) &&
        (!o || r.object.text.toLowerCase().includes(o)),
    );
  });
}

export function get_relation_neighborhood(node_id: string): Promise<RelationNeighborhood> {
  return guard(() => {
    const bundle = store.requireBundle();
    const { relations } = store.concepts(bundle);
    const needle = node_id.toLowerCase();
    const matches = relations.filter(
      (r) =>
        r.id.toLowerCase().includes(needle) ||
        r.knowledge_piece_id.toLowerCase().includes(needle) ||
        r.subject.id.toLowerCase().includes(needle) ||
        r.subject.text.toLowerCase().includes(needle) ||
        r.predicate.text.toLowerCase().includes(needle) ||
        r.object.text.toLowerCase().includes(needle),
    );
    return { node_id, relations: matches };
  });
}

// ---------------------------------------------------------------------------
// §4.10 MCP
// ---------------------------------------------------------------------------

export function get_mcp_invocation(key?: BundleKey): Promise<McpInvocation> {
  return guard(() => {
    const bundle = store.requireBundle(key);
    return {
      command: 'sopkb-mcp',
      args: ['--bundle', bundle.bundle_path],
      enable_review_notes_flag: '--enable-review-notes',
      entry_name: `sopkb-${bundle.key}`,
    };
  });
}

export function list_mcp_client_targets(): Promise<McpClientTarget[] | null> {
  return guard(() => store.listMcpClientTargets());
}

export function rescan_mcp_client_targets(): Promise<McpClientTarget[]> {
  return guard(() => store.listMcpClientTargets());
}

export function configure_mcp_client(client_id: string, force: boolean, key?: BundleKey): Promise<McpConfigureResult> {
  return guard(() => store.configureMcpClient(client_id, key, force));
}

// ---------------------------------------------------------------------------
// Task #38: diagnostics export
// ---------------------------------------------------------------------------

/** No native save dialog in mock mode -- always "saves" to a fixed fake path. */
export function export_diagnostics_bundle(): Promise<string | null> {
  return guard(() => `${store.root}\\sopkb-diagnostics.zip`);
}
