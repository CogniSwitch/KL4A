---
title: Code Workbench UI
description: >-
  The codekb workbench: six sections over a code bundle - overview, code,
  architecture, knowledge, docs and a live agent probe.
---

# Workbench UI

The workbench is a local web app: `codekb serve <dir>`, no build step, no SPA. Every
view is a full page load and every filter lives in the URL, so any state you can
reach you can also bookmark and share.

## Navigation

A code bundle has six sections, plus settings on the right:

```
Overview │ Code │ Architecture │ Knowledge │ Docs │ Agent │ ⚙ Settings
```

The order follows the pipeline the product implements — repository, parsed
structure, interpreted architecture, claims, consumption — so left to right is
also roughly the order you use them in.

The server serves one bundle, so routes are `/<section>` at the root, with
sub-views as a mode on the section rather than a tab of their own:

```
/code?mode=symbols&q=batch
/knowledge?filter=review_required
/architecture?mode=endpoints
```

!!! note "The nav is built from your bundle"
    Sections and modes that a bundle cannot fill do not appear. A COBOL
    repository is not offered FastAPI views; a repository with no ORM models has
    no **Database models** tab. An unbuilt bundle is the exception: it shows the
    full skeleton so you can see what the pipeline will produce.

### Search

One box in the header searches symbols, claims, endpoints, files and documents at
once, grouping results by kind with a link through to the view that owns each
group. It exists because scoped boxes make finding something start with a guess
about which section owns the answer.

### Getting back

Detail pages carry a breadcrumb that restores the list you came from, including
its filters:

```
Code / Symbols  q:"milvus" kind:"class"  /  milvus_classes.MilvusClass
```

## Overview

The front page of a bundle, and where you rebuild it.

It opens with the one thing this bundle needs next — *nothing built yet*,
*N claims awaiting review*, or *built and reviewed, try a retrieval* — each with
the control to act on it. Below that: counts (each a link), detected architecture,
the repository record, relation resolution, and breakdowns by predicate and tier.

The **claims** count is of *active* claims, matching the page the tile opens, so
a headline never disagrees with the list behind it. A separate **retired claims**
tile appears only when a bundle has non-active claims, and links to the `retired`
filter.

The **Build** mode holds the pipeline form and the run panel. See
[Pipeline & Mining](pipeline.md).

## Code

One browser over the parsed repository, with five modes:

| Mode | Shows |
| --- | --- |
| **Files** | Every scanned file, its language, parse status and checksum, with the symbols found in it |
| **Modules** | Module name, file, import count, symbol count |
| **Symbols** | Functions, classes and methods, with kind, location and fan-out/fan-in |
| **Relations** | The edge list — subject, predicate, object, resolution status |
| **Graph** | The same edges drawn |

These are five lenses on one dataset, so they share a search box and the term
survives a mode change. A selected symbol follows you between the symbol-shaped
modes.

### Symbol detail

Reached by clicking any symbol anywhere in the app. Shows kind, module, location,
docstring, signature, and the **evidence** — the actual source excerpt — followed by
outgoing and incoming relations and any claims attached to the symbol.

### Graph modes

- **Symbol neighborhood** — callers left, symbol centre, callees right. Nodes are
  clickable, so you walk the call graph. Callee borders are coloured by resolution
  status. The rail is ranked by degree, so the busiest symbols surface first.
- **Package structure** — folder hierarchy with module counts.
- **Request flow (layered)** — route → service → repository → model, aggregated
  across all endpoints.

## Architecture

Framework-level structure, contributed by whichever adapter can populate it. What
you see depends on the repository — see
[Architecture Detection](architecture-detection.md).

## Knowledge

Claims and their review, as one page. A queue switcher across the top
(*all · review required · proposed · anchored · unanchored · retired*, each with
a count), then search, tier, sort and grouping controls. Every filter but
*retired* lists active claims.

The list both selects and navigates: the checkbox selects for a batch decision,
the title opens the claim. Selecting anything reveals the batch bar, which applies
one decision — with one rationale — to everything selected, while still recording
one event per claim.

The detail pane shows the claim, its evidence, its anchor status, and the decision
form. See [Knowledge & Review](knowledge-and-review.md).

## Docs

The human-readable Markdown layer the pipeline wrote into the bundle, plus the
generated reports, in one rail grouped into **Guides** and **Reports**. Mermaid
diagrams render; a **raw** control shows the source text.

## Agent

A live probe of what an agent actually gets back. Enter a task id and a query and
the page runs the real retrieval, showing matched symbols, usable claims,
relations, tests, warnings and context rules, with the raw JSON collapsed
underneath.

This is the tab that answers "is this bundle useful yet?".

## Settings

The LLM provider configuration hybrid mining will use. Right-aligned because it
is not a view of the bundle.

In `codekb serve` this page is **read-only**: it reports which provider is in
effect and where each value came from, with secrets masked. `codekb` reads the
same saved settings as `sopkb`, so a second editor here could disagree with the
first. Change them in the SOP workbench's Settings page. A provider environment
variable or a `.env` file is only consulted for a field whose **Source** is not
already `saved` — neither one overrides a saved value.

Values are stored outside every bundle, so a key cannot reach a manifest or an
export.
