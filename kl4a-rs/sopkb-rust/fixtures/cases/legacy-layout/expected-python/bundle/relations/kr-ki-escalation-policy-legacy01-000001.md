---
type: SOP Knowledge Relation
title: kr-ki-escalation-policy-legacy01-000001
description: Escalation Policy requires On-call staff must confirm receipt of every
  critical alert.
resource: ../knowledge/ki-escalation-policy-legacy01-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-policy-legacy01
  title: Escalation Policy
  resource: ../sources/escalation-policy-legacy01.md
sopkb:
  relation:
    id: kr-ki-escalation-policy-legacy01-000001
    type: Knowledge Relation
    subject:
      id: concept-escalation-policy
      label: Escalation Policy
      text: Escalation Policy
      okf_path: concepts/concept-escalation-policy.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-on-call-staff-must-confirm-receipt-of-every-critical-alert
      text: On-call staff must confirm receipt of every critical alert within 15 minutes.
      label: On-call staff must confirm receipt of every critical alert
    knowledge_piece_id: ki-escalation-policy-legacy01-000001
    evidence_id: evidence-ki-escalation-policy-legacy01-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-policy-legacy01-000001

## Assertion

- Subject: [Escalation Policy](../concepts/concept-escalation-policy.md)
- Predicate: `requires`
- Object: On-call staff must confirm receipt of every critical alert within 15 minutes.

## Connected Knowledge

- Knowledge piece: [ki-escalation-policy-legacy01-000001](../knowledge/ki-escalation-policy-legacy01-000001.md)
- Evidence: [evidence-ki-escalation-policy-legacy01-000001](../evidence/evidence-ki-escalation-policy-legacy01-000001.md)
- Decision rule: [Escalation Policy](../rules/rule-ki-escalation-policy-legacy01-000001-requires.md)
