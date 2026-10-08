---
title: codekb CLI Reference
description: >-
  Every codekb command and flag: building a bundle, inspecting it, feeding an agent,
  cross-bundle trace and transform, and serving it.
---

# codekb CLI Reference

Every command is a subcommand of `codekb`. `codekb --help` lists them; each
subcommand takes `--help` of its own. `kl4a --use codekb <command> ...` runs
the identical command through the [unified dispatcher](../kl4a.md) — same
flags, same output, same exit code; every example below works either way.

Most commands take a **bundle** directory first. Three take a **repository**
directory first — `build`, `scan`, and `trace`/`transform`'s source arguments —
because they read the repository to produce or update a bundle.

## Build the bundle

| Command | Purpose |
| --- | --- |
| `codekb init [bundle_dir] --repo <path>` | Create a code bundle pointed at a repository. Does not scan. |
| `codekb build <repo_dir>` | Run the whole pipeline in one command. |
| `codekb scan <repo_dir>` | Inventory the repository's source files. |
| `codekb parse <bundle_dir>` | Parse supported source files into modules and symbols. |
| `codekb relations <bundle_dir>` | Generate and resolve relations between symbols. |
| `codekb architecture <bundle_dir>` | Detect framework architecture. |
| `codekb mine <bundle_dir>` | Generate proposed claims. |
| `codekb render <bundle_dir>` | Compose the human-readable layer. |
| `codekb validate <bundle_dir>` | Validate the bundle. |

Options:

| Command | Options |
| --- | --- |
| `init` | `[bundle_dir]` (optional), `--repo <path>` **(required)**, `--title <text>`, `--mining static\|hybrid` |
| `build` | `--bundle <path>`, `--title <text>`, `--mining static\|hybrid`, `--provider <id>`, `--skip-render` |
| `scan` | `--bundle <path>` |
| `parse` | `--language <id>` |
| `mine` | `--provider <id>` |

!!! note "`--mining`, not a per-run flag"
    Mining mode is operational policy, so it is recorded in the bundle's manifest
    once rather than re-supplied on every command. Re-running `codekb init` with a
    different `--mining` is how you switch an existing bundle between
    `static` and `hybrid`.

When `--bundle` is omitted, `build` and `scan` use the standard bundle location
for that repository — and `init` does too, when its own `bundle_dir` is
omitted. When `--provider` is omitted, the bundle's configured provider is
used. Hybrid providers are `llm`, `azure-llm`, and `hybrid`.

## Inspect the bundle

| Command | Purpose |
| --- | --- |
| `codekb repo describe <bundle_dir>` | Repository record and counts |
| `codekb files search <bundle_dir> <query> [--language]` | Search source files |
| `codekb symbols search <bundle_dir> <query> [--language]` | Search symbols |
| `codekb symbols get <bundle_dir> <symbol_id>` | One symbol in full |
| `codekb relation search <bundle_dir>` | Search edges — `--subject`, `--predicate`, `--object`, `--resolution-status` |
| `codekb relation neighborhood <bundle_dir> <node_id> [--depth N]` | Edges around a node (default depth 1) |

All of these print JSON.

## Feed an agent

| Command | Purpose |
| --- | --- |
| `codekb context <bundle_dir> --task <id>` | Task-ready context. `--query`, `--language` narrow it. |
| `codekb agent <bundle_dir> --task <id> --query <text>` | Run the agent harness. `--provider` defaults to `fixture`. |
| `codekb tests for-symbol <bundle_dir> <symbol_id>` | Tests related to a symbol |
| `codekb change-impact <bundle_dir> <symbol_id>` | Direct impact of changing a symbol |

`--task` is required for both `context` and `agent`; `agent` also requires
`--query`.

## Cross-bundle trace and transform

Used when migrating a codebase — a COBOL bundle traced to its replacement, for
instance. See [Knowledge & Review](knowledge-and-review.md).

| Command | Purpose |
| --- | --- |
| `codekb canonicalize <bundle_dir>` | Emit canonical, language-neutral artifacts |
| `codekb trace create <source_bundle_dir> <target_bundle_dir> --out <dir>` | Build a cross-KB trace |
| `codekb trace validate <trace_dir>` | Validate a trace |
| `codekb trace report <trace_dir>` | Print the coverage report |
| `codekb trace review <trace_dir> <source_artifact_id>` | Record one decision — `--disposition`, `--reviewer`, `--rationale` all required; `--target` repeatable |
| `codekb transform plan <source_bundle_dir> --target <id> --out <dir>` | Plan a target repository |
| `codekb transform generate <plan_dir> --out-repo <dir>` | Generate the target repository |

## Cache

| Command | Purpose |
| --- | --- |
| `codekb cache clear <bundle_dir>` | Delete cached enrichment responses |

Only hybrid mining uses the cache. Clearing it forces every selected symbol to be
re-sent on the next hybrid run.

## Serve

| Command | Purpose |
| --- | --- |
| `codekb serve <bundle_dir> [--host] [--port]` | Local web workbench. Defaults to `127.0.0.1:8765`. |
| `codekb mcp serve <bundle_dir>` | Serve the read-only `code.*` tools over JSON-RPC stdio |

`codekb serve` takes **one bundle directory**. It refuses a path that is not a
code bundle rather than starting and showing an empty page. See
[Workbench UI](workbench-ui.md) and [MCP Server](mcp.md).

## Exit behaviour

Commands return `0` on success and `1` on error, with the message on stderr.

`validate` and `trace validate` return `1` when there are **errors**, and `0`
when there are only warnings. Unresolved relations are warnings — a call to
something outside the parsed repository is a fact about the repository, not a
defect. Treat errors as blocking and warnings as information about where the
bundle's knowledge stops.
