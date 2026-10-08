---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000013
description: 7. Power Tools requires Power Tools must be returned within 30 days of.
resource: ../knowledge/ki-category-policy-v1-000013.md
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
    id: kr-ki-category-policy-v1-000013
    type: Knowledge Relation
    subject:
      id: concept-7-power-tools
      label: 7. Power Tools
      text: 7. Power Tools
      okf_path: concepts/concept-7-power-tools.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-power-tools-must-be-returned-within-30-days-of
      text: Power Tools must be returned within 30 days of delivery to qualify for
        a refund.
      label: Power Tools must be returned within 30 days of
    knowledge_piece_id: ki-category-policy-v1-000013
    evidence_id: evidence-ki-category-policy-v1-000013
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000013

## Assertion

- Subject: [7. Power Tools](../concepts/concept-7-power-tools.md)
- Predicate: `requires`
- Object: Power Tools must be returned within 30 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000013](../knowledge/ki-category-policy-v1-000013.md)
- Evidence: [evidence-ki-category-policy-v1-000013](../evidence/evidence-ki-category-policy-v1-000013.md)
- Decision rule: [7. Power Tools](../rules/rule-ki-category-policy-v1-000013-requires.md)
