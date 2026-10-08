---
title: Code Bundle State
description: >-
  What a Code Knowledge Bundle holds on disk: the .codekb state files, what writes
  each one, and what is safe to delete.
---

# Bundle State

A code bundle is a folder. Nothing is hidden in a database, and every stage's
output is a file you can read.

## Layout

```
my-code-bundle/
  manifest.yaml              profile, repository, mining mode
  overview.md                human entry point
  code/                      rendered module and symbol pages
  reports/                   validation, coverage, resolution
  .codekb/                    pipeline state
    code_inventory.json
    code_symbols.json
    code_relations.json
    code_architecture.json
    code_knowledge.json
    code_reviews.json
    code_pipeline_run.json
    parser_runs.json
    cache/
    indexes/
```

## State Files

| File | Written by | Holds |
| --- | --- | --- |
| `code_inventory.json` | Scan | `repository`, `detected_languages`, `sources` |
| `code_symbols.json` | Parse | `modules`, `symbols`, `evidence` |
| `code_relations.json` | Resolve relations | `relations`, `summary` by predicate and resolution status |
| `code_architecture.json` | Detect architecture | Per-adapter keys plus a merged `summary` |
| `code_knowledge.json` | Mine | `items` (claims), `summary` by tier, `enrichment` stats |
| `code_reviews.json` | Review, in the workbench | `status` per claim, and an append-only `events` list |
| `code_pipeline_run.json` | A run started in the workbench | Stage list with per-stage status, plus the run summary |
| `parser_runs.json` | Parse | Which backend parsed what, and how it went |
| `cache/` | Hybrid mining | Enrichment responses keyed by symbol content |
| `indexes/` | Created at init | Reserved for generated indexes; empty until something writes one |

## Two Rules Worth Knowing

**Reviews are an overlay.** `code_reviews.json` is written separately and the mined
claims are never rewritten, so re-running the pipeline does not lose review
decisions. Both readers apply it — the workbench and `code_context` resolve a
claim's status the same way, so what you approve is what an agent receives. See
[Knowledge & Review](knowledge-and-review.md#what-reaches-an-agent).

**Writes are atomic.** The workbench serves reads of these files while they are
being written, so state is written whole rather than in place. `manifest.yaml`
gets the same treatment, and it is the one that mattered most: a torn state file
raises, but the readers that load the manifest swallow the parse error and fall
back to `{}` — so a torn manifest failed silently, rendering a hybrid bundle as
`static` with a blank repository path.

## manifest.yaml

```yaml
profile: code-knowledge-bundle
id: my-code-bundle
title: My Code Bundle
codekb:
  repository:
    path: /path/to/repo
  mining:
    mode: static          # or hybrid
    selection:
      min_relations: 2
      max_symbols: 250
```

`profile` is what identifies a code bundle. The workbench reads it to decide which
set of views to render, which is why a freshly created bundle is recognised before
anything has been parsed.

## Identifiers

| Prefix | Refers to |
| --- | --- |
| `source-` | A scanned file |
| `module-` | A module or compilation unit |
| `symbol-` | A function, class, method, paragraph, data item |
| `ki-` | A knowledge item — a claim |
| `crev-` | A review event |

Ids are derived from names, so they are stable across runs as long as the name is.

## Inspecting By Hand

```bash
jq '.summary' ./b/.codekb/code_relations.json
jq '.summary.by_tier' ./b/.codekb/code_knowledge.json
jq '.status' ./b/.codekb/code_reviews.json
jq '.stages[] | {label, status}' ./b/.codekb/code_pipeline_run.json
```

The last two only exist once there is something to hold: `code_reviews.json`
after a first review decision, `code_pipeline_run.json` after a run started from
the workbench. A `codekb build` on the command line runs in the foreground and
writes no run state.

## What Is Safe To Delete

`cache/` is reconstructible — clearing it only costs provider calls on the next
hybrid run. Everything else in `.codekb/` is pipeline output and will be rebuilt by
re-running the relevant stage, **except** `code_reviews.json`, which holds human
decisions and cannot be regenerated.
