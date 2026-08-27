/**
 * In-memory mock backend. Stands in for the Rust command layer + on-disk
 * bundle state while that layer is being built in parallel
 * (`desktop-tauri/src-tauri/`, per the frozen §4 contract). Every mutation
 * here is the mock equivalent of a filesystem write in the real backend,
 * and every mutation publishes the same event(s) §4.11 says the real
 * backend emits, via `src/lib/events.ts`.
 *
 * `src/mock/api.mock.ts` is the only consumer of this module. Screens never
 * import `store` directly — they go through `src/lib/api.ts`.
 */
import type {
  AgentEntry,
  AgentTask,
  BundleCard,
  BundleKey,
  BundlePromptOverrides,
  BundleStateScope,
  Concept,
  DecisionRule,
  EnvOverride,
  IngestRunStatus,
  KnowledgeItem,
  McpClientTarget,
  McpConfigureResult,
  ProfileInput,
  ProfileView,
  Relation,
  ReviewAction,
  ReviewEvent,
  Section,
  Source,
  WorkbenchContext,
  WorkbenchMode,
} from '../types/commands';
import { publish } from '../lib/events';
import { deriveConcepts, deriveRelation, deriveRule } from './derive';
import * as glp1 from './fixtures/glp1';
import * as escalation from './fixtures/escalation';

export interface BundleRecord {
  key: BundleKey;
  id: string;
  title: string;
  profile: string;
  status: string;
  bundle_path: string;
  export_path: string;
  load_error?: string;
  created_at: string;
  sources: Source[];
  sections: Section[];
  items: KnowledgeItem[];
  reviews: ReviewEvent[];
  reportsMarkdown: Record<string, string>;
  agentTasks: AgentTask[];
  agentTranscript: AgentEntry[];
  promptOverrides: BundlePromptOverrides;
}

const MOCK_ROOT = 'C:\\Users\\demo\\KL4A Workbench';
const MOCK_SETTINGS_PATH = `${MOCK_ROOT}\\settings.json`;

function makeBundle(
  key: BundleKey,
  title: string,
  sources: Source[],
  sections: Section[],
  items: KnowledgeItem[],
  reviews: ReviewEvent[],
  reportsMarkdown: Record<string, string>,
  agentTasks: AgentTask[],
  // Fixed, deterministic fixture timestamps (never `new Date()`) so "last created"
  // sort-order tests are reproducible instead of depending on wall-clock time.
  createdAt = '2026-01-01T00:00:00Z',
): BundleRecord {
  return {
    key,
    id: 'bundle',
    title,
    profile: 'sop-knowledge-bundle',
    status: 'draft',
    bundle_path: `${MOCK_ROOT}\\bundles\\${key}`,
    export_path: `${MOCK_ROOT}\\exports\\${key}`,
    created_at: createdAt,
    sources,
    sections,
    items,
    reviews,
    reportsMarkdown,
    agentTasks,
    agentTranscript: [],
    promptOverrides: { mining_prompt: '', chat_prompt: '' },
  };
}

class MockStore {
  root = MOCK_ROOT;
  mode: WorkbenchMode = 'WorkbenchRoot';
  bundlesRoot = `${MOCK_ROOT}\\bundles`;
  selectedBundle: BundleKey | undefined = glp1.BUNDLE_KEY;
  generation = 1;
  contextError: string | undefined;

