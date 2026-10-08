---
type: SOP Knowledge Relation
title: kr-ki-follow-up-monitoring-procedure-v1-000002
description: Follow-up Monitoring records Staff must record dose tolerance and adverse
  event symptoms.
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000002.md
tags:
- relation
- rdf-compatible
- records
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-follow-up-monitoring-procedure
  title: follow up monitoring procedure
  resource: ../sources/follow-up-monitoring-procedure.md
sopkb:
  relation:
    id: kr-ki-follow-up-monitoring-procedure-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-follow-up-monitoring
      label: Follow-up Monitoring
      text: Follow-up Monitoring
      okf_path: concepts/concept-follow-up-monitoring.md
    predicate:
      id: predicate-records
      text: records
    object:
      id: object-staff-must-record-dose-tolerance-and-adverse-event-symptoms
      text: Staff must record dose tolerance and adverse event symptoms during follow-up.
      label: Staff must record dose tolerance and adverse event symptoms
    knowledge_piece_id: ki-follow-up-monitoring-procedure-v1-000002
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-follow-up-monitoring-procedure-v1-000002

## Assertion

- Subject: [Follow-up Monitoring](../concepts/concept-follow-up-monitoring.md)
- Predicate: `records`
- Object: Staff must record dose tolerance and adverse event symptoms during follow-up.

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000002](../knowledge/ki-follow-up-monitoring-procedure-v1-000002.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000002](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000002.md)
- Decision rule: [Follow-up Monitoring](../rules/rule-ki-follow-up-monitoring-procedure-v1-000002-records.md)
