---
type: SOP Knowledge Relation
title: kr-ki-safety-crlf-v1-000001
description: Reporting Requirements records Staff must record all adverse events within
  24 hours..
resource: ../knowledge/ki-safety-crlf-v1-000001.md
tags:
- relation
- rdf-compatible
- records
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-safety-crlf
  title: safety crlf
  resource: ../sources/safety-crlf.md
sopkb:
  relation:
    id: kr-ki-safety-crlf-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-reporting-requirements
      label: Reporting Requirements
      text: Reporting Requirements
      okf_path: concepts/concept-reporting-requirements.md
    predicate:
      id: predicate-records
      text: records
    object:
      id: object-staff-must-record-all-adverse-events-within-24-hours
      text: Staff must record all adverse events within 24 hours.
      label: Staff must record all adverse events within 24 hours.
    knowledge_piece_id: ki-safety-crlf-v1-000001
    evidence_id: evidence-ki-safety-crlf-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-safety-crlf-v1-000001

## Assertion

- Subject: [Reporting Requirements](../concepts/concept-reporting-requirements.md)
- Predicate: `records`
- Object: Staff must record all adverse events within 24 hours.

## Connected Knowledge

- Knowledge piece: [ki-safety-crlf-v1-000001](../knowledge/ki-safety-crlf-v1-000001.md)
- Evidence: [evidence-ki-safety-crlf-v1-000001](../evidence/evidence-ki-safety-crlf-v1-000001.md)
- Decision rule: [Reporting Requirements](../rules/rule-ki-safety-crlf-v1-000001-records.md)
