/**
 * The one module every screen/component calls into for backend data.
 * Nothing outside `src/lib/` and `src/mock/` should import
 * `@tauri-apps/api` or `src/mock/api.mock.ts` directly.
 *
 * ---------------------------------------------------------------------------
 * Real backend vs. mock fallback
 * ---------------------------------------------------------------------------
 *
 * Every export below calls the real `desktop-tauri/src-tauri` command layer
 * via `invoke()`, e.g.:
 *
 *   export function get_workbench_context(): Promise<WorkbenchContext> {
 *     return call('get_workbench_context');
 *   }
 *
 * (`call()` is a thin wrapper around `invoke()` — see below.)
 *
 * `npm run dev` and `npm test` still need to work with no Tauri runtime
 * present (a plain browser tab / jsdom), so this module picks between the
 * real backend and `src/mock/api.mock.ts` using `isTauri()` from
 * `@tauri-apps/api/core` — the library's own official "is a Tauri IPC bridge
 * actually present in this webview" check (it reads `globalThis.isTauri`,
 * the flag Tauri's injected init script sets, NOT a Tauri-conf-mode or
 * `window.__TAURI_INTERNALS__` presence check we'd have to keep in sync with
 * the library ourselves).
 *
 * The rule this module enforces: the real desktop app must NEVER silently
 * fall back to the mock backend.
 *   - Inside a real Tauri webview, `isTauri()` is true, so the real branch is
 *     always taken, unconditionally — this is checked FIRST and short-
 *     circuits everything below, regardless of dev/prod mode.
 *   - Outside Tauri (`isTauri()` false), mock fallback is only permitted when
 *     `import.meta.env.DEV` is true — true for both `vite dev` (browser tab,
 *     no Tauri) and `vitest run` (confirmed empirically: Vitest sets
 *     `MODE=test`, and Vite's `DEV = mode !== 'production'`), false for
 *     `vite build`'s production bundle.
 *   - Outside Tauri AND NOT in dev/test mode (i.e. someone opened the
 *     production `dist/` build directly in a plain browser, or something is
 *     wrong with Tauri's injected globals in what should be the real app) —
 *     this throws loudly and immediately at module-load time instead of
 *     quietly mocking. A broken "real" deployment should fail fast and
 *     visibly, not degrade into fixture data with no indication anything is
 *     wrong.
 *
 * The `isTauri()` check + throw-or-fall-back decision itself lives in
 * `src/lib/runtime.ts` (as `USE_MOCK`), shared with `src/lib/events.ts` —
 * the six §4.11 events (see below) need the exact same real-vs-mock decision
 * for `listen()` vs. the in-memory pub/sub `src/mock/api.mock.ts` publishes
 * to, and factoring it out avoids a circular import (`api.ts` -> `mock` ->
 * `events.ts` -> would be -> `api.ts`).
 *
 * ---------------------------------------------------------------------------
 * Real mismatches found while wiring against the actual Rust command layer
 * ---------------------------------------------------------------------------
 *
 * `docs/port/PORT_PLAN.md` §4 is the frozen contract both `src/types/
 * commands.ts` and `desktop-tauri/src-tauri` were built against independently,
 * and command *names* line up exactly, but a handful of *shapes* drifted.
 * Each is translated right here (never in `commands.ts`, never by patching the
 * Rust side) so the rest of the frontend keeps talking to the contract as
 * documented:
 *
 * 1. `WireMode` (the Rust `mode` enum backing `WorkbenchContext`/
 *    `ResolveLaunchTargetResult`) is `#[serde(rename_all = "snake_case")]`,
 *    so it serializes as `"single_bundle" | "workbench_root" | "degraded"` —
 *    but `WorkbenchMode` here is PascalCase (`"SingleBundle" | "WorkbenchRoot"
 *    | "Degraded"`), and `App.tsx`/`AppShell.tsx` compare against the
 *    PascalCase form directly (`context.mode === 'Degraded'` gates the entire
 *    degraded-mode screen). Left untranslated, degraded mode would simply
 *    never trigger. Fixed by `fixWireMode()` below, applied to every command
 *    that returns a `mode` field.
 * 2. `edit_item`'s Rust `EditValue` is `#[serde(tag = "type", content =
 *    "value")]` (`{ type: "text"|"confidence", value }`), not `{ kind, value
 *    }` like this contract's `EditFieldValue`. Translated in `edit_item()`.
 * 3. `run_agent_chat`'s Rust `AgentRequest.allow_proposed_advisory` is the
 *    same field as this contract's `AgentRequest.allow_proposed`, renamed on
 *    the wire (the Rust doc comment explains the rename is deliberate, to
 *    make the "advisory only, filters nothing" property visible at the call
 *    site) — but the JSON *key* differs, so it still needs translating.
 *    Translated in `run_agent_chat()`.
 * 4. `ProfileView`/`ProfileInput.max_output_tokens` and `.timeout_seconds` are
 *    `number` in this contract but `String` on the Rust wire (`sopkb_config
 *    ::ModelProfile` stores them as strings). Translated in `get_settings()`/
 *    `save_profile()`.
 *
 * Two more observed but NOT translated (harmless, or nothing sensible to
 * translate to):
 *   - `deselect_bundle` returns the full `WireWorkbenchContext` on the real
 *     backend vs. `void` here — harmless, no caller uses the return value
 *     (`WorkbenchContext.tsx` always calls `refresh()` right after).
 *   - `WireBundleCard` flattens `Option<WireBundleSummary>` — when a bundle
 *     fails to load, `id`/`title`/`profile`/`status`/the two counts/
 *     `export_path` are entirely ABSENT from the JSON (not just falsy),
 *     despite `BundleCard extends BundleSummary` declaring them required.
 *     There's no sensible placeholder value to synthesize; screens must check
 *     `load_error` first, which the type's own doc comment already says.
 *   - `run_ingest_pipeline`'s Rust request type carries two extra optional
 *     fields (`uploaded_file_count`, `key`) this contract's `IngestRequest`
 *     doesn't declare; harmless since both are `Option` server-side.
 *   - `get_graph`'s `limits` parameter is accepted for wire-shape
 *     compatibility only — the Rust command's own comment says it's
 *     currently a no-op (the underlying traversal's caps are hardcoded).
 */
