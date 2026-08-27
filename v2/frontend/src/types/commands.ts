/**
 * TypeScript type surface for the Tauri command contract.
 *
 * Source of truth: `docs/port/PORT_PLAN.md` §4 (lines 335-623), the "frozen
 * contract" against which this frontend, the `desktop-tauri/src-tauri`
 * command layer, and `sopkb-cli` are all being built in parallel and
 * independently. Do not "improve" shapes here without updating that
 * document first — a mismatch here is exactly the class of bug this file
 * exists to catch at compile time instead of at integration.
 *
 * Every command in `src/lib/api.ts` is typed against this file, and every
 * mock implementation in `src/mock/` returns values assignable to these
 * types. When the real `#[tauri::command]` layer lands, swapping
 * `src/lib/api.ts`'s bodies from mock calls to `invoke()` calls should not
 * require touching this file at all (that is the point of freezing the
 * contract first).
 */

// ---------------------------------------------------------------------------
// §4.0 Shared shapes
// ---------------------------------------------------------------------------

export type BundleKey = string;

export type ReviewStatus =
  | 'proposed'
  | 'approved'
  | 'rejected'
  | 'deferred'
  | 'edited';

export type ReviewAction =
  | 'approved'
  | 'rejected'
  | 'deferred'
  | 'commented'
  | 'edited';

/** "unresolved" is read-only from the frontend's point of view (F15). */
export type SpanStatus = 'exact' | 'llm_claimed' | 'unresolved';

/** Unicode scalar offset into normalized Markdown. */
export type CharPos = number;

/** Unknown-field bag that must round-trip untouched (§3.3 / P-D1). */
export type ExtraFields = Record<string, unknown>;

export interface BundleSummary {
  key: BundleKey;
  id: string;
  title: string;
  profile: string;
  status: string;
  source_count: number;
  knowledge_item_count: number;
  /** ISO8601 UTC, from the manifest's own `created_at` (§4.2 `WireBundleSummary`). */
  created_at: string;
}

export interface BundleCard extends BundleSummary {
  bundle_path: string;
  export_path: string;
  /**
   * Set when this bundle failed to load (malformed manifest, unreadable
   * directory, etc). The card must still be rendered in `list_bundles` —
   * §4.2 is explicit that a broken bundle must not silently vanish from the
   * index the way the original Python UI did.
   */
  load_error?: string;
}

export interface Source {
  id: string;
  title: string;
  type: string;
  original_path: string;
  normalized_path?: string;
  checksum: string;
  size: number;
  modified_time: string;
  parse_status: string;
  warnings: string[];
  metadata: ExtraFields;
  extra: ExtraFields;
  /** `"active"` (default) or `"retired"` -- set by `retire_source`. Never deleted. */
  status?: string;
}

/** `retire_source`'s result -- `event` is the appended `source_retired` audit event, or null if the source was already retired (idempotent no-op). */
export interface RetireSourceResult {
  retired: boolean;
  event: Record<string, unknown> | null;
}

export interface Section {
  id: string;
  source_id: string;
  heading: string;
  semantic_role: string;
  start_pos: CharPos;
  end_pos: CharPos;
  normalized_path: string;
  extra: ExtraFields;
}

export interface KnowledgeItem {
  id: string;
  item_type: string;
  subject: string;
  predicate: string;
  object: string;
  source_id: string;
  section_id: string;
  source_text: string;
  start_pos?: CharPos;
  end_pos?: CharPos;
  span_status: SpanStatus;
  derivation: string;
  confidence: number;
  review_status: ReviewStatus;
  metadata: ExtraFields;
  extra: ExtraFields;
}

/** Projection returned by `search_knowledge` — NOT the full item shape. */
export interface KnowledgeSearchHit {
  id: string;
  subject: string;
  predicate: string;
  object: string;
  review_status: ReviewStatus;
  source_id: string;
  section_id: string;
  /** Rename of `source_text` (D 18) — deliberately not called `source_text` here. */
  evidence: string;
}

