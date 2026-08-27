---
title: KL4A OKF Bundle Spec
---

# KL4A OKF Bundle Spec

Status: draft  
Spec version: 0.2.0  
Target OKF version: 0.2

<p style="text-align: center;">
  <strong><a href="https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf">OKF</a></strong> — Open Knowledge Format
</p>
<p style="text-align: right; font-size: 0.85em;">
  <em>an open standard published by Google Cloud</em>
</p>

OKF is not a format invented by this project. This document's bundle shape is built on OKF's core model — YAML-frontmatter Markdown documents and the same `okf_version` root-declaration mechanism defined in Google's spec — and extends it with KL4A-specific fields kept under this project's own `sopkb` namespace, per OKF's own extension rules (producers may add unknown keys; consumers must preserve them). This is not a claim of full drop-in conformance with every field Google's spec defines (for example, this document's `generated.actor`/`generated.date` fields use different key names than OKF's `generated.by`/`generated.at`, and this bundle's directory layout is more prescriptive than OKF's directory-agnostic model) — it is a deliberate alignment with a real external standard's core mechanism, not merely a shared name.

This document specifies the canonical KL4A bundle shape on top of that standard. OKF is not merely an export format.

Exports are for additional representations such as Graph JSON, RDF/Turtle, package archives, or downstream system-specific formats.

## 1. Design Principle

!!! abstract "Design Principle"
    Everything provided as a KL4A knowledge bundle MUST be OKF-compliant.

The workbench MAY maintain implementation indexes, caches, upload staging, and run logs, but those implementation artifacts MUST NOT be the canonical knowledge bundle. They MUST either:

- be derived from the OKF documents;
- live under an implementation namespace such as `.sopkb/`; or
- be explicitly marked as non-canonical runtime state.

## 2. Terms

OKF bundle
:   The canonical KL4A directory containing Markdown documents with YAML frontmatter, cross-links, evidence, relations, rules, and agent task contexts.

Implementation state
:   Derived JSON indexes, caches, upload staging, validation logs, UI chat history, and temporary files used by the workbench.

Knowledge piece
:   A reviewable assertion mined or authored from source evidence.

Knowledge Relation
:   An RDF-compatible subject-predicate-object assertion connected to a knowledge piece and evidence span.

Decision rule
:   A task-usable rule derived from or attached to a knowledge piece.

Evidence
:   A source span supporting a knowledge piece, Knowledge Relation, and any derived rule.

Normative keywords such as **MUST**, **SHOULD**, and **MAY** are used intentionally.

## 3. Canonical Bundle Shape

The canonical bundle root is the bundle directory itself:

```text
sop-knowledge-bundle/
  index.md
  manifest.yaml
  log.md
  sources/
    index.md
    <source_id>.md
    originals/
    normalized/
  sections/
    index.md
    <source_id>/
      <section_id>.md
  concepts/
    index.md
    <concept_id>.md
  knowledge/
    index.md
    <knowledge_item_id>.md
  relations/
    index.md
    <knowledge_relation_id>.md
  rules/
    index.md
    <rule_id>.md
  evidence/
    index.md
    <evidence_id>.md
  tasks/
    index.md
    <task_id>.md
  references/
    index.md
    agent-guide.md
  authored/
    index.md
    ...
  reports/
    validation.md
    freshness.md
    conflicts.md
    extraction_summary.md
    review_summary.md
  .sopkb/
    inventory.json
    sections.json
    items.json
    entities.json
    triples.json
    reviews.json
    llm_authoring.json
    agent_chat.json
    source_versions.json
    source_events.json
    document_contexts.json
    uploads/
    cache/
```

Rules:

| Path | Rule |
|---|---|
| `index.md`, `manifest.yaml`, and the OKF document directories | Canonical. |
| `sources/originals/` and `sources/normalized/` | MAY be included for provenance and reproducibility. |
| `reports/` | SHOULD contain human-readable Markdown reports. |
| `.sopkb/` | Implementation state. SHOULD be derivable from canonical OKF documents where possible. |

Workbench-local bundle registries SHOULD use this directory shape:

```text
workbench/
  knowledge-bundles/
    <bundle_id>/
      manifest.yaml
      index.md
      ...
  exports/
    <bundle_id>/
      graph/
        graph.json
        triples.ttl
      export_summary.md
```

`workbench/knowledge-bundles/<bundle_id>/` is the OKF-native bundle. `workbench/exports/<bundle_id>/` contains derivative exports and is not the primary OKF model.

