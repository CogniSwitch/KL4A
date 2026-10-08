---
type: SOP Knowledge Relation
title: kr-ki-escalation-runbook-v1-000004
description: Legal Notification should Staff should confirm legal has been notified
  for any.
resource: ../knowledge/ki-escalation-runbook-v1-000004.md
tags:
- relation
- rdf-compatible
- should
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
    id: kr-ki-escalation-runbook-v1-000004
    type: Knowledge Relation
    subject:
      id: concept-legal-notification
      label: Legal Notification
      text: Legal Notification
      okf_path: concepts/concept-legal-notification.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-staff-should-confirm-legal-has-been-notified-for-any
      text: Staff should confirm legal has been notified for any reportable event.
      label: Staff should confirm legal has been notified for any
    knowledge_piece_id: ki-escalation-runbook-v1-000004
    evidence_id: evidence-ki-escalation-runbook-v1-000004
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-runbook-v1-000004

## Assertion

- Subject: [Legal Notification](../concepts/concept-legal-notification.md)
- Predicate: `should`
- Object: Staff should confirm legal has been notified for any reportable event.

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000004](../knowledge/ki-escalation-runbook-v1-000004.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000004](../evidence/evidence-ki-escalation-runbook-v1-000004.md)
- Decision rule: [Legal Notification](../rules/rule-ki-escalation-runbook-v1-000004-should.md)
