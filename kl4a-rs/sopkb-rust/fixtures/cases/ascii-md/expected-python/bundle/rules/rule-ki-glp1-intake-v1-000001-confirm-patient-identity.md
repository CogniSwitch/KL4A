---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-glp1-intake-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-glp1-intake
  title: glp1 intake
  resource: ../sources/glp1-intake.md
sopkb:
  rule:
    id: rule-ki-glp1-intake-v1-000001-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-glp1-intake-v1-000001
    source_id: glp1-intake
    section_id: section-glp1-intake-003
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-glp1-intake-v1-000001
    relation_id: kr-ki-glp1-intake-v1-000001
    okf_path: rules/rule-ki-glp1-intake-v1-000001-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-glp1-intake-v1-000001.md
  knowledge_relation: ../relations/kr-ki-glp1-intake-v1-000001.md
  evidence: ../evidence/evidence-ki-glp1-intake-v1-000001.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-glp1-intake-v1-000001](../knowledge/ki-glp1-intake-v1-000001.md)
- Knowledge relation: [kr-ki-glp1-intake-v1-000001](../relations/kr-ki-glp1-intake-v1-000001.md)
- Evidence: [evidence-ki-glp1-intake-v1-000001](../evidence/evidence-ki-glp1-intake-v1-000001.md)
