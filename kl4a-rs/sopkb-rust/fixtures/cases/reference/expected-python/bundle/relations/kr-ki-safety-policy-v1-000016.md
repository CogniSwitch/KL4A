---
type: SOP Knowledge Relation
title: kr-ki-safety-policy-v1-000016
description: Adverse Event Monitoring should Clinicians should record severe gastrointestinal
  symptoms, dehydration risk, and.
resource: ../knowledge/ki-safety-policy-v1-000016.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-safety-policy
  title: safety policy
  resource: ../sources/safety-policy.md
sopkb:
  relation:
    id: kr-ki-safety-policy-v1-000016
    type: Knowledge Relation
    subject:
      id: concept-adverse-event-monitoring
      label: Adverse Event Monitoring
      text: Adverse Event Monitoring
      okf_path: concepts/concept-adverse-event-monitoring.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-clinicians-should-record-severe-gastrointestinal-symptoms-dehydr
      text: Clinicians should record severe gastrointestinal symptoms, dehydration
        risk, and suspected pancreatitis symptoms.
      label: Clinicians should record severe gastrointestinal symptoms, dehydration
        risk, and
    knowledge_piece_id: ki-safety-policy-v1-000016
    evidence_id: evidence-ki-safety-policy-v1-000016
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-safety-policy-v1-000016

## Assertion

- Subject: [Adverse Event Monitoring](../concepts/concept-adverse-event-monitoring.md)
- Predicate: `should`
- Object: Clinicians should record severe gastrointestinal symptoms, dehydration risk, and suspected pancreatitis symptoms.

## Connected Knowledge

- Knowledge piece: [ki-safety-policy-v1-000016](../knowledge/ki-safety-policy-v1-000016.md)
- Evidence: [evidence-ki-safety-policy-v1-000016](../evidence/evidence-ki-safety-policy-v1-000016.md)
- Decision rule: [Adverse Event Monitoring](../rules/rule-ki-safety-policy-v1-000016-should.md)
