/// <reference types="vitest/config" />
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// https://vite.dev/config/
//
// This frontend is consumed by the Tauri shell in the sibling `desktop-tauri/`
// directory -- both live under `v2/` (alongside `sopkb-rust/`, the pure Rust
// backend, and `v2/dist/`, the final packaged portable build output), kept
// clearly separate from the still-live, still-shipping Python implementation
// at `tools/sopkb/`. `desktop-tauri/src-tauri/tauri.conf.json` has
// `build.frontendDist` set to `../dist` (i.e. `v2/desktop-tauri/dist`,
// resolved relative to `v2/desktop-tauri/src-tauri/`), so this points our
// build output at the directory Tauri already expects -- unchanged by the
// `v2/` reorganization since `frontend/` and `desktop-tauri/` are still
// siblings, just under a differently-named parent. `emptyOutDir` wipes
// whatever stale build previously lived there.
//
// `base: "./"` keeps asset URLs relative, which is what Tauri's
// `tauri://localhost` / `asset://` webview origins need (an absolute `/`
// base would resolve against the wrong origin).
//
// A THIRD mode, `web` (`npm run build:web`, i.e. `vite build --mode web`),
// builds instead for `sopkb-server` (`v2/sopkb-rust/bin/sopkb-server`) to
// serve as a normal HTTP static site -- separate `outDir` (`dist-web/`, never
// mixed with the Tauri-specific `../desktop-tauri/dist`) and an absolute `/`
// base (correct for a real HTTP server root, unlike Tauri's custom origin).
// Vite's mode system automatically loads `.env.web` here, which sets
// `VITE_BACKEND=web` -- the flag `src/lib/runtime.ts`'s `IS_WEB_BUILD` reads
// to route through the HTTP backend instead of Tauri IPC or the mock.
export default defineConfig(({ mode }) => {
  const isWeb = mode === 'web'
  return {
    plugins: [react(), tailwindcss()],
    base: isWeb ? '/' : './',
    build: {
      outDir: isWeb ? 'dist-web' : '../desktop-tauri/dist',
      emptyOutDir: true,
    },
    test: {
      environment: 'jsdom',
      globals: true,
      setupFiles: ['./src/test/setup.ts'],
    },
  }
})
