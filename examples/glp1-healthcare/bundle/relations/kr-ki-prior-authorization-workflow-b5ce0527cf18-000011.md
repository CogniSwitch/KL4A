---
type: SOP Knowledge Relation
title: kr-ki-prior-authorization-workflow-b5ce0527cf18-000011
description: Prior Authorization Requirements requires Staff must attach diagnosis
  evidence and prior therapy documentation.
resource: ../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000011.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-prior-authorization-workflow-b5ce0527cf18
  title: prior-authorization-workflow-b5ce0527cf18
  resource: ../sources/prior-authorization-workflow-b5ce0527cf18.md
sopkb:
  relation:
    id: kr-ki-prior-authorization-workflow-b5ce0527cf18-000011
    type: Knowledge Relation
    subject:
      id: concept-prior-authorization-requirements
      label: Prior Authorization Requirements
      text: Prior Authorization Requirements
      okf_path: concepts/concept-prior-authorization-requirements.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-attach-diagnosis-evidence-and-prior-therapy-documenta
      text: Staff must attach diagnosis evidence and prior therapy documentation when
        required by the payer.
      label: Staff must attach diagnosis evidence and prior therapy documentation
    knowledge_piece_id: ki-prior-authorization-workflow-b5ce0527cf18-000011
    evidence_id: evidence-ki-prior-authorization-workflow-b5ce0527cf18-000011
    review_status: rejected
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-prior-authorization-workflow-b5ce0527cf18-000011

## Assertion

- Subject: [Prior Authorization Requirements](../concepts/concept-prior-authorization-requirements.md)
- Predicate: `requires`
- Object: Staff must attach diagnosis evidence and prior therapy documentation when required by the payer.

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-b5ce0527cf18-000011](../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000011.md)
- Evidence: [evidence-ki-prior-authorization-workflow-b5ce0527cf18-000011](../evidence/evidence-ki-prior-authorization-workflow-b5ce0527cf18-000011.md)
- Decision rule: [Prior Authorization Requirements](../rules/rule-ki-prior-authorization-workflow-b5ce0527cf18-000011-requires.md)
