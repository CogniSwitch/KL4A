---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-preamble-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-preamble
  title: preamble
  resource: ../sources/preamble.md
sopkb:
  rule:
    id: rule-ki-preamble-v1-000001-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-preamble-v1-000001
    source_id: preamble
    section_id: section-preamble-001
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-preamble-v1-000001
    relation_id: kr-ki-preamble-v1-000001
    okf_path: rules/rule-ki-preamble-v1-000001-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-preamble-v1-000001.md
  knowledge_relation: ../relations/kr-ki-preamble-v1-000001.md
  evidence: ../evidence/evidence-ki-preamble-v1-000001.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-preamble-v1-000001](../knowledge/ki-preamble-v1-000001.md)
- Knowledge relation: [kr-ki-preamble-v1-000001](../relations/kr-ki-preamble-v1-000001.md)
- Evidence: [evidence-ki-preamble-v1-000001](../evidence/evidence-ki-preamble-v1-000001.md)
