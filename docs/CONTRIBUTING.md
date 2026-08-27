---
title: Contributing
---

# Contributing

Thanks for considering a contribution to KL4A. This document covers dev setup,
running tests, coding style, the PR process, and DCO sign-off.

## Repository layout

Everything shippable lives under `v2/`:

| Path | What it is |
|---|---|
| `v2/sopkb-rust` | The Cargo workspace: the `sopkb-*` library crates plus the `sopkb-cli`, `sopkb-mcp`, and `sopkb-server` binaries. |
| `v2/desktop-tauri` | The Tauri v2 desktop shell (KL4A Workbench). A **standalone** Cargo project with its own `Cargo.lock` and release profile — deliberately not a member of the workspace above. |
| `v2/frontend` | The React + Vite UI, built into `v2/desktop-tauri/dist` and embedded in the app. |

## Dev setup

You need a stable Rust toolchain (edition 2021, `rust-version` 1.77) and Node 20.

```bash
# Rust workspace: CLI, MCP server, web server, and all library crates
cd v2/sopkb-rust
cargo build --workspace --all-targets

# Frontend
cd ../frontend
npm ci
npm run build          # writes v2/desktop-tauri/dist

# Desktop app (run from v2/desktop-tauri; needs the frontend built first)
cd ../desktop-tauri
npx --yes @tauri-apps/cli@^2 dev      # or `build` for an installer
```

On Linux the Tauri build additionally needs the webkit2gtk stack —
`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`,
`libxdo-dev`, `libssl-dev`. See `.github/workflows/linux-appimage.yml` for the
exact package list CI installs.

## Running tests

```bash
cd v2/sopkb-rust
cargo test --workspace

cd ../frontend
npm test               # vitest
```

Please add or update tests for any behavior change, and make sure the Rust suite
passes locally before opening a PR.

Two things worth knowing about the current state of CI, so a red or green run
doesn't mislead you:

- **CI covers `v2/sopkb-rust` only** (`ci.yml` runs `cargo build` + `cargo test`
  across a Linux/macOS/Windows matrix). The frontend's vitest suite and the
  desktop crate's tests are not wired into CI yet — run them locally.
- **`cargo test --workspace` in CI carries a `--skip` list** of known-failing
  tests, listed explicitly in `ci.yml` with the reasoning inline. They're
  skipped visibly rather than deleted or masked. If your change fixes one,
  remove it from that list in the same PR.

### The `phaseN_*` test naming convention

Several test files are named `phase<N>_<slug>.rs` — `phase4_v1_diff.rs`,
`phase5_export_bundle.rs`, `phase8_reference_diff.rs`, and so on. The `N` is the
phase of the build-out the test was written for. A `*_v1_diff` or
`*_reference_diff` test asserts that the output is **byte-for-byte identical**
to a frozen reference output checked into the fixtures tree for the same input.
They're differential tests, not ordinary unit tests, and a failure usually means
real output drift rather than a broken assertion.

Feature-scoped tests use plain descriptive names instead
(`cli_integration.rs`, `docx_fixtures.rs`, `golden_roundtrip.rs`).

## Coding style

```bash
cd v2/sopkb-rust
cargo fmt --all              # format
cargo clippy --workspace --all-targets -- -D warnings

cd ../frontend
npm run lint                 # oxlint
```

Note that the `lint.yml` workflow is **non-blocking today**
(`continue-on-error: true`): `cargo fmt --all -- --check` currently fails
against the existing tree, which has never been uniformly rustfmt'd. Please
format the code you touch, but don't take a green lint badge as proof the whole
tree is clean, and don't reformat unrelated files in a feature PR — a
tree-wide format pass should be its own commit.

## PR process

1. Fork/branch, make your change, and add/update tests.
2. Run `cargo test --workspace` and format/lint locally.
3. Open a PR describing what changed, why, and how you tested it.
4. Update user-facing docs if you changed CLI flags, the bundle schema, the
   desktop UI, or MCP tools. The docs site is built from `docs/` by
   `mkdocs.yml`; a new page needs a `nav` entry there to be reachable.
5. A maintainer will review; see [Governance](GOVERNANCE.md) for how decisions
   get made and response-time expectations (best-effort, no SLA).

### Checklist

- [ ] Read this document.
- [ ] Commits are signed off (`git commit -s`) per the DCO requirement below.
- [ ] Tests added/updated, and `cargo test --workspace` passes locally.
- [ ] Format/lint run over the code you touched.
- [ ] Docs updated if user-facing behavior changed.
- [ ] No secrets, real credentials, or non-synthetic PII anywhere in the diff,
      including fixtures and examples.

## DCO sign-off

This project uses the **Developer Certificate of Origin (DCO)** instead of a
Contributor License Agreement (CLA). It's a lighter-weight way of recording that
you have the right to submit your contribution under the project's license,
without a separate signed document.

Sign off every commit with:

```bash
git commit -s -m "Your commit message"
```

The `-s` flag appends a line to your commit message:

```text
Signed-off-by: Your Name <you@example.com>
```

That line is your certification that you wrote the change (or otherwise have the
right to submit it) under the terms of the
[Developer Certificate of Origin](https://developercertificate.org/), and that
you're contributing it under this project's license
([Apache-2.0](https://github.com/CogniSwitch/KL4A/blob/main/LICENSE)). It uses
the name and email from your local `git config user.name` / `user.email`, so make
sure those are set to something real and identifiable — anonymous or obviously
fake sign-offs won't be accepted.

If you forgot `-s` on a commit already made, amend it: `git commit --amend -s`
(for the most recent commit), or use
`git rebase --exec 'git commit --amend --no-edit -s' <base>` for a range.

PRs with unsigned commits will be asked to add sign-off before merge.
