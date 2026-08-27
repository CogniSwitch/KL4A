---
type: SOP Knowledge Piece
title: Document
description: Staff must confirm patient identity before dispensing medication.
resource: ../sections/no-heading/section-no-heading-001.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-no-heading
  title: no heading
  resource: ../sources/no-heading.md
sopkb:
  knowledge_item_id: ki-no-heading-v1-000001
  source_id: no-heading
  source_version_id: no-heading:v1
  section_id: section-no-heading-001
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-no-heading-v1-000001.md
  knowledge_relation: ../relations/kr-ki-no-heading-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-no-heading-v1-000001-confirm-patient-identity.md
  structured_statement:
    subject: Document
    predicate: requires
    object: Staff must confirm patient identity before dispensing medication.
---
# Document

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Document](../concepts/concept-document.md) |
| Predicate | `requires` |
| Object | Staff must confirm patient identity before dispensing medication. |

## Evidence

- [evidence-ki-no-heading-v1-000001](../evidence/evidence-ki-no-heading-v1-000001.md)

## Relations

- [kr-ki-no-heading-v1-000001](../relations/kr-ki-no-heading-v1-000001.md)

## Decision Rules

- [Confirm patient identity](../rules/rule-ki-no-heading-v1-000001-confirm-patient-identity.md)

## Source Context

Staff must confirm patient identity before dispensing medication. [^src-no-heading]

[^src-no-heading]: no heading section `section-no-heading-001`.
