---
type: SOP Knowledge Relation
title: kr-ki-escalation-runbook-v1-000005
description: Patient Communication requires Care coordinators must confirm the patient
  has been informed.
resource: ../knowledge/ki-escalation-runbook-v1-000005.md
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
    id: kr-ki-escalation-runbook-v1-000005
    type: Knowledge Relation
    subject:
      id: concept-patient-communication
      label: Patient Communication
      text: Patient Communication
      okf_path: concepts/concept-patient-communication.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-care-coordinators-must-confirm-the-patient-has-been-informed
      text: Care coordinators must confirm the patient has been informed of the outcome.
      label: Care coordinators must confirm the patient has been informed
    knowledge_piece_id: ki-escalation-runbook-v1-000005
    evidence_id: evidence-ki-escalation-runbook-v1-000005
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-escalation-runbook-v1-000005

## Assertion

- Subject: [Patient Communication](../concepts/concept-patient-communication.md)
- Predicate: `requires`
- Object: Care coordinators must confirm the patient has been informed of the outcome.

## Connected Knowledge

- Knowledge piece: [ki-escalation-runbook-v1-000005](../knowledge/ki-escalation-runbook-v1-000005.md)
- Evidence: [evidence-ki-escalation-runbook-v1-000005](../evidence/evidence-ki-escalation-runbook-v1-000005.md)
- Decision rule: [Patient Communication](../rules/rule-ki-escalation-runbook-v1-000005-requires.md)
