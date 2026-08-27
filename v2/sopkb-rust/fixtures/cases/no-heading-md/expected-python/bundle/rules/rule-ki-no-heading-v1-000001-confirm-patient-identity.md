---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-no-heading-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-no-heading
  title: no heading
  resource: ../sources/no-heading.md
sopkb:
  rule:
    id: rule-ki-no-heading-v1-000001-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-no-heading-v1-000001
    source_id: no-heading
    section_id: section-no-heading-001
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-no-heading-v1-000001
    relation_id: kr-ki-no-heading-v1-000001
    okf_path: rules/rule-ki-no-heading-v1-000001-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-no-heading-v1-000001.md
  knowledge_relation: ../relations/kr-ki-no-heading-v1-000001.md
  evidence: ../evidence/evidence-ki-no-heading-v1-000001.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-no-heading-v1-000001](../knowledge/ki-no-heading-v1-000001.md)
- Knowledge relation: [kr-ki-no-heading-v1-000001](../relations/kr-ki-no-heading-v1-000001.md)
- Evidence: [evidence-ki-no-heading-v1-000001](../evidence/evidence-ki-no-heading-v1-000001.md)
