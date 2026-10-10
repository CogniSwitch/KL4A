---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000003
description: 1. General Return Window requires Items returned after day 45 must not
  receive any.
resource: ../knowledge/ki-support-policy-v1-000003.md
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
    id: kr-ki-support-policy-v1-000003
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
      id: object-items-returned-after-day-45-must-not-receive-any
      text: Items returned after day 45 must not receive any refund or credit.
      label: Items returned after day 45 must not receive any
    knowledge_piece_id: ki-support-policy-v1-000003
    evidence_id: evidence-ki-support-policy-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000003

## Assertion

- Subject: [1. General Return Window](../concepts/concept-1-general-return-window.md)
- Predicate: `requires`
- Object: Items returned after day 45 must not receive any refund or credit.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000003](../knowledge/ki-support-policy-v1-000003.md)
- Evidence: [evidence-ki-support-policy-v1-000003](../evidence/evidence-ki-support-policy-v1-000003.md)
- Decision rule: [1. General Return Window](../rules/rule-ki-support-policy-v1-000003-requires.md)