import { invoke } from '@tauri-apps/api/core';
import * as mock from '../mock/api.mock';
import { httpCall, uploadSourceFiles } from './httpClient';
import { USE_HTTP_BACKEND, USE_MOCK } from './runtime';
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
  IngestRequest,
  IngestResult,
  IngestSource,
  IngestPreviewResult,
  IngestRunStatus,
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
  WorkbenchMode,
} from '../types/commands';
import { SopkbApiError } from '../types/commands';

// ---------------------------------------------------------------------------
// Real-vs-mock selection: `USE_MOCK` (see `src/lib/runtime.ts` for the actual
// `isTauri()` check + dev/test-only fallback rule this enforces).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// invoke() wrapper: normalizes a rejected Tauri command into `SopkbApiError`
// ---------------------------------------------------------------------------
//
// A `#[tauri::command]` returning `Err(AppError)` rejects the JS promise with
// the raw deserialized `{ kind, message, detail? }` payload — a plain object,
// not an `Error`. Every screen's catch block does
// `err instanceof Error ? err.message : String(err)` (matching the mock
// backend's behavior, which always throws `SopkbApiError`), so a raw
// rejection object would print as `"[object Object]"` instead of the actual
// message. `call()` re-throws it as a `SopkbApiError` so real and mock errors
// behave identically for every existing call site.

function isSopkbError(value: unknown): value is SopkbError {
  return !!value && typeof value === 'object' && 'kind' in value && 'message' in value;
}

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (USE_HTTP_BACKEND) return httpCall<T>(cmd, args);
  return invoke<T>(cmd, args).catch((err: unknown) => {
    if (isSopkbError(err)) throw new SopkbApiError(err);
    throw err instanceof Error ? err : new Error(String(err));
  });
}

