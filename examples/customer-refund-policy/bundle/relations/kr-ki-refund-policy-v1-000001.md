---
type: SOP Knowledge Relation
title: kr-ki-refund-policy-v1-000001
description: Refund Eligibility requires Customers must submit refund requests within
  30 days of.
resource: ../knowledge/ki-refund-policy-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-refund-policy
  title: refund policy
  resource: ../sources/refund-policy.md
sopkb:
  relation:
    id: kr-ki-refund-policy-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-refund-eligibility
      label: Refund Eligibility
      text: Refund Eligibility
      okf_path: concepts/concept-refund-eligibility.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-customers-must-submit-refund-requests-within-30-days-of
      text: Customers must submit refund requests within 30 days of purchase.
      label: Customers must submit refund requests within 30 days of
    knowledge_piece_id: ki-refund-policy-v1-000001
    evidence_id: evidence-ki-refund-policy-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-refund-policy-v1-000001

## Assertion

- Subject: [Refund Eligibility](../concepts/concept-refund-eligibility.md)
- Predicate: `requires`
- Object: Customers must submit refund requests within 30 days of purchase.

## Connected Knowledge

- Knowledge piece: [ki-refund-policy-v1-000001](../knowledge/ki-refund-policy-v1-000001.md)
- Evidence: [evidence-ki-refund-policy-v1-000001](../evidence/evidence-ki-refund-policy-v1-000001.md)
- Decision rule: [Refund Eligibility](../rules/rule-ki-refund-policy-v1-000001-requires.md)
