---
type: SOP Knowledge Relation
title: kr-ki-support-policy-v1-000002
description: 1. General Return Window requires Items returned between day 31 and day
  45 must.
resource: ../knowledge/ki-support-policy-v1-000002.md
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
    id: kr-ki-support-policy-v1-000002
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
      id: object-items-returned-between-day-31-and-day-45-must
      text: Items returned between day 31 and day 45 must be processed as store credit
        only.
      label: Items returned between day 31 and day 45 must
    knowledge_piece_id: ki-support-policy-v1-000002
    evidence_id: evidence-ki-support-policy-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-support-policy-v1-000002

## Assertion

- Subject: [1. General Return Window](../concepts/concept-1-general-return-window.md)
- Predicate: `requires`
- Object: Items returned between day 31 and day 45 must be processed as store credit only.

## Connected Knowledge

- Knowledge piece: [ki-support-policy-v1-000002](../knowledge/ki-support-policy-v1-000002.md)
- Evidence: [evidence-ki-support-policy-v1-000002](../evidence/evidence-ki-support-policy-v1-000002.md)
- Decision rule: [1. General Return Window](../rules/rule-ki-support-policy-v1-000002-requires.md)
