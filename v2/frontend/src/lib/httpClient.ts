/**
 * Web-mode backend: talks to `sopkb-server` (`v2/sopkb-rust/bin/sopkb-server`)
 * over `fetch()`/SSE instead of Tauri IPC. Only active when
 * `src/lib/runtime.ts`'s `USE_HTTP_BACKEND` is true (the `npm run build:web`
 * build, not the desktop app and not `vite dev`/`vitest run`).
 *
 * Coverage is NOT 1:1 with the full Tauri command surface -- see
 * `docs/port/CATCHUP_PLAN.md` for the exact list of what has an HTTP
 * equivalent today. A command with no entry in `ENDPOINTS` below throws a
 * clear "not supported in web mode yet" error rather than silently no-op'ing.
 */
import { SopkbApiError } from '../types/commands';

const TOKEN_STORAGE_KEY = 'sopkb.serverToken';

/**
 * The server prints a `?token=<...>` URL on startup for exactly this: open it
 * once, the token is picked up from the query string and saved to
 * `localStorage`, then stripped from the visible URL (a bearer token has no
 * business sitting in browser history/the `Referer` header on every
 * subsequent same-origin navigation).
 */
function bootstrapTokenFromUrl(): void {
  const params = new URLSearchParams(window.location.search);
  const fromUrl = params.get('token');
  if (!fromUrl) return;
  window.localStorage.setItem(TOKEN_STORAGE_KEY, fromUrl);
  params.delete('token');
  const rest = params.toString();
  const newUrl = window.location.pathname + (rest ? `?${rest}` : '') + window.location.hash;
  window.history.replaceState(null, '', newUrl);
}

export function getToken(): string | null {
  if (typeof window === 'undefined') return null;
  bootstrapTokenFromUrl();
  return window.localStorage.getItem(TOKEN_STORAGE_KEY);
}

export function setToken(token: string): void {
  window.localStorage.setItem(TOKEN_STORAGE_KEY, token);
}

