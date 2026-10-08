---
type: SOP Knowledge Relation
title: kr-ki-prior-authorization-workflow-v1-000012
description: Denial Routing should Staff should route prior authorization denials
  to the prescribing.
resource: ../knowledge/ki-prior-authorization-workflow-v1-000012.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-prior-authorization-workflow
  title: prior authorization workflow
  resource: ../sources/prior-authorization-workflow.md
sopkb:
  relation:
    id: kr-ki-prior-authorization-workflow-v1-000012
    type: Knowledge Relation
    subject:
      id: concept-denial-routing
      label: Denial Routing
      text: Denial Routing
      okf_path: concepts/concept-denial-routing.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-staff-should-route-prior-authorization-denials-to-the-prescribin
      text: Staff should route prior authorization denials to the prescribing clinician
        for clinical review.
      label: Staff should route prior authorization denials to the prescribing
    knowledge_piece_id: ki-prior-authorization-workflow-v1-000012
    evidence_id: evidence-ki-prior-authorization-workflow-v1-000012
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-prior-authorization-workflow-v1-000012

## Assertion

- Subject: [Denial Routing](../concepts/concept-denial-routing.md)
- Predicate: `should`
- Object: Staff should route prior authorization denials to the prescribing clinician for clinical review.

## Connected Knowledge

- Knowledge piece: [ki-prior-authorization-workflow-v1-000012](../knowledge/ki-prior-authorization-workflow-v1-000012.md)
- Evidence: [evidence-ki-prior-authorization-workflow-v1-000012](../evidence/evidence-ki-prior-authorization-workflow-v1-000012.md)
- Decision rule: [Denial Routing](../rules/rule-ki-prior-authorization-workflow-v1-000012-should.md)
