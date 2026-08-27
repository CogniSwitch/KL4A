---
type: SOP Knowledge Piece
title: Eligibility Requirements
description: Clinicians must confirm patient identity before reviewing therapy eligibility.
resource: ../sections/preamble/section-preamble-001.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-preamble
  title: preamble
  resource: ../sources/preamble.md
sopkb:
  knowledge_item_id: ki-preamble-v1-000001
  source_id: preamble
  source_version_id: preamble:v1
  section_id: section-preamble-001
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-preamble-v1-000001.md
  knowledge_relation: ../relations/kr-ki-preamble-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-preamble-v1-000001-confirm-patient-identity.md
  structured_statement:
    subject: Eligibility Requirements
    predicate: requires
    object: Clinicians must confirm patient identity before reviewing therapy eligibility.
---
# Eligibility Requirements

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Eligibility Requirements](../concepts/concept-eligibility-requirements.md) |
| Predicate | `requires` |
| Object | Clinicians must confirm patient identity before reviewing therapy eligibility. |

## Evidence

- [evidence-ki-preamble-v1-000001](../evidence/evidence-ki-preamble-v1-000001.md)

## Relations

- [kr-ki-preamble-v1-000001](../relations/kr-ki-preamble-v1-000001.md)

## Decision Rules

- [Confirm patient identity](../rules/rule-ki-preamble-v1-000001-confirm-patient-identity.md)

## Source Context

Clinicians must confirm patient identity before reviewing therapy eligibility. [^src-preamble]

[^src-preamble]: preamble section `section-preamble-001`.
