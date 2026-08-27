/**
 * Event subscription layer, matching §4.11's event list.
 *
 * `subscribe()` is the only export screens/hooks should use — it has the
 * same shape `@tauri-apps/api/event`'s `listen()` does (name, handler,
 * returns an unsubscribe/unlisten function). Nothing else in the app should
 * import `@tauri-apps/api/event` directly.
 *
 * Two backends, picked once via `USE_MOCK` from `src/lib/runtime.ts` (the
 * same real-vs-mock decision `src/lib/api.ts` uses, factored out to avoid a
 * circular import — see that module's header comment):
 *
 *   - Real (`USE_MOCK` false): `subscribe()` calls the real
 *     `@tauri-apps/api/event`'s `listen()`, unwrapping `event.payload` before
 *     handing it to `handler`. `listen()` itself is async (it resolves to an
 *     `UnlistenFn` only once IPC registration completes), but every existing
 *     call site (`WorkbenchContext.tsx`, `IngestScreen.tsx`, `AgentScreen.tsx`,
 *     `src/lib/hooks.ts`) uses `subscribe()`'s return value directly as a
 *     `useEffect` cleanup function, which MUST be synchronous — React just
 *     silently ignores a returned Promise (no cleanup ever runs, no error).
 *     So `subscribe()` still returns a real function synchronously, that
 *     closes over the eventual real `unlisten`: if unsubscribe is called
 *     before `listen()` has resolved, it's deferred and applied the instant
 *     registration completes, rather than dropped.
 *   - Mock (`USE_MOCK` true): the original in-memory pub/sub, published to by
 *     `src/mock/api.mock.ts` after every mutation, standing in for Tauri's
 *     event system so `npm run dev`/`npm test` keep working with no Tauri
 *     runtime present.
 */
import { listen } from '@tauri-apps/api/event';
import type { SopkbEventMap, SopkbEventName } from '../types/commands';
import { getToken } from './httpClient';
import { USE_HTTP_BACKEND, USE_MOCK } from './runtime';

type Handler<K extends SopkbEventName> = (payload: SopkbEventMap[K]) => void;

const listeners = new Map<SopkbEventName, Set<Handler<SopkbEventName>>>();

/** Mock-mode publish. Called by `src/mock/store.ts`; not part of the public API screens use. */
export function publish<K extends SopkbEventName>(name: K, payload: SopkbEventMap[K]): void {
  const set = listeners.get(name);
  if (!set) return;
  // Copy before iterating: a handler may unsubscribe itself mid-dispatch.
  for (const handler of [...set]) {
    (handler as Handler<K>)(payload);
  }
}

/** Exported only so `events.test.ts` can exercise each backend directly, bypassing the module-load-time `USE_MOCK` gate. Not part of the public API — screens/hooks use `subscribe()`. */
export function subscribeReal<K extends SopkbEventName>(name: K, handler: Handler<K>): () => void {
  let unlistenFn: (() => void) | null = null;
  let cancelled = false;
  listen<SopkbEventMap[K]>(name, (event) => handler(event.payload)).then((unlisten) => {
    if (cancelled) {
      unlisten();
      return;
    }
    unlistenFn = unlisten;
  });
  return () => {
    cancelled = true;
    unlistenFn?.();
  };
}

/** Exported only for `events.test.ts`; see `subscribeReal`'s comment. */
export function subscribeMock<K extends SopkbEventName>(name: K, handler: Handler<K>): () => void {
  let set = listeners.get(name);
  if (!set) {
    set = new Set();
    listeners.set(name, set);
  }
  set.add(handler as Handler<SopkbEventName>);
  return () => {
    set!.delete(handler as Handler<SopkbEventName>);
  };
}

// ---------------------------------------------------------------------------
// Web-mode (`USE_HTTP_BACKEND`): one shared SSE connection to `sopkb-server`'s
// `GET /api/events` (see `sopkb-server/src/events.rs`), fanned out to every
// `subscribeHttp` caller. `EventSource` can't set an `Authorization` header,
// and the server requires a bearer token on every route -- so this reads the
// stream via `fetch()` + a manual `ReadableStream` reader instead, splitting
// on blank-line-delimited SSE records by hand, rather than the `EventSource`
// API.
// ---------------------------------------------------------------------------

const httpListeners = new Map<SopkbEventName, Set<Handler<SopkbEventName>>>();
let httpStreamStarted = false;

function dispatchHttp(topic: string, payload: unknown) {
  const set = httpListeners.get(topic as SopkbEventName);
  if (!set) return;
  for (const handler of [...set]) {
    (handler as Handler<SopkbEventName>)(payload as never);
  }
}

async function pumpSseStream(): Promise<void> {
  const token = getToken();
  const resp = await fetch('/api/events', { headers: token ? { Authorization: `Bearer ${token}` } : {} });
  if (!resp.ok || !resp.body) return;
  const reader = resp.body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    buffer += value;
    let sepIndex: number;
    // eslint-disable-next-line no-cond-assign
    while ((sepIndex = buffer.indexOf('\n\n')) !== -1) {
      const record = buffer.slice(0, sepIndex);
      buffer = buffer.slice(sepIndex + 2);
      const dataLine = record.split('\n').find((l) => l.startsWith('data:'));
      if (!dataLine) continue;
      try {
        const parsed = JSON.parse(dataLine.slice(5).trim());
        if (parsed && typeof parsed.topic === 'string') dispatchHttp(parsed.topic, parsed.payload);
      } catch {
        // Malformed/partial record -- drop it, the next one is unaffected.
      }
    }
  }
}

/** Reconnects with a short fixed backoff -- an SSE stream this app depends on for live UI updates should not just silently die on one dropped connection. */
function startHttpStreamOnce(): void {
  if (httpStreamStarted) return;
  httpStreamStarted = true;
  const loop = () => {
    pumpSseStream()
      .catch(() => {})
      .finally(() => setTimeout(loop, 2000));
  };
  loop();
}

export function subscribeHttp<K extends SopkbEventName>(name: K, handler: Handler<K>): () => void {
  startHttpStreamOnce();
  let set = httpListeners.get(name);
  if (!set) {
    set = new Set();
    httpListeners.set(name, set);
  }
  set.add(handler as Handler<SopkbEventName>);
  return () => {
    set!.delete(handler as Handler<SopkbEventName>);
  };
}

/**
 * Subscribe to a backend event. Returns an unsubscribe function, always
 * synchronously, regardless of which backend is active (see header comment).
 */
export function subscribe<K extends SopkbEventName>(name: K, handler: Handler<K>): () => void {
  if (USE_HTTP_BACKEND) return subscribeHttp(name, handler);
  return USE_MOCK ? subscribeMock(name, handler) : subscribeReal(name, handler);
}

/** Test-only: drop every mock-mode listener between test cases. */
export function __resetAllListeners(): void {
  listeners.clear();
}
