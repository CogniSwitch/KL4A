---
type: SOP Knowledge Relation
title: kr-ki-vendor-onboarding-v1-000001
description: Eligibility Requirements requires A vendor must hold a valid tax identification
  number.
resource: ../knowledge/ki-vendor-onboarding-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-vendor-onboarding
  title: vendor onboarding
  resource: ../sources/vendor-onboarding.md
sopkb:
  relation:
    id: kr-ki-vendor-onboarding-v1-000001
    type: Knowledge Relation
    subject:
      id: concept-eligibility-requirements
      label: Eligibility Requirements
      text: Eligibility Requirements
      okf_path: concepts/concept-eligibility-requirements.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-a-vendor-must-hold-a-valid-tax-identification-number
      text: A vendor must hold a valid tax identification number and a signed NDA
        on file.
      label: A vendor must hold a valid tax identification number
    knowledge_piece_id: ki-vendor-onboarding-v1-000001
    evidence_id: evidence-ki-vendor-onboarding-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-vendor-onboarding-v1-000001

## Assertion

- Subject: [Eligibility Requirements](../concepts/concept-eligibility-requirements.md)
- Predicate: `requires`
- Object: A vendor must hold a valid tax identification number and a signed NDA on file.

## Connected Knowledge

- Knowledge piece: [ki-vendor-onboarding-v1-000001](../knowledge/ki-vendor-onboarding-v1-000001.md)
- Evidence: [evidence-ki-vendor-onboarding-v1-000001](../evidence/evidence-ki-vendor-onboarding-v1-000001.md)
- Decision rule: [Eligibility Requirements](../rules/rule-ki-vendor-onboarding-v1-000001-requires.md)
