---
title: KL4A — Documentation
---

# KL4A

*Knowledge Layer For Agents — an independent open-source project.*

**Create OKF-compliant knowledge bundles from your SOP docs, and enable your agents to use them.**

Your organization's SOPs, policies, procedures, regulations, and standards live in Word
docs, PDFs, and wikis — written for humans, not for agents. KL4A reads them, proposes
structured knowledge, and puts a person in front of every claim before an agent can act
on it. What comes out the other side is a **Knowledge Bundle**:

- Every claim traces back to the exact source text it came from.
- Nothing an agent can act on until a person has reviewed it.
- Plain files, git-diffable, no lock-in — built on [OKF](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf), the open format this project implements. (KL4A is not affiliated with or endorsed by the OKF project.)

It doesn't claim to mine everything in a document — known gaps are tracked in the repo.

**Ground it. Review it. Then trust it.**

## Built for knowledge engineers

KL4A is for the person who owns turning a folder of SOPs into something an agent can be
trusted against: ingest a document set, inspect what got extracted and why, work through
review until the bundle is clean, and hand it off — to an agent, an MCP server, or an
enterprise import.

You review the knowledge once, so you don't have to review every answer forever.

That review happens once, at authoring time, on a bounded set of extracted claims — not
at runtime, on every answer an agent gives. It's also usable without a knowledge
engineer in the room:

- **SOP & policy owners** see exactly what was extracted, verify it against source
  evidence, and approve, reject, or correct it — no CLI required.
- **Agent developers** search knowledge items, resolve citations, and retrieve
  source-grounded context locally, via CLI, MCP, or the bundle's plain files.

## What it does

- **Structure** — sources become OKF-native bundles agents can query directly.
- **Ground** — every claim keeps its exact source span, or is flagged when one can't be matched.
- **Review** — nothing becomes accepted knowledge until a person approves, rejects, defers, edits, or comments on it.
- **Consume** — agents query the bundle via CLI, agent chat, or MCP; export to Graph JSON or RDF/TTL when other tooling needs it.

## Reference bundle

A synthetic, fully worked GLP-1 healthcare example lives at [`examples/glp1-healthcare`](../examples/glp1-healthcare). It includes:

- Multiple SOP-like sources, with DOCX/PDF ingestion
- Evidence-backed proposed knowledge
- Persisted human-in-the-loop review states
- Conflict and freshness reports
- OKF, Graph JSON, and RDF exports

## Where this fits

!!! info "Free and complete on its own"
    KL4A handles extraction, evidence grounding, and human review. Free. Local-first.
    Yours to run anywhere, forever.

    Deterministic enforcement, cross-bundle reasoning, audit trails, and governed
    multi-tenant operation in production are a separate, deliberately out-of-scope
    concern — a downstream layer this project doesn't try to be. KL4A is a tool for the
    layer where ontology projects actually fail — this is how your SOPs get ready for
    that layer, whether or not you ever adopt anything downstream of it.
    See `GOVERNANCE.md` for exactly where that line sits, why, and who
    maintains this project.

## Guides

- **[Why This Exists](WHY.md)** — why KL4A exists and why it's open.
- **[Quickstart](quickstart.md)** — install and open the desktop app, first bundle.
- **[Desktop UI Guide](DESKTOP_UI_GUIDE.md)** — the desktop app: browsing, review, ingest, and the agent chat.
- **[Architecture](ARCHITECTURE.md)** — how the pieces fit together, linking down to the deeper design docs.
- **[FAQ](FAQ.md)** — common setup and ingestion problems.

## Project

- **[Contributing](../CONTRIBUTING.md)** — dev setup, tests, PR process, DCO sign-off.
- **[Governance](../GOVERNANCE.md)** — project governance and the open-source/enterprise boundary.
- **[Roadmap](../ROADMAP.md)** — where the project is headed.
- **[License](../LICENSE)** — Apache-2.0.
- **[GitHub repository](https://github.com/CogniSwitch/KL4A)**

---

*KL4A · Apache-2.0 · maintainers & boundary: see `GOVERNANCE.md`*
