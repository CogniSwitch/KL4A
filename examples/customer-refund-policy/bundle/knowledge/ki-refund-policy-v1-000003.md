---
type: SOP Knowledge Piece
title: Refund Approval
description: Refund requests above $1,000 must receive finance approval before processing.
resource: ../sections/refund-policy/section-refund-policy-003.md
tags:
- knowledge
- requires
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-refund-policy
  title: refund policy
  resource: ../sources/refund-policy.md
sopkb:
  knowledge_item_id: ki-refund-policy-v1-000003
  source_id: refund-policy
  source_version_id: refund-policy:v1
  section_id: section-refund-policy-003
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-refund-policy-v1-000003.md
  knowledge_relation: ../relations/kr-ki-refund-policy-v1-000003.md
  decision_rules:
  - ../rules/rule-ki-refund-policy-v1-000003-requires.md
  structured_statement:
    subject: Refund Approval
    predicate: requires
    object: Refund requests above $1,000 must receive finance approval before processing.
---
# Refund Approval

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Refund Approval](../concepts/concept-refund-approval.md) |
| Predicate | `requires` |
| Object | Refund requests above $1,000 must receive finance approval before processing. |

## Evidence

- [evidence-ki-refund-policy-v1-000003](../evidence/evidence-ki-refund-policy-v1-000003.md)

## Relations

- [kr-ki-refund-policy-v1-000003](../relations/kr-ki-refund-policy-v1-000003.md)

## Decision Rules

- [Refund Approval](../rules/rule-ki-refund-policy-v1-000003-requires.md)

## Source Context

Refund requests above $1,000 must receive finance approval before processing. [^src-refund-policy]

[^src-refund-policy]: refund policy section `section-refund-policy-003`.