// ---------------------------------------------------------------------------
// Mismatch #1 adapter: WireMode (snake_case) -> WorkbenchMode (PascalCase)
// ---------------------------------------------------------------------------

const WIRE_MODE_TO_CONTRACT: Record<string, WorkbenchMode> = {
  single_bundle: 'SingleBundle',
  workbench_root: 'WorkbenchRoot',
  degraded: 'Degraded',
};

function fixWireMode<T extends { mode: string }>(wire: T): T & { mode: WorkbenchMode } {
  return { ...wire, mode: WIRE_MODE_TO_CONTRACT[wire.mode] ?? (wire.mode as WorkbenchMode) };
}

// ---------------------------------------------------------------------------
// Mismatch #4 adapter: ProfileView/ProfileInput numeric fields <-> wire strings
// ---------------------------------------------------------------------------

type WireProfileView = Omit<ProfileView, 'max_output_tokens' | 'timeout_seconds'> & {
  max_output_tokens: string;
  timeout_seconds: string;
};

type WireSettingsView = Omit<SettingsView, 'profiles' | 'default_profile_id'> & {
  profiles: WireProfileView[];
  default_profile_id: string;
};

function fromWireProfile(p: WireProfileView): ProfileView {
  return { ...p, max_output_tokens: Number(p.max_output_tokens), timeout_seconds: Number(p.timeout_seconds) };
}

function toWireProfileInput(input: ProfileInput): Record<string, unknown> {
  return { ...input, max_output_tokens: String(input.max_output_tokens), timeout_seconds: String(input.timeout_seconds) };
}

// ---------------------------------------------------------------------------
// §4.1 Workbench context and lifecycle
// ---------------------------------------------------------------------------

export function get_workbench_context(): Promise<WorkbenchContext> {
  if (USE_MOCK) return mock.get_workbench_context();
  return call<WorkbenchContext>('get_workbench_context').then(fixWireMode);
}

export function resolve_launch_target(path?: string): Promise<ResolveLaunchTargetResult> {
  if (USE_MOCK) return mock.resolve_launch_target(path);
  return call<ResolveLaunchTargetResult>('resolve_launch_target', { path }).then(fixWireMode);
}

export function set_workbench_root(path: string): Promise<WorkbenchContext> {
  if (USE_MOCK) return mock.set_workbench_root(path);
  return call<WorkbenchContext>('set_workbench_root', { path }).then(fixWireMode);
}

export function pick_workbench_folder(): Promise<string | null> {
  if (USE_MOCK) return mock.pick_workbench_folder();
  return call('pick_workbench_folder');
}

export function select_bundle(key: BundleKey): Promise<BundleSummary> {
  if (USE_MOCK) return mock.select_bundle(key);
  return call('select_bundle', { key });
}

export function deselect_bundle(): Promise<void> {
  if (USE_MOCK) return mock.deselect_bundle();
  // Real backend resolves with `WireWorkbenchContext`, not `void` — see header
  // comment; no caller uses the return value.
  return call<void>('deselect_bundle');
}

// ---------------------------------------------------------------------------
// §4.2 Bundle management
// ---------------------------------------------------------------------------

export function list_bundles(): Promise<BundleCard[]> {
  if (USE_MOCK) return mock.list_bundles();
  return call('list_bundles');
}

export function create_project(title: string): Promise<CreateProjectResult> {
  if (USE_MOCK) return mock.create_project(title);
  return call('create_project', { title });
}

export function describe_bundle(key?: BundleKey): Promise<BundleSummary> {
  if (USE_MOCK) return mock.describe_bundle(key);
  return call('describe_bundle', { key });
}

export function init_bundle(path: string, title?: string): Promise<BundleSummary> {
  if (USE_MOCK) return mock.init_bundle(path, title);
  return call('init_bundle', { path, title });
}

