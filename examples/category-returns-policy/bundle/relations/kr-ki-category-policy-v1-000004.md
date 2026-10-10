---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000004
description: 2. Major Appliances requires Major Appliances must include original packaging
  and all accessories.
resource: ../knowledge/ki-category-policy-v1-000004.md
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
    id: kr-ki-category-policy-v1-000004
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
      id: object-major-appliances-must-include-original-packaging-and-all-accesso
      text: Major Appliances must include original packaging and all accessories to
        avoid a 20% restocking fee.
      label: Major Appliances must include original packaging and all accessories
    knowledge_piece_id: ki-category-policy-v1-000004
    evidence_id: evidence-ki-category-policy-v1-000004
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000004

## Assertion

- Subject: [2. Major Appliances](../concepts/concept-2-major-appliances.md)
- Predicate: `requires`
- Object: Major Appliances must include original packaging and all accessories to avoid a 20% restocking fee.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000004](../knowledge/ki-category-policy-v1-000004.md)
- Evidence: [evidence-ki-category-policy-v1-000004](../evidence/evidence-ki-category-policy-v1-000004.md)
- Decision rule: [2. Major Appliances](../rules/rule-ki-category-policy-v1-000004-requires.md)