export interface Evidence {
  knowledge_item_id: string;
  source_id: string;
  section_id: string;
  source_text: string;
  span_status: SpanStatus;
  start_pos?: CharPos;
  end_pos?: CharPos;
}

export interface ReviewEvent {
  id: string;
  knowledge_item_id: string;
  action: ReviewAction;
  reviewer: string;
  timestamp: string;
  rationale: string;
  previous_value?: unknown;
  new_value?: unknown;
}

export interface RelationSubject {
  id: string;
  label: string;
  text: string;
  okf_path: string;
}

export interface RelationPredicate {
  id: string;
  text: string;
}

export interface RelationObject {
  id: string;
  text: string;
  label: string;
}

export interface Relation {
  id: string;
  type: 'Knowledge Relation';
  subject: RelationSubject;
  predicate: RelationPredicate;
  object: RelationObject;
  knowledge_piece_id: string;
  evidence_id: string;
  review_status: ReviewStatus;
  confidence: number;
  rdf_compatible: true;
}

export interface DecisionRuleCondition {
  fact: string;
  operator: string;
  label: string;
}

export interface DecisionRuleObligation {
  fact: string;
  action: string;
  label: string;
}

export interface DecisionRuleOtherwise {
  action_required: boolean;
  label: string;
}

/** Key ORDER as declared here mirrors the contract text; treat it as load-bearing. */
export interface DecisionRule {
  id: string;
  type: string;
  title: string;
  knowledge_item_id: string;
  source_id: string;
  section_id: string;
  review_status: ReviewStatus;
  confidence: number;
  condition?: DecisionRuleCondition;
  obligation: DecisionRuleObligation;
  evidence_id: string;
  relation_id: string;
  okf_path: string;
  otherwise?: DecisionRuleOtherwise;
}

export interface ExportArtifact {
  type: 'graph_json' | 'rdf';
  path: string;
}

export type SopkbErrorKind =
  | 'NotFound'
  | 'InvalidInput'
  | 'Conflict'
  | 'MissingConfiguration'
  | 'Upstream'
  | 'Io'
  | 'Format';

export interface SopkbError {
  kind: SopkbErrorKind;
  message: string;
  detail?: string;
}

/** Thrown/rejected by every command in `src/lib/api.ts`. */
export class SopkbApiError extends Error implements SopkbError {
  kind: SopkbErrorKind;
  detail?: string;

  constructor(error: SopkbError) {
    super(error.message);
    this.name = 'SopkbApiError';
    this.kind = error.kind;
    this.detail = error.detail;
  }
}

// ---------------------------------------------------------------------------
// §4.1 Workbench context and lifecycle
// ---------------------------------------------------------------------------

export type WorkbenchMode = 'SingleBundle' | 'WorkbenchRoot' | 'Degraded';

export interface WorkbenchContext {
  root: string;
  mode: WorkbenchMode;
  bundles_root: string;
  selected_bundle?: BundleKey;
  /** Monotonic counter; bumped on root switch, used to guard in-flight mutations. */
  generation: number;
  settings_path: string;
  error?: string;
}

export interface ResolveLaunchTargetResult {
  root: string;
  mode: WorkbenchMode;
  bundles_root: string;
  created_default: boolean;
}

// ---------------------------------------------------------------------------
// §4.2 Bundle management
// ---------------------------------------------------------------------------

export interface CreateProjectResult {
  bundle: BundleCard;
  already_existed: boolean;
  okf_synced: boolean;
}

export interface RepairBundleDirsResult {
  created: string[];
}

export interface LegacyStateStatusEntry {
  name: string;
  canonical_present: boolean;
  legacy_present: boolean;
  legacy_path: string;
}

// ---------------------------------------------------------------------------
// §4.3 Ingest pipeline
// ---------------------------------------------------------------------------

