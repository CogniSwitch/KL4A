---
type: SOP Knowledge Relation
title: kr-ki-primary-care-glp1-sop-v1-000007
description: Contraindication Screening requires Clinicians must screen for pregnancy,
  pancreatitis history, medullary thyroid.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000007.md
tags:
- relation
- rdf-compatible
- requires
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
    id: kr-ki-primary-care-glp1-sop-v1-000007
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
      id: object-clinicians-must-screen-for-pregnancy-pancreatitis-history-medull
      text: Clinicians must screen for pregnancy, pancreatitis history, medullary
        thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.
      label: Clinicians must screen for pregnancy, pancreatitis history, medullary
        thyroid
    knowledge_piece_id: ki-primary-care-glp1-sop-v1-000007
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000007
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-primary-care-glp1-sop-v1-000007

## Assertion

- Subject: [Contraindication Screening](../concepts/concept-contraindication-screening.md)
- Predicate: `requires`
- Object: Clinicians must screen for pregnancy, pancreatitis history, medullary thyroid carcinoma history, and multiple endocrine neoplasia type 2 history.

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000007](../knowledge/ki-primary-care-glp1-sop-v1-000007.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000007](../evidence/evidence-ki-primary-care-glp1-sop-v1-000007.md)
- Decision rule: [Contraindication Screening](../rules/rule-ki-primary-care-glp1-sop-v1-000007-requires.md)
