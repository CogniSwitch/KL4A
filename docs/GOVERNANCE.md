---
title: Governance
---

# Governance

This document explains how KL4A (Knowledge Layer For Agents) is run, who makes
decisions, how the open-source project relates to CogniSwitch's commercial
product, and how to reach the people behind it.

It exists because trust has to be explicit, not assumed. If you're deciding
whether to build on this project, contribute to it, or depend on it in
production, you should be able to answer "what happens to this over time?"
without guessing.

## 1. The open-source / enterprise boundary

CogniSwitch maintains this project and also sells a commercial product,
**CS Governed KB**. That relationship creates an obvious risk for an
open-source project: features get held back, quietly degraded, or slowly
migrated behind a paywall to push adoption toward the paid product.

We are committing, in public, to not doing that. The boundary below is the
actual boundary — not a starting point that shrinks over time.

### 1.1 Stays open-source, forever

The following are core to what this project is and will not be removed,
crippled, or turned into a paid feature:

- **Bundle creation** — inventorying, normalizing, and AI-assisted mining of
  SOP/policy/procedure documents into proposed knowledge.
- **Extraction** — evidence-grounded knowledge proposals with source
  provenance, derivation tracking, and confidence.
- **Human-in-the-loop review** — approve, reject, edit, defer, and comment
  on proposed knowledge, with reviewer rationale and review state stored in
  the bundle itself, not in a hosted service.
- **Export** — OKF-based bundle files, graph JSON, RDF/TTL, and validation
  reports, in the bundle format described in
  [`OKF_BUNDLE_SPEC.md`](OKF_BUNDLE_SPEC.md).
- **Local agent / MCP consumption** — CLI tools and an MCP server for
  agents to describe, search, and cite a finished bundle without going
  through any CogniSwitch service.

A Knowledge Bundle produced by this project must remain fully useful —
readable, reviewable, exportable, and agent-queryable — without CS Governed
KB or any other CogniSwitch product. That's a design constraint, not a
promise we're making lightly.

### 1.2 Enterprise-only by design

**CS Governed KB** is a separate, commercial product that imports the
bundles this project produces and operationalizes them. It is enterprise-only
by design, not by artificial restriction, because it solves problems that
are specific to running governed knowledge in production across an
organization:

- **RBAC** — role-based access control over who can see, edit, or publish
  governed knowledge.
- **Tenancy** — multi-tenant isolation for hosted deployments.
- **Governed lifecycle** — publication workflows, policy gates, and
  lifecycle states beyond local HITL review.
- **Audit and decision trace** — persistent decision trace and interaction
  trace suitable for compliance and audit requirements.
- **Hosted multi-tenancy** — running the governed knowledgebase as a hosted
  service with API-first access for agents, apps, and workflows.

None of these are things a local, single-tenant, file-based OSS workbench
should try to be. If you don't need them, you never have to think about
CS Governed KB at all.

### 1.3 How this boundary changes

If this boundary ever shifts, it will shift by a documented change to this
file with a rationale in the commit/PR description and in the project's
release notes — not silently, and not by a feature just disappearing from a
release.

## 2. Maintainer roles

This is an early-stage project. Governance is intentionally lightweight —
enough structure to be predictable, not so much that it slows the project
down before it has a community to serve.

- **Maintainers** are the people with merge rights on this repository.
  Maintainers review and merge pull requests, cut releases, and are
  responsible for the direction of the OSS project.
- **Triage** (issues and PR labeling, reproduction, first response) may be
  done by maintainers or by contributors explicitly given triage access.
  Anyone can help triage informally by commenting on issues even without
  that access.
- Until the contributor base grows, CogniSwitch engineers are the de facto
  maintainers. The current list of people with merge rights is the set of
  people with write access to this repository on GitHub; we'll list
  maintainers by name here once the group stabilizes beyond the founding
  team.

**Response-time expectations:** this project is maintained **best effort,
with no SLA**. Issues and PRs will be looked at, but there is no guaranteed
turnaround time. If something is urgent for your use case, say so in the
issue — it helps prioritization, but it doesn't change the underlying
best-effort commitment.

## 3. Communication channels

- **GitHub Issues** — bug reports and well-scoped feature requests.
- **GitHub Discussions** — the primary channel for questions, design
  discussion, proposals, and anything that isn't a crisp bug report.
  This is the channel to use if you're unsure where something belongs.

We don't currently run a Discord or Slack for this project. A chat channel
may be added later if someone — maintainer or community member — commits to
actually monitoring it. Until that happens, assume no such channel exists,
regardless of what you might find referenced elsewhere.

## 4. Decision-making

Kept deliberately pragmatic for a project at this stage:

- **Day-to-day changes** (bug fixes, small features, docs) are decided by
  normal PR review. Any maintainer approval is sufficient to merge.
- **Larger or ambiguous changes** (new export formats, changes to the bundle
  profile, anything touching the OSS/enterprise boundary in Section 1) should
  start as a GitHub Discussion or an issue describing the proposal before a
  PR is opened, so the reasoning is visible before the implementation is.
- **Disagreements among maintainers** are resolved by discussion and rough
  consensus. If consensus doesn't emerge, the maintainer who owns the area
  of the codebase in question makes the call, and reasoning is recorded in
  the issue or PR.
- **Escalation:** if a decision affects the OSS/enterprise boundary itself,
  it is escalated to CogniSwitch's project leadership for a public,
  documented resolution — it does not get decided quietly inside a single
  PR.

This process is intentionally small. As the contributor base grows, expect
this document to grow with it — including a real maintainer list, and
possibly a more formal proposal process. Any such change will itself go
through the process described above.
