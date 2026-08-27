# sopkb-rust

The pure-Rust backend: Rust reimplementation of the Python package at `tools/sopkb/sopkb/`, per the plan in
[`docs/port/PORT_PLAN.md`](../../docs/port/PORT_PLAN.md). Decisions on the plan's open questions (§7) are recorded in
[`docs/port/DECISIONS.md`](../../docs/port/DECISIONS.md).

This directory lives under [`v2/`](../README.md) alongside its two siblings, `frontend/` (the React UI)
and `desktop-tauri/` (the Tauri shell that wires the two together) -- see `v2/README.md` for how the three
fit together and where the final packaged/portable build lands. This directory holds only the backend:
no GUI code, no Tauri dependency, buildable and testable entirely on its own.

## Layout

This directory holds the entire new Rust backend and is kept separate from `tools/sopkb/` (the
original Python package, including the `web_app.py` HTML UI it replaces) — that package is still the
live, CI-tested, PyPI-released codebase and stays where it is, at the repository root.

```
v2/sopkb-rust/
  Cargo.toml           workspace manifest
  crates/              the 9 library crates from PORT_PLAN.md §3.1
  bin/
    sopkb-cli/         ported CLI + differential-test-harness driver + `sopkb serve` HTTP shim subcommand
    sopkb-mcp/         standalone stdio MCP server binary
  fixtures/            golden bundle corpus + differential test cases (PORT_PLAN.md §6.0 V1)
  DEVIATIONS.md         fix-tracking log: every behavior change from Python must have an entry here
```

Build order followed PORT_PLAN.md §6 (Phase 0 harness first, then §6.2 `sopkb-fmt`/`sopkb-core`, ...,
through Phase 9's `sopkb-workbench` orchestration layer and Phase 10's Tauri command layer + frontend).

## Relationship to `../desktop-tauri/`

`../desktop-tauri/src-tauri` is the Tauri shell (window management, app lifecycle) and a thin consumer of
`sopkb-workbench` and its sibling crates here via path dependencies (`../../sopkb-rust/crates/...`, since
it's a sibling under `v2/`, not nested inside this directory) -- nothing in the command layer spawns a
subprocess or talks HTTP. It is deliberately its own standalone Cargo project, not a member of this
workspace, since it has its own `Cargo.lock` and release profile that a shared workspace profile would
otherwise override. The old sidecar/pywebview-era tooling this section used to describe as still-present
(`sidecar.rs`, the pywebview-compat shim, `sidecar_build/`) has been fully removed; what's left of that
era is preserved for reference, not deleted outright, at the repository root under `legacy/desktop-tauri-*`.
