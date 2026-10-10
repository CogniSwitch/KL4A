<img src="docs/images/kl4a-logo-light.svg" alt="KL4A" width="96">

# Knowledge Layer For Agents (KL4A)

**Create grounded, portable knowledge bundles from SOP documents and source code, then let people and agents use them with evidence.** KL4A includes two workflows: SOP Knowledge Bundles for policies and procedures, and Code Knowledge Bundles for software repositories.

> **For teams that run on SOPs and developers that build with code.** Turn policies into evidence-backed, OKF-compatible knowledge; turn repositories into structure-aware context. Give agents something they can cite, inspect, and reuse — not another opaque chat response.

For SOPs, feed it a PDF, DOCX, or plain-text procedure. For code, point it at a
repository. KL4A then:

- Normalizes SOPs into clean Markdown, sections, evidence-linked claims, and reviewable decisions.
- Parses repositories into files, symbols, relationships, architecture signals, and task-ready context.
- Keeps the resulting bundle as plain files that can be reviewed, diffed, versioned, and queried.
- Exposes both bundle types through a CLI and a read-only MCP interface for agents.

**The result is a plain-file artifact.** Markdown with YAML frontmatter:
readable in a text editor, diffable with `git`, and queryable by an agent over
MCP or the CLI — with no database, and no server standing between them and the
source of truth.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

