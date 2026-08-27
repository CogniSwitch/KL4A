---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-item-v1-000003.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-item
  title: "\u5317\u4EAC"
  resource: ../sources/item.md
sopkb:
  rule:
    id: rule-ki-item-v1-000003-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-item-v1-000003
    source_id: item
    section_id: section-item-002
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-item-v1-000003
    relation_id: kr-ki-item-v1-000003
    okf_path: rules/rule-ki-item-v1-000003-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-item-v1-000003.md
  knowledge_relation: ../relations/kr-ki-item-v1-000003.md
  evidence: ../evidence/evidence-ki-item-v1-000003.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-item-v1-000003](../knowledge/ki-item-v1-000003.md)
- Knowledge relation: [kr-ki-item-v1-000003](../relations/kr-ki-item-v1-000003.md)
- Evidence: [evidence-ki-item-v1-000003](../evidence/evidence-ki-item-v1-000003.md)