## 4. Export Semantics

!!! note "Export is not what makes a bundle OKF"
    `sopkb-cli export` MUST NOT be required to create an OKF bundle. The bundle is already OKF.

`sopkb-cli export` is reserved for derivative or packaged representations. Implemented today:

```text
sopkb-cli export <bundle_dir> --format graph-json
sopkb-cli export <bundle_dir> --format rdf
```

(`zip`/package and other downstream-specific formats are anticipated by this spec's design but not yet implemented — passing an unrecognized format token is silently ignored rather than erroring.)

Export lands in a sibling `exports/` directory next to the bundle, not inside it — `<bundle_dir>`'s parent gets an `exports/<bundle_id>/` subdirectory (or, if the bundle sits under a `knowledge-bundles/` folder, the export lands under that folder's own parent instead, keeping `exports/` and `knowledge-bundles/` as siblings):

```text
exports/<bundle_id>/graph/graph.json
exports/<bundle_id>/graph/triples.ttl
exports/<bundle_id>/export_summary.md
```

An `okf` export MAY exist only as a packaging/copy operation, not as the step that creates OKF compliance.

## 5. Common Markdown Document Contract

Every canonical non-index Markdown document SHOULD include YAML frontmatter followed by a human-readable Markdown body.

Common frontmatter fields:

```yaml
type: SOP Knowledge Piece
title: Human title
description: Short description
resource: relative-or-local-resource
tags: []
status: stable
generated:
  actor: sopkb/0.0.1
  date: "2026-07-31"
sources:
  - id: src-...
    title: Source title
    resource: ../sources/<source_id>.md
sopkb: {}
```

Rules:

| Field | Requirement | Description |
|---|---|---|
| `type` | MUST | Identify the document class. |
| `title` | MUST | Be human readable. |
| `resource` | SHOULD | Point to the primary related resource. |
| `tags` | SHOULD | Include useful query/filter labels. |
| `status` | SHOULD | Be one of `stable`, `draft`, or `deprecated`. |
| `generated.actor` / `generated.date` | SHOULD | Identify the generator. |
| `sources` | SHOULD | Be present when a document is traceable to source material. |

- KL4A extensions MUST live under the `sopkb` namespace.
- Unknown frontmatter fields MUST be preserved by tools that update OKF documents.

## 6. Root Index

`index.md` MUST declare the OKF version:

```yaml
---
okf_version: "0.2"
---
```

The body MUST link to:

- `sources/index.md`
- `sections/index.md`
- `concepts/index.md`
- `knowledge/index.md`
- `relations/index.md`
- `rules/index.md`
- `evidence/index.md`
- `tasks/index.md`
- `references/agent-guide.md`

It SHOULD include a bundle summary with source, section, and knowledge-piece counts.

## 7. Manifest

`manifest.yaml` MUST describe the OKF bundle and available derivative exports.

Example:

```yaml
id: glp1-healthcare-sop
version: 0.0.1
title: GLP-1 Healthcare SOP Bundle
profile: sop-knowledge-bundle
profile_version: 0.2.0
okf_version: "0.2"
status: draft
created_at: "2026-07-29T00:00:00Z"
updated_at: "2026-07-29T00:00:00Z"
sources:
  - id: primary-care-sop-glp1
    type: docx
    path: sources/primary-care-sop-glp1.md
exports:
  - type: rdf
    path: ../exports/glp1-healthcare-sop/graph/triples.ttl
  - type: graph_json
    path: ../exports/glp1-healthcare-sop/graph/graph.json
```

The manifest MUST NOT list OKF as an export required for normal bundle use.

## 8. Source Documents

Path:

```text
sources/<source_id>.md
```

Type:

```yaml
type: SOP Source
```

Required `sopkb` fields:

```yaml
sopkb:
  source_id: <source_id>
  checksum: sha256:...
  original_path: sources/originals/<file>
  normalized_path: sources/normalized/<source_id>__v<version_number>.md
  mime_type: null
  size_bytes: null
  # Source-versioning fields (immutable-source-versioning feature):
  source_version_id: <source_id>__v<version_number>
  active_version_id: <source_id>__v<version_number>
  version_number: 1
  versions: [<source_id>__v1, ...]
  lifecycle_status: active
```

`normalized_path` is version-qualified (`<source_id>__v<version_number>.md`), not a flat `<source_id>.md` — each new source version normalizes to its own file rather than overwriting the previous one.

The body MUST include:

