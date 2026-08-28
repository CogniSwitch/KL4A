<img src="docs/images/kl4a-logo-light.svg" alt="KL4A" width="96">

# Knowledge Layer For Agents (KL4A)

*An independent open-source project.*

**Create OKF-compliant knowledge bundles from your SOP docs, and enable your agents to use them.** Every claim carries the exact source span it came from, and nothing is marked `verified` until a human approves it.

Feed it a PDF, a DOCX, or a plain-text SOP — real, unstructured prose, not a website's DOM or a codebase's AST. It normalizes the document, extracts obligation-shaped claims (`must`, `shall`, `should record`, ...), and attaches each one to the exact byte range of the source sentence it came from. Every approve/reject/edit is recorded as its own event, with a reviewer, a rationale, and a before/after diff, directly in the bundle. Once approved, the claim's OKF v0.2 trust fields (`provenance`, `verified`, `lifecycle_status`) are populated with who verified it and when — not left blank for someone downstream to fill in by hand.

The result is a plain-file artifact: Markdown and YAML frontmatter, readable in a text editor, diffable with `git`, queryable by an agent over MCP or the CLI without a database or a server standing between them and the source of truth.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

A native desktop app (Windows/macOS/Linux, built on Tauri) — one application to install, not a server to stand up or a runtime to provision.

## Why KL4A exists

Your SOPs, policies, and regulations live in prose written for a human reader, not an agent. Pasting that prose straight into a prompt means the agent is trusting a wall of text with no accountability trail — if it gets a clause wrong, nothing tells you which sentence it misread or who signed off on it.

KL4A's job is to turn that prose into small, sourced, checkable claims *before* an agent ever sees them: extract, attach evidence, put a person in front of each one, and only then hand it to an agent — with the review trail attached, not thrown away.

## Quickstart

| Platform | Download |
|---|---|
| **Windows** | [⬇ Installer (`.exe`)](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_x64-setup.exe) |
| **macOS** | [⬇ Disk image (`.dmg`)](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_universal.dmg) — universal, runs on Apple Silicon and Intel |
| **Linux** | [⬇ AppImage](https://github.com/CogniSwitch/KL4A/releases/download/0.0.1-alpha/KL4A.Workbench_0.0.1-alpha_amd64.AppImage) |

The builds aren't code-signed yet, so your OS will warn you on first launch: on macOS right-click → **Open** to get past Gatekeeper, on Windows click **More info** → **Run anyway**, and on Linux `chmod +x` the AppImage first.

Install it, open it, and everything else — creating a bundle, ingesting sources, reviewing mined knowledge, exporting, talking to the agent — happens inside the app. There's no CLI to touch to get started; see [docs/quickstart.md](docs/quickstart.md) and the [Desktop UI Guide](docs/DESKTOP_UI_GUIDE.md) for the full walkthrough.

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
{"id": 1, "jsonrpc": "2.0", "result": {"capabilities": {"tools": {}}, "instructions": "Ground every answer only in what these tools return — never in general/internet/training-data knowledge, even when labeled as such. Call knowledge.search (or agent.context) first; if nothing relevant comes back, say explicitly that this knowledge base has no grounded answer for that part instead of filling the gap. (...)", "protocolVersion": "2024-11-05", "serverInfo": {"name": "sopkb", "version": "0.0.1"}}}
{"id": 2, "jsonrpc": "2.0", "result": {"content": [{"text": "{\n  \"id\": \"demo-bundle\",\n  \"knowledge_item_count\": 1,\n  \"profile\": \"sop-knowledge-bundle\",\n  \"source_count\": 1,\n  \"status\": \"draft\",\n  \"title\": \"Demo Bundle\"\n}", "type": "text"}]}}
```

*(the `instructions` string is truncated above with `(...)` — it's several sentences longer in the real response, laying out the full grounding contract for the connecting agent.)*

All MCP tools are read-only by default (`knowledge.search`, `knowledge.get`, `sections.get`, `evidence.get`, `agent.context`, ...); the one mutating tool, `review.note`, is disabled unless the server is started with `--enable-review-notes`. See the [Desktop UI Guide](docs/DESKTOP_UI_GUIDE.md) for the in-app agent chat that consumes the same context.

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

The desktop app wraps this same pipeline behind a GUI — see the [Desktop UI Guide](docs/DESKTOP_UI_GUIDE.md).

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

- [docs/quickstart.md](docs/quickstart.md) — get the app, first bundle.
- [Desktop UI Guide](docs/DESKTOP_UI_GUIDE.md) — the app, screen by screen.
- [MCP Server](docs/MCP_SERVER.md) — connecting an agent to a bundle you've built.
- [Architecture](docs/ARCHITECTURE.md) — how the pieces fit together, with links to the deeper design docs.
- [FAQ](docs/FAQ.md) — common setup and ingestion problems.

## License

Apache-2.0 — see [LICENSE](LICENSE).