/** Permanently deletes a bundle directory. Irreversible -- caller must confirm first. */
export function delete_bundle(key: BundleKey): Promise<void> {
  if (USE_MOCK) return mock.delete_bundle(key);
  return call('delete_bundle', { key });
}

export function repair_bundle_dirs(key?: BundleKey): Promise<RepairBundleDirsResult> {
  if (USE_MOCK) return mock.repair_bundle_dirs(key);
  return call('repair_bundle_dirs', { key });
}

export function get_legacy_state_status(key?: BundleKey): Promise<LegacyStateStatusEntry[]> {
  if (USE_MOCK) return mock.get_legacy_state_status(key);
  return call('get_legacy_state_status', { key });
}

export function get_bundle_prompt_overrides(key?: BundleKey): Promise<BundlePromptOverrides> {
  if (USE_MOCK) return mock.get_bundle_prompt_overrides(key);
  return call('get_bundle_prompt_overrides', { key });
}

export function set_bundle_prompt_overrides(overrides: BundlePromptOverrides, key?: BundleKey): Promise<void> {
  if (USE_MOCK) return mock.set_bundle_prompt_overrides(overrides, key);
  return call('set_bundle_prompt_overrides', { overrides, key });
}

// ---------------------------------------------------------------------------
// §4.3 Ingest pipeline
// ---------------------------------------------------------------------------

export function pick_source_files(): Promise<string[]> {
  if (USE_MOCK) return mock.pick_source_files();
  return call('pick_source_files');
}

export function pick_source_folder(): Promise<string | null> {
  if (USE_MOCK) return mock.pick_source_folder();
  return call('pick_source_folder');
}

export function stage_source_files(paths: string[], mode: 'files' | 'folder', replace: boolean): Promise<StagedUpload> {
  if (USE_MOCK) return mock.stage_source_files(paths, mode, replace);
  return call('stage_source_files', { paths, mode, replace });
}

/**
 * Web-mode-only replacement for `pick_source_files` + `stage_source_files` (no
 * native file dialog exists in a browser) -- uploads real file contents via
 * `multipart/form-data` instead of passing local paths. Throws in Tauri/mock mode
 * (there is no reason to call this outside `USE_HTTP_BACKEND`; callers should check
 * `USE_HTTP_BACKEND` before offering this in the UI at all, same as
 * `IngestScreen.tsx` does).
 */
export function upload_source_files(files: File[]): Promise<{ staging_dir: string; file_count: number }> {
  if (!USE_HTTP_BACKEND) return Promise.reject(new Error('upload_source_files is only available in the web build'));
  return uploadSourceFiles(files);
}

export function get_staged_sources(): Promise<StagedUpload | null> {
  if (USE_MOCK) return mock.get_staged_sources();
  return call('get_staged_sources');
}

export function clear_staged_sources(): Promise<void> {
  if (USE_MOCK) return mock.clear_staged_sources();
  return call('clear_staged_sources');
}

/** Removes exactly one staged file (by its `StagedUpload.files` relative path), leaving the rest of the batch staged. */
export function remove_staged_source(relative_path: string): Promise<StagedUpload | null> {
  if (USE_MOCK) return mock.remove_staged_source(relative_path);
  return call('remove_staged_source', { relative_path });
}

export function run_ingest_pipeline(request: IngestRequest): Promise<IngestResult> {
  if (USE_MOCK) return mock.run_ingest_pipeline(request);
  return call('run_ingest_pipeline', { request });
}

/**
 * Cooperative cancellation of the CURRENTLY RUNNING `run_ingest_pipeline` call, if
 * any -- see the Rust `cancel_ingest` command's own doc comment. Not instant: it
 * stops the pipeline from starting its NEXT not-yet-started step (or, within
 * mine/normalize, its next not-yet-dispatched section/chunk), letting whatever
 * already started finish normally. Safe to call with nothing running.
 */