function authHeaders(): HeadersInit {
  const token = getToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

async function throwIfNotOk(resp: Response): Promise<void> {
  if (resp.ok) return;
  const contentType = resp.headers.get('content-type') ?? '';
  if (contentType.includes('application/json')) {
    const body = await resp.json().catch(() => null);
    if (body && typeof body === 'object' && 'kind' in body && 'message' in body) {
      throw new SopkbApiError(body);
    }
  }
  throw new SopkbApiError({ kind: 'Io', message: `${resp.status} ${resp.statusText}` });
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const resp = await fetch(path, {
    method,
    headers: { ...authHeaders(), ...(body !== undefined ? { 'Content-Type': 'application/json' } : {}) },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  await throwIfNotOk(resp);
  if (resp.status === 204) return undefined as T;
  const text = await resp.text();
  if (!text) return undefined as T;
  const contentType = resp.headers.get('content-type') ?? '';
  return (contentType.includes('application/json') ? JSON.parse(text) : text) as T;
}

function query(params: Record<string, unknown>): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null || v === '') continue;
    q.set(k, String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : '';
}

type Args = Record<string, unknown>;

/** One entry per Tauri command name this backend can actually serve. */
const ENDPOINTS: Record<string, (args: Args) => Promise<unknown>> = {
  get_workbench_context: () => request('GET', '/api/context'),
  set_workbench_root: (a) => request('POST', '/api/workbench-root', { path: a.path }),
  select_bundle: (a) => request('POST', `/api/bundles/${encodeURIComponent(a.key as string)}/select`),
  deselect_bundle: () => request('POST', '/api/bundles/deselect'),

  list_bundles: () => request('GET', '/api/bundles'),
  create_project: (a) => request('POST', '/api/bundles', { title: a.title }),
  describe_bundle: (a) => request('GET', `/api/bundles/describe${query({ key: a.key })}`),
  delete_bundle: (a) => request('DELETE', `/api/bundles/${encodeURIComponent(a.key as string)}`),

  list_sources: (a) => request('GET', `/api/sources${query({ key: a.key })}`),
  get_source: (a) => request('GET', `/api/sources/${encodeURIComponent(a.source_id as string)}${query({ key: a.key })}`),
  get_source_stats: (a) => request('GET', `/api/source-stats${query({ key: a.key })}`),
  get_normalized_text: (a) =>
    request('GET', `/api/sources/${encodeURIComponent(a.source_id as string)}/normalized-text${query({ max_chars: a.max_chars, key: a.key })}`),
  get_staged_sources: (a) => request('GET', `/api/sources/staged${query({ key: a.key })}`),
  clear_staged_sources: (a) => request('DELETE', `/api/sources/staged${query({ key: a.key })}`),

  list_sections: (a) => request('GET', `/api/sections${query({ key: a.key })}`),
  get_section: (a) => request('GET', `/api/sections/${encodeURIComponent(a.section_id as string)}${query({ key: a.key })}`),

  list_knowledge_items: (a) => request('GET', `/api/knowledge${query({ key: a.key })}`),
  get_knowledge_item: (a) => request('GET', `/api/knowledge/${encodeURIComponent(a.item_id as string)}${query({ key: a.key })}`),
  search_knowledge: (a) => request('GET', `/api/knowledge/search${query({ q: a.query, key: a.key })}`),
  get_evidence: (a) => request('GET', `/api/evidence/${encodeURIComponent(a.item_id as string)}${query({ key: a.key })}`),
  resolve_citation: (a) => request('GET', `/api/citations/${encodeURIComponent(a.citation_id as string)}${query({ key: a.key })}`),
  get_conflicts_report: (a) => request('GET', `/api/conflicts-report${query({ key: a.key })}`),
  get_freshness_report: (a) => request('GET', `/api/freshness-report${query({ key: a.key })}`),
  get_agent_guide: (a) => request('GET', `/api/agent-guide${query({ key: a.key })}`),
  get_validation_summary: (a) => request('GET', `/api/validation-summary${query({ key: a.key })}`),
  get_concept_index: (a) => request('GET', `/api/concepts${query({ key: a.key })}`),
  get_concept_detail: (a) => request('GET', `/api/concepts/${encodeURIComponent(a.concept_id as string)}${query({ key: a.key })}`),
  get_reports: (a) => request('GET', `/api/reports${query({ key: a.key })}`),

  get_review_detail: (a) => request('GET', `/api/review/${encodeURIComponent(a.item_id as string)}${query({ key: a.key })}`),
  list_review_events: (a) => request('GET', `/api/review/${encodeURIComponent(a.item_id as string)}/events${query({ key: a.key })}`),
  approve_item: (a) => request('POST', `/api/review/${encodeURIComponent(a.item_id as string)}/approve`, { reviewer: a.reviewer, rationale: a.rationale, key: a.key }),
  reject_item: (a) => request('POST', `/api/review/${encodeURIComponent(a.item_id as string)}/reject`, { reviewer: a.reviewer, rationale: a.rationale, key: a.key }),
  defer_item: (a) => request('POST', `/api/review/${encodeURIComponent(a.item_id as string)}/defer`, { reviewer: a.reviewer, rationale: a.rationale, key: a.key }),
  comment_item: (a) => request('POST', `/api/review/${encodeURIComponent(a.item_id as string)}/comment`, { reviewer: a.reviewer, rationale: a.rationale, key: a.key }),
  edit_item: (a) =>
    request('PATCH', `/api/review/${encodeURIComponent(a.item_id as string)}/edit`, { field: a.field, value: a.value, reviewer: a.reviewer, rationale: a.rationale, key: a.key }),

  search_relations: (a) => request('GET', `/api/relations/search${query({ subject: a.subject, predicate: a.predicate, object: a.object, key: a.key })}`),
  get_relation_neighborhood: (a) => request('GET', `/api/relations/${encodeURIComponent(a.node_id as string)}/neighborhood${query({ key: a.key })}`),

  list_agent_tasks: (a) => request('GET', `/api/agent/tasks${query({ key: a.key })}`),
  get_agent_transcript: (a) => request('GET', `/api/agent/transcript${query({ limit: a.limit, key: a.key })}`),
  clear_agent_transcript: (a) => request('DELETE', `/api/agent/transcript${query({ key: a.key })}`),
  run_agent_chat: (a) => request('POST', '/api/agent/chat', (a.request as Args) ?? a),
  get_task_context: (a) => request('GET', `/api/agent/task-context${query({ task_id: a.task_id, include_rejected: a.include_rejected, key: a.key })}`),
  get_scenario_context: (a) =>
    request(
      'GET',
      `/api/agent/scenario-context${query({ scenario: a.scenario, task_id: a.task_id, include_rejected: a.include_rejected, item_limit: a.item_limit, key: a.key })}`,
    ),

  scan_sources: (a) => request('POST', '/api/ingest/scan', { source_dir: a.source_dir, key: a.key }),
  normalize_sources: (a) => request('POST', `/api/ingest/normalize${query({ key: a.key })}`),
  mine_knowledge: (a) => request('POST', '/api/ingest/mine', { provider: a.provider, profile_id: a.profile_id, key: a.key }),
  validate_bundle: (a) => request('POST', `/api/ingest/validate${query({ key: a.key })}`),
  preview_ingest_pipeline: (a) => request('POST', '/api/ingest/preview', { source: a.source, key: a.key }),
  run_ingest_pipeline: (a) => request('POST', '/api/ingest/run', a.request as Args),

  sync_okf_documents: (a) => request('POST', `/api/export/sync${query({ key: a.key })}`),
  get_export_dir: (a) => request('GET', `/api/export/dir${query({ key: a.key })}`),

  get_settings: () => request('GET', '/api/settings'),
  get_default_prompts: () => request('GET', '/api/settings/default-prompts'),
  get_mcp_invocation: (a) => request('GET', `/api/mcp/invocation${query({ key: a.key })}`),
};

/**
 * Web-mode's replacement for the native file/folder pickers (`pick_source_files`/
 * `pick_source_folder` have no browser equivalent -- see `docs/port/CATCHUP_PLAN.md`):
 * `multipart/form-data` upload straight into the bundle's staging area, hitting the
 * same `/api/sources/upload` endpoint `sopkb-server`'s `routes::bundles::upload_sources`
 * exposes. Not part of `ENDPOINTS`/`httpCall` since it needs `FormData`, not a JSON body.
 */
export async function uploadSourceFiles(files: File[], key?: string): Promise<{ staging_dir: string; file_count: number }> {
  const form = new FormData();
  for (const file of files) {
    form.append('file', file, file.name);
  }
  const resp = await fetch(`/api/sources/upload${query({ key })}`, { method: 'POST', headers: authHeaders(), body: form });
  await throwIfNotOk(resp);
  return resp.json();
}

/** `src/lib/api.ts`'s `call()` delegates here when `USE_HTTP_BACKEND` is true. */
export function httpCall<T>(cmd: string, args: Args = {}): Promise<T> {
  const endpoint = ENDPOINTS[cmd];
  if (!endpoint) {
    return Promise.reject(
      new SopkbApiError({ kind: 'InvalidInput', message: `"${cmd}" is not yet available in the web build -- see docs/port/CATCHUP_PLAN.md's coverage list.` }),
    );
  }
  return endpoint(args) as Promise<T>;
}
