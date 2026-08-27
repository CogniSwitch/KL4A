---
title: Architecture Overview
---

# Architecture Overview

This is a short, public-facing tour of how KL4A fits together. For full detail — the product requirements, the implementation design, and the normative bundle spec — see the deeper docs linked at the end of each section; this page intentionally does not duplicate them.

## The shape of the system

```mermaid
flowchart TD
    A["Source documents<br/>(.md, .txt, .docx, .pdf)"]
        -- "sopkb scan" -->
    B["Inventory<br/>(checksummed originals)"]

    B
        -- "sopkb normalize" -->
    C["Normalized Markdown<br/>+ sections"]

    C
        -- "sopkb mine<br/>--provider fixture | azure-llm" -->
    D["Proposed knowledge items<br/>(evidence-linked)"]

    D
        -- "sopkb review<br/>approve | reject | defer | comment | edit" -->
    E["Reviewed knowledge"]

    E
        -- "sopkb validate" -->
    F["Validation / freshness<br/>/ conflict reports"]

    F
        -- "sopkb export<br/>--format ..." -->
    G["OKF-native bundle<br/>(canonical)"]

    G --> H["Graph JSON / RDF-TTL<br/>(derivative)"]
    G --> I["sopkb serve<br/>(local web app:<br/>browse, review, ingest, agent chat)"]
    G --> J["sopkb mcp serve<br/>(MCP server:<br/>read-only agent tool surface)"]

    classDef box stroke-width:1.5px,rx:8,ry:8;
    class A,B,C,D,E,F,G,H,I,J box;
```

??? note "Diagram as a linear list (for screen readers, or if you copied the diagram's text)"
    Copying text out of a Mermaid diagram (or reading it with a screen reader) pulls out node labels and edge labels as two separate groups, not in visual reading order. Here's the same pipeline top to bottom:

    1. **source documents** (.md, .txt, .docx, .pdf)
    2. → `sopkb scan` → **inventory** (checksummed originals)
    3. → `sopkb normalize` → **normalized Markdown + sections**
    4. → `sopkb mine --provider fixture|azure-llm` → **proposed knowledge items** (evidence-linked)
    5. → `sopkb review` → **reviewed knowledge**
    6. → `sopkb validate` → **validation / freshness / conflict reports**
    7. → `sopkb export --format ...` → **OKF-native bundle** (canonical), which then feeds three things in parallel:
        - **Graph JSON / RDF-TTL** (derivative)
        - `sopkb serve` (local web app: browse, review, ingest, agent chat)
        - `sopkb mcp serve` (MCP server: read-only agent tool surface)

!!! note "No database. Network calls depend on which surface you use, and which provider it defaults to."
    Everything left of `sopkb serve`/`sopkb mcp serve` is implemented as a plain CLI pipeline over files on disk — there is no database anywhere in this path. Whether a network call happens depends on the mining/agent provider, and **the CLI and the web UI default to different providers**:

    - **CLI** (`sopkb mine`, no `--provider` given): defaults to `fixture` — zero dependencies, offline, no network call.
    - **Web UI's Ingest screen**: the **Mining provider** dropdown defaults to `azure-llm`, not `fixture` — submitting the form without changing the dropdown calls Azure OpenAI. Both fail closed (raise, rather than silently proceed) if `AZURE_OPENAI_*` env vars aren't set, so there's no silent leak absent configuration — but the *default posture* in the web UI leans toward the network-dependent path on both screens.

    Pick `fixture` explicitly (CLI flag or dropdown) for a fully offline run.

## Bundle store

A **SOP Knowledge Bundle** is a directory created by `sopkb init`: a fixed set of subdirectories (`sources/`, `sections/`, `concepts/`, `knowledge/`, `relations/`, `rules/`, `evidence/`, `tasks/`, `references/`, `authored/`, `reports/`) plus a `manifest.yaml`.

