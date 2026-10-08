---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000001
description: 1. General Return Window requires Customers must initiate a return within
  30 days of.
resource: ../knowledge/ki-support-policy-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-support-policy
  title: support policy
  resource: ../sources/support-policy.md
sopkb:
  relation:
    id: kr-ki-support-policy-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-1-general-return-window
      label: 1. General Return Window
      text: 1. General Return Window
      okf_path: concepts/concept-1-general-return-window.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-customers-must-initiate-a-return-within-30-days-of
      text: Customers must initiate a return within 30 days of delivery to receive
        a full refund.
      label: Customers must initiate a return within 30 days of
    knowledge_piece_id: ki-support-policy-v1-000001
    evidence_id: evidence-ki-support-policy-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000001

## Assertion

- Subject: [1. General Return Window](../concepts/concept-1-general-return-window.md)
- Predicate: `requires`
- Object: Customers must initiate a return within 30 days of delivery to receive a full refund.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000001](../knowledge/ki-support-policy-v1-000001.md)
- Evidence: [evidence-ki-support-policy-v1-000001](../evidence/evidence-ki-support-policy-v1-000001.md)
- Decision rule: [1. General Return Window](../rules/rule-ki-support-policy-v1-000001-requires.md)
