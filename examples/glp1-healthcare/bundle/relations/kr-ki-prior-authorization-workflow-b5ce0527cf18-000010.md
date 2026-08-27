---
type: SOP Knowledge Relation
title: kr-ki-prior-authorization-workflow-b5ce0527cf18-000010
description: Prior Authorization Requirements records Staff must record payer requirements
  before submitting a GLP-1.
resource: ../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000010.md
tags:
- relation
- rdf-compatible
- records
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
    id: kr-ki-prior-authorization-workflow-b5ce0527cf18-000010
    type: Knowledge Relation
    subject:
      id: concept-prior-authorization-requirements
      label: Prior Authorization Requirements
      text: Prior Authorization Requirements
      okf_path: concepts/concept-prior-authorization-requirements.md
    predicate:
      id: predicate-records
      text: records
    object:
      id: object-staff-must-record-payer-requirements-before-submitting-a-glp-1
      text: Staff must record payer requirements before submitting a GLP-1 prior authorization
        request.
      label: Staff must record payer requirements before submitting a GLP-1
    knowledge_piece_id: ki-prior-authorization-workflow-b5ce0527cf18-000010
    evidence_id: evidence-ki-prior-authorization-workflow-b5ce0527cf18-000010
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-prior-authorization-workflow-b5ce0527cf18-000010

## Assertion

- Subject: [Prior Authorization Requirements](../concepts/concept-prior-authorization-requirements.md)
- Predicate: `records`
- Object: Staff must record payer requirements before submitting a GLP-1 prior authorization request.

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-b5ce0527cf18-000010](../knowledge/ki-prior-authorization-workflow-b5ce0527cf18-000010.md)
- Evidence: [evidence-ki-prior-authorization-workflow-b5ce0527cf18-000010](../evidence/evidence-ki-prior-authorization-workflow-b5ce0527cf18-000010.md)
- Decision rule: [Prior Authorization Requirements](../rules/rule-ki-prior-authorization-workflow-b5ce0527cf18-000010-records.md)
