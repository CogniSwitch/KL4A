---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000001
description: 1. Small Appliances requires Small Appliances must be returned within
  21 days of.
resource: ../knowledge/ki-category-policy-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-category-policy
  title: category policy
  resource: ../sources/category-policy.md
sopkb:
  relation:
    id: kr-ki-category-policy-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-1-small-appliances
      label: 1. Small Appliances
      text: 1. Small Appliances
      okf_path: concepts/concept-1-small-appliances.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-small-appliances-must-be-returned-within-21-days-of
      text: Small Appliances must be returned within 21 days of delivery to qualify
        for a refund.
      label: Small Appliances must be returned within 21 days of
    knowledge_piece_id: ki-category-policy-v1-000001
    evidence_id: evidence-ki-category-policy-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000001

## Assertion

- Subject: [1. Small Appliances](../concepts/concept-1-small-appliances.md)
- Predicate: `requires`
- Object: Small Appliances must be returned within 21 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000001](../knowledge/ki-category-policy-v1-000001.md)
- Evidence: [evidence-ki-category-policy-v1-000001](../evidence/evidence-ki-category-policy-v1-000001.md)
- Decision rule: [1. Small Appliances](../rules/rule-ki-category-policy-v1-000001-requires.md)
