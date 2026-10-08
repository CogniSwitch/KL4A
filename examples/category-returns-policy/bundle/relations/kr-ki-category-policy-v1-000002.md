---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000002
description: 1. Small Appliances requires Small Appliances must include original packaging
  and all accessories.
resource: ../knowledge/ki-category-policy-v1-000002.md
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
    id: kr-ki-category-policy-v1-000002
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
      id: object-small-appliances-must-include-original-packaging-and-all-accesso
      text: Small Appliances must include original packaging and all accessories to
        avoid a 10% restocking fee.
      label: Small Appliances must include original packaging and all accessories
    knowledge_piece_id: ki-category-policy-v1-000002
    evidence_id: evidence-ki-category-policy-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000002

## Assertion

- Subject: [1. Small Appliances](../concepts/concept-1-small-appliances.md)
- Predicate: `requires`
- Object: Small Appliances must include original packaging and all accessories to avoid a 10% restocking fee.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000002](../knowledge/ki-category-policy-v1-000002.md)
- Evidence: [evidence-ki-category-policy-v1-000002](../evidence/evidence-ki-category-policy-v1-000002.md)
- Decision rule: [1. Small Appliances](../rules/rule-ki-category-policy-v1-000002-requires.md)