export interface StagedUpload {
  staging_dir: string;
  file_count: number;
  skipped: { path: string; reason: string }[];
  /** Relative paths (forward-slash-joined) of every currently staged file, for an itemized, individually-removable list. */
  files: string[];
}

export type MineProvider = 'fixture' | 'azure-llm';

export type IngestSource =
  | { kind: 'staged' }
  | { kind: 'folder'; path: string };

export interface IngestRequest {
  source: IngestSource;
  scan: boolean;
  normalize: boolean;
  mine: boolean;
  validate: boolean;
  export: boolean;
  mine_provider: MineProvider;
  profile_id?: string;
}

/**
 * Every field is `| null`, not just optional: the Rust side's `Option<T>`
 * serializes to a present key with value `null` for a skipped step (no
 * `skip_serializing_if`), it doesn't omit the key. A consumer that only
 * guards with `!== undefined` (or a `field?: T` type without `| null`) will
 * let `null` through to a `.property`/`.length` access -- see
 * `IngestScreen.tsx`'s `IngestResultStats`, which hit exactly this.
 */
export interface IngestResult {
  uploaded_files?: number | null;
  sources?: number | null;
  sections?: number | null;
  items?: number | null;
  mine_provider?: string | null;
  validation?: { errors: number; warnings: number } | null;
  okf_bundle?: { type: 'okf_native'; path: '.' } | null;
  exports?: ExportArtifact[] | null;
}

/**
 * `.sopkb/ingest_run.json`'s persisted shape -- the last `run_ingest_pipeline`
 * call's outcome, kept across reloads so a failed run's detail doesn't just
 * disappear on refresh (`get_last_ingest_run`). Mirrors the Python original's
 * `write_ingest_run` on the `ui/sopkb-web-redesign` branch (not this branch's
 * tools/sopkb -- see CATCHUP_PLAN.md's "ui/sopkb-web-redesign branch scan"
 * idea #4, 2026-08-22). `status[step]` is one of `"done"|"error"|"pending"`
 * ("pending" covers both "checkbox was off" and "an earlier step's failure
 * stopped the pipeline before reaching it").
 */
export interface IngestRunStatus {
  ok: boolean;
  finished_at: string;
  status: Record<string, string>;
  detail: Record<string, string>;
}

/**
 * `preview_ingest_pipeline`'s result — a read-only dry run of what a real
 * `run_ingest_pipeline`/`scan_sources` would do to this source directory,
 * shown before committing to a destructive run. `classification` is
 * deliberately a 3-way read of the CURRENT inventory (no source-versioning
 * concept exists in the Rust engine yet — see the Rust
 * `classify_source_updates` doc comment), not oss-launch's richer
 * `new_source`/`new_version`/`unchanged_version`/`version_number` shape.
 */
export type IngestPreviewClassification = 'new' | 'updated' | 'unchanged' | 'unsupported';

export interface IngestPreviewFile {
  path: string;
  classification: IngestPreviewClassification;
  source_id?: string;
}

export interface IngestPreviewResult {
  source_dir: string;
  files: IngestPreviewFile[];
}

export interface ScanSourcesResult {
  sources: number;
  warnings: { path: string; warning: string }[];
}

export interface NormalizeSourcesResult {
  sections: number;
  failed: { source_id: string; warning: string }[];
}

export interface MineKnowledgeResult {
  items: number;
  provider: string;
}

export interface ValidateBundleResult {
  errors: string[];
  warnings: string[];
}

// ---------------------------------------------------------------------------
// §4.4 Read / query — screen data
// ---------------------------------------------------------------------------

export interface NormalizedTextResult {
  text: string;
  truncated: boolean;
  present: boolean;
}

export interface SourceStats {
  total: number;
  by_type: Record<string, number>;
  by_parse_status: Record<string, number>;
}

export interface Concept {
  id: string;
  label: string;
  item_count: number;
  rule_count: number;
  statuses: Partial<Record<ReviewStatus, number>>;
}

