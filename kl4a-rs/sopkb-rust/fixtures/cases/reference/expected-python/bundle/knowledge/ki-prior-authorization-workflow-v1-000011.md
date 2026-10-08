---
type: SOP Knowledge Piece
title: Prior Authorization Requirements
description: Staff must attach diagnosis evidence and prior therapy documentation
  when required by the payer.
resource: ../sections/prior-authorization-workflow/section-prior-authorization-workflow-002.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-prior-authorization-workflow
  title: prior authorization workflow
  resource: ../sources/prior-authorization-workflow.md
sopkb:
  knowledge_item_id: ki-prior-authorization-workflow-v1-000011
  source_id: prior-authorization-workflow
  source_version_id: prior-authorization-workflow:v1
  section_id: section-prior-authorization-workflow-002
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-prior-authorization-workflow-v1-000011.md
  knowledge_relation: ../relations/kr-ki-prior-authorization-workflow-v1-000011.md
  decision_rules:
  - ../rules/rule-ki-prior-authorization-workflow-v1-000011-requires.md
  structured_statement:
    subject: Prior Authorization Requirements
    predicate: requires
    object: Staff must attach diagnosis evidence and prior therapy documentation when
      required by the payer.
---
# Prior Authorization Requirements

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Prior Authorization Requirements](../concepts/concept-prior-authorization-requirements.md) |
| Predicate | `requires` |
| Object | Staff must attach diagnosis evidence and prior therapy documentation when required by the payer. |

## Evidence

- [evidence-ki-prior-authorization-workflow-v1-000011](../evidence/evidence-ki-prior-authorization-workflow-v1-000011.md)

## Relations

- [kr-ki-prior-authorization-workflow-v1-000011](../relations/kr-ki-prior-authorization-workflow-v1-000011.md)

## Decision Rules

- [Prior Authorization Requirements](../rules/rule-ki-prior-authorization-workflow-v1-000011-requires.md)

## Source Context

Staff must attach diagnosis evidence and prior therapy documentation when required by the payer. [^src-prior-authorization-workflow]

[^src-prior-authorization-workflow]: prior authorization workflow section `section-prior-authorization-workflow-002`.
