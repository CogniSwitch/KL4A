---
type: SOP Knowledge Relation
title: kr-ki-primary-care-glp1-sop-v1-000008
description: Contraindication Screening should Patients with uncertain contraindication
  history should be routed for.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000008.md
tags:
- relation
- rdf-compatible
- should
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-primary-care-glp1-sop
  title: primary care glp1 sop
  resource: ../sources/primary-care-glp1-sop.md
sopkb:
  relation:
    id: kr-ki-primary-care-glp1-sop-v1-000008
    type: Knowledge Relation
    subject:
      id: concept-contraindication-screening
      label: Contraindication Screening
      text: Contraindication Screening
      okf_path: concepts/concept-contraindication-screening.md
    predicate:
      id: predicate-should
      text: should
    object:
      id: object-patients-with-uncertain-contraindication-history-should-be-route
      text: Patients with uncertain contraindication history should be routed for
        clinical review before therapy is prescribed.
      label: Patients with uncertain contraindication history should be routed for
    knowledge_piece_id: ki-primary-care-glp1-sop-v1-000008
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000008
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-primary-care-glp1-sop-v1-000008

## Assertion

- Subject: [Contraindication Screening](../concepts/concept-contraindication-screening.md)
- Predicate: `should`
- Object: Patients with uncertain contraindication history should be routed for clinical review before therapy is prescribed.

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000008](../knowledge/ki-primary-care-glp1-sop-v1-000008.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000008](../evidence/evidence-ki-primary-care-glp1-sop-v1-000008.md)
- Decision rule: [Route uncertain cases for clinical review](../rules/rule-ki-primary-care-glp1-sop-v1-000008-route-uncertain-case.md)