export interface ConceptDetail {
  concept: Concept;
  items: KnowledgeItem[];
  rules: DecisionRule[];
}

export interface ReviewDetail {
  item: KnowledgeItem;
  relation?: Relation;
  rules: DecisionRule[];
  evidence_id: string;
  /**
   * Backend-computed mutability flag. The frontend must gate the four
   * status-changing actions (approve/reject/defer/edit) on
   * `allowed_actions`, not by re-deriving "is this item terminal?" from
   * `item.review_status` client-side — the contract calls this out
   * explicitly so the two sides of the app can't silently drift.
   */
  mutable: boolean;
  allowed_actions: ReviewAction[];
}

export interface GraphNode {
  id: string;
  type: string;
  label?: string;
  [key: string]: unknown;
}

export interface GraphEdge {
  source: string;
  target: string;
  type: string;
  [key: string]: unknown;
}

/**
 * Tagged union — discriminate explicitly on `origin`. §4.4 / F3: the two
 * origins carry incompatible node/edge vocabularies
 * (`knowledge_relation`/`decision_rule` vs `relation`/`rule`, disjoint edge
 * sets) and the export graph legitimately contains dangling edges by
 * design. Never branch on shape ("does this have `export_path`?") — branch
 * on `origin`.
 */
export type GraphResult =
  | { origin: 'export'; nodes: GraphNode[]; edges: GraphEdge[]; export_path: string }
  | { origin: 'live'; nodes: GraphNode[]; edges: GraphEdge[] };

export interface GraphLimits {
  max_nodes?: number;
  items_per_concept?: number;
  rules_per_concept?: number;
}

export interface ReportEntry {
  name: string;
  path: string;
  present: boolean;
  text: string;
}

export interface ValidationSummary {
  errors: number;
  warnings: number;
  export_dir: string;
}

export interface AuthoredDraftListing {
  rel_path: string;
  bytes: number;
}

export interface AuthoredDraft {
  text: string;
}

// ---------------------------------------------------------------------------
// §4.5 Review
// ---------------------------------------------------------------------------

export type EditFieldValue =
  | { kind: 'text'; value: string }
  | { kind: 'confidence'; value: number };

// ---------------------------------------------------------------------------
// §4.6 Agent
// ---------------------------------------------------------------------------

export interface AgentTask {
  id: string;
  title: string;
  description: string;
  matching_knowledge_count: number;
}

/**
 * `'azure-llm-tools'` (`sopkb_agent::react`) is the same model call as `'azure-llm'`
 * with a bounded tool-use back-and-forth first — the model may look up specific
 * knowledge/evidence/relations before answering, up to `MAX_REACT_ITERATIONS`
 * round trips, rather than being handed a fixed context bundle up front and asked
 * to answer in one shot. There is no Python equivalent; see that module's own doc
 * comment for why it's a separate path rather than a mode of `'azure-llm'` itself.
 */
export type AgentProvider = 'azure-llm' | 'azure-llm-tools' | 'context';

export interface AgentRequest {
  scenario: string;
  /** Defaults to "auto". */
  task_id: string;
  provider: AgentProvider;
  profile_id?: string;
  /**
   * ADVISORY ONLY. Filters nothing on the backend — it is embedded in the
   * prompt payload, echoed back, and referenced by one sentence of the
   * system prompt. Never build a UI affordance implying this excludes or
   * hides anything from retrieval; label it accordingly wherever it
   * appears (see `AgentScreen`).
   */
  allow_proposed: boolean;
  /**
   * Generated client-side (`crypto.randomUUID()`) once per chat, sent on
   * every turn within it — this is what gives the desktop app real per-chat
   * memory (`sopkb_agent::handle_chat_turn` folds this chat's own prior
   * turns into the outgoing request) and what the Agent screen's chat list
   * groups by. Desktop-only: web mode's `sopkb-server` route ignores this
   * field entirely (serde drops unknown JSON keys), so it still answers
   * single-shot with no memory there.
   */
  chat_id: string;
}

