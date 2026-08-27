---
type: SOP Knowledge Relation
title: kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005
description: Intake Requirements requires Clinicians must confirm patient identity
  before reviewing GLP-1 therapy.
resource: ../knowledge/ki-primary-care-glp1-sop-c6cb3b4ec11d-000005.md
tags:
- relation
- rdf-compatible
- requires
status: stable
generated:
  actor: sopkb/0.1.0
  date: '2026-08-10'
sources:
- id: src-primary-care-glp1-sop-c6cb3b4ec11d
  title: primary-care-glp1-sop-c6cb3b4ec11d
  resource: ../sources/primary-care-glp1-sop-c6cb3b4ec11d.md
sopkb:
  relation:
    id: kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005
    type: Knowledge Relation
    subject:
      id: concept-intake-requirements
      label: Intake Requirements
      text: Intake Requirements
      okf_path: concepts/concept-intake-requirements.md
    predicate:
      id: predicate-requires
      text: requires
    object:
      id: object-clinicians-must-confirm-patient-identity-before-reviewing-glp-1-
      text: Clinicians must confirm patient identity before reviewing GLP-1 therapy
        eligibility.
      label: Clinicians must confirm patient identity before reviewing GLP-1 therapy
    knowledge_piece_id: ki-primary-care-glp1-sop-c6cb3b4ec11d-000005
    evidence_id: evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005
    review_status: approved
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005

## Assertion

- Subject: [Intake Requirements](../concepts/concept-intake-requirements.md)
- Predicate: `requires`
- Object: Clinicians must confirm patient identity before reviewing GLP-1 therapy eligibility.

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-c6cb3b4ec11d-000005](../knowledge/ki-primary-care-glp1-sop-c6cb3b4ec11d-000005.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005](../evidence/evidence-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005.md)
- Decision rule: [Confirm patient identity](../rules/rule-ki-primary-care-glp1-sop-c6cb3b4ec11d-000005-confirm-patient-identity.md)