export function cancel_ingest(): Promise<void> {
  if (USE_MOCK) return mock.cancel_ingest();
  return call('cancel_ingest');
}

export function get_last_ingest_run(): Promise<IngestRunStatus | null> {
  if (USE_MOCK) return mock.get_last_ingest_run();
  return call<IngestRunStatus | null>('get_last_ingest_run');
}

/**
 * Read-only dry run of `run_ingest_pipeline`'s scan step — shows what would
 * change (path / classification / source id) without running anything.
 * Rust's `preview_ingest_pipeline` command takes `source`/`key` directly
 * (no full `IngestRequestWire`), unlike `run_ingest_pipeline`.
 */
export function preview_ingest_pipeline(source: IngestSource): Promise<IngestPreviewResult> {
  if (USE_MOCK) return mock.preview_ingest_pipeline(source);
  return call('preview_ingest_pipeline', { source });
}

export function scan_sources(source_dir: string, key?: BundleKey): Promise<ScanSourcesResult> {
  if (USE_MOCK) return mock.scan_sources(source_dir, key);
  return call('scan_sources', { source_dir, key });
}

export function normalize_sources(key?: BundleKey): Promise<NormalizeSourcesResult> {
  if (USE_MOCK) return mock.normalize_sources(key);
  return call('normalize_sources', { key });
}

export function mine_knowledge(provider: string, profile_id: string | undefined, key?: BundleKey): Promise<MineKnowledgeResult> {
  if (USE_MOCK) return mock.mine_knowledge(provider, profile_id, key);
  return call('mine_knowledge', { provider, profile_id, key });
}

export function validate_bundle(key?: BundleKey): Promise<ValidateBundleResult> {
  if (USE_MOCK) return mock.validate_bundle(key);
  return call('validate_bundle', { key });
}

// ---------------------------------------------------------------------------
// §4.4 Read / query
// ---------------------------------------------------------------------------

export function list_sources(): Promise<Source[]> {
  if (USE_MOCK) return mock.list_sources();
  return call('list_sources');
}

export function get_source(source_id: string): Promise<Source> {
  if (USE_MOCK) return mock.get_source(source_id);
  return call('get_source', { source_id });
}

/**
 * "Delete a source" is really "retire" it -- non-destructive, reversible in
 * principle, fully audit-logged (see `retire_source`'s own Rust doc comment).
 * Originals, normalized text, and evidence all survive; the source and any
 * still-active knowledge items derived from it are marked retired.
 */
export function retire_source(source_id: string, rationale: string, reviewer?: string): Promise<RetireSourceResult> {
  if (USE_MOCK) return mock.retire_source(source_id, rationale, reviewer);
  return call('retire_source', { source_id, rationale, reviewer });
}

export function get_normalized_text(source_id: string, max_chars?: number): Promise<NormalizedTextResult> {
  if (USE_MOCK) return mock.get_normalized_text(source_id, max_chars);
  return call('get_normalized_text', { source_id, max_chars });
}

export function get_source_stats(): Promise<SourceStats> {
  if (USE_MOCK) return mock.get_source_stats();
  return call('get_source_stats');
}

export function list_sections(): Promise<Section[]> {
  if (USE_MOCK) return mock.list_sections();
  return call('list_sections');
}

export function get_section(section_id: string): Promise<Section> {
  if (USE_MOCK) return mock.get_section(section_id);
  return call('get_section', { section_id });
}

export function list_knowledge_items(): Promise<KnowledgeItem[]> {
  if (USE_MOCK) return mock.list_knowledge_items();
  return call('list_knowledge_items');
}

export function get_knowledge_item(item_id: string): Promise<KnowledgeItem> {
  if (USE_MOCK) return mock.get_knowledge_item(item_id);
  return call('get_knowledge_item', { item_id });
}

export function search_knowledge(query: string): Promise<KnowledgeSearchHit[]> {
  if (USE_MOCK) return mock.search_knowledge(query);
  return call('search_knowledge', { query });
}

