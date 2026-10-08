---
title: Agent Consumption
description: >-
  What an agent gets back from a Code Knowledge Bundle: task-ready context, matched
  symbols, evidence, relations, tests and warnings.
---

# Agent Consumption

A bundle exists to be read by an agent. The read path is `code.context`, available
as a CLI command, an MCP tool, and a page in the workbench.

## code.context

```bash
kl4a --use codekb context ./my-code-bundle --task explain --query "batch processing"
```

`--task` is a free-form task id describing what the agent is doing (`explain`,
`plan-change`, `find`). `--query` is the term to retrieve around; when omitted, the
task id itself is used as the query. `--language` narrows the search.

### What comes back

| Key | Contains |
| --- | --- |
| `task` | The task id and the query actually used |
| `symbols` | Matched symbols |
| `evidence` | The source excerpt behind each matched symbol |
| `relations` | Edges touching the matched symbols |
| `code_knowledge` | Usable claims about them |
| `tests` | Tests related to the selected symbols |
| `warnings` | What to be careful about in this result |
| `context_rules` | How the result is meant to be used |

Retrieval widens by one hop: symbols matching the query, plus symbols reachable
from them by a relation. Claims are returned when they concern any symbol in that
set.

If the query matches nothing, retrieval falls back to a small sample of symbols
rather than returning empty — so check `task.query` and the symbol list before
concluding the bundle lacks something.

### Warnings are part of the answer

Unresolved relations and review-required claims are reported alongside the result.
An agent that ignores the warnings is reasoning past the edge of what the bundle
actually knows.

## What Is Filtered Out

Claims are excluded from `code.context` when rejected, when no longer active, or
when Tier 4 and flagged for review. See
[Knowledge & Review](knowledge-and-review.md#what-reaches-an-agent).

Approving a Tier 4 claim makes it retrievable; deferring or commenting does not.

## MCP

```bash
kl4a --use codekb mcp serve ./my-code-bundle
```

Serves the bundle over stdio as read-only MCP tools. The code surface:

| Tool | Purpose |
| --- | --- |
| `code.repo.describe` | Describe the bundle |
| `code.files.search` | Search source files |
| `code.symbols.search` | Search symbols |
| `code.symbols.get` | One symbol, with evidence and relations |
| `code.relations.search` | Search edges by subject, predicate, object or status |
| `code.callgraph.neighborhood` | Relations around a node, to a given depth |
| `code.tests.for_symbol` | Tests related to a symbol |
| `code.change_impact` | Direct impact of changing a symbol |
| `code.context` | Task-ready context, as above |

That is the whole surface, and all of it is read-only. There is no tool that
writes to the bundle, so an agent cannot approve a claim, change review state, or
alter the repository record — review stays a human action in the workbench.

(`sopkb`'s MCP server has an optional `review.note` writer, off unless explicitly
enabled. `codekb` has no equivalent.)

## Change Impact

Before editing a symbol, ask what depends on it:

```bash
kl4a --use codekb change-impact ./my-code-bundle symbol-app-answer-function
kl4a --use codekb tests for-symbol ./my-code-bundle symbol-app-answer-function
```

Impact is direct — callers and the relations touching the symbol — not a
transitive closure. It is a starting point for a blast radius, not the whole one.

## Trying It Without An Agent

The **Agent** section of the workbench runs the same retrieval and renders the
result: matched symbols, claims with tier and review status, relations, tests,
warnings, context rules, and the raw JSON.

Use it to answer "is this bundle useful yet?" before wiring anything up. If the
Agent page returns nothing helpful for a query you care about, an agent will do no
better.
