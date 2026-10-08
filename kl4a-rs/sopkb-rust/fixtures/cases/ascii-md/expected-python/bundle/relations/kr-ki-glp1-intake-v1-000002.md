---
type: SOP Knowledge Relation
title: kr-ki-glp1-intake-v1-000002
description: Procedure should Staff should record contraindications and route uncertain
  cases for.
resource: ../knowledge/ki-glp1-intake-v1-000002.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-glp1-intake
  title: glp1 intake
  resource: ../sources/glp1-intake.md
sopkb:
  relation:
    id: kr-ki-glp1-intake-v1-000002
    type: Knowledge Relation
    subject:
      id: concept-procedure
      label: Procedure
      text: Procedure
      okf_path: concepts/concept-procedure.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-staff-should-record-contraindications-and-route-uncertain-cases-
      text: Staff should record contraindications and route uncertain cases for clinical
        review.
      label: Staff should record contraindications and route uncertain cases for
    knowledge_piece_id: ki-glp1-intake-v1-000002
    evidence_id: evidence-ki-glp1-intake-v1-000002
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-glp1-intake-v1-000002

## Assertion

- Subject: [Procedure](../concepts/concept-procedure.md)
- Predicate: `should`
- Object: Staff should record contraindications and route uncertain cases for clinical review.

## Connected Knowledge

- Knowledge piece: [ki-glp1-intake-v1-000002](../knowledge/ki-glp1-intake-v1-000002.md)
- Evidence: [evidence-ki-glp1-intake-v1-000002](../evidence/evidence-ki-glp1-intake-v1-000002.md)
- Decision rule: [Record contraindications](../rules/rule-ki-glp1-intake-v1-000002-record-contraindications.md)
- Decision rule: [Route uncertain cases for clinical review](../rules/rule-ki-glp1-intake-v1-000002-route-uncertain-case.md)