export function get_evidence(item_id: string): Promise<Evidence> {
  if (USE_MOCK) return mock.get_evidence(item_id);
  return call('get_evidence', { item_id });
}

export function resolve_citation(citation_id: string): Promise<Evidence> {
  if (USE_MOCK) return mock.resolve_citation(citation_id);
  return call('resolve_citation', { citation_id });
}

export function get_concept_index(): Promise<Concept[]> {
  if (USE_MOCK) return mock.get_concept_index();
  return call('get_concept_index');
}

export function get_concept_detail(concept_id: string): Promise<ConceptDetail> {
  if (USE_MOCK) return mock.get_concept_detail(concept_id);
  return call('get_concept_detail', { concept_id });
}

export function get_review_detail(item_id: string): Promise<import('../types/commands').ReviewDetail> {
  if (USE_MOCK) return mock.get_review_detail(item_id);
  return call('get_review_detail', { item_id });
}

export function get_graph(concept_id?: string, limits?: GraphLimits): Promise<GraphResult> {
  if (USE_MOCK) return mock.get_graph(concept_id, limits);
  return call('get_graph', { concept_id, limits });
}

export function get_reports(): Promise<ReportEntry[]> {
  if (USE_MOCK) return mock.get_reports();
  return call('get_reports');
}

export function get_validation_summary(): Promise<ValidationSummary> {
  if (USE_MOCK) return mock.get_validation_summary();
  return call('get_validation_summary');
}

export function get_conflicts_report(): Promise<string> {
  if (USE_MOCK) return mock.get_conflicts_report();
  return call('get_conflicts_report');
}

export function get_freshness_report(): Promise<string> {
  if (USE_MOCK) return mock.get_freshness_report();
  return call('get_freshness_report');
}

export function get_agent_guide(): Promise<string> {
  if (USE_MOCK) return mock.get_agent_guide();
  return call('get_agent_guide');
}

export function list_authored_drafts(): Promise<AuthoredDraftListing[]> {
  if (USE_MOCK) return mock.list_authored_drafts();
  return call('list_authored_drafts');
}

export function get_authored_draft(rel_path: string): Promise<AuthoredDraft> {
  if (USE_MOCK) return mock.get_authored_draft(rel_path);
  return call('get_authored_draft', { rel_path });
}

// ---------------------------------------------------------------------------
// §4.5 Review
// ---------------------------------------------------------------------------

export function approve_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  if (USE_MOCK) return mock.approve_item(item_id, rationale, reviewer);
  return call('approve_item', { item_id, rationale, reviewer });
}

export function reject_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  if (USE_MOCK) return mock.reject_item(item_id, rationale, reviewer);
  return call('reject_item', { item_id, rationale, reviewer });
}

export function defer_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  if (USE_MOCK) return mock.defer_item(item_id, rationale, reviewer);
  return call('defer_item', { item_id, rationale, reviewer });
}

export function comment_item(item_id: string, rationale: string, reviewer?: string): Promise<ReviewEvent> {
  if (USE_MOCK) return mock.comment_item(item_id, rationale, reviewer);
  return call('comment_item', { item_id, rationale, reviewer });
}

export function edit_item(
  item_id: string,
  field: string,
  value: EditFieldValue,
  rationale: string,
  reviewer?: string,
): Promise<ReviewEvent> {
  if (USE_MOCK) return mock.edit_item(item_id, field, value, rationale, reviewer);
  // Mismatch #2 (see header comment): Rust's `EditValue` is tagged on `type`,
  // not `kind`.
  const wireValue = { type: value.kind, value: value.value };
  return call('edit_item', { item_id, field, value: wireValue, rationale, reviewer });
}

export function list_review_events(item_id?: string): Promise<ReviewEvent[]> {
  if (USE_MOCK) return mock.list_review_events(item_id);
  return call('list_review_events', { item_id });
}

// ---------------------------------------------------------------------------
// §4.6 Agent
// ---------------------------------------------------------------------------

