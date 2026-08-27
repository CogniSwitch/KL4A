# frontend

React frontend for the SOP Knowledge Workbench desktop app, talking to the Rust backend
exclusively via Tauri `invoke()` commands and events (no HTTP, no sidecar). Replaces
`tools/sopkb/sopkb/web_app.py`'s server-rendered HTML entirely.

This directory lives under [`v2/`](../README.md) alongside its siblings `sopkb-rust/` (the pure
Rust backend) and `desktop-tauri/` (the Tauri shell) — see `v2/README.md` for the full layout.

Vite + React + TypeScript + Tailwind CSS. Built with `npm run build`, which outputs directly
into the sibling `../desktop-tauri/dist` — see `vite.config.ts`, whose `outDir` points there
because that's where `desktop-tauri/src-tauri/tauri.conf.json`'s `build.frontendDist` already
expects it.

## Status: wired to the real `invoke()` layer, with a dev/test-only mock fallback

Built in parallel with the actual `desktop-tauri/src-tauri` command layer against the frozen §4
command contract (`docs/port/PORT_PLAN.md` §6.11), originally backed entirely by an in-memory mock
"backend." That mock is **not** gone — it's still the dev/test-mode backend (see
`src/lib/runtime.ts`) — but every export in `src/lib/api.ts` now calls the real Tauri command via
`invoke()` first, falling back to the mock only when no Tauri runtime is present (`isTauri()` is
false) and the build is in dev/test mode; a non-Tauri production build refuses to fall back at all
and throws instead. `src/lib/events.ts`'s six §4.11 events are wired the same way, against
`@tauri-apps/api/event`'s `listen()`. See `src/lib/api.ts`'s header comment for the full rationale,
including four real shape mismatches found between this contract and the actual Rust wire format
(and translated in that file rather than in the contract or the Rust side).

### What's real vs. mocked

- **Real (always)**: all TypeScript types (`src/types/commands.ts`), all UI components and screens,
  the routing/shell/navigation, the review `allowed_actions` gating logic, the `get_graph`
  tagged-union discrimination, the settings API-key-never-rendered guarantee, the event-driven
  cache-invalidation pattern (`src/lib/hooks.ts`'s `useBundleInvalidation`).
- **Real when running inside `desktop-tauri`, mocked only in a plain browser tab / `vitest`**: every
  command in `src/lib/api.ts` and every event in `src/lib/events.ts`.
- **Always mocked (dev/test mode only)**: `src/mock/fixtures/*.ts` (adapted from
  `../sopkb-rust/fixtures/cases/{reference,reviewed}/expected-python/bundle/`), `src/mock/store.ts`'s
  in-memory mutation logic, the native file/folder dialogs (`pick_workbench_folder`,
  `pick_source_files`, `pick_source_folder` all return canned paths in mock mode), `reveal_path`
  (no-op in mock mode).

## Screens built

1. Shell, navigation, `WorkbenchContext`, degraded/first-run screen — `src/context/WorkbenchContext.tsx`,
   `src/components/layout/AppShell.tsx`, `src/screens/DegradedScreen.tsx`. Type a path containing
   "broken" or "missing" into the workbench-root switcher to reach the degraded screen without a
   real filesystem.
2. Settings — `src/screens/SettingsScreen.tsx`. Independent of bundle selection.
3. Bundle picker — `src/screens/BundlePickerScreen.tsx`. Surfaces `load_error` per card; the mock
   index always includes one broken bundle so this stays visibly true.
4. Read-only bundle screens: Sources, Viewer, Knowledge (with `search_knowledge` actually wired,
   unlike the original Python UI), Concepts + Concept detail, Relations, Reports, Agent
   (task list + transcript + the zero-config `context` provider path).
5. Review — `src/screens/ReviewScreen.tsx`. All five mutating actions, gated on
   `get_review_detail`'s `allowed_actions` rather than a client-side re-derivation of "terminal".
6. Graph (stretch) — `src/screens/GraphScreen.tsx`. Simple hand-rolled SVG circular layout, no
   external graph library.
7. Export (stretch) — `src/screens/ExportScreen.tsx`.
8. Ingest (stretch) — `src/screens/IngestScreen.tsx`. Staged uploads, step toggles, provider
   selection, `ingest://progress` events.

All eight from the build order got built this pass, 1-5 at full depth and 6-8 as the stretch
goals the brief called for.

## Development

```sh
npm install
npm run dev      # Vite dev server with HMR, mock data only
npm run build    # tsc -b && vite build -> ../desktop-tauri/dist
npm test         # vitest
```
