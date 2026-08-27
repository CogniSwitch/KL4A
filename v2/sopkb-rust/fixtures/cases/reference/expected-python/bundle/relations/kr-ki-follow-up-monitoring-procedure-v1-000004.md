---
type: SOP Knowledge Relation
title: kr-ki-follow-up-monitoring-procedure-v1-000004
description: Dose Titration should Clinicians should defer dose escalation when severe
  gastrointestinal symptoms.
resource: ../knowledge/ki-follow-up-monitoring-procedure-v1-000004.md
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
    id: kr-ki-follow-up-monitoring-procedure-v1-000004
    type: Knowledge Relation
    subject:
      id: concept-dose-titration
      label: Dose Titration
      text: Dose Titration
      okf_path: concepts/concept-dose-titration.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-clinicians-should-defer-dose-escalation-when-severe-gastrointest
      text: Clinicians should defer dose escalation when severe gastrointestinal symptoms
        are reported.
      label: Clinicians should defer dose escalation when severe gastrointestinal
        symptoms
    knowledge_piece_id: ki-follow-up-monitoring-procedure-v1-000004
    evidence_id: evidence-ki-follow-up-monitoring-procedure-v1-000004
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-follow-up-monitoring-procedure-v1-000004

## Assertion

- Subject: [Dose Titration](../concepts/concept-dose-titration.md)
- Predicate: `should`
- Object: Clinicians should defer dose escalation when severe gastrointestinal symptoms are reported.

## Connected Knowledge

- Knowledge piece: [ki-follow-up-monitoring-procedure-v1-000004](../knowledge/ki-follow-up-monitoring-procedure-v1-000004.md)
- Evidence: [evidence-ki-follow-up-monitoring-procedure-v1-000004](../evidence/evidence-ki-follow-up-monitoring-procedure-v1-000004.md)
- Decision rule: [Dose Titration](../rules/rule-ki-follow-up-monitoring-procedure-v1-000004-should.md)
