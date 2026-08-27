---
type: SOP Knowledge Relation
title: kr-ki-escalation-runbook-v1-000001
description: Initial Notification requires On-call staff must confirm receipt of the
  alert within.
resource: ../knowledge/ki-escalation-runbook-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-escalation-runbook
  title: escalation runbook
  resource: ../sources/escalation-runbook.md
sopkb:
  relation:
    id: kr-ki-escalation-runbook-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-initial-notification
      label: Initial Notification
      text: Initial Notification
      okf_path: concepts/concept-initial-notification.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-on-call-staff-must-confirm-receipt-of-the-alert-within
      text: On-call staff must confirm receipt of the alert within 15 minutes.
      label: On-call staff must confirm receipt of the alert within
    knowledge_piece_id: ki-escalation-runbook-v1-000001
    evidence_id: evidence-ki-escalation-runbook-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-runbook-v1-000001

## Assertion

- Subject: [Initial Notification](../concepts/concept-initial-notification.md)
- Predicate: `requires`
- Object: On-call staff must confirm receipt of the alert within 15 minutes.

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000001](../knowledge/ki-escalation-runbook-v1-000001.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000001](../evidence/evidence-ki-escalation-runbook-v1-000001.md)
- Decision rule: [Initial Notification](../rules/rule-ki-escalation-runbook-v1-000001-requires.md)
