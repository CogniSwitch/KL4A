/**
 * Shared "which backend should `src/lib/api.ts` (commands) and
 * `src/lib/events.ts` (events) talk to" decision. Three modes, checked in
 * this order:
 *
 *   1. Real Tauri webview (`isTauri()`, the official `@tauri-apps/api/core`
 *      check reading the `globalThis.isTauri` flag Tauri's injected init
 *      script sets) — always wins when true. The desktop app never falls
 *      back to anything else.
 *   2. The web build (`npm run build:web`, which sets `VITE_BACKEND=web` —
 *      see `vite.config.ts`/`package.json`) talking to `sopkb-server` over
 *      HTTP/SSE instead of Tauri IPC. Distinguishing this from "someone
 *      opened the Tauri `dist/` build in a plain browser" matters: the
 *      latter is a broken deployment that should fail loudly (see below),
 *      not silently degrade to some other backend.
 *   3. Dev/test with no Tauri runtime and not the web build
 *      (`import.meta.env.DEV`, true for both `vite dev` and `vitest run`) —
 *      falls back to the in-memory mock backend.
 *
 * Anything outside those three (a non-Tauri, non-web-build, non-dev/test
 * load — e.g. the Tauri production bundle opened directly in a browser)
 * throws immediately instead of quietly mocking, so a broken "real"
 * deployment fails fast and visibly.
 */
import { isTauri } from '@tauri-apps/api/core';

export const REAL_BACKEND = isTauri();

/** Set only by `npm run build:web`'s `VITE_BACKEND=web` -- never true for the Tauri build or plain `vite dev`/`vitest run`. */
export const IS_WEB_BUILD = import.meta.env.VITE_BACKEND === 'web';

if (!REAL_BACKEND && !IS_WEB_BUILD && !import.meta.env.DEV) {
  throw new Error(
    'sopkb: no Tauri runtime detected (isTauri() is false) in what is not a dev/test or ' +
      'web build. Refusing to silently fall back to mock/in-memory behavior — if this is ' +
      "meant to be a real desktop-tauri build, something is wrong with the Tauri webview's " +
      "injected globals; if this is meant to be the web build, it must be built with " +
      "`npm run build:web`; if this is meant to be dev mode, run `npm run dev`.",
  );
}

/** True exactly when `src/lib/api.ts`/`src/lib/events.ts` should route through `fetch()`/SSE against `sopkb-server` instead of Tauri IPC. */
export const USE_HTTP_BACKEND = !REAL_BACKEND && IS_WEB_BUILD;

/** True exactly when they should route through the mock/in-memory implementations instead. */
export const USE_MOCK = !REAL_BACKEND && !IS_WEB_BUILD;