The bundle root itself is the canonical, OKF-compliant artifact — Markdown documents with YAML frontmatter, cross-linked to each other.

A `.sopkb/` subdirectory holds implementation state (JSON indexes, upload staging, caches) that is derived from, and re-synced into, the canonical Markdown on every mutating command.

→ Full normative shape: [`OKF_BUNDLE_SPEC.md`](OKF_BUNDLE_SPEC.md).

## Mining / extraction

`sopkb mine` turns normalized section text into **proposed knowledge items**, each an evidence-linked subject/predicate/object claim with a source span, confidence score, and `review_status: proposed`. Two providers exist today:

| Provider | Default | Implementation | Dependencies / network | Behavior |
|---|---|---|---|---|
| `fixture` | Yes on the CLI (`sopkb mine`'s `--provider` default) and yes in the web UI's Ingest sources screen (its mining-provider dropdown defaults to `fixture (offline, no network)`). | `sopkb/mine.py` | Zero dependencies, deterministic, offline | Regex-based obligation-sentence detection |
| `azure-llm` | Opt-in on the CLI and in the web UI's Ingest sources screen, once an LLM provider is configured on the Settings screen. | `sopkb/okf_author.py` | Azure OpenAI's Responses API | LLM-authored path that also emits full OKF documents (concepts, decision rules) alongside knowledge items |

Both write the same underlying `KnowledgeItem` shape, so downstream review, export, and agent consumption are provider-agnostic.

This mining-provider axis is unrelated to how the web UI's **Agent Studio** screen evaluates a scenario against already-mined knowledge — that's a separate call, not a user-facing provider choice, and not how knowledge is *extracted*. See [DESKTOP_UI_GUIDE.md](DESKTOP_UI_GUIDE.md#agent-studio).

## Review

Human-in-the-loop review (`sopkb review`, or the Inspect bundle screen in the web UI) is first-class, not a preview feature: approve, reject, defer, comment, and edit actions are all persisted as review events with reviewer identity and rationale (`sopkb/review.py`). Approved and rejected are terminal states. Review state flows directly into validation reports and into `has_review` edges in graph exports — there's no separate publish step.

## Export

`sopkb export` re-syncs the canonical OKF bundle and additionally writes derivative formats — Graph JSON and RDF/Turtle — under a sibling `exports/` directory (`sopkb/export.py`). The OKF bundle itself never requires "exporting" to be useful; these are additional representations for graph tooling and the enterprise import path.

## Web app and MCP server

`sopkb serve` (`sopkb/web_app.py`) is a dependency-free `http.server`-based app exposing the full pipeline plus review and an in-browser agent chat over plain HTTP. `sopkb mcp serve` (`sopkb/mcp_server.py`) exposes a read-only-by-default Model Context Protocol tool surface (bundle/sources/sections/knowledge/evidence/conflicts/freshness/citations/agent/relations) over JSON-RPC/stdio, with an explicit `--enable-review-notes` opt-in for the one mutating tool.

→ Desktop app screens: [`DESKTOP_UI_GUIDE.md`](DESKTOP_UI_GUIDE.md). → MCP server: [`MCP_SERVER.md`](MCP_SERVER.md).

## Agent consumption

`sopkb.agent_consumption` provides task-scoped context retrieval (`agent.context`, `agent.tasks`, `agent.guide`) and RDF-compatible relation traversal (`relations.search`, `relations.neighborhood`), usable identically from the CLI, the web UI's Agent screen, or MCP. So an agent gets the same evidence-grounded, review-aware context regardless of integration surface.

## Where this stops

!!! note "Out of scope"
    This project owns bundle **creation and export**. It does not implement multi-tenant governance, RBAC, audit trails, or hosted APIs at organizational scale — that boundary, and why it's drawn there, is covered in `GOVERNANCE.md`.

## Further reading

- [`OKF_BUNDLE_SPEC.md`](OKF_BUNDLE_SPEC.md) — the normative bundle shape.
