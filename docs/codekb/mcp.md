---
title: codekb MCP Server
description: >-
  Serve a Code Knowledge Bundle to an agent over MCP: the read-only code.* tool
  surface, how to wire it up, and what review state withholds.
---

# MCP Server

`codekb mcp serve` exposes one code bundle to an agent over the Model Context
Protocol, speaking JSON-RPC on stdio.

```bash
kl4a --use codekb mcp serve ./my-code-bundle
```

The surface is **read-only**. There is no tool that writes to the bundle, so an
agent cannot approve a claim, change review state, or alter the repository
record. Review remains a human-controlled workflow.

## Wiring it up

Point any MCP client at the command. For a client that reads a JSON config:

```json
{
  "mcpServers": {
    "codekb": {
      "command": "codekb",
      "args": ["mcp", "serve", "/absolute/path/to/my-code-bundle"]
    }
  }
}
```

The server reports itself as `codekb` and negotiates protocol versions
`2025-06-18`, `2025-03-26`, and `2024-11-05`, falling back to the last of those
when a client asks for something else.

## Tools

`*` marks a required argument.

| Tool | What it does | Arguments |
| --- | --- | --- |
| `code.repo.describe` | Describe the bundle: repository record and counts | — |
| `code.files.search` | Search source files | `query*`, `language` |
| `code.symbols.search` | Search symbols | `query*`, `language` |
| `code.symbols.get` | One symbol with its evidence and relations | `symbol_id*` |
| `code.relations.search` | Search edges | `subject`, `predicate`, `object`, `resolution_status` |
| `code.callgraph.neighborhood` | Edges around a symbol or module | `node_id*`, `depth` |
| `code.tests.for_symbol` | Tests related to a symbol | `symbol_id*` |
| `code.change_impact` | Direct impact of changing a symbol | `symbol_id*` |
| `code.context` | Task-ready context for a developer agent | `task*`, `query`, `language` |

Each returns JSON. The same operations are available from the shell — see the
[CLI Reference](cli.md) — so you can see exactly what an agent will get before
wiring anything up.

## What an agent is allowed to see

Retrieval is filtered by review state, not by what was mined. A claim reaches an
agent unless:

- it was **rejected**, or
- it is no longer **active** in the bundle's lifecycle, or
- it is **tier 4 or above and flagged for review** — those open only on an
  explicit approval. Deferring or commenting on such a claim leaves it closed.

The MCP surface has no more
permissive path into the data.

The practical consequence is that a freshly built bundle answers thinly. That is
the intended behaviour — see
[Knowledge & Review](knowledge-and-review.md#what-reaches-an-agent).

## Grounding

The server sends instructions telling the client to answer only from the bundle
and to cite the evidence attached to each claim, rather than answering from
general knowledge about the codebase. Every claim carries the source span it was
derived from, so an answer can always be checked against the code.

## Relationship to the sopkb MCP server

`sopkb mcp serve` exposes the SOP tool surface (`knowledge.*`, `sections.*`,
`evidence.*`, `relations.*`). `codekb mcp serve` exposes `code.*` only.

They are separate servers over separate bundles. Point an agent at whichever
matches the question, or register both — the tool names do not collide.
