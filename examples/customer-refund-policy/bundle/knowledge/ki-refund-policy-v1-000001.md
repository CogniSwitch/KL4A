---
type: SOP Knowledge Piece
title: Refund Eligibility
description: Customers must submit refund requests within 30 days of purchase.
resource: ../sections/refund-policy/section-refund-policy-002.md
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
  knowledge_item_id: ki-refund-policy-v1-000001
  source_id: refund-policy
  source_version_id: refund-policy:v1
  section_id: section-refund-policy-002
  review_status: proposed
  lifecycle_status: active
  confidence: 0.82
  span_status: exact
  evidence: ../evidence/evidence-ki-refund-policy-v1-000001.md
  knowledge_relation: ../relations/kr-ki-refund-policy-v1-000001.md
  decision_rules:
  - ../rules/rule-ki-refund-policy-v1-000001-requires.md
  structured_statement:
    subject: Refund Eligibility
    predicate: requires
    object: Customers must submit refund requests within 30 days of purchase.
---
# Refund Eligibility

## Structured Statement

| Field | Value |
| --- | --- |
| Subject | [Refund Eligibility](../concepts/concept-refund-eligibility.md) |
| Predicate | `requires` |
| Object | Customers must submit refund requests within 30 days of purchase. |

## Evidence

- [evidence-ki-refund-policy-v1-000001](../evidence/evidence-ki-refund-policy-v1-000001.md)

## Relations

- [kr-ki-refund-policy-v1-000001](../relations/kr-ki-refund-policy-v1-000001.md)

## Decision Rules

- [Refund Eligibility](../rules/rule-ki-refund-policy-v1-000001-requires.md)

## Source Context

Customers must submit refund requests within 30 days of purchase. [^src-refund-policy]

[^src-refund-policy]: refund policy section `section-refund-policy-002`.
