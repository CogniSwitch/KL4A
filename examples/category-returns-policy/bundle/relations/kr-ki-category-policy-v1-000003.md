---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000003
description: 2. Major Appliances requires Major Appliances must be returned within
  45 days of.
resource: ../knowledge/ki-category-policy-v1-000003.md
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
    id: kr-ki-category-policy-v1-000003
    type: Knowledge Relation
    subject:
      id: concept-2-major-appliances
      label: 2. Major Appliances
      text: 2. Major Appliances
      okf_path: concepts/concept-2-major-appliances.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-major-appliances-must-be-returned-within-45-days-of
      text: Major Appliances must be returned within 45 days of delivery to qualify
        for a refund.
      label: Major Appliances must be returned within 45 days of
    knowledge_piece_id: ki-category-policy-v1-000003
    evidence_id: evidence-ki-category-policy-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000003

## Assertion

- Subject: [2. Major Appliances](../concepts/concept-2-major-appliances.md)
- Predicate: `requires`
- Object: Major Appliances must be returned within 45 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000003](../knowledge/ki-category-policy-v1-000003.md)
- Evidence: [evidence-ki-category-policy-v1-000003](../evidence/evidence-ki-category-policy-v1-000003.md)
- Decision rule: [2. Major Appliances](../rules/rule-ki-category-policy-v1-000003-requires.md)