  bundles = new Map<BundleKey, BundleRecord>([
    [
      glp1.BUNDLE_KEY,
      makeBundle(
        glp1.BUNDLE_KEY,
        glp1.BUNDLE_TITLE,
        // Shallow copy so mutations (e.g. retire_source's `source.status = ...`)
        // don't leak into the shared fixture module object across test resets --
        // `items`/`reviews` already got this treatment; `sources`/`sections` had
        // been missed, which was a real, reproduced test-isolation bug (a source
        // retired in one test stayed retired in the next, since `__resetStoreForTests`
        // only rebuilds the bundle wrapper, not these long-lived element objects).
        glp1.sources.map((s) => ({ ...s })),
        glp1.sections.map((s) => ({ ...s })),
        glp1.items.map((i) => ({ ...i })),
        [],
        glp1.reportsMarkdown,
        glp1.agentTasks,
        '2026-01-01T00:00:00Z',
      ),
    ],
    [
      escalation.BUNDLE_KEY,
      makeBundle(
        escalation.BUNDLE_KEY,
        escalation.BUNDLE_TITLE,
        escalation.sources.map((s) => ({ ...s })),
        escalation.sections.map((s) => ({ ...s })),
        escalation.items.map((i) => ({ ...i })),
        escalation.reviews.map((r) => ({ ...r })),
        escalation.reportsMarkdown,
        escalation.agentTasks,
        '2026-01-15T00:00:00Z',
      ),
    ],
  ]);

  /** A bundle directory that exists but fails to load — exercises §4.2's `load_error` requirement. */
  brokenBundleKey = 'legacy-intake-notes';
  brokenBundleError =
    'manifest.yaml at line 3: invalid profile_version "0.1" (expected a semantic version) — this bundle predates the current profile schema';

  stagedUpload: { staging_dir: string; file_count: number; skipped: { path: string; reason: string }[]; files: string[] } | null =
    null;

  lastIngestRun: IngestRunStatus | null = null;

  /**
   * Mirrors the real backend's cooperative `cancel_ingest` flag
   * (`WorkbenchHandle::cancel_requested`) -- `true` once `cancel_ingest()` has
   * been called for the currently in-flight (or next) `run_ingest_pipeline`
   * mock run. Cleared at the start of `run_ingest_pipeline` itself, same as
   * the real `clear_cancel_request()` call, so a stale cancellation from a
   * PREVIOUS run never dead-on-arrival kills a new one.
   */
  ingestCancelRequested = false;

  settingsProfiles: ProfileView[] = [
    {
      id: 'profile-default',
      name: 'Azure OpenAI (default)',
      base_url: 'https://example-resource.openai.azure.com',
      auth_style: 'api-key',
      model: 'gpt-4o-mini',
      max_output_tokens: 2048,
      timeout_seconds: 60,
      reasoning_effort: 'medium',
      mining_prompt: '',
      chat_prompt: '',
      has_api_key: true,
      is_default: true,
    },
  ];
  defaultProfileId: string | undefined = 'profile-default';
  reviewerName = 'local:user';
  maxParallelWorkers = 6;
  envOverrides: EnvOverride[] = [
    { field: 'base_url', env_var: 'SOPKB_AZURE_BASE_URL', active_value_present: false },
    { field: 'api_key', env_var: 'SOPKB_AZURE_API_KEY', active_value_present: true },
  ];

  // ---------------------------------------------------------------------
  // Workbench context / lifecycle
  // ---------------------------------------------------------------------

  getContext(): WorkbenchContext {
    return {
      root: this.root,
      mode: this.mode,
      bundles_root: this.bundlesRoot,
      selected_bundle: this.selectedBundle,
      generation: this.generation,
      settings_path: MOCK_SETTINGS_PATH,
      error: this.contextError,
    };
  }

  /**
   * Mock stand-in for a native folder picker + `set_workbench_root`. A path
   * containing "broken" or "missing" simulates an unreadable/malformed
   * root so the degraded screen is reachable without a real filesystem —
   * type such a path into the "switch workbench root" field to see it.
   */
  setWorkbenchRoot(path: string): WorkbenchContext {
    this.generation += 1;
    if (/broken|missing|does-not-exist/i.test(path)) {
      this.mode = 'Degraded';
      this.contextError = `could not read workbench root "${path}": directory does not exist or is not writable`;
      this.selectedBundle = undefined;
    } else {
      this.root = path;
      this.bundlesRoot = `${path}\\bundles`;
      this.mode = 'WorkbenchRoot';
      this.contextError = undefined;
      this.selectedBundle = undefined;
    }
    const ctx = this.getContext();
    publish('workbench://context-changed', ctx);
    return ctx;
  }