- links to normalized section documents derived from the source;
- links to knowledge pieces derived from the source.

## 9. Section Documents

Path:

```text
sections/<source_id>/<section_id>.md
```

Type:

```yaml
type: SOP Section
```

Required `sopkb` fields:

```yaml
sopkb:
  section_id: <section_id>
  source_id: <source_id>
  ordinal: 1
  normalized_path: sources/normalized/<source_id>__v<version_number>.md
  # Source-versioning fields:
  source_version_id: <source_id>__v<version_number>
  lifecycle_status: active
```

The body MUST include:

- a link back to the source document;
- links to knowledge pieces mined from the section;
- a source excerpt.

## 10. Knowledge Piece Documents

Path:

```text
knowledge/<knowledge_item_id>.md
```

Type:

```yaml
type: SOP Knowledge Piece
```

Required `sopkb` fields:

```yaml
sopkb:
  knowledge_item_id: <knowledge_item_id>
  source_id: <source_id>
  section_id: <section_id>
  review_status: proposed
  confidence: 0.7
  span_status: exact
  evidence: ../evidence/<evidence_id>.md
  knowledge_relation: ../relations/<knowledge_relation_id>.md
  decision_rules:
    - ../rules/<rule_id>.md
  structured_statement:
    subject: Tirzepatide
    predicate: is_deferred_to
    object: Endocrinology
  # Source-versioning / knowledge-lifecycle fields:
  source_version_id: <source_id>__v<version_number>
  lifecycle_status: active
  supersedes: null
  superseded_by: null
```

The body MUST include:

- a structured statement table;
- a link to the subject concept;
- a link to evidence;
- a link to the Knowledge Relation;
- links to decision rules when present;
- source context with citation text.

Review status mapping:

| `review_status` | Mapped `status` |
|---|---|
| `proposed` | `draft` |
| `deferred` | `draft` |
| `approved` | `stable` |
| `edited` | `stable` |
| `rejected` | `deprecated` |

Human review metadata SHOULD be represented with `verified` frontmatter when a knowledge piece is approved or edited.

## 11. Knowledge Relation Documents

Path:

```text
relations/<knowledge_relation_id>.md
```

Type:

```yaml
type: SOP Knowledge Relation
```

Required `sopkb` fields:

```yaml
sopkb:
  relation:
    id: <knowledge_relation_id>
    type: Knowledge Relation
    subject:
      id: <concept_id>
      label: Tirzepatide
      text: Tirzepatide
      okf_path: concepts/<concept_id>.md
    predicate:
      id: predicate-is-deferred-to
      text: is_deferred_to
    object:
      id: object-endocrinology
      text: Endocrinology
      label: Endocrinology
    knowledge_piece_id: <knowledge_item_id>
    evidence_id: <evidence_id>
    review_status: proposed
    confidence: 0.7
    rdf_compatible: true
```

The body MUST include:

- the subject, predicate, and object assertion;
- a link to the connected knowledge piece;
- a link to evidence;
- links to decision rules when present.

Knowledge Relations are the OKF-wrapped RDF compatibility layer. They MUST remain connected to their supporting knowledge piece and evidence.

## 12. Decision Rule Documents

Path:

```text
rules/<rule_id>.md
```

Type:

```yaml
type: SOP Decision Rule
```

Required `sopkb` fields:

```yaml
sopkb:
  rule:
    id: <rule_id>
    title: Rule title
    type: SOP Decision Rule
    condition:
      fact: agent_is_tirzepatide
      label: Agent is tirzepatide
      operator: is_true
    obligation:
      action: defer_to_endocrinology
      fact: tirzepatide_deferred_to_endocrinology
      label: Tirzepatide is deferred to Endocrinology
    knowledge_item_id: <knowledge_item_id>
    source_id: <source_id>
    section_id: <section_id>
    review_status: proposed
    confidence: 0.7
    evidence_id: <evidence_id>
    relation_id: <knowledge_relation_id>
    okf_path: rules/<rule_id>.md
  knowledge_piece: ../knowledge/<knowledge_item_id>.md
  knowledge_relation: ../relations/<knowledge_relation_id>.md
  evidence: ../evidence/<evidence_id>.md
```

The body MUST include:

- the condition when present;
- the obligation;
- the review status;
- links to the connected knowledge piece, Knowledge Relation, and evidence.

## 13. Evidence Documents

Path:

```text
evidence/<evidence_id>.md
```

Type:

```yaml
type: SOP Evidence
```

Required `sopkb` fields:

