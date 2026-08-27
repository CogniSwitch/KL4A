---
type: SOP Knowledge Relation
title: kr-ki-follow-up-monitoring-procedure-v1-000001
description: Follow-up Monitoring should Patients should receive follow-up contact
  within 14 days after.
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000001.md
tags:
- relation
- rdf-compatible
- should
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
    id: kr-ki-follow-up-monitoring-procedure-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-follow-up-monitoring
      label: Follow-up Monitoring
      text: Follow-up Monitoring
      okf_path: concepts/concept-follow-up-monitoring.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-patients-should-receive-follow-up-contact-within-14-days-after
      text: Patients should receive follow-up contact within 14 days after GLP-1 therapy
        initiation.
      label: Patients should receive follow-up contact within 14 days after
    knowledge_piece_id: ki-follow-up-monitoring-procedure-v1-000001
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-follow-up-monitoring-procedure-v1-000001

## Assertion

- Subject: [Follow-up Monitoring](../concepts/concept-follow-up-monitoring.md)
- Predicate: `should`
- Object: Patients should receive follow-up contact within 14 days after GLP-1 therapy initiation.

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000001](../knowledge/ki-follow-up-monitoring-procedure-v1-000001.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000001](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000001.md)
- Decision rule: [Follow-up Monitoring](../rules/rule-ki-follow-up-monitoring-procedure-v1-000001-should.md)