  recoverFromDegraded(): WorkbenchContext {
    return this.setWorkbenchRoot(MOCK_ROOT);
  }

  selectBundle(key: BundleKey): BundleCard {
    const bundle = this.bundles.get(key);
    if (!bundle) {
      throw notFound(`no bundle with key "${key}"`);
    }
    this.selectedBundle = key;
    publish('workbench://context-changed', this.getContext());
    return this.bundleCard(bundle);
  }

  deselectBundle(): void {
    this.selectedBundle = undefined;
    publish('workbench://context-changed', this.getContext());
  }

  // ---------------------------------------------------------------------
  // Bundle management
  // ---------------------------------------------------------------------

  requireBundle(key?: BundleKey): BundleRecord {
    const resolvedKey = key ?? this.selectedBundle;
    if (!resolvedKey) {
      throw invalidInput('no bundle selected and no key override was given');
    }
    const bundle = this.bundles.get(resolvedKey);
    if (!bundle) {
      throw notFound(`no bundle with key "${resolvedKey}"`);
    }
    return bundle;
  }

  bundleCard(bundle: BundleRecord): BundleCard {
    return {
      key: bundle.key,
      id: bundle.id,
      title: bundle.title,
      profile: bundle.profile,
      status: bundle.status,
      source_count: bundle.sources.length,
      knowledge_item_count: bundle.items.length,
      bundle_path: bundle.bundle_path,
      export_path: bundle.export_path,
      load_error: bundle.load_error,
      created_at: bundle.created_at,
    };
  }

  listBundles(): BundleCard[] {
    const cards = [...this.bundles.values()].map((b) => this.bundleCard(b));
    cards.push({
      key: this.brokenBundleKey,
      id: this.brokenBundleKey,
      title: this.brokenBundleKey,
      profile: 'unknown',
      status: 'unknown',
      source_count: 0,
      knowledge_item_count: 0,
      bundle_path: `${this.bundlesRoot}\\${this.brokenBundleKey}`,
      export_path: `${MOCK_ROOT}\\exports\\${this.brokenBundleKey}`,
      load_error: this.brokenBundleError,
      created_at: '2026-01-10T00:00:00Z',
    });
    return cards;
  }

  /** Irreversible -- deselects first if the deleted bundle was the selected one. */
  deleteBundle(key: BundleKey): void {
    if (key === this.brokenBundleKey) {
      throw notFound(`no bundle with key "${key}"`);
    }
    if (!this.bundles.has(key)) {
      throw notFound(`no bundle with key "${key}"`);
    }
    this.bundles.delete(key);
    if (this.selectedBundle === key) {
      this.selectedBundle = undefined;
    }
    publish('bundle://index-changed', { bundles_root: this.bundlesRoot });
    publish('workbench://context-changed', this.getContext());
  }

