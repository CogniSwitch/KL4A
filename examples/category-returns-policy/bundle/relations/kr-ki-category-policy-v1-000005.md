---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000005
description: 3. Indoor Furniture requires Indoor Furniture must be returned within
  30 days of.
resource: ../knowledge/ki-category-policy-v1-000005.md
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
    id: kr-ki-category-policy-v1-000005
    type: Knowledge Relation
    subject:
      id: concept-3-indoor-furniture
      label: 3. Indoor Furniture
      text: 3. Indoor Furniture
      okf_path: concepts/concept-3-indoor-furniture.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-indoor-furniture-must-be-returned-within-30-days-of
      text: Indoor Furniture must be returned within 30 days of delivery to qualify
        for a refund.
      label: Indoor Furniture must be returned within 30 days of
    knowledge_piece_id: ki-category-policy-v1-000005
    evidence_id: evidence-ki-category-policy-v1-000005
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000005

## Assertion

- Subject: [3. Indoor Furniture](../concepts/concept-3-indoor-furniture.md)
- Predicate: `requires`
- Object: Indoor Furniture must be returned within 30 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000005](../knowledge/ki-category-policy-v1-000005.md)
- Evidence: [evidence-ki-category-policy-v1-000005](../evidence/evidence-ki-category-policy-v1-000005.md)
- Decision rule: [3. Indoor Furniture](../rules/rule-ki-category-policy-v1-000005-requires.md)
