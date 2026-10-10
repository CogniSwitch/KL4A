---
type: SOP Knowledge Relation
title: kr-ki-category-policy-v1-000015
description: 8. Hand Tools requires Hand Tools must be returned within 60 days of.
resource: ../knowledge/ki-category-policy-v1-000015.md
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
    id: kr-ki-category-policy-v1-000015
    type: Knowledge Relation
    subject:
      id: concept-8-hand-tools
      label: 8. Hand Tools
      text: 8. Hand Tools
      okf_path: concepts/concept-8-hand-tools.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-hand-tools-must-be-returned-within-60-days-of
      text: Hand Tools must be returned within 60 days of delivery to qualify for
        a refund.
      label: Hand Tools must be returned within 60 days of
    knowledge_piece_id: ki-category-policy-v1-000015
    evidence_id: evidence-ki-category-policy-v1-000015
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-category-policy-v1-000015

## Assertion

- Subject: [8. Hand Tools](../concepts/concept-8-hand-tools.md)
- Predicate: `requires`
- Object: Hand Tools must be returned within 60 days of delivery to qualify for a refund.

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000015](../knowledge/ki-category-policy-v1-000015.md)
- Evidence: [evidence-ki-category-policy-v1-000015](../evidence/evidence-ki-category-policy-v1-000015.md)
- Decision rule: [8. Hand Tools](../rules/rule-ki-category-policy-v1-000015-requires.md)
