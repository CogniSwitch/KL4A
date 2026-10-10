---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000010
description: 5. Prescription Eyewear requires Prescription Eyewear must include original
  packaging and all accessories.
resource: ../knowledge/ki-category-policy-v1-000010.md
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
    id: kr-ki-category-policy-v1-000010
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
      id: object-prescription-eyewear-must-include-original-packaging-and-all-acc
      text: Prescription Eyewear must include original packaging and all accessories
        to avoid a 0% restocking fee.
      label: Prescription Eyewear must include original packaging and all accessories
    knowledge_piece_id: ki-category-policy-v1-000010
    evidence_id: evidence-ki-category-policy-v1-000010
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000010

## Assertion

- Subject: [5. Prescription Eyewear](../concepts/concept-5-prescription-eyewear.md)
- Predicate: `requires`
- Object: Prescription Eyewear must include original packaging and all accessories to avoid a 0% restocking fee.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000010](../knowledge/ki-category-policy-v1-000010.md)
- Evidence: [evidence-ki-category-policy-v1-000010](../evidence/evidence-ki-category-policy-v1-000010.md)
- Decision rule: [5. Prescription Eyewear](../rules/rule-ki-category-policy-v1-000010-requires.md)
