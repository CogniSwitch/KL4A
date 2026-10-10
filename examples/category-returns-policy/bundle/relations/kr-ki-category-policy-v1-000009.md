---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000009
description: 5. Prescription Eyewear requires Prescription Eyewear must be returned
  within 7 days of.
resource: ../knowledge/ki-category-policy-v1-000009.md
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
    id: kr-ki-category-policy-v1-000009
    type: Knowledge Relation
    subject:
      id: concept-5-prescription-eyewear
      label: 5. Prescription Eyewear
      text: 5. Prescription Eyewear
      okf_path: concepts/concept-5-prescription-eyewear.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-prescription-eyewear-must-be-returned-within-7-days-of
      text: Prescription Eyewear must be returned within 7 days of delivery to qualify
        for a refund.
      label: Prescription Eyewear must be returned within 7 days of
    knowledge_piece_id: ki-category-policy-v1-000009
    evidence_id: evidence-ki-category-policy-v1-000009
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000009

## Assertion

- Subject: [5. Prescription Eyewear](../concepts/concept-5-prescription-eyewear.md)
- Predicate: `requires`
- Object: Prescription Eyewear must be returned within 7 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000009](../knowledge/ki-category-policy-v1-000009.md)
- Evidence: [evidence-ki-category-policy-v1-000009](../evidence/evidence-ki-category-policy-v1-000009.md)
- Decision rule: [5. Prescription Eyewear](../rules/rule-ki-category-policy-v1-000009-requires.md)
