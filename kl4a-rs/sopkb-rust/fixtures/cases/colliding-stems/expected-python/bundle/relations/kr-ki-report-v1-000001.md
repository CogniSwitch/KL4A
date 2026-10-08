---
type: SOP Knowledge Relation
title: kr-ki-report-v1-000001
description: Report requires Staff must confirm the report contents before filing..
resource: ../knowledge/ki-report-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-report
  title: report
  resource: ../sources/report.md
sopkb:
  relation:
    id: kr-ki-report-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-report
      label: Report
      text: Report
      okf_path: concepts/concept-report.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-staff-must-confirm-the-report-contents-before-filing
      text: Staff must confirm the report contents before filing.
      label: Staff must confirm the report contents before filing.
    knowledge_piece_id: ki-report-v1-000001
    evidence_id: evidence-ki-report-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-report-v1-000001

## Assertion

- Subject: [Report](../concepts/concept-report.md)
- Predicate: `requires`
- Object: Staff must confirm the report contents before filing.

## Connected Knowledge

- Knowledge piece: [ki-report-v1-000001](../knowledge/ki-report-v1-000001.md)
- Evidence: [evidence-ki-report-v1-000001](../evidence/evidence-ki-report-v1-000001.md)
- Decision rule: [Report](../rules/rule-ki-report-v1-000001-requires.md)
