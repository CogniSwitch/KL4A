---
type: SOP Knowledge Relation
title: kr-ki-preamble-v1-000001
description: Eligibility Requirements requires Clinicians must confirm patient identity
  before reviewing therapy eligibility..
resource: ../knowledge/ki-preamble-v1-000001.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-preamble
  title: preamble
  resource: ../sources/preamble.md
sopkb:
  relation:
    id: kr-ki-preamble-v1-000001
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
      id: object-clinicians-must-confirm-patient-identity-before-reviewing-therap
      text: Clinicians must confirm patient identity before reviewing therapy eligibility.
      label: Clinicians must confirm patient identity before reviewing therapy eligibility.
    knowledge_piece_id: ki-preamble-v1-000001
    evidence_id: evidence-ki-preamble-v1-000001
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-preamble-v1-000001

## Assertion

- Subject: [Eligibility Requirements](../concepts/concept-eligibility-requirements.md)
- Predicate: `requires`
- Object: Clinicians must confirm patient identity before reviewing therapy eligibility.

## Connected Knowledge

- Knowledge piece: [ki-preamble-v1-000001](../knowledge/ki-preamble-v1-000001.md)
- Evidence: [evidence-ki-preamble-v1-000001](../evidence/evidence-ki-preamble-v1-000001.md)
- Decision rule: [Confirm patient identity](../rules/rule-ki-preamble-v1-000001-confirm-patient-identity.md)
