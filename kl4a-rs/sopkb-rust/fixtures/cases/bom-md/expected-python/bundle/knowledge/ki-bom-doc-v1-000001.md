---
type: SOP Knowledge Piece
title: Document
description: Staff must confirm patient identity before proceeding.
resource: ../sections/bom-doc/section-bom-doc-001.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-bom-doc
  title: bom doc
  resource: ../sources/bom-doc.md
sopkb:
  knowledge_item_id: ki-bom-doc-v1-000001
  source_id: bom-doc
  source_version_id: bom-doc:v1
  section_id: section-bom-doc-001
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-bom-doc-v1-000001.md
  knowledge_relation: ../relations/kr-ki-bom-doc-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-bom-doc-v1-000001-confirm-patient-identity.md
  structured_statement:
    subject: Document
    predicate: requires
    object: Staff must confirm patient identity before proceeding.
---
# Document

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Document](../concepts/concept-document.md) |
| Predicate | `requires` |
| Object | Staff must confirm patient identity before proceeding. |

## Evidence

- [evidence-ki-bom-doc-v1-000001](../evidence/evidence-ki-bom-doc-v1-000001.md)

## Relations

- [kr-ki-bom-doc-v1-000001](../relations/kr-ki-bom-doc-v1-000001.md)

## Decision Rules

- [Confirm patient identity](../rules/rule-ki-bom-doc-v1-000001-confirm-patient-identity.md)

## Source Context

Staff must confirm patient identity before proceeding. [^src-bom-doc]

[^src-bom-doc]: bom doc section `section-bom-doc-001`.
