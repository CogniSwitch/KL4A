---
title: Roadmap
description: >-
  Where the project is headed, organized into Now / Next / Later buckets instead of
  dates or version numbers.
---

# Roadmap

This roadmap is organized into **Now / Next / Later** buckets instead of
dates or version numbers. Open-source roadmaps that promise ship dates and
then miss them tend to cost more trust than they earn — a vague-but-honest
sense of sequencing is more useful to users and contributors than a
calendar we'd likely have to walk back. Nothing here is a commitment; it's
a statement of current intent and rough ordering, and it will change as we
learn more.

## Now

- **Web UI for codekb.** A browser-based view surfacing codebase architecture
  and execution flow for repositories already indexed by the `codekb` tool,
  instead of today's
  CLI/MCP-only access. The prerequisite step — assessing whether an
  experimental from-scratch Rust reimplementation (`kl4a-rs/`) can serve
  as this UI's backend instead of the existing implementation — is
  substantially complete: a parity audit (see
  [Rust Port Parity](codekb/rust-port-parity.md)) found the Rust port now
  produces byte-identical `codekb` output on a full self-build. One known gap
  remains before the port can be
  treated as a drop-in backend: YAML 1.1 vs 1.2 ambiguous-scalar handling
  (`yes`/`no`/dates) diverges from the reference implementation's parsing
  and needs a proper resolver fix, not a patch. The UI itself hasn't
  started.
- **Pluggable LLM provider interface.** Mining (`sopkb.okf_author`) and
  agent chat (`sopkb.agent_chat`), along with the example agents (e.g.
  `tools/sopkb/examples/azure_llm_task_agent.py`), currently call out to
  an LLM exclusively through hardcoded `AZURE_OPENAI_*` environment
  variables (endpoint, API key, deployment, timeout, etc.). We want a
  small provider abstraction so OpenAI, Anthropic, and local models (via
  Ollama) can be used without vendor lock-in to Azure.
- **Document and test MCP editor integrations.** The MCP server
  (`tools/kl4a/sopkb/mcp_server.py`) already exists with a read-only
  tool contract and JSON-RPC handling, and is covered by
  `test_gate_m11_mcp.py`. What's missing is user-facing documentation and
  explicit test coverage for connecting it to real MCP clients — Claude
  Desktop, Claude Code, and other IDE integrations — so it's usable
  outside of this repo's own test harness.
- **PyPI release.** `tools/sopkb/pyproject.toml` already defines the
  `sopkb` package and a `sopkb` console script entry point, so publishing
  a first release to PyPI is mostly packaging and release-process work
  rather than new code.

## Next

- **More reference bundles.** `examples/glp1-healthcare` is currently the
  only end-to-end reference bundle (see `test_gate_m10_reference_bundle.py`).
  Adding reference bundles in other verticals would demonstrate that the
  workbench generalizes beyond healthcare SOPs rather than being tuned to
  one domain.
- **Bundle schema stabilization toward 1.0.** `docs/OKF_BUNDLE_SPEC.md` is
  currently at spec version 0.2.0 (targeting OKF version 0.2). Getting to
  a documented 1.0 means locking down the canonical bundle shape and
  export semantics, plus writing a migration story for bundles created
  under 0.x.
- **Packaging polish beyond PyPI.** A Docker image for `sopkb serve`, and
  possibly a hosted read-only demo of the example bundle, to lower the
  barrier to trying the workbench without a local Python setup.

## Later

- **i18n / non-English SOP support.** Ingestion, mining, and the web
  workbench currently assume English-language source documents. This is
  speculative and contingent on actual demand signal from users working
  with non-English SOPs before we invest in it.