  createProject(title: string): { bundle: BundleCard; already_existed: boolean; okf_synced: boolean } {
    const key = title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '') || `project-${this.bundles.size + 1}`;
    const alreadyExisted = this.bundles.has(key);
    if (!alreadyExisted) {
      this.bundles.set(
        key,
        makeBundle(key, title, [], [], [], [], {
          extraction_summary: '# Extraction Summary\n\nNo sources ingested yet.\n',
          validation: '# Validation\n\nNo errors.\nNo warnings.\n',
          conflicts: '# Conflicts\n\nNone.\n',
          freshness: '# Freshness\n\nNo sources yet.\n',
          review_summary: '# Review Summary\n\nNo knowledge items yet.\n',
          agent_guide: '# Agent Guide\n\nThis bundle has no ingested content yet.\n',
        }, [], new Date().toISOString()),
      );
      publish('bundle://index-changed', { bundles_root: this.bundlesRoot });
    }
    const bundle = this.bundles.get(key)!;
    return { bundle: this.bundleCard(bundle), already_existed: alreadyExisted, okf_synced: true };
  }

  // ---------------------------------------------------------------------
  // Read/query helpers
  // ---------------------------------------------------------------------

  concepts(bundle: BundleRecord): { concepts: Concept[]; relations: Relation[]; rules: DecisionRule[] } {
    const relations = bundle.items.map((item) => deriveRelation(item));
    const rules = bundle.items.map((item) => {
      const condition =
        bundle.key === glp1.BUNDLE_KEY ? glp1.conditionsByItemId[item.id] : undefined;
      return deriveRule(item, { condition });
    });
    const concepts = deriveConcepts(bundle.items, rules);
    return { concepts, relations, rules };
  }

  // ---------------------------------------------------------------------
  // Review mutations
  // ---------------------------------------------------------------------

  allowedActions(item: KnowledgeItem): ReviewAction[] {
    const terminal = item.review_status === 'approved' || item.review_status === 'rejected';
    return terminal
      ? ['commented']
      : ['approved', 'rejected', 'deferred', 'edited', 'commented'];
  }

  private appendReview(
    bundle: BundleRecord,
    itemId: string,
    action: ReviewAction,
    rationale: string,
    reviewer: string | undefined,
    previousValue: unknown,
    newValue: unknown,
  ): ReviewEvent {
    const event: ReviewEvent = {
      id: `review-${String(bundle.reviews.length + 1).padStart(6, '0')}`,
      knowledge_item_id: itemId,
      action,
      reviewer: reviewer || this.reviewerName || 'local:user',
      timestamp: new Date().toISOString(),
      rationale,
      previous_value: previousValue,
      new_value: newValue,
    };
    bundle.reviews.push(event);
    return event;
  }

  private emitStateChanged(bundle: BundleRecord, scope: BundleStateScope[]): void {
    publish('bundle://state-changed', { key: bundle.key, scope });
  }

  private findItem(bundle: BundleRecord, itemId: string): KnowledgeItem {
    const item = bundle.items.find((i) => i.id === itemId);
    if (!item) throw notFound(`no knowledge item "${itemId}" in bundle "${bundle.key}"`);
    return item;
  }

  private ensureMutable(item: KnowledgeItem): void {
    if (item.review_status === 'approved' || item.review_status === 'rejected') {
      throw conflict(
        `item "${item.id}" is ${item.review_status} and can only be commented on`,
      );
    }
  }

  setReviewStatus(
    key: BundleKey | undefined,
    itemId: string,
    status: 'approved' | 'rejected' | 'deferred',
    action: ReviewAction,
    rationale: string,
    reviewer?: string,
  ): ReviewEvent {
    const bundle = this.requireBundle(key);
    const item = this.findItem(bundle, itemId);
    this.ensureMutable(item);
    const previous = { review_status: item.review_status };
    item.review_status = status;
    const event = this.appendReview(bundle, itemId, action, rationale, reviewer, previous, {
      review_status: status,
    });
    this.emitStateChanged(bundle, ['items', 'reviews', 'okf']);
    return event;
  }

  commentItem(key: BundleKey | undefined, itemId: string, rationale: string, reviewer?: string): ReviewEvent {
    const bundle = this.requireBundle(key);
    this.findItem(bundle, itemId); // validates existence; comment does not require mutability
    const event = this.appendReview(bundle, itemId, 'commented', rationale, reviewer, null, null);
    // Deliberately does not touch items.json-equivalent state — only the review log.
    this.emitStateChanged(bundle, ['reviews', 'okf']);
    return event;
  }

  editItem(
    key: BundleKey | undefined,
    itemId: string,
    field: string,
    value: { kind: 'text'; value: string } | { kind: 'confidence'; value: number },
    rationale: string,
    reviewer?: string,
  ): ReviewEvent {
    const TEXT_FIELDS = ['subject', 'predicate', 'object', 'source_text'] as const;
    const validField =
      (field === 'confidence' && value.kind === 'confidence') ||
      (TEXT_FIELDS.includes(field as (typeof TEXT_FIELDS)[number]) && value.kind === 'text');
    if (!validField) {
      // Field validated before any state is touched — mirrors the backend
      // guarantee that an invalid field never creates a reviews entry.
      throw invalidInput(
        `field "${field}" is not editable with a ${value.kind} value`,
      );
    }
    const bundle = this.requireBundle(key);
    const item = this.findItem(bundle, itemId);
    this.ensureMutable(item);
    const previous = { [field]: (item as unknown as Record<string, unknown>)[field] };
    (item as unknown as Record<string, unknown>)[field] = value.value;
    item.review_status = 'edited';
    const event = this.appendReview(bundle, itemId, 'edited', rationale, reviewer, previous, {
      [field]: value.value,
    });
    this.emitStateChanged(bundle, ['items', 'reviews', 'okf']);
    return event;
  }

  listReviewEvents(key: BundleKey | undefined, itemId?: string): ReviewEvent[] {
    const bundle = this.requireBundle(key);
    return itemId ? bundle.reviews.filter((r) => r.knowledge_item_id === itemId) : bundle.reviews;
  }

  // ---------------------------------------------------------------------
  // Agent
  // ---------------------------------------------------------------------

  appendAgentEntry(key: BundleKey | undefined, entry: AgentEntry): AgentEntry {
    const bundle = this.requireBundle(key);
    bundle.agentTranscript.push(entry);
    // Desktop's own chat-turn path caps at the last 1000 entries, not the 25-entry
    // Python-parity cap the non-chat path still uses (see sopkb-agent's
    // `chat::CHAT_TRANSCRIPT_CAP` doc comment) — a real multi-chat history needs far
    // more room than 25 turns shared across every chat combined.
    if (bundle.agentTranscript.length > 1000) {
      bundle.agentTranscript = bundle.agentTranscript.slice(-1000);
    }
    return entry;
  }

  clearAgentTranscript(key: BundleKey | undefined): void {
    const bundle = this.requireBundle(key);
    bundle.agentTranscript = [];
  }

  /** Returns the number of turns removed, matching the real `delete_agent_chat` command. */
  deleteAgentChat(key: BundleKey | undefined, chatId: string): number {
    const bundle = this.requireBundle(key);
    const before = bundle.agentTranscript.length;
    bundle.agentTranscript = bundle.agentTranscript.filter((e) => e.chat_id !== chatId);
    return before - bundle.agentTranscript.length;
  }

  // ---------------------------------------------------------------------
  // Bundle-specific prompt overrides
  // ---------------------------------------------------------------------

  getBundlePromptOverrides(key: BundleKey | undefined): BundlePromptOverrides {
    return this.requireBundle(key).promptOverrides;
  }

  setBundlePromptOverrides(overrides: BundlePromptOverrides, key: BundleKey | undefined): void {
    this.requireBundle(key).promptOverrides = overrides;
  }

  // ---------------------------------------------------------------------
  // One-click MCP client configuration
  // ---------------------------------------------------------------------

  /** `"<clientId>:<bundleKey>"` pairs already "configured" this session -- deterministic stand-in for a real external config file. */
  configuredMcpClients = new Set<string>();

  /** Lets a test simulate "Rescan found it this time" without a full remount -- flip this, then call listMcpClientTargets()/rescanMcpClientTargets() again. */
  mcpCodexLocated = false;

  listMcpClientTargets(): McpClientTarget[] {
    return [
      {
        id: 'claude-desktop',
        label: 'Claude Desktop',
        method: 'config-file',
        located: true,
        location: `${MOCK_ROOT}\\Claude\\claude_desktop_config.json`,
        default_location_hint: `${MOCK_ROOT}\\Claude\\claude_desktop_config.json`,
      },
      { id: 'claude-code', label: 'Claude Code', method: 'cli', located: true, location: 'claude' },
      this.mcpCodexLocated
        ? { id: 'codex', label: 'Codex (CLI & Desktop)', method: 'cli', located: true, location: 'codex' }
        : { id: 'codex', label: 'Codex (CLI & Desktop)', method: 'cli', located: false, note: 'Could not find the `codex` CLI on PATH.' },
    ];
  }

  configureMcpClient(clientId: string, key: BundleKey | undefined, force: boolean): McpConfigureResult {
    const bundle = this.requireBundle(key);
    const target = this.listMcpClientTargets().find((t) => t.id === clientId);
    if (!target || !target.located) {
      return { outcome: 'not_found', message: `Could not find ${target?.label ?? clientId} automatically.` };
    }
    const entryKey = `${clientId}:${bundle.key}`;
    if (this.configuredMcpClients.has(entryKey) && !force) {
      return { outcome: 'already_configured', message: `An MCP server named "sopkb-${bundle.key}" is already configured for ${target.label}.` };
    }
    this.configuredMcpClients.add(entryKey);
    return { outcome: 'configured', message: `Added "sopkb-${bundle.key}" to ${target.label}.` };
  }

  // ---------------------------------------------------------------------
  // Settings
  // ---------------------------------------------------------------------

  saveProfile(input: ProfileInput): ProfileView {
    const id = input.id ?? `profile-${this.settingsProfiles.length + 1}`;
    const existingIndex = this.settingsProfiles.findIndex((p) => p.id === id);
    const hasApiKey =
      existingIndex >= 0
        ? Boolean(input.api_key) || this.settingsProfiles[existingIndex].has_api_key
        : Boolean(input.api_key);
    const view: ProfileView = {
      id,
      name: input.name,
      base_url: input.base_url,
      auth_style: input.auth_style,
      model: input.model,
      max_output_tokens: input.max_output_tokens,
      timeout_seconds: input.timeout_seconds,
      reasoning_effort: input.reasoning_effort,
      mining_prompt: input.mining_prompt,
      chat_prompt: input.chat_prompt,
      has_api_key: hasApiKey,
      is_default: existingIndex >= 0 ? this.settingsProfiles[existingIndex].is_default : this.settingsProfiles.length === 0,
    };
    if (existingIndex >= 0) {
      this.settingsProfiles[existingIndex] = view;
    } else {
      this.settingsProfiles.push(view);
      if (view.is_default) this.defaultProfileId = view.id;
    }
    publish('settings://changed', {});
    return view;
  }

  deleteProfile(id: string): void {
    this.settingsProfiles = this.settingsProfiles.filter((p) => p.id !== id);
    if (this.defaultProfileId === id) {
      this.defaultProfileId = this.settingsProfiles[0]?.id;
    }
    publish('settings://changed', {});
  }

  setDefaultProfile(id: string): void {
    const profile = this.settingsProfiles.find((p) => p.id === id);
    if (!profile) {
      throw notFound(`no profile with id "${id}"`);
    }
    this.settingsProfiles = this.settingsProfiles.map((p) => ({ ...p, is_default: p.id === id }));
    this.defaultProfileId = id;
    publish('settings://changed', {});
  }

  setReviewerName(name: string): void {
    this.reviewerName = name;
    publish('settings://changed', {});
  }
}

export function notFound(message: string) {
  return { kind: 'NotFound' as const, message };
}
export function invalidInput(message: string) {
  return { kind: 'InvalidInput' as const, message };
}
export function conflict(message: string) {
  return { kind: 'Conflict' as const, message };
}
export function missingConfiguration(message: string) {
  return { kind: 'MissingConfiguration' as const, message };
}

export const store = new MockStore();

/**
 * Test-only: restore the singleton to a fresh instance's state in place.
 * `store` is a shared module-level singleton (screens across the whole app
 * import the same one), so tests that mutate it need a way to undo that
 * between cases without every test file getting its own module registry.
 */
export function __resetStoreForTests(): void {
  Object.assign(store, new MockStore());
}
