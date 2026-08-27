---
type: SOP Knowledge Relation
title: kr-ki-weird-headings-v1-000001
description: 'Purpose ## requires Staff must confirm the escalation procedure described
  below..'
resource: ../knowledge/ki-weird-headings-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-weird-headings
  title: weird headings
  resource: ../sources/weird-headings.md
sopkb:
  relation:
    id: kr-ki-weird-headings-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-purpose
      label: 'Purpose ##'
      text: 'Purpose ##'
      okf_path: concepts/concept-purpose.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-the-escalation-procedure-described-below
      text: Staff must confirm the escalation procedure described below.
      label: Staff must confirm the escalation procedure described below.
    knowledge_piece_id: ki-weird-headings-v1-000001
    evidence_id: evidence-ki-weird-headings-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-weird-headings-v1-000001

## Assertion

- Subject: [Purpose ##](../concepts/concept-purpose.md)
- Predicate: `requires`
- Object: Staff must confirm the escalation procedure described below.

## Connected Knowledge

- Knowledge piece: [ki-weird-headings-v1-000001](../knowledge/ki-weird-headings-v1-000001.md)
- Evidence: [evidence-ki-weird-headings-v1-000001](../evidence/evidence-ki-weird-headings-v1-000001.md)
- Decision rule: [Purpose ##](../rules/rule-ki-weird-headings-v1-000001-requires.md)
