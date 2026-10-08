---
type: SOP Decision Rule
title: 5. Prescription Eyewear
description: Prescription Eyewear must include original packaging and all accessories
  to avoid a 0% restocking fee.
resource: ../knowledge/ki-category-policy-v1-000010.md
tags:
- decision-rule
- proposed
status: draft
generated:
  actor: sopkb/0.0.2
  date: '2026-10-07'
sources:
- id: src-category-policy
  title: category policy
  resource: ../sources/category-policy.md
sopkb:
  rule:
    id: rule-ki-category-policy-v1-000010-requires
    type: SOP Decision Rule
    title: 5. Prescription Eyewear
    knowledge_item_id: ki-category-policy-v1-000010
    source_id: category-policy
    section_id: section-category-policy-006
    review_status: proposed
    confidence: 0.82
    condition: null
    obligation:
      fact: scenario_mentions_5_prescription_eyewear
      action: requires
      label: Prescription Eyewear must include original packaging and all accessories
        to avoid a 0% restocking fee.
    evidence_id: evidence-ki-category-policy-v1-000010
    relation_id: kr-ki-category-policy-v1-000010
    okf_path: rules/rule-ki-category-policy-v1-000010-requires.md
  knowledge_piece: ../knowledge/ki-category-policy-v1-000010.md
  knowledge_relation: ../relations/kr-ki-category-policy-v1-000010.md
  evidence: ../evidence/evidence-ki-category-policy-v1-000010.md
---
# 5. Prescription Eyewear

## Rule

- Condition: always applies
- Obligation: `scenario_mentions_5_prescription_eyewear`
- Review status: `proposed`

## Connected Knowledge

- Knowledge piece: [ki-category-policy-v1-000010](../knowledge/ki-category-policy-v1-000010.md)
- Knowledge relation: [kr-ki-category-policy-v1-000010](../relations/kr-ki-category-policy-v1-000010.md)
- Evidence: [evidence-ki-category-policy-v1-000010](../evidence/evidence-ki-category-policy-v1-000010.md)
