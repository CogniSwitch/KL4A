---
type: SOP Knowledge Relation
title: kr-ki-safety-policy-01ff452b1d44-000015
description: Contraindication Screening requires Patients with confirmed contraindications
  must not start GLP-1 therapy.
resource: ../knowledge/ki-safety-policy-01ff452b1d44-000015.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-safety-policy-01ff452b1d44
  title: safety-policy-01ff452b1d44
  resource: ../sources/safety-policy-01ff452b1d44.md
sopkb:
  relation:
    id: kr-ki-safety-policy-01ff452b1d44-000015
    type: Knowledge Relation
    subject:
      id: concept-contraindication-screening
      label: Contraindication Screening
      text: Contraindication Screening
      okf_path: concepts/concept-contraindication-screening.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-patients-with-confirmed-contraindications-must-not-start-glp-1-t
      text: Patients with confirmed contraindications must not start GLP-1 therapy
        without specialist review.
      label: Patients with confirmed contraindications must not start GLP-1 therapy
    knowledge_piece_id: ki-safety-policy-01ff452b1d44-000015
    evidence_id: evidence-ki-safety-policy-01ff452b1d44-000015
    review_status: deferred
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-safety-policy-01ff452b1d44-000015

## Assertion

- Subject: [Contraindication Screening](../concepts/concept-contraindication-screening.md)
- Predicate: `requires`
- Object: Patients with confirmed contraindications must not start GLP-1 therapy without specialist review.

## Connected Knowledge

- Knowledge piece: [ki-safety-policy-01ff452b1d44-000015](../knowledge/ki-safety-policy-01ff452b1d44-000015.md)
- Evidence: [evidence-ki-safety-policy-01ff452b1d44-000015](../evidence/evidence-ki-safety-policy-01ff452b1d44-000015.md)
- Decision rule: [Contraindication Screening](../rules/rule-ki-safety-policy-01ff452b1d44-000015-requires.md)
