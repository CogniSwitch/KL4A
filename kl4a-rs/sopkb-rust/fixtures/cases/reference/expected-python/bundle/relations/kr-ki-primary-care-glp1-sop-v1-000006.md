---
type: SOP Knowledge Relation
title: kr-ki-primary-care-glp1-sop-v1-000006
description: Intake Requirements requires Clinicians must document current medications,
  diabetes history, weight history.
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000006.md
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
    id: kr-ki-primary-care-glp1-sop-v1-000006
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
      id: object-clinicians-must-document-current-medications-diabetes-history-we
      text: Clinicians must document current medications, diabetes history, weight
        history, and relevant comorbidities.
      label: Clinicians must document current medications, diabetes history, weight
        history
    knowledge_piece_id: ki-primary-care-glp1-sop-v1-000006
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000006
    review_status: proposed
    confidence: 0.82
    rdf_compatible: true
---
# kr-ki-primary-care-glp1-sop-v1-000006

## Assertion

- Subject: [Intake Requirements](../concepts/concept-intake-requirements.md)
- Predicate: `requires`
- Object: Clinicians must document current medications, diabetes history, weight history, and relevant comorbidities.

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000006](../knowledge/ki-primary-care-glp1-sop-v1-000006.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000006](../evidence/evidence-ki-primary-care-glp1-sop-v1-000006.md)
- Decision rule: [Intake Requirements](../rules/rule-ki-primary-care-glp1-sop-v1-000006-requires.md)
