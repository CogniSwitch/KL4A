---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-primary-care-glp1-sop-v1-000005.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-primary-care-glp1-sop
  title: primary care glp1 sop
  resource: ../sources/primary-care-glp1-sop.md
sopkb:
  rule:
    id: rule-ki-primary-care-glp1-sop-v1-000005-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-primary-care-glp1-sop-v1-000005
    source_id: primary-care-glp1-sop
    section_id: section-primary-care-glp1-sop-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-primary-care-glp1-sop-v1-000005
    relation_id: kr-ki-primary-care-glp1-sop-v1-000005
    okf_path: rules/rule-ki-primary-care-glp1-sop-v1-000005-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-primary-care-glp1-sop-v1-000005.md
  knowledge_relation: ../relations/kr-ki-primary-care-glp1-sop-v1-000005.md
  evidence: ../evidence/evidence-ki-primary-care-glp1-sop-v1-000005.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-primary-care-glp1-sop-v1-000005](../knowledge/ki-primary-care-glp1-sop-v1-000005.md)
- Knowledge relation: [kr-ki-primary-care-glp1-sop-v1-000005](../relations/kr-ki-primary-care-glp1-sop-v1-000005.md)
- Evidence: [evidence-ki-primary-care-glp1-sop-v1-000005](../evidence/evidence-ki-primary-care-glp1-sop-v1-000005.md)
