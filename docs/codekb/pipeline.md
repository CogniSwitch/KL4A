---
title: Code Pipeline and Mining
description: >-
  How codekb builds a bundle: scan, parse, resolve relations, detect architecture,
  mine claims, render and validate - static or LLM-assisted hybrid mining.
---

# Pipeline & Mining

## Stages

A run is eight stages. Each writes its own state before the next begins, so a
failure partway leaves everything already produced.

| Stage | Does | Writes |
| --- | --- | --- |
| Create bundle | Records the repository and mining mode in the manifest | `manifest.yaml` |
| Scan repository | Walks the repository, skipping ignored directories, and hashes every source file it keeps | `code_inventory.json` |
| Parse sources | AST parse into modules, symbols and evidence | `code_symbols.json`, `parser_runs.json` |
| Resolve relations | Turns references into edges with a resolution status | `code_relations.json` |
| Detect architecture | Framework-level structure, per language | `code_architecture.json` |
| Mine knowledge | Extracts claims, each bound to symbols and evidence | `code_knowledge.json` |
| Render human layer | Module and symbol pages, indexes, overview | `code/`, `overview.md` |
| Validate bundle | Checks the result against the bundle rules | `reports/validation.*` |

Architecture and render are optional. The workbench exposes both as checkboxes
(**Detect architecture**, **Render human layer**). The CLI only exposes one of the
two: `codekb build --skip-render` skips rendering; there is no CLI flag to skip
architecture detection, which a CLI-driven `build` always runs.

Runs start in the background. The request returns as soon as the run is queued, so
closing the page does not stop it — a large repository in hybrid mode takes
minutes. The **Build** page polls only while something is actually running.

### What the scan ignores

The walk drops a whole directory by name rather than filtering its files
afterwards, so nothing underneath a skipped name reaches the inventory. The
default list:

```
.git   .hg   .svn   .sopkb   .codekb   .test_runs   .mypy_cache
.pytest_cache   .pytest_tmp   .pytest_tmp_current   .ruff_cache   .tox
.venv   venv   __pycache__   node_modules   dist   build
bundles   knowledge-bundles
```

`.codekb`, `bundles` and `knowledge-bundles` are on it so that a bundle living
inside the repository it describes is never scanned back into itself.

It is configurable under `inventory`, as either `skip_dirs` (replaces the list)
or `extra_skip_dirs` (added to whatever the list already is). Three layers are
deep-merged, each overriding the one before it:

1. the packaged default, `codekb/default_codekb_config.yaml`;
2. the bundle's `manifest.yaml`, under its `codekb:` key;
3. a config file at the root of the scanned repository — the first of
   `.codekb.yaml`, `codekb.yaml`, `.sopkb-codekb.yaml` or `sopkb-codekb.yaml`
   that exists. The last two predate the package split and are still read so
   repositories already carrying one keep working.

The resolved list is recorded in `code_inventory.json` under `config.skip_dirs`,
so what a scan actually ignored is always readable after the fact.

## Static Versus Hybrid

Set per bundle, recorded in `manifest.yaml` under `codekb.mining`, so CLI runs use
the same mode as the workbench.

=== "static"

    AST parsing, relation resolution, and rule-based claim extraction.
    Reproducible, offline, no network calls. This is the mode for gate evidence
    and CI.

=== "hybrid"

    Everything static does first, then an LLM interprets the highest-signal
    symbols. The static baseline is never replaced, so a provider failure
    degrades to a valid static bundle.

### What hybrid is allowed to do

Enrichment runs under constraints that make its output checkable:

- It may only cite symbol and relation ids that static extraction already
  produced. Claims citing invented ids are **dropped**.
- It must quote code verbatim from the symbol's own lines. A claim whose quote
  cannot be located is **kept but demoted to Tier 4**, which sends it to review.

The Build page reports what happened: symbols attempted, claims accepted, claims
rejected, and the reason for each rejection.

### Selection gate

Hybrid does not send the whole repository. Symbols are selected by a gate stored
alongside the mode — by default at least 2 relations, at most 250 symbols.

!!! warning "Hybrid sends source code to your configured endpoint"
    Confirm that is acceptable for the repositories you point it at, particularly
    with a hosted provider. Configure the provider under **Settings**; an
    unconfigured provider still produces a valid static bundle, with enrichment
    skipped.

### Response cache

Unchanged symbols are answered from `.codekb/cache/` rather than re-sent. Editing a
file, or changing the model or the prompt, re-asks only what changed.

```bash
kl4a --use codekb cache clear ./my-code-bundle
```

## Relation Resolution

Every edge carries a resolution status, and the whole trust story of a bundle
rests on it:

| Status | Means |
| --- | --- |
| `exact` | The parser resolved the target |
| `inferred` | Matched heuristically |
| `unresolved` | The target is outside the parsed repository |

A high unresolved count is normal and is not a defect — calls into the standard
library and third-party packages land there. It tells you where the call graph
stops, which is exactly what an agent needs to know before it reasons past the
edge.

## Rebuilding

Re-running against the **same** repository re-derives claims in place; review
decisions are preserved.

Re-running against a **different** repository retires every existing claim and
replaces it. The workbench names both repositories and requires an explicit
confirmation before it will do this, because the review decisions on retired
claims are kept but no longer describe the bundle.

## Commands

```bash
# whole pipeline
kl4a --use codekb build /path/to/repo --bundle ./b --mining static

# one stage at a time
kl4a --use codekb scan /path/to/repo --bundle ./b
kl4a --use codekb parse ./b
kl4a --use codekb relations ./b
kl4a --use codekb architecture ./b
kl4a --use codekb mine ./b
kl4a --use codekb render ./b
kl4a --use codekb validate ./b
```