export interface DetectedConcept {
  id: string;
  label: string;
  score?: number;
  match_reasons: string[];
}

export interface AgentContextSummary {
  usable_knowledge_count: number;
  decision_rule_count: number;
  relation_count: number;
  evidence_count: number;
  concept_count: number;
  detected_concept_count: number;
  excluded_knowledge_count: number;
}

export interface AgentToolCallTrace {
  tool: string;
  args: Record<string, unknown>;
  result: unknown;
}

export interface AgentEntry {
  created_at: string;
  provider: AgentProvider;
  profile_id?: string;
  task_id: string;
  task_title: string;
  scenario: string;
  allow_proposed: boolean;
  detected_concepts: DetectedConcept[];
  context_summary: AgentContextSummary;
  answer: string;
  /** Only present for `provider: 'azure-llm-tools'` entries. */
  trace?: AgentToolCallTrace[];
  tool_iterations?: number;
  /**
   * Absent on entries written before round 6's per-chat memory feature —
   * the Agent screen's chat-grouping groups every such entry into one
   * shared "Unfiled" bucket rather than crashing on a missing id.
   */
  chat_id?: string;
}

export interface AgentContextRetrieval {
  mode: string;
  [key: string]: unknown;
}

export interface AgentContext {
  usable_knowledge: KnowledgeItem[];
  knowledge_relations: Relation[];
  evidence: Evidence[];
  decision_rules: DecisionRule[];
  concepts: Concept[];
  detected_concepts?: DetectedConcept[];
  retrieval?: AgentContextRetrieval;
  [key: string]: unknown;
}

// ---------------------------------------------------------------------------
// §4.7 Export
// ---------------------------------------------------------------------------

export interface SyncOkfDocumentsResult {
  type: 'okf_native';
  path: '.';
}

// ---------------------------------------------------------------------------
// §4.8 Settings — global, never bundle-scoped
// ---------------------------------------------------------------------------

/**
 * NEVER contains the raw API key — `has_api_key` only. A component holding a
 * `ProfileView` must never be able to render, log, or otherwise leak a key,
 * because the type itself doesn't carry one.
 */
export interface ProfileView {
  id: string;
  name: string;
  base_url: string;
  auth_style: string;
  model: string;
  max_output_tokens: number;
  timeout_seconds: number;
  reasoning_effort: string;
  mining_prompt: string;
  chat_prompt: string;
  has_api_key: boolean;
  is_default: boolean;
}

export interface ProfileInput {
  id?: string;
  name: string;
  base_url: string;
  auth_style: string;
  model: string;
  max_output_tokens: number;
  timeout_seconds: number;
  reasoning_effort: string;
  mining_prompt: string;
  chat_prompt: string;
  /** Absent or blank = "keep existing key". Never round-tripped from `get_settings`. */
  api_key?: string;
}

export interface EnvOverride {
  field: string;
  env_var: string;
  active_value_present: boolean;
}

/**
 * The built-in prompts each pipeline step sends to the LLM. `mining_prompt`/
 * `chat_prompt` are the DEFAULTS a blank per-profile override falls back to (see
 * `ProfileView`). `heading_index_prompt`/`heading_relevel_prompt` (the Normalize
 * step's own two LLM calls) have no per-profile override at all -- shown here
 * purely for visibility into what actually gets sent, not because they're
 * user-editable.
 */
export interface DefaultPrompts {
  mining_prompt: string;
  chat_prompt: string;
  heading_index_prompt: string;
  heading_relevel_prompt: string;
}

export interface SettingsView {
  profiles: ProfileView[];
  default_profile_id?: string;
  reviewer_name: string;
  settings_path: string;
  env_overrides: EnvOverride[];
  max_parallel_workers: number;
}

