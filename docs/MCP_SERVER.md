---
title: MCP Server
---

# MCP Server

KL4A ships a built-in [Model Context Protocol](https://modelcontextprotocol.io/) server, so any
MCP-capable agent or editor can connect straight to a bundle — no separate integration to build.

## `sopkb-mcp <bundle_dir> [--enable-review-notes]`

`sopkb-mcp` is a standalone binary — there is no `sopkb-cli mcp` subcommand — that serves the
Model Context Protocol tool surface over stdio (JSON-RPC), for connecting an MCP-capable
agent/editor to a bundle.

| Argument | Required | Default | Notes |
|---|---|---|---|
| `bundle_dir` | yes | — | |
| `--enable-review-notes` | no | off (flag) | Without this flag, the mutating `review.note` tool is not advertised/callable and any attempt raises `review.note is disabled; start with --enable-review-notes`. All other tools are read-only by default. |

This command doesn't open a network port — it's a stdio server: it reads one JSON-RPC request per line from stdin and writes one response per line to stdout. You don't run it by hand day to day; an MCP client spawns it as a subprocess and owns its stdin/stdout. Point your client's config at it:

```json
{
  "mcpServers": {
    "kl4a": {
      "command": "sopkb-mcp",
      "args": ["/absolute/path/to/demo-bundle"]
    }
  }
}
```

Use an absolute path for the bundle — the client launches the process from its own working directory, not the bundle's. `"command": "sopkb-mcp"` only resolves if `sopkb-mcp` is on the `PATH` the client's subprocess inherits, which isn't guaranteed for a build-from-source checkout; if it can't be found, point `command` at the built binary directly instead (e.g. `v2/sopkb-rust/target/debug/sopkb-mcp.exe` on Windows, `v2/sopkb-rust/target/debug/sopkb-mcp` on macOS/Linux).

In Claude Code, the equivalent one-liner is:

```console
$ claude mcp add kl4a -- sopkb-mcp /absolute/path/to/demo-bundle
```

The piped example below drives the same protocol by hand — useful for verifying the server works before wiring up a client, not the day-to-day usage path. Real captured output, run against a one-line "New hires must confirm identity before systems access is granted" demo bundle:

??? example "Example — piping two JSON-RPC requests over stdin"
    ```console
    $ printf '%s\n%s\n' \
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
        '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bundle.describe","arguments":{}}}' \
        | sopkb-mcp demo-bundle
    ```
    ```json
    {"id": 1, "jsonrpc": "2.0", "result": {"capabilities": {"tools": {}}, "instructions": "Ground every answer only in what these tools return — never in general/internet/training-data knowledge, even when labeled as such. Call knowledge.search (or agent.context) first; if nothing relevant comes back, say explicitly that this knowledge base has no grounded answer for that part instead of filling the gap. Always show the actual facts too: alongside any summary or paraphrase, quote the raw evidence/source_text returned by knowledge.search or evidence.get verbatim, and cite the knowledge item, section, or source it came from. If a knowledge.search result carries rule_ids, or knowledge.get/agent.context shows a decision_rules entry, that item is governed by a structured condition/obligation/otherwise rule — fetch it (knowledge.get for the item, or agent.context for the task) and apply that rule's logic wherever it's applicable to the question, rather than answering from the prose evidence alone.", "protocolVersion": "2024-11-05", "serverInfo": {"name": "sopkb", "version": "0.0.1"}}}
    {"id": 2, "jsonrpc": "2.0", "result": {"content": [{"text": "{\n  \"id\": \"demo-bundle\",\n  \"knowledge_item_count\": 1,\n  \"profile\": \"sop-knowledge-bundle\",\n  \"source_count\": 1,\n  \"status\": \"draft\",\n  \"title\": \"Demo Bundle\"\n}", "type": "text"}]}}
    ```
    (one JSON-RPC response object per line, in request order — note the real `initialize` response also carries an `instructions` string laying out the grounding contract for the connecting agent, not shown in earlier drafts of this page)

The same read-only functions are also exposed as plain `sopkb-cli` subcommands (`knowledge search`, `agent context`, `relations search`, ...), callable outside an MCP client.