```yaml
sopkb:
  knowledge_item_id: <knowledge_item_id>
  source_id: <source_id>
  section_id: <section_id>
  span_status: exact
  start_pos: 650
  end_pos: 735
```

The body MUST include:

- the evidence span;
- links to the supported knowledge piece and Knowledge Relation.

## 14. Concept Documents

Path:

```text
concepts/<concept_id>.md
```

Type:

```yaml
type: SOP Concept
```

Required `sopkb` fields:

```yaml
sopkb:
  concept_id: <concept_id>
```

The body MUST include:

- links to related knowledge pieces;
- links to related Knowledge Relations;
- links to source sections when available.

Concepts are cross-source graph anchors. If the same concept appears across multiple source documents, the concept document SHOULD link to all related knowledge pieces and relations.

## 15. Agent Task Documents

Path:

```text
tasks/<task_id>.md
```

Type:

```yaml
type: SOP Agent Task Context
```

Required `sopkb` fields:

```yaml
sopkb:
  task_id: eligibility-check
  query_terms:
    - eligibility
    - identity
    - contraindication
    - clinical review
  agent_cli: sopkb agent context <bundle_dir> --task eligibility-check
```

The body MUST describe how an agent retrieves task context and uses returned Knowledge Relations and evidence.

## 16. Agent Guide

Path:

```text
references/agent-guide.md
```

Type:

```yaml
type: SOP Agent Guide
```

The guide MUST state:

- use the bundle as read-only unless a human review workflow enables writes;
- retrieve task-scoped context through CLI or MCP;
- resolve evidence before making claims;
- treat Knowledge Relations as RDF-compatible assertions connected to evidence;
- do not infer human approval from generated/proposed knowledge.

## 17. RDF and Graph Derivatives

Graph JSON and RDF/Turtle are derivative exports from the OKF bundle:

```text
workbench/exports/<bundle_id>/graph/
  graph.json
  triples.ttl
```

`triples.ttl` MUST model Knowledge Relations as RDF-compatible assertions and SHOULD include:

- `sopkb:KnowledgeRelation`
- `sopkb:Concept`
- `sopkb:RelationObject`
- links to knowledge pieces;
- links to evidence;
- review status;
- decision rules when present.

## 18. Validation Expectations

An OKF bundle SHOULD pass these structural checks:

- required canonical directories exist;
- root `index.md` declares `okf_version`;
- every non-index Markdown document has frontmatter with `type`;
- knowledge pieces link to evidence and a Knowledge Relation;
- relations are marked `rdf_compatible: true`;
- concepts link back to knowledge and relations;
- decision rules link back to knowledge, relation, and evidence;
- agent guide exists;
- implementation state under `.sopkb/` does not override canonical OKF documents;
- no canonical document depends on a live LLM or hosted service to be read.

## 19. Implementation State

The implementation now materializes the OKF Markdown tree at the bundle root and writes derived JSON indexes and upload staging under `.sopkb/`.

Implementation rules:

1. Generate the OKF Markdown tree at the bundle root.
2. Keep implementation JSON files and upload staging under `.sopkb/`.
3. Keep `sources/originals/` and `sources/normalized/` as provenance assets.
4. Make CLI, web, MCP, and agent APIs read canonical OKF documents or `.sopkb` indexes derived from them.
5. Keep derived JSON indexes as caches that can be rebuilt from OKF.
6. Reserve `sopkb-cli export` for `graph-json`, `rdf`, archive/package, and downstream-specific formats.

## 20. Implementation Mapping

Target implementation entry points:

| Component | Path | Notes |
|---|---|---|
| Bundle creation | `v2/sopkb-rust/crates/sopkb-workbench/src/bundles.rs`, `.../ingest.rs` | |
| OKF document writer | `v2/sopkb-rust/crates/sopkb-export/src/sync.rs` | `sync_okf_bundle()` writes the canonical OKF tree directly at the bundle root (`export_dir = bundle_dir`) — this is done, not pending. |
| Agent consumption layer | `v2/sopkb-rust/crates/sopkb-derive/src/context.rs` | |
| MCP read-only layer | `v2/sopkb-rust/bin/sopkb-mcp/src/` (`jsonrpc.rs`, `tools.rs`) | |
| Structural tests | `v2/sopkb-rust/crates/sopkb-export/tests/phase5_v1_diff.rs`, `.../sopkb-derive/tests/phase4_v1_diff.rs` | Byte-level differential tests against the frozen reference output checked into the fixtures tree. |
