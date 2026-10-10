---
title: "Why Open Core: What Belongs Here vs. Downstream"
description: >-
  Where the line sits between what KL4A does and what a downstream governance layer
  does, and who maintains this project.
---

# Why Open Core: What Belongs Here vs. Downstream

This document explains the design boundary between the open-source **KL4A** (Knowledge Layer For Agents — this repository) and the closed-source enterprise product it can feed into. It exists so that contributors, reviewers, and downstream integrators can quickly tell what belongs upstream (in this project) versus downstream (in a commercial system), and why.

**CogniSwitch** is the company that maintains this open-source project. CogniSwitch also builds and sells an enterprise product, referred to below as **CS Governed KB**, that is not part of this repository and is not required to use it.

## What this project is

KL4A is an open-source, local-first workbench for turning enterprise SOPs, policies, procedures, regulations, and standards into rich, reviewable knowledge bundles for humans and agents, built on:

<p style="text-align: center;">
  <strong><a href="https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf">OKF</a></strong> — Open Knowledge Format
</p>
<p style="text-align: right; font-size: 0.85em;">
  <em>an open standard published by Google Cloud</em>
</p>

OKF is not a format invented by this project — see [OKF_BUNDLE_SPEC.md](OKF_BUNDLE_SPEC.md) for exactly which parts of Google's spec this project aligns with and extends.

The artifact this project produces is an **SOP Knowledge Bundle**: a portable, OKF-compliant, standards-friendly collection of markdown and structured files. A bundle must be genuinely useful on its own — inspectable, queryable by local agents, and exportable to RDF/TTL or graph JSON — without requiring any proprietary software.

## The boundary, in one line

!!! abstract "In one line"
    **This project owns creating and exporting knowledge bundles. It does not own operationalizing them at enterprise scale.**

Concretely, the open-source workbench owns the full creation and export lifecycle:

- source inventory and document normalization,
- AI-assisted extraction of candidate knowledge,
- evidence grounding (every claim traceable to source text),
- human-in-the-loop (HITL) review — approve, reject, defer, edit, comment,
- bundle validation,
- conflict and freshness reporting,
- RDF/TTL and graph JSON export,
- OKF-based bundle export,
- local mechanisms (CLI, web app, MCP server) for agents to inspect and use bundles.

A separate, closed-source enterprise product (CS Governed KB) owns governed operationalization once a bundle leaves this project:

- importing SOP Knowledge Bundles into a governed enterprise graph,
- mapping bundles into richer enterprise graph layers,
- running governed lifecycle and review workflows at organizational scale,
- multi-tenancy, role-based access control (RBAC), and audit,
- preserving decision trace and interaction trace across many users and systems,
- exposing governed knowledge to other enterprise systems, primarily through APIs.

## Why draw the line here

The rule of thumb applied throughout this project: **if a capability makes a single bundle better, more trustworthy, or more usable on its own, it belongs here.** If a capability is only meaningful once you have many bundles, many tenants, or organization-wide policy and audit requirements, it belongs downstream.

This keeps the open-source project honest and independently useful:

- You should never need an enterprise account, connector, or license to create, review, validate, or export a bundle, or to point a local agent at one.
- HITL review is a first-class open-source requirement, not a stripped-down preview of a paid feature. Review state and reviewer rationale live in the bundle itself.
- The bundle format is designed to be readable and useful with ordinary tools (a text editor, `grep`, a markdown renderer) — no proprietary runtime required.

What moves downstream is specifically the *governance-at-scale* layer: RBAC, multi-tenant isolation, organization-wide audit trails, and API-first governed access — concerns that only exist once an organization is managing many bundles across many teams, not concerns of an individual bundle.

## How a bundle and the enterprise product relate

Bundles produced by this project are the input contract for the enterprise connector, not a dependency of it:

```text
enterprise SOP docs -> SOP Knowledge Bundle (this project) -> enterprise import connector -> CS Governed KB -> governed APIs, inference, audit, decision trace
```

The enterprise import connector and CS Governed KB itself live outside this repository. This project's only obligation to that boundary is to keep the bundle format well-specified and complete enough that such a connector *could* import it later — it does not implement or depend on that connector.

## What this means if you are contributing

| If you are proposing… | Scope |
|---|---|
| A feature that improves bundle creation, extraction quality, evidence grounding, review workflows, validation, or local/agent consumption of a single bundle | Very likely **in scope** for this project |
| Multi-tenant access control, org-wide policy enforcement, or cross-bundle governance and audit | Very likely **out of scope** — belongs in a governed enterprise system, not in the open-source workbench |
| Compliance/certification claims (SOC 2, HIPAA, GDPR, etc.) in this project's docs or README | **Out of scope, and shouldn't be added here** — see note below |
| Encryption-at-rest, key management, or RBAC/access-control features for the workbench itself | **Out of scope for this project's threat model** — see note below |

When in doubt, ask whether the feature makes one bundle better on its own. If yes, it is upstream. If the feature only makes sense across many bundles and many organizational users, it is downstream.

!!! warning "This repository does not hold, and should not claim, compliance certifications"
    CogniSwitch (the company) holds real, current certifications — **SOC 2 Type 2** (completed audit) and **HIPAA** (architecture accordance, BAA available on request), plus GDPR data processing agreements. See [cogniswitch.ai/compliance](https://cogniswitch.ai/compliance) and [cogniswitch.ai/trust](https://cogniswitch.ai/trust) for the current, authoritative claims.

    Those certifications cover **CS Governed KB's hosted infrastructure** — the encryption, access control, and data-handling of a multi-tenant service that processes customer data on CogniSwitch's servers (per [cogniswitch.ai/trust](https://cogniswitch.ai/trust): AES-256 at rest via AWS KMS, TLS 1.3 in transit, RBAC with MFA enforcement and quarterly access reviews). This OSS workbench is local-first and has no equivalent data flow to certify: `sopkb` reads and writes files on your own machine and never transmits anything to CogniSwitch. Adding a "SOC 2 compliant" or "HIPAA compliant" badge to this repository would misrepresent what it actually does. If you need a compliance-backed hosted deployment, that's what CS Governed KB is for — **not a claim this project can honestly make about itself.**

!!! warning "This workbench has no encryption-at-rest or access-control layer of its own — by design"
    `sopkb` has no built-in authentication, no login/session system, and no encryption-at-rest anywhere in the codebase (verified against `tools/kl4a/sopkb/` — `sopkb serve` is a dependency-free `http.server` app with no auth layer). This is by design, not an oversight: the workbench assumes a single local user with normal filesystem access to their own machine, the same trust boundary as any other local CLI tool or text editor.

    Whatever encryption-at-rest your disk/OS provides is what protects a bundle at rest; there is no separate access-control layer to add, because there's no multi-user boundary here for one to enforce. Multi-user RBAC and key management are exactly the *governed, multi-tenant* concerns this document already draws downstream to CS Governed KB — CogniSwitch's own controls there (per [cogniswitch.ai/trust](https://cogniswitch.ai/trust): RBAC with MFA enforcement, quarterly access reviews, AES-256/AWS KMS) — **not gaps to patch in this project.**

## Open questions this boundary does not yet resolve

!!! warning "Not yet resolved"
    A few areas are still being worked out as the project matures, and are not fixed by this document:

    - the exact OKF profile fields for SOP Knowledge Bundles,
    - whether the MCP server ships alongside the CLI/agent tools or follows shortly after,
    - the parsing stack for early releases (lightweight built-in parsers vs. integrating a library such as Docling or Unstructured),
    - the precise import contract expected by enterprise connectors, which is versioned and documented independently of this repository.
