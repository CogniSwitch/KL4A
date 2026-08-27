---
type: SOP Knowledge Relation
title: kr-ki-safety-crlf-v1-000002
description: Escalation Procedure should Clinicians should route urgent cases to the
  on-call physician.
resource: ../knowledge/ki-safety-crlf-v1-000002.md
tags:
- relation
- rdf-compatible
- should
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
    id: kr-ki-safety-crlf-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-escalation-procedure
      label: Escalation Procedure
      text: Escalation Procedure
      okf_path: concepts/concept-escalation-procedure.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-clinicians-should-route-urgent-cases-to-the-on-call-physician
      text: Clinicians should route urgent cases to the on-call physician immediately.
      label: Clinicians should route urgent cases to the on-call physician
    knowledge_piece_id: ki-safety-crlf-v1-000002
    evidence_id: evidence-ki-safety-crlf-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-safety-crlf-v1-000002

## Assertion

- Subject: [Escalation Procedure](../concepts/concept-escalation-procedure.md)
- Predicate: `should`
- Object: Clinicians should route urgent cases to the on-call physician immediately.

## Connected Knowledge

- Knowledge piece: [ki-safety-crlf-v1-000002](../knowledge/ki-safety-crlf-v1-000002.md)
- Evidence: [evidence-ki-safety-crlf-v1-000002](../evidence/evidence-ki-safety-crlf-v1-000002.md)
- Decision rule: [Escalation Procedure](../rules/rule-ki-safety-crlf-v1-000002-should.md)
