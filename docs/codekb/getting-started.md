---
title: Getting Started with Code Bundles
description: >-
  Install codekb and build your first Code Knowledge Bundle from a repository, then
  browse and review it in the local workbench.
---

# Getting Started

Build a bundle from a repository, then look at it in the workbench.

## Install

```bash
pip install kl4a
```

`kl4a` is the one PyPI project this product publishes — `codekb` isn't a
separate package to install; it ships inside `kl4a` together with `sopkb`,
and `pip install kl4a` gets you both plus the shared
substrate they build on. There's no extra to remember and nothing partial
about it.

From a checkout of this repository instead:

```bash
pip install -e tools/kl4a
```

Verify the CLI resolves:

```bash
kl4a --use codekb --help
```

## Build A Bundle In One Command

`codekb build` runs the whole pipeline — create, scan, parse, resolve, detect,
mine, render, validate — against a repository folder:

```bash
kl4a --use codekb build /path/to/repo --bundle ./my-code-bundle --mining static
```

`--mining static` is deterministic and offline. Use it first: it needs no
provider configuration and gives you a complete bundle to look at.

When it finishes you have:

```
my-code-bundle/
  manifest.yaml          profile: code-knowledge-bundle
  .codekb/                pipeline state, one file per stage
  code/                  rendered module and symbol pages
  reports/               validation, coverage, relation resolution
  overview.md            the human entry point
```

## Look At It

Start the workbench against the bundle:

```bash
kl4a --use codekb serve ./my-code-bundle
```

Open `http://127.0.0.1:8765/`. The bundle lands on **Overview**, which opens with
the single thing the bundle needs next.

!!! note "One bundle per server"
    `codekb serve` serves exactly one code bundle, and refuses a path that is not
    one rather than starting and showing an empty page. Run a second server on
    another `--port` for a second bundle.

## Build It Step By Step Instead

`codekb build` is the composition of commands you can also run individually — useful
when you want to re-run one stage without redoing the rest:

```bash
kl4a --use codekb init ./my-code-bundle --repo /path/to/repo
kl4a --use codekb scan /path/to/repo --bundle ./my-code-bundle
kl4a --use codekb parse ./my-code-bundle
kl4a --use codekb relations ./my-code-bundle
kl4a --use codekb architecture ./my-code-bundle
kl4a --use codekb mine ./my-code-bundle
kl4a --use codekb render ./my-code-bundle
kl4a --use codekb validate ./my-code-bundle
```

## Verify It

```bash
kl4a --use codekb validate ./my-code-bundle
kl4a --use codekb repo describe ./my-code-bundle
```

`validate` reports errors and warnings; the full report is written to
`reports/validation.md` and is readable in the workbench under **Docs**.

Unresolved relations are warnings, not errors. A call to something outside the
parsed repository is a fact about the repository, not a defect —
see [Relation resolution](pipeline.md#relation-resolution).

## Ask It Something

The point of a bundle is what an agent gets back from it:

```bash
kl4a --use codekb context ./my-code-bundle --task explain --query "batch processing"
```

The same retrieval is visible in the workbench under **Agent**, which is the
quickest way to see whether a bundle is actually useful yet.

## Next

- [Workbench UI](workbench-ui.md) — what each section shows
- [Pipeline & Mining](pipeline.md) — static versus hybrid, and what hybrid adds
- [Knowledge & Review](knowledge-and-review.md) — why some claims need a decision
- [MCP Server](mcp.md) — handing the bundle to an agent
- [CLI Reference](cli.md) — every command and flag
