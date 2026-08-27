---
type: SOP Knowledge Relation
title: kr-ki-safety-policy-v1-000014
description: Contraindication Screening requires Clinicians must confirm contraindication
  screening before prescribing GLP-1 therapy..
resource: ../knowledge/ki-safety-policy-v1-000014.md
tags:
- relation
- rdf-compatible
- requires
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
    id: kr-ki-safety-policy-v1-000014
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
      id: object-clinicians-must-confirm-contraindication-screening-before-prescr
      text: Clinicians must confirm contraindication screening before prescribing
        GLP-1 therapy.
      label: Clinicians must confirm contraindication screening before prescribing
        GLP-1 therapy.
    knowledge_piece_id: ki-safety-policy-v1-000014
    evidence_id: evidence-ki-safety-policy-v1-000014
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-safety-policy-v1-000014

## Assertion

- Subject: [Contraindication Screening](../concepts/concept-contraindication-screening.md)
- Predicate: `requires`
- Object: Clinicians must confirm contraindication screening before prescribing GLP-1 therapy.

## Connected Knowledge

- Knowledge piece: [ki-safety-policy-v1-000014](../knowledge/ki-safety-policy-v1-000014.md)
- Evidence: [evidence-ki-safety-policy-v1-000014](../evidence/evidence-ki-safety-policy-v1-000014.md)
- Decision rule: [Contraindication Screening](../rules/rule-ki-safety-policy-v1-000014-requires.md)