**Latest release: [v0.0.2](https://github.com/CogniSwitch/KL4A/releases/latest).**

The native desktop app (Windows/macOS/Linux, built on Tauri) is the easiest way to use the **SOP Knowledge Bundle** workflow — one application to install, not a server to stand up or a runtime to provision. **CodeKB does not currently have a desktop app; use its CLI or MCP server instead.**

## Two knowledge bundle workflows

| Workflow | Input | What it produces | Main command |
|---|---|---|---|
| **SOP Knowledge Bundle** | Policies, procedures, PDFs, DOCX, Markdown | Evidence-linked, human-reviewed operational knowledge | `sopkb-cli` |
| **Code Knowledge Bundle** | A software repository | Files, symbols, relations, architecture, and task-ready code context | `codekb` (CLI/MCP only; no desktop app yet) |

`kl4a` is the unified dispatcher: use `kl4a --use sopkb ...` or `kl4a --use codekb ...` when you prefer one entry point.

```text
 SOPs, policies, PDFs                 Source repositories
          │                                    │
          ▼                                    ▼
       sopkb                                codekb
          │                                    │
          └───────────────┬────────────────────┘
                          ▼
                        KL4A
                          │
                          ▼
        Grounded, reviewable knowledge bundles
                          │
                          ▼
                    Agents and apps
```

Create the knowledge once. Let people, agents, and applications reuse it with its evidence and review history intact.

## Why KL4A exists

SOPs and codebases are both difficult for an agent to consume safely. Prose has no built-in accountability trail; code has structure, but not a concise explanation of intent, relationships, and impact. Asking an agent to infer either directly from raw input produces answers that are hard to inspect or trust.

KL4A turns both into small, sourced, checkable knowledge artifacts *before* an agent uses them. It preserves evidence, structure, and review state instead of hiding those decisions behind a chat response.

### Why not just RAG?

RAG retrieves relevant source material. KL4A turns source material into individually addressable, evidence-backed, reviewable knowledge that can be reused across tasks and agents. RAG finds context; KL4A creates a durable knowledge layer. See the [RAG comparison](docs/sopkb/rag-comparison.md) for the detailed distinction.

## See it in action

Start with a policy, a procedure, or a codebase. KL4A produces a bundle that a person can inspect and an agent can query without losing the connection to the original source. The SOP flow demonstrates the full lifecycle — ingest, evidence, review, validation, and agent context — while CodeKB maps repository structure, symbols, relationships, and architecture into the same reusable form.

- [See KL4A in action](docs/see-kl4a-in-action.md) — a product walkthrough and examples.
- [SOP quickstart](docs/sopkb/quickstart.md) — build a first SOP Knowledge Bundle.
- [Getting started with CodeKB](docs/codekb/getting-started.md) — build a first Code Knowledge Bundle.

## Quickstart

### Download SOPKB Desktop

| Platform | Download |
|---|---|
| **Windows** | [⬇ Installer (`.exe`)](https://github.com/CogniSwitch/KL4A/releases/latest/download/KL4A.Workbench_0.0.2_x64-setup.exe) |
| **macOS** | [⬇ Disk image (`.dmg`)](https://github.com/CogniSwitch/KL4A/releases/latest/download/KL4A.Workbench_0.0.2_universal.dmg) - universal, runs on Apple Silicon and Intel |
| **Linux** | [⬇ AppImage](https://github.com/CogniSwitch/KL4A/releases/latest/download/KL4A.Workbench_0.0.2_amd64.AppImage) |

The builds aren't code-signed yet, so your OS will warn you on first launch: on macOS right-click then **Open** to get past Gatekeeper, on Windows click **More info** then **Run anyway**, and on Linux `chmod +x` the AppImage first.

Install it, open it, and everything else - creating an SOP bundle, ingesting sources, reviewing mined knowledge, exporting, talking to the agent - happens inside the app. This desktop application does not support CodeKB; use the `codekb` CLI or its MCP server for repository knowledge bundles. See the [SOP quickstart](docs/sopkb/quickstart.md) and [Desktop UI Guide](docs/sopkb/DESKTOP_UI_GUIDE.md) for the full walkthrough.

### Download CLIs and MCP servers

If you'd rather not install the desktop app, or want an agent to read a bundle, the release ships four standalone binaries. No toolchain or source build is required.

| Platform | `kl4a` | `codekb` | `sopkb-cli` | `sopkb-mcp` |
|---|---|---|---|---|
| **Windows** | [⬇ `.exe`](https://github.com/CogniSwitch/KL4A/releases/latest/download/kl4a-windows-x86_64.exe) | [⬇ `.exe`](https://github.com/CogniSwitch/KL4A/releases/latest/download/codekb-windows-x86_64.exe) | [⬇ `.exe`](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-cli-windows-x86_64.exe) | [⬇ `.exe`](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-mcp-windows-x86_64.exe) |
| **macOS** | [⬇ universal](https://github.com/CogniSwitch/KL4A/releases/latest/download/kl4a-macos-universal) | [⬇ universal](https://github.com/CogniSwitch/KL4A/releases/latest/download/codekb-macos-universal) | [⬇ universal](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-cli-macos-universal) | [⬇ universal](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-mcp-macos-universal) |
| **Linux** | [⬇ `x86_64`](https://github.com/CogniSwitch/KL4A/releases/latest/download/kl4a-linux-x86_64) | [⬇ `x86_64`](https://github.com/CogniSwitch/KL4A/releases/latest/download/codekb-linux-x86_64) | [⬇ `x86_64`](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-cli-linux-x86_64) | [⬇ `x86_64`](https://github.com/CogniSwitch/KL4A/releases/latest/download/sopkb-mcp-linux-x86_64) |

`sopkb-cli` runs the SOP pipeline: `scan`, `normalize`, `mine`, `review`, `validate`, `export`. `sopkb-mcp` serves finished SOP bundles over read-only stdio MCP. `codekb` builds and queries Code Knowledge Bundles and includes its own MCP server through `codekb mcp serve <bundle_dir>`. `kl4a` dispatches to either workflow.

On macOS and Linux, make the file executable and put it somewhere on your `PATH` so a client can resolve it by name:

```bash
chmod +x sopkb-mcp-linux-x86_64
sudo mv sopkb-mcp-linux-x86_64 /usr/local/bin/sopkb-mcp
```

The macOS binaries are universal, so one file covers Apple Silicon and Intel.

### Build a Code Knowledge Bundle

Build a bundle from a repository in one command. Static mining is deterministic and does not require an LLM provider:

```bash
codekb build /path/to/repository --bundle ./my-code-bundle --mining static
codekb validate ./my-code-bundle
codekb context ./my-code-bundle --task explain --query "authentication flow"
```

To expose the bundle to an MCP-capable agent, configure `codekb` as a stdio server:

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

See [Getting started with CodeKB](docs/codekb/getting-started.md) for the full workflow.

## An empty result is not a bug

An agent can't tell a grounded answer from a confident guess unless the tool is honest about what it doesn't have. Search a bundle for a term it actually contains, and for one it doesn't — real output, captured against a one-line "New hires must confirm identity before systems access is granted" SOP:

```console
$ sopkb-cli knowledge search demo-bundle "access"
[
  {
    "id": "ki-onboarding-v1-000001",
    "subject": "Access Requirements",
    "predicate": "requires",
    "object": "New hires must confirm identity before systems access is granted.",
    "evidence": "New hires must confirm identity before systems access is granted.",
    "review_status": "proposed",
    "source_id": "onboarding"
  }
]

$ sopkb-cli knowledge search demo-bundle "quantum-encryption-protocol-xyz"
[]
```

No match, no padding — an empty array, not a low-confidence guess dressed up as an answer. The same discipline applies to extraction itself: when the mining step can't locate an LLM-claimed sentence verbatim in its source section, the knowledge item is written with `span_status: "llm_claimed"` instead of a fabricated byte range — the gap is recorded, not hidden.

## Examples: how an agent uses it

These assume a bundle already exists — built through the desktop app, or via `sopkb-cli` (the Rust CLI that ships alongside it). To reproduce the exact bundle these examples run against:

```bash
mkdir -p sources
printf '# Access SOP\n\n## Access Requirements\n\nNew hires must confirm identity before systems access is granted.\n' > sources/onboarding.md

sopkb-cli init demo-bundle
sopkb-cli scan sources --bundle demo-bundle
sopkb-cli normalize demo-bundle
sopkb-cli mine demo-bundle --provider fixture
sopkb-cli validate demo-bundle
```

**An agent retrieves grounded, task-scoped context (CLI):**

```console
$ sopkb-cli agent context demo-bundle --task eligibility-check
```

```json
{
  "task": {
    "id": "eligibility-check",
    "title": "Eligibility Check",
    "query_terms": ["eligibility", "identity", "contraindication", "clinical review"]
  },
  "usable_knowledge": [
    {
      "id": "ki-onboarding-v1-000001",
      "subject": "Access Requirements",
      "predicate": "requires",
      "object": "New hires must confirm identity before systems access is granted.",
      "review_status": "proposed",
      "evidence_id": "evidence-ki-onboarding-v1-000001",
      "rule_ids": ["rule-ki-onboarding-v1-000001-requires"]
    }
  ],
  "agent_rules": [
    "Use only usable_knowledge items as task rules.",
    "Treat rejected knowledge as excluded unless include_rejected is true.",
    "Resolve evidence before making a claim to a downstream user or system.",
    "Use Knowledge Relations for graph traversal and RDF-compatible assertions.",
    "Use decision_rules for conditional task handling; do not infer conditions from prose when structured rules exist.",
    "Check freshness and conflict reports before finalizing a decision."
  ]
}
```

*(trimmed for length — the real response also includes `decision_rules`, `concepts`, `evidence`, `knowledge_relations`, and freshness/conflict reports.)*

**Connect an MCP-capable agent** — the same tools are exposed over the Model Context Protocol on stdio, and the server tells the connecting agent how to ground its answers before it's asked anything.

`sopkb-mcp <bundle_dir>` doesn't open a network port: it's a stdio server — it reads one JSON-RPC request per line from stdin and writes one response per line to stdout. In practice you don't run it by hand; your MCP client spawns it as a subprocess and owns its stdin/stdout for you. Point your client's config at it:

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

Use an absolute path for the bundle — the client launches the process from its own working directory, not the bundle's. In Claude Code, the equivalent one-liner is:

```bash
claude mcp add kl4a -- sopkb-mcp /absolute/path/to/demo-bundle
```

The `printf ... | sopkb-mcp ...` example below is the same protocol driven by hand, useful for verifying the server works before wiring up a client — not how you'd use it day to day. Real captured output:

```console
$ printf '%s\n%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"bundle.describe","arguments":{}}}' \
    | sopkb-mcp demo-bundle
```

```json
{"id": 1, "jsonrpc": "2.0", "result": {"capabilities": {"tools": {}}, "instructions": "Ground every answer only in what these tools return — never in general/internet/training-data knowledge, even when labeled as such. Call knowledge.search (or agent.context) first; if nothing relevant comes back, say explicitly that this knowledge base has no grounded answer for that part instead of filling the gap. (...)", "protocolVersion": "2024-11-05", "serverInfo": {"name": "sopkb", "version": "0.0.2"}}}
{"id": 2, "jsonrpc": "2.0", "result": {"content": [{"text": "{\n  \"id\": \"demo-bundle\",\n  \"knowledge_item_count\": 1,\n  \"profile\": \"sop-knowledge-bundle\",\n  \"source_count\": 1,\n  \"status\": \"draft\",\n  \"title\": \"Demo Bundle\"\n}", "type": "text"}]}}
```

*(the `instructions` string is truncated above with `(...)` — it's several sentences longer in the real response, laying out the full grounding contract for the connecting agent.)*

All MCP tools are read-only by default (`knowledge.search`, `knowledge.get`, `sections.get`, `evidence.get`, `agent.context`, ...); the one mutating tool, `review.note`, is disabled unless the server is started with `--enable-review-notes`. See the [Desktop UI Guide](docs/sopkb/DESKTOP_UI_GUIDE.md) for the in-app agent chat that consumes the same context.

## Commands

| Command | Purpose |
|---|---|
| `sopkb-cli init <bundle_dir>` | Create an empty knowledge bundle |
| `sopkb-cli scan <source_dir> --bundle <bundle_dir>` | Inventory and checksum source documents (`.md`, `.txt`, `.pdf`, `.docx`) |
| `sopkb-cli normalize <bundle_dir>` | Convert sources to normalized Markdown, split into sections |
| `sopkb-cli mine <bundle_dir> --provider fixture\|azure-llm` | Propose knowledge items from normalized sections |
| `sopkb-cli review approve\|reject\|defer\|comment <bundle_dir> <id> --rationale <text>` | Record a human review decision, with rationale |
| `sopkb-cli review edit <bundle_dir> <id> --field <f> --value <v> --rationale <text>` | Correct a field on a knowledge item, with rationale |
| `sopkb-cli validate <bundle_dir>` | Check bundle structure and required fields; non-zero exit on errors |
| `sopkb-cli export <bundle_dir> --format graph-json,rdf` | Re-sync OKF documents and write derivative exports |
| `sopkb-cli knowledge search <bundle_dir> <query>` | Free-text search over knowledge items |
| `sopkb-cli agent context <bundle_dir> --task TASK` | Task-scoped context: usable knowledge, rules, evidence, relations |
| `sopkb-mcp <bundle_dir>` | Expose the same read-only tools over MCP for any MCP-capable agent |
| `codekb build <repository> --bundle <bundle_dir> --mining static` | Build a Code Knowledge Bundle from a repository |
| `codekb context <bundle_dir> --task TASK --query <text>` | Retrieve task-ready code context |
| `codekb mcp serve <bundle_dir>` | Expose read-only CodeKB tools over stdio MCP |
| `kl4a --use sopkb\|codekb <command>` | Dispatch to either workflow from one CLI |

The desktop app wraps this same pipeline behind a GUI — see the [Desktop UI Guide](docs/sopkb/DESKTOP_UI_GUIDE.md).

## The schema layer: OKF v0.2

Every bundle is built on [OKF](https://github.com/GoogleCloudPlatform/knowledge-catalog), Google's Open Knowledge Format — Markdown files with YAML frontmatter that turn a folder of documents into a queryable knowledge graph. KL4A populates the v0.2 trust-signal fields for real, not as placeholders:

- `provenance` — where a claim's evidence came from, down to the exact source span.
- `verified` — actor and date, written only when a human approves the item through the review screen or `sopkb-cli review approve`.
- `lifecycle_status` — `active`, `superseded`, `retired`, or `conflicted`, so a stale claim doesn't sit next to a current one unmarked.

Because it's plain OKF, the bundle is readable and diffable without KL4A at all — the app is one way to produce and consume it, not a required runtime.

## Fixture or LLM — both are first class

`mine` runs either way, and every other command works the same regardless of which one produced the knowledge:

- **`fixture`** — offline, deterministic pattern matching over obligation phrases (`must`, `shall`, `should record`, ...). No key, no network call, no per-run variance. (Note: unlike the app's other steps, `mine`'s own default provider is `azure-llm`, not `fixture` — pass `--provider fixture` explicitly for the offline path, as shown above.)
- **`azure-llm`** trades determinism for recall: an LLM proposes claims (including ones that don't use an obligation keyword), each still required to carry a `source_text` span — checked against the section it claims to come from, and flagged `span_status: "llm_claimed"` rather than trusted blindly when it can't be located verbatim.

## Learn more

- [SOP quickstart](docs/sopkb/quickstart.md) — get the app and build a first SOP bundle.
- [Getting started with CodeKB](docs/codekb/getting-started.md) — build and query a first Code Knowledge Bundle.
- [Desktop UI Guide](docs/sopkb/DESKTOP_UI_GUIDE.md) — the app, screen by screen.
- [MCP Server](docs/sopkb/MCP_SERVER.md) — connecting an agent to an SOP bundle.
- [SOP architecture](docs/sopkb/ARCHITECTURE.md) — how the SOP pipeline fits together.
- [CodeKB overview](docs/codekb/index.md) — CodeKB architecture and operations.

## License

Apache-2.0 — see [LICENSE](LICENSE).
