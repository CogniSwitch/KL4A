---
title: Use Cases
description: >-
  Concrete scenarios for sopkb and codekb - grounded in what each pipeline
  actually produces, not speculative capability.
---

# Use Cases

Each scenario below is tied to a command, a file, or a behavior that actually exists
today — not a speculative capability. Where something is a known gap rather than a
feature, it's called out as one.

## sopkb — SOPs, policies, procedures

### Answer "what do I need to do" with a citation, not a guess

- **Command:** `kl4a --use sopkb agent context <bundle> --task eligibility-check`
- **Scenario:** an agent is asked *"what do I need for an eligibility check?"*
- **What comes back:** the matched knowledge item, its source `evidence_id`, and a
  machine-checkable decision rule derived from it — the exact obligation sentence
  the answer came from, not a paraphrase
- **See:** [Quickstart](sopkb/quickstart.md)

### Keep a human in front of every policy claim before an agent can act on it

- **Default state:** mined knowledge starts `proposed`
- **Gate:** nothing becomes `usable_knowledge` an agent can retrieve until a person
  approves it from the review workbench
- **Other dispositions:** reject, defer, edit, comment — each tracked
- **Cost model:** a one-time review at authoring time, not a per-answer review gate
- **See:** [Desktop UI Guide](sopkb/DESKTOP_UI_GUIDE.md)

### Catch contradictory or outdated policy before an agent trusts it

- **Where:** a bundle's `agent context` response includes conflict and freshness
  reports alongside the matched knowledge
- **Example:** two SOPs making incompatible claims about the same procedure get
  flagged before either one reaches an agent's answer
- **Proven on:** the [GLP-1 healthcare reference bundle](../examples/glp1-healthcare),
  end to end

### Feed knowledge into a tool that doesn't speak OKF

- **Command:** `kl4a --use sopkb export <bundle_dir> --format graph-json` and
  `kl4a --use sopkb export <bundle_dir> --format rdf`
- **Output:** Graph JSON and RDF/Turtle artifacts, written alongside the native OKF
  bundle
- **For:** pipelines or triple stores that expect one of those formats instead
- **See:** [Architecture Overview](sopkb/ARCHITECTURE.md)

### Let a policy owner test-drive an agent before it goes live

- **Feature:** the desktop app's Agent screen — chat against the bundle's knowledge
  and decision rules, with per-chat memory
- **Who:** a policy or SOP owner who has never touched a CLI
- **How:** type a plain-English scenario, see the response evaluated against
  already-mined knowledge, side by side with the chat
- **Why it matters:** catches a bad answer during authoring review, by the person
  who owns the policy — not from a production incident
- **See:** [Desktop UI Guide — Agent](sopkb/DESKTOP_UI_GUIDE.md#agent)

## codekb — source code

### Give a coding agent reviewed context instead of raw grep results

- **Command/tool:** `code.context` — CLI, MCP tool, or workbench page
- **Returns:** matched symbols, the exact source excerpt behind each one, the
  relations touching them, and the tests that cover them
- **Plus:** explicit warnings about unresolved relations or claims still awaiting
  review, so an agent knows where the bundle's knowledge stops instead of silently
  reasoning past it
- **See:** [Agent Consumption](codekb/agent-consumption.md)

### Understand an unfamiliar repository's actual architecture, not just its files

- **Feature:** architecture detection — reads framework-level structure per language
- **Python:** FastAPI routes with effective mount paths, SQLAlchemy models and their
  columns/relationships, Pydantic request/response schemas, ranked third-party
  dependencies
- **COBOL:** programs and copybooks
- **Key property:** a repository only ever shows the views its own stack actually
  populates
- **See:** [Architecture Detection](codekb/architecture-detection.md)

### Check blast radius before changing a symbol

- **Tools:** `code.change_impact <symbol_id>` and `code.tests.for_symbol <symbol_id>`
- **Answers:** "what breaks if I touch this" and "is this tested"
- **Grounded in:** the bundle's already-resolved relations, not a fresh ad-hoc search
- **See:** [MCP Server](codekb/mcp.md)

### Carry review-gated knowledge through a language migration

- **Commands:** `kl4a --use codekb trace` / `kl4a --use codekb transform`
- **What they build:** a cross-bundle trace between a source bundle and a target
  bundle — e.g. a COBOL codebase being traced to its planned replacement
- **What's recorded:** per-artifact coverage plus disposition decisions
  (`disposition`, `reviewer`, `rationale`) — not a blind one-shot rewrite
- **See:** [Knowledge & Review](codekb/knowledge-and-review.md)

### Onboard a coding agent to a codebase it doesn't know yet

- **Setup:** build a hybrid-mined bundle over a repository, expose it via
  `kl4a --use codekb mcp serve`
- **What the agent gets:** `code.context`, `code.symbols.search`, and the rest of
  the MCP surface, instead of starting from plain grep
- **Already resolved for it:** evidence, relations, and test coverage for every
  matched symbol
- **Best fit:** a large or actively-changing repository, where re-deriving
  architecture and call relationships from scratch every session is exactly the
  cost this avoids
- **See:** [MCP Server](codekb/mcp.md), [Pipeline & Mining](codekb/pipeline.md)

### Verify a language port by diffing bundles, not by eyeballing code

- **Mechanism:** a codekb bundle reduces a repository to comparable counts and
  structures — symbols, relations, modules, detected architecture
- **Pattern:** build one bundle from the reference implementation, another from a
  reimplementation in a different language, then diff them
- **Why it's better than a read-through:** "does the port behave the same way"
  becomes something you check mechanically — matching symbol/relation counts,
  diffable JSON output — not something you assert
- **Worked example:** this repository's own experimental Rust port of
  `codekb` was audited exactly this way
- **See:** [Rust Port Parity](codekb/rust-port-parity.md), including where the two
  implementations still disagree

## Across both — gate bundle integrity in CI

- **Commands:** `kl4a --use sopkb validate`, `kl4a --use codekb validate`
- **Exit codes:** `0` when there are no errors, `1` otherwise, with the message on
  stderr — the same contract a test suite gives a CI pipeline
- **Use:** run the relevant `validate` command as a merge gate, so a knowledge
  bundle becomes a maintained artifact that can't merge in a broken state, instead
  of a one-off export nobody re-checks after the first review