export function list_agent_tasks(): Promise<AgentTask[]> {
  if (USE_MOCK) return mock.list_agent_tasks();
  return call('list_agent_tasks');
}

export function get_agent_transcript(limit?: number): Promise<AgentEntry[]> {
  if (USE_MOCK) return mock.get_agent_transcript(limit);
  return call('get_agent_transcript', { limit });
}

export function run_agent_chat(request: AgentRequest): Promise<AgentEntry> {
  if (USE_MOCK) return mock.run_agent_chat(request);
  // Mismatch #3 (see header comment): Rust's field is `allow_proposed_advisory`.
  const { allow_proposed, ...rest } = request;
  return call('run_agent_chat', { request: { ...rest, allow_proposed_advisory: allow_proposed } });
}

export function get_task_context(task_id: string, include_rejected: boolean): Promise<AgentContext> {
  if (USE_MOCK) return mock.get_task_context(task_id, include_rejected);
  return call('get_task_context', { task_id, include_rejected });
}

export function get_scenario_context(
  scenario: string,
  task_id?: string,
  include_rejected = false,
  item_limit?: number,
): Promise<AgentContext> {
  if (USE_MOCK) return mock.get_scenario_context(scenario, task_id, include_rejected, item_limit);
  return call('get_scenario_context', { scenario, task_id, include_rejected, item_limit });
}

export function clear_agent_transcript(): Promise<void> {
  if (USE_MOCK) return mock.clear_agent_transcript();
  return call('clear_agent_transcript');
}

/** Returns the number of turns removed. See the Rust command's own doc comment. */
export function delete_agent_chat(chat_id: string): Promise<number> {
  if (USE_MOCK) return mock.delete_agent_chat(chat_id);
  return call('delete_agent_chat', { chat_id });
}

// ---------------------------------------------------------------------------
// §4.7 Export
// ---------------------------------------------------------------------------

/**
 * `export_bundle(formats)` (graph-json/rdf) was removed with the standalone
 * Export screen -- OKF sync already happens automatically on every ingest
 * run and review action, so there was no format left worth a user-facing
 * choice. `sync_okf_documents`/`get_export_dir`/`reveal_path` below are the
 * remaining "make sure the OKF tree is current" / "show me where it lives"
 * actions, now surfaced on Sources instead of a dedicated page.
 */
export function sync_okf_documents(key?: BundleKey): Promise<{ type: 'okf_native'; path: '.' }> {
  if (USE_MOCK) return mock.sync_okf_documents(key);
  return call('sync_okf_documents', { key });
}

export function get_export_dir(): Promise<string> {
  if (USE_MOCK) return mock.get_export_dir();
  return call('get_export_dir');
}

export function reveal_path(path: string): Promise<void> {
  if (USE_MOCK) return mock.reveal_path(path);
  return call('reveal_path', { path });
}

// ---------------------------------------------------------------------------
// §4.8 Settings
// ---------------------------------------------------------------------------

export function get_settings(): Promise<SettingsView> {
  if (USE_MOCK) return mock.get_settings();
  // Mismatch #4 (see header comment): wire profiles carry numeric fields as strings.
  return call<WireSettingsView>('get_settings').then((wire) => ({
    ...wire,
    profiles: wire.profiles.map(fromWireProfile),
    default_profile_id: wire.default_profile_id || undefined,
  }));
}

export function get_default_prompts(): Promise<DefaultPrompts> {
  if (USE_MOCK) return mock.get_default_prompts();
  return call<DefaultPrompts>('get_default_prompts');
}

export function save_profile(input: ProfileInput): Promise<ProfileView> {
  if (USE_MOCK) return mock.save_profile(input);
  return call<WireProfileView>('save_profile', { input: toWireProfileInput(input) }).then(fromWireProfile);
}

export function delete_profile(id: string): Promise<void> {
  if (USE_MOCK) return mock.delete_profile(id);
  return call('delete_profile', { id });
}

