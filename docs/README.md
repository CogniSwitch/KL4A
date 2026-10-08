# Documentation Index

Short, public-facing guides live here alongside the deeper internal design/spec docs they link down to. Start at the repository root [README.md](../README.md) and [QUICKSTART.md](../QUICKSTART.md) if you're new to the project.

## Public guides (this directory)

- **[CLI_REFERENCE.md](sopkb/CLI_REFERENCE.md)** — every `sopkb` subcommand and flag, with defaults, organized by command group.
- **[WEB_UI_GUIDE.md](sopkb/WEB_UI_GUIDE.md)** — how to use `sopkb serve`: browsing a bundle, the review workflow, ingest, and the in-browser agent chat.
- **[ARCHITECTURE.md](sopkb/ARCHITECTURE.md)** — short overview of how the pipeline, bundle store, review, export, web app, and MCP server fit together, linking down to the deep docs below for detail.
- **[FAQ.md](sopkb/FAQ.md)** — common setup and ingestion problems.

## Deep design and spec docs

- **[KL4A_REQUIREMENTS.md](sopkb/KL4A_REQUIREMENTS.md)** — product statement, goals, non-goals, and primary user personas.
- **[KL4A_IMPLEMENTATION_DESIGN.md](sopkb/KL4A_IMPLEMENTATION_DESIGN.md)** — detailed implementation design.
- **[KL4A_IMPLEMENTATION_PLAN.md](sopkb/KL4A_IMPLEMENTATION_PLAN.md)** — the milestone-shaped implementation plan (the "M-numbers" referenced by the `test_gate_*` test suite; see [CONTRIBUTING.md](../CONTRIBUTING.md)).
- **[OKF_BUNDLE_SPEC.md](OKF_BUNDLE_SPEC.md)** — the normative SOP Knowledge Bundle shape (OKF-compliant directory layout, document frontmatter, canonical vs. implementation state).
- **[KL4A_MCP_PREVIEW.md](sopkb/KL4A_MCP_PREVIEW.md)** — design notes for the MCP JSON-RPC/stdio server preview.

## Project hygiene and process docs

Some of these currently exist only on other branches and will appear here once those branches merge — linked in advance so this index stays complete:

- **[WHY_OPEN_CORE.md](WHY_OPEN_CORE.md)** — the open-source/enterprise boundary: what stays in this project forever vs. what belongs to the separate, closed-source CS Governed KB product.
- **[SECURITY_SCAN_NOTES.md](SECURITY_SCAN_NOTES.md)** — the manual secret-scan pass performed ahead of making this repository public.
- **[POST_LAUNCH_OPERATIONS.md](POST_LAUNCH_OPERATIONS.md)** — internal maintainer playbook for issue/PR triage cadence, security-report acknowledgment, and roadmap-refresh cadence after launch.

## Root-level docs

These live at the repository root rather than under `docs/`, but are part of the same documentation set:

- **[README.md](../README.md)** — project overview and quickstart.
- **[QUICKSTART.md](../QUICKSTART.md)** — step-by-step install and first bundle.
- **[CONTRIBUTING.md](../CONTRIBUTING.md)** — dev setup, tests, PR process, DCO sign-off.
- **[CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md)** — Contributor Covenant v2.1.
- **[SECURITY.md](../SECURITY.md)** — vulnerability disclosure process and supported versions.
- **[SUPPORT.md](../SUPPORT.md)** — where to ask questions and file issues.
- **[GOVERNANCE.md](../GOVERNANCE.md)** — project governance and the open-source/enterprise boundary.
- **[ROADMAP.md](../ROADMAP.md)** — Now/Next/Later roadmap.
- **[CHANGELOG.md](../CHANGELOG.md)** — notable changes by release.
- **[LICENSE](../LICENSE)** / **[NOTICE](../NOTICE)** — Apache-2.0 license and third-party notices.
