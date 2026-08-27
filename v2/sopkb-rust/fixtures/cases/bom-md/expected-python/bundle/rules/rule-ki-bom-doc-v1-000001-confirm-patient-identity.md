---
type: SOP Decision Rule
title: Confirm patient identity
description: Patient identity confirmed
resource: ../knowledge/ki-bom-doc-v1-000001.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.1.0
  date: <TS>
sources:
- id: src-bom-doc
  title: bom doc
  resource: ../sources/bom-doc.md
sopkb:
  rule:
    id: rule-ki-bom-doc-v1-000001-confirm-patient-identity
    type: SOP Decision Rule
    title: Confirm patient identity
    knowledge_item_id: ki-bom-doc-v1-000001
    source_id: bom-doc
    section_id: section-bom-doc-001
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: patient_identity_confirmed
      action: confirm
      label: Patient identity confirmed
    evidence_id: evidence-ki-bom-doc-v1-000001
    relation_id: kr-ki-bom-doc-v1-000001
    okf_path: rules/rule-ki-bom-doc-v1-000001-confirm-patient-identity.md
  knowledge_piece: ../knowledge/ki-bom-doc-v1-000001.md
  knowledge_relation: ../relations/kr-ki-bom-doc-v1-000001.md
  evidence: ../evidence/evidence-ki-bom-doc-v1-000001.md
---
# Confirm patient identity

## Rule

- Condition: always applies
- Obligation: `patient_identity_confirmed`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-bom-doc-v1-000001](../knowledge/ki-bom-doc-v1-000001.md)
- Knowledge relation: [kr-ki-bom-doc-v1-000001](../relations/kr-ki-bom-doc-v1-000001.md)
- Evidence: [evidence-ki-bom-doc-v1-000001](../evidence/evidence-ki-bom-doc-v1-000001.md)