export function set_default_profile(id: string): Promise<void> {
  if (USE_MOCK) return mock.set_default_profile(id);
  return call('set_default_profile', { id });
}

export function set_reviewer_name(name: string): Promise<void> {
  if (USE_MOCK) return mock.set_reviewer_name(name);
  return call('set_reviewer_name', { name });
}

export function set_max_parallel_workers(value: number): Promise<void> {
  if (USE_MOCK) return mock.set_max_parallel_workers(value);
  return call('set_max_parallel_workers', { value });
}

export function test_profile_connection(id?: string): Promise<TestConnectionResult> {
  if (USE_MOCK) return mock.test_profile_connection(id);
  return call('test_profile_connection', { id });
}

// ---------------------------------------------------------------------------
// §4.9 Relations
// ---------------------------------------------------------------------------

export function search_relations(subject?: string, predicate?: string, object?: string): Promise<Relation[]> {
  if (USE_MOCK) return mock.search_relations(subject, predicate, object);
  return call('search_relations', { subject, predicate, object });
}

export function get_relation_neighborhood(node_id: string): Promise<RelationNeighborhood> {
  if (USE_MOCK) return mock.get_relation_neighborhood(node_id);
  return call('get_relation_neighborhood', { node_id });
}

// ---------------------------------------------------------------------------
// §4.10 MCP
// ---------------------------------------------------------------------------

export function get_mcp_invocation(key?: BundleKey): Promise<McpInvocation> {
  if (USE_MOCK) return mock.get_mcp_invocation(key);
  return call('get_mcp_invocation', { key });
}

/**
 * Reads the cached result of the background client-detection scan that runs once at
 * app startup -- never itself re-probes PATH/config directories, so this is safe to
 * call on every Settings mount. `null` means detection genuinely hasn't finished yet
 * (a narrow startup race); call `rescan_mcp_client_targets` to force a fresh scan.
 */
export function list_mcp_client_targets(): Promise<McpClientTarget[] | null> {
  if (USE_MOCK) return mock.list_mcp_client_targets();
  return call('list_mcp_client_targets');
}

/** Forces a fresh client-detection scan (the only thing allowed to re-probe PATH/config directories after startup) and updates the cache `list_mcp_client_targets` reads. Use for an explicit user-initiated "Rescan", never automatically. */
export function rescan_mcp_client_targets(): Promise<McpClientTarget[]> {
  if (USE_MOCK) return mock.rescan_mcp_client_targets();
  return call('rescan_mcp_client_targets');
}

/** Writes this bundle's MCP invocation into `client_id`'s own config. `force: true` only after the user has confirmed overwriting an existing same-named entry. */
export function configure_mcp_client(client_id: string, force: boolean, key?: BundleKey): Promise<McpConfigureResult> {
  if (USE_MOCK) return mock.configure_mcp_client(client_id, force, key);
  return call('configure_mcp_client', { client_id, force, key });
}

// ---------------------------------------------------------------------------
// Task #38: diagnostics export
// ---------------------------------------------------------------------------

/**
 * Writes a zip (redacted settings + startup log + the selected bundle's own
 * diagnostic state, if any) to a path the user picks via a native save dialog.
 * Returns the saved path, or `null` if the user cancelled the dialog -- a normal
 * outcome, not an error, matching every other picker command in this file.
 */
export function export_diagnostics_bundle(): Promise<string | null> {
  if (USE_MOCK) return mock.export_diagnostics_bundle();
  return call('export_diagnostics_bundle');
}

// ---------------------------------------------------------------------------
// TEMP DIAGNOSTIC (not part of the §4 command contract): a fire-and-forget
// ping so `main.tsx` can tell the Rust side "the webview finished
// bootstrapping" -- see `desktop-tauri/src-tauri/src/startup_log.rs` for why.
// No-op in mock mode; there's no backend there to log anything.
// ---------------------------------------------------------------------------

export function notify_frontend_ready(): void {
  if (USE_MOCK) return;
  call('frontend_ready').catch(() => {});
}
