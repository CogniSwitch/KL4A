---
type: SOP Knowledge Relation
title: kr-ki-follow-up-monitoring-procedure-v1-000003
description: Dose Titration requires Clinicians must confirm patient tolerance before
  dose escalation..
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000003.md
tags:
- relation
- rdf-compatible
- requires
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
    id: kr-ki-follow-up-monitoring-procedure-v1-000003
    type: Knowledge Relation
    subject:
      id: concept-dose-titration
      label: Dose Titration
      text: Dose Titration
      okf_path: concepts/concept-dose-titration.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-clinicians-must-confirm-patient-tolerance-before-dose-escalation
      text: Clinicians must confirm patient tolerance before dose escalation.
      label: Clinicians must confirm patient tolerance before dose escalation.
    knowledge_piece_id: ki-follow-up-monitoring-procedure-v1-000003
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000003
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-follow-up-monitoring-procedure-v1-000003

## Assertion

- Subject: [Dose Titration](../concepts/concept-dose-titration.md)
- Predicate: `requires`
- Object: Clinicians must confirm patient tolerance before dose escalation.

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000003](../knowledge/ki-follow-up-monitoring-procedure-v1-000003.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000003](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000003.md)
- Decision rule: [Dose Titration](../rules/rule-ki-follow-up-monitoring-procedure-v1-000003-requires.md)