/**
 * Per-BUNDLE `mining_prompt`/`chat_prompt` overrides -- distinct from the global,
 * per-PROFILE overrides in `ProfileView`. A non-blank bundle override wins over
 * even a non-blank profile override; a blank value here means "use the profile's
 * (or built-in) prompt". Local to one bundle, unlike everything else in
 * `SettingsView`.
 */
export interface BundlePromptOverrides {
  mining_prompt: string;
  chat_prompt: string;
}

export interface TestConnectionResult {
  ok: boolean;
  detail: string;
}

// ---------------------------------------------------------------------------
// §4.9 Relations
// ---------------------------------------------------------------------------

export interface RelationNeighborhood {
  node_id: string;
  relations: Relation[];
}

// ---------------------------------------------------------------------------
// §4.10 MCP
// ---------------------------------------------------------------------------

export interface McpInvocation {
  command: string;
  args: string[];
  enable_review_notes_flag: string;
  /** The exact name this bundle would be (or was) registered under in a host's MCP config -- see `entry_name_for` in `mcp.rs`. */
  entry_name: string;
}

/**
 * One-click MCP client configuration -- no Python equivalent. `method` is
 * `"config-file"` (this app edits the client's config JSON directly, currently
 * only Claude Desktop) or `"cli"` (this app shells out to that client's own
 * `mcp add`, currently Claude Code and Codex). `location` is the config file
 * path or resolved CLI binary path when `located` is true; `note` explains why
 * not, and what to do instead, when it's false.
 */
export interface McpClientTarget {
  id: string;
  label: string;
  method: 'config-file' | 'cli';
  located: boolean;
  location?: string;
  note?: string;
  /** `config-file` clients only: where that config file would go even when `located` is false, for a manual-setup snippet. Always absent for `cli` clients. */
  default_location_hint?: string;
}

/** `outcome`: `"configured"` | `"already_configured"` | `"not_found"`. */
export interface McpConfigureResult {
  outcome: 'configured' | 'already_configured' | 'not_found';
  message: string;
  backup_path?: string;
}

// ---------------------------------------------------------------------------
// §4.11 Events
// ---------------------------------------------------------------------------

export type BundleStateScope =
  | 'inventory'
  | 'sections'
  | 'items'
  | 'reviews'
  | 'exports'
  | 'okf';

export interface BundleStateChangedPayload {
  key: BundleKey;
  scope: BundleStateScope[];
}

export interface BundleIndexChangedPayload {
  bundles_root: string;
}

export interface IngestProgressPayload {
  step: string;
  // 'progress': an in-flight step reporting incremental status (e.g. "42/215
  // sections mined") while it is still running -- distinct from 'started'
  // (no progress detail yet) so a long-running network-bound step (mine,
  // normalize's LLM heading-restructuring) can show live movement instead of
  // sitting frozen on "Running..." for however long the whole step takes.
  // 'cancelled': this step never started because a cancel_ingest() request
  // was already in effect when the pipeline reached it -- cooperative
  // cancellation only stops work from STARTING, so a step already
  // `'started'`/`'progress'` when cancel_ingest() was called still finishes
  // normally and reports its real `'done'`/`'failed'` outcome, never
  // `'cancelled'`.
  status: 'started' | 'progress' | 'done' | 'failed' | 'cancelled';
  detail: string;
}

export interface AgentProgressPayload {
  phase: string;
  detail: string;
}

/** Event name -> payload type. Kept in one place so `subscribe()` stays type-safe. */
export interface SopkbEventMap {
  'workbench://context-changed': WorkbenchContext;
  'bundle://state-changed': BundleStateChangedPayload;
  'bundle://index-changed': BundleIndexChangedPayload;
  'ingest://progress': IngestProgressPayload;
  'agent://progress': AgentProgressPayload;
  'settings://changed': Record<string, never>;
}

export type SopkbEventName = keyof SopkbEventMap;
